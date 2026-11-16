//! The roles' editor of "Software Factory für <Ordner>": a role's ranked list with its entries added, removed and
//! moved, and the model and effort of the selected entry from what its agent offers or typed; every change goes into
//! the project's workflow file at once, and the rows above show it with its origin.

use std::cell::{Cell, OnceCell};

use ide_factory::routing::RankEntry;
use ide_factory::station::Role;
use ide_ui_common::util::{guarded, label, ns, set_action};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSComboBox, NSControlTextEditingDelegate, NSPopUpButton, NSScrollView, NSStackView,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::ProjectSheet;
use crate::model::ranks;
use crate::model::settings::{AGENTS, agent_name, takes};
use crate::model::words;
use crate::ui;
use ide_ui_common::style::space;

/// How wide the list and the fields of model and effort are.
const WIDTH: f64 = 420.0;

pub(crate) struct RolesIvars {
    sheet: Weak<ProjectSheet>,
    role: Cell<Role>,
    ui: OnceCell<RolesUi>,
}

struct RolesUi {
    view: Retained<NSStackView>,
    role: Retained<NSPopUpButton>,
    table: Retained<NSTableView>,
    model: Retained<NSComboBox>,
    effort: Retained<NSComboBox>,
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
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let sheet = self.ivars().sheet.load()?;
                let entry = self.list().get(usize::try_from(row).ok()?)?.clone();
                let role = self.role();
                let words = ranks::entry_words(&entry, role, &sheet.file().catalog(&entry.agent));
                let cell = label(self.mtm(), &words);
                cell.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingMiddle);
                if !takes(&entry, role) {
                    cell.setAlphaValue(0.5);
                }
                Some(Retained::into_super(Retained::into_super(cell)))
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &objc2_foundation::NSNotification) {
            guarded((), || self.show_entry());
        }
    }

    impl RolesEditor {
        #[unsafe(method(ideFactoryRolesRole:))]
        fn role_chosen(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(self.ui().role.indexOfSelectedItem()).unwrap_or(0);
                self.ivars().role.set(Role::ALL.get(index).copied().unwrap_or(Role::Builder));
                self.deselect();
                self.reload();
            });
        }

        #[unsafe(method(ideFactoryRolesAdd:))]
        fn add(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = sender
                    .and_then(|s| s.downcast_ref::<NSPopUpButton>())
                    .map_or(-1, NSPopUpButton::indexOfSelectedItem);
                // The pull-down's first item is its title.
                if let Some(agent) = usize::try_from(index - 1).ok().and_then(|i| AGENTS.get(i)) {
                    let agent = agent.clone();
                    self.change(|list| {
                        ranks::add(list, agent);
                        list.len().checked_sub(1)
                    });
                }
            });
        }

        #[unsafe(method(ideFactoryRolesRemove:))]
        fn remove(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(index) = self.selected() {
                    self.change(|list| {
                        ranks::remove(list, index);
                        None
                    });
                }
            });
        }

        #[unsafe(method(ideFactoryRolesShift:))]
        fn shift(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let up = sender.and_then(|s| s.downcast_ref::<NSButton>()).is_some_and(|b| b.tag() == 0);
                if let Some(index) = self.selected() {
                    self.change(|list| Some(ranks::shift(list, index, up)));
                }
            });
        }

        #[unsafe(method(ideFactoryRolesModel:))]
        fn model(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let model = self.ui().model.stringValue().to_string();
                if let Some(index) = self.selected() {
                    self.change(|list| {
                        ranks::set_model(list, index, &model);
                        Some(index)
                    });
                }
            });
        }

        #[unsafe(method(ideFactoryRolesEffort:))]
        fn effort(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let effort = self.ui().effort.stringValue().to_string();
                if let Some(index) = self.selected() {
                    self.change(|list| {
                        ranks::set_effort(list, index, &effort);
                        Some(index)
                    });
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

    /// The editor's views.
    pub(crate) fn view(&self) -> Retained<NSView> {
        Retained::into_super(self.ui().view.clone())
    }

    fn ui(&self) -> &RolesUi {
        self.ivars().ui.get().expect("the editor builds its views in `new`")
    }

    fn role(&self) -> Role {
        self.ivars().role.get()
    }

    /// The list of the role shown, as the file says now.
    fn list(&self) -> Vec<RankEntry> {
        self.ivars()
            .sheet
            .load()
            .map(|sheet| sheet.file().ranks(self.role()).0)
            .unwrap_or_default()
    }

    fn build(&self, mtm: MainThreadMarker) -> RolesUi {
        let me: &AnyObject = self.as_ref();
        let role = NSPopUpButton::new(mtm);
        for each in Role::ALL {
            role.addItemWithTitle(&ns(words::role(each)));
        }
        set_action(&role, me, sel!(ideFactoryRolesRole:));
        let (table, list) = self.table(mtm);
        let add = NSPopUpButton::new(mtm);
        add.setPullsDown(true);
        add.addItemWithTitle(&ns(t("Hinzufügen")));
        for agent in &AGENTS {
            add.addItemWithTitle(&ns(&agent_name(agent)));
        }
        set_action(&add, me, sel!(ideFactoryRolesAdd:));
        ide_ui_common::util::set_label(&add, Some(t("Agent hinzufügen")));
        let remove = ui::button(mtm, t("Eintrag entfernen"), me, sel!(ideFactoryRolesRemove:), 0);
        let up = ui::button(mtm, t("Nach oben"), me, sel!(ideFactoryRolesShift:), 0);
        let down = ui::button(mtm, t("Nach unten"), me, sel!(ideFactoryRolesShift:), 1);
        let bar = ui::row(mtm, space::S);
        bar.addArrangedSubview(&remove);
        bar.addArrangedSubview(&add);
        bar.addArrangedSubview(&up);
        bar.addArrangedSubview(&down);
        let model = combo(mtm, me, sel!(ideFactoryRolesModel:));
        let effort = combo(mtm, me, sel!(ideFactoryRolesEffort:));
        let line = |title: &str, control: &NSView| {
            let line = ui::row(mtm, space::M);
            line.addArrangedSubview(&label(mtm, title));
            line.addArrangedSubview(control);
            line
        };
        let view = ui::column(mtm, space::S);
        view.addArrangedSubview(&line(t("Rolle:"), &role));
        view.addArrangedSubview(&list);
        view.addArrangedSubview(&bar);
        view.addArrangedSubview(&line(t("Modell:"), &model));
        view.addArrangedSubview(&line(t("Denktiefe:"), &effort));
        RolesUi {
            view,
            role,
            table,
            model,
            effort,
        }
    }

    /// The list of entries, one column, in a scroll view.
    fn table(&self, mtm: MainThreadMarker) -> (Retained<NSTableView>, Retained<NSScrollView>) {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 100.0));
        let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), frame);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("entry"));
        column.setTitle(&ns(t("Rangliste")));
        column.setWidth(WIDTH);
        table.addTableColumn(&column);
        table.setUsesAlternatingRowBackgroundColors(true);
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
        ide_ui_common::util::fixed_size(&scroll, WIDTH, 100.0);
        (table, scroll)
    }

    /// Shows the role's list again, as after a change or a reset in the rows above.
    pub(crate) fn reload(&self) {
        let ui = self.ui();
        let index = Role::ALL.iter().position(|r| *r == self.role()).unwrap_or(0);
        ui.role.selectItemAtIndex(isize::try_from(index).unwrap_or(0));
        let selected = self.selected();
        ui.table.reloadData();
        if let Some(index) = selected.filter(|i| *i < self.list().len()) {
            self.select(index);
        }
        self.show_entry();
    }

    /// Fills model and effort from the selected entry and what its agent offers.
    fn show_entry(&self) {
        let ui = self.ui();
        let entry = self.selected().and_then(|index| self.list().get(index).cloned());
        let catalog = match (&entry, self.ivars().sheet.load()) {
            (Some(entry), Some(sheet)) => sheet.file().catalog(&entry.agent),
            _ => ide_factory::models::Catalog::default(),
        };
        let model = entry.as_ref().and_then(|e| e.model.clone());
        let models: Vec<String> = catalog.models.iter().map(|m| m.value.clone()).collect();
        fill(
            &ui.model,
            &models,
            &ranks::default_words(catalog.default_model.as_deref()),
            model.as_deref(),
        );
        let efforts = catalog.efforts_of(model.as_deref()).to_vec();
        let effort = entry.as_ref().and_then(|e| e.effort.clone());
        fill(
            &ui.effort,
            &efforts,
            &ranks::default_words(catalog.default_effort.as_deref()),
            effort.as_deref(),
        );
        ui.model.setEnabled(entry.is_some());
        ui.effort.setEnabled(entry.is_some());
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

    /// Changes the role's list with `edit`, which says the row to select after it, and writes it into the file.
    fn change(&self, edit: impl FnOnce(&mut Vec<RankEntry>) -> Option<usize>) {
        let Some(sheet) = self.ivars().sheet.load() else { return };
        let before = self.list();
        let mut list = before.clone();
        let select = edit(&mut list);
        if list != before {
            sheet.set_ranks(self.role(), &list);
        }
        self.ui().table.reloadData();
        match select {
            Some(index) if index < list.len() => self.select(index),
            _ => self.deselect(),
        }
        self.show_entry();
    }
}

/// A field that offers choices and takes a typed value, which sends `action` to `target`.
pub(crate) fn combo(mtm: MainThreadMarker, target: &AnyObject, action: objc2::runtime::Sel) -> Retained<NSComboBox> {
    let combo = NSComboBox::new(mtm);
    combo.setCompletes(true);
    combo.widthAnchor().constraintEqualToConstant(260.0).setActive(true);
    set_action(&combo, target, action);
    combo
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
