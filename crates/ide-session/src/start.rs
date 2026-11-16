//! Starting a session on each path, with the contract in the path's own terms (E173 and its table per path): the
//! user's choices as the AI tab makes them, or an order's bounds; a path that cannot hold an order's bounds is
//! refused before anything starts (C48, C59, C72).

use std::path::Path;
use std::sync::Arc;

use ide_process::Supervisor;
use ide_thread::{Thread, ThreadId};

use crate::backend::Backend;
use crate::inbox::Inbox;
use crate::launch::{AgentPath, Contract, Launch, LaunchError, Order, PathKind, same_folder};

/// What an order needs that the path lacks: a list of domains it may reach, where it can only switch the network
/// on or off.
const ALLOW_LIST: &str = "network allow-list";

/// Starts the session `launch` describes, reporting to `inbox`.
pub(crate) fn start(launch: Launch, supervisor: &Supervisor, inbox: &Arc<Inbox>) -> Result<Backend, LaunchError> {
    let Launch {
        path,
        folder,
        env,
        model,
        effort,
        resume,
        session_id,
        thread,
        git: _,
        contract,
    } = launch;
    if let Contract::Order(order) = &contract
        && !same_folder(&folder, &order.worktree)
    {
        return Err(LaunchError::Folder {
            folder,
            worktree: order.worktree.clone(),
        });
    }
    let thread = thread.unwrap_or_else(|| Thread::new(ThreadId::new(), agent_id(&path), &folder));
    let common = Common {
        folder,
        env,
        model,
        effort,
        resume,
        session_id,
        thread,
        contract,
    };
    match path {
        AgentPath::Claude {
            program,
            remote_control,
            rewind,
        } => claude(program, (remote_control, rewind), common, supervisor, inbox),
        AgentPath::Codex { program, version } => codex(program, version, common, supervisor, inbox),
        AgentPath::OpenCode { program, version } => opencode(program, version, common, supervisor, inbox),
        AgentPath::Acp(launch) => acp(*launch, common, supervisor, inbox),
        AgentPath::Own(config) => own(*config, common, inbox),
        #[cfg(any(test, feature = "selftest"))]
        AgentPath::Scripted(script) => Ok(Backend::Script(crate::script::Session::start(
            script,
            common.folder,
            common.thread,
            inbox.script(),
        ))),
        #[cfg(any(test, feature = "selftest"))]
        AgentPath::Replay(replay) => Ok(Backend::Replay(std::cell::RefCell::new(*replay))),
    }
}

/// What every path takes from the launch.
struct Common {
    folder: std::path::PathBuf,
    env: Vec<(String, String)>,
    model: Option<String>,
    effort: Option<String>,
    resume: Option<String>,
    session_id: Option<String>,
    thread: Thread,
    contract: Contract,
}

/// The agent a new thread names, as the AI tab names it for the same path.
fn agent_id(path: &AgentPath) -> String {
    match path {
        AgentPath::Claude { .. } => ide_claude::AGENT.into(),
        AgentPath::Codex { .. } => ide_codex::AGENT.into(),
        AgentPath::OpenCode { .. } => ide_opencode::AGENT.into(),
        AgentPath::Acp(launch) => launch.agent.clone(),
        AgentPath::Own(_) => ide_agent::session::AGENT_ID.into(),
        #[cfg(any(test, feature = "selftest"))]
        AgentPath::Scripted(_) => "script".into(),
        #[cfg(any(test, feature = "selftest"))]
        AgentPath::Replay(_) => ide_claude::AGENT.into(),
    }
}

/// The Claude launch of an order (E173): its bounds with its frozen configuration (E172), and no Remote Control.
pub(crate) fn claude_order(launch: &mut ide_claude::Launch, order: &Order) {
    launch.order = Some(ide_claude::OrderBounds {
        builds: order.role.builds(),
        asks: !order.refuse_outside,
        worktree: order.worktree.clone(),
        build_dir: order.build_dir.clone(),
        read_roots: order.read_roots.clone(),
        allowed_domains: order.allowed_domains.clone(),
        frozen: order.frozen.clone(),
        frozen_settings: order.frozen.as_deref().and_then(frozen_claude_settings),
    });
    launch.allow_bypass = false;
    launch.remote_control = Some(false);
}

fn claude(
    program: std::path::PathBuf,
    (remote_control, rewind): (Option<bool>, Option<ide_thread::Rewind>),
    common: Common,
    supervisor: &Supervisor,
    inbox: &Arc<Inbox>,
) -> Result<Backend, LaunchError> {
    let mut launch = ide_claude::Launch::new(program, common.env, &common.folder);
    launch.model = common.model;
    launch.effort = common.effort;
    launch.resume = common.resume;
    launch.session_id = common.session_id;
    launch.remote_control = remote_control;
    if let (Some(rewind), Some(session)) = (rewind, launch.resume.clone()) {
        launch.rewind(&session, &rewind);
    }
    match &common.contract {
        Contract::User(choices) => {
            launch.permission_mode = Some(choices.mode.clone().unwrap_or_else(|| "default".into()));
            launch.sandbox = choices.sandbox;
            launch.allow_bypass = true;
            launch.file_checkpoints = true;
        }
        Contract::Order(order) => claude_order(&mut launch, order),
    }
    ide_claude::Session::start(supervisor, &launch, common.thread, inbox.claude())
        .map(Backend::Claude)
        .map_err(|error| match error {
            ide_claude::SessionError::NotFound(program) => LaunchError::NotFound(program),
            other => LaunchError::Start(other.to_string()),
        })
}

