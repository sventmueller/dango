//! How an AI tab's session runs: starting it, the events that come from it, and what changes of its status set off.

use ide_core::LockIgnoringPoison;
use ide_ui_common::window::{Notices, Trust};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

use ide_session::{AgentPath, Contract, Launch, LaunchError, UserChoices};
use ide_thread::{Change, Status, Thread};
use splat_i18n::tf;

use super::driven::Driven;
use super::notify;
use super::rewind::Fork;
use super::source::{self, Problem, Source};
use super::tab::{AgentTab, Incoming};
use crate::AgentKey;
use ide_ui_common::util::{main_after, main_soon};

impl AgentTab {
    /// Looks for `claude` and the login shell's environment on a worker thread, then starts the session.
    pub(crate) fn start(&self) {
        if self.is_driven() {
            return;
        }
        let Some(controller) = self.controller() else { return };
        if !controller.is_trusted() {
            self.set_problem(Some(Problem::Untrusted));
            return;
        }
        if self.agent.is_own() {
            self.state.borrow_mut().start();
            self.refresh();
            self.start_own();
            return;
        }
        if let Some(agent) = self.acp_agent() {
            let thread = {
                let mut state = self.state.borrow_mut();
                state.start();
                state.thread().as_ref().clone()
            };
            self.refresh();
            self.start_acp(agent, thread);
            return;
        }
        if self.state.borrow().source().tab_session().is_some() {
            let old = self.state.borrow_mut().end();
            old.end();
        }
        let transcript = {
            let mut state = self.state.borrow_mut();
            state.start();
            state.resume().and_then(|r| r.transcript.clone())
        };
        self.refresh();
        let key = self.key;
        let thread = self.snapshot();
        if let Err(why) = ide_ui_common::util::spawn("claude-start", move || {
            let env = ide_ui_common::window::environment(Duration::from_secs(15));
            let exe = source::find_claude(&env);
            let git = crate::host::host().git();
            let thread = match transcript {
                Some(path) if thread.entries(&ide_thread::Lane::Main).is_empty() => {
                    ide_claude::history::rebuild(thread.clone(), &path).unwrap_or(thread)
                }
                _ => thread,
            };
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.launch(exe, env, thread, git);
                }
            });
        }) {
            self.start_failed(Problem::Start(why.0));
        }
    }

    /// Starts the tab's Claude session in the app's registry, which takes the checkpoint before each message with
    /// `git` (I6).
    pub(super) fn launch(
        &self,
        exe: Option<PathBuf>,
        env: Vec<(String, String)>,
        thread: Thread,
        git: Option<ide_git::Git>,
    ) {
        let Some(exe) = exe else {
            self.start_failed(Problem::NotFound);
            return;
        };
        let Some(sessions) = self.sessions.clone() else {
            self.start_failed(Problem::Start(String::new()));
            return;
        };
        let launch = self.claude_launch(exe, env, thread, git);
        match Driven::start(&sessions, launch) {
            Ok((driven, first)) => {
                self.state.borrow_mut().started(Source::Session(driven));
                if let Some(first) = first {
                    self.absorb(first.thread, &[Change::Cleared]);
                }
                self.send_queued();
            }
            Err(error) => {
                let problem = match error {
                    LaunchError::NotFound(_) => Problem::NotFound,
                    LaunchError::Start(why) => Problem::Start(why),
                    other => Problem::Start(other.to_string()),
                };
                self.start_failed(problem);
            }
        }
    }

    /// The launch of the tab's Claude session with `exe` in `env` on `thread`, as the user's picks, the sandbox
    /// switch, Remote Control and a rewind set it.
    fn claude_launch(
        &self,
        exe: PathBuf,
        env: Vec<(String, String)>,
        thread: Thread,
        git: Option<ide_git::Git>,
    ) -> Launch {
        let state = self.state.borrow();
        let picks = state.picks();
        let (resume, rewind) = match state.fork() {
            Some(Fork {
                session,
                rewind: Some(rewind),
                ..
            }) => (Some(session.clone()), Some(rewind.clone())),
            Some(Fork { rewind: None, .. }) => (None, None),
            None => (
                (thread.session().map(str::to_string)).or_else(|| state.resume().map(|r| r.session.clone())),
                None,
            ),
        };
        let path = AgentPath::Claude {
            program: exe,
            remote_control: state.remote(),
            rewind,
        };
        let contract = Contract::User(UserChoices {
            mode: Some(picks.mode().to_string()),
            sandbox: state.sandbox(),
            shared: false,
        });
        let mut launch = Launch::new(path, &self.cwd, env, contract);
        launch.model = picks.model().map(str::to_string);
        launch.effort = picks.effort().map(str::to_string);
        launch.resume = resume;
        launch.thread = Some(thread);
        launch.git = git;
        launch
    }

    /// Takes in what the session reported since the last time.
    pub(super) fn flush(&self) {
        let inbox = self.inbox.borrow().clone();
        inbox.scheduled.store(false, Ordering::SeqCst);
        let pending = std::mem::take(&mut *inbox.events.lock_ignoring_poison());
        if pending.is_empty() {
            return;
        }
        let changes: Vec<Change> = pending.changes.into_iter().collect();
        let mut problem = None;
        let mut learned = false;
        let mut ended = None;
        for event in pending.events {
            match &event {
                Incoming::Acp(ide_acp::Event::Changed(c))
                | Incoming::Own(ide_agent::Event::Changed(c))
                | Incoming::OpenCode(ide_opencode::Event::Changed(c)) => {
                    debug_assert!(c.is_empty(), "invalidations are coalesced before scheduling");
                }
                Incoming::Own(other) => self.own_event(other),
                Incoming::Acp(ide_acp::Event::Capabilities) | Incoming::OpenCode(ide_opencode::Event::Capabilities) => {
                    learned = true
                }
                Incoming::OpenCode(ide_opencode::Event::Ended { stderr }) => ended = Some(stderr.clone()),
                Incoming::OpenCode(ide_opencode::Event::McpChanged(_)) => {}
                Incoming::OpenCode(other) => problem = self.opencode_problem(other).or(problem),
                Incoming::Acp(ide_acp::Event::Failed(error)) => problem = self.acp_problem(error),
                Incoming::Acp(ide_acp::Event::Ended { stderr }) => ended = Some(stderr.clone()),
            }
        }
        let snapshot = {
            let mut state = self.state.borrow_mut();
            if learned {
                state.learn();
            }
            state.source().snapshot()
        };
        if learned && self.agent.is_acp() {
            self.acp_apply_choice();
            self.send_queued();
        }
        if problem.is_some() {
            self.set_problem(problem);
        }
        if let Some(snapshot) = snapshot {
            self.absorb(snapshot, &changes);
            super::dialogs::requests_changed(self);
        }
        if learned {
            self.refresh();
            super::dialogs::session_ready(self);
        }
        if let Some(stderr) = ended {
            self.acp_ended(&stderr);
        }
    }

    /// What an ACP session's failure means for the tab; failures of a running session's requests are only logged.
    fn acp_problem(&self, error: &ide_acp::SessionError) -> Option<Problem> {
        let ready = self.state.borrow().source().is_live();
        let problem = acp_problem_of(self.agent.name(), ready, error);
        if problem.is_none() {
            ide_ui_common::diag::report("a request to the agent", error);
        }
        problem
    }

    pub(super) fn status_moved(&self, before: Status, now: Status) {
        let title = self.title();
        let elsewhere = notify::user_looks_elsewhere(self.is_shown());
        match now {
            Status::Idle if before != Status::Idle => {
                if let Some(controller) = self.controller() {
                    crate::host::host().turn_ended(controller.id(), &self.cwd);
                }
                let turn = self.state.borrow_mut().turn_ended();
                let driver_records = self.state.borrow().source().tab_session().is_some();
                if let Some(checkpoint) = turn.filter(|_| !driver_records) {
                    let key = self.key;
                    let started = checkpoint.commit.clone();
                    self.store.finish(checkpoint, move |finished| {
                        main_soon(move || {
                            if let Some(tab) = super::get(key) {
                                tab.state.borrow_mut().checkpoint_finished(&started, &finished);
                            }
                        });
                    });
                }
                self.send_queued();
            }
            Status::RequiresAction if elsewhere => {
                notify::post(
                    &self.tab_key(),
                    &title,
                    &tf("{} wartet auf deine Antwort.", &[self.agent.name()]),
                );
            }
            _ => {}
        }
        self.refresh_tab_mark();
    }

    /// Tells the user who looks elsewhere that the session finished, once its turn and every subagent have ended.
    pub(super) fn work_moved(&self, busy_before: bool, busy_now: bool) {
        if busy_before && !busy_now && notify::user_looks_elsewhere(self.is_shown()) {
            notify::post(
                &self.tab_key(),
                &self.title(),
                &tf("{} ist fertig.", &[self.agent.name()]),
            );
        }
    }

    /// Advances running times every second while anything runs.
    pub(super) fn start_ticking(&self) {
        if self.ticking.replace(true) {
            return;
        }
        tick(self.key);
    }
}

