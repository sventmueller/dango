//! AI tabs with the app's own agent (E6): starting its session, or continuing a stored one, with the endpoint and
//! model chosen; the composer's pills for model and effort, for the profile and approval mode (E27) and for the
//! sandbox (E19); and answering its permission requests.

use ide_agent::{Approval, Choice, Profile, Session};
use ide_thread::{Change, EntryKind, PermissionOption, RequestState};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSColor, NSControlStateValueOn, NSMenu, NSView};
use splat_i18n::{t, tf};

use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE};
use super::source::{Problem, Source};
use super::tab::{AgentTab, Incoming};
use crate::own_agent::{self, ModelChoice};
use crate::util::{item, main_soon, ns, separator};

/// How many models of one endpoint the "+" menu lists.
const MENU_MODELS: usize = 12;

/// Fills the "+" menu's submenu of the own agent: each endpoint that can be asked with its models as last loaded,
/// else the model it starts with, the default for new threads ticked; then the way to the settings.
pub fn fill_menu(mtm: MainThreadMarker, menu: &NSMenu) {
    menu.removeAllItems();
    menu.setAutoenablesItems(false);
    let default = own_agent::default_choice();
    let mut listed = false;
    for endpoint in own_agent::endpoints().into_iter().filter(own_agent::usable) {
        let chosen = default
            .as_ref()
            .filter(|c| c.endpoint == endpoint.id)
            .map(|c| c.model.clone());
        let mut models: Vec<String> = own_agent::models(&endpoint.id)
            .into_iter()
            .map(|m| m.id)
            .take(MENU_MODELS)
            .collect();
        if models.is_empty() {
            models.extend(endpoint.kind.default_model().map(str::to_string));
        }
        if let Some(model) = chosen.as_ref().filter(|m| !models.contains(m)) {
            models.insert(0, model.clone());
        }
        if models.is_empty() {
            continue;
        }
        let heading = item(mtm, &endpoint.name, None, "");
        heading.setEnabled(false);
        menu.addItem(&heading);
        for model in models {
            let entry = item(mtm, &model, Some(objc2::sel!(ideNewOwnThread:)), "");
            entry.setIndentationLevel(1);
            // SAFETY: A menu item can represent an object of any type.
            unsafe { entry.setRepresentedObject(Some(&ns(&format!("{}\u{1f}{model}", endpoint.id)))) };
            if chosen.as_deref() == Some(model.as_str()) {
                entry.setState(NSControlStateValueOn);
            }
            menu.addItem(&entry);
        }
        listed = true;
    }
    if !listed {
        let none = item(mtm, t("Noch kein Anbieter eingerichtet"), None, "");
        none.setEnabled(false);
        menu.addItem(&none);
    }
    menu.addItem(&separator(mtm));
    menu.addItem(&item(
        mtm,
        t("Anbieter und Modelle …"),
        Some(objc2::sel!(ideOpenAgentSettings:)),
        "",
    ));
}

/// The endpoint and model a menu entry of `fill_menu` stands for.
pub fn menu_choice(represented: &str) -> Option<ModelChoice> {
    let (endpoint, model) = represented.split_once('\u{1f}')?;
    Some(ModelChoice {
        endpoint: endpoint.to_string(),
        model: model.to_string(),
    })
}

/// The approval modes, with their names and symbols.
fn approvals() -> [(Approval, &'static str, &'static str); 3] {
    [
        (Approval::Manual, t("Manuell"), "hand.raised"),
        (Approval::Edits, t("Änderungen erlauben"), "pencil"),
        (Approval::All, t("Alles erlauben"), "bolt"),
    ]
}

/// What the sandbox limits for the own agent.
fn sandbox_effect() -> String {
    t("Dateien, Webabrufe und Befehle bleiben im Projekt und den erlaubten Ordnern, ohne Netzwerk").to_string()
}

