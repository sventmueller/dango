//! AI tabs with the app's own agent (E6): starting its session, or continuing a stored one, with the endpoint and
//! model chosen; the composer's pills for model and effort, for the profile and approval mode (E27) and for the
//! sandbox (E19); and answering its permission requests.

use ide_agent::{Approval, Profile, Session};
use ide_core::explain::Explained;
use ide_git::Git;
use ide_session::{AgentPath, Contract, Launch, LaunchError, UserChoices};
use ide_thread::{Change, EntryKind, PermissionOption, RequestState};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSControlStateValueOn, NSMenu};
use splat_i18n::{t, tf};

use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE};
use super::driven::Driven;
use super::source::Problem;
use super::tab::AgentTab;
use crate::own_agent::{self, ModelChoice};
use ide_ui_common::util::{item, main_soon, ns, separator};

/// How many models of one endpoint the "+" menu lists.
const MENU_MODELS: usize = 12;

/// What a session of the own agent starts with: its configuration and the conversation its store kept.
type Prepared = (ide_agent::Config, Option<ide_agent::store::Stored>);

/// The endpoints the "+" menu offers, each that can be asked with its models as last loaded, else the model it
/// starts with, and the model ticked as the default for new threads.
fn offers() -> Vec<(ide_agent::Endpoint, Vec<String>, Option<String>)> {
    let default = own_agent::default_choice();
    own_agent::endpoints()
        .into_iter()
        .filter(own_agent::usable)
        .filter_map(|endpoint| {
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
            (!models.is_empty()).then_some((endpoint, models, chosen))
        })
        .collect()
}

/// Whether the own agent is set up: an endpoint can be asked and has a model to pick.
pub(crate) fn set_up() -> bool {
    !offers().is_empty()
}

