//! Where a tab's thread comes from: a running `claude` process, an agent over ACP, or a recording played for the
//! self-test.

use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(feature = "selftest")]
use std::time::SystemTime;

#[cfg(feature = "selftest")]
use ide_claude::replay::Replay;
use ide_claude::{Capabilities, Event, PermissionAnswer, PlanAnswer, Session, SessionError};
use ide_thread::{Answer, Thread, UserMessage};

/// A past session to continue.
#[derive(Debug, Clone)]
pub struct Resume {
    /// The CLI's id for it.
    pub session: String,
    /// The app's id for its thread, when the index knows it.
    pub thread: Option<ide_thread::ThreadId>,
    /// The title it had.
    pub title: Option<String>,
    /// The transcript the CLI saved, whose conversation is shown before it continues.
    pub transcript: Option<PathBuf>,
}

/// What keeps a tab from running a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The folder is not trusted (E22).
    Untrusted,
    /// No `claude` on the login shell's PATH.
    NotFound,
    /// The CLI has no usable sign-in.
    SignIn(String),
    /// The process did not start.
    Start(String),
    /// The ACP agent is neither on the PATH nor installed; how to install it, when it can be.
    Missing(Option<Box<ide_acp::Plan>>),
    /// The agent is being installed; how far it got.
    Installing(String),
    /// The ACP agent wants a sign-in before it opens a session.
    AcpSignIn,
    /// The own agent has no endpoint with a key and a model yet.
    NoModel,
}

pub enum Source {
    /// Nothing runs yet, or any more.
    Idle,
    /// Looking for `claude` and the login shell's environment.
    Starting,
    Live(Session),
    /// An agent over ACP.
    Acp(ide_acp::Session),
    /// The app's own agent.
    Own(ide_agent::Session),
    #[cfg(feature = "selftest")]
    Replay(Box<Replay>),
}

impl Source {
    pub fn session(&self) -> Option<&Session> {
        match self {
            Source::Live(session) => Some(session),
            _ => None,
        }
    }

    /// The thread as it is now, when there is one.
    pub fn snapshot(&self) -> Option<Thread> {
        match self {
            Source::Live(session) => Some(session.snapshot()),
            Source::Acp(session) => Some(session.snapshot()),
            Source::Own(session) => Some(session.snapshot()),
            #[cfg(feature = "selftest")]
            Source::Replay(replay) => Some(replay.thread().clone()),
            Source::Idle | Source::Starting => None,
        }
    }

    pub fn capabilities(&self) -> Option<Capabilities> {
        self.session().map(Session::capabilities)
    }

    /// Whether a recording plays instead of a process, which a restart must not replace with a real `claude`.
    pub fn is_replay(&self) -> bool {
        #[cfg(feature = "selftest")]
        {
            matches!(self, Source::Replay(_))
        }
        #[cfg(not(feature = "selftest"))]
        {
            false
        }
    }

    pub fn is_live(&self) -> bool {
        match self {
            Source::Acp(session) => session.is_ready(),
            Source::Own(_) => true,
            _ => self.session().is_some_and(|s| !s.is_ended()),
        }
    }

    /// The own agent's session, when the tab runs one.
    pub fn own(&self) -> Option<&ide_agent::Session> {
        match self {
            Source::Own(session) => Some(session),
            _ => None,
        }
    }

    /// The ACP session, when the tab runs one.
    pub fn acp(&self) -> Option<&ide_acp::Session> {
        match self {
            Source::Acp(session) => Some(session),
            _ => None,
        }
    }

    pub fn acp_capabilities(&self) -> Option<ide_acp::Capabilities> {
        self.acp().map(ide_acp::Session::capabilities)
    }