impl AgentTab {
    /// Starts the session on a thread of its own, as it waits for the login shell's environment and reads a stored
    /// conversation: anew, or from the store when the thread is opened again. The model is the tab's choice, the
    /// stored thread's, else the default for new threads.
    pub(super) fn start_own(&self) {
        let chosen = self.state.borrow().own.clone();
        let (key, cwd, thread_id) = (self.key, self.cwd.clone(), self.thread_id.clone());
        let sandbox = self.state.borrow().sandbox;
        let sink = self.sink(Incoming::Own);
        let _ = std::thread::Builder::new()
            .name("own-agent-start".into())
            .spawn(move || {
                let store = own_agent::store_path(&thread_id);
                let stored = store.exists().then(|| ide_agent::store::load(&store).ok()).flatten();
                let choice = chosen
                    .or_else(|| {
                        stored.filter(|s| !s.model.is_empty()).map(|s| ModelChoice {
                            endpoint: s.endpoint,
                            model: s.model,
                        })
                    })
                    .or_else(own_agent::default_choice);
                let session = choice.clone().map(|choice| {
                    let config = own_agent::config(thread_id, &cwd, &choice, sandbox)?;
                    if store.exists() {
                        Session::resume(config, sink).map_err(|e| own_agent::load_error(&e))
                    } else {
                        Ok(Session::start(config, sink))
                    }
                });
                main_soon(move || {
                    if let Some(tab) = super::get(key) {
                        tab.launch_own(choice, session);
                    }
                });
            });
    }

    fn launch_own(&self, choice: Option<ModelChoice>, session: Option<Result<Session, String>>) {
        let session = match session {
            Some(Ok(session)) => session,
            Some(Err(why)) => {
                self.state.borrow_mut().source = Source::Idle;
                return self.set_problem(Some(Problem::Start(why)));
            }
            None => {
                self.state.borrow_mut().source = Source::Idle;
                return self.set_problem(Some(Problem::NoModel));
            }
        };
        let snapshot = session.snapshot();
        {
            let mut state = self.state.borrow_mut();
            state.own = choice;
            state.source = Source::Own(session);
            state.problem = None;
        }
        self.absorb(snapshot, &[Change::Cleared]);
    }

    /// The session reported something besides changes to the thread.
    pub(super) fn own_event(&self, event: &ide_agent::Event) {
        match event {
            ide_agent::Event::RuleAdded(rule) => own_agent::add_rule(&self.cwd, rule.clone()),
            ide_agent::Event::Failed(why) => crate::diag::report("the own agent", why),
            ide_agent::Event::Changed(_) => {}
        }
    }

    /// The pills, the placeholder and the send button, as the state is.
    pub(super) fn refresh_own_composer(&self) {
        self.refresh_own_pills();
        let state = self.state.borrow();
        let placeholder = if state.sending {
            t("Sicherungspunkt wird angelegt …").to_string()
        } else {
            tf("⌘Esc zum Fokussieren oder Verlassen von {}", &[self.agent.name()])
        };
        self.composer.set_placeholder(&placeholder);
        let running = state.status != ide_thread::Status::Idle || state.sending;
        let can_send = !self.composer.text().trim().is_empty() || !state.attachments.is_empty();
        self.composer.show_running(running, can_send);
    }

    /// The model, mode and sandbox pills, and the warning of a thread that allows everything without a sandbox.
    fn refresh_own_pills(&self) {
        let state = self.state.borrow();
        let model = state
            .thread
            .model()
            .map(str::to_string)
            .or_else(|| state.own.as_ref().map(|c| c.model.clone()))
            .unwrap_or_else(|| t("Kein Modell").to_string());
        let pill = match state.thread.effort() {
            Some(effort) => tf("{} · {}", &[&model, effort]),
            None => model,
        };
        self.composer.model.setTitle(&ns(&pill));
        self.composer.model.setEnabled(true);
        let policy = state.source.own().map(Session::policy);
        let (profile, approval) = policy
            .as_ref()
            .map_or((Profile::Write, Approval::Manual), |p| (p.profile, p.approval));
        let (title, symbol) = if profile == Profile::Ask {
            (t("Nur fragen").to_string(), "eye")
        } else {
            approvals()
                .iter()
                .find(|(a, _, _)| *a == approval)
                .map_or((t("Manuell").to_string(), "hand.raised"), |(_, name, symbol)| {
                    ((*name).to_string(), *symbol)
                })
        };
        self.composer.mode.setTitle(&ns(&title));
        self.composer.mode.setEnabled(true);
        self.composer
            .mode
            .setImage(crate::util::symbol(symbol, &title).as_deref());
        let (symbol, tint, tip) = if state.sandbox {
            (
                "checkmark.shield.fill",
                NSColor::systemGreenColor(),
                tf("Sandbox an: {}", &[&sandbox_effect()]),
            )
        } else {
            (
                "shield.slash",
                NSColor::secondaryLabelColor(),
                tf("Sandbox aus; eingeschaltet gilt: {}", &[&sandbox_effect()]),
            )
        };
        self.composer.sandbox.setEnabled(true);
        self.composer
            .sandbox
            .setImage(crate::util::symbol(symbol, &tip).as_deref());
        self.composer.sandbox.setContentTintColor(Some(&tint));
        self.composer.sandbox.setToolTip(Some(&ns(&tip)));
        let unguarded = approval == Approval::All && profile == Profile::Write && !state.sandbox;
        drop(state);
        self.set_warning_text(t(
            "Alles erlauben ohne Sandbox: Der eigene Agent ändert Dateien und führt Befehle ohne Rückfrage aus; nur zerstörerische Git-Befehle fragen noch.",
        ));
        self.set_warning(unguarded);
    }

