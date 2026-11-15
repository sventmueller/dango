//! The Agent tab of the settings (E6): the providers and endpoints with their keys, which the keychain keeps; the
//! model, effort and prices of new threads of the own agent; the rules stored per project (E27), which only the user
//! adds and which can be removed here; the folders the thread's sandbox lets the agent reach besides the project
//! (E19); and the model for commit messages with the permissions given for it (E88).

use ide_core::explain::Explain;
use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

use ide_agent::{Endpoint, Price, ProviderKind, Rule};
use ide_core::settings::Settings;
use ide_ui_common::alert::{Alert, Role, first};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSColor, NSComboBox, NSControlTextEditingDelegate, NSLayoutAttribute, NSModalResponseOK,
    NSOpenPanel, NSPopUpButton, NSScrollView, NSSecureTextField, NSStackView, NSTableColumn, NSTableView,
    NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::form::{self, Row};
use super::list_bar;
use crate::util::{activate, guarded, ns, pin_inset, set_action};
use ide_ui_agent::own_agent::{self, ModelChoice};

/// The height of the endpoints' table, which lists the services and the user's endpoints.
const TABLE_HEIGHT: f64 = 120.0;
/// The height of the other tables, which start empty.
const SHORT_TABLE: f64 = 82.0;
/// The tallest the tab gets; it scrolls beyond, so that the window fits a 13-inch screen.
const MAX_HEIGHT: f64 = 640.0;

/// The tab's lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum List {
    Endpoints,
    Prices,
    Rules,
    Folders,
}

pub(crate) struct AgentIvars {
    endpoints: RefCell<Vec<Endpoint>>,
    prices: RefCell<Vec<(String, Price)>>,
    rules: RefCell<Vec<(String, usize, Rule)>>,
    folders: RefCell<Vec<PathBuf>>,
    views: OnceCell<Views>,
}

