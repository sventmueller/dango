//! OpenCode through its own v2 HTTP API (E147, "SDK first, ACP as fallback"): the OpenCode choice starts
//! `opencode serve` when the installed OpenCode is a release this build speaks (`ide_opencode::TESTED` or newer), and
//! `opencode acp` otherwise. Its agents show as modes and its models and their variants as the composer's choices,
//! as an ACP agent's do.

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
pub use selftest_probe::set_test_opencode;

/// The id of the preset whose agent OpenCode is.
pub(crate) const PRESET: &str = "opencode";

/// How the OpenCode choice starts: its server when the program the preset finds speaks the v2 API; none to take
/// `opencode acp`. This runs `opencode --version`, so it runs off the main thread.
pub(crate) fn launch_for(
    agent: &crate::acp::AcpAgent,
    env: &[(String, String)],
    cwd: &Path,
) -> Option<ide_opencode::Launch> {
    if agent.id != PRESET {
        return None;
    }
    #[cfg(feature = "selftest")]
    {
        let _ = env;
        selftest_probe::test_launch(cwd)
    }
    #[cfg(not(feature = "selftest"))]
    {
        let program = crate::acp::launch_of(agent, env, cwd)?.program;
        let version = ide_opencode::version::probe(Supervisor::global(), &program, env).ok()?;
        let mut launch = ide_opencode::Launch::new(program, env.to_vec(), cwd);
        launch.version = Some(version);
        Some(launch)
    }
}

fn choice(value: String, name: String, description: Option<String>) -> OptionChoice {
    OptionChoice {
        value,
        name,
        description,
        group: None,
    }
}

/// OpenCode's settings as an ACP agent's: its agents as modes, the model and the model's variants as options.
pub(crate) fn capabilities(caps: &ide_opencode::Capabilities) -> ide_acp::Capabilities {
    let mut acp = ide_acp::Capabilities::default();
    acp.agent = Some((
        ide_opencode::AGENT.to_string(),
        Some("OpenCode".to_string()),
        caps.server_version.clone().unwrap_or_default(),
    ));
    acp.load_session = true;
    acp.images = caps.images();
    acp.modes = caps
        .modes()
        .into_iter()
        .map(|agent| Mode {
            id: agent.id.clone(),
            name: if agent.name.is_empty() {
                agent.id.clone()
            } else {
                agent.name.clone()
            },
            description: agent.description.clone(),
        })
        .collect();
    acp.mode = caps.mode().map(str::to_string);
    let models: Vec<OptionChoice> = caps
        .models
        .iter()
        .filter(|m| m.enabled)
        .map(|m| {
            let id = format!("{}/{}", m.provider, m.model_id);
            choice(id.clone(), if m.name.is_empty() { id } else { m.name.clone() }, None)
        })
        .collect();
    if !models.is_empty() {
        let current = caps
            .effective_model()
            .map(|m| format!("{}/{}", m.provider, m.model))
            .unwrap_or_default();
        acp.options.push(ConfigOption {
            id: "model".into(),
            name: t("Modell").into(),
            description: None,
            category: Category::Model,
            value: OptionValue::Select {
                current,
                choices: models,
            },
        });
    }
    if !caps.variants().is_empty() {
        // Without a variant OpenCode takes the model's own default, which the picker offers by name.
        let variants: Vec<OptionChoice> = std::iter::once(choice(String::new(), t("Standard").into(), None))
            .chain(caps.variants().iter().map(|v| choice(v.clone(), v.clone(), None)))
            .collect();
        acp.options.push(ConfigOption {
            id: "effort".into(),
            name: t("Aufwand").into(),
            description: None,
            category: Category::ThoughtLevel,
            value: OptionValue::Select {
                current: caps.variant().unwrap_or_default().to_string(),
                choices: variants,
            },
        });
    }
    acp
}

/// `error` in the user's words.
pub(crate) fn explained(error: &ide_opencode::SessionError) -> String {
    match error {
        ide_opencode::SessionError::SignIn => {
            t("OpenCode hat noch kein Modell. Verbinde einen Anbieter mit „opencode auth login“ im Terminal.")
                .to_string()
        }
        ide_opencode::SessionError::Unsupported { tested, .. } => {
            tf("Diese Version von OpenCode ist älter als {}.", &[&tested.to_string()])
        }
        other => tf("OpenCode: {}", &[&other.to_string()]),
    }
}

impl AgentTab {
    /// Starts OpenCode's server with `launch` for `thread`, continuing the thread's OpenCode session where it has one.
    pub(crate) fn launch_opencode(&self, mut launch: ide_opencode::Launch, thread: Thread) {
        {
            let state = self.state.borrow();
            launch.resume = match state.fork() {
                // A rewind continues the thread the agent cut, or one before the first prompt starts anew.
                Some(fork) => fork.rewind.is_some().then(|| fork.session.clone()),
                None => thread
                    .session()
                    .map(str::to_string)
                    .or_else(|| state.resume().map(|r| r.session.clone())),
            };
        }
        let sink = self.sink(Incoming::OpenCode);
        match ide_opencode::Session::start(Supervisor::global(), launch, thread.clone(), sink) {
            Ok(session) => {
                self.state.borrow_mut().started(Source::OpenCode(session));
                self.absorb(thread, &[ide_thread::Change::Cleared]);
            }
            Err(error) => self.start_failed(Problem::Start(explained(&error))),
        }
    }

    /// What an event of the OpenCode session that is not a change of the thread means for the tab.
    pub(crate) fn opencode_problem(&self, event: &ide_opencode::Event) -> Option<Problem> {
        match event {
            ide_opencode::Event::Failed(error) => Some(Problem::Start(explained(error))),
            _ => None,
        }
    }
}
