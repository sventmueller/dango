//! Getting a session of an order ready to launch (E165, E170, E172): off the main thread, the path's program, the
//! login shell's environment and what E186 leaves of it, the folders the order's build reads beside the repository
//! (E188) and its toolchains, and the id that resumes the session after a restart (A15): a UUID Claude starts under, or the own agent's
//! thread, whose stored conversation it picks up. The launch itself happens on the main thread.

use std::path::PathBuf;

use ide_factory::routing::{Agent, RankEntry};
use ide_factory::{OrderId, Role};
use sha2::{Digest, Sha256};

use super::effects::login;
use super::sessions::{Program, Ready};
use super::{Delivery, Leitstand, own, siblings};

/// What an order asked of a session to start.
pub(super) struct Wanted {
    pub(super) order: OrderId,
    pub(super) role: Role,
    pub(super) entry: RankEntry,
    pub(super) resume: Option<String>,
    pub(super) worktree: PathBuf,
    pub(super) frozen: Option<PathBuf>,
    pub(super) network: Vec<String>,
}

/// A path's program, and the ids it starts under.
struct Found {
    program: Program,
    resume: Option<String>,
    session_id: Option<String>,
}

impl Leitstand {
    /// Looks for the session's program and environment off the main thread, then launches it there.
    pub(super) fn start_session(&self, root: &std::path::Path, wanted: Wanted) {
        let order = self.with_order(root, &wanted.order, |o| {
            (o.title().to_string(), super::sessions::starting(o.phase(), wanted.role))
        });
        let Some((title, Some(index))) = order else {
            return;
        };
        #[cfg(feature = "selftest")]
        let script = self.probe.script_for(&title, wanted.role);
        let root = root.to_path_buf();
        let back = self.back();
        let started = crate::util::spawn("order-session", move || {
            let login = login();
            let env = ide_process::order_env::for_orders(&login, &[]);
            let found = find(&login, &wanted);
            #[cfg(feature = "selftest")]
            let found = match script {
                Some(script) => Found {
                    program: Program::Script(script),
                    ..found
                },
                None => found,
            };
            let mut read_roots = siblings::targets(&root);
            read_roots.extend(siblings::links(&root, &wanted.worktree));
            read_roots.extend(toolchains(&ide_core::paths::home()));
            back.deliver(Delivery::Launch(Box::new(Ready {
                order: wanted.order,
                title,
                role: wanted.role,
                index,
                entry: wanted.entry,
                resume: found.resume,
                session_id: found.session_id,
                worktree: wanted.worktree,
                frozen: wanted.frozen,
                read_roots,
                program: found.program,
                root,
                login,
                env,
                network: wanted.network,
            })));
        });
        if let Err(why) = started {
            ide_core::diag::report("starting an order's session", why);
        }
    }
}

/// The paths below `home` a build reads as a gate does, where toolchains keep their caches and package stores, as far
/// as they exist.
fn toolchains(home: &std::path::Path) -> Vec<PathBuf> {
    use ide_process::sandbox::{TOOL_FILES, TOOL_FOLDERS};
    TOOL_FOLDERS
        .iter()
        .chain(TOOL_FILES)
        .map(|path| home.join(path))
        .filter(|path| path.exists())
        .collect()
}

/// The program of the path `wanted` names on the login shell's PATH, with the ids its session starts under.
fn find(login: &[(String, String)], wanted: &Wanted) -> Found {
    let path = ide_process::tool::var(login, "PATH");
    let program = |found: Option<Program>| Found {
        program: found.unwrap_or(Program::Missing),
        resume: wanted.resume.clone(),
        session_id: None,
    };
    match &wanted.entry.agent {
        Agent::Claude => Found {
            session_id: wanted.resume.is_none().then(|| session_id(&wanted.order)),
            ..program(ide_ui_agent::claude_program(login).map(Program::Claude))
        },
        Agent::Codex => program(ide_process::tool::which("codex", path).map(|codex| {
            match ide_codex::version::mcp_servers(ide_process::Supervisor::global(), &codex, login) {
                Ok(servers) => Program::Codex(codex, servers),
                Err(why) => Program::Unready(why.to_string()),
            }
        })),
        Agent::Own => match own::start(&wanted.worktree, &wanted.entry, wanted.resume.as_deref()) {
            Ok(start) => Found {
                program: Program::Own(start.config),
                resume: start.resumes.then(|| start.thread.clone()),
                session_id: Some(start.thread),
            },
            Err(why) => program(Some(Program::Unready(why.0))),
        },
        _ => program(None),
    }
}

/// A new UUID in the version 4 layout for a Claude session of `order`, hashed from a new thread id, which no other
/// thread on this Mac has, so two sessions never share one.
fn session_id(order: &OrderId) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ide_thread::ThreadId::new().as_str().as_bytes());
    hasher.update(order.0.as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize()[..16].try_into().unwrap_or_default();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_session_id_is_a_fresh_version_four_uuid() {
        let order = OrderId("o-1".into());
        let (a, b) = (session_id(&order), session_id(&order));
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12],
            "{a}"
        );
        assert!(parts[2].starts_with('4'), "{a}");
        assert!(matches!(parts[3].chars().next(), Some('8' | '9' | 'a' | 'b')), "{a}");
        assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }
}
