//! Codex's Remote Control in the AI tab (E148): the phone in the head turns it on through `codex remote-control
//! start`, after which the session moves to Codex's shared server, the one the phone reaches; its menu pairs a phone
//! and turns Remote Control off again. Remote Control is the shared server's, so it is on or off for every Codex
//! session of the Mac.

use std::rc::Weak;

use dispatch2::MainThreadBound;
use ide_codex::RemoteControl;
use ide_core::explain::Explained;
use ide_process::Supervisor;
use ide_ui_common::alert::Alert;
use ide_ui_common::util::main_soon;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSMenu, NSView};
use splat_i18n::{t, tf};

use crate::header::Remote;
use crate::tab::AgentTab;

/// The menu's item that pairs a phone.
const PAIR: isize = 10;
/// The menu's item that turns Remote Control off.
const END: isize = 11;

/// What the tab does about Codex's Remote Control that the session cannot say yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Turning {
    /// `codex remote-control start` runs.
    On,
    /// Remote Control is on; the session moves to the shared server once the agent is done.
    Moving,
    /// Turning it on or off failed, for this reason.
    Failed(String),
}

/// Lets `launch` join Codex's shared server when the user turned Remote Control on, starting that server unless it
/// runs; a server that does not start leaves the session a server of its own. This waits for Codex, so it runs off
/// the main thread.
pub(crate) fn share(launch: &mut ide_codex::Launch) {
    if !ide_codex::remote::wanted(&launch.env) {
        return;
    }
    match ide_codex::remote::ensure_daemon(Supervisor::global(), &launch.program, &launch.env) {
        Ok(()) => launch.shared = true,
        Err(error) => ide_ui_common::diag::report("starting Codex's shared server", error),
    }
}

impl AgentTab {
    /// The running Codex session over its own protocol, if the tab has one.
    pub(crate) fn codex_session(&self) -> Option<ide_codex::Session> {
        self.state.borrow().source().codex().filter(|s| !s.is_ended())
    }

    /// What the head's phone shows for Codex's `session`.
    pub(crate) fn codex_remote_shown(&self, session: &ide_codex::Session) -> Remote {
        match self.codex_remote.borrow().as_ref() {
            Some(Turning::On) => Remote::Connecting,
            Some(Turning::Moving) => Remote::Moving,
            Some(Turning::Failed(why)) => Remote::Failed(why.clone()),
            None => match session.remote_control() {
                RemoteControl::Off => Remote::Off,
                RemoteControl::Connecting => Remote::Connecting,
                RemoteControl::Connected(_) => Remote::Connected,
                RemoteControl::Failed => Remote::Failed(t("Codex hat keine Verbindung zu OpenAI").to_string()),
            },
        }
    }

    /// The phone in the head of a Codex tab: turns Remote Control on, or offers pairing and its end.
    pub(crate) fn codex_remote_click(&self, session: &ide_codex::Session, view: &NSView) {
        match self.codex_remote_shown(session) {
            Remote::Off | Remote::Failed(_) => self.codex_remote_on(session),
            Remote::Connected => crate::actions::pop_up(&self.codex_remote_items(), view),
            Remote::Hidden | Remote::Connecting | Remote::Moving => {}
        }
    }

