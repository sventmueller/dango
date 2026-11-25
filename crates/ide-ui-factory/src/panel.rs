//! The panel "Software Factory" of one window (E175): the folder's orders grouped by station in an outline, a red dot
//! beside each that waits, "+ Neuer Auftrag" and the Leitstand tab in its title, an empty state without orders; a double
//! click opens an order's tab, the context menu runs the order's commands.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Weak;

use ide_ui_common::panel::{HeldReload, KeptCells, Nesting, PanelOutline, RowItem, RowItems};
use ide_ui_common::registry::WindowPart;
use ide_ui_common::util::{guarded, ns};
use ide_ui_common::window::{Panel, Panels, Window};
use ide_ui_common::{style, ui_size};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSControlTextEditingDelegate, NSImageView, NSLayoutAttribute, NSMenu, NSMenuDelegate, NSMenuItem,
    NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSStackView, NSStackViewGravity, NSTableColumn,
    NSTextAlignment, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use crate::actions::Target;
use crate::host::{State, host};
use crate::model::detail::Command;
use crate::model::list::{Key, Line, List};
use ide_factory::station::Station;

pub(crate) struct PanelIvars {
    id: u64,
    window: Window,
    root: PathBuf,
    /// The crate's state, weak because the state holds the panel.
    state: Weak<State>,
    list: RefCell<List>,
    /// The objects that stand for the outline's rows, the same one for a key as long as its row shows.
    items: RowItems<Key>,
    ui: OnceCell<PanelUi>,
    held: HeldReload,
    /// Whether the outline shows `list`, which a step that leaves the list as it was does not load again.
    fresh: Cell<bool>,
    cells: KeptCells<Key>,
    /// The rows below each row of `list`, worked out once per change of its lines.
    tree: RefCell<Tree>,
    /// Whether the lines changed since the outline last opened every station.
    unopened: Cell<bool>,
    /// Counts the changes of the runtimes `list` has.
    clock: Cell<u64>,
    /// The runtimes of which change each kept cell shows.
    timed: RefCell<HashMap<Key, u64>>,
}

/// The stations and the orders below each, and where each line stands in the list.
#[derive(Default)]
struct Tree {
    stations: Vec<Key>,
    orders: HashMap<Station, Vec<Key>>,
    index: HashMap<Key, usize>,
}

impl Tree {
    fn of(list: &List) -> Tree {
        let mut tree = Tree::default();
        let mut station = None;
        for (at, line) in list.lines().iter().enumerate() {
            let key = line.key();
            match line {
                Line::Station(s, _) => {
                    station = Some(*s);
                    tree.stations.push(key.clone());
                }
                Line::Order(_) => {
                    if let Some(station) = station {
                        tree.orders.entry(station).or_default().push(key.clone());
                    }
                }
            }
            tree.index.entry(key).or_insert(at);
        }
        tree
    }

    fn children(&self, item: Option<&AnyObject>) -> &[Key] {
        match item.map(RowItems::<Key>::key_of) {
            None => &self.stations,
            Some(Some(Key::Station(station))) => self.orders.get(&station).map_or(&[], Vec::as_slice),
            Some(_) => &[],
        }
    }
}

