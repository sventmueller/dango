//! The debugger of one window: its model, the views of the panel and the inspector, and the AppKit callbacks of both
//! lists, which hand their events to the model (E98); `controller.rs` carries out what it returns.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSTextField, NSTextFieldDelegate, NSTextView, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSString};

use super::model::Model;
use super::model::rows::Key;
use super::model::variables::Row;
use super::ui::Ui;
use ide_ui_common::registry::WindowPart;
use ide_ui_common::util::{guarded, ns};
use ide_ui_common::window::{Inspector, Panels, Trust, Window};

pub struct PanelIvars {
    id: u64,
    window: Window,
    root: PathBuf,
    ui: OnceCell<Ui>,
    model: RefCell<Model>,
    /// The objects that stand for the rows of both lists, the same one for a key as long as the panel lives.
    items: RefCell<HashMap<String, Retained<NSString>>>,
    /// The panel itself opens and closes rows, so the user's folding is not touched.
    pub unfolding: Cell<bool>,
    /// The running sessions and their consoles.
    pub links: RefCell<super::controller::Links>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `DebugPanel` does not implement Drop.
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
            guarded(false, || self.expands(view, item))
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
            guarded(24.0, || {
                let empty = matches!(key_of(item), Some(Key::Empty(_))) || matches!(row_of(item), Some(Row::Empty(_)));
                if empty {
                    super::ui::NOTE_HEIGHT
                } else if matches!(key_of(item), Some(Key::Session(_))) {
                    super::ui::SESSION_HEIGHT
                } else {
                    24.0
                }
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
pub fn with_panel(id: u64, f: impl FnOnce(&DebugPanel)) {
    crate::host::state().panels.with(id, f);
}

/// The panel of `window`.
pub fn of(window: &Window) -> Option<Retained<DebugPanel>> {
    crate::host::state().panels.of(*window)
}

/// The panel of the window that shows `root`.
pub fn for_root(root: &Path) -> Option<Retained<DebugPanel>> {
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
    pub fn attach(controller: &Window, root: PathBuf) -> Retained<DebugPanel> {
        let mtm = controller.mtm();
        if let Some(old) = of(controller) {
            old.end_all();
        }
        let state = crate::host::state();
        let panels = &state.panels;
        panels.detach(*controller);
        let panel = DebugPanel::new(mtm, controller, root);
        panels.insert(panel.id(), panel.clone());
        controller.set_panel_page(ide_ui_common::window::Panel::Debug, &panel.ui().page);
        controller.set_panel_accessory(ide_ui_common::window::Panel::Debug, &panel.ui().header);
        controller.set_inspector_content(&panel.ui().inspector.page);
        panel.load_configurations();
        panel.refresh();
        panel
    }

    fn new(mtm: MainThreadMarker, controller: &Window, root: PathBuf) -> Retained<DebugPanel> {
        let mut model = Model::new(ide_core::debugging::load(&root));
        model.fit_filters(ide_dap::Languages::of(&root));
        let ivars = PanelIvars {
            id: crate::host::state().panels.next_id(),
            window: *controller,
            root,
            ui: OnceCell::new(),
            model: RefCell::new(model),
            items: RefCell::new(HashMap::new()),
            unfolding: Cell::new(false),
            links: RefCell::new(super::controller::Links::default()),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let panel: Retained<DebugPanel> = unsafe { msg_send![super(DebugPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = panel.ivars().ui.set(Ui::new(mtm, &panel));
        panel
    }

    pub fn id(&self) -> u64 {
        self.ivars().id
    }

    /// The languages of the project, which decide the exception filters it offers.
    pub fn languages(&self) -> ide_dap::Languages {
        ide_dap::Languages::of(self.root())
    }

    pub fn root(&self) -> &Path {
        &self.ivars().root
    }

    pub fn ui(&self) -> &Ui {
        self.ivars().ui.get().expect("the panel builds its views in `new`")
    }

    pub fn model(&self) -> std::cell::RefMut<'_, Model> {
        self.ivars().model.borrow_mut()
    }

    pub fn read(&self) -> std::cell::Ref<'_, Model> {
        self.ivars().model.borrow()
    }

    pub fn controller(&self) -> Option<Window> {
        self.ivars().window.is_open().then_some(self.ivars().window)
    }

    /// Whether the views are built; the lists ask their data source already while they are.
    pub fn is_built(&self) -> bool {
        self.ivars().ui.get().is_some()
    }

    /// Whether `view` is the inspector's list rather than the panel's.
    fn is_inspector(&self, view: &NSOutlineView) -> bool {
        self.ivars()
            .ui
            .get()
            .is_some_and(|ui| std::ptr::eq(view, &*ui.inspector.outline))
    }

    /// The keys of the rows below `item` in `view`, encoded.
    fn child_keys(&self, view: &NSOutlineView, item: Option<&AnyObject>) -> Vec<String> {
        if !self.is_built() {
            return Vec::new();
        }
        let text = item
            .and_then(|item| item.downcast_ref::<NSString>())
            .map(ToString::to_string);
        if self.is_inspector(view) {
            let row = text.as_deref().and_then(|t| t.strip_prefix('i')).and_then(Row::decode);
            if item.is_some() && row.is_none() {
                return Vec::new();
            }
            let rows = self.read().variables.rows(row.as_ref());
            rows.iter().map(|row| format!("i{}", row.encode())).collect()
        } else {
            let key = text.as_deref().and_then(|t| t.strip_prefix('p')).and_then(Key::decode);
            if item.is_some() && key.is_none() {
                return Vec::new();
            }
            let rows = self.read().rows(key.as_ref());
            rows.iter().map(|key| format!("p{}", key.encode())).collect()
        }
    }

    fn expands(&self, view: &NSOutlineView, item: &AnyObject) -> bool {
        if self.is_inspector(view) {
            row_of(item).is_some_and(|row| self.read().variables.opens(&row))
        } else {
            key_of(item).is_some_and(|key| self.read().opens(&key))
        }
    }

    fn is_section(&self, view: &NSOutlineView, item: &AnyObject) -> bool {
        if self.is_inspector(view) {
            matches!(row_of(item), Some(Row::Part(_)))
        } else {
            matches!(key_of(item), Some(Key::Section(_)))
        }
    }

    /// The object that stands for the encoded key `key`.
    pub fn item_for(&self, key: &str) -> Retained<NSString> {
        self.ivars()
            .items
            .borrow_mut()
            .entry(key.to_string())
            .or_insert_with_key(|text| ns(text))
            .clone()
    }

    pub fn panel_item(&self, key: &Key) -> Retained<NSString> {
        self.item_for(&format!("p{}", key.encode()))
    }

    pub fn inspector_item(&self, row: &Row) -> Retained<NSString> {
        self.item_for(&format!("i{}", row.encode()))
    }
}

/// The panel row `item` stands for.
pub fn key_of(item: &AnyObject) -> Option<Key> {
    let text = item.downcast_ref::<NSString>()?.to_string();
    Key::decode(text.strip_prefix('p')?)
}

/// The inspector row `item` stands for.
pub fn row_of(item: &AnyObject) -> Option<Row> {
    let text = item.downcast_ref::<NSString>()?.to_string();
    Row::decode(text.strip_prefix('i')?)
}
