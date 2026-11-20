//! How hard the app's threads and the processes it starts compete for the processor: the terminal, agents' streams
//! and language servers' replies before indexing and git, the agents themselves in the middle, and everything an agent
//! starts, in whatever language, last and in macOS's background state, so the app stays quick while agents build.

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

/// The nice value of the agents, and of the agent programs an adapter starts, whose subagents run inside them.
pub(crate) const AGENT_NICE: i32 = 10;

/// The nice value a process of `kind` starts with, above the app's 0; `None` keeps the app's.
pub(crate) fn nice(kind: Kind) -> Option<i32> {
    match kind {
        Kind::LanguageServer | Kind::Tool => Some(5),
        Kind::Agent => Some(AGENT_NICE),
        Kind::AgentCommand | Kind::Gate => Some(AGENT_COMMAND_NICE),
        Kind::Terminal | Kind::DevServer | Kind::Git | Kind::DebugAdapter | Kind::Test => None,
    }
}

/// Whether a process of `kind` runs in macOS's background state from its start.
pub(crate) fn in_background(kind: Kind) -> bool {
    kind == Kind::AgentCommand
}

/// Gives process `pid` the nice value `nice`; children it starts afterwards inherit it.
pub(crate) fn lower(pid: u32, nice: i32) {
    // SAFETY: setpriority reads only its arguments; for a process that ended meanwhile it fails and changes nothing.
    unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };
}

/// Puts process `pid` in macOS's background state: the lowest scheduling priority, and disk and network access
/// throttled; children it starts afterwards inherit it.
pub(crate) fn to_background(pid: u32) {
    // SAFETY: setpriority reads only its arguments; for a process that ended meanwhile it fails and changes nothing.
    unsafe { libc::setpriority(libc::PRIO_DARWIN_PROCESS, pid, libc::PRIO_DARWIN_BG) };
}

/// The agent programs an adapter starts, which keep [`AGENT_NICE`] where everything else an agent starts goes to the
/// background.
const AGENTS: &[&str] = &["claude", "codex", "opencode", "gemini"];

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

/// Whether a process named `name` is an agent program an adapter starts.
fn is_agent_program(name: Option<&str>) -> bool {
    name.is_some_and(|name| AGENTS.contains(&name))
}

/// Whether a process named `name` runs an agent's commands.
pub(crate) fn runs_commands(name: &str) -> bool {
    COMMANDS.contains(&name.trim_start_matches('-'))
}

/// Puts every process below one of `agents` in the background state, whatever it runs, except the agent programs an
/// adapter starts, which get [`AGENT_NICE`]. `lowered` remembers the ones done between calls and loses those that
/// ended.
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
        let command = below_command || !is_agent_program(name(child).as_deref());
        if lowered.insert(child) {
            if command {
                to_background(child);
            } else {
                lower(child, AGENT_NICE);
            }
        }
        walk(child, command, seen, lowered);
    }
}

/// The scheduling priority of process `pid`'s task, for tests; macOS 27 shows the background state here and no longer
/// in the nice value.
#[cfg(test)]
fn base_priority(pid: u32) -> Option<i32> {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::uninit();
    let size = i32::try_from(size_of::<libc::proc_taskinfo>()).ok()?;
    // SAFETY: The kernel writes at most `size` bytes into the buffer. Only a full result is read.
    let read = unsafe {
        libc::proc_pidinfo(
            i32::try_from(pid).ok()?,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    // SAFETY: The full task info was written by proc_pidinfo.
    (read == size).then(|| unsafe { info.assume_init() }.pti_priority)
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    use super::*;

    /// The priority macOS gives a task in the background state, `MAXPRI_THROTTLE` in the kernel's sources.
    const BACKGROUND_PRIORITY: i32 = 4;

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
    fn everything_an_agent_starts_goes_to_the_background() {
        // A stand-in agent: a helper, and a shell whose `sleep` started before the shell was lowered.
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
        let named = |wanted: &str| {
            below
                .iter()
                .copied()
                .find(|&c| name(c).as_deref() == Some(wanted))
                .unwrap()
        };
        let (helper, shell) = (named("sleep"), named("zsh"));
        let command = children(shell)[0];
        assert_eq!(
            [base_priority(helper), base_priority(shell), base_priority(command)],
            [Some(BACKGROUND_PRIORITY); 3],
            "everything, and what it started before, is in the background"
        );
        assert_eq!(lowered.len(), 3);
        // SAFETY: Ends the stand-in's own process group, which the test started.
        unsafe { libc::killpg(libc::pid_t::try_from(pid).unwrap(), libc::SIGKILL) };
        let _ = agent.wait();
        lower_agent_commands(&[pid], &mut lowered);
        assert!(lowered.is_empty(), "ended processes are forgotten");
    }

    #[test]
    fn only_agent_programs_stay_out_of_the_background() {
        assert!(is_agent_program(Some("claude")) && is_agent_program(Some("codex")));
        assert!(!is_agent_program(Some("node")) && !is_agent_program(Some("python3")) && !is_agent_program(None));
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
        assert_eq!(nice(Kind::Agent), Some(AGENT_NICE));
        assert!(in_background(Kind::AgentCommand) && !in_background(Kind::Gate) && !in_background(Kind::Agent));
        assert_eq!(nice(Kind::Terminal), None);
        assert!(runs_commands("-zsh") && runs_commands("cargo") && !runs_commands("claude"));
    }
}
