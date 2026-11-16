//! AI tabs with agents over ACP (E4, E5, E19): which agent a tab runs, whether it is there or can be installed,
//! starting it through `ide_acp`, and the composer's menus filled from what the agent offers instead of Claude's fixed
//! lists. Installing it on the user's word is in `acp_install`.

#[cfg(feature = "selftest")]
use ide_core::LockIgnoringPoison;
use std::path::Path;
use std::time::Duration;

use ide_acp::launch::Origin;
use ide_acp::register::{self, PRESETS};
use ide_acp::{Effect, Installs, Launch, OptionValue, Plan, Register, Sandbox};

use ide_session::{LaunchError, PathKind};
use ide_thread::{EntryKind, PermissionOption, RequestState, Thread};
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSColor, NSMenu};
use splat_i18n::{t, tf};

use super::acp_install::refusal_text;
use super::acp_words;
use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE};
use super::source::{Problem, Source};
use super::tab::AgentTab;
use ide_ui_common::util::{main_soon, ns};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// An agent spoken to over ACP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAgent {
    /// The register's id, which threads name as their agent.
    pub(crate) id: String,
    /// The name shown.
    pub(crate) name: String,
    /// Its colour as `#RRGGBB`, where it has one.
    pub(crate) colour: Option<String>,
    /// How its sandbox is switched on.
    pub(crate) sandbox: Sandbox,
    /// What its sandbox limits.
    pub(crate) effect: Effect,
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
        brand(self.id()).or_else(|| match self {
            Agent::Acp(agent) => agent.colour.as_deref().and_then(rgb_of),
            Agent::Claude | Agent::Own => None,
        })
    }

    /// The colours of its mark's left, top, right and bottom point in the tab bar, for an agent that shows four.
    pub(crate) fn four_colours(&self) -> Option<[Retained<NSColor>; 4]> {
        four_colours_of(self.id())
    }

    pub(crate) fn is_acp(&self) -> bool {
        matches!(self, Agent::Acp(_))
    }

    pub fn is_own(&self) -> bool {
        matches!(self, Agent::Own)
    }
}

/// The colour the owner gave an agent the app ships with, by the agent's or its CLI's id (E66).
fn brand(id: &str) -> Option<splat_markdown::palette::Rgb> {
    use splat_markdown::palette::Rgb;
    match id {
        "claude" | "claude-acp" => Some(super::style::TERRACOTTA),
        "codex-acp" | "codex" => Some(Rgb(0x1F, 0x6F, 0xEB)),
        "opencode" => Some(Rgb(0x3A, 0x3A, 0x3C)),
        "gemini" | "antigravity-acp" | "agy" => Some(GOOGLE[0]),
        own if own == ide_agent::session::AGENT_ID => Some(Rgb(0x2D, 0xA4, 0x4E)),
        _ => None,
    }
}

/// Google's blue, yellow, red and green, which its agents' marks show as Gemini's star does: blue at the right, yellow
/// at the left, red at the top, green at the bottom.
const GOOGLE: [splat_markdown::palette::Rgb; 4] = [
    splat_markdown::palette::Rgb(0x42, 0x85, 0xF4),
    splat_markdown::palette::Rgb(0xFB, 0xBC, 0x04),
    splat_markdown::palette::Rgb(0xEA, 0x43, 0x35),
    splat_markdown::palette::Rgb(0x34, 0xA8, 0x53),
];

