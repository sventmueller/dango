//! Where a tab's thread comes from: a running `claude` process, an agent over ACP, or a recording played for the
//! self-test.

#[cfg(feature = "selftest")]
use ide_core::LockIgnoringPoison;
use ide_core::explain::{Explain, Explained};
use std::path::PathBuf;
use std::time::Duration;
#[cfg(feature = "selftest")]
use std::time::SystemTime;

#[cfg(feature = "selftest")]
use ide_claude::replay::Replay;
use ide_claude::{Capabilities, Event, PermissionAnswer, PlanAnswer, Session, SessionError};
use ide_process::tool;
use ide_thread::{Answer, Thread, UserMessage};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

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
    /// The register's id of the agent of a session the app keeps no thread of, such as a Codex thread from outside
    /// the app; Claude's when none.
    pub agent: Option<String>,
}

/// What keeps a tab from running a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Problem {
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
    /// The ACP agent's program ended on its own after its session opened; the last line it wrote to stderr.
    Exited(Option<String>),
}

pub(crate) enum Source {
    /// Nothing runs yet, or any more.
    Idle,
    /// Looking for `claude` and the login shell's environment.
    Starting,
    Live(Session),
    /// An agent over ACP.
    Acp(ide_acp::Session),
    /// The app's own agent.
    Own(ide_agent::Session),
    /// Codex over its app-server (E147).
    Codex(ide_codex::Session),
    /// OpenCode over its v2 HTTP API (E147).
    OpenCode(ide_opencode::Session),
    #[cfg(feature = "selftest")]
    Replay(Box<Replay>),
}

impl Source {
    pub(crate) fn session(&self) -> Option<&Session> {
        match self {
            Source::Live(session) => Some(session),
            _ => None,
        }
    }

    /// The thread as it is now, when there is one.
    pub(crate) fn snapshot(&self) -> Option<Thread> {
        match self {
            Source::Live(session) => Some(session.snapshot()),
            Source::Acp(session) => Some(session.snapshot()),
            Source::Own(session) => Some(session.snapshot()),
            Source::Codex(session) => Some(session.snapshot()),
            Source::OpenCode(session) => Some(session.snapshot()),
            #[cfg(feature = "selftest")]
            Source::Replay(replay) => Some(replay.thread().clone()),
            Source::Idle | Source::Starting => None,
        }
    }

    pub(crate) fn capabilities(&self) -> Option<Capabilities> {
        self.session().map(Session::capabilities)
    }

    /// Whether a recording plays instead of a process, which a restart must not replace with a real `claude`.
    pub(crate) fn is_replay(&self) -> bool {
        #[cfg(feature = "selftest")]
        {
            matches!(self, Source::Replay(_))
        }
        #[cfg(not(feature = "selftest"))]
        {
            false
        }
    }

    pub(crate) fn is_live(&self) -> bool {
        match self {
            Source::Acp(session) => session.is_ready(),
            Source::Codex(session) => session.is_ready() && !session.is_ended(),
            Source::OpenCode(session) => session.is_ready() && !session.is_ended(),
            Source::Own(_) => true,
            _ => self.session().is_some_and(|s| !s.is_ended()),
        }
    }

    /// The own agent's session, when the tab runs one.
    pub(crate) fn own(&self) -> Option<&ide_agent::Session> {
        match self {
            Source::Own(session) => Some(session),
            _ => None,
        }
    }

    /// The ACP session, when the tab runs one.
    pub(crate) fn acp(&self) -> Option<&ide_acp::Session> {
        match self {
            Source::Acp(session) => Some(session),
            _ => None,
        }
    }

    pub(crate) fn acp_capabilities(&self) -> Option<ide_acp::Capabilities> {
        if let Some(session) = self.codex() {
            return Some(crate::codex::capabilities(&session.capabilities()));
        }
        if let Some(session) = self.opencode() {
            return Some(crate::opencode::capabilities(&session.capabilities()));
        }
        self.acp().map(ide_acp::Session::capabilities)
    }

    /// The OpenCode session, when the tab runs one over its server.
    pub(crate) fn opencode(&self) -> Option<&ide_opencode::Session> {
        match self {
            Source::OpenCode(session) => Some(session),
            _ => None,
        }
    }

