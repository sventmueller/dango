//! What a session starts with: the path to the agent, the folder, the environment, the model, the session to resume
//! and the contract it runs under, the user's or a work order's (docs/phase-13.md, E165, E173).

use std::path::{Path, PathBuf};

use ide_thread::Thread;

/// The five paths to an agent, and the scripted session of the tests.
pub enum AgentPath {
    /// Claude Code in stream-json mode (`ide-claude`).
    Claude {
        /// The `claude` executable.
        program: PathBuf,
        /// Whether the session connects to Remote Control as it starts; `None` follows the user's Claude settings.
        /// An order's session never connects.
        remote_control: Option<bool>,
        /// Where the session `Launch::resume` names branches off: it continues as a new session that ends just
        /// before a prompt, guarded so that the CLI refuses rather than drops more than that prompt's turn (I6).
        rewind: Option<ide_thread::Rewind>,
    },
    /// Codex over a server of its own (`ide-codex`); never the shared one of Remote Control (E193).
    Codex {
        /// The `codex` executable.
        program: PathBuf,
        /// The release already read; the handshake reads it when `None`.
        version: Option<ide_codex::Version>,
    },
    /// OpenCode over its HTTP API (`ide-opencode`).
    OpenCode {
        /// The `opencode` executable.
        program: PathBuf,
        /// The release already read; the handshake reads it when `None`.
        version: Option<ide_opencode::Version>,
    },
    /// An agent of the ACP register, as the app resolved its preset (`ide-acp`).
    Acp(Box<ide_acp::Launch>),
    /// The app's own agent with its endpoint, key, model and workspace (`ide-agent`).
    Own(Box<ide_agent::Config>),
    /// The own agent continuing the conversation its store kept, which the caller read off the main thread.
    OwnStored(Box<ide_agent::Config>, Box<ide_agent::store::Stored>),
    /// A session that plays a script, for the self-test and the tests (F61).
    #[cfg(any(test, feature = "selftest"))]
    Scripted(crate::script::Script),
    /// A recording of Claude Code played into its thread by the caller, with no process behind it, for the
    /// self-test's AI tabs; it takes no prompts.
    #[cfg(any(test, feature = "selftest"))]
    Replay(Box<ide_claude::replay::Replay>),
}

/// Which path a session takes, without what it needs to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathKind {
    /// Claude Code.
    Claude,
    /// Codex.
    Codex,
    /// OpenCode.
    OpenCode,
    /// An ACP agent.
    Acp,
    /// The own agent.
    Own,
    /// A scripted session.
    Scripted,
    /// A played recording of Claude Code.
    Replay,
}

impl AgentPath {
    /// Claude Code's `program`, as the user's Claude settings set Remote Control, with no rewind.
    pub fn claude(program: impl Into<PathBuf>) -> AgentPath {
        AgentPath::Claude {
            program: program.into(),
            remote_control: None,
            rewind: None,
        }
    }

    /// Which path it is.
    pub fn kind(&self) -> PathKind {
        match self {
            AgentPath::Claude { .. } => PathKind::Claude,
            AgentPath::Codex { .. } => PathKind::Codex,
            AgentPath::OpenCode { .. } => PathKind::OpenCode,
            AgentPath::Acp(_) => PathKind::Acp,
            AgentPath::Own(_) | AgentPath::OwnStored(..) => PathKind::Own,
            #[cfg(any(test, feature = "selftest"))]
            AgentPath::Scripted(_) => PathKind::Scripted,
            #[cfg(any(test, feature = "selftest"))]
            AgentPath::Replay(_) => PathKind::Replay,
        }
    }
}

/// The roles of a work order (E162).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Writes the specification; only reads.
    Planner,
    /// Writes in the worktree.
    Builder,
    /// Judges the result; only reads.
    Reviewer,
    /// Reads and sums up; only reads.
    Scout,
}

impl Role {
    /// Whether the role writes; every other role only reads, on paths that block writing (C48).
    pub fn builds(self) -> bool {
        self == Role::Builder
    }

    /// Its name in ids, such as the checkpoint thread's.
    pub fn id(self) -> &'static str {
        match self {
            Role::Planner => "planner",
            Role::Builder => "builder",
            Role::Reviewer => "reviewer",
            Role::Scout => "scout",
        }
    }
}

/// The bounds a work order's session runs in (E173): its worktree, its build folder, what it may read and reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// The order's id.
    pub order: String,
    /// The role the session fills.
    pub role: Role,
    /// The order's worktree, which is the session's folder.
    pub worktree: PathBuf,
    /// The order's build folder.
    pub build_dir: Option<PathBuf>,
    /// Folders that may be read besides the worktree and the build folder, such as toolchains, where the path can
    /// keep reads to named folders.
    pub read_roots: Vec<PathBuf>,
    /// The domains commands may reach; none means no network.
    pub allowed_domains: Vec<String>,
    /// Whether requests outside the bounds are refused and the agent told why; without the owner's decision O1 they
    /// go to the user instead (C72). With it, git that discards only the worktree's own work runs without asking,
    /// since the checkpoint before each turn brings it back (E173, C47).
    pub refuse_outside: bool,
    /// The agent configuration frozen from the user's working copy (E172): the project settings and instruction files
    /// the session loads instead of the worktree's.
    pub frozen: Option<PathBuf>,
    /// The user's working copy the worktree belongs to, whose trust a path may hand on to the worktree.
    pub repository: Option<PathBuf>,
    /// The MCP servers the path's own configuration would start, each switched off by name (E191).
    pub mcp_servers: Vec<String>,
}