fn four_colours_of(id: &str) -> Option<[Retained<NSColor>; 4]> {
    let [blue, yellow, red, green] = GOOGLE.map(super::style::accent);
    matches!(id, "gemini" | "antigravity-acp" | "agy").then(|| [yellow, red, blue, green])
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

/// The register as last fetched on the user's request, and when.
pub fn cached_register() -> Option<(Register, std::time::SystemTime)> {
    #[cfg(feature = "selftest")]
    if let Some(register) = TEST_REGISTER.lock_ignoring_poison().clone() {
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

/// How `agent` starts: its own command on the PATH, else the version the app installed.
pub fn launch_of(agent: &AcpAgent, env: &[(String, String)], cwd: &Path) -> Option<Launch> {
    #[cfg(feature = "selftest")]
    if let Some(test) = TEST_LAUNCH.lock_ignoring_poison().clone() {
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
        if let Err(why) = ide_ui_common::util::spawn("acp-start", move || {
            let env = ide_ui_common::window::environment(Duration::from_secs(15));
            let codex = crate::codex::launch_for(&agent.id, &env, &cwd);
            let opencode = crate::opencode::launch_for(&agent, &env, &cwd);
            let sdk = codex.is_some() || opencode.is_some();
            let launch = (!sdk)
                .then(|| launch_of(&agent, &env, &cwd))
                .flatten()
                .map(|mut launch| {
                    // Sessions start only in a trusted folder, so the project's servers may go over (E22).
                    launch.mcp_servers = crate::dialogs::mcp_for_launch(&cwd, &env);
                    launch
                });
            let missing = (!sdk && launch.is_none()).then(|| availability(&agent, &env, &cwd));
            let git = crate::host::host().git();
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    match (codex, opencode) {
                        (Some(codex), _) => tab.launch_codex(codex, thread, git),
                        (None, Some(opencode)) => tab.launch_opencode(opencode, thread, git),
                        (None, None) => tab.launch_acp(launch, missing, thread, git),
                    }
                }
            });
        }) {
            self.start_failed(Problem::Start(why.0));
        }
    }

    /// The agent's id of the session Codex or OpenCode continue for `thread`: after a rewind the one the agent cut,
    /// or none before the first prompt, which starts anew; else the thread's or the one the tab was opened to resume.
    pub(super) fn sdk_resume(&self, thread: &Thread) -> Option<String> {
        let state = self.state.borrow();
        match state.fork() {
            Some(fork) => fork.rewind.is_some().then(|| fork.session.clone()),
            None => thread
                .session()
                .map(str::to_string)
                .or_else(|| state.resume().map(|r| r.session.clone())),
        }
    }

    /// Starts the agent as `launch` says for `thread` in the app's registry, or shows what it takes when there is no
    /// launch; the registry takes the checkpoint before each message with `git` (I6).
    fn launch_acp(
        &self,
        launch: Option<Launch>,
        missing: Option<Availability>,
        thread: Thread,
        git: Option<ide_git::Git>,
    ) {
        let Some(agent) = self.acp_agent() else { return };
        let Some(mut launch) = launch else {
            let problem = match missing {
                Some(Availability::Installable(plan)) => Problem::Missing(Some(Box::new(plan))),
                Some(Availability::Unavailable(why)) => Problem::Start(why),
                _ => Problem::Missing(None),
            };
            self.start_failed(problem);
            return;
        };
        let resume = {
            let state = self.state.borrow();
            if state.sandbox() {
                launch = launch.sandboxed(&agent.sandbox);
            }
            thread
                .session()
                .map(str::to_string)
                .or_else(|| state.resume().map(|r| r.session.clone()))
        };
        let Some(sessions) = self.sessions.clone() else {
            return self.start_failed(Problem::Start(start_problem(LaunchError::Start(String::new()))));
        };
        let (cwd, env) = (launch.cwd.clone(), launch.env.clone());
        let contract = ide_session::Contract::User(ide_session::UserChoices::default());
        let mut start = ide_session::Launch::new(ide_session::AgentPath::Acp(Box::new(launch)), cwd, env, contract);
        start.resume = resume;
        start.thread = Some(thread);
        start.git = git;
        match crate::driven::Driven::start(&sessions, start) {
            Ok((driven, first)) => {
                self.state.borrow_mut().started(Source::Session(driven));
                if let Some(first) = first {
                    self.absorb(first.thread, &[ide_thread::Change::Cleared]);
                }
            }
            Err(error) => self.start_failed(Problem::Start(start_problem(error))),
        }
    }

    /// The agent's program ended without the app stopping it: a session that had opened says the agent was ended,
    /// one that had not says it did not start, each with the last line the agent wrote; either offers to start again.
    pub(super) fn acp_ended(&self, stderr: &str) {
        let source = {
            let mut state = self.state.borrow_mut();
            let path = state.source().tab_session().map(crate::driven::Driven::kind);
            if !matches!(path, Some(PathKind::Acp | PathKind::Codex | PathKind::OpenCode)) {
                return;
            }
            state.end()
        };
        let opened = source.acp().is_some_and(|s| s.session_id().is_some())
            || source.codex().is_some_and(|s| s.session_id().is_some())
            || source.opencode().is_some_and(|s| s.session_id().is_some());
        source.end();
        self.set_problem(Some(ended_problem(opened, stderr)));
    }

    /// Signs in with method `index` of the agent: one the agent runs itself goes through the session; one in a
    /// terminal opens a terminal with the command on the pasteboard.
    pub(crate) fn acp_sign_in(&self, index: usize) {
        let caps = self.state.borrow().acp_caps().cloned().unwrap_or_default();
        let Some(method) = caps.auth_methods.get(index) else {
            return;
        };
        match &method.terminal {
            None => {
                let result = self.state.borrow().source().authenticate(&method.id);
                if let Err(error) = result {
                    ide_ui_common::diag::report("signing in to an agent", &error);
                }
            }
            Some(terminal) => {
                let program = match self.state.borrow().source() {
                    Source::Session(driven) if driven.kind() == PathKind::Acp => self.launch_program(),
                    Source::Idle | Source::Starting => self.launch_program(),
                    Source::Session(_) | Source::Own(_) | Source::Driven(_) => None,
                };
                let Some(program) = program else { return };
                let mut parts: Vec<String> = terminal.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
                parts.push(program);
                parts.extend(terminal.args.iter().cloned());
                super::actions::copy_text(&parts.join(" "));
                if let Some(controller) = self.controller() {
                    crate::host::host().open_terminal(controller.id(), &self.cwd);
                }
            }
        }
    }

    fn launch_program(&self) -> Option<String> {
        let agent = self.acp_agent()?;
        let env = &ide_ui_common::window::environment(std::time::Duration::ZERO)[..];
        launch_of(&agent, env, &self.cwd).map(|l| l.program.display().to_string())
    }

    /// The model and mode pills, the sandbox switch and its explanation, from what the agent offers.
    pub(super) fn refresh_acp_pills(&self, agent: &AcpAgent) {
        let state = self.state.borrow();
        let caps = state.acp_caps().cloned().unwrap_or_default();
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
            .setImage(ide_ui_common::util::symbol("slider.horizontal.3", &mode).as_deref());
        let available = agent.sandbox.is_available();
        let (symbol, tint, tip) = match (available, state.sandbox()) {
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
            .setImage(ide_ui_common::util::symbol(symbol, &tip).as_deref());
        self.composer.sandbox.setContentTintColor(Some(&tint));
        self.composer.sandbox.setToolTip(Some(&ns(&tip)));
    }

    /// The menu of the model pill: the agent's models, grouped as it groups them, and how much it thinks.
    pub(super) fn acp_model_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let caps = self.state.borrow().acp_caps().cloned().unwrap_or_default();
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
        self.state.borrow_mut().offered_mut().models = offered;
        menu
    }

    pub(super) fn acp_pick_model(&self, index: usize) {
        let Some(picked) = self.state.borrow().offered().models.get(index).cloned() else {
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
        let result = self.state.borrow().source().set_option(option, value);
        if let Err(error) = result {
            return ide_ui_common::diag::report("choosing a model", &error);
        }
        let caps = self.state.borrow().acp_caps().cloned().unwrap_or_default();
        let effort = caps.thought_level().is_some_and(|o| o.id == option);
        super::choices::remember(self.agent.id(), |choice| {
            let slot = if effort { &mut choice.effort } else { &mut choice.model };
            *slot = Some(value.to_string());
        });
    }

    /// Puts a session that just told its options on the model, thought level and mode the user picked last for the
    /// agent, those it offers; once per session.
    pub(super) fn acp_apply_choice(&self) {
        let Some(caps) = self.state.borrow_mut().apply_picks() else {
            return;
        };
        let choice = super::choices::of(self.agent.id());
        for (option, wanted) in [(caps.model(), choice.model), (caps.thought_level(), choice.effort)] {
            let (Some(option), Some(wanted)) = (option, wanted) else {
                continue;
            };
            let OptionValue::Select { current, choices } = &option.value else {
                continue;
            };
            if *current != wanted && choices.iter().any(|c| c.value == wanted) {
                let result = self.state.borrow().source().set_option(&option.id, &wanted);
                if let Err(error) = result {
                    ide_ui_common::diag::report("choosing the model picked last", &error);
                }
            }
        }
        if let Some(mode) = choice.mode
            && caps.mode.as_ref() != Some(&mode)
            && caps.modes.iter().any(|m| m.id == mode)
        {
            let result = self.state.borrow().source().set_acp_mode(&mode);
            if let Err(error) = result {
                ide_ui_common::diag::report("choosing the mode picked last", &error);
            }
        }
    }

    /// The menu of the mode pill: the agent's modes.
    pub(super) fn acp_mode_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let caps = self.state.borrow().acp_caps().cloned().unwrap_or_default();
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
        self.state.borrow_mut().offered_mut().modes = caps.modes.iter().map(|m| m.id.clone()).collect();
        menu
    }

    pub(super) fn acp_pick_mode(&self, index: usize) {
        let Some(mode) = self.state.borrow().offered().modes.get(index).cloned() else {
            return;
        };
        self.acp_set_mode(mode);
    }

    /// Switches the agent to the mode `mode` and remembers it for the agent's next sessions.
    fn acp_set_mode(&self, mode: String) {
        let result = self.state.borrow().source().set_acp_mode(&mode);
        if let Err(error) = result {
            return ide_ui_common::diag::report("choosing a mode", &error);
        }
        super::choices::remember(self.agent.id(), |choice| choice.mode = Some(mode));
    }

    /// The commands the agent offers, for the slash menu.
    pub(super) fn acp_commands(&self) -> Vec<(String, String)> {
        self.state
            .borrow()
            .acp_caps()
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
        let result = self.state.borrow().source().answer_option(&request.request_id, index);
        if let Err(error) = result {
            ide_ui_common::diag::report("answering a permission request", &error);
        }
    }
}