    /// The Codex session, when the tab runs one over the app-server.
    pub(crate) fn codex(&self) -> Option<&ide_codex::Session> {
        match self {
            Source::Codex(session) => Some(session),
            _ => None,
        }
    }

    pub(crate) fn authenticate(&self, method: &str) -> Result<(), ide_acp::SessionError> {
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.authenticate(method))
    }

    pub(crate) fn set_option(&self, option: &str, value: &str) -> Result<(), ide_acp::SessionError> {
        if let Some(session) = self.opencode() {
            return session
                .set_option(option, value)
                .map_err(|e| ide_acp::SessionError::Agent(crate::opencode::explained(&e)));
        }
        if let Some(session) = self.codex() {
            return session
                .set_option(option, value)
                .map_err(|e| ide_acp::SessionError::Agent(crate::codex::explained(&e)));
        }
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.set_option(option, value))
    }

    pub(crate) fn set_acp_mode(&self, mode: &str) -> Result<(), ide_acp::SessionError> {
        if let Some(session) = self.opencode() {
            return session
                .set_mode(mode)
                .map_err(|e| ide_acp::SessionError::Agent(crate::opencode::explained(&e)));
        }
        if let Some(session) = self.codex() {
            return session
                .set_mode(mode)
                .map_err(|e| ide_acp::SessionError::Agent(crate::codex::explained(&e)));
        }
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.set_mode(mode))
    }

    pub(crate) fn answer_acp(&self, request: &str, choice: ide_acp::Choice) -> Result<(), ide_acp::SessionError> {
        if let Some(session) = self.opencode() {
            return session
                .answer(request, crate::opencode::choice_of(choice))
                .map_err(|e| ide_acp::SessionError::Agent(crate::opencode::explained(&e)));
        }
        if let Some(session) = self.codex() {
            return session
                .answer(request, crate::codex::choice(choice))
                .map_err(|e| ide_acp::SessionError::Agent(crate::codex::explained(&e)));
        }
        self.acp()
            .map_or(Err(ide_acp::SessionError::Closed), |s| s.answer(request, choice))
    }

    /// Sends `message` as a prompt; the CLI's id for it, which an ACP agent does not give.
    pub(crate) fn send(&self, message: UserMessage) -> Result<Option<String>, Explained> {
        match self {
            Source::Live(session) => session.send(message).map(Some).map_err(|e| Explained::of(&e)),
            Source::Acp(session) => session.send(message).map(|()| None).map_err(|e| Explained::of(&e)),
            Source::Own(session) => session.send(message).map(|()| None).map_err(|e| Explained::of(&e)),
            Source::Codex(session) => session
                .send(message)
                .map(|()| None)
                .map_err(|e| Explained(crate::codex::explained(&e))),
            Source::OpenCode(session) => session
                .send(message)
                .map(|()| None)
                .map_err(|e| Explained(crate::opencode::explained(&e))),
            _ => Err(Explained::of(&SessionError::Closed)),
        }
    }

    pub(crate) fn interrupt(&self) {
        if let Some(session) = self.session() {
            session.interrupt().then(|_| ());
        }
        if let Some(session) = self.acp() {
            session.cancel();
        }
        if let Some(session) = self.own() {
            session.cancel();
        }
        if let Some(session) = self.codex() {
            session.cancel();
        }
        if let Some(session) = self.opencode() {
            session.cancel();
        }
    }

    pub(crate) fn answer(&self, request: &str, answer: PermissionAnswer) -> Result<(), SessionError> {
        self.session()
            .map_or(Err(SessionError::Closed), |s| s.answer(request, answer))
    }

    /// Answers the questions of request `request`, whichever agent asked them; an MCP server's form of Codex takes
    /// the answers as its fields.
    pub(crate) fn answer_questions(&self, request: &str, answers: Vec<Answer>) -> Result<(), Explained> {
        if let Some(session) = self.codex() {
            let form: serde_json::Map<String, serde_json::Value> = answers
                .iter()
                .map(|a| (a.question.clone(), serde_json::Value::from(a.answer.clone())))
                .collect();
            return session
                .answer_questions(request, answers)
                .or_else(|_| session.answer_form(request, serde_json::Value::Object(form)))
                .map_err(|e| Explained(crate::codex::explained(&e)));
        }
        if let Some(session) = self.opencode() {
            return session
                .answer_questions(request, answers)
                .map_err(|e| Explained(crate::opencode::explained(&e)));
        }
        self.session()
            .map_or(Err(SessionError::Closed), |s| s.answer_questions(request, answers))
            .map_err(|e| Explained::of(&e))
    }

    /// Answers a plan; what a recording played for the self-test changed, which a session reports as an event.
    pub(crate) fn answer_plan(
        &mut self,
        request: &str,
        answer: PlanAnswer,
    ) -> Result<Vec<ide_thread::Change>, SessionError> {
        #[cfg(feature = "selftest")]
        if let Source::Replay(replay) = self {
            return Ok(replay.answer_plan(request, &answer));
        }
        self.session()
            .map_or(Err(SessionError::Closed), |s| s.answer_plan(request, answer))
            .map(|()| Vec::new())
    }

    pub(crate) fn stop_task(&self, task: &str) {
        if let Some(session) = self.session() {
            session.stop_task(task).then(|_| ());
        }
    }

    pub(crate) fn background(&self, call: &str) {
        if let Some(session) = self.session() {
            session.background(Some(call)).then(|_| ());
        }
    }

    /// Switches the session's permission mode; `refused` gets the CLI's reason when it says no.
    pub(crate) fn set_mode(&self, mode: &str, refused: impl FnOnce(String) + Send + 'static) {
        if let Some(session) = self.session() {
            session
                .set_permission_mode(ide_claude::PermissionMode::from(mode))
                .then(move |answer| {
                    if let Err(error) = answer {
                        refused(error.explain());
                    }
                });
        }
    }

    pub(crate) fn set_model(&self, model: &str) {
        if let Some(session) = self.session() {
            session.set_model(Some(model)).then(|_| ());
        }
    }

    pub(crate) fn set_effort(&self, level: &str) {
        if let Some(session) = self.session() {
            session.set_effort(ide_claude::Effort::from(level)).then(|_| ());
        }
    }

    /// Ends the process: input closed, then the supervisor's signals.
    pub(crate) fn end(&self) {
        if let Some(session) = self.session() {
            session.stop(Duration::from_secs(2));
        }
        if let Some(session) = self.acp() {
            session.stop(Duration::from_secs(2));
        }
        if let Some(session) = self.own() {
            session.stop();
        }
        if let Some(session) = self.codex() {
            session.stop(Duration::from_secs(2));
        }
        if let Some(session) = self.opencode() {
            session.stop(Duration::from_secs(2));
        }
    }
}

