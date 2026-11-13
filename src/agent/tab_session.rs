//! How an AI tab's session runs: starting it, the events that come from it, and what changes of its status set off.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

use ide_claude::{Event, Launch, Session, SessionError};
use ide_process::Supervisor;
use ide_thread::{Change, Status, Thread};
use splat_i18n::tf;

use super::notify;
use super::rewind::Fork;
use super::source::{self, Problem, Source};
use super::tab::{AgentTab, Incoming};
use crate::util::{main_after, main_soon};

impl AgentTab {
    /// Looks for `claude` and the login shell's environment on a worker thread, then starts the session.
    pub fn start(&self) {
        let Some(controller) = self.controller() else { return };
        if !controller.is_trusted() {
            self.set_problem(Some(Problem::Untrusted));
            return;
        }
        if self.agent.is_own() {
            {
                let mut state = self.state.borrow_mut();
                state.source = Source::Starting;
                state.problem = None;
            }
            self.refresh();
            self.start_own();
            return;
        }
        if let Some(agent) = self.acp_agent() {
            let thread = {
                let mut state = self.state.borrow_mut();
                state.source = Source::Starting;
                state.problem = None;
                state.choice_applied = false;
                state.thread.as_ref().clone()
            };
            self.refresh();
            self.start_acp(agent, thread);
            return;
        }
        let transcript = {
            let mut state = self.state.borrow_mut();
            state.source = Source::Starting;
            state.problem = None;
            state.resume.as_ref().and_then(|r| r.transcript.clone())
        };
        self.refresh();
        let key = self.key;
        let thread = self.snapshot();
        let _ = std::thread::Builder::new().name("claude-start".into()).spawn(move || {
            let env = crate::environment::wait(Duration::from_secs(15))
                .map(<[(String, String)]>::to_vec)
                .unwrap_or_default();
            let exe = source::find_claude(&env);
            let thread = match transcript {
                Some(path) if thread.entries(&ide_thread::Lane::Main).is_empty() => {
                    ide_claude::history::rebuild(thread.clone(), &path).unwrap_or(thread)
                }
                _ => thread,
            };
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.launch(exe, env, thread);
                }
            });
        });
    }

    pub(super) fn launch(&self, exe: Option<PathBuf>, env: Vec<(String, String)>, thread: Thread) {
        let Some(exe) = exe else {
            self.state.borrow_mut().source = Source::Idle;
            self.set_problem(Some(Problem::NotFound));
            return;
        };
        let mut launch = Launch::new(exe, env, &self.cwd);
        {
            let state = self.state.borrow();
            launch.resume = thread
                .session()
                .map(str::to_string)
                .or_else(|| state.resume.as_ref().map(|r| r.session.clone()));
            launch.sandbox = state.sandbox;
            launch.remote_control = state.remote;
            launch.permission_mode = Some(state.mode.clone());
            launch.allow_bypass = true;
            launch.model.clone_from(&state.model);
            launch.effort.clone_from(&state.effort);
            launch.file_checkpoints = true;
            match &state.fork {
                Some(Fork {
                    session,
                    rewind: Some(rewind),
                    ..
                }) => launch.rewind(session, rewind),
                Some(Fork { rewind: None, .. }) => launch.resume = None,
                None => {}
            }
        }
        let sink = self.sink(Incoming::Claude);
        let started = Session::start(Supervisor::global(), &launch, thread.clone(), sink);
        match started {
            Ok(session) => {
                {
                    let mut state = self.state.borrow_mut();
                    state.source = Source::Live(session);
                    state.problem = None;
                }
                self.absorb(thread, &[Change::Cleared]);
                self.send_queued();
            }
            Err(error) => {
                self.state.borrow_mut().source = Source::Idle;
                let problem = match error {
                    ide_claude::SessionError::NotFound(_) => Problem::NotFound,
                    other => Problem::Start(other.to_string()),
                };
                self.set_problem(Some(problem));
            }
        }
    }

    /// Takes in what the session reported since the last time.
    pub(super) fn flush(&self) {
        let inbox = self.inbox.borrow().clone();
        inbox.scheduled.store(false, Ordering::SeqCst);
        let pending = std::mem::take(&mut *inbox.events.lock().unwrap_or_else(|e| e.into_inner()));
        if pending.is_empty() {
            return;
        }
        let changes: Vec<Change> = pending.changes.into_iter().collect();
        let mut problem = None;
        let mut learned = false;
        let mut remote = false;
        let mut refused = None;
        let mut ended = None;
        for event in pending.events {
            match &event {
                Incoming::Claude(Event::Changed(c))
                | Incoming::Acp(ide_acp::Event::Changed(c))
                | Incoming::Own(ide_agent::Event::Changed(c)) => {
                    debug_assert!(c.is_empty(), "invalidations are coalesced before scheduling");
                }
                Incoming::Own(other) => self.own_event(other),
                Incoming::Claude(Event::Capabilities) | Incoming::Acp(ide_acp::Event::Capabilities) => learned = true,
                Incoming::Claude(Event::RemoteControl) => remote = true,
                Incoming::Claude(Event::Failed(SessionError::RewindRejected(why))) => refused = Some(why.clone()),
                Incoming::Claude(event @ Event::Failed(_)) => problem = source::problem_of(event),
                Incoming::Acp(ide_acp::Event::Failed(error)) => problem = self.acp_problem(error),
                Incoming::Acp(ide_acp::Event::Ended { stderr }) => ended = Some(stderr.clone()),
                Incoming::Claude(Event::Ended { .. }) => {}
            }
        }
        if let Some(why) = refused {
            self.rewind_refused(&why);
            return;
        }
        let snapshot = {
            let mut state = self.state.borrow_mut();
            if learned && let Some(caps) = state.source.capabilities() {
                state.caps = caps;
            }
            if learned && let Some(caps) = state.source.acp_capabilities() {
                state.acp = Some(caps);
            }
            state.source.snapshot()
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
        }
        if learned {
            self.refresh();
        } else if remote {
            self.refresh_remote();
        }
        if let Some(stderr) = ended {
            self.acp_ended(&stderr);
        }
    }

    /// What an ACP session's failure means for the tab; failures of a running session's requests are only logged.
    fn acp_problem(&self, error: &ide_acp::SessionError) -> Option<Problem> {
        let ready = self.state.borrow().source.is_live();
        match error {
            ide_acp::SessionError::SignIn => Some(Problem::AcpSignIn),
            ide_acp::SessionError::Version(v) => Some(Problem::Start(tf(
                "{} spricht die Protokollversion {}, diese App Version 1",
                &[self.agent.name(), &v.to_string()],
            ))),
            other if !ready => Some(Problem::Start(other.to_string())),
            other => {
                crate::diag::report("a request to the agent", other);
                None
            }
        }
    }

    pub(super) fn status_moved(&self, before: Status, now: Status) {
        let title = self.title();
        let elsewhere = notify::user_looks_elsewhere(self.is_shown());
        match now {
            Status::Idle if before != Status::Idle => {
                if let Some(controller) = self.controller() {
                    crate::worktrees::turn_ended(&controller, &self.cwd);
                }
                let turn = self.state.borrow_mut().turn.take();
                if let Some(checkpoint) = turn {
                    let key = self.key;
                    let started = checkpoint.commit.clone();
                    self.store.finish(checkpoint, move |finished| {
                        main_soon(move || {
                            if let Some(tab) = super::get(key) {
                                for kept in tab.state.borrow_mut().checkpoints.values_mut() {
                                    if kept.commit == started {
                                        *kept = finished.clone();
                                    }
                                }
                            }
                        });
                    });
                }
                if elsewhere {
                    notify::post(&self.tab_key(), &title, &tf("{} ist fertig.", &[self.agent.name()]));
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

    /// Advances running times every second while anything runs.
    pub(super) fn start_ticking(&self) {
        if self.ticking.replace(true) {
            return;
        }
        tick(self.key);
    }
}

/// One second of running time for the tab `key`, again while anything runs.
fn tick(key: u64) {
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
        crate::activity::refresh(&controller);
    }
}
