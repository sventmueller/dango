//! The roles' editor of "Software Factory für <Ordner>": a switch between the four roles, the role's ranked list as a
//! table of agent, model and effort, where model and effort are picked from what the agent offers or typed, and the
//! bar under it as macOS draws one under a list: add an agent, remove, move up and down. Every change goes into the
//! project's workflow file at once; a role the file sets offers "Auf Vorgabe zurücksetzen".

use std::cell::{Cell, OnceCell};

use ide_factory::routing::RankEntry;
use ide_factory::station::Role;
use ide_ui_common::util::{guarded, label, ns, set_action};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSComboBox, NSControlSize, NSControlTextEditingDelegate, NSImage, NSMenu,
    NSScrollView, NSSegmentStyle, NSSegmentSwitchTracking, NSSegmentedControl, NSStackView, NSTableColumn, NSTableView,
    NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSView,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use splat_i18n::t;

use super::ProjectSheet;
use super::form::{CONTENT, Form};
use crate::model::settings::{AGENTS, Origin, ProjectValue, agent_name, takes};
use crate::model::{ranks, words};
use crate::ui;
use ide_ui_common::style::space;

/// The list's height, five rows and its header.
const HEIGHT: f64 = 150.0;

/// The columns of the list: identifier, title and width.
const COLUMNS: [(&str, &str, f64); 3] = [
    ("agent", "Agent", 190.0),
    ("model", "Modell", 220.0),
    ("effort", "Denktiefe", 130.0),
];

/// The segments of the bar under the list, in their order.
const ADD: isize = 0;
const REMOVE: isize = 1;
const UP: isize = 2;
const DOWN: isize = 3;

pub(crate) struct RolesIvars {
    sheet: Weak<ProjectSheet>,
    role: Cell<Role>,
    ui: OnceCell<RolesUi>,
}

struct RolesUi {
    roles: Retained<NSSegmentedControl>,
    table: Retained<NSTableView>,
    list: Retained<NSScrollView>,
    bar: Retained<NSStackView>,
    buttons: Retained<NSSegmentedControl>,
    origin: Retained<NSTextField>,
    reset: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `RolesEditor` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryRolesEditor"]
    #[ivars = RolesIvars]
    pub(crate) struct RolesEditor;

    unsafe impl NSObjectProtocol for RolesEditor {}

    unsafe impl NSControlTextEditingDelegate for RolesEditor {}

    unsafe impl NSTableViewDataSource for RolesEditor {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.list().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for RolesEditor {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || self.cell(&column?.identifier().to_string(), row))
        }
    }

    impl RolesEditor {
        #[unsafe(method(ideFactoryRolesRole:))]
        fn role_chosen(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(self.ui().roles.selectedSegment()).unwrap_or(0);
                self.ivars().role.set(Role::ALL.get(index).copied().unwrap_or(Role::Builder));
                self.deselect();
                self.reload();
            });
        }

        #[unsafe(method(ideFactoryRolesBar:))]
        fn bar_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || match self.ui().buttons.selectedSegment() {
                ADD => self.offer_agents(),
                REMOVE => self.remove_selected(),
                segment @ (UP | DOWN) => self.shift_selected(segment == UP),
                _ => {}
            });
        }

        #[unsafe(method(ideFactoryRolesAddAgent:))]
        fn add_agent(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender
                    .and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>())
                    .map_or(-1, objc2_app_kit::NSMenuItem::tag);
                if let Some(agent) = usize::try_from(tag).ok().and_then(|i| AGENTS.get(i)) {
                    let agent = agent.clone();
                    self.change(|list| {
                        ranks::add(list, agent);
                        list.len().checked_sub(1)
                    });
                }
            });
        }

        #[unsafe(method(ideFactoryRolesModel:))]
        fn model(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some((index, text)) = combo_value(sender) {
                    self.change(|list| {
                        ranks::set_model(list, index, &text);
                        Some(index)
                    });
                }
            });
        }

        #[unsafe(method(ideFactoryRolesEffort:))]
        fn effort(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some((index, text)) = combo_value(sender) {
                    self.change(|list| {
                        ranks::set_effort(list, index, &text);
                        Some(index)
                    });
                }
            });
        }

        #[unsafe(method(ideFactoryRolesReset:))]
        fn reset_role(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(sheet) = self.ivars().sheet.load() {
                    sheet.reset(ProjectValue::Role(self.role()));
                }
            });
        }
    }
);