/// Why the agent's session did not start, as the ACP session's own error words it.
fn start_problem(error: LaunchError) -> String {
    use ide_core::explain::Explain;
    match error {
        LaunchError::NotFound(program) => ide_acp::SessionError::NotFound(program).explain(),
        LaunchError::Start(why) => ide_acp::SessionError::Start(why).explain(),
        other => other.to_string(),
    }
}

/// What an agent whose program ended on its own shows, from the end of its stderr: an exit once its session had
/// `opened`, else a start that failed.
fn ended_problem(opened: bool, stderr: &str) -> Problem {
    let last = stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string);
    if opened {
        return Problem::Exited(last);
    }
    Problem::Start(last.map_or_else(
        || t("Das Programm hat sich ohne Meldung beendet.").to_string(),
        |why| tf("Das Programm hat sich beim Start beendet: {}", &[&why]),
    ))
}

/// What switching the sandbox on limits for an agent with `effect`.
pub(crate) fn effect_text(effect: Effect) -> String {
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

#[cfg(test)]
mod colour_tests {
    use super::*;

    #[test]
    fn every_shipped_agent_has_the_colour_the_owner_gave_it() {
        assert_eq!(brand("claude"), Some(super::super::style::TERRACOTTA));
        assert_eq!(brand("codex-acp"), brand("codex"), "the agent and its CLI look alike");
        assert!(brand("opencode").is_some());
        assert!(brand(ide_agent::session::AGENT_ID).is_some());
        assert_eq!(brand("someone-else"), None);
    }

    #[test]
    fn only_googles_agents_show_four_colours() {
        assert!(four_colours_of("antigravity-acp").is_some());
        assert!(four_colours_of("agy").is_some());
        assert!(four_colours_of("codex-acp").is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_names_its_agent_by_the_registers_id() {
        assert_eq!(Agent::of_thread(ide_claude::AGENT), Agent::Claude);
        assert_eq!(Agent::of_thread(ide_agent::session::AGENT_ID), Agent::Own);
        let codex = Agent::of_thread("codex-acp");
        assert!(codex.is_acp() && !codex.is_own());
        assert_eq!((codex.id(), codex.name()), ("codex-acp", "Codex"));
        assert_eq!(agent_name("gemini"), "Gemini");
        assert_eq!(Agent::Own.name(), "Eigener Agent");
        assert!(
            AcpAgent::presets().iter().all(|a| a.id != "claude-acp"),
            "Claude runs through its own CLI, not as a preset of the menu"
        );
    }

    #[test]
    fn colours_of_the_register_are_read_as_six_hex_digits() {
        assert_eq!(rgb_of("#1F6FEB"), Some(splat_markdown::palette::Rgb(0x1F, 0x6F, 0xEB)));
        assert_eq!(rgb_of("1F6FEB"), None);
        assert_eq!(rgb_of("#FFF"), None);
        assert_eq!(rgb_of("#GGGGGG"), None);
    }

    #[test]
    fn an_agent_that_ends_on_its_own_says_so_with_its_last_words() {
        assert_eq!(
            ended_problem(true, "loading\nthe model crashed\n\n"),
            Problem::Exited(Some("the model crashed".into()))
        );
        assert_eq!(ended_problem(true, "  \n"), Problem::Exited(None));
        assert_eq!(
            ended_problem(false, "error: no API key\n"),
            Problem::Start("Das Programm hat sich beim Start beendet: error: no API key".into())
        );
        assert_eq!(
            ended_problem(false, ""),
            Problem::Start("Das Programm hat sich ohne Meldung beendet.".into())
        );
    }

    #[test]
    fn the_sandbox_says_what_it_limits() {
        let effects = [
            Effect::ShellCommands,
            Effect::WorkspaceWrite,
            Effect::ProjectWritesOnly,
            Effect::None,
        ]
        .map(effect_text);
        assert_eq!(effects[3], "Keine Sandbox");
        assert_eq!(
            effects.iter().collect::<std::collections::BTreeSet<_>>().len(),
            4,
            "each effect has words of its own"
        );
        assert_eq!(
            sandbox_unavailable("OpenCode"),
            "OpenCode bietet keine Sandbox, die sich hier einschalten lässt"
        );
    }
}