    /// The model pill's menu: the endpoint's models as last loaded, then the effort levels.
    pub(super) fn own_model_menu(&self, view: &NSView) {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let choice = self.state.borrow().own.clone();
        let mut offered = Vec::new();
        for endpoint in own_agent::endpoints().into_iter().filter(own_agent::usable) {
            let models = own_agent::models(&endpoint.id);
            let current = choice
                .as_ref()
                .filter(|c| c.endpoint == endpoint.id)
                .map(|c| c.model.clone());
            let shown: Vec<String> = if models.is_empty() {
                current.clone().into_iter().collect()
            } else {
                models.iter().map(|m| m.id.clone()).collect()
            };
            if shown.is_empty() {
                continue;
            }
            let heading = self.menu_item(&menu, &endpoint.name, sel_pick_model(), -1);
            heading.setEnabled(false);
            for model in shown {
                let tag = isize::try_from(offered.len()).unwrap_or(-1);
                let entry = self.menu_item(&menu, &model, sel_pick_model(), tag);
                if current.as_deref() == Some(model.as_str()) {
                    entry.setState(objc2_app_kit::NSControlStateValueOn);
                }
                offered.push(format!("{}\u{1f}{model}", endpoint.id));
            }
            menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(self.mtm));
        }
        let effort = self.state.borrow().thread.effort().map(str::to_string);
        let heading = self.menu_item(&menu, t("Denktiefe"), sel_pick_effort(), -1);
        heading.setEnabled(false);
        let mut efforts = vec![String::new()];
        efforts.extend(own_agent::EFFORTS.iter().map(|e| (*e).to_string()));
        for (n, level) in efforts.iter().enumerate() {
            let title = if level.is_empty() {
                t("Wie der Anbieter")
            } else {
                level.as_str()
            };
            let entry = self.menu_item(&menu, title, sel_pick_effort(), isize::try_from(n).unwrap_or(-1));
            if effort.as_deref().unwrap_or("") == level {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        {
            let mut state = self.state.borrow_mut();
            state.offered.models = offered;
            state.offered.efforts = efforts;
        }
        super::actions::pop_up(&menu, view);
    }

    pub(super) fn own_pick_model(&self, index: usize) {
        let Some(picked) = self.state.borrow().offered.models.get(index).cloned() else {
            return;
        };
        let Some((endpoint, model)) = picked.split_once('\u{1f}') else {
            return;
        };
        let choice = ModelChoice {
            endpoint: endpoint.to_string(),
            model: model.to_string(),
        };
        let same_endpoint = self
            .state
            .borrow()
            .own
            .as_ref()
            .is_some_and(|c| c.endpoint == choice.endpoint);
        self.state.borrow_mut().own = Some(choice.clone());
        let session = self.state.borrow().source.own().cloned();
        match session {
            Some(session) if same_endpoint => {
                let listed = own_agent::models(endpoint).into_iter().find(|m| m.id == model);
                session.set_model(
                    model,
                    listed.as_ref().and_then(|m| m.price),
                    listed.and_then(|m| m.context),
                );
                self.refresh_composer();
            }
            _ => self.restart_own(),
        }
    }

    pub(super) fn own_pick_effort(&self, index: usize) {
        let Some(level) = self.state.borrow().offered.efforts.get(index).cloned() else {
            return;
        };
        if let Some(session) = self.state.borrow().source.own() {
            session.set_effort(Some(level.as_str()).filter(|l| !l.is_empty()));
        }
        self.refresh_composer();
    }

    /// Starts the session again, as another endpoint or the sandbox needs; the conversation goes on from its store.
    fn restart_own(&self) {
        if self.state.borrow().status != ide_thread::Status::Idle {
            return;
        }
        let source = std::mem::replace(&mut self.state.borrow_mut().source, Source::Idle);
        source.end();
        self.start();
    }

    /// The mode pill's menu: the profile, then how much runs without asking.
    pub(super) fn own_mode_menu(&self, view: &NSView) {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let policy = self.state.borrow().source.own().map(Session::policy);
        let (profile, approval) = policy.map_or((Profile::Write, Approval::Manual), |p| (p.profile, p.approval));
        let mut modes = Vec::new();
        for (n, (item, title, symbol)) in [
            (Profile::Ask, t("Nur fragen"), "eye"),
            (Profile::Write, t("Schreiben"), "square.and.pencil"),
        ]
        .into_iter()
        .enumerate()
        {
            let entry = self.menu_item(&menu, title, sel_pick_mode(), isize::try_from(n).unwrap_or(-1));
            entry.setImage(crate::util::symbol(symbol, title).as_deref());
            if item == profile {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
            modes.push(if item == Profile::Ask { "ask" } else { "write" }.to_string());
        }
        menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(self.mtm));
        for (item, title, symbol) in approvals() {
            let entry = self.menu_item(
                &menu,
                title,
                sel_pick_mode(),
                isize::try_from(modes.len()).unwrap_or(-1),
            );
            entry.setImage(crate::util::symbol(symbol, title).as_deref());
            entry.setEnabled(profile == Profile::Write);
            if item == approval {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
            modes.push(ide_agent::session::mode_id(item).to_string());
        }
        self.state.borrow_mut().offered.modes = modes;
        super::actions::pop_up(&menu, view);
    }

    pub(super) fn own_pick_mode(&self, index: usize) {
        let Some(mode) = self.state.borrow().offered.modes.get(index).cloned() else {
            return;
        };
        if let Some(session) = self.state.borrow().source.own() {
            match mode.as_str() {
                "ask" => session.set_profile(Profile::Ask),
                "write" => session.set_profile(Profile::Write),
                "edits" => session.set_approval(Approval::Edits),
                "all" => session.set_approval(Approval::All),
                _ => session.set_approval(Approval::Manual),
            }
        }
        self.refresh_composer();
    }

    /// The sandbox switch: it takes effect with the next tool call, also in a running turn.
    pub(super) fn own_toggle_sandbox(&self) {
        let on = {
            let mut state = self.state.borrow_mut();
            state.sandbox = !state.sandbox;
            state.sandbox
        };
        if let Some(session) = self.state.borrow().source.own() {
            session.set_sandbox(on);
        }
        self.refresh_composer();
    }

    /// Answers permission request `id`: allowing once, allowing always with the rule it offers, or refusing.
    pub(super) fn own_permission(&self, id: u64, what: u8) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PermissionRequest(request) = &entry.kind else {
            return;
        };
        if request.state != RequestState::Pending {
            return;
        }
        let wanted = |option: &PermissionOption| match what {
            ALLOW_ONCE => *option == PermissionOption::AllowOnce,
            ALLOW_ALWAYS => matches!(option, PermissionOption::AllowAlways { .. }),
            _ => *option == PermissionOption::Deny,
        };
        let choice = request
            .options
            .iter()
            .position(wanted)
            .map_or(Choice::Dismiss, Choice::Option);
        let result = self
            .state
            .borrow()
            .source
            .own()
            .map(|s| s.answer(&request.request_id, choice));
        if let Some(Err(error)) = result {
            crate::diag::report("answering a permission request", &error);
        }
    }
}

fn sel_pick_model() -> objc2::runtime::Sel {
    objc2::sel!(ideAgentPickModel:)
}

fn sel_pick_effort() -> objc2::runtime::Sel {
    objc2::sel!(ideAgentPickEffort:)
}

fn sel_pick_mode() -> objc2::runtime::Sel {
    objc2::sel!(ideAgentPickMode:)
}
