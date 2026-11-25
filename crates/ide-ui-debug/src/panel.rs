//! The debugger of one window: its model, the views of the panel and the inspector, and the AppKit callbacks of both
//! lists, which hand their events to the model (E98); `controller.rs` carries out what it returns.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSTextField, NSTextFieldDelegate, NSTextView, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};

use super::model::Model;
use super::model::rows::Key;
use super::model::variables::Row;
use super::ui::Ui;
use ide_ui_common::panel::{KeptCells, RowItem, RowItems};
use ide_ui_common::registry::WindowPart;
use ide_ui_common::ui_size;
use ide_ui_common::util::{guarded, ns};
use ide_ui_common::window::{Inspector, Panels, Trust, Window};

/// A row whose rows below a list asks for, `None` for the top, in the inspector (`true`) or the panel's list.
type Parent = (bool, Option<RowKey>);

pub struct PanelIvars {
    id: u64,
    window: Window,
    root: PathBuf,
    ui: OnceCell<Ui>,
    model: RefCell<Model>,
    /// The objects that stand for the rows of both lists, the same one for a key as long as its row exists.
    items: RowItems<RowKey>,
    /// The cells of both lists' rows, handed back while a row shows the same.
    pub(crate) cells: KeptCells<RowKey>,
    /// The rows below each row of either list, as the model has them now.
    children: RefCell<HashMap<Parent, Rc<[RowKey]>>>,
    /// Whether each row could open when its list last asked.
    pub(crate) expandable: RefCell<HashMap<RowKey, bool>>,
    /// What the lists show of the model that changed since they were last shown again.
    pub(crate) stale: Cell<super::actions::Stale>,
    /// The panel itself opens and closes rows, so the user's folding is not touched.
    pub(crate) unfolding: Cell<bool>,
    /// The running sessions and their consoles.
    pub(crate) links: RefCell<super::controller::Links>,
    /// Whether the project's launch configurations were read, which waits until the panel shows or a start needs them.
    pub(crate) configured: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `DebugPanel` does not implement Drop.
    /// The debugger panel of one window, the data source and delegate of its lists and the inspector's.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDebugPanel"]
    #[ivars = PanelIvars]
    pub struct DebugPanel;

    unsafe impl NSObjectProtocol for DebugPanel {}

    unsafe impl NSOutlineViewDataSource for DebugPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.child_keys(view, item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let children = self.child_keys(view, item);
                let key = children.get(usize::try_from(index).ok()?)?;
                Some(self.item_for(key))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || {
                let expands = self.expands(view, item);
                if let Some(key) = row_key(item) {
                    let mut asked = self.ivars().expandable.borrow_mut();
                    if asked.get(key) != Some(&expands) {
                        asked.insert(key.clone(), expands);
                    }
                }
                expands
            })
        }
    }

    unsafe impl NSControlTextEditingDelegate for DebugPanel {
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn do_command(&self, control: &NSControl, _text: &NSTextView, selector: Sel) -> bool {
            guarded(false, || {
                let Some(field) = control.downcast_ref::<NSTextField>() else {
                    return false;
                };
                if selector == sel!(moveUp:) {
                    self.console_key(field, true)
                } else if selector == sel!(moveDown:) {
                    self.console_key(field, false)
                } else {
                    false
                }
            })
        }
    }

    unsafe impl NSTextFieldDelegate for DebugPanel {}

    unsafe impl NSOutlineViewDelegate for DebugPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.row_view(view, item))
        }

        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn row_height(&self, _view: &NSOutlineView, item: &AnyObject) -> f64 {
            guarded(ui_size::scaled(24.0), || {
                let key = row_key(item);
                ui_size::scaled(if matches!(key, Some(RowKey::Panel(Key::Empty(_)) | RowKey::Inspector(Row::Empty(_)))) {
                    super::ui::NOTE_HEIGHT
                } else if matches!(key, Some(RowKey::Panel(Key::Session(_)))) {
                    super::ui::SESSION_HEIGHT
                } else {
                    24.0
                })
            })
        }

        #[unsafe(method(outlineView:shouldSelectItem:))]
        fn should_select(&self, view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || !self.is_section(view, item))
        }

        #[unsafe(method(outlineViewSelectionDidChange:))]
        fn selection_changed(&self, notification: &NSNotification) {
            guarded((), || self.selection_changed_in(notification));
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || self.note_expansion(notification, false));
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &NSNotification) {
            guarded((), || self.note_expansion(notification, true));
        }
    }

    unsafe impl NSMenuDelegate for DebugPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    impl DebugPanel {
        #[unsafe(method(ideDebugControl:))]
        fn control_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(command) = super::ui::command_of(sender) {
                    self.run_command(command);
                }
            });
        }

        #[unsafe(method(ideDebugStart:))]
        fn start_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.start_chosen());
        }

        #[unsafe(method(ideDebugChoose:))]
        fn choose_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.configuration_chosen());
        }

        #[unsafe(method(ideDebugFilter:))]
        fn filter_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.filter_switched(sender));
        }

        #[unsafe(method(ideDebugBreakpointSwitch:))]
        fn breakpoint_switch_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.breakpoint_switched(sender));
        }

        #[unsafe(method(ideDebugAddWatch:))]
        fn add_watch_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.watch_typed(sender));
        }

        #[unsafe(method(ideDebugRemoveWatch:))]
        fn remove_watch_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.remove_watch_of(sender));
        }

        #[unsafe(method(ideDebugEditBreakpoint:))]
        fn edit_breakpoint_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.edit_breakpoint_of(sender));
        }

        #[unsafe(method(ideDebugRemoveBreakpoint:))]
        fn remove_breakpoint_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.remove_breakpoint_of(sender));
        }

        #[unsafe(method(ideDebugDoubleClicked:))]
        fn double_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.ui().outline.clickedRow();
                self.activate_row(row);
            });
        }

        #[unsafe(method(ideDebugEvaluate:))]
        fn evaluate_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.console_typed(sender));
        }

        #[unsafe(method(ideDebugTrust:))]
        fn trust_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(controller) = self.controller() {
                    controller.ask_trust_again();
                }
            });
        }
    }
);

