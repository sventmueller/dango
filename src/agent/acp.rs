//! AI tabs with agents over ACP (E4, E5, E19): which agent a tab runs, whether it is there or can be installed,
//! starting it through `ide_acp`, installing it on the user's word, and the composer's menus filled from what the
//! agent offers instead of Claude's fixed lists.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use ide_acp::install::{InstallError, Progress, Refusal, Source as InstallSource};
use ide_acp::launch::Origin;
use ide_acp::net::Http;
use ide_acp::register::{self, PRESETS};
use ide_acp::{Choice, Effect, Installer, Installs, Launch, OptionValue, Plan, Register, Sandbox};
use ide_process::Supervisor;
use ide_thread::{EntryKind, PermissionOption, RequestState, Thread};
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSColor, NSMenu, NSView};
use splat_i18n::{t, tf};

use super::acp_words;
use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE};
use super::source::{Problem, Source};
use super::tab::{AgentTab, Incoming};
use crate::util::{main_soon, ns};

/// An agent spoken to over ACP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAgent {
    /// The register's id, which threads name as their agent.
    pub id: String,
    /// The name shown.
    pub name: String,
    /// Its colour as `#RRGGBB`, where it has one.
    pub colour: Option<String>,
    /// How its sandbox is switched on.
    pub sandbox: Sandbox,
    /// What its sandbox limits.
    pub effect: Effect,
}

impl AcpAgent {
    /// The agent `preset` describes.
    pub fn of_preset(preset: &register::Preset) -> AcpAgent {
        AcpAgent {
            id: preset.id.to_string(),
            name: preset.name.to_string(),
            colour: preset.colour.map(str::to_string),
            sandbox: preset.sandbox.clone(),
            effect: preset.effect,
        }
    }

    /// Whether new threads are offered with it; an entry of the register always is.
    pub fn offered(&self) -> bool {
        register::preset(&self.id).is_none_or(|preset| preset.offered)
    }

    /// The agent with the register id `id`: one this build knows, else an entry of the register last fetched.
    pub fn find(id: &str) -> Option<AcpAgent> {
        if let Some(preset) = register::preset(id) {
            return Some(AcpAgent::of_preset(preset));
        }
        let (register, _) = cached_register()?;
        let entry = register.entry(id)?;
        Some(AcpAgent {
            id: entry.id.clone(),
            name: entry.name.clone(),
            colour: None,
            sandbox: Sandbox::Unavailable,
            effect: Effect::None,
        })
    }

    /// The agents of the "+" menu, in its order.
    pub fn presets() -> Vec<AcpAgent> {
        PRESETS
            .iter()
            .filter(|p| p.id != "claude-acp")
            .map(AcpAgent::of_preset)
            .collect()
    }
}

/// Which agent a tab runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Agent {
    /// Claude through its own CLI (E21).
    Claude,
    /// An agent over ACP.
    Acp(AcpAgent),
    /// The app's own agent (E6).
    Own,
}

impl Agent {
    /// The agent a thread names: Claude, an agent this build or the register knows, else Claude.
    pub fn of_thread(agent: &str) -> Agent {
        if agent == ide_claude::AGENT {
            return Agent::Claude;
        }
        if agent == ide_agent::session::AGENT_ID {
            return Agent::Own;
        }
        AcpAgent::find(agent).map_or(Agent::Claude, Agent::Acp)
    }

    /// The id threads name.
    pub fn id(&self) -> &str {
        match self {
            Agent::Claude => ide_claude::AGENT,
            Agent::Acp(agent) => &agent.id,
            Agent::Own => ide_agent::session::AGENT_ID,
        }
    }

    /// The name shown.
    pub fn name(&self) -> &str {
        match self {
            Agent::Claude => "Claude",
            Agent::Acp(agent) => &agent.name,
            Agent::Own => t("Eigener Agent"),
        }
    }

    /// Its colour, which tells its tabs apart in the tab bar, with enough contrast in every appearance; the system's
    /// accent colour for an agent without one.
    pub fn colour(&self) -> Retained<NSColor> {
        match (self, self.rgb()) {
            (Agent::Claude, _) => super::style::terracotta(),
            (_, Some(rgb)) => super::style::accent(rgb),
            (_, None) => NSColor::controlAccentColor(),
        }
    }

