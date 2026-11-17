//! The Leitstand tab (E175; the Kanban board struck, F66): the fleet line with the figures of E180 folded under it, the
//! station strip whose buttons filter the table, and the sortable table of every order, which shows first what has
//! waited longest for the user. A double
//! click or Return opens an order's tab; the gear opens "Software Factory für <Ordner>".

use std::cell::{Cell, OnceCell, RefCell};

use ide_factory::OrderId;
use ide_factory::station::Station;
use ide_ui_common::util::{guarded, ns};
use ide_ui_common::window::{TabDecor, TabId, TabKind, Tabs};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSControlTextEditingDelegate,
    NSStackView, NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSView,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSSortDescriptor};
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::host;
use crate::model::board::{
    BoardView, COLUMNS, FleetWords, column_of, column_title, confirm_label, key_of, loosening_of,
};
use crate::table::OrderTable;
use crate::ui;
use ide_factory::workflow::rules::Loosening;
use ide_ui_common::style::space;

/// The key of the Leitstand tab, one per window (E64).
pub(crate) const TAB_KEY: &str = "leitstand";

pub(crate) struct BoardIvars {
    target: Target,
    tab: Cell<Option<TabId>>,
    view: RefCell<BoardView>,
    /// The hashes of the loosenings shown, by the tags of their buttons.
    loosenings: RefCell<Vec<String>>,
    ui: OnceCell<BoardUi>,
}

struct BoardUi {
    notices: Retained<NSStackView>,
    fleet: Retained<NSStackView>,
    metrics: Retained<crate::metrics::MetricsSection>,
    strip: Retained<NSStackView>,
    table: Retained<OrderTable>,
    empty: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `BoardTab` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryBoardTab"]
    #[ivars = BoardIvars]
    pub(crate) struct BoardTab;

    unsafe impl NSObjectProtocol for BoardTab {}

    unsafe impl NSTableViewDataSource for BoardTab {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().view.borrow().rows().len()).unwrap_or(0))
        }

        #[unsafe(method(tableView:sortDescriptorsDidChange:))]
        fn sort_changed(&self, table: &NSTableView, _old: &NSArray<NSSortDescriptor>) {
            guarded((), || self.sorted(table));
        }
    }

    unsafe impl NSControlTextEditingDelegate for BoardTab {}

    unsafe impl NSTableViewDelegate for BoardTab {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let column = column_of(&column?.identifier().to_string())?;
                let rows = self.ivars().view.borrow().rows();
                let row = rows.get(usize::try_from(row).ok()?)?;
                Some(crate::table::cell(self.mtm(), row, column))
            })
        }
    }

    impl BoardTab {
        #[unsafe(method(ideFactoryStation:))]
        fn station_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let station = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .and_then(|b| Station::ALL.get(usize::try_from(b.tag()).ok()?).copied());
                if let Some(station) = station {
                    self.ivars().view.borrow_mut().click(station);
                    self.show();
                }
            });
        }

        #[unsafe(method(ideFactoryOpenRow:))]
        fn open_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let table = &self.ui().table;
                let row = if table.clickedRow() >= 0 { table.clickedRow() } else { table.selectedRow() };
                if let Some(id) = self.order_at(row) {
                    crate::order::open(&self.ivars().target, &id);
                }
            });
        }

        #[unsafe(method(ideFactoryConfirm:))]
        fn confirm_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(button) = sender.and_then(|s| s.downcast_ref::<objc2_app_kit::NSButton>()) {
                    self.confirm(button.tag());
                }
            });
        }

        #[unsafe(method(ideFactoryProjectSettings:))]
        fn settings_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::project::show(&self.ivars().target));
        }
    }
);

/// Opens the Leitstand tab in `target`'s window, or brings it to the front; whether it shows.
pub(crate) fn show(target: &Target) -> bool {
    let window = target.window();
    if window.select_tab(TAB_KEY) {
        return true;
    }
    let Some(state) = target.state() else { return false };
    let tab = BoardTab::new(window.mtm(), target.clone());
    let Some(id) = window.add_tab(
        TabKind::Leitstand,
        Some(TAB_KEY.to_string()),
        t("Software Factory"),
        Retained::into_super(tab.clone()),
    ) else {
        return false;
    };
    tab.ivars().tab.set(Some(id));
    let number = state.boards.next_id();
    state.boards.insert(number, objc2::rc::Weak::from_retained(&tab));
    tab.reload();
    true
}

impl BoardTab {
    fn new(mtm: MainThreadMarker, target: Target) -> Retained<BoardTab> {
        let ivars = BoardIvars {
            target,
            tab: Cell::new(None),
            view: RefCell::new(BoardView::default()),
            loosenings: RefCell::new(Vec::new()),
            ui: OnceCell::new(),
        };
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let tab: Retained<BoardTab> = unsafe { msg_send![super(BoardTab::alloc(mtm).set_ivars(ivars)), init] };
        let _ = tab.ivars().ui.set(tab.build(mtm));
        tab
    }