/// Runs `f` with the panel `id`, if its window is still open.
pub(crate) fn with_panel(id: u64, f: impl FnOnce(&DebugPanel)) {
    crate::host::state().panels.with(id, f);
}

/// The panel of `window`.
pub(crate) fn of(window: &Window) -> Option<Retained<DebugPanel>> {
    crate::host::state().panels.of(*window)
}

/// The panel of the window that shows `root`.
pub(crate) fn for_root(root: &Path) -> Option<Retained<DebugPanel>> {
    crate::host::state().panels.for_root(root).into_iter().next()
}

impl WindowPart for DebugPanel {
    fn window(&self) -> Option<Window> {
        self.controller()
    }

    fn root(&self) -> &Path {
        DebugPanel::root(self)
    }
}

impl DebugPanel {
    /// The debugger of `controller`'s window for the folder `root`, with what the project kept of debugging.
    pub(crate) fn attach(controller: &Window, root: PathBuf) -> Retained<DebugPanel> {
        let mtm = controller.mtm();
        let state = crate::host::state();
        let panels = &state.panels;
        if let Some(old) = panels.of(*controller) {
            old.end_all();
        }
        panels.detach(*controller);
        let panel = DebugPanel::new(mtm, controller, root, panels.next_id());
        panels.insert(panel.id(), panel.clone());
        controller.set_panel_page(ide_ui_common::window::Panel::Debug, &panel.ui().page);
        controller.set_panel_accessory(ide_ui_common::window::Panel::Debug, &panel.ui().header);
        controller.set_inspector_content(&panel.ui().inspector.page);
        panel.refresh();
        panel
    }