/// Fills the "+" menu's submenu of the own agent: each endpoint it offers with its models, the default for new
/// threads ticked; then the way to the settings.
pub fn fill_menu(mtm: MainThreadMarker, menu: &NSMenu) {
    menu.removeAllItems();
    menu.setAutoenablesItems(false);
    let offers = offers();
    for (endpoint, models, chosen) in &offers {
        let heading = item(mtm, &endpoint.name, None, "");
        heading.setEnabled(false);
        menu.addItem(&heading);
        for model in models {
            let entry = item(mtm, model, Some(objc2::sel!(ideNewOwnThread:)), "");
            entry.setIndentationLevel(1);
            // SAFETY: A menu item can represent an object of any type.
            unsafe { entry.setRepresentedObject(Some(&ns(&format!("{}\u{1f}{model}", endpoint.id)))) };
            if chosen.as_deref() == Some(model.as_str()) {
                entry.setState(NSControlStateValueOn);
            }
            menu.addItem(&entry);
        }
    }
    if offers.is_empty() {
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
        let chosen = self
            .state
            .borrow()
            .own_choice()
            .cloned()
            .or_else(|| picked_model(self.agent.id()));
        let (key, cwd, thread_id) = (self.key, self.cwd.clone(), self.thread_id.clone());
        let sandbox = self.state.borrow().sandbox();
        let diagnostics = crate::host::host().agent_diagnostics(&cwd);
        if let Err(why) = ide_ui_common::util::spawn("own-agent-start", move || {
            let store = own_agent::store_path(&thread_id);
            let stored = store.exists().then(|| ide_agent::store::load(&store));
            let choice = chosen
                .or_else(|| {
                    let stored = stored.as_ref()?.as_ref().ok()?;
                    (!stored.model.is_empty()).then(|| ModelChoice {
                        endpoint: stored.endpoint.clone(),
                        model: stored.model.clone(),
                    })
                })
                .or_else(own_agent::default_choice);
            let prepared = choice.clone().map(|choice| {
                let config = own_agent::config(thread_id, &cwd, &choice, sandbox, diagnostics)?;
                let stored = stored.transpose().map_err(|e| Explained(own_agent::load_error(&e)))?;
                Ok((config, stored))
            });
            let git = crate::host::host().git();
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.launch_own(choice, prepared, git);
                }
            });
        }) {
            self.start_failed(Problem::Start(why.0));
        }
    }

    /// Starts the session `prepared` describes in the app's registry: its configuration, and the conversation its
    /// store kept, which it continues; the registry takes the checkpoint before each message with `git` (I6).
    fn launch_own(&self, choice: Option<ModelChoice>, prepared: Option<Result<Prepared, Explained>>, git: Option<Git>) {
        let (config, stored) = match prepared {
            Some(Ok(prepared)) => prepared,
            Some(Err(Explained(why))) => {
                return self.start_failed(Problem::Start(why));
            }
            None => {
                return self.start_failed(Problem::NoModel);
            }
        };
        let Some(sessions) = self.sessions.clone() else {
            return self.start_failed(Problem::Start(String::new()));
        };
        let path = match stored {
            Some(stored) => AgentPath::OwnStored(Box::new(config), Box::new(stored)),
            None => AgentPath::Own(Box::new(config)),
        };
        let thread = ide_thread::Thread::new(self.thread_id.clone(), self.agent.id(), &self.cwd);
        let mut launch = Launch::new(path, &self.cwd, Vec::new(), Contract::User(UserChoices::default()));
        launch.thread = Some(thread);
        launch.git = git;
        let (driven, first) = match Driven::start(&sessions, launch) {
            Ok(started) => started,
            Err(LaunchError::Start(why)) => return self.start_failed(Problem::Start(why)),
            Err(other) => return self.start_failed(Problem::Start(other.to_string())),
        };
        let session = driven.own_agent();
        if let Some(session) = &session {
            apply_choice(session, &super::choices::of(self.agent.id()));
        }
        let snapshot = session
            .map(|s| std::sync::Arc::new(s.snapshot()))
            .or(first.map(|batch| batch.thread));
        self.state.borrow_mut().pick_own(choice);
        self.run_session(driven);
        if let Some(snapshot) = snapshot {
            self.absorb(snapshot, &[Change::Cleared]);
        }
    }

    /// The pills, the placeholder and the send button, as the state is.
    pub(super) fn refresh_own_composer(&self) {
        self.refresh_own_pills();
        let state = self.state.borrow();
        let placeholder = if state.is_sending() {
            t("Sicherungspunkt wird angelegt …").to_string()
        } else {
            t("⌘Esc zum Fokussieren oder Verlassen").to_string()
        };
        self.composer.set_placeholder(&placeholder);
        drop(state);
        self.refresh_send(&self.composer.text());
    }

    /// The model, mode and sandbox pills, and the warning of a thread that allows everything without a sandbox.
    fn refresh_own_pills(&self) {
        let state = self.state.borrow();
        let model = state
            .thread()
            .model()
            .map(str::to_string)
            .or_else(|| state.own_choice().map(|c| c.model.clone()))
            .unwrap_or_else(|| t("Kein Modell").to_string());
        let pill = match state.thread().effort() {
            Some(effort) => tf("{} · {}", &[&model, effort]),
            None => model,
        };
        self.composer.model.setTitle(&ns(&pill));
        self.composer.model.setEnabled(true);
        let policy = state.source().own_agent().map(|session| session.policy());
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
            .setImage(ide_ui_common::util::symbol(symbol, &title).as_deref());
        let (symbol, tint, tip) = if state.sandbox() {
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
            .setImage(ide_ui_common::util::symbol(symbol, &tip).as_deref());
        self.composer.sandbox.setContentTintColor(Some(&tint));
        self.composer.sandbox.setToolTip(Some(&ns(&tip)));
    }

    /// The model pill's menu: the endpoint's models as last loaded, then the effort levels.
    pub(super) fn own_model_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let choice = self.state.borrow().own_choice().cloned();
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
        let effort = self.state.borrow().thread().effort().map(str::to_string);
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
            state.offered_mut().models = offered;
            state.offered_mut().efforts = efforts;
        }
        menu
    }

    pub(super) fn own_pick_model(&self, index: usize) {
        let Some(picked) = self.state.borrow().offered().models.get(index).cloned() else {
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
            .own_choice()
            .is_some_and(|c| c.endpoint == choice.endpoint);
        self.state.borrow_mut().pick_own(Some(choice.clone()));
        super::choices::remember(self.agent.id(), |remembered| remembered.model = Some(picked.clone()));
        let session = self.state.borrow().source().own_agent();
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
        let Some(level) = self.state.borrow().offered().efforts.get(index).cloned() else {
            return;
        };
        if let Some(session) = self.state.borrow().source().own_agent() {
            session.set_effort(Some(level.as_str()).filter(|l| !l.is_empty()));
        }
        super::choices::remember(self.agent.id(), |choice| {
            choice.effort = Some(level.clone()).filter(|l| !l.is_empty());
        });
        self.refresh_composer();
    }

    /// Starts the session again, as another endpoint or the sandbox needs; the conversation goes on from its store.
    fn restart_own(&self) {
        if self.state.borrow().status() != ide_thread::Status::Idle {
            return;
        }
        let source = self.state.borrow_mut().end();
        source.end();
        self.start();
    }

    /// The mode pill's menu: the profile, then how much runs without asking.
    pub(super) fn own_mode_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let policy = self.state.borrow().source().own_agent().map(|session| session.policy());
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
            entry.setImage(ide_ui_common::util::symbol(symbol, title).as_deref());
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
            entry.setImage(ide_ui_common::util::symbol(symbol, title).as_deref());
            entry.setEnabled(profile == Profile::Write);
            if item == approval {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
            modes.push(ide_agent::session::mode_id(item).to_string());
        }
        self.state.borrow_mut().offered_mut().modes = modes;
        menu
    }

    pub(super) fn own_pick_mode(&self, index: usize) {
        let Some(mode) = self.state.borrow().offered().modes.get(index).cloned() else {
            return;
        };
        if let Some(session) = self.state.borrow().source().own_agent() {
            match mode.as_str() {
                "ask" => session.set_profile(Profile::Ask),
                "write" => session.set_profile(Profile::Write),
                "edits" => session.set_approval(Approval::Edits),
                "all" => session.set_approval(Approval::All),
                _ => session.set_approval(Approval::Manual),
            }
            let policy = session.policy();
            super::choices::remember(self.agent.id(), |choice| {
                choice.mode = Some(mode_of(policy.profile, policy.approval));
            });
        }
        self.refresh_composer();
    }

    /// The sandbox switch: it takes effect with the next tool call, also in a running turn.
    pub(super) fn own_toggle_sandbox(&self) {
        let on = self.state.borrow_mut().toggle_sandbox();
        if let Some(session) = self.state.borrow().source().own_agent() {
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
        let option = request.options.iter().position(wanted);
        let session = self.state.borrow().source().tab_session().cloned();
        let result = session.map(|driven| driven.answer_option(&request.request_id, option));
        if let Some(Err(error)) = result {
            ide_ui_common::diag::report("answering a permission request", &error);
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

/// The model the user picked last for the own agent, while its service is still set up.
fn picked_model(agent: &str) -> Option<ModelChoice> {
    let picked = super::choices::of(agent).model?;
    let (endpoint, model) = picked.split_once('\u{1f}')?;
    own_agent::endpoint(endpoint)?;
    Some(ModelChoice {
        endpoint: endpoint.to_string(),
        model: model.to_string(),
    })
}

/// A profile and an approval mode as one remembered mode, such as `write all`.
fn mode_of(profile: Profile, approval: Approval) -> String {
    let profile = if profile == Profile::Ask { "ask" } else { "write" };
    format!("{profile} {}", ide_agent::session::mode_id(approval))
}

/// Puts a new session on the reasoning effort and mode the user picked last for the own agent.
fn apply_choice(session: &Session, choice: &ide_core::settings::AgentChoice) {
    if let Some(effort) = &choice.effort {
        session.set_effort(Some(effort.as_str()));
    }
    let Some((profile, approval)) = choice.mode.as_deref().and_then(|mode| mode.split_once(' ')) else {
        return;
    };
    session.set_profile(if profile == "ask" { Profile::Ask } else { Profile::Write });
    session.set_approval(match approval {
        "edits" => Approval::Edits,
        "all" => Approval::All,
        _ => Approval::Manual,
    });
}