impl Order {
    /// The name of the checkpoint thread of this order and role (F67): the order's id, with what a ref name cannot
    /// hold replaced, and the role.
    pub fn checkpoint_thread(&self) -> String {
        let id: String = self
            .order
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '-' })
            .collect();
        let id = id.trim_matches('-');
        let id = if id.is_empty() { "order" } else { id };
        let mut name = format!("{id}-{}", self.role.id());
        if name.len() > 128 {
            name = format!("{}-{}", &id[..100.min(id.len())], self.role.id());
        }
        name
    }
}

/// The choices of a session the user drives, as the AI tab makes them today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserChoices {
    /// The mode: Claude's permission mode, a Codex preset of `ide_codex::MODES`, an OpenCode agent; the path's own
    /// when `None`.
    pub mode: Option<String>,
    /// Whether Claude's shell commands run in its sandbox.
    pub sandbox: bool,
    /// Whether a Codex session joins Codex's shared server, the one Remote Control reaches (E148), which the caller
    /// made sure runs; an order's session never does (E193).
    pub shared: bool,
}

/// Whom the session works for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contract {
    /// The user, as an AI tab drives a session today.
    User(UserChoices),
    /// A work order, in its bounds.
    Order(Order),
}

impl Contract {
    /// The order, when the session works on one.
    pub fn order(&self) -> Option<&Order> {
        match self {
            Contract::Order(order) => Some(order),
            Contract::User(_) => None,
        }
    }
}

/// Everything that decides how a session starts.
pub struct Launch {
    /// The path to the agent.
    pub path: AgentPath,
    /// The folder the agent works in; for an order, its worktree.
    pub folder: PathBuf,
    /// The environment the agent gets: the login shell's, which the app filters for an order (E186).
    pub env: Vec<(String, String)>,
    /// The model; the path's default when `None`.
    pub model: Option<String>,
    /// The reasoning effort; the path's default when `None`.
    pub effort: Option<String>,
    /// The agent's id of a session to resume (A15); for the own agent, any value resumes the thread its store keeps.
    pub resume: Option<String>,
    /// The id a new session takes, where the path lets the caller choose it at the start (Claude: a UUID), so it can
    /// be resumed before its first turn named it.
    pub session_id: Option<String>,
    /// The thread to fill, such as one rebuilt from a transcript; a new one when `None`.
    pub thread: Option<Thread>,
    /// git for the checkpoint before each prompt (I6, E34); none are taken without it.
    pub git: Option<ide_git::Git>,
    /// Whom the session works for.
    pub contract: Contract,
}

impl Launch {
    /// A launch of `path` in `folder` with `env` under `contract`, with the path's own model and no checkpoints.
    pub fn new(path: AgentPath, folder: impl Into<PathBuf>, env: Vec<(String, String)>, contract: Contract) -> Launch {
        Launch {
            path,
            folder: folder.into(),
            env,
            model: None,
            effort: None,
            resume: None,
            session_id: None,
            thread: None,
            git: None,
            contract,
        }
    }
}

/// Why a session did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The path takes no role in work orders: nothing bounds it technically (C48, C72).
    NoRole(PathKind),
    /// The path lacks what the order's contract needs in the release this build was tested with (C59).
    Unsupported {
        /// The path.
        path: PathKind,
        /// What it lacks.
        missing: &'static str,
    },
    /// An order's session works in its worktree, and the launch named another folder.
    Folder {
        /// The folder the launch named.
        folder: PathBuf,
        /// The order's worktree.
        worktree: PathBuf,
    },
    /// The program is not there.
    NotFound(PathBuf),
    /// The session did not start; the path's own words.
    Start(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::NoRole(path) => write!(f, "{path:?} takes no role in work orders: it has no sandbox"),
            LaunchError::Unsupported { path, missing } => write!(f, "{path:?} offers no {missing}"),
            LaunchError::Folder { folder, worktree } => write!(
                f,
                "an order's session works in {}, not in {}",
                worktree.display(),
                folder.display()
            ),
            LaunchError::NotFound(program) => write!(f, "{} is not there", program.display()),
            LaunchError::Start(why) => write!(f, "the session did not start: {why}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// Whether `a` and `b` name the same folder, links resolved where they exist.
pub(crate) fn same_folder(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(id: &str, role: Role) -> Order {
        Order {
            order: id.into(),
            role,
            worktree: "/w".into(),
            build_dir: None,
            read_roots: Vec::new(),
            allowed_domains: Vec::new(),
            refuse_outside: true,
            frozen: None,
            repository: None,
            mcp_servers: Vec::new(),
        }
    }

    #[test]
    fn the_checkpoint_thread_is_the_order_and_the_role_in_what_a_ref_name_holds() {
        assert_eq!(order("o-17", Role::Builder).checkpoint_thread(), "o-17-builder");
        assert_eq!(
            order("CSV export/2", Role::Reviewer).checkpoint_thread(),
            "CSV-export-2-reviewer"
        );
        assert_eq!(order("..", Role::Scout).checkpoint_thread(), "order-scout");
        let long = order(&"x".repeat(300), Role::Planner).checkpoint_thread();
        assert!(long.len() <= 128 && long.ends_with("-planner"), "{long}");
        for name in [
            order("a.b", Role::Builder).checkpoint_thread(),
            order("ref.lock", Role::Builder).checkpoint_thread(),
        ] {
            assert!(ide_git::CheckpointThread::new(name.clone()).is_ok(), "{name}");
        }
    }

    #[test]
    fn only_the_builder_writes() {
        assert!(Role::Builder.builds());
        for role in [Role::Planner, Role::Reviewer, Role::Scout] {
            assert!(!role.builds(), "{role:?}");
        }
    }
}
