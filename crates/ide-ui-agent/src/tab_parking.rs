//! Releases an idle Claude CLI while retaining its conversation and composer in the tab.

use std::cell::Cell;
use std::time::{Duration, Instant};

use dispatch2::MainThreadBound;
use ide_process::{ProcessId, Supervisor};
use ide_ui_common::util::{main_after, main_soon};
use objc2::MainThreadMarker;

use crate::tab::AgentTab;

const CHECK_AFTER: Duration = Duration::from_secs(30);
const PARK_AFTER: Duration = Duration::from_secs(180);
const WAKE_CHECK: Duration = Duration::from_millis(100);

#[derive(Default)]
pub(super) struct Parking {
    pub(super) hidden_since: Cell<Option<Instant>>,
    pub(super) process: Cell<Option<ProcessId>>,
    watching: Cell<bool>,
    waking: Cell<bool>,
}

impl AgentTab {
    pub(super) fn watch_for_parking(&self) {
        if self.parking.watching.replace(true) {
            return;
        }
        main_after(
            CHECK_AFTER,
            self.parking_callback(|tab| {
                tab.parking.watching.set(false);
                tab.consider_parking();
            }),
        );
    }

    fn parking_callback(&self, then: impl FnOnce(&AgentTab) + Send + 'static) -> impl FnOnce() + Send + 'static {
        let tab = MainThreadBound::new(self.me.borrow().clone(), self.mtm);
        move || {
            if let Some(tab) = MainThreadMarker::new().and_then(|mtm| tab.get(mtm).upgrade()) {
                then(&tab);
            }
        }
    }

    fn consider_parking(&self) {
        if self
            .state
            .borrow()
            .source()
            .session()
            .is_none_or(|session| session.is_ended())
        {
            self.parking.hidden_since.take();
            return;
        }
        self.watch_for_parking();
        let Some(session) = self.parkable_session() else {
            self.parking.hidden_since.take();
            return;
        };
        let since = self.parking.hidden_since.get().unwrap_or_else(Instant::now);
        self.parking.hidden_since.set(Some(since));
        if since.elapsed() < PARK_AFTER {
            return;
        }
        let process = session.process_id();
        let done = self.parking_callback(move |tab| tab.park_claude(process, since));
        let _ = ide_core::lane::WORK.ask(
            move || session.has_process_work(),
            move |busy| {
                if busy == Some(false) {
                    main_soon(done);
                }
            },
        );
    }

    fn parkable_session(&self) -> Option<ide_claude::Session> {
        if self.is_shown() || self.dialog.borrow().is_some() {
            return None;
        }
        let state = self.state.borrow();
        if state.running() || !state.queue().is_empty() || state.fork().is_some() || state.remote() == Some(true) {
            return None;
        }
        state.source().tab_session()?.parkable_claude()
    }

    fn park_claude(&self, process: ProcessId, since: Instant) {
        if self.parking.hidden_since.get() != Some(since)
            || !self
                .parkable_session()
                .is_some_and(|session| session.process_id() == process)
        {
            return;
        }
        self.persist();
        self.parking.process.set(Some(process));
        let source = self.state.borrow_mut().end();
        source.end();
        self.refresh();
    }

    /// Keeps the queued prompt until the old process has exited, then uses the usual Claude resume path.
    pub(super) fn resume_parked(&self) {
        let Some(process) = self.parking.process.get() else {
            return;
        };
        if self.state.borrow().queue().is_empty() {
            return;
        }
        if Supervisor::global().wait(process, Duration::ZERO).is_some() {
            self.start();
        } else if !self.parking.waking.replace(true) {
            main_after(
                WAKE_CHECK,
                self.parking_callback(|tab| {
                    tab.parking.waking.set(false);
                    tab.resume_parked();
                }),
            );
        }
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