    /// Its own colour, if it has one.
    fn rgb(&self) -> Option<splat_markdown::palette::Rgb> {
        match self {
            Agent::Claude => Some(super::style::TERRACOTTA),
            Agent::Own => None,
            Agent::Acp(agent) => agent.colour.as_deref().and_then(rgb_of),
        }
    }

    pub fn is_acp(&self) -> bool {
        matches!(self, Agent::Acp(_))
    }

    pub fn is_own(&self) -> bool {
        matches!(self, Agent::Own)
    }
}

/// The colour of the agent whose CLI is `name`, as the tabs of its CLI threads show it.
pub fn cli_colour(name: &str) -> Retained<NSColor> {
    if name == "claude" {
        return Agent::Claude.colour();
    }
    AcpAgent::presets()
        .into_iter()
        .find(|agent| {
            register::preset(&agent.id)
                .is_some_and(|p| p.cli == Some(name) || p.on_path.is_some_and(|(program, _)| program == name))
        })
        .map_or_else(NSColor::controlAccentColor, |agent| Agent::Acp(agent).colour())
}

/// The name shown for the agent a thread names, as the session list reads it.
pub fn agent_name(agent: &str) -> String {
    Agent::of_thread(agent).name().to_string()
}

fn rgb_of(hex: &str) -> Option<splat_markdown::palette::Rgb> {
    let hex = hex.strip_prefix('#')?;
    let value = u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() == 6)?;
    let part = |shift: u32| u8::try_from((value >> shift) & 0xff).unwrap_or(0);
    Some(splat_markdown::palette::Rgb(part(16), part(8), part(0)))
}

/// The register the self-test uses instead of the one last fetched; process-wide, since lookups run on any thread.
#[cfg(feature = "selftest")]
static TEST_REGISTER: std::sync::Mutex<Option<Register>> = std::sync::Mutex::new(None);

/// Uses `register` as the one last fetched, so the self-test needs no network.
#[cfg(feature = "selftest")]
pub fn set_test_register(register: Register) {
    *TEST_REGISTER.lock().unwrap_or_else(|e| e.into_inner()) = Some(register);
}

/// The register as last fetched on the user's request, and when.
pub fn cached_register() -> Option<(Register, std::time::SystemTime)> {
    #[cfg(feature = "selftest")]
    if let Some(register) = TEST_REGISTER.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some((register, std::time::SystemTime::now()));
    }
    Register::cached()
}

/// Whether an agent can run now, or what it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// It starts as `launch` says.
    Ready(Origin),
    /// It can be installed as `plan` says.
    Installable(Plan),
    /// It cannot be had, for this reason.
    Unavailable(String),
}

fn refusal_text(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Unversioned(package) => tf(
            "{} nennt keine feste Version und wird deshalb nicht installiert",
            &[package],
        ),
        Refusal::Nothing { unknown } if unknown.is_empty() => t("Für diesen Mac gibt es keine Ausgabe").to_string(),
        Refusal::Nothing { unknown } => tf(
            "Nur als {} erhältlich, was hier nicht unterstützt wird",
            &[&unknown.join(", ")],
        ),
    }
}

/// Whether `agent` runs with the login shell's environment `env` in `cwd`, or how it could.
pub fn availability(agent: &AcpAgent, env: &[(String, String)], cwd: &Path) -> Availability {
    if let Some(launch) = launch_of(agent, env, cwd) {
        return Availability::Ready(launch.origin);
    }
    if let Some(preset) = register::preset(&agent.id) {
        return Availability::Installable(Plan::of_preset(preset));
    }
    let Some((register, _)) = cached_register() else {
        return Availability::Unavailable(t("Nicht im geladenen Register").to_string());
    };
    match register.entry(&agent.id).map(Plan::of_entry) {
        Some(Ok(plan)) => Availability::Installable(plan),
        Some(Err(refusal)) => Availability::Unavailable(refusal_text(&refusal)),
        None => Availability::Unavailable(t("Nicht im geladenen Register").to_string()),
    }
}

/// The launch every ACP tab gets in the self-test; process-wide, since agents are looked up on other threads.
#[cfg(feature = "selftest")]
static TEST_LAUNCH: std::sync::Mutex<Option<Option<Launch>>> = std::sync::Mutex::new(None);

