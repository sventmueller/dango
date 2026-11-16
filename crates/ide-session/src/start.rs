//! Starting a session on each path, with the contract in the path's own terms (E173 and its table per path): the
//! user's choices as the AI tab makes them, or an order's bounds; a path that cannot hold an order's bounds is
//! refused before anything starts (C48, C59, C72).

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
        thread,
        contract,
    };
    match path {
        AgentPath::Claude { program } => claude(program, common, supervisor, inbox),
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
    }
}

/// What every path takes from the launch.
struct Common {
    folder: std::path::PathBuf,
    env: Vec<(String, String)>,
    model: Option<String>,
    effort: Option<String>,
    resume: Option<String>,
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
    }
}

/// The Claude launch of an order (E173): its bounds, and no Remote Control.
pub(crate) fn claude_order(launch: &mut ide_claude::Launch, order: &Order) {
    launch.order = Some(ide_claude::OrderBounds {
        builds: order.role.builds(),
        asks: !order.refuse_outside,
        worktree: order.worktree.clone(),
        build_dir: order.build_dir.clone(),
        read_roots: order.read_roots.clone(),
        allowed_domains: order.allowed_domains.clone(),
    });
    launch.allow_bypass = false;
    launch.remote_control = Some(false);
}

fn claude(
    program: std::path::PathBuf,
    common: Common,
    supervisor: &Supervisor,
    inbox: &Arc<Inbox>,
) -> Result<Backend, LaunchError> {
    let mut launch = ide_claude::Launch::new(program, common.env, &common.folder);
    launch.model = common.model;
    launch.effort = common.effort;
    launch.resume = common.resume;
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
    Ok(ide_codex::Confinement {
        writes: order.role.builds(),
        write_roots: order.build_dir.iter().cloned().collect(),
    })
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
    config.workspace = config.workspace.clone().allow(&allowed);
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