    pub fn authenticate(&self, method: &str) -> Result<(), ide_acp::SessionError> {
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.authenticate(method))
    }

    pub fn set_option(&self, option: &str, value: &str) -> Result<(), ide_acp::SessionError> {
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.set_option(option, value))
    }

    pub fn set_acp_mode(&self, mode: &str) -> Result<(), ide_acp::SessionError> {
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.set_mode(mode))
    }

    pub fn answer_acp(&self, request: &str, choice: ide_acp::Choice) -> Result<(), ide_acp::SessionError> {
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.answer(request, choice))
    }

    /// Sends `message` as a prompt; the CLI's id for it, which an ACP agent does not give.
    pub fn send(&self, message: UserMessage) -> Result<Option<String>, String> {
        match self {
            Source::Live(session) => session.send(message).map(Some).map_err(|e| e.to_string()),
            Source::Acp(session) => session.send(message).map(|()| None).map_err(|e| e.to_string()),
            Source::Own(session) => session.send(message).map(|()| None).map_err(|e| e.to_string()),
            _ => Err(SessionError::Closed.to_string()),
        }
    }

    pub fn interrupt(&self) {
        if let Some(session) = self.session() {
            session.interrupt().then(|_| ());
        }
        if let Some(session) = self.acp() {
            session.cancel();
        }
        if let Some(session) = self.own() {
            session.cancel();
        }
    }

    pub fn answer(&self, request: &str, answer: PermissionAnswer) -> Result<(), SessionError> {
        self.session()
            .map_or(Err(SessionError::Closed), |s| s.answer(request, answer))
    }

    pub fn answer_questions(&self, request: &str, answers: Vec<Answer>) -> Result<(), SessionError> {
        self.session()
            .map_or(Err(SessionError::Closed), |s| s.answer_questions(request, answers))
    }

    /// Answers a plan; what a recording played for the self-test changed, which a session reports as an event.
    pub fn answer_plan(&mut self, request: &str, answer: PlanAnswer) -> Result<Vec<ide_thread::Change>, SessionError> {
        #[cfg(feature = "selftest")]
        if let Source::Replay(replay) = self {
            return Ok(replay.answer_plan(request, &answer));
        }
        self.session()
            .map_or(Err(SessionError::Closed), |s| s.answer_plan(request, answer))
            .map(|()| Vec::new())
    }

    pub fn stop_task(&self, task: &str) {
        if let Some(session) = self.session() {
            session.stop_task(task).then(|_| ());
        }
    }

    pub fn background(&self, call: &str) {
        if let Some(session) = self.session() {
            session.background(Some(call)).then(|_| ());
        }
    }

    pub fn set_mode(&self, mode: &str) {
        if let Some(session) = self.session() {
            session.set_permission_mode(mode).then(|_| ());
        }
    }

    pub fn set_model(&self, model: &str) {
        if let Some(session) = self.session() {
            session.set_model(Some(model)).then(|_| ());
        }
    }

    pub fn set_effort(&self, level: &str) {
        if let Some(session) = self.session() {
            session.set_effort(level).then(|_| ());
        }
    }

    /// Ends the process: input closed, then the supervisor's signals.
    pub fn end(&self) {
        if let Some(session) = self.session() {
            session.stop(Duration::from_secs(2));
        }
        if let Some(session) = self.acp() {
            session.stop(Duration::from_secs(2));
        }
        if let Some(session) = self.own() {
            session.stop();
        }
    }

    /// Plays recorded lines into the replay; what changed.
    #[cfg(feature = "selftest")]
    pub fn feed(&mut self, lines: &str) -> Vec<ide_thread::Change> {
        match self {
            Source::Replay(replay) => replay.feed_all(lines),
            _ => Vec::new(),
        }
    }
}

/// A replay into an empty thread, as the self-test plays it.
#[cfg(feature = "selftest")]
pub fn replay(thread: Thread) -> Source {
    Source::Replay(Box::new(Replay::new(
        thread,
        SystemTime::now(),
        Duration::from_millis(40),
    )))
}

/// The `claude` executable on the login shell's PATH, else in the places its installers use.
pub fn find_claude(env: &[(String, String)]) -> Option<PathBuf> {
    let home = ide_core::paths::home();
    let path = env
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .chain([
            home.join(".local/bin"),
            home.join(".claude/local"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ])
        .map(|dir| dir.join("claude"))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// What a session event means for the tab, beyond the thread changes.
pub fn problem_of(event: &Event) -> Option<Problem> {
    match event {
        Event::Failed(SessionError::SignInRequired(why)) => Some(Problem::SignIn(why.clone())),
        Event::Failed(SessionError::NotFound(_)) => Some(Problem::NotFound),
        Event::Failed(SessionError::Start(why)) => Some(Problem::Start(why.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_is_found_on_the_path_the_login_shell_gives() {
        let dir = std::env::temp_dir().join(format!("ide-find-claude-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("claude");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = vec![("PATH".to_string(), format!("/nonexistent:{}", dir.display()))];
        assert_eq!(find_claude(&env), Some(exe));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