/// Starts every ACP tab with `launch`, as the self-test runs its test agent; with `None` no agent is found at all,
/// so the self-test never starts an agent installed on the Mac.
#[cfg(feature = "selftest")]
pub fn set_test_launch(launch: Option<Launch>) {
    *TEST_LAUNCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(launch);
}

/// How `agent` starts: its own command on the PATH, else the version the app installed.
pub fn launch_of(agent: &AcpAgent, env: &[(String, String)], cwd: &Path) -> Option<Launch> {
    #[cfg(feature = "selftest")]
    if let Some(test) = TEST_LAUNCH.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return test.map(|mut launch| {
            launch.agent.clone_from(&agent.id);
            launch.name.clone_from(&agent.name);
            launch.cwd = cwd.to_path_buf();
            launch
        });
    }
    let installs = Installs::new(Installs::default_root());
    match register::preset(&agent.id) {
        Some(preset) => Launch::of_preset(preset, &installs, env, cwd),
        None => Launch::installed(&agent.id, &agent.name, &installs, env, cwd),
    }
}

/// What installing `plan` would download, for the question before it.
fn plan_text(plan: &Plan) -> String {
    let source = match &plan.source {
        InstallSource::Archive { url, .. } => tf("Archiv von {}", &[url]),
        InstallSource::Npm { package, version, .. } => tf("npm-Paket {}", &[&format!("{package}@{version}")]),
        InstallSource::Uv { package, version, .. } => {
            tf("Python-Paket {} über uv", &[&format!("{package}=={version}")])
        }
    };
    let mut lines = vec![
        tf("Version {}", &[&plan.version]),
        source,
        tf(
            "Ziel: {}",
            &[&Installs::default_root().join(&plan.id).display().to_string()],
        ),
    ];
    if !plan.verified {
        lines.push(t("Diese Version wurde mit dieser App nicht erprobt.").to_string());
    }
    if !plan.checksum {
        lines.push(t("Das Register nennt keine Prüfsumme; der Download wird nicht geprüft.").to_string());
    }
    lines.join("\n")
}

fn install_error_text(error: &InstallError) -> String {
    match error {
        InstallError::Runtime(name, Some(major)) => tf("{} {} oder neuer wird gebraucht", &[name, &major.to_string()]),
        InstallError::Runtime(name, None) => tf("{} wird gebraucht", &[name]),
        InstallError::Checksum { .. } => {
            t("Der Download stimmt nicht mit seiner Prüfsumme überein und wurde verworfen").to_string()
        }
        other => other.to_string(),
    }
}

fn progress_text(name: &str, progress: Progress) -> String {
    match progress {
        Progress::Checking => tf("{} wird vorbereitet …", &[name]),
        Progress::Downloading(bytes) => tf("{} wird geladen: {}", &[name, &super::format::bytes(bytes)]),
        Progress::Unpacking => tf("{} wird entpackt …", &[name]),
        Progress::Installing => tf("{} wird installiert …", &[name]),
    }
}