/// What the failure `error` of the ACP agent `name` means for its tab, with `ready` once the session runs; `None` for
/// a failed request of a running session.
fn acp_problem_of(name: &str, ready: bool, error: &ide_acp::SessionError) -> Option<Problem> {
    match error {
        ide_acp::SessionError::SignIn => Some(Problem::AcpSignIn),
        ide_acp::SessionError::Version(v) => Some(Problem::Start(tf(
            "{} spricht die Protokollversion {}, diese App Version 1",
            &[name, &v.to_string()],
        ))),
        other if !ready => Some(Problem::Start(ide_core::explain::Explain::explain(other))),
        _ => None,
    }
}

/// One second of running time for the tab `key`, again while anything runs.
fn tick(key: AgentKey) {
    let Some(tab) = super::get(key) else { return };
    let live = tab.timeline.tick(SystemTime::now());
    let running = tab.status() != Status::Idle;
    if running && let Some(session) = tab.session() {
        super::usage::fetch(&session);
    }
    if live || running {
        main_after(Duration::from_secs(1), move || tick(key));
    } else {
        tab.ticking.set(false);
    }
    if running && let Some(controller) = tab.controller() {
        controller.refresh_activity();
    }
}

#[cfg(test)]
mod tests {
    use ide_acp::SessionError;
    use ide_thread::{BackgroundItem, BackgroundKind, OutputTail, SubagentPatch, TaskStatus, Thread, ThreadId, Update};