struct Views {
    endpoints: Retained<NSTableView>,
    prices: Retained<NSTableView>,
    rules: Retained<NSTableView>,
    folders: Retained<NSTableView>,
    remove_endpoint: Retained<NSButton>,
    key: Retained<NSButton>,
    address: Retained<NSButton>,
    remove_price: Retained<NSButton>,
    remove_rule: Retained<NSButton>,
    remove_folder: Retained<NSButton>,
    endpoint: Retained<NSPopUpButton>,
    model: Retained<NSComboBox>,
    effort: Retained<NSPopUpButton>,
    status: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `AgentSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentSettings"]
    #[ivars = AgentIvars]
    pub(crate) struct AgentSettings;

    unsafe impl NSObjectProtocol for AgentSettings {}

    unsafe impl NSControlTextEditingDelegate for AgentSettings {}

    unsafe impl NSTableViewDataSource for AgentSettings {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.rows(self.list_of(table)).len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for AgentSettings {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let index: usize = column?.identifier().to_string().parse().ok()?;
                let row = self.rows(self.list_of(table)).into_iter().nth(usize::try_from(row).ok()?)?;
                Some(form::cell(self.mtm(), row.get(index)?))
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &NSNotification) {
            guarded((), || self.update_buttons());
        }
    }

    impl AgentSettings {
        #[unsafe(method(ideAddAgentEndpoint:))]
        fn add_endpoint_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_endpoint());
        }

        #[unsafe(method(ideRemoveAgentEndpoint:))]
        fn remove_endpoint_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_endpoint());
        }

        #[unsafe(method(ideAgentKey:))]
        fn key_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_key());
        }

        #[unsafe(method(ideAgentAddress:))]
        fn address_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_address());
        }

        #[unsafe(method(ideAgentDefaultEndpoint:))]
        fn default_endpoint_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.endpoint_chosen());
        }

        #[unsafe(method(ideAgentDefaultModel:))]
        fn default_model_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.store_choice());
        }

        #[unsafe(method(ideAgentEffort:))]
        fn effort_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.store_effort());
        }

        #[unsafe(method(ideAddAgentPrice:))]
        fn add_price_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_price());
        }

        #[unsafe(method(ideRemoveAgentPrice:))]
        fn remove_price_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected(List::Prices));
        }

        #[unsafe(method(ideRemoveAgentRule:))]
        fn remove_rule_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected(List::Rules));
        }

        #[unsafe(method(ideAddAgentFolder:))]
        fn add_folder_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_folders());
        }

        #[unsafe(method(ideRemoveAgentFolder:))]
        fn remove_folder_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected(List::Folders));
        }
    }
);

impl AgentSettings {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<AgentSettings> {
        let this = Self::alloc(mtm).set_ivars(AgentIvars {
            endpoints: RefCell::new(Vec::new()),
            prices: RefCell::new(Vec::new()),
            rules: RefCell::new(Vec::new()),
            folders: RefCell::new(Vec::new()),
            views: OnceCell::new(),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<AgentSettings> = unsafe { msg_send![super(this), init] };
        this.read();
        this
    }

    /// Reads everything again and shows it.
    pub(crate) fn reload(&self) {
        self.read();
        if let Some(views) = self.ivars().views.get() {
            views.endpoints.reloadData();
            views.prices.reloadData();
            views.rules.reloadData();
            views.folders.reloadData();
            self.fill_defaults(views);
        }
        self.update_buttons();
    }

    fn read(&self) {
        let settings = Settings::load_user();
        *self.ivars().endpoints.borrow_mut() = own_agent::endpoints();
        let mut prices: Vec<(String, Price)> = settings.get(&own_agent::PRICES).models.into_iter().collect();
        prices.sort_by(|a, b| a.0.cmp(&b.0));
        *self.ivars().prices.borrow_mut() = prices;
        *self.ivars().rules.borrow_mut() = settings
            .get(&own_agent::RULES)
            .into_iter()
            .flat_map(|(project, rules)| {
                rules
                    .into_iter()
                    .enumerate()
                    .map(move |(n, rule)| (project.clone(), n, rule))
            })
            .collect();
        *self.ivars().folders.borrow_mut() = settings.get(&own_agent::ALLOWED);
    }

    fn list_of(&self, table: &NSTableView) -> List {
        match self.ivars().views.get() {
            Some(views) if std::ptr::eq(table, &*views.prices) => List::Prices,
            Some(views) if std::ptr::eq(table, &*views.rules) => List::Rules,
            Some(views) if std::ptr::eq(table, &*views.folders) => List::Folders,
            _ => List::Endpoints,
        }
    }

    /// The rows of `list` as its table shows them.
    pub(crate) fn rows(&self, list: List) -> Vec<Vec<String>> {
        let dollars = |value: f64| format!("{value:.2}");
        match list {
            List::Endpoints => self
                .ivars()
                .endpoints
                .borrow()
                .iter()
                .map(|e| {
                    let key = if !e.kind.takes_key() {
                        t("nicht nötig").to_string()
                    } else if own_agent::key(e).is_some() {
                        t("im Schlüsselbund").to_string()
                    } else {
                        "—".to_string()
                    };
                    vec![e.name.clone(), e.base_url.clone(), key]
                })
                .collect(),
            List::Prices => self
                .ivars()
                .prices
                .borrow()
                .iter()
                .map(|(model, price)| vec![model.clone(), dollars(price.input), dollars(price.output)])
                .collect(),
            List::Rules => self
                .ivars()
                .rules
                .borrow()
                .iter()
                .map(|(project, _, rule)| vec![project.clone(), own_agent::rule_text(rule)])
                .collect(),
            List::Folders => self
                .ivars()
                .folders
                .borrow()
                .iter()
                .map(|folder| vec![crate::app::display_path(folder)])
                .collect(),
        }
    }

    fn selected(&self, list: List) -> Option<usize> {
        let views = self.ivars().views.get()?;
        let table = match list {
            List::Endpoints => &views.endpoints,
            List::Prices => &views.prices,
            List::Rules => &views.rules,
            List::Folders => &views.folders,
        };
        usize::try_from(table.selectedRow()).ok()
    }

    fn selected_endpoint(&self) -> Option<Endpoint> {
        let row = self.selected(List::Endpoints)?;
        self.ivars().endpoints.borrow().get(row).cloned()
    }

    fn update_buttons(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let endpoint = self.selected_endpoint();
        let custom = endpoint
            .as_ref()
            .is_some_and(|e| ProviderKind::from_id(&e.id).is_none());
        let moved = endpoint
            .as_ref()
            .is_some_and(|e| !custom && e.kind.default_base().is_some_and(|base| base != e.base_url));
        views.remove_endpoint.setEnabled(custom || moved);
        views.key.setEnabled(endpoint.is_some());
        views.address.setEnabled(endpoint.is_some());
        views.remove_price.setEnabled(self.selected(List::Prices).is_some());
        views.remove_rule.setEnabled(self.selected(List::Rules).is_some());
        views.remove_folder.setEnabled(self.selected(List::Folders).is_some());
    }

    fn set_status(&self, text: Option<&str>, problem: bool) {
        let Some(views) = self.ivars().views.get() else { return };
        views.status.setStringValue(&ns(text.unwrap_or("")));
        form::show(&views.status, text.is_some());
        let colour = if problem {
            NSColor::systemRedColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        views.status.setTextColor(Some(&colour));
    }

    fn remove_endpoint(&self) {
        let Some(endpoint) = self.selected_endpoint() else {
            return;
        };
        own_agent::remove_endpoint(&endpoint.id);
        self.reload();
    }

    fn remove_selected(&self, list: List) {
        let Some(row) = self.selected(list) else { return };
        match list {
            List::Prices => {
                let Some((model, _)) = self.ivars().prices.borrow().get(row).cloned() else {
                    return;
                };
                let mut settings = Settings::load_user();
                let mut prices = settings.get(&own_agent::PRICES);
                prices.models.remove(&model);
                if let Err(error) = settings.set(&own_agent::PRICES, &prices) {
                    crate::diag::report("saving the agent's prices", error);
                }
            }
            List::Rules => {
                let Some((project, index, _)) = self.ivars().rules.borrow().get(row).cloned() else {
                    return;
                };
                own_agent::remove_rule(&project, index);
            }
            List::Folders => {
                let mut folders = self.ivars().folders.borrow().clone();
                if row < folders.len() {
                    folders.remove(row);
                    store_folders(&folders);
                }
            }
            List::Endpoints => {}
        }
        self.reload();
    }
}

fn store_folders(folders: &[PathBuf]) {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&own_agent::ALLOWED, &folders.to_vec()) {
        crate::diag::report("saving the agent's sandbox folders", error);
    }
}

/// Sheets that ask for an endpoint, a key, an address, a price and folders.
impl AgentSettings {
    /// Asks for folders the sandbox lets the agent reach, and keeps those not listed yet.
    fn ask_folders(&self) {
        let panel = NSOpenPanel::openPanel(self.mtm());
        panel.setCanChooseDirectories(true);
        panel.setCanChooseFiles(false);
        panel.setAllowsMultipleSelection(true);
        panel.setPrompt(Some(&ns(t("Erlauben"))));
        if panel.runModal() != NSModalResponseOK {
            return;
        }
        let picked: Vec<PathBuf> = panel
            .URLs()
            .iter()
            .filter_map(|url| url.path())
            .map(|path| PathBuf::from(path.to_string()))
            .collect();
        self.add_folders(picked);
    }

    fn add_folders(&self, picked: Vec<PathBuf>) {
        let mut folders = self.ivars().folders.borrow().clone();
        for folder in picked {
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        store_folders(&folders);
        self.reload();
    }

    fn window(&self) -> Option<Retained<objc2_app_kit::NSWindow>> {
        self.ivars().views.get().and_then(|v| v.endpoints.window())
    }

    fn alert(&self, title: &str, info: &str, fields: &[Retained<NSView>]) -> Alert {
        let mtm = self.mtm();
        let accessory = NSStackView::new(mtm);
        accessory.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        accessory.setAlignment(NSLayoutAttribute::Leading);
        accessory.setSpacing(ide_ui_common::style::space::M);
        for field in fields {
            accessory.addArrangedSubview(field);
            field.widthAnchor().constraintEqualToConstant(320.0).setActive(true);
        }
        let height = 30.0 * f64::from(u32::try_from(fields.len()).unwrap_or(1));
        accessory.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, height)));
        let alert = Alert::new(mtm, title)
            .informative(info)
            .button(t("Sichern"), Role::Plain)
            .button(t("Abbrechen"), Role::Plain)
            .accessory(&accessory);
        match fields.first() {
            Some(first) => alert.focus(first),
            None => alert,
        }
    }

    fn field(&self, value: &str, placeholder: &str) -> Retained<NSTextField> {
        let field = NSTextField::textFieldWithString(&ns(value), self.mtm());
        field.setPlaceholderString(Some(&ns(placeholder)));
        field
    }

    fn ask_endpoint(&self) {
        let Some(window) = self.window() else { return };
        let mtm = self.mtm();
        let kind = NSPopUpButton::new(mtm);
        for k in [ProviderKind::OpenAiCompatible, ProviderKind::AnthropicCompatible] {
            kind.addItemWithTitle(&ns(&own_agent::kind_name(k)));
        }
        let name = self.field("", t("Name"));
        let address = self.field("", "http://localhost:8000/v1");
        let alert = self.alert(
            t("Endpunkt hinzufügen"),
            t("Ein Server, der die Schnittstelle von OpenAI oder Anthropic spricht. Den Schlüssel gibst du danach ein."),
            &[
                kind.clone().into_super().into_super().into_super(),
                name.clone().into_super().into_super(),
                address.clone().into_super().into_super(),
            ],
        );
        let this = self.retain();
        alert.sheet(&window, move |response| {
            if !first(response) {
                return;
            }
            let kind = if kind.indexOfSelectedItem() == 0 {
                ProviderKind::OpenAiCompatible
            } else {
                ProviderKind::AnthropicCompatible
            };
            this.add_endpoint(
                kind,
                &name.stringValue().to_string(),
                &address.stringValue().to_string(),
            );
        });
    }

    /// Keeps a new endpoint of `kind`, or says why not.
    fn add_endpoint(&self, kind: ProviderKind, name: &str, address: &str) {
        let address = address.trim();
        if !(address.starts_with("http://") || address.starts_with("https://")) {
            self.set_status(Some(&tf("„{}“ ist keine http- oder https-Adresse.", &[address])), true);
            return;
        }
        let name = if name.trim().is_empty() { address } else { name.trim() };
        own_agent::save_endpoint(Endpoint::new(&own_agent::new_endpoint_id(), kind, name, address));
        self.set_status(None, false);
        self.reload();
    }

    fn ask_key(&self) {
        let (Some(window), Some(endpoint)) = (self.window(), self.selected_endpoint()) else {
            return;
        };
        let mtm = self.mtm();
        let field = NSSecureTextField::initWithFrame(
            NSSecureTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 24.0)),
        );
        let alert = self.alert(
            &tf("Schlüssel für {}", &[&endpoint.name]),
            t("Der Schlüssel kommt in den Schlüsselbund dieses Macs, nie in eine Einstellungsdatei. Leer lassen entfernt ihn."),
            &[field.clone().into_super().into_super().into_super()],
        );
        let this = self.retain();
        alert.sheet(&window, move |response| {
            if first(response) {
                this.store_key(&endpoint, &field.stringValue().to_string());
            }
        });
    }

    fn store_key(&self, endpoint: &Endpoint, key: &str) {
        match own_agent::set_key(endpoint, key) {
            Ok(()) => self.set_status(None, false),
            Err(error) => self.set_status(
                Some(&tf("Der Schlüssel ließ sich nicht sichern: {}", &[&error.explain()])),
                true,
            ),
        }
        self.reload();
    }

    fn ask_address(&self) {
        let (Some(window), Some(endpoint)) = (self.window(), self.selected_endpoint()) else {
            return;
        };
        let field = self.field(&endpoint.base_url, endpoint.kind.default_base().unwrap_or("https://"));
        let alert = self.alert(
            &tf("Adresse von {}", &[&endpoint.name]),
            t("Etwa für Ollama oder LM Studio auf einem anderen Mac. Leer lassen stellt die übliche Adresse wieder her."),
            &[field.clone().into_super().into_super()],
        );
        let this = self.retain();
        alert.sheet(&window, move |response| {
            if !first(response) {
                return;
            }
            let address = field.stringValue().to_string();
            if address.trim().is_empty() {
                own_agent::remove_endpoint(&endpoint.id);
                this.reload();
            } else if address.starts_with("http://") || address.starts_with("https://") {
                own_agent::save_endpoint(Endpoint::new(
                    &endpoint.id,
                    endpoint.kind,
                    &endpoint.name,
                    address.trim(),
                ));
                this.reload();
            } else {
                this.set_status(Some(&tf("„{}“ ist keine http- oder https-Adresse.", &[&address])), true);
            }
        });
    }

    fn ask_price(&self) {
        let Some(window) = self.window() else { return };
        let model = self.field("", t("Modell, etwa gpt-x"));
        let input = self.field("", t("Eingabe: US-$ je Million Tokens"));
        let output = self.field("", t("Ausgabe: US-$ je Million Tokens"));
        let alert = self.alert(
            t("Preis eintragen"),
            t("Für Modelle, deren Preis die App nicht kennt. Er gilt vor jedem anderen Preis."),
            &[
                model.clone().into_super().into_super(),
                input.clone().into_super().into_super(),
                output.clone().into_super().into_super(),
            ],
        );
        let this = self.retain();
        alert.sheet(&window, move |response| {
            if !first(response) {
                return;
            }
            let number = |field: &NSTextField| field.stringValue().to_string().trim().replace(',', ".").parse::<f64>();
            match (model.stringValue().to_string().trim(), number(&input), number(&output)) {
                (id, Ok(input), Ok(output)) if !id.is_empty() && input >= 0.0 && output >= 0.0 => {
                    let mut settings = Settings::load_user();
                    let mut prices = settings.get(&own_agent::PRICES);
                    prices.models.insert(id.to_string(), Price::new(input, output));
                    if let Err(error) = settings.set(&own_agent::PRICES, &prices) {
                        crate::diag::report("saving the agent's prices", error);
                    }
                    this.set_status(None, false);
                    this.reload();
                }
                _ => this.set_status(Some(t("Ein Preis braucht ein Modell und zwei Zahlen.")), true),
            }
        });
    }
}

/// The defaults of new threads.
impl AgentSettings {
    fn fill_defaults(&self, views: &Views) {
        let choice = own_agent::default_choice();
        views.endpoint.removeAllItems();
        for endpoint in self.ivars().endpoints.borrow().iter() {
            views.endpoint.addItemWithTitle(&ns(&endpoint.name));
        }
        let index = choice
            .as_ref()
            .and_then(|c| self.ivars().endpoints.borrow().iter().position(|e| e.id == c.endpoint))
            .unwrap_or(0);
        views.endpoint.selectItemAtIndex(isize::try_from(index).unwrap_or(0));
        views
            .model
            .setStringValue(&ns(choice.as_ref().map_or("", |c| c.model.as_str())));
        self.fill_models(views);
        let effort = Settings::load_user().get(&own_agent::EFFORT);
        let at = effort
            .as_deref()
            .and_then(|e| own_agent::EFFORTS.iter().position(|x| *x == e))
            .map_or(0, |n| n + 1);
        views.effort.selectItemAtIndex(isize::try_from(at).unwrap_or(0));
    }

    fn chosen_endpoint(&self) -> Option<Endpoint> {
        let views = self.ivars().views.get()?;
        let index = usize::try_from(views.endpoint.indexOfSelectedItem()).ok()?;
        self.ivars().endpoints.borrow().get(index).cloned()
    }

    fn fill_models(&self, views: &Views) {
        views.model.removeAllItems();
        let Some(endpoint) = self.chosen_endpoint() else { return };
        for model in own_agent::models(&endpoint.id) {
            // SAFETY: The combo box takes any object as an item; a string is what it shows and completes.
            unsafe { views.model.addItemWithObjectValue(&ns(&model.id)) };
        }
    }

    /// The endpoint for new threads changed: its models are loaded and its usual model proposed.
    fn endpoint_chosen(&self) {
        let (Some(views), Some(endpoint)) = (self.ivars().views.get(), self.chosen_endpoint()) else {
            return;
        };
        views
            .model
            .setStringValue(&ns(endpoint.kind.default_model().unwrap_or("")));
        self.fill_models(views);
        self.store_choice();
        if !own_agent::usable(&endpoint) {
            self.set_status(Some(&tf("{} braucht zuerst einen Schlüssel.", &[&endpoint.name])), true);
            return;
        }
        self.set_status(Some(&tf("Lade die Modelle von {} …", &[&endpoint.name])), false);
        let name = endpoint.name.clone();
        own_agent::load_models(endpoint, move |result| {
            let Some(this) = super::agent_tab_now() else { return };
            match result {
                Ok(models) => {
                    if let Some(views) = this.ivars().views.get() {
                        this.fill_models(views);
                        if views.model.stringValue().to_string().trim().is_empty()
                            && let Some(first) = models.first()
                        {
                            views.model.setStringValue(&ns(&first.id));
                            this.store_choice();
                        }
                    }
                    let offered = match models.len() {
                        1 => tf("{} bietet ein Modell.", &[&name]),
                        count => tf("{} bietet {} Modelle.", &[&name, &count.to_string()]),
                    };
                    this.set_status(Some(&offered), false);
                }
                Err(error) => this.set_status(Some(&tf("Die Modelle ließen sich nicht laden: {}", &[&error.0])), true),
            }
        });
    }

    fn store_choice(&self) {
        let (Some(views), Some(endpoint)) = (self.ivars().views.get(), self.chosen_endpoint()) else {
            return;
        };
        let model = views.model.stringValue().to_string().trim().to_string();
        let choice = (!model.is_empty()).then_some(ModelChoice {
            endpoint: endpoint.id,
            model,
        });
        let mut settings = Settings::load_user();
        if let Err(error) = settings.set(&own_agent::MODEL, &choice) {
            crate::diag::report("saving the agent's model", error);
        }
    }

    fn store_effort(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let effort = usize::try_from(views.effort.indexOfSelectedItem() - 1)
            .ok()
            .and_then(|n| own_agent::EFFORTS.get(n))
            .map(|e| (*e).to_string());
        let mut settings = Settings::load_user();
        if let Err(error) = settings.set(&own_agent::EFFORT, &effort) {
            crate::diag::report("saving the agent's effort", error);
        }
    }
}

/// The tab's view.
impl AgentSettings {
    pub(crate) fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let (endpoints, endpoints_scroll) = form::table(
            mtm,
            self,
            &[(t("Anbieter"), 140.0), (t("Adresse"), 260.0), (t("Schlüssel"), 0.0)],
            TABLE_HEIGHT,
        );
        let (prices, prices_scroll) = form::table(
            mtm,
            self,
            &[(t("Modell"), 300.0), (t("Eingabe"), 100.0), (t("Ausgabe"), 0.0)],
            SHORT_TABLE,
        );
        let (rules, rules_scroll) = form::table(mtm, self, &[(t("Projekt"), 300.0), (t("Regel"), 0.0)], SHORT_TABLE);
        let (folders, folders_scroll) = form::table(mtm, self, &[(t("Ordner"), 0.0)], SHORT_TABLE);
        let button = |add: bool, title: &str, action: Sel| list_bar::button(mtm, add, title, self, action);
        let add_endpoint = button(true, t("Endpunkt hinzufügen"), sel!(ideAddAgentEndpoint:));
        let remove_endpoint = button(false, t("Endpunkt entfernen"), sel!(ideRemoveAgentEndpoint:));
        let key = text_button(mtm, self, t("Schlüssel …"), sel!(ideAgentKey:));
        let address = text_button(mtm, self, t("Adresse …"), sel!(ideAgentAddress:));
        let add_price = button(true, t("Preis eintragen"), sel!(ideAddAgentPrice:));
        let remove_price = button(false, t("Preis entfernen"), sel!(ideRemoveAgentPrice:));
        let remove_rule = button(false, t("Regel entfernen"), sel!(ideRemoveAgentRule:));
        let add_folder = button(true, t("Ordner erlauben …"), sel!(ideAddAgentFolder:));
        let remove_folder = button(false, t("Ordner entfernen"), sel!(ideRemoveAgentFolder:));
        let (endpoint, model, effort) = self.default_controls();
        let status = form::problem(mtm);
        let mut views = section(
            mtm,
            t("Anbieter und Endpunkte"),
            t(
                "Schlüssel liegen im Schlüsselbund, nie in einer Einstellungsdatei. Ollama und LM Studio brauchen keinen.",
            ),
        );
        views.extend([
            endpoints_scroll,
            list_bar::bar(mtm, &[&add_endpoint, &remove_endpoint], &[&key, &address]),
            status.clone().into_super().into_super(),
            form::heading(mtm, t("Neue Threads")),
            form::grid(
                mtm,
                &[
                    Row::new(mtm, t("Anbieter:"), &endpoint),
                    Row::new(mtm, t("Modell:"), &model),
                    Row::new(mtm, t("Denktiefe:"), &effort),
                ],
            )
            .into_super(),
        ]);
        views.extend(section(
            mtm,
            t("Eigene Preise (US-$ je Million Tokens)"),
            t("Die Preise von Anthropic kennt die App, OpenRouter nennt seine selbst, lokale Modelle kosten nichts. Für andere zeigt der Thread Kosten erst mit einem Preis von hier."),
        ));
        views.extend([prices_scroll, list_bar::bar(mtm, &[&add_price, &remove_price], &[])]);
        views.extend(section(
            mtm,
            t("Gespeicherte Regeln"),
            t("Was du mit „Immer erlauben“ freigegeben hast, je Projekt. Zerstörerische Git-Befehle fragen trotzdem jedes Mal."),
        ));
        views.extend([rules_scroll, list_bar::bar(mtm, &[&remove_rule], &[])]);
        views.extend(section(
            mtm,
            t("Ordner für die Sandbox"),
            t("Mit eingeschalteter Sandbox erreichen die Datei- und Webwerkzeuge des eigenen Agenten nur das Projekt, den temporären Ordner und diese Ordner; Befehle schreiben nirgends sonst hin, und es gibt kein Netzwerk. Gilt für Threads, die danach starten."),
        ));
        views.extend([folders_scroll, list_bar::bar(mtm, &[&add_folder, &remove_folder], &[])]);
        views.extend(super::commit_message::views(mtm));
        let _ = self.ivars().views.set(Views {
            endpoints,
            prices,
            rules,
            folders,
            remove_endpoint,
            key,
            address,
            remove_price,
            remove_rule,
            remove_folder,
            endpoint,
            model,
            effort,
            status,
        });
        self.reload();
        stacked(mtm, &views)
    }

    /// The endpoint, model and effort of new threads.
    fn default_controls(&self) -> (Retained<NSPopUpButton>, Retained<NSComboBox>, Retained<NSPopUpButton>) {
        let mtm = self.mtm();
        let endpoint = NSPopUpButton::new(mtm);
        set_action(&endpoint, self, sel!(ideAgentDefaultEndpoint:));
        let model = NSComboBox::new(mtm);
        model.setCompletes(true);
        model.widthAnchor().constraintEqualToConstant(260.0).setActive(true);
        set_action(&model, self, sel!(ideAgentDefaultModel:));
        let effort = NSPopUpButton::new(mtm);
        effort.addItemWithTitle(&ns(t("Wie der Anbieter")));
        for level in own_agent::EFFORTS {
            effort.addItemWithTitle(&ns(level));
        }
        set_action(&effort, self, sel!(ideAgentEffort:));
        (endpoint, model, effort)
    }
}

/// A section's heading and the note under it.
fn section(mtm: MainThreadMarker, title: &str, text: &str) -> Vec<Retained<NSView>> {
    vec![form::heading(mtm, title), form::note(mtm, text)]
}

/// The views one under another, in a scroll view no taller than a small screen shows whole.
fn stacked(mtm: MainThreadMarker, views: &[Retained<NSView>]) -> Retained<NSView> {
    let stack = form::stack(mtm, views);
    let document = ide_ui_agent::agent_panel::flipped(mtm);
    document.addSubview(&stack);
    let margin = form::MARGIN;
    pin_inset(
        &stack,
        &document,
        NSEdgeInsets {
            top: margin,
            left: margin,
            bottom: margin,
            right: margin,
        },
    );
    let size = document.fittingSize();
    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(size.width, size.height.min(MAX_HEIGHT)),
        ),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    document.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll.setDocumentView(Some(&document));
    let clip = scroll.contentView();
    activate([
        document.leadingAnchor().constraintEqualToAnchor(&clip.leadingAnchor()),
        document
            .trailingAnchor()
            .constraintEqualToAnchor(&clip.trailingAnchor()),
        document.topAnchor().constraintEqualToAnchor(&clip.topAnchor()),
    ]);
    scroll.into_super()
}

fn text_button(mtm: MainThreadMarker, owner: &AgentSettings, title: &str, action: Sel) -> Retained<NSButton> {
    // SAFETY: A nil target sends nothing until `set_action` gives the button its target and action.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, owner, action);
    button.setBezelStyle(NSBezelStyle::SmallSquare);
    button
}

/// What the self-test reads from and does with the Agent tab, through the code its controls run.
#[cfg(feature = "selftest")]
pub(crate) mod selftest {
    use objc2::DefinedClass;

    use super::{AgentSettings, List};

    /// The rows the table of `list` shows.
    pub(crate) fn rows(owner: &AgentSettings, list: List) -> Vec<Vec<String>> {
        owner.rows(list)
    }

    /// Adds an endpoint as the add sheet does.
    pub(crate) fn add_endpoint(owner: &AgentSettings, kind: ide_agent::ProviderKind, name: &str, address: &str) {
        owner.add_endpoint(kind, name, address);
    }

    /// Keeps `key` for the endpoint in row `row`, as the key sheet does.
    pub(crate) fn set_key(owner: &AgentSettings, row: usize, key: &str) {
        let endpoint = owner.ivars().endpoints.borrow().get(row).cloned();
        if let Some(endpoint) = endpoint {
            owner.store_key(&endpoint, key);
        }
    }

    /// Picks the endpoint in row `row` for new threads, as the popup does.
    pub(crate) fn choose_endpoint(owner: &AgentSettings, row: usize) {
        if let Some(views) = owner.ivars().views.get() {
            views.endpoint.selectItemAtIndex(isize::try_from(row).unwrap_or(0));
        }
        owner.endpoint_chosen();
    }

    /// Adds folders for the sandbox as the open panel does.
    pub(crate) fn add_folders(owner: &AgentSettings, folders: Vec<std::path::PathBuf>) {
        owner.add_folders(folders);
    }

    /// Selects row `row` of `list`, as a click on it does.
    pub(crate) fn select(owner: &AgentSettings, list: List, row: usize) {
        if let Some(views) = owner.ivars().views.get() {
            let table = [&views.endpoints, &views.prices, &views.rules, &views.folders][list as usize];
            table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
    }

    /// Removes the folder in row `row`, as selecting it and clicking "−" does.
    pub(crate) fn remove_folder(owner: &AgentSettings, row: usize) {
        select(owner, List::Folders, row);
        owner.remove_selected(List::Folders);
    }

    /// Scrolls the tab down to the folders of the sandbox, for a picture.
    pub(crate) fn reveal_folders(owner: &AgentSettings) {
        if let Some(views) = owner.ivars().views.get()
            && let Some(scroll) = views.folders.enclosingScrollView()
        {
            scroll.scrollRectToVisible(scroll.bounds());
        }
    }

    /// The model the combo box shows and the line under the providers.
    pub(crate) fn shown(owner: &AgentSettings) -> (String, String) {
        owner.ivars().views.get().map_or_else(Default::default, |views| {
            (
                views.model.stringValue().to_string(),
                views.status.stringValue().to_string(),
            )
        })
    }
}