impl RolesEditor {
    /// The editor of `sheet`'s roles, showing the builder's list.
    pub(crate) fn new(mtm: MainThreadMarker, sheet: &ProjectSheet) -> Retained<RolesEditor> {
        let ivars = RolesIvars {
            sheet: Weak::from(sheet),
            role: Cell::new(Role::Builder),
            ui: OnceCell::new(),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let editor: Retained<RolesEditor> = unsafe { msg_send![super(RolesEditor::alloc(mtm).set_ivars(ivars)), init] };
        let _ = editor.ivars().ui.set(editor.build(mtm));
        editor.reload();
        editor
    }

    /// Puts the editor into `form`: the roles' switch, the list, its bar and the note on the ranking.
    pub(crate) fn add_to(&self, form: &Form) {
        let ui = self.ui();
        form.wide(&ui.roles);
        form.wide(&ui.list);
        form.wide(&ui.bar);
        form.note(t(
            "Die Reihenfolge ist die Rangliste: Ein Auftrag nimmt den ersten Eintrag, der bereit ist. „Anderer Hersteller bevorzugt“ gilt erst nach deiner Bestätigung.",
        ));
    }

    fn ui(&self) -> &RolesUi {
        self.ivars().ui.get().expect("the editor builds its views in `new`")
    }

    fn role(&self) -> Role {
        self.ivars().role.get()
    }

    /// The list of the role shown and where it comes from, as the file says now.
    fn ranks(&self) -> (Vec<RankEntry>, Origin) {
        self.ivars()
            .sheet
            .load()
            .map_or((Vec::new(), Origin::Default), |sheet| sheet.file().ranks(self.role()))
    }

    fn list(&self) -> Vec<RankEntry> {
        self.ranks().0
    }

    fn build(&self, mtm: MainThreadMarker) -> RolesUi {
        let me: &AnyObject = self.as_ref();
        let titles: Vec<Retained<NSString>> = Role::ALL.iter().map(|role| ns(words::role(*role))).collect();
        // SAFETY: The editor answers the action, and the sheet keeps the editor as long as the control.
        let roles = unsafe {
            NSSegmentedControl::segmentedControlWithLabels_trackingMode_target_action(
                &NSArray::from_retained_slice(&titles),
                NSSegmentSwitchTracking::SelectOne,
                Some(me),
                Some(sel!(ideFactoryRolesRole:)),
                mtm,
            )
        };
        roles.setAccessibilityLabel(Some(&ns(t("Rolle"))));
        let (table, list) = self.table(mtm);
        let buttons = bar_buttons(mtm, me);
        let origin = ui::note(mtm, Origin::Default.words(), 200.0);
        let reset = ui::button(mtm, t("Auf Vorgabe zurücksetzen"), me, sel!(ideFactoryRolesReset:), 0);
        reset.setControlSize(NSControlSize::Small);
        let bar = ui::row(mtm, space::M);
        bar.addArrangedSubview(&buttons);
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Horizontal);
        bar.addArrangedSubview(&spacer);
        bar.addArrangedSubview(&origin);
        bar.addArrangedSubview(&reset);
        bar.widthAnchor().constraintEqualToConstant(CONTENT).setActive(true);
        RolesUi {
            roles,
            table,
            list,
            bar,
            buttons,
            origin,
            reset,
        }
    }

