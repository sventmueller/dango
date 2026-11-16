//! Codex through its own app-server protocol (E147, "SDK first, ACP as fallback"): the Codex choice starts
//! `codex app-server` when the installed Codex is a release this build speaks (`ide_codex::TESTED` or newer), and
//! the `codex-acp` adapter otherwise. The session fills the same thread model, and its settings show in the composer
//! as an ACP agent's do, under the names `ide_codex` gives them.

use std::path::Path;

use ide_acp::capabilities::{Category, ConfigOption, Mode, OptionChoice, OptionValue};
#[cfg(not(feature = "selftest"))]
use ide_process::Supervisor;
use ide_session::{AgentPath, Contract, Launch, LaunchError, UserChoices};
use ide_thread::Thread;
use splat_i18n::{t, tf};

use crate::driven::Driven;
use crate::source::{Problem, Source};
use crate::tab::AgentTab;

mod remote;
#[cfg(feature = "selftest")]
mod selftest_probe;
pub(crate) use remote::Turning;
#[cfg(feature = "selftest")]
pub use selftest_probe::set_test_codex;

/// The id of the preset whose agent Codex is.
pub(crate) const PRESET: &str = "codex-acp";

/// How the Codex choice starts in `cwd` with the login shell's `env`: the app-server when an installed Codex speaks
/// its protocol, the shared one when Remote Control is on (E148); none to take the ACP adapter. This runs Codex, so
/// it runs off the main thread.
pub(crate) fn launch_for(agent: &str, env: &[(String, String)], cwd: &Path) -> Option<ide_codex::Launch> {
    if agent != PRESET {
        return None;
    }
    #[cfg(feature = "selftest")]
    let mut launch = {
        let _ = env;
        selftest_probe::test_launch(cwd)?
    };
    #[cfg(not(feature = "selftest"))]
    let mut launch = {
        let program = ide_process::tool::which("codex", ide_process::tool::var(env, "PATH"))?;
        let version = ide_codex::version::probe(Supervisor::global(), &program, env).ok()?;
        let mut launch = ide_codex::Launch::new(program, env.to_vec(), cwd);
        launch.version = Some(version);
        launch
    };
    remote::share(&mut launch);
    Some(launch)
}

/// The file that keeps the Codex threads of each folder as `thread/list` last named them, for the Agents panel.
fn history_file() -> std::path::PathBuf {
    ide_core::paths::data_dir().join("codex-threads.json")
}

/// Asks the running Codex for the stored threads of `cwd`, also those begun outside the app, and keeps them for the
/// Agents panel, which lists them as sessions to continue.
pub(crate) fn remember_threads(session: &ide_codex::Session, cwd: &Path) {
    let query = ide_codex::ThreadQuery {
        cwd: Some(cwd.to_path_buf()),
        limit: Some(50),
        ..ide_codex::ThreadQuery::default()
    };
    let folder = cwd.to_string_lossy().into_owned();
    session.threads(&query).then(move |page| {
        let Ok(page) = page else { return };
        let threads: Vec<serde_json::Value> = page
            .threads
            .iter()
            .map(|t| {
                let title = t
                    .name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| t.preview.clone());
                serde_json::json!({"id": t.id, "title": title, "updated": t.updated_at})
            })
            .collect();
        let file = history_file();
        let mut all: serde_json::Map<String, serde_json::Value> = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        all.insert(folder, serde_json::Value::from(threads));
        let bytes = serde_json::to_vec(&all).unwrap_or_default();
        if let Err(error) = ide_core::atomic_write::write(&file, &bytes) {
            ide_ui_common::diag::report("keeping the Codex threads", error);
        }
    });
}

/// The Codex threads of `cwd` the running Codex last named, newest first, as sessions to continue.
pub(crate) fn past_threads(cwd: &Path) -> Vec<(crate::Resume, std::time::SystemTime)> {
    let all: serde_json::Value = std::fs::read(history_file())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    all[cwd.to_string_lossy().as_ref()]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let updated = u64::try_from(t["updated"].as_i64()?).ok()?;
            Some((
                crate::Resume {
                    session: t["id"].as_str()?.to_string(),
                    thread: None,
                    title: t["title"].as_str().map(str::to_string).filter(|s| !s.is_empty()),
                    transcript: None,
                    agent: Some(PRESET.to_string()),
                },
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(updated),
            ))
        })
        .collect()
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

/// Why the Codex session did not start, as `explained` words Codex's own error.
fn start_problem(error: LaunchError) -> String {
    match error {
        LaunchError::NotFound(program) => explained(&ide_codex::SessionError::NotFound(program)),
        LaunchError::Start(why) => tf("Codex: {}", &[&why]),
        other => tf("Codex: {}", &[&other.to_string()]),
    }
}

impl AgentTab {
    /// Starts Codex's app-server as `launch` says for `thread` in the app's registry, continuing the thread's Codex
    /// thread where it has one; the registry takes the checkpoint before each message with `git` (I6).
    pub(crate) fn launch_codex(&self, launch: ide_codex::Launch, thread: Thread, git: Option<ide_git::Git>) {
        let resume = {
            let state = self.state.borrow();
            match state.fork() {
                // A rewind continues the thread the agent cut, or one before the first prompt starts anew.
                Some(fork) => fork.rewind.is_some().then(|| fork.session.clone()),
                None => thread
                    .session()
                    .map(str::to_string)
                    .or_else(|| state.resume().map(|r| r.session.clone())),
            }
        };
        let Some(sessions) = self.sessions.clone() else {
            return self.start_failed(Problem::Start(start_problem(LaunchError::Start(String::new()))));
        };
        let path = AgentPath::Codex {
            program: launch.program,
            version: launch.version,
        };
        let contract = Contract::User(UserChoices {
            shared: launch.shared,
            ..UserChoices::default()
        });
        let mut start = Launch::new(path, launch.cwd, launch.env, contract);
        start.resume = resume;
        start.thread = Some(thread);
        start.git = git;
        match Driven::start(&sessions, start) {
            Ok((driven, first)) => {
                self.state.borrow_mut().started(Source::Session(driven));
                if let Some(first) = first {
                    self.absorb(first.thread, &[ide_thread::Change::Cleared]);
                }
            }
            Err(error) => self.start_failed(Problem::Start(start_problem(error))),
        }
    }
}