    use super::*;
    use crate::tab::delegates;

    #[test]
    fn a_running_subagent_or_workflow_keeps_the_session_working_but_a_background_shell_does_not() {
        let mut thread = Thread::new(ThreadId::from("t"), "claude", "/work");
        let item = |kind| BackgroundItem {
            id: format!("{kind:?}"),
            kind,
            label: "work".into(),
            status: TaskStatus::Running,
            started: None,
            ended: None,
            tool_call: None,
            parent: None,
            output: OutputTail::default(),
            is_backgrounded: true,
            ambient: false,
        };
        thread.apply(Update::Background(vec![item(BackgroundKind::Shell)]));
        assert!(!delegates(&thread));
        thread.apply(Update::Background(vec![item(BackgroundKind::Workflow)]));
        assert!(delegates(&thread));
        thread.apply(Update::Background(Vec::new()));
        let mut sub = SubagentPatch::new("toolu_1", SystemTime::UNIX_EPOCH);
        sub.status = Some(TaskStatus::Running);
        thread.apply(Update::Subagent(sub));
        assert!(delegates(&thread));
        let mut done = SubagentPatch::new("toolu_1", SystemTime::UNIX_EPOCH);
        done.status = Some(TaskStatus::Completed);
        thread.apply(Update::Subagent(done));
        assert!(!delegates(&thread));
    }

    #[test]
    fn a_sign_in_or_a_foreign_protocol_stops_the_tab_even_while_running() {
        assert_eq!(
            acp_problem_of("Gemini", true, &SessionError::SignIn),
            Some(Problem::AcpSignIn)
        );
        let Some(Problem::Start(why)) = acp_problem_of("Gemini", true, &SessionError::Version(2)) else {
            panic!("a version mismatch is a start problem");
        };
        assert!(why.contains("Gemini") && why.contains('2'), "{why}");
    }

    #[test]
    fn other_failures_stop_only_a_tab_that_is_still_starting() {
        let error = SessionError::Agent("boom".into());
        assert_eq!(
            acp_problem_of("Gemini", false, &error),
            Some(Problem::Start("boom".into()))
        );
        assert_eq!(acp_problem_of("Gemini", true, &error), None);
        assert_eq!(acp_problem_of("Gemini", true, &SessionError::Closed), None);
    }
}
