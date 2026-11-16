//! The Orders tab (Einstellungen ▸ Aufträge, E183): the user's defaults for the Leitstand in every project. Stage 1
//! offers the builder's ranked list, the limits, the budgets per order and per day, and how orders behave after a
//! restart and on battery; a project's `.dango/factory.toml` can only tighten them (E190). The values are stored per
//! Mac (E113) through the Leitstand's host.

use std::cell::{OnceCell, RefCell};

use ide_factory::station::Role;
use ide_ui_factory::model::settings::{
    AGENTS, DefaultsForm, LIMITS, agent_name, entry_words, limit_title, read_dollars, takes,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSControlTextEditingDelegate, NSPopUpButton,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::form::{self, Row};
use crate::util::{guarded, ns, set_action};

/// The tag of the order budget's field; the day's is one more.
const ORDER_BUDGET: isize = 100;

pub(crate) struct OrdersIvars {
    form: RefCell<DefaultsForm>,
    ui: OnceCell<OrdersUi>,
}

struct OrdersUi {
    view: Retained<NSView>,
    table: Retained<NSTableView>,
    model: Retained<NSTextField>,
    limits: Vec<Retained<NSTextField>>,
    budgets: [Retained<NSTextField>; 2],
    resume: Retained<NSButton>,
    battery: Retained<NSButton>,
    proposals: Retained<NSButton>,
    problem: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `OrdersSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEOrdersSettings"]
    #[ivars = OrdersIvars]
    pub(crate) struct OrdersSettings;

    unsafe impl NSObjectProtocol for OrdersSettings {}

    unsafe impl NSControlTextEditingDelegate for OrdersSettings {}

    unsafe impl NSTableViewDataSource for OrdersSettings {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().form.borrow().list(Role::Builder).len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for OrdersSettings {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let form = self.ivars().form.borrow();
                let entry = form.list(Role::Builder).get(usize::try_from(row).ok()?)?;
                let cell = form::cell(self.mtm(), &entry_words(entry, Role::Builder));
                if !takes(entry, Role::Builder) {
                    cell.setAlphaValue(0.5);
                }
                Some(cell)
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &objc2_foundation::NSNotification) {
            guarded((), || self.show_model());
        }
    }

    impl OrdersSettings {
        #[unsafe(method(ideOrdersAdd:))]
        fn add(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = sender
                    .and_then(|s| s.downcast_ref::<NSPopUpButton>())
                    .map_or(-1, NSPopUpButton::indexOfSelectedItem);
                // The pull-down's first item is its title.
                if let Some(agent) = usize::try_from(index - 1).ok().and_then(|i| AGENTS.get(i)) {
                    self.change(|form| form.add(Role::Builder, agent.clone()));
                }
            });
        }

        #[unsafe(method(ideOrdersRemove:))]
        fn remove(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(index) = self.selected() {
                    self.change(|form| form.remove(Role::Builder, index));
                }
            });
        }

        #[unsafe(method(ideOrdersShift:))]
        fn shift(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let up = sender.and_then(|s| s.downcast_ref::<NSButton>()).is_some_and(|b| b.tag() == 0);
                if let Some(index) = self.selected() {
                    let mut to = index;
                    self.change(|form| to = form.shift(Role::Builder, index, up));
                    self.select(to);
                }
            });
        }

        #[unsafe(method(ideOrdersModel:))]
        fn model(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(index) = self.selected() {
                    let model = self.ui().model.stringValue().to_string();
                    self.change(|form| form.set_model(Role::Builder, index, &model));
                    self.select(index);
                }
            });
        }

        #[unsafe(method(ideOrdersValue:))]
        fn value(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(field) = sender.and_then(|s| s.downcast_ref::<NSTextField>()) {
                    self.value_typed(field);
                }
            });
        }

        #[unsafe(method(ideOrdersSwitch:))]
        fn switch(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let ui = self.ui();
                let on = |button: &NSButton| button.state() == NSControlStateValueOn;
                let (resume, battery, proposals) = (on(&ui.resume), on(&ui.battery), on(&ui.proposals));
                self.change(|form| {
                    form.set_resume_after_restart(resume);
                    form.set_awake_on_battery(battery);
                    form.set_rule_proposals(proposals);
                });
            });
        }
    }
);

impl OrdersSettings {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<OrdersSettings> {
        let this = Self::alloc(mtm).set_ivars(OrdersIvars {
            form: RefCell::new(DefaultsForm::new(crate::leitstand_ui::defaults())),
            ui: OnceCell::new(),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<OrdersSettings> = unsafe { msg_send![super(this), init] };
        let _ = this.ivars().ui.set(this.build(mtm));
        this.show();
        this
    }

    /// The tab's view.
    pub(crate) fn view(&self) -> Retained<NSView> {
        self.ui().view.clone()
    }

    fn ui(&self) -> &OrdersUi {
        self.ivars().ui.get().expect("the tab builds its views in `new`")
    }

    /// Reads the stored defaults again, as when the settings window comes to the front.
    pub(crate) fn reload(&self) {
        *self.ivars().form.borrow_mut() = DefaultsForm::new(crate::leitstand_ui::defaults());
        self.show();
    }

    fn build(&self, mtm: MainThreadMarker) -> OrdersUi {
        let me: &AnyObject = self.as_ref();
        let (table, list) = form::table(mtm, self, &[(t("Builder"), form::WIDTH)], 110.0);
        let add = NSPopUpButton::new(mtm);
        add.setPullsDown(true);
        add.addItemWithTitle(&ns(t("Hinzufügen")));
        for agent in &AGENTS {
            add.addItemWithTitle(&ns(&agent_name(agent)));
        }
        set_action(&add, me, sel!(ideOrdersAdd:));
        crate::util::set_label(&add, Some(t("Agent hinzufügen")));
        let remove = super::list_bar::button(mtm, false, t("Eintrag entfernen"), me, sel!(ideOrdersRemove:));
        let up = button(mtm, t("Nach oben"), me, sel!(ideOrdersShift:), 0);
        let down = button(mtm, t("Nach unten"), me, sel!(ideOrdersShift:), 1);
        let bar = super::list_bar::bar(mtm, &[&remove], &[&add, &up, &down]);
        let model = NSTextField::textFieldWithString(&ns(""), mtm);
        model.setPlaceholderString(Some(&ns(t("Vorgabe des Agenten"))));
        set_action(&model, me, sel!(ideOrdersModel:));
        let mut rows = vec![Row::new(mtm, t("Modell:"), &model)];
        let mut limits = Vec::new();
        for (index, limit) in LIMITS.into_iter().enumerate() {
            let field = number_field(mtm, me, isize::try_from(index).unwrap_or(0));
            rows.push(Row::new(mtm, limit_title(limit), &field));
            limits.push(field);
        }
        let budgets = [
            number_field(mtm, me, ORDER_BUDGET),
            number_field(mtm, me, ORDER_BUDGET + 1),
        ];
        let money = vec![
            Row::new(mtm, t("Je Auftrag ($):"), &budgets[0]),
            Row::new(mtm, t("Je Tag ($):"), &budgets[1]),
        ];
        let resume = checkbox(mtm, me, t("Pausierte Aufträge nach einem Neustart selbst fortsetzen"));
        let battery = checkbox(mtm, me, t("Auf Batterie für laufende Aufträge wach bleiben"));
        let proposals = checkbox(
            mtm,
            me,
            t("Nach gescheiterten Aufträgen Regeln für AGENTS.md oder CLAUDE.md vorschlagen"),
        );
        let problem = form::problem(mtm);
        let content = form::stack(
            mtm,
            &[
                form::heading(mtm, t("Builder")),
                form::note(
                    mtm,
                    t("Die Reihenfolge ist die Rangliste: Ein Auftrag nimmt den ersten Eintrag, der bereit ist."),
                ),
                list,
                bar,
                Retained::into_super(form::grid(mtm, &rows[..1])),
                form::heading(mtm, t("Grenzen")),
                Retained::into_super(form::grid(mtm, &rows[1..])),
                form::heading(mtm, t("Budgets")),
                form::note(
                    mtm,
                    t(
                        "Leer heißt ohne Grenze. Ein Projekt kann Grenzen und Budgets in .dango/factory.toml nur verschärfen.",
                    ),
                ),
                Retained::into_super(form::grid(mtm, &money)),
                form::heading(mtm, t("Verhalten")),
                Retained::into_super(Retained::into_super(resume.clone())),
                Retained::into_super(Retained::into_super(battery.clone())),
                Retained::into_super(Retained::into_super(proposals.clone())),
                form::note(
                    mtm,
                    t(
                        "Ein Späher fasst wiederkehrende Befunde zusammen; sein Vorschlag ist ein eigener Auftrag, der auf deine Freigabe und Abnahme wartet.",
                    ),
                ),
                Retained::into_super(Retained::into_super(problem.clone())),
            ],
        );
        OrdersUi {
            view: form::page(mtm, &content),
            table,
            model,
            limits,
            budgets,
            resume,
            battery,
            proposals,
            problem,
        }
    }

    /// Shows the form's values in the controls.
    fn show(&self) {
        let form = self.ivars().form.borrow();
        let ui = self.ui();
        ui.table.reloadData();
        for (limit, field) in LIMITS.into_iter().zip(&ui.limits) {
            field.setStringValue(&ns(&form.limit(limit).to_string()));
        }
        let (order, day) = form.budgets();
        for (field, dollars) in ui.budgets.iter().zip([order, day]) {
            field.setStringValue(&ns(&dollars.map(|d| format!("{d:.2}")).unwrap_or_default()));
        }
        let state = |on: bool| {
            if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            }
        };
        ui.resume.setState(state(form.resume_after_restart()));
        ui.battery.setState(state(form.awake_on_battery()));
        ui.proposals.setState(state(form.rule_proposals()));
        drop(form);
        self.show_model();
    }

    fn show_model(&self) {
        let form = self.ivars().form.borrow();
        let ui = self.ui();
        let model = self
            .selected()
            .and_then(|index| form.list(Role::Builder).get(index))
            .and_then(|entry| entry.model.clone());
        ui.model.setStringValue(&ns(&model.unwrap_or_default()));
        ui.model.setEnabled(self.selected().is_some());
    }

    fn selected(&self) -> Option<usize> {
        usize::try_from(self.ui().table.selectedRow()).ok()
    }

    fn select(&self, index: usize) {
        let rows = objc2_foundation::NSIndexSet::indexSetWithIndex(index);
        self.ui().table.selectRowIndexes_byExtendingSelection(&rows, false);
    }

    /// Changes the form with `edit`, stores the defaults and shows them.
    fn change(&self, edit: impl FnOnce(&mut DefaultsForm)) {
        edit(&mut self.ivars().form.borrow_mut());
        crate::leitstand_ui::set_defaults(self.ivars().form.borrow().defaults());
        let selected = self.selected();
        self.show();
        if let Some(index) = selected {
            self.select(index);
        }
    }

    fn value_typed(&self, field: &NSTextField) {
        let text = field.stringValue().to_string();
        let tag = field.tag();
        let ui = self.ui();
        ui.problem.setHidden(true);
        if tag >= ORDER_BUDGET {
            let Ok(dollars) = read_dollars(&text) else {
                return self.problem(t("Ein Budget ist ein Betrag in Dollar, etwa 4,50."));
            };
            let (order, day) = self.ivars().form.borrow().budgets();
            let (order, day) = if tag == ORDER_BUDGET {
                (dollars, day)
            } else {
                (order, dollars)
            };
            self.change(|form| form.set_budgets(order, day));
            return;
        }
        let Some(limit) = usize::try_from(tag).ok().and_then(|i| LIMITS.get(i).copied()) else {
            return;
        };
        match text.trim().parse::<u64>() {
            Ok(value) if value > 0 => self.change(|form| form.set_limit(limit, value)),
            _ => self.problem(t("Eine Grenze ist eine ganze Zahl ab 1.")),
        }
    }

    fn problem(&self, words: &str) {
        let problem = &self.ui().problem;
        problem.setStringValue(&ns(words));
        problem.setTextColor(Some(&NSColor::systemRedColor()));
        form::show(problem, true);
        self.show();
    }
}

fn number_field(mtm: MainThreadMarker, target: &AnyObject, tag: isize) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setTag(tag);
    set_action(&field, target, sel!(ideOrdersValue:));
    field.widthAnchor().constraintEqualToConstant(80.0).setActive(true);
    field
}

fn checkbox(mtm: MainThreadMarker, target: &AnyObject, title: &str) -> Retained<NSButton> {
    // SAFETY: Without a target the checkbox sends nothing until `set_action` gives it the tab.
    let button = unsafe { NSButton::checkboxWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, sel!(ideOrdersSwitch:));
    button
}

fn button(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: objc2::runtime::Sel,
    tag: isize,
) -> Retained<NSButton> {
    // SAFETY: Without a target the button sends nothing until `set_action` gives it the tab.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, action);
    button.setTag(tag);
    button
}