/// The Codex bounds of an order (E173, E193), or why Codex cannot hold them.
pub(crate) fn codex_order(order: &Order) -> Result<ide_codex::Confinement, LaunchError> {
    if !order.allowed_domains.is_empty() {
        return Err(LaunchError::Unsupported {
            path: PathKind::Codex,
            missing: ALLOW_LIST,
        });
    }
    let mut untrusted = vec![order.worktree.clone()];
    untrusted.extend(order.repository.iter().cloned());
    Ok(ide_codex::Confinement {
        writes: order.role.builds(),
        write_roots: order.build_dir.iter().cloned().collect(),
        instructions: order.frozen.as_deref().and_then(frozen_agents_md),
        untrusted,
    })
}

/// The frozen project settings of Claude: `.claude/settings.json` with `.claude/settings.local.json` over it; a file
/// that does not parse adds nothing.
pub(crate) fn frozen_claude_settings(frozen: &Path) -> Option<serde_json::Value> {
    let read = |name: &str| -> Option<serde_json::Value> {
        serde_json::from_str(&std::fs::read_to_string(frozen.join(".claude").join(name)).ok()?).ok()
    };
    let mut rules: [Vec<serde_json::Value>; 2] = [Vec::new(), Vec::new()];
    let mut found = false;
    for settings in ["settings.json", "settings.local.json"].into_iter().filter_map(read) {
        found = true;
        for (list, key) in rules.iter_mut().zip(["deny", "ask"]) {
            list.extend(settings["permissions"][key].as_array().cloned().unwrap_or_default());
        }
    }
    let [deny, ask] = rules;
    found.then(|| serde_json::json!({"permissions": {"deny": deny, "ask": ask}}))
}

/// The frozen `AGENTS.md` at the project's root, where Codex reads it for a session in the worktree's root; its
/// `AGENTS.override.md` first, as Codex prefers it.
pub(crate) fn frozen_agents_md(frozen: &Path) -> Option<String> {
    ["AGENTS.override.md", "AGENTS.md"]
        .iter()
        .filter_map(|name| std::fs::read_to_string(frozen.join(name)).ok())
        .find(|text| !text.trim().is_empty())
}

fn codex(
    program: std::path::PathBuf,
    version: Option<ide_codex::Version>,
    common: Common,
    supervisor: &Supervisor,
    inbox: &Arc<Inbox>,
) -> Result<Backend, LaunchError> {
    let mut launch = ide_codex::Launch::new(program, common.env, &common.folder);
    launch.version = version;
    launch.model = common.model;
    launch.effort = common.effort;
    launch.resume = common.resume;
    match &common.contract {
        Contract::User(choices) => {
            if let Some(mode) = ide_codex::MODES.iter().find(|m| Some(m.id) == choices.mode.as_deref()) {
                launch.approval = Some(mode.approval);
                launch.sandbox = Some(mode.sandbox);
            }
        }
        Contract::Order(order) => launch.confinement = Some(codex_order(order)?),
    }
    ide_codex::Session::start(supervisor, launch, common.thread, inbox.codex())
        .map(Backend::Codex)
        .map_err(|error| match error {
            ide_codex::SessionError::NotFound(program) => LaunchError::NotFound(program),
            other => LaunchError::Start(other.to_string()),
        })
}

fn opencode(
    program: std::path::PathBuf,
    version: Option<ide_opencode::Version>,
    common: Common,
    supervisor: &Supervisor,
    inbox: &Arc<Inbox>,
) -> Result<Backend, LaunchError> {
    let Contract::User(choices) = &common.contract else {
        return Err(LaunchError::NoRole(PathKind::OpenCode));
    };
    let mut launch = ide_opencode::Launch::new(program, common.env, &common.folder);
    launch.version = version;
    launch.model = common.model;
    launch.resume = common.resume;
    launch.agent = choices.mode.clone();
    ide_opencode::Session::start(supervisor, launch, common.thread, inbox.opencode())
        .map(Backend::OpenCode)
        .map_err(|error| match error {
            ide_opencode::SessionError::NotFound(program) => LaunchError::NotFound(program),
            other => LaunchError::Start(other.to_string()),
        })
}

