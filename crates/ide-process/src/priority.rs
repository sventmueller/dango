//! How hard the app's threads and the processes it starts compete for the processor: the terminal, agents' streams
//! and language servers' replies before indexing and git, and the commands agents run, such as `cargo build` or
//! `gopls`, last, so typing and reading stay quick while an agent builds.

use std::collections::HashSet;

use crate::Kind;
use crate::system::children;

/// The short name of process `pid`.
fn name(pid: u32) -> Option<String> {
    crate::system::name(libc::c_int::try_from(pid).ok()?)
}

/// How much a thread's work matters to the user right now; a thread nobody sets one for runs at macOS's `default`,
/// the same as every build tool an agent starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qos {
    /// Answers the user waits for: the terminal's output, an agent's stream, a language server's reply.
    UserInitiated,
    /// Work whose result shows later: indexing, git, watching files, waiting for processes.
    Utility,
    /// Upkeep nobody waits for, such as recovery copies.
    Background,
}

/// Gives the calling thread `qos`.
pub fn set_qos(qos: Qos) {
    let class = match qos {
        Qos::UserInitiated => libc::qos_class_t::QOS_CLASS_USER_INITIATED,
        Qos::Utility => libc::qos_class_t::QOS_CLASS_UTILITY,
        Qos::Background => libc::qos_class_t::QOS_CLASS_BACKGROUND,
    };
    // SAFETY: Changes only the calling thread's class; a refusal leaves it as it was.
    unsafe { libc::pthread_set_qos_class_self_np(class, 0) };
}

/// `set_qos(Qos::UserInitiated)`, as a lane's start.
pub fn user_initiated() {
    set_qos(Qos::UserInitiated);
}

/// `set_qos(Qos::Utility)`, as a lane's start.
pub fn utility() {
    set_qos(Qos::Utility);
}

/// `set_qos(Qos::Background)`, as a lane's start.
pub fn background() {
    set_qos(Qos::Background);
}

/// The nice value of the commands agents run: the lowest there is.
pub(crate) const AGENT_COMMAND_NICE: i32 = 20;

/// The nice value a process of `kind` starts with, above the app's 0; `None` keeps the app's.
pub(crate) fn nice(kind: Kind) -> Option<i32> {
    match kind {
        Kind::LanguageServer | Kind::Tool => Some(5),
        Kind::AgentCommand | Kind::Gate => Some(AGENT_COMMAND_NICE),
        Kind::Terminal | Kind::Agent | Kind::DevServer | Kind::Git | Kind::DebugAdapter | Kind::Test => None,
    }
}

/// Gives process `pid` the nice value `nice`; children it starts afterwards inherit it.
pub(crate) fn lower(pid: u32, nice: i32) {
    // SAFETY: setpriority reads only its arguments; for a process that ended meanwhile it fails and changes nothing.
    unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };
}

/// The programs that run an agent's commands: shells, and the build tools and language servers agents start on
/// their own.
const COMMANDS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "dash",
    "fish",
    "sandbox-exec",
    "cargo",
    "rustc",
    "rust-analyzer",
    "gopls",
    "go",
    "clang",
    "clang++",
    "cc",
    "c++",
    "ld",
    "swift",
    "swiftc",
    "swift-frontend",
    "xcodebuild",
    "make",
    "cmake",
    "ninja",
    "tsc",
    "javac",
    "gradle",
    "mvn",
];

/// Whether a process named `name` runs an agent's commands.
pub(crate) fn runs_commands(name: &str) -> bool {
    COMMANDS.contains(&name.trim_start_matches('-'))
}

/// Lowers to [`AGENT_COMMAND_NICE`] every process below one of `agents` that runs their commands, and all below it;
/// the agent and its own helpers, such as the `claude` an adapter starts, keep theirs. `lowered` remembers the ones
/// done between calls and loses those that ended.
pub(crate) fn lower_agent_commands(agents: &[u32], lowered: &mut HashSet<u32>) {
    let mut seen = HashSet::new();
    for &agent in agents {
        walk(agent, false, &mut seen, lowered);
    }
    lowered.retain(|pid| seen.contains(pid));
}

fn walk(parent: u32, below_command: bool, seen: &mut HashSet<u32>, lowered: &mut HashSet<u32>) {
    for child in children(parent) {
        if !seen.insert(child) {
            continue;
        }
        let command = below_command || name(child).is_some_and(|n| runs_commands(&n));
        if command && lowered.insert(child) {
            lower(child, AGENT_COMMAND_NICE);
        }
        walk(child, command, seen, lowered);
    }
}

/// The nice value of process `pid`, for tests.
#[cfg(test)]
fn nice_of(pid: u32) -> i32 {
    // SAFETY: getpriority reads only its arguments.
    unsafe { libc::getpriority(libc::PRIO_PROCESS, pid) }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    use super::*;

    #[test]
    fn a_thread_gets_the_class_it_asks_for() {
        let class = std::thread::spawn(|| {
            set_qos(Qos::Utility);
            let mut class = libc::qos_class_t::QOS_CLASS_UNSPECIFIED;
            // SAFETY: Reads the calling thread's class into a local.
            unsafe { libc::pthread_get_qos_class_np(libc::pthread_self(), &raw mut class, std::ptr::null_mut()) };
            class as u32
        })
        .join()
        .unwrap();
        assert_eq!(class, libc::qos_class_t::QOS_CLASS_UTILITY as u32);
    }

    #[test]
    fn only_what_runs_commands_is_lowered_and_what_it_starts_inherits() {
        // A stand-in agent: `sleep` is its own helper, the shell runs its command, and the shell's `sleep` started
        // before the shell was lowered.
        let mut agent = Command::new("/bin/sh")
            .args(["-c", "/bin/sleep 30 & /bin/zsh -c '/bin/sleep 30 & wait' & wait"])
            .stdin(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let pid = agent.id();
        let mut lowered = HashSet::new();
        lower_agent_commands(&[pid], &mut lowered);
        let below = children(pid);
        let helper = below
            .iter()
            .copied()
            .find(|&c| name(c).as_deref() == Some("sleep"))
            .unwrap();
        let shell = below
            .iter()
            .copied()
            .find(|&c| name(c).as_deref() == Some("zsh"))
            .unwrap();
        let command = children(shell)[0];
        assert_eq!(
            nice_of(helper),
            nice_of(std::process::id()),
            "the agent's own helper keeps the priority it started with"
        );
        assert_eq!(
            (nice_of(shell), nice_of(command)),
            (AGENT_COMMAND_NICE, AGENT_COMMAND_NICE)
        );
        assert_eq!(lowered.len(), 2);
        // SAFETY: Ends the stand-in's own process group, which the test started.
        unsafe { libc::killpg(libc::pid_t::try_from(pid).unwrap(), libc::SIGKILL) };
        let _ = agent.wait();
        lower_agent_commands(&[pid], &mut lowered);
        assert!(lowered.is_empty(), "ended processes are forgotten");
    }

    #[test]
    fn language_servers_yield_a_little_and_agent_commands_most() {
        assert_eq!(nice(Kind::LanguageServer), Some(5));
        assert_eq!(nice(Kind::AgentCommand), Some(AGENT_COMMAND_NICE));
        assert_eq!(
            nice(Kind::Gate),
            Some(AGENT_COMMAND_NICE),
            "gates yield like agents' commands (F70)"
        );
        assert_eq!(nice(Kind::Terminal), None);
        assert!(runs_commands("-zsh") && runs_commands("cargo") && !runs_commands("claude"));
    }
}
