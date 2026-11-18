//! The figures' section of the Leitstand tab (E180): a disclosure under the fleet line that opens a second sortable
//! table of first-pass rate, review rounds, cost and runtime per role, agent, model and risk class, with the lines
//! that say how many orders the figures rest on. Closed, it takes one line of the tab.

use std::cell::{OnceCell, RefCell};

use ide_factory::metrics::Metrics;
use ide_ui_common::style::space;
use ide_ui_common::util::{guarded, label, ns, set_label};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSControlStateValueOff, NSControlStateValueOn, NSControlTextEditingDelegate,
    NSLineBreakMode, NSScrollView, NSStackView, NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate,
    NSView,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSSortDescriptor};
use splat_i18n::t;

use crate::model::metrics::{METRICS_COLUMNS, MetricsView, metrics_column_of, metrics_column_title, metrics_key_of};
use crate::ui;

/// The width of each column, by its place.
const WIDTHS: [f64; 8] = [90.0, 170.0, 90.0, 70.0, 110.0, 80.0, 130.0, 90.0];

pub(crate) struct MetricsIvars {
    view: RefCell<MetricsView>,
    ui: OnceCell<MetricsUi>,
}

struct MetricsUi {
    disclosure: Retained<NSButton>,
    title: Retained<objc2_app_kit::NSTextField>,
    lines: Retained<NSStackView>,
    table: Retained<NSTableView>,
    scroll: Retained<NSScrollView>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `MetricsSection` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryMetricsSection"]
    #[ivars = MetricsIvars]
    pub(crate) struct MetricsSection;

    unsafe impl NSObjectProtocol for MetricsSection {}

    unsafe impl NSControlTextEditingDelegate for MetricsSection {}

    unsafe impl NSTableViewDataSource for MetricsSection {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().view.borrow().rows().len()).unwrap_or(0))
        }

        #[unsafe(method(tableView:sortDescriptorsDidChange:))]
        fn sort_changed(&self, table: &NSTableView, _old: &NSArray<NSSortDescriptor>) {
            guarded((), || self.sorted(table));
        }
    }

    unsafe impl NSTableViewDelegate for MetricsSection {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let column = metrics_column_of(&column?.identifier().to_string())?;
                let view = self.ivars().view.borrow();
                let row = view.rows().get(usize::try_from(row).ok()?)?;
                let text = label(self.mtm(), row.cell(column));
                text.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
                Some(text.into_super().into_super())
            })
        }
    }

    impl MetricsSection {
        #[unsafe(method(ideFactoryToggleMetrics:))]
        fn toggle_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.ivars().view.borrow_mut().toggle();
                self.show();
            });
        }
    }
);

impl MetricsSection {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<MetricsSection> {
        let ivars = MetricsIvars {
            view: RefCell::new(MetricsView::default()),
            ui: OnceCell::new(),
        };
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let section: Retained<MetricsSection> =
            unsafe { msg_send![super(MetricsSection::alloc(mtm).set_ivars(ivars)), init] };
        let _ = section.ivars().ui.set(section.build(mtm));
        section.show();
        section
    }

    fn build(&self, mtm: MainThreadMarker) -> MetricsUi {
        let me: &AnyObject = self.as_ref();
        let disclosure = ui::button(mtm, "", me, sel!(ideFactoryToggleMetrics:), 0);
        disclosure.setBezelStyle(objc2_app_kit::NSBezelStyle::Disclosure);
        disclosure.setButtonType(objc2_app_kit::NSButtonType::PushOnPushOff);
        disclosure.setTitle(&ns(""));
        let title = label(mtm, t("Kennzahlen"));
        let head = ui::row(mtm, space::XS);
        head.addArrangedSubview(&disclosure);
        head.addArrangedSubview(&title);
        let lines = ui::column(mtm, space::XXS);
        let table = NSTableView::new(mtm);
        for ((column, key), width) in METRICS_COLUMNS.into_iter().zip(WIDTHS) {
            let item = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(key));
            item.setTitle(&ns(metrics_column_title(column)));
            item.setSortDescriptorPrototype(Some(&NSSortDescriptor::sortDescriptorWithKey_ascending(
                Some(&ns(key)),
                true,
            )));
            item.setWidth(width);
            table.addTableColumn(&item);
        }
        table.setUsesAlternatingRowBackgroundColors(true);
        table.setAllowsColumnReordering(false);
        table.setAccessibilityLabel(Some(&ns(t("Kennzahlen je Rolle, Agent, Modell und Risikoklasse"))));
        let (sort, ascending) = self.ivars().view.borrow().sorting();
        let first = NSSortDescriptor::sortDescriptorWithKey_ascending(Some(&ns(metrics_key_of(sort))), ascending);
        table.setSortDescriptors(&NSArray::from_retained_slice(&[first]));
        // SAFETY: The table holds its data source and delegate weakly; the section owns the table.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        let scroll = NSScrollView::new(mtm);
        scroll.setHasVerticalScroller(true);
        scroll.setDocumentView(Some(&table));
        scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
        scroll.heightAnchor().constraintEqualToConstant(160.0).setActive(true);
        let column = ui::column(mtm, space::XS);
        for view in [&*head as &NSView, &lines, &scroll] {
            column.addArrangedSubview(view);
        }
        scroll
            .widthAnchor()
            .constraintEqualToAnchor(&column.widthAnchor())
            .setActive(true);
        self.addSubview(&column);
        ide_ui_common::util::auto_layout(&[&column]);
        ide_ui_common::util::pin_edges(&column, self);
        MetricsUi {
            disclosure,
            title,
            lines,
            table,
            scroll,
        }
    }

    fn ui(&self) -> &MetricsUi {
        self.ivars().ui.get().expect("the section builds its views in `new`")
    }

    /// What it shows.
    pub(crate) fn view(&self) -> std::cell::Ref<'_, MetricsView> {
        self.ivars().view.borrow()
    }

    /// Takes the folder's figures, with the sort and whether the section is open kept.
    pub(crate) fn set(&self, metrics: Option<&Metrics>) {
        self.ivars().view.borrow_mut().set(metrics);
        self.show();
    }

    fn show(&self) {
        let mtm = self.mtm();
        let ui = self.ui();
        let view = self.view();
        let title = view.title();
        ui.title.setStringValue(&ns(&title));
        set_label(&ui.disclosure, Some(&title));
        ui.disclosure.setState(if view.open() {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        ui::clear(&ui.lines);
        for line in view.lines() {
            ui.lines.addArrangedSubview(&ui::note(mtm, line, 640.0));
        }
        ui.lines.setHidden(!view.open());
        ui.scroll.setHidden(!view.open() || view.rows().is_empty());
        drop(view);
        ui.table.reloadData();
    }

    fn sorted(&self, table: &NSTableView) {
        let descriptors = table.sortDescriptors();
        let Some(first) = descriptors.firstObject() else { return };
        let Some(column) = first.key().and_then(|key| metrics_column_of(&key.to_string())) else {
            return;
        };
        self.ivars().view.borrow_mut().sort(column, first.ascending());
        self.ui().table.reloadData();
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