    fn build(&self, mtm: MainThreadMarker) -> BoardUi {
        let me: &AnyObject = self.as_ref();
        let head = ui::row(mtm, space::M);
        head.addArrangedSubview(&ui::title(mtm, t("Software Factory")));
        let gear = ide_ui_common::util::symbol_button(
            mtm,
            "gearshape",
            t("Software-Factory-Einstellungen …"),
            ide_ui_common::style::symbol::MEDIUM,
            Some(me),
            sel!(ideFactoryProjectSettings:),
        );
        ide_ui_common::util::set_label(&gear, Some(t("Software-Factory-Einstellungen …")));
        head.addArrangedSubview(&gear);
        let notices = ui::column(mtm, space::XS);
        let fleet = ui::column(mtm, space::XXS);
        let metrics = crate::metrics::MetricsSection::new(mtm);
        let strip = ui::row(mtm, space::XS);
        strip.setAccessibilityLabel(Some(&ns(t("Stationen"))));
        let table = OrderTable::new(mtm);
        for (column, key) in COLUMNS {
            let item = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(key));
            item.setTitle(&ns(column_title(column)));
            item.setSortDescriptorPrototype(Some(&NSSortDescriptor::sortDescriptorWithKey_ascending(
                Some(&ns(key)),
                true,
            )));
            item.setWidth(if key == "title" { 220.0 } else { 120.0 });
            table.addTableColumn(&item);
        }
        table.setUsesAlternatingRowBackgroundColors(true);
        table.setAllowsColumnReordering(false);
        table.setAccessibilityLabel(Some(&ns(t("Aufträge"))));
        let (sort, ascending) = self.ivars().view.borrow().sorting();
        let first = NSSortDescriptor::sortDescriptorWithKey_ascending(Some(&ns(key_of(sort))), ascending);
        table.setSortDescriptors(&NSArray::from_retained_slice(&[first]));
        // SAFETY: The table holds its data source, delegate and target weakly; the tab owns the table.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
            table.setTarget(Some(me));
            table.setDoubleAction(Some(sel!(ideFactoryOpenRow:)));
        }
        let scroll = objc2_app_kit::NSScrollView::new(mtm);
        scroll.setHasVerticalScroller(true);
        scroll.setDocumentView(Some(&table));
        scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
        scroll
            .heightAnchor()
            .constraintGreaterThanOrEqualToConstant(240.0)
            .setActive(true);
        let empty = ui::note(mtm, "", 520.0);
        let column = ui::column(mtm, space::L);
        column.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: space::M,
            left: space::GROUP,
            bottom: space::GROUP,
            right: space::GROUP,
        });
        for view in [&*head as &NSView, &notices, &fleet, &metrics, &strip, &empty, &scroll] {
            column.addArrangedSubview(view);
        }
        for wide in [&*scroll as &NSView, &metrics] {
            wide.widthAnchor()
                .constraintEqualToAnchor_constant(&column.widthAnchor(), -2.0 * space::GROUP)
                .setActive(true);
        }
        column.setHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Vertical);
        self.addSubview(&column);
        ide_ui_common::util::auto_layout(&[&column]);
        ide_ui_common::util::pin_edges(&column, self);
        BoardUi {
            notices,
            fleet,
            metrics,
            strip,
            table,
            empty,
        }
    }

    fn ui(&self) -> &BoardUi {
        self.ivars().ui.get().expect("the tab builds its views in `new`")
    }

    /// The folder of its window.
    pub(crate) fn root(&self) -> &std::path::Path {
        self.ivars().target.root()
    }

    /// Its tab.
    pub(crate) fn tab(&self) -> Option<TabId> {
        self.ivars().tab.get()
    }

    /// What it shows.
    pub(crate) fn view(&self) -> std::cell::Ref<'_, BoardView> {
        self.ivars().view.borrow()
    }

    /// Reads the folder's orders and fleet again and shows them, with the filter and sort kept.
    pub(crate) fn reload(&self) {
        let now = crate::now();
        let board = host().board(self.root());
        self.ivars().view.borrow_mut().set(board, now);
        let fleet = FleetWords::new(host().fleet(self.root()).as_ref(), now, &crate::local_day(now));
        let mtm = self.mtm();
        let ui = self.ui();
        ui::clear(&ui.fleet);
        for line in [fleet.orders(), fleet.day(), fleet.disk()]
            .into_iter()
            .chain(fleet.quotas().iter().map(String::as_str))
            .filter(|line| !line.is_empty())
        {
            ui.fleet.addArrangedSubview(&ui::note(mtm, line, 640.0));
        }
        ui.metrics.set(host().metrics(self.root()).as_ref());
        self.show_notices();
        self.show();
    }

    /// Errors of the workflow file, unreadable logs and loosenings that wait for confirmation, above the fleet.
    fn show_notices(&self) {
        let mtm = self.mtm();
        let me: &AnyObject = self.as_ref();
        let ui = self.ui();
        ui::clear(&ui.notices);
        let notices = host().notices(self.root());
        for error in &notices.errors {
            let line = tf("Workflow-Datei: {}", &[&crate::model::words::workflow_error(error)]);
            ui.notices.addArrangedSubview(&ui::note(mtm, &line, 640.0));
        }
        for problem in &notices.problems {
            let line = crate::model::words::restore_problem(problem);
            ui.notices.addArrangedSubview(&ui::note(mtm, &line, 640.0));
        }
        for (index, loosening) in notices.loosenings.iter().enumerate() {
            let row = ui::row(mtm, space::M);
            row.addArrangedSubview(&ui::note(mtm, &crate::model::words::loosening(loosening), 520.0));
            let tag = isize::try_from(index).unwrap_or(0);
            let confirm = ui::button(mtm, t("Bestätigen"), me, sel!(ideFactoryConfirm:), tag);
            ide_ui_common::util::set_label(&confirm, Some(&confirm_label(loosening)));
            row.addArrangedSubview(&confirm);
            ui.notices.addArrangedSubview(&row);
        }
        *self.ivars().loosenings.borrow_mut() = notices.loosenings.iter().map(Loosening::hash).collect();
        ui.notices
            .setHidden(notices.errors.is_empty() && notices.problems.is_empty() && notices.loosenings.is_empty());
    }

    /// Confirms the loosening the button at `index` stood beside, if the folder still asks for it.
    fn confirm(&self, index: isize) {
        let shown = usize::try_from(index)
            .ok()
            .and_then(|i| self.ivars().loosenings.borrow().get(i).cloned());
        let notices = host().notices(self.root());
        if let Some(loosening) = shown.and_then(|hash| loosening_of(&notices.loosenings, &hash)) {
            host().confirm_loosening(self.root(), loosening);
        }
        self.reload();
    }

    /// Shows the strip and the table as the model has them now.
    fn show(&self) {
        let mtm = self.mtm();
        let ui = self.ui();
        let me: &AnyObject = self.as_ref();
        let selected = self.order_at(ui.table.selectedRow());
        ui::clear(&ui.strip);
        let view = self.view();
        for (index, stop) in view.strip().iter().enumerate() {
            if index > 0 {
                let chevron = objc2_app_kit::NSImageView::new(mtm);
                chevron.setImage(ide_ui_common::util::symbol("chevron.forward", "").as_deref());
                chevron.setContentTintColor(Some(&NSColor::tertiaryLabelColor()));
                chevron.setAccessibilityElement(false);
                ui.strip.addArrangedSubview(&chevron);
            }
            let button = ui::button(
                mtm,
                &stop.title(),
                me,
                sel!(ideFactoryStation:),
                isize::try_from(index).unwrap_or(0),
            );
            button.setButtonType(objc2_app_kit::NSButtonType::PushOnPushOff);
            button.setBezelStyle(objc2_app_kit::NSBezelStyle::Push);
            button.setState(if stop.active() {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            ide_ui_common::util::set_label(&button, Some(&stop.spoken()));
            ui.strip.addArrangedSubview(&button);
        }
        let rows = view.rows();
        ui.empty.setStringValue(&ns(&if rows.is_empty() {
            view.empty_words()
        } else {
            String::new()
        }));
        ui.empty.setHidden(!rows.is_empty());
        let waits = view.waits();
        drop(view);
        ui.table.reloadData();
        if let Some(id) = selected
            && let Some(index) = self.ivars().view.borrow().rows().iter().position(|r| *r.id() == id)
        {
            let rows = objc2_foundation::NSIndexSet::indexSetWithIndex(index);
            ui.table.selectRowIndexes_byExtendingSelection(&rows, false);
        }
        if let Some(tab) = self.tab() {
            let red = NSColor::systemRedColor();
            self.ivars().target.window().set_tab_status(tab, waits.then_some(&*red));
        }
    }

    fn sorted(&self, table: &NSTableView) {
        let descriptors = table.sortDescriptors();
        let Some(first) = descriptors.firstObject() else { return };
        let Some(column) = first.key().and_then(|key| column_of(&key.to_string())) else {
            return;
        };
        self.ivars().view.borrow_mut().sort(column, first.ascending());
        self.show();
    }

    fn order_at(&self, row: isize) -> Option<OrderId> {
        let index = usize::try_from(row).ok()?;
        self.ivars().view.borrow().rows().get(index).map(|r| r.id().clone())
    }

    /// The order selected in the table.
    pub(crate) fn selected_order(&self) -> Option<OrderId> {
        self.order_at(self.ui().table.selectedRow())
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