/// Asks before installing `plan` of `agent`; whether the user agreed.
pub fn confirm_install(mtm: objc2::MainThreadMarker, agent: &str, plan: &Plan) -> bool {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&tf("{} installieren?", &[agent])));
    alert.setInformativeText(&ns(&plan_text(plan)));
    alert.addButtonWithTitle(&ns(t("Installieren")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    alert.runModal() == NSAlertFirstButtonReturn
}

impl AgentTab {
    pub(super) fn acp_agent(&self) -> Option<AcpAgent> {
        match &self.agent {
            Agent::Acp(agent) => Some(agent.clone()),
            Agent::Claude | Agent::Own => None,
        }
    }

    /// Finds the agent's program on a thread of its own, then starts it.
    pub(super) fn start_acp(&self, agent: AcpAgent, thread: Thread) {
        let key = self.key;
        let cwd = self.cwd.clone();
        let _ = std::thread::Builder::new().name("acp-start".into()).spawn(move || {
            let env = crate::environment::wait(Duration::from_secs(15))
                .map(<[(String, String)]>::to_vec)
                .unwrap_or_default();
            let launch = launch_of(&agent, &env, &cwd);
            let missing = launch.is_none().then(|| availability(&agent, &env, &cwd));
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.launch_acp(launch, missing, thread);
                }
            });
        });
    }

    fn launch_acp(&self, launch: Option<Launch>, missing: Option<Availability>, thread: Thread) {
        let Some(agent) = self.acp_agent() else { return };
        let Some(mut launch) = launch else {
            self.state.borrow_mut().source = Source::Idle;
            let problem = match missing {
                Some(Availability::Installable(plan)) => Problem::Missing(Some(Box::new(plan))),
                Some(Availability::Unavailable(why)) => Problem::Start(why),
                _ => Problem::Missing(None),
            };
            self.set_problem(Some(problem));
            return;
        };
        {
            let state = self.state.borrow();
            if state.sandbox {
                launch = launch.sandboxed(&agent.sandbox);
            }
            launch.resume = thread
                .session()
                .map(str::to_string)
                .or_else(|| state.resume.as_ref().map(|r| r.session.clone()));
        }
        let sink = self.sink(Incoming::Acp);
        match ide_acp::Session::start(Supervisor::global(), launch, thread.clone(), sink) {
            Ok(session) => {
                {
                    let mut state = self.state.borrow_mut();
                    state.source = Source::Acp(session);
                    state.problem = None;
                }
                self.absorb(thread, &[ide_thread::Change::Cleared]);
            }
            Err(error) => {
                self.state.borrow_mut().source = Source::Idle;
                self.set_problem(Some(Problem::Start(error.to_string())));
            }
        }
    }

    /// Installs the agent after the user agreed, showing the progress, then starts it.
    pub fn install_agent(&self) {
        let (Some(agent), Some(Problem::Missing(Some(plan)))) = (self.acp_agent(), self.state.borrow().problem.clone())
        else {
            return;
        };
        if !confirm_install(self.mtm, &agent.name, &plan) {
            return;
        }
        let key = self.key;
        let name = agent.name.clone();
        self.set_problem(Some(Problem::Installing(progress_text(&name, Progress::Checking))));
        let _ = std::thread::Builder::new().name("acp-install".into()).spawn(move || {
            let env = crate::environment::wait(Duration::from_secs(15))
                .map(<[(String, String)]>::to_vec)
                .unwrap_or_default();
            let installer = Installer::new(
                Supervisor::global(),
                env,
                Installs::new(Installs::default_root()),
                Arc::new(Http::new()),
            );
            let shown = name.clone();
            let progress = move |step: Progress| {
                let text = progress_text(&shown, step);
                main_soon(move || {
                    if let Some(tab) = super::get(key)
                        && matches!(tab.state.borrow().problem, Some(Problem::Installing(_)))
                    {
                        tab.set_problem(Some(Problem::Installing(text)));
                    }
                });
            };
            let result = installer.install(&plan, true, &progress);
            main_soon(move || {
                let Some(tab) = super::get(key) else { return };
                match result {
                    Ok(_) => tab.start(),
                    Err(error) => tab.set_problem(Some(Problem::Start(install_error_text(&error)))),
                }
            });
        });
    }

    /// Signs in with method `index` of the agent: one the agent runs itself goes through the session; one in a
    /// terminal opens a terminal with the command on the pasteboard.
    pub fn acp_sign_in(&self, index: usize) {
        let caps = self.state.borrow().acp.clone().unwrap_or_default();
        let Some(method) = caps.auth_methods.get(index) else {
            return;
        };
        match &method.terminal {
            None => {
                let result = self.state.borrow().source.authenticate(&method.id);
                if let Err(error) = result {
                    crate::diag::report("signing in to an agent", &error);
                }
            }
            Some(terminal) => {
                let program = match &self.state.borrow().source {
                    Source::Acp(_) | Source::Idle | Source::Starting => self.launch_program(),
                    #[cfg(feature = "selftest")]
                    Source::Replay(_) => None,
                    Source::Live(_) | Source::Own(_) => None,
                };
                let Some(program) = program else { return };
                let mut parts: Vec<String> = terminal.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                parts.push(program);
                parts.extend(terminal.args.iter().cloned());
                super::actions::copy_text(&parts.join(" "));
                if let Some(controller) = self.controller() {
                    crate::terminal::open_tab(&controller, Some(&self.cwd));
                }
            }
        }
    }

    fn launch_program(&self) -> Option<String> {
        let agent = self.acp_agent()?;
        let env = crate::environment::get().unwrap_or_default();
        launch_of(&agent, env, &self.cwd).map(|l| l.program.display().to_string())
    }

    /// The model and mode pills, the sandbox switch and its explanation, from what the agent offers.
    pub(super) fn refresh_acp_pills(&self, agent: &AcpAgent) {
        let state = self.state.borrow();
        let caps = state.acp.clone().unwrap_or_default();
        let model = caps
            .model()
            .map_or_else(|| t("Modell des Agenten").to_string(), |m| m.current_name());
        let pill = match caps.thought_level() {
            Some(level) => tf("{} · {}", &[&model, &acp_words::current(level)]),
            None => model,
        };
        self.composer.model.setTitle(&ns(&pill));
        self.composer
            .model
            .setEnabled(caps.model().is_some() || caps.thought_level().is_some());
        let mode = caps
            .mode
            .as_ref()
            .and_then(|id| caps.modes.iter().find(|m| m.id == *id))
            .map_or_else(
                || t("Standard").to_string(),
                |m| acp_words::mode(&agent.id, &m.id).map_or_else(|| m.name.clone(), |(name, _)| name.to_string()),
            );
        self.composer.mode.setTitle(&ns(&mode));
        self.composer.mode.setEnabled(!caps.modes.is_empty());
        self.composer
            .mode
            .setImage(crate::util::symbol("slider.horizontal.3", &mode).as_deref());
        let available = agent.sandbox.is_available();
        let (symbol, tint, tip) = match (available, state.sandbox) {
            (false, _) => (
                "shield.slash",
                NSColor::tertiaryLabelColor(),
                sandbox_unavailable(&agent.name),
            ),
            (true, true) => (
                "checkmark.shield.fill",
                NSColor::systemGreenColor(),
                tf("Sandbox an: {}", &[&effect_text(agent.effect)]),
            ),
            (true, false) => (
                "shield.slash",
                NSColor::secondaryLabelColor(),
                tf("Sandbox aus; eingeschaltet gilt: {}", &[&effect_text(agent.effect)]),
            ),
        };
        self.composer.sandbox.setEnabled(available);
        self.composer
            .sandbox
            .setImage(crate::util::symbol(symbol, &tip).as_deref());
        self.composer.sandbox.setContentTintColor(Some(&tint));
        self.composer.sandbox.setToolTip(Some(&ns(&tip)));
    }

    /// The menu of the model pill: the agent's models, grouped as it groups them, and how much it thinks.
    pub(super) fn acp_model_menu(&self, view: &NSView) {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let caps = self.state.borrow().acp.clone().unwrap_or_default();
        let mut offered = Vec::new();
        for option in [caps.model(), caps.thought_level()].into_iter().flatten() {
            if !offered.is_empty() {
                menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(self.mtm));
            }
            let heading = self.menu_item(&menu, &acp_words::heading(option), sel!(ideAgentPickModel:), -1);
            heading.setEnabled(false);
            let OptionValue::Select { current, choices } = &option.value else {
                continue;
            };
            let mut group = None;
            for choice in choices {
                if choice.group != group
                    && let Some(name) = &choice.group
                {
                    let label = self.menu_item(&menu, name, sel!(ideAgentPickModel:), -1);
                    label.setEnabled(false);
                }
                group.clone_from(&choice.group);
                let tag = isize::try_from(offered.len()).unwrap_or(-1);
                let name = acp_words::choice(option, &choice.value, &choice.name);
                let entry = self.menu_item(&menu, &name, sel!(ideAgentPickModel:), tag);
                if let Some(description) = &choice.description {
                    entry.setToolTip(Some(&ns(description)));
                }
                if choice.value == *current {
                    entry.setState(objc2_app_kit::NSControlStateValueOn);
                }
                offered.push(format!("{}\u{1f}{}", option.id, choice.value));
            }
        }
        self.state.borrow_mut().offered.models = offered;
        super::actions::pop_up(&menu, view);
    }

    pub(super) fn acp_pick_model(&self, index: usize) {
        let Some(picked) = self.state.borrow().offered.models.get(index).cloned() else {
            return;
        };
        let Some((option, value)) = picked.split_once('\u{1f}') else {
            return;
        };
        self.acp_set_option(option, value);
    }

    /// Sets the agent's option `option`, its model or how much it thinks, to `value`, and remembers it for the
    /// agent's next sessions.
    fn acp_set_option(&self, option: &str, value: &str) {
        let result = self.state.borrow().source.set_option(option, value);
        if let Err(error) = result {
            return crate::diag::report("choosing a model", &error);
        }
        let caps = self.state.borrow().acp.clone().unwrap_or_default();
        let effort = caps.thought_level().is_some_and(|o| o.id == option);
        super::choices::remember(self.agent.id(), |choice| {
            let slot = if effort { &mut choice.effort } else { &mut choice.model };
            *slot = Some(value.to_string());
        });
    }

    /// Puts a session that just told its options on the model, thought level and mode the user picked last for the
    /// agent, those it offers; once per session.
    pub(super) fn acp_apply_choice(&self) {
        let caps = {
            let state = self.state.borrow();
            match &state.acp {
                Some(caps) if !state.choice_applied && (!caps.modes.is_empty() || !caps.options.is_empty()) => {
                    caps.clone()
                }
                _ => return,
            }
        };
        self.state.borrow_mut().choice_applied = true;
        let choice = super::choices::of(self.agent.id());
        for (option, wanted) in [(caps.model(), choice.model), (caps.thought_level(), choice.effort)] {
            let (Some(option), Some(wanted)) = (option, wanted) else {
                continue;
            };
            let OptionValue::Select { current, choices } = &option.value else {
                continue;
            };
            if *current != wanted && choices.iter().any(|c| c.value == wanted) {
                let result = self.state.borrow().source.set_option(&option.id, &wanted);
                if let Err(error) = result {
                    crate::diag::report("choosing the model picked last", &error);
                }
            }
        }
        if let Some(mode) = choice.mode
            && caps.mode.as_ref() != Some(&mode)
            && caps.modes.iter().any(|m| m.id == mode)
        {
            let result = self.state.borrow().source.set_acp_mode(&mode);
            if let Err(error) = result {
                crate::diag::report("choosing the mode picked last", &error);
            }
        }
    }

    /// The menu of the mode pill: the agent's modes.
    pub(super) fn acp_mode_menu(&self, view: &NSView) {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let caps = self.state.borrow().acp.clone().unwrap_or_default();
        let agent = self.agent.id().to_string();
        for (n, mode) in caps.modes.iter().enumerate() {
            let own = acp_words::mode(&agent, &mode.id);
            let name = own.map_or_else(|| mode.name.clone(), |(name, _)| name.to_string());
            let entry = self.menu_item(&menu, &name, sel!(ideAgentPickMode:), isize::try_from(n).unwrap_or(-1));
            if let Some(description) = own
                .map(|(_, words)| words.to_string())
                .or_else(|| mode.description.clone())
            {
                entry.setToolTip(Some(&ns(&description)));
            }
            if caps.mode.as_ref() == Some(&mode.id) {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        self.state.borrow_mut().offered.modes = caps.modes.iter().map(|m| m.id.clone()).collect();
        super::actions::pop_up(&menu, view);
    }

    pub(super) fn acp_pick_mode(&self, index: usize) {
        let Some(mode) = self.state.borrow().offered.modes.get(index).cloned() else {
            return;
        };
        self.acp_set_mode(mode);
    }

    /// Switches the agent to the mode `mode` and remembers it for the agent's next sessions.
    fn acp_set_mode(&self, mode: String) {
        let result = self.state.borrow().source.set_acp_mode(&mode);
        if let Err(error) = result {
            return crate::diag::report("choosing a mode", &error);
        }
        super::choices::remember(self.agent.id(), |choice| choice.mode = Some(mode));
    }

    /// The commands the agent offers, for the slash menu.
    pub(super) fn acp_commands(&self) -> Vec<(String, String)> {
        self.state
            .borrow()
            .acp
            .as_ref()
            .map(|caps| {
                caps.commands
                    .iter()
                    .map(|c| (c.name.clone(), c.description.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Answers permission request `id` with the option the user's choice stands for: allowing once, allowing
    /// always (the rule `rule`, when given), refusing.
    pub(super) fn acp_permission(&self, id: u64, what: u8, rule: Option<usize>) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PermissionRequest(request) = &entry.kind else {
            return;
        };
        if request.state != RequestState::Pending {
            return;
        }
        let index = match (what, rule) {
            (_, Some(rule)) => Some(rule),
            (ALLOW_ONCE, None) => request.options.iter().position(|o| *o == PermissionOption::AllowOnce),
            (ALLOW_ALWAYS, None) => request
                .options
                .iter()
                .position(|o| matches!(o, PermissionOption::AllowAlways { .. })),
            _ => request.options.iter().position(|o| *o == PermissionOption::Deny),
        };
        let choice = index.map_or(Choice::Dismiss, Choice::Option);
        let result = self.state.borrow().source.answer_acp(&request.request_id, choice);
        if let Err(error) = result {
            crate::diag::report("answering a permission request", &error);
        }
    }
}

/// What switching the sandbox on limits for an agent with `effect`.
pub fn effect_text(effect: Effect) -> String {
    match effect {
        Effect::ShellCommands => t("Shell-Befehle laufen abgeschirmt; Datei- und Webwerkzeuge nicht").to_string(),
        Effect::WorkspaceWrite => t("Befehle und Änderungen schreiben nur im Projekt, ohne Netzwerk").to_string(),
        Effect::ProjectWritesOnly => {
            t("Schreiben nur im Projekt; Lesen überall und Netzwerk bleiben offen").to_string()
        }
        Effect::None => t("Keine Sandbox").to_string(),
    }
}

fn sandbox_unavailable(name: &str) -> String {
    tf("{} bietet keine Sandbox, die sich hier einschalten lässt", &[name])
}

#[cfg(feature = "selftest")]
impl AgentTab {
    /// What the ACP agent offers, as the tab last learned it.
    pub fn acp_capabilities(&self) -> Option<ide_acp::Capabilities> {
        self.state.borrow().acp.clone()
    }

    /// The titles of the model and mode pills and the sandbox switch's explanation.
    pub fn pills(&self) -> (String, String, String) {
        (
            self.composer.model.title().to_string(),
            self.composer.mode.title().to_string(),
            self.composer
                .sandbox
                .toolTip()
                .map(|t| t.to_string())
                .unwrap_or_default(),
        )
    }

    /// What keeps the session from running, by name.
    pub fn problem_name(&self) -> Option<&'static str> {
        self.state.borrow().problem.as_ref().map(|p| match p {
            Problem::Untrusted => "untrusted",
            Problem::NotFound => "not found",
            Problem::SignIn(_) | Problem::AcpSignIn => "sign in",
            Problem::Start(_) => "start",
            Problem::Missing(_) => "missing",
            Problem::Installing(_) => "installing",
            Problem::NoModel => "no model",
        })
    }

    /// Sets the agent's option `option` to `value`, as picking it in the model menu does.
    pub fn choose_option(&self, option: &str, value: &str) {
        self.acp_set_option(option, value);
    }

    /// Switches the agent to mode `id`, as picking it in the mode menu does.
    pub fn choose_mode(&self, id: &str) {
        self.acp_set_mode(id.to_string());
    }

    /// The agent's name, as the tab shows it.
    pub fn agent_name(&self) -> String {
        self.agent.name().to_string()
    }

    /// How many checkpoints before prompts belong to turns that have ended.
    pub fn finished_checkpoints(&self) -> usize {
        self.state.borrow().checkpoints.values().filter(|c| c.finished).count()
    }

    /// What the rewind menu of user message `id` offers, by title.
    pub fn rewind_titles(&self, id: u64) -> Vec<String> {
        let Some(entry) = self.entry(id) else { return Vec::new() };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return Vec::new();
        };
        let Some(uuid) = message.message_id.clone() else {
            return Vec::new();
        };
        self.rewind_choices(&entry, &uuid)
            .into_iter()
            .map(|(title, _, _)| title.to_string())
            .collect()
    }
}