    /// What the phone offers while Remote Control is on: pairing a phone and the end.
    pub(crate) fn codex_remote_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let action = sel!(ideAgentRemoteControlAction:);
        self.menu_item(&menu, t("Handy koppeln …"), action, PAIR);
        menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(self.mtm));
        self.menu_item(&menu, t("Remote Control beenden"), action, END);
        menu
    }

    /// An item of the phone's menu of a Codex tab.
    pub(crate) fn codex_remote_action(&self, session: &ide_codex::Session, action: isize) {
        match action {
            PAIR => self.codex_pair(session),
            END => self.codex_remote_off(session),
            _ => {}
        }
    }

    fn codex_remote_on(&self, session: &ide_codex::Session) {
        *self.codex_remote.borrow_mut() = Some(Turning::On);
        self.refresh_remote();
        let launch = session.launch().clone();
        let tab = self.bound();
        let spawned = ide_ui_common::util::spawn("codex-remote", move || {
            let started = ide_codex::remote::start(Supervisor::global(), &launch.program, &launch.env)
                .map(drop)
                .map_err(|e| Explained(super::explained(&e)));
            back(tab, move |tab| tab.codex_remote_started(started));
        });
        if let Err(why) = spawned {
            self.codex_remote_started(Err(why));
        }
    }

    /// The tab, for work on another thread that comes back to it on the main thread.
    fn bound(&self) -> MainThreadBound<Weak<AgentTab>> {
        MainThreadBound::new(self.me.borrow().clone(), self.mtm)
    }

    /// `codex remote-control start` has ended: a session of its own moves to the shared server once nothing runs.
    fn codex_remote_started(&self, started: Result<(), Explained>) {
        let moving = self.codex_session().is_some_and(|s| !s.launch().shared);
        *self.codex_remote.borrow_mut() = match started {
            Err(why) => Some(Turning::Failed(why.0)),
            Ok(()) if moving => Some(Turning::Moving),
            Ok(()) => None,
        };
        self.refresh_remote();
        self.codex_move_if_idle();
    }

    /// Moves a session waiting for it to Codex's shared server once the agent is done and asks nothing, continuing
    /// its thread there.
    pub(crate) fn codex_move_if_idle(&self) {
        if self.codex_remote.borrow().as_ref() != Some(&Turning::Moving) {
            return;
        }
        if self.state.borrow().thread().status() != ide_thread::Status::Idle {
            return;
        }
        *self.codex_remote.borrow_mut() = None;
        let source = self.state.borrow_mut().end();
        source.end();
        self.start();
    }

    /// The shared server said where Remote Control stands, which replaces a failure the tab showed.
    pub(crate) fn codex_remote_heard(&self) {
        let failed = matches!(self.codex_remote.borrow().as_ref(), Some(Turning::Failed(_)));
        if failed {
            *self.codex_remote.borrow_mut() = None;
        }
    }

    fn codex_remote_off(&self, session: &ide_codex::Session) {
        let Ok(reply) = session.disable_remote_control() else {
            return;
        };
        let tab = self.bound();
        reply.then(move |result| {
            let Err(error) = result else { return };
            back(tab, move |tab| {
                *tab.codex_remote.borrow_mut() = Some(Turning::Failed(super::explained(&error)));
                tab.refresh_remote();
            });
        });
    }

    /// Asks Codex for a pairing code and shows it, for the user to type into Codex on the phone.
    fn codex_pair(&self, session: &ide_codex::Session) {
        let launch = session.launch().clone();
        let tab = self.bound();
        let spawned = ide_ui_common::util::spawn("codex-pair", move || {
            let paired = ide_codex::remote::pair(Supervisor::global(), &launch.program, &launch.env);
            back(tab, move |tab| {
                let mtm = tab.mtm;
                let alert = match paired {
                    Ok(pairing) => Alert::new(mtm, t("Handy koppeln")).informative(&tf(
                        "Gib den Code {} in Codex auf dem Handy ein. Er gilt bis {}.",
                        &[&pairing.code, &crate::format::time_of_day(pairing.expires)],
                    )),
                    Err(error) => Alert::new(mtm, t("Koppeln fehlgeschlagen")).informative(&super::explained(&error)),
                };
                alert.run_modal();
            });
        });
        if let Err(why) = spawned {
            ide_ui_common::diag::report("pairing a phone with Codex", why.0);
        }
    }
}

/// Calls `then` with the tab `bound` holds on the main thread, unless the tab closed meanwhile.
fn back(bound: MainThreadBound<Weak<AgentTab>>, then: impl FnOnce(&AgentTab) + Send + 'static) {
    main_soon(move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(tab) = bound.into_inner(mtm).upgrade() {
            then(&tab);
        }
    });
}