    fn new(mtm: MainThreadMarker, controller: &Window, root: PathBuf, id: u64) -> Retained<DebugPanel> {
        let mut model = Model::new(ide_dap::kept::load(&root));
        model.fit_filters(ide_dap::Languages::of(&root));
        let ivars = PanelIvars {
            id,
            window: *controller,
            root,
            ui: OnceCell::new(),
            model: RefCell::new(model),
            items: RowItems::default(),
            cells: KeptCells::default(),
            children: RefCell::new(HashMap::new()),
            expandable: RefCell::new(HashMap::new()),
            stale: Cell::new(super::actions::Stale::default()),
            unfolding: Cell::new(false),
            links: RefCell::new(super::controller::Links::default()),
            configured: Cell::new(false),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let panel: Retained<DebugPanel> = unsafe { msg_send![super(DebugPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = panel.ivars().ui.set(Ui::new(mtm, &panel));
        panel
    }

    pub(crate) fn id(&self) -> u64 {
        self.ivars().id
    }

    /// The languages of the project, which decide the exception filters it offers.
    pub(crate) fn languages(&self) -> ide_dap::Languages {
        ide_dap::Languages::of(self.root())
    }

    pub(crate) fn root(&self) -> &Path {
        &self.ivars().root
    }

    pub(crate) fn ui(&self) -> &Ui {
        self.ivars().ui.get().expect("the panel builds its views in `new`")
    }

    /// The panel's model, to change; the lists ask for their rows anew.
    pub(crate) fn model(&self) -> std::cell::RefMut<'_, Model> {
        self.ivars().children.borrow_mut().clear();
        self.ivars().model.borrow_mut()
    }

    /// The panel's model, to read.
    pub fn read(&self) -> std::cell::Ref<'_, Model> {
        self.ivars().model.borrow()
    }

    pub(crate) fn controller(&self) -> Option<Window> {
        self.ivars().window.is_open().then_some(self.ivars().window)
    }

    /// Whether the views are built; the lists ask their data source already while they are.
    pub(crate) fn is_built(&self) -> bool {
        self.ivars().ui.get().is_some()
    }

    /// Whether `view` is the inspector's list rather than the panel's.
    fn is_inspector(&self, view: &NSOutlineView) -> bool {
        self.ivars()
            .ui
            .get()
            .is_some_and(|ui| std::ptr::eq(view, &*ui.inspector.outline))
    }

    /// The keys of the rows below `item` in `view`, worked out once per change of the model.
    fn child_keys(&self, view: &NSOutlineView, item: Option<&AnyObject>) -> Rc<[RowKey]> {
        if !self.is_built() {
            return Rc::from([]);
        }
        let inspector = self.is_inspector(view);
        let parent = match item {
            None => None,
            Some(item) => match row_key(item) {
                Some(key) if matches!(key, RowKey::Inspector(_)) == inspector => Some(key.clone()),
                _ => return Rc::from([]),
            },
        };
        let cache = (inspector, parent);
        if let Some(keys) = self.ivars().children.borrow().get(&cache) {
            return keys.clone();
        }
        let keys: Rc<[RowKey]> = {
            let model = self.read();
            match &cache.1 {
                Some(RowKey::Inspector(row)) => model
                    .variables
                    .rows(Some(row))
                    .into_iter()
                    .map(RowKey::Inspector)
                    .collect(),
                Some(RowKey::Panel(key)) => model.rows(Some(key)).into_iter().map(RowKey::Panel).collect(),
                None if inspector => model.variables.rows(None).into_iter().map(RowKey::Inspector).collect(),
                None => model.rows(None).into_iter().map(RowKey::Panel).collect(),
            }
        };
        self.ivars().children.borrow_mut().insert(cache, keys.clone());
        keys
    }

    pub(crate) fn expands(&self, view: &NSOutlineView, item: &AnyObject) -> bool {
        match (row_key(item), self.is_inspector(view)) {
            (Some(RowKey::Inspector(row)), true) => self.read().variables.opens(row),
            (Some(RowKey::Panel(key)), false) => self.read().opens(key),
            _ => false,
        }
    }

    fn is_section(&self, view: &NSOutlineView, item: &AnyObject) -> bool {
        match (row_key(item), self.is_inspector(view)) {
            (Some(RowKey::Inspector(row)), true) => matches!(row, Row::Part(_)),
            (Some(RowKey::Panel(key)), false) => matches!(key, Key::Section(_)),
            _ => false,
        }
    }

    /// The object that stands for `key`, the same one for as long as the row exists.
    pub(crate) fn item_for(&self, key: &RowKey) -> Retained<RowItem> {
        self.ivars().items.item(self.mtm(), key)
    }

    pub(crate) fn panel_item(&self, key: &Key) -> Retained<RowItem> {
        self.item_for(&RowKey::Panel(key.clone()))
    }

    pub(crate) fn inspector_item(&self, row: &Row) -> Retained<RowItem> {
        self.item_for(&RowKey::Inspector(row.clone()))
    }

    /// Lets go of the items of rows neither list has any more, after both lists read their rows anew.
    pub(super) fn forget_gone_items(&self) {
        let model = self.read();
        let mut live = HashSet::new();
        let mut keys = model.rows(None);
        while let Some(key) = keys.pop() {
            keys.extend(model.rows(Some(&key)));
            live.insert(RowKey::Panel(key));
        }
        let mut rows = model.variables.rows(None);
        while let Some(row) = rows.pop() {
            rows.extend(model.variables.rows(Some(&row)));
            live.insert(RowKey::Inspector(row));
        }
        self.ivars().items.keep(|key| live.contains(key));
        self.ivars().cells.keep(|key| live.contains(key));
        self.ivars().expandable.borrow_mut().retain(|key, _| live.contains(key));
    }
}

/// What a row of the panel's list or of the inspector stands for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RowKey {
    /// A row of the panel's list.
    Panel(Key),
    /// A row of the inspector.
    Inspector(Row),
}

/// What the row `item` stands for, read in place.
pub(crate) fn row_key(item: &AnyObject) -> Option<&RowKey> {
    item.downcast_ref::<RowItem>()?.ivars().downcast_ref::<RowKey>()
}

/// The panel row `item` stands for.
pub(crate) fn key_of(item: &AnyObject) -> Option<Key> {
    match row_key(item)? {
        RowKey::Panel(key) => Some(key.clone()),
        RowKey::Inspector(_) => None,
    }
}

/// The inspector row `item` stands for.
pub(crate) fn row_of(item: &AnyObject) -> Option<Row> {
    match row_key(item)? {
        RowKey::Inspector(row) => Some(row.clone()),
        RowKey::Panel(_) => None,
    }
}
