//! Codex through its own app-server protocol (E147, "SDK first, ACP as fallback"): the Codex choice starts
//! `codex app-server` when the installed Codex is a release this build speaks (`ide_codex::TESTED` or newer), and
//! the `codex-acp` adapter otherwise. The session fills the same thread model, and its settings show in the composer
//! as an ACP agent's do, under the names `ide_codex` gives them.

use std::path::Path;

use ide_acp::capabilities::{Category, ConfigOption, Mode, OptionChoice, OptionValue};
use ide_process::Supervisor;
use ide_thread::Thread;
use splat_i18n::{t, tf};

use crate::source::{Problem, Source};
use crate::tab::{AgentTab, Incoming};

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::set_test_codex;

/// The id of the preset whose agent Codex is.
pub(crate) const PRESET: &str = "codex-acp";

/// How the Codex choice starts in `cwd` with the login shell's `env`: the app-server when an installed Codex speaks
/// its protocol; none to take the ACP adapter. This runs `codex --version`, so it runs off the main thread.
pub(crate) fn launch_for(agent: &str, env: &[(String, String)], cwd: &Path) -> Option<ide_codex::Launch> {
    if agent != PRESET {
        return None;
    }
    #[cfg(feature = "selftest")]
    {
        let _ = env;
        selftest_probe::test_launch(cwd)
    }
    #[cfg(not(feature = "selftest"))]
    {
        let program = ide_process::tool::which("codex", ide_process::tool::var(env, "PATH"))?;
        ide_codex::version::probe(Supervisor::global(), &program, env).ok()?;
        Some(ide_codex::Launch::new(program, env.to_vec(), cwd))
    }
}

/// Codex's settings as an ACP agent's: its approval presets as modes, the model and the model's efforts as options.
pub(crate) fn capabilities(caps: &ide_codex::Capabilities) -> ide_acp::Capabilities {
    let modes = ide_codex::MODES
        .iter()
        .map(|mode| Mode {
            id: mode.id.to_string(),
            name: mode_name(mode.id).to_string(),
            description: None,
        })
        .collect();
    let mut options = Vec::new();
    let models: Vec<OptionChoice> = caps
        .models
        .iter()
        .filter(|m| !m.hidden)
        .map(|m| OptionChoice {
            value: m.model.clone(),
            name: if m.display_name.is_empty() {
                m.model.clone()
            } else {
                m.display_name.clone()
            },
            description: (!m.description.is_empty()).then(|| m.description.clone()),
            group: None,
        })
        .collect();
    if !models.is_empty() {
        options.push(ConfigOption {
            id: "model".into(),
            name: t("Modell").into(),
            description: None,
            category: Category::Model,
            value: OptionValue::Select {
                current: caps.model.clone().unwrap_or_default(),
                choices: models,
            },
        });
    }
    let efforts: Vec<OptionChoice> = caps
        .efforts()
        .iter()
        .map(|e| OptionChoice {
            value: e.reasoning_effort.clone(),
            name: e.reasoning_effort.clone(),
            description: (!e.description.is_empty()).then(|| e.description.clone()),
            group: None,
        })
        .collect();
    if !efforts.is_empty() {
        options.push(ConfigOption {
            id: "effort".into(),
            name: t("Aufwand").into(),
            description: None,
            category: Category::ThoughtLevel,
            value: OptionValue::Select {
                current: caps.effort.clone().unwrap_or_default(),
                choices: efforts,
            },
        });
    }
    let mut acp = ide_acp::Capabilities::default();
    acp.agent = Some((
        ide_codex::AGENT.to_string(),
        Some("Codex".to_string()),
        caps.version.map(|v| v.to_string()).unwrap_or_default(),
    ));
    acp.load_session = true;
    acp.images = caps.images();
    acp.modes = modes;
    acp.mode = caps.mode().map(str::to_string);
    acp.options = options;
    acp
}

fn mode_name(id: &str) -> &'static str {
    match id {
        "read-only" => t("Nur lesen"),
        "auto" => t("Im Projekt arbeiten"),
        _ => t("Voller Zugriff"),
    }
}

/// `error` in the user's words.
pub(crate) fn explained(error: &ide_codex::SessionError) -> String {
    match error {
        ide_codex::SessionError::SignIn => {
            t("Codex braucht eine Anmeldung. Melde dich im Terminal mit „codex login“ an.").to_string()
        }
        ide_codex::SessionError::Unsupported { tested, .. } => {
            tf("Diese Version von Codex ist älter als {}.", &[&tested.to_string()])
        }
        other => tf("Codex: {}", &[&other.to_string()]),
    }
}

/// The ACP choice as Codex takes it.
pub(crate) fn choice(choice: ide_acp::Choice) -> ide_codex::Choice {
    match choice {
        ide_acp::Choice::Option(index) => ide_codex::Choice::Option(index),
        _ => ide_codex::Choice::Dismiss,
    }
}

impl AgentTab {
    /// Starts Codex's app-server with `launch` for `thread`, continuing the thread's Codex thread where it has one.
    pub(crate) fn launch_codex(&self, mut launch: ide_codex::Launch, thread: Thread) {
        {
            let state = self.state.borrow();
            launch.resume = thread
                .session()
                .map(str::to_string)
                .or_else(|| state.resume().map(|r| r.session.clone()));
        }
        let sink = self.sink(Incoming::Codex);
        match ide_codex::Session::start(Supervisor::global(), launch, thread.clone(), sink) {
            Ok(session) => {
                self.state.borrow_mut().started(Source::Codex(session));
                self.absorb(thread, &[ide_thread::Change::Cleared]);
            }
            Err(error) => self.start_failed(Problem::Start(explained(&error))),
        }
    }

    /// What an event of the Codex session that is not a change of the thread means for the tab.
    pub(crate) fn codex_problem(&self, event: &ide_codex::Event) -> Option<Problem> {
        match event {
            ide_codex::Event::Failed(error) => Some(Problem::Start(explained(error))),
            _ => None,
        }
    }
}
