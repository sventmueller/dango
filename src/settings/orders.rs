//! The Software Factory tab (Einstellungen ▸ Software Factory, E183): the user's defaults for the Leitstand in every project. It offers
//! the ranked list of every role (planner, builder, reviewer, scout) with model and effort per entry, the reviewer
//! rule, the limits, the budgets per order, per day and per provider, how orders behave after a restart and on
//! battery, and the gate sandbox's level; a project's `.dango/factory.toml` can only tighten them (E190). The values are stored per Mac (E113)
//! through the Leitstand's host.

use std::cell::{Cell, OnceCell, RefCell};

use ide_factory::models::Catalog;
use ide_factory::routing::Agent;
use ide_factory::station::Role;
use ide_factory::workflow::GateSandbox;
use ide_ui_factory::model::ranks;
use ide_ui_factory::model::settings::{
    AGENTS, DefaultsForm, LIMITS, PROVIDERS, agent_name, api_service, gate_sandbox_words, limit_title, provider_title,
    read_dollars, reviewer_rules, takes,
};
use ide_ui_factory::model::words;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSComboBox, NSControlStateValueOff, NSControlStateValueOn, NSControlTextEditingDelegate,
    NSPopUpButton, NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::form::{self, Row};
use crate::util::{guarded, ns, set_action};

/// How tall the tab is; its content scrolls, since it is taller than many screens.
const PAGE_HEIGHT: f64 = 560.0;

/// The tag of the order budget's field; the day's is one more.
const ORDER_BUDGET: isize = 100;

/// The tag of the first provider's budget field; the others follow in the order of `PROVIDERS`.
const PROVIDER_BUDGET: isize = 200;

pub(crate) struct OrdersIvars {
    form: RefCell<DefaultsForm>,
    /// What each agent offers for an entry, read again with the defaults.
    catalogs: RefCell<Vec<(Agent, Catalog)>>,
    /// The role whose ranked list the table shows.
    role: Cell<Role>,
    ui: OnceCell<OrdersUi>,
}

struct OrdersUi {
    view: Retained<NSView>,
    role: Retained<NSPopUpButton>,
    table: Retained<NSTableView>,
    model: Retained<NSComboBox>,
    effort: Retained<NSComboBox>,
    reviewer: Retained<NSPopUpButton>,
    limits: Vec<Retained<NSTextField>>,
    budgets: [Retained<NSTextField>; 2],
    providers: Vec<Retained<NSTextField>>,
    resume: Retained<NSButton>,
    battery: Retained<NSButton>,
    proposals: Retained<NSButton>,
    sandbox: Retained<NSPopUpButton>,
    problem: Retained<NSTextField>,
}

/// The controls of the roles' part, before they go into the tab.
struct RolesPart {
    views: Vec<Retained<NSView>>,
    role: Retained<NSPopUpButton>,
    table: Retained<NSTableView>,
    model: Retained<NSComboBox>,
    effort: Retained<NSComboBox>,
    reviewer: Retained<NSPopUpButton>,
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
            guarded(0, || isize::try_from(self.ivars().form.borrow().list(self.role()).len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for OrdersSettings {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let form = self.ivars().form.borrow();
                let role = self.role();
                let entry = form.list(role).get(usize::try_from(row).ok()?)?;
                let cell = form::cell(self.mtm(), &ranks::entry_words(entry, role, &self.catalog(&entry.agent)));
                if !takes(entry, role) {
                    cell.setAlphaValue(0.5);
                }
                Some(cell)
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &objc2_foundation::NSNotification) {
            guarded((), || self.show_entry());
        }
    }

    impl OrdersSettings {
        #[unsafe(method(ideOrdersRole:))]
        fn role_chosen(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(self.ui().role.indexOfSelectedItem()).unwrap_or(0);
                self.ivars().role.set(Role::ALL.get(index).copied().unwrap_or(Role::Builder));
                let none = objc2_foundation::NSIndexSet::new();
                self.ui().table.selectRowIndexes_byExtendingSelection(&none, false);
                self.show();
            });
        }