    /// The list: agent, model and effort of each entry, in a scroll view as wide as the content.
    fn table(&self, mtm: MainThreadMarker) -> (Retained<NSTableView>, Retained<NSScrollView>) {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(CONTENT, HEIGHT));
        let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), frame);
        for (id, title, width) in COLUMNS {
            let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(id));
            column.setTitle(&ns(t(title)));
            column.setWidth(width);
            table.addTableColumn(&column);
        }
        table.setRowHeight(26.0);
        table.setAccessibilityLabel(Some(&ns(t("Rangliste"))));
        // SAFETY: The table holds its data source and delegate weakly; the sheet keeps the editor alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
        scroll.setDocumentView(Some(&table));
        ide_ui_common::util::fixed_size(&scroll, CONTENT, HEIGHT);
        (table, scroll)
    }

    /// The cell of `column` in `row`: the agent with its rank, or a field for model or effort with what the agent
    /// offers.
    fn cell(&self, column: &str, row: isize) -> Option<Retained<NSView>> {
        let sheet = self.ivars().sheet.load()?;
        let index = usize::try_from(row).ok()?;
        let entry = self.list().get(index)?.clone();
        let catalog = sheet.file().catalog(&entry.agent);
        let mtm = self.mtm();
        let me: &AnyObject = self.as_ref();
        let view: Retained<NSView> = match column {
            "model" => {
                let combo = cell_combo(mtm, me, sel!(ideFactoryRolesModel:), row);
                let models: Vec<String> = catalog.models.iter().map(|m| m.value.clone()).collect();
                let placeholder = ranks::default_words(catalog.default_model.as_deref());
                fill(&combo, &models, &placeholder, entry.model.as_deref());
                Retained::into_super(Retained::into_super(Retained::into_super(combo)))
            }
            "effort" => {
                let combo = cell_combo(mtm, me, sel!(ideFactoryRolesEffort:), row);
                let efforts = catalog.efforts_of(entry.model.as_deref()).to_vec();
                let placeholder = ranks::default_words(catalog.default_effort.as_deref());
                fill(&combo, &efforts, &placeholder, entry.effort.as_deref());
                Retained::into_super(Retained::into_super(Retained::into_super(combo)))
            }
            _ => {
                let words = format!("{}.  {}", index + 1, agent_name(&entry.agent));
                let cell = label(mtm, &words);
                if !takes(&entry, self.role()) {
                    cell.setTextColor(Some(&NSColor::tertiaryLabelColor()));
                    cell.setToolTip(Some(&ns(&ranks::entry_words(&entry, self.role(), &catalog))));
                }
                Retained::into_super(Retained::into_super(cell))
            }
        };
        Some(view)
    }

    /// Shows the role's list again, as after a change or a reset, with where it comes from.
    pub(crate) fn reload(&self) {
        let ui = self.ui();
        let index = Role::ALL.iter().position(|r| *r == self.role()).unwrap_or(0);
        ui.roles.setSelectedSegment(isize::try_from(index).unwrap_or(0));
        let (list, origin) = self.ranks();
        let selected = self.selected();
        ui.table.reloadData();
        if let Some(index) = selected.filter(|i| *i < list.len()) {
            self.select(index);
        }
        ui.reset.setHidden(origin == Origin::Default);
        ui.origin.setHidden(origin == Origin::File);
    }

    /// Opens the menu of the agents to add under the bar's add segment.
    fn offer_agents(&self) {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        for (index, agent) in AGENTS.iter().enumerate() {
            let item = ide_ui_common::util::tagged(
                mtm,
                &agent_name(agent),
                sel!(ideFactoryRolesAddAgent:),
                "",
                isize::try_from(index).unwrap_or(0),
            );
            // SAFETY: The editor answers the action and outlives the menu, which shows while the sheet is open.
            unsafe { item.setTarget(Some(self.as_ref())) };
            menu.addItem(&item);
        }
        let buttons = &self.ui().buttons;
        let below = NSPoint::new(0.0, buttons.frame().size.height + 4.0);
        menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(buttons));
    }

    fn remove_selected(&self) {
        if let Some(index) = self.selected() {
            self.change(|list| {
                ranks::remove(list, index);
                None
            });
        }
    }

    fn shift_selected(&self, up: bool) {
        if let Some(index) = self.selected() {
            self.change(|list| Some(ranks::shift(list, index, up)));
        }
    }

    fn selected(&self) -> Option<usize> {
        usize::try_from(self.ui().table.selectedRow()).ok()
    }

    fn deselect(&self) {
        let none = objc2_foundation::NSIndexSet::new();
        self.ui().table.selectRowIndexes_byExtendingSelection(&none, false);
    }

    fn select(&self, index: usize) {
        let rows = objc2_foundation::NSIndexSet::indexSetWithIndex(index);
        self.ui().table.selectRowIndexes_byExtendingSelection(&rows, false);
    }

    /// Changes the role's list with `edit`, which says the row to select after it, and writes it into the file when
    /// it changed.
    fn change(&self, edit: impl FnOnce(&mut Vec<RankEntry>) -> Option<usize>) {
        let Some(sheet) = self.ivars().sheet.load() else { return };
        let before = self.list();
        let mut list = before.clone();
        let select = edit(&mut list);
        if list != before {
            sheet.set_ranks(self.role(), &list);
        }
        self.reload();
        match select {
            Some(index) if index < list.len() => self.select(index),
            _ => self.deselect(),
        }
    }
}