/// The `claude` executable on the login shell's PATH, else in the places its installers use.
pub(crate) fn find_claude(env: &[(String, String)]) -> Option<PathBuf> {
    #[cfg(feature = "selftest")]
    if let Some(program) = TEST_PROGRAM.lock_ignoring_poison().clone() {
        return Some(program);
    }
    tool::which("claude", tool::var(env, "PATH")).or_else(|| {
        let home = ide_core::paths::home();
        [
            home.join(".local/bin"),
            home.join(".claude/local"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ]
        .into_iter()
        .map(|dir| dir.join("claude"))
        .find(|candidate| tool::is_executable(candidate))
    })
}

/// What a session event means for the tab, beyond the thread changes.
pub(crate) fn problem_of(event: &Event) -> Option<Problem> {
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
        let dir = ide_core::testing::scratch("find-claude");
        let exe = dir.join("claude");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = vec![("PATH".to_string(), format!("/nonexistent:{}", dir.display()))];
        assert_eq!(find_claude(&env), Some(exe.clone()));
        let overridden = vec![
            ("PATH".to_string(), "/nonexistent".to_string()),
            ("PATH".to_string(), dir.display().to_string()),
        ];
        assert_eq!(
            find_claude(&overridden),
            Some(exe),
            "the last PATH of the list is the one a child sees"
        );
    }
}