        #[unsafe(method(ideOrdersAdd:))]
        fn add(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = sender
                    .and_then(|s| s.downcast_ref::<NSPopUpButton>())
                    .map_or(-1, NSPopUpButton::indexOfSelectedItem);
                // The pull-down's first item is its title.
                if let Some(agent) = usize::try_from(index - 1).ok().and_then(|i| AGENTS.get(i)) {
                    let role = self.role();
                    self.change(|form| form.add(role, agent.clone()));
                }
            });
        }

        #[unsafe(method(ideOrdersRemove:))]
        fn remove(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(index) = self.selected() {
                    let role = self.role();
                    self.change(|form| form.edit_list(role, |list| ranks::remove(list, index)));
                }
            });
        }

        #[unsafe(method(ideOrdersShift:))]
        fn shift(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let up = sender.and_then(|s| s.downcast_ref::<NSButton>()).is_some_and(|b| b.tag() == 0);
                if let Some(index) = self.selected() {
                    let (role, mut to) = (self.role(), index);
                    self.change(|form| to = form.shift(role, index, up));
                    self.select(to);
                }
            });
        }

        #[unsafe(method(ideOrdersModel:))]
        fn model(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(index) = self.selected() {
                    let role = self.role();
                    let model = self.ui().model.stringValue().to_string();
                    self.change(|form| form.edit_list(role, |list| ranks::set_model(list, index, &model)));
                    self.select(index);
                }
            });
        }

        #[unsafe(method(ideOrdersEffort:))]
        fn effort(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(index) = self.selected() {
                    let role = self.role();
                    let effort = self.ui().effort.stringValue().to_string();
                    self.change(|form| form.set_effort(role, index, &effort));
                    self.select(index);
                }
            });
        }

        #[unsafe(method(ideOrdersReviewer:))]
        fn reviewer(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let preferred = self.ui().reviewer.indexOfSelectedItem() == 1;
                self.change(|form| form.set_reviewer_preferred(preferred));
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

        #[unsafe(method(ideOrdersSandbox:))]
        fn sandbox(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(self.ui().sandbox.indexOfSelectedItem()).unwrap_or(0);
                let level = GateSandbox::ALL.get(index).copied().unwrap_or(GateSandbox::Strict);
                self.change(|form| form.set_gate_sandbox(level));
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
            catalogs: RefCell::new(catalogs()),
            role: Cell::new(Role::Builder),
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

    fn role(&self) -> Role {
        self.ivars().role.get()
    }

    /// What `agent` offers for an entry.
    fn catalog(&self, agent: &Agent) -> Catalog {
        self.ivars()
            .catalogs
            .borrow()
            .iter()
            .find(|(a, _)| a == agent)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    /// Reads the stored defaults again, as when the settings window comes to the front.
    pub(crate) fn reload(&self) {
        *self.ivars().form.borrow_mut() = DefaultsForm::new(crate::leitstand_ui::defaults());
        *self.ivars().catalogs.borrow_mut() = catalogs();
        self.show();
    }

    /// The role menu, its ranked list with model and effort of the selected entry, and the reviewer rule.
    fn roles_part(&self, mtm: MainThreadMarker) -> RolesPart {
        let me: &AnyObject = self.as_ref();
        let role = NSPopUpButton::new(mtm);
        for each in Role::ALL {
            role.addItemWithTitle(&ns(words::role(each)));
        }
        set_action(&role, me, sel!(ideOrdersRole:));
        let (table, list) = form::table(mtm, self, &[(t("Rangliste"), form::WIDTH)], 110.0);
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
        let model = combo(mtm, me, sel!(ideOrdersModel:));
        let effort = combo(mtm, me, sel!(ideOrdersEffort:));
        let reviewer = NSPopUpButton::new(mtm);
        for words in reviewer_rules() {
            reviewer.addItemWithTitle(&ns(words));
        }
        set_action(&reviewer, me, sel!(ideOrdersReviewer:));
        let entry = [
            Row::new(mtm, t("Modell:"), &model),
            Row::new(mtm, t("Denktiefe:"), &effort),
        ];
        let views = vec![
            form::heading(mtm, t("Rollen")),
            form::note(
                mtm,
                t("Die Reihenfolge ist die Rangliste: Ein Auftrag nimmt den ersten Eintrag, der bereit ist."),
            ),
            Retained::into_super(form::grid(mtm, &[Row::new(mtm, t("Rolle:"), &role)])),
            list,
            bar,
            Retained::into_super(form::grid(mtm, &entry)),
            Retained::into_super(form::grid(mtm, &[Row::new(mtm, t("Prüfer-Regel:"), &reviewer)])),
            form::note(
                mtm,
                t(
                    "Ist kein Prüfer eines anderen Herstellers bereit, wartet der Auftrag bei „Pflicht“ auf dich; bei „bevorzugt“ prüft derselbe Hersteller, und der Nachweis sagt es.",
                ),
            ),
        ];
        RolesPart {
            views,
            role,
            table,
            model,
            effort,
            reviewer,
        }
    }

    fn build(&self, mtm: MainThreadMarker) -> OrdersUi {
        let me: &AnyObject = self.as_ref();
        let roles = self.roles_part(mtm);
        let mut rows = Vec::new();
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
        let mut money = vec![
            Row::new(mtm, t("Je Auftrag ($):"), &budgets[0]),
            Row::new(mtm, t("Je Tag ($):"), &budgets[1]),
        ];
        let mut providers = Vec::new();
        for (index, agent) in PROVIDERS.iter().enumerate() {
            let field = number_field(mtm, me, PROVIDER_BUDGET + isize::try_from(index).unwrap_or(0));
            money.push(Row::new(mtm, &provider_title(agent), &field));
            providers.push(field);
        }
        let resume = checkbox(mtm, me, t("Pausierte Aufträge nach einem Neustart selbst fortsetzen"));
        let battery = checkbox(mtm, me, t("Auf Batterie für laufende Aufträge wach bleiben"));
        let proposals = checkbox(
            mtm,
            me,
            t("Nach gescheiterten Aufträgen Regeln für AGENTS.md oder CLAUDE.md vorschlagen"),
        );
        let sandbox = NSPopUpButton::new(mtm);
        for level in GateSandbox::ALL {
            sandbox.addItemWithTitle(&ns(gate_sandbox_words(level)));
        }
        set_action(&sandbox, me, sel!(ideOrdersSandbox:));
        let problem = form::problem(mtm);
        let mut views = roles.views;
        views.extend([
            form::heading(mtm, t("Grenzen")),
            Retained::into_super(form::grid(mtm, &rows)),
            form::heading(mtm, t("Budgets")),
            form::note(
                mtm,
                t("Leer heißt ohne Grenze. Ein Projekt kann Grenzen und Budgets in .dango/factory.toml nur verschärfen."),
            ),
            Retained::into_super(form::grid(mtm, &money)),
            form::heading(mtm, t("Verhalten")),
            Retained::into_super(Retained::into_super(resume.clone())),
            Retained::into_super(Retained::into_super(battery.clone())),
            Retained::into_super(Retained::into_super(proposals.clone())),
            form::note(
                mtm,
                t("Ein Späher fasst wiederkehrende Befunde zusammen; sein Vorschlag ist ein eigener Auftrag, der auf deine Freigabe und Abnahme wartet."),
            ),
            Retained::into_super(form::grid(mtm, &[Row::new(mtm, t("Sandbox der Gates:"), &sandbox)])),
            form::note(
                mtm,
                t("Gates führen Code aus, den der Agent geschrieben hat. „Gelockert“ erlaubt zusätzlich lokale Ports und Terminals, „Aus“ startet Gates mit deinen Rechten. Ein Projekt kann in .dango/factory.toml einen anderen Wert setzen."),
            ),
            Retained::into_super(Retained::into_super(problem.clone())),
        ]);
        let content = form::stack(mtm, &views);
        OrdersUi {
            view: form::scrolling_page(mtm, &content, PAGE_HEIGHT),
            role: roles.role,
            table: roles.table,
            model: roles.model,
            effort: roles.effort,
            reviewer: roles.reviewer,
            limits,
            budgets,
            providers,
            resume,
            battery,
            proposals,
            sandbox,
            problem,
        }
    }

    /// Shows the form's values in the controls.
    fn show(&self) {
        let form = self.ivars().form.borrow();
        let ui = self.ui();
        let role = Role::ALL.iter().position(|r| *r == self.role()).unwrap_or(0);
        ui.role.selectItemAtIndex(isize::try_from(role).unwrap_or(0));
        ui.table.reloadData();
        ui.reviewer.selectItemAtIndex(isize::from(form.reviewer_preferred()));
        for (limit, field) in LIMITS.into_iter().zip(&ui.limits) {
            field.setStringValue(&ns(&form.limit(limit).to_string()));
        }
        let dollars = |amount: Option<f64>| amount.map(|d| format!("{d:.2}")).unwrap_or_default();
        let (order, day) = form.budgets();
        for (field, amount) in ui.budgets.iter().zip([order, day]) {
            field.setStringValue(&ns(&dollars(amount)));
        }
        for (agent, field) in PROVIDERS.iter().zip(&ui.providers) {
            field.setStringValue(&ns(&dollars(form.provider_budget(&api_service(agent)))));
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
        let level = GateSandbox::ALL
            .iter()
            .position(|l| *l == form.gate_sandbox())
            .unwrap_or(0);
        ui.sandbox.selectItemAtIndex(isize::try_from(level).unwrap_or(0));
        drop(form);
        self.show_entry();
    }

    /// Shows model and effort of the selected entry.
    fn show_entry(&self) {
        let form = self.ivars().form.borrow();
        let ui = self.ui();
        let entry = self
            .selected()
            .and_then(|index| form.list(self.role()).get(index).cloned());
        let catalog = entry.as_ref().map(|e| self.catalog(&e.agent)).unwrap_or_default();
        let model = entry.as_ref().and_then(|e| e.model.clone());
        let effort = entry.as_ref().and_then(|e| e.effort.clone());
        let models: Vec<String> = catalog.models.iter().map(|m| m.value.clone()).collect();
        let placeholder = ranks::default_words(catalog.default_model.as_deref());
        ide_ui_factory::fill_choices(&ui.model, &models, &placeholder, model.as_deref());
        let efforts = catalog.efforts_of(model.as_deref()).to_vec();
        let placeholder = ranks::default_words(catalog.default_effort.as_deref());
        ide_ui_factory::fill_choices(&ui.effort, &efforts, &placeholder, effort.as_deref());
        ui.model.setEnabled(entry.is_some());
        ui.effort.setEnabled(entry.is_some());
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
            if tag >= PROVIDER_BUDGET {
                let agent = usize::try_from(tag - PROVIDER_BUDGET)
                    .ok()
                    .and_then(|i| PROVIDERS.get(i));
                if let Some(agent) = agent {
                    self.change(|form| form.set_provider_budget(&api_service(agent), dollars));
                }
                return;
            }
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

/// What every agent of the ranked lists offers, from its CLI's files.
fn catalogs() -> Vec<(Agent, Catalog)> {
    AGENTS
        .iter()
        .map(|agent| (agent.clone(), crate::leitstand::models::of(agent, None)))
        .collect()
}

/// A field that offers the models or efforts and takes a typed one.
fn combo(mtm: MainThreadMarker, target: &AnyObject, action: objc2::runtime::Sel) -> Retained<NSComboBox> {
    let combo = NSComboBox::new(mtm);
    combo.setCompletes(true);
    set_action(&combo, target, action);
    combo
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