/// The bar under the list: add, remove, up and down, as small square segments that click and spring back.
fn bar_buttons(mtm: MainThreadMarker, target: &AnyObject) -> Retained<NSSegmentedControl> {
    let segments = [
        ("plus", t("Agent hinzufügen")),
        ("minus", t("Eintrag entfernen")),
        ("chevron.up", t("Nach oben")),
        ("chevron.down", t("Nach unten")),
    ];
    let images: Vec<Retained<NSImage>> = segments
        .iter()
        .map(|(name, words)| ide_ui_common::util::symbol(name, words).unwrap_or_default())
        .collect();
    // SAFETY: The editor answers the action, and the sheet keeps the editor as long as the control.
    let control = unsafe {
        NSSegmentedControl::segmentedControlWithImages_trackingMode_target_action(
            &NSArray::from_retained_slice(&images),
            NSSegmentSwitchTracking::Momentary,
            Some(target),
            Some(sel!(ideFactoryRolesBar:)),
            mtm,
        )
    };
    control.setSegmentStyle(NSSegmentStyle::SmallSquare);
    for (segment, (_, words)) in segments.iter().enumerate() {
        control.setToolTip_forSegment(Some(&ns(words)), isize::try_from(segment).unwrap_or(0));
    }
    control
}

/// A field in a cell of the list, which says its row by its tag.
fn cell_combo(
    mtm: MainThreadMarker,
    target: &AnyObject,
    action: objc2::runtime::Sel,
    row: isize,
) -> Retained<NSComboBox> {
    let combo = NSComboBox::new(mtm);
    combo.setControlSize(NSControlSize::Small);
    combo.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(
        objc2_app_kit::NSFont::smallSystemFontSize(),
    )));
    combo.setCompletes(true);
    combo.setTag(row);
    set_action(&combo, target, action);
    combo
}

/// The row and the text of the field in the list that sent an action.
fn combo_value(sender: Option<&AnyObject>) -> Option<(usize, String)> {
    let combo = sender?.downcast_ref::<NSComboBox>()?;
    Some((usize::try_from(combo.tag()).ok()?, combo.stringValue().to_string()))
}

/// Offers `choices` in `combo`, with `placeholder` while `value` is none.
pub fn fill(combo: &NSComboBox, choices: &[String], placeholder: &str, value: Option<&str>) {
    combo.removeAllItems();
    for choice in choices {
        // SAFETY: The combo box takes strings as its items' values.
        unsafe { combo.addItemWithObjectValue(&ns(choice)) };
    }
    combo.setPlaceholderString(Some(&ns(placeholder)));
    combo.setStringValue(&ns(value.unwrap_or_default()));
}
