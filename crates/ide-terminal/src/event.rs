//! What a terminal session reports to its owner.

use std::path::PathBuf;

use ide_process::Ending;

use crate::sys;

/// A change in a terminal session, delivered on the session's reader thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The screen changed. Sent once until the next `Session::snapshot`, however much output arrives meanwhile.
    Redraw,
    /// The program set the window title, or reset it with `None`.
    Title(Option<String>),
    /// The program rang the bell.
    Bell,
    /// The program asked to put `text` on a clipboard (OSC 52).
    ClipboardStore {
        /// Which clipboard.
        clipboard: Clipboard,
        /// What to put there.
        text: String,
    },
    /// The program asked to read a clipboard (OSC 52); only sent when the session allows it. The answer goes to
    /// `Session::answer_clipboard`.
    ClipboardLoad(Clipboard),
    /// The shell reported its working directory (OSC 7).
    WorkingDirectory(WorkingDirectory),
    /// The shell shows its prompt (OSC 133 A).
    Prompt,
    /// The shell started a command line (OSC 133 C).
    CommandStarted,
    /// The command line ended (OSC 133 D), with its exit code when the shell reported one.
    CommandFinished {
        /// The exit code.
        exit_code: Option<i32>,
    },
    /// The output announced a local web server, such as `http://localhost:5173/`; each address once per session.
    DevServer(String),
    /// The program in the terminal ended.
    Exited(Ending),
}

/// One of the clipboards OSC 52 addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clipboard {
    /// The general pasteboard.
    Clipboard,
    /// The X11 primary selection, which a Mac does not have; the app decides what to do with it.
    Selection,
}

/// A working directory as a shell reports it, possibly on another machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingDirectory {
    /// The host the shell runs on; empty when the shell left it out.
    pub host: String,
    /// The directory.
    pub path: PathBuf,
}

impl WorkingDirectory {
    /// Whether the directory is on this Mac rather than at the other end of an `ssh` session.
    pub fn is_local(&self) -> bool {
        let host = self.host.as_str();
        if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
            return true;
        }
        let local = sys::host_name();
        let short = |name: &str| name.split('.').next().unwrap_or(name).to_ascii_lowercase();
        host.eq_ignore_ascii_case(local) || (!local.is_empty() && short(host) == short(local))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_without_host_or_on_this_mac_is_local() {
        let at = |host: &str| WorkingDirectory {
            host: host.into(),
            path: "/tmp".into(),
        };
        assert!(at("").is_local());
        assert!(at("localhost").is_local());
        assert!(at(sys::host_name()).is_local());
        assert!(!at("build-server.example.com").is_local());
    }
}