fn acp(
    mut launch: ide_acp::Launch,
    common: Common,
    supervisor: &Supervisor,
    inbox: &Arc<Inbox>,
) -> Result<Backend, LaunchError> {
    if common.contract.order().is_some() {
        return Err(LaunchError::NoRole(PathKind::Acp));
    }
    launch.cwd = common.folder;
    if common.resume.is_some() {
        launch.resume = common.resume;
    }
    ide_acp::Session::start(supervisor, launch, common.thread, inbox.acp())
        .map(Backend::Acp)
        .map_err(|error| match error {
            ide_acp::SessionError::NotFound(program) => LaunchError::NotFound(program),
            other => LaunchError::Start(other.to_string()),
        })
}

/// The own agent's configuration for an order (E173): the order's policy, the build folder reachable besides the
/// worktree, which must be the workspace's project.
pub(crate) fn own_order(config: &mut ide_agent::Config, order: &Order) -> Result<(), LaunchError> {
    if !order.allowed_domains.is_empty() {
        return Err(LaunchError::Unsupported {
            path: PathKind::Own,
            missing: ALLOW_LIST,
        });
    }
    if !same_folder(config.workspace.root(), &order.worktree) {
        return Err(LaunchError::Folder {
            folder: config.workspace.root().to_path_buf(),
            worktree: order.worktree.clone(),
        });
    }
    config.policy = ide_agent::Policy::order(order.role.builds());
    let mut allowed = config.workspace.allowed().to_vec();
    allowed.extend(order.build_dir.iter().cloned());
    let mut workspace = config.workspace.clone().allow(&allowed);
    if let Some(frozen) = &order.frozen {
        workspace = workspace.rules_from(frozen);
    }
    if order.refuse_outside {
        workspace = workspace.checkpointed();
    }
    config.workspace = workspace;
    Ok(())
}

fn own(mut config: ide_agent::Config, common: Common, inbox: &Arc<Inbox>) -> Result<Backend, LaunchError> {
    if let Some(order) = common.contract.order() {
        own_order(&mut config, order)?;
    }
    if let Some(model) = common.model {
        config.model = model;
    }
    if common.effort.is_some() {
        config.effort = common.effort;
    }
    config.thread = common.thread.id().clone();
    let session = if common.resume.is_some() {
        ide_agent::Session::resume(config, inbox.own()).map_err(|e| LaunchError::Start(format!("{e:?}")))?
    } else {
        ide_agent::Session::start(config, inbox.own())
    };
    Ok(Backend::Own(session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::Role;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ide-session-start-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        dir
    }

    fn order(frozen: Option<&Path>) -> Order {
        Order {
            order: "o-1".into(),
            role: Role::Builder,
            worktree: "/data/worktrees/o-1".into(),
            build_dir: None,
            read_roots: Vec::new(),
            allowed_domains: Vec::new(),
            refuse_outside: true,
            frozen: frozen.map(Path::to_path_buf),
            repository: Some("/work/shop".into()),
        }
    }

    #[test]
    fn claude_takes_the_frozen_folder_with_the_rules_of_its_settings() {
        let frozen = scratch("claude");
        std::fs::write(
            frozen.join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["Bash"], "deny": ["Read(./.env)"]}, "hooks": {}}"#,
        )
        .unwrap();
        std::fs::write(
            frozen.join(".claude/settings.local.json"),
            r#"{"permissions": {"ask": ["Bash(npm publish:*)"]}}"#,
        )
        .unwrap();
        let mut launch = ide_claude::Launch::new("/bin/claude", Vec::new(), "/data/worktrees/o-1");
        claude_order(&mut launch, &order(Some(&frozen)));
        let bounds = launch.order.clone().unwrap();
        assert_eq!(bounds.frozen.as_deref(), Some(frozen.as_path()));
        assert_eq!(
            bounds.frozen_settings,
            Some(serde_json::json!({"permissions": {"deny": ["Read(./.env)"], "ask": ["Bash(npm publish:*)"]}}))
        );
        assert!(!bounds.asks, "O1 refuses instead of asking");
        assert_eq!(frozen_claude_settings(&scratch("empty")), None);
        let _ = std::fs::remove_dir_all(&frozen);
    }

    #[test]
    fn codex_reads_the_frozen_agents_md_and_trusts_neither_the_worktree_nor_the_working_copy() {
        let frozen = scratch("codex");
        assert_eq!(codex_order(&order(Some(&frozen))).unwrap().instructions, None);
        std::fs::write(frozen.join("AGENTS.md"), "Run make check.\n").unwrap();
        let bounds = codex_order(&order(Some(&frozen))).unwrap();
        assert_eq!(bounds.instructions.as_deref(), Some("Run make check.\n"));
        assert_eq!(
            bounds.untrusted,
            [
                std::path::PathBuf::from("/data/worktrees/o-1"),
                std::path::PathBuf::from("/work/shop")
            ]
        );
        std::fs::write(frozen.join("AGENTS.override.md"), "Override.\n").unwrap();
        assert_eq!(frozen_agents_md(&frozen).as_deref(), Some("Override.\n"));
        let _ = std::fs::remove_dir_all(&frozen);
    }
}