/// The views of the panel.
pub(crate) struct PanelUi {
    pub(crate) outline: Retained<NSOutlineView>,
    pub(crate) empty: Retained<NSStackView>,
    page: Retained<NSStackView>,
    header: Retained<NSStackView>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `FactoryPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryPanel"]
    #[ivars = PanelIvars]
    pub(crate) struct FactoryPanel;

    unsafe impl NSObjectProtocol for FactoryPanel {}

    unsafe impl NSControlTextEditingDelegate for FactoryPanel {}

    unsafe impl NSOutlineViewDataSource for FactoryPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.ivars().tree.borrow().children(item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let key = self.ivars().tree.borrow().children(item).get(usize::try_from(index).ok()?)?.clone();
                Some(self.item_for(&key))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || matches!(RowItems::<Key>::key_of(item), Some(Key::Station(_))))
        }
    }

    unsafe impl NSOutlineViewDelegate for FactoryPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.kept_view(&RowItems::<Key>::key_of(item)?))
        }

        #[unsafe(method(outlineView:shouldSelectItem:))]
        fn should_select(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || matches!(RowItems::<Key>::key_of(item), Some(Key::Order(_))))
        }
    }

    unsafe impl NSMenuDelegate for FactoryPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    impl FactoryPanel {
        #[unsafe(method(ideFactoryNewOrder:))]
        fn new_order_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::sheet::show(&self.target()));
        }

        #[unsafe(method(ideFactoryShowBoard:))]
        fn board_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                crate::board::show(&self.target());
            });
        }

        #[unsafe(method(ideFactoryOpenRow:))]
        fn open_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.ui().outline.clickedRow();
                if let Some(id) = self.order_at(row) {
                    crate::order::open(&self.target(), &id);
                }
            });
        }

        #[unsafe(method(ideFactoryRowCommand:))]
        fn command_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let command = sender
                    .and_then(|s| s.downcast_ref::<NSMenuItem>())
                    .and_then(|item| Command::of_tag(item.tag()));
                let row = self.ui().outline.clickedRow();
                if let (Some(command), Some(id)) = (command, self.order_at(row)) {
                    crate::actions::run_on(&self.target(), &id, command);
                }
            });
        }
    }
);

impl WindowPart for FactoryPanel {
    fn window(&self) -> Option<Window> {
        self.ivars().window.load()
    }

    fn root(&self) -> &Path {
        &self.ivars().root
    }
}

impl FactoryPanel {
    /// The panel of `window` for the folder `root`, put in place of the window's Leitstand page.
    pub(crate) fn attach(window: Window, root: PathBuf, state: &std::rc::Rc<State>) -> Retained<FactoryPanel> {
        state.panels.detach(window);
        let mtm = window.mtm();
        let ivars = PanelIvars {
            id: state.panels.next_id(),
            window,
            root,
            state: std::rc::Rc::downgrade(state),
            list: RefCell::new(List::default()),
            items: RowItems::default(),
            ui: OnceCell::new(),
            held: HeldReload::default(),
            fresh: Cell::new(false),
            cells: KeptCells::default(),
            tree: RefCell::new(Tree::default()),
            unopened: Cell::new(true),
            clock: Cell::new(0),
            timed: RefCell::new(HashMap::new()),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let panel: Retained<FactoryPanel> =
            unsafe { msg_send![super(FactoryPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = panel.ivars().ui.set(panel.build(mtm));
        state.panels.insert(panel.ivars().id, panel.clone());
        window.set_panel_page(Panel::Leitstand, &panel.ui().page);
        let weak = objc2::rc::Weak::from_retained(&panel);
        HeldReload::watch(&panel.ui().page, move || {
            if let Some(panel) = weak.load()
                && panel.ivars().held.catch_up(&panel.ui().outline)
            {
                panel.reload();
            }
        });
        window.set_panel_accessory(Panel::Leitstand, &panel.ui().header);
        panel.reload();
        panel
    }

    fn build(&self, mtm: MainThreadMarker) -> PanelUi {
        let me: &AnyObject = self.as_ref();
        let header = ide_ui_common::panel::header_row(mtm);
        let board = ide_ui_common::panel::header_button(
            mtm,
            ide_ui_common::util::FACTORY_SYMBOL,
            t("Software-Factory-Tab zeigen"),
            Some(me),
            sel!(ideFactoryShowBoard:),
        );
        let add =
            ide_ui_common::panel::header_button(mtm, "plus", t("Neuer Auftrag …"), Some(me), sel!(ideFactoryNewOrder:));
        header.addArrangedSubview(&board);
        header.addArrangedSubview(&add);
        let outline: Retained<NSOutlineView> = PanelOutline::new(mtm, Nesting::UnderTitle).into_super();
        let scroll = ide_ui_common::panel::source_list_outline(mtm, &outline, "orders", t("Aufträge"));
        // SAFETY: The outline holds its data source and delegate weakly; the state's registry keeps the panel alive.
        unsafe {
            outline.setDataSource(Some(ProtocolObject::from_ref(self)));
            outline.setDelegate(Some(ProtocolObject::from_ref(self)));
            outline.setTarget(Some(me));
            outline.setDoubleAction(Some(sel!(ideFactoryOpenRow:)));
        }
        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        // SAFETY: The outline keeps the menu; its delegate, the panel, outlives the outline through the registry.
        unsafe { outline.setMenu(Some(&menu)) };
        let empty = empty_state(mtm, me);
        let page = ide_ui_common::panel::panel_page(mtm);
        page.addView_inGravity(&scroll, NSStackViewGravity::Top);
        page.addView_inGravity(&empty, NSStackViewGravity::Center);
        PanelUi {
            outline,
            empty,
            page,
            header,
        }
    }

    pub(crate) fn ui(&self) -> &PanelUi {
        self.ivars().ui.get().expect("the panel builds its views in `attach`")
    }

    /// What its window, folder and state are, for the commands it starts.
    pub(crate) fn target(&self) -> Target {
        Target::new(
            self.ivars().window,
            self.ivars().root.clone(),
            self.ivars().state.clone(),
        )
    }

    /// The panel's list as it shows.
    pub(crate) fn list(&self) -> std::cell::Ref<'_, List> {
        self.ivars().list.borrow()
    }

    /// Reads the folder's orders again and shows them, with the symbol's words in its tooltip and VoiceOver label; a
    /// change of runtimes alone goes into the rows' cells in place.
    pub(crate) fn reload(&self) {
        let list = List::new(&host().panel(&self.ivars().root));
        let selected = self.selected_order();
        let words = list.symbol_words();
        let empty = list.is_empty();
        let (same, steady, keys) = {
            let old = self.ivars().list.borrow();
            (*old == list, old.same_but_runtimes(&list), old.same_keys(&list))
        };
        if !keys {
            *self.ivars().tree.borrow_mut() = Tree::of(&list);
            self.ivars().unopened.set(true);
        }
        if !same {
            self.ivars().clock.set(self.ivars().clock.get() + 1);
        }
        *self.ivars().list.borrow_mut() = list;
        let Some(ui) = self.ivars().ui.get() else { return };
        ui.empty.setHidden(!empty);
        if let Some(superview) = ui.outline.enclosingScrollView() {
            superview.setHidden(empty);
        }
        let held = self.ivars().held.holds(&ui.outline);
        let was_fresh = self.ivars().fresh.replace(!held);
        if !held && !(steady && was_fresh) {
            self.ivars().cells.keep_shown();
            ide_ui_common::panel::reload(&ui.outline);
            if self.ivars().unopened.replace(false) {
                let tree = self.ivars().tree.borrow();
                self.ivars().items.keep(|key| tree.index.contains_key(key));
                self.ivars()
                    .timed
                    .borrow_mut()
                    .retain(|key, _| tree.index.contains_key(key));
                drop(tree);
                // SAFETY: `None` stands for the outline's root, which holds every station's row.
                unsafe { ui.outline.expandItem_expandChildren(None, true) };
            }
            if let Some(id) = selected {
                self.select(&id);
            }
        } else if !held && !same {
            self.retime_shown(&ui.outline);
        }
        if let Some(window) = self.ivars().window.load() {
            window.set_symbol_status(Panel::Leitstand, words.as_deref());
        }
    }

    /// The cell of `key`'s row, the one built before while the row looks the same, with the runtime as of now.
    fn kept_view(&self, key: &Key) -> Option<Retained<NSView>> {
        let look = {
            let list = self.list();
            let at = *self.ivars().tree.borrow().index.get(key)?;
            list.lines().get(at)?.look()
        };
        let mut built = false;
        let cell = self.ivars().cells.cell(key, &look, || {
            built = true;
            self.with_line(key, |line| super::rows::view(self.mtm(), line))
        })?;
        let clock = self.ivars().clock.get();
        if built || self.ivars().timed.borrow().get(key) != Some(&clock) {
            if !built {
                self.with_line(key, |line| super::rows::retime(&cell, line));
            }
            self.ivars().timed.borrow_mut().insert(key.clone(), clock);
        }
        Some(cell)
    }

    /// Runs `f` with the line of `key`.
    fn with_line<T>(&self, key: &Key, f: impl FnOnce(&Line) -> T) -> Option<T> {
        let list = self.list();
        let at = *self.ivars().tree.borrow().index.get(key)?;
        list.lines().get(at).map(f)
    }

    /// Puts the runtimes into the cells on screen; the others take theirs when they show again.
    fn retime_shown(&self, outline: &NSOutlineView) {
        let shown = outline.rowsInRect(outline.visibleRect());
        for row in shown.location..shown.location + shown.length {
            let row = row.cast_signed();
            let Some(key) = outline.itemAtRow(row).and_then(|item| RowItems::<Key>::key_of(&item)) else {
                continue;
            };
            if let Some(cell) = outline.viewAtColumn_row_makeIfNecessary(0, row, false) {
                self.with_line(&key, |line| super::rows::retime(&cell, line));
                self.ivars().timed.borrow_mut().insert(key, self.ivars().clock.get());
            }
        }
    }

    fn item_for(&self, key: &Key) -> Retained<RowItem> {
        self.ivars().items.item(self.mtm(), key)
    }

    /// The order of the row at `row`, if it is an order's.
    fn order_at(&self, row: isize) -> Option<ide_factory::OrderId> {
        if row < 0 {
            return None;
        }
        let item = self.ui().outline.itemAtRow(row)?;
        match RowItems::<Key>::key_of(&item)? {
            Key::Order(id) => Some(id),
            Key::Station(_) => None,
        }
    }

    /// The order selected in the list.
    pub(crate) fn selected_order(&self) -> Option<ide_factory::OrderId> {
        let ui = self.ivars().ui.get()?;
        self.order_at(ui.outline.selectedRow())
    }

    /// Selects the row of the order `id`; whether it shows.
    pub(crate) fn select(&self, id: &ide_factory::OrderId) -> bool {
        let Some(item) = self.ivars().items.get(&Key::Order(id.clone())) else {
            return false;
        };
        let outline = &self.ui().outline;
        // SAFETY: The item is one of the outline's own row items.
        let row = unsafe { outline.rowForItem(Some(&item)) };
        if row < 0 {
            return false;
        }
        let rows = objc2_foundation::NSIndexSet::indexSetWithIndex(row.unsigned_abs());
        outline.selectRowIndexes_byExtendingSelection(&rows, false);
        true
    }

    fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let mtm = self.mtm();
        let Some(id) = self.order_at(self.ui().outline.clickedRow()) else {
            return;
        };
        let detail = host().order(&self.ivars().root, &id);
        let me: &AnyObject = self.as_ref();
        let open = ide_ui_common::util::item(mtm, t("Auftrag öffnen"), Some(sel!(ideFactoryOpenRow:)), "");
        // SAFETY: The panel answers the action and outlives the menu through the state's registry.
        unsafe { open.setTarget(Some(me)) };
        menu.addItem(&open);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        for command in Command::ALL {
            let item =
                ide_ui_common::util::tagged(mtm, command.title(), sel!(ideFactoryRowCommand:), "", command.tag());
            // SAFETY: The panel answers the action and outlives the menu through the state's registry.
            unsafe { item.setTarget(Some(me)) };
            item.setEnabled(detail.as_ref().is_some_and(|d| d.can(command)));
            menu.addItem(&item);
        }
        menu.setAutoenablesItems(false);
    }
}

/// The size the empty state draws the factory's symbol at.
const EMPTY_SYMBOL: f64 = 28.0;

/// The factory's symbol, "Noch kein Auftrag", what an order is and a button that starts the first, centred as the
/// panels show a place without content.
fn empty_state(mtm: MainThreadMarker, panel: &AnyObject) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::CenterX);
    ui_size::follow_spacing(&stack, style::space::M);
    let title = t("Noch kein Auftrag");
    let symbol = move |points: f64| ide_ui_common::util::symbol_at(ide_ui_common::util::FACTORY_SYMBOL, title, points);
    if let Some(image) = symbol(EMPTY_SYMBOL) {
        let view = NSImageView::imageViewWithImage(&image, mtm);
        ui_size::keep(&*view, ui_size::Aspect::Symbol, move |view| {
            view.setImage(symbol(ui_size::scaled(EMPTY_SYMBOL)).as_deref());
        });
        view.setContentTintColor(Some(&NSColor::tertiaryLabelColor()));
        stack.addArrangedSubview(&view);
    }
    let heading = ide_ui_common::util::label(mtm, title);
    ui_size::set_font(&heading, &style::bold(style::HEADING));
    heading.setAlignment(NSTextAlignment::Center);
    stack.addArrangedSubview(&heading);
    let explanation = NSTextField::wrappingLabelWithString(&ns(&crate::model::words::order_explained()), mtm);
    ui_size::set_system_font(&explanation, style::BODY);
    explanation.setTextColor(Some(&NSColor::secondaryLabelColor()));
    explanation.setAlignment(NSTextAlignment::Center);
    explanation.setPreferredMaxLayoutWidth(200.0);
    stack.addArrangedSubview(&explanation);
    let start = ide_ui_common::util::borderless_button(
        mtm,
        t("Ersten Auftrag anlegen …"),
        Some(panel),
        sel!(ideFactoryNewOrder:),
    );
    ui_size::set_system_font(&start, style::BODY);
    start.setContentTintColor(Some(&NSColor::controlAccentColor()));
    stack.addArrangedSubview(&start);
    stack
}

#[cfg(feature = "selftest")]
mod selftest_probe;
