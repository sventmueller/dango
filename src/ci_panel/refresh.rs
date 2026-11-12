//! The CI panel's controller for what it shows: attaching it to a window, reading the service on the CI lane when the
//! user, a timer or a moved HEAD asks, handing the result to the model and bringing the views in line.

use std::time::Duration;

use ide_ci::State as RunState;
use objc2::MainThreadOnly;

use super::load::{self, Loaded};
use super::model::{FIRST_WAIT, Phase, Refresh, Tab};
use super::panel::{CiPanel, with_panel, with_panels};
use super::{text, work};
use crate::util::main_after;
use crate::window::{Panel, WindowController};

/// How long changes in `.git` gather before HEAD is read again.
const HEAD_DEBOUNCE: Duration = Duration::from_millis(1500);

impl CiPanel {
    /// Builds the panel for `controller`'s folder, puts it on the CI page and starts reading.
    pub fn attach(controller: &WindowController) {
        let Some(root) = controller.project_root() else { return };
        let root = root.canonicalize().unwrap_or(root);
        with_panels(|panels| {
            panels
                .borrow_mut()
                .retain(|p| p.controller().is_some_and(|c| !std::ptr::eq(&*c, controller)));
        });
        let panel = CiPanel::new(controller.mtm(), controller, root);
        controller.set_panel_page(Panel::Ci, &panel.ui().page);
        controller.set_panel_accessory(Panel::Ci, &panel.ui().header);
        with_panels(|panels| panels.borrow_mut().push(panel.clone()));
        panel.refresh(true);
    }

    /// Reads the service again now, or right after the read that runs; `fresh` starts the wait for running runs over.
    pub fn refresh(&self, fresh: bool) {
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        let refresh = self.state().refresh(trusted, fresh);
        let (known, tab) = match refresh {
            Refresh::Untrusted => return self.show(),
            Refresh::Queued => return,
            Refresh::Read { known, tab } => (known, tab),
        };
        let root = self.root().to_path_buf();
        let id = self.id();
        let queued = work::run(
            move || load::load(&root, known, tab),
            move |loaded| with_panel(id, |p| p.loaded(loaded)),
        );
        if !queued {
            self.state().refresh_not_started();
            self.plan(FIRST_WAIT);
        }
    }

    fn loaded(&self, loaded: Option<Loaded>) {
        let again = self.state().refresh_finished();
        let loaded = loaded.unwrap_or(Loaded::Failed(ide_ci::Error::Decode("panic".into()), None));
        let wait = self.state().apply(loaded);
        self.show();
        super::log_tab::refresh_for(self.root(), &self.read().runs);
        if again {
            self.refresh(false);
        } else if let Some(wait) = wait {
            self.plan(wait);
        }
    }

    /// Reads again after `wait`, unless another read was planned or started since.
    pub fn plan(&self, wait: Duration) {
        let planned = self.state().plan();
        let id = self.id();
        main_after(wait, move || {
            with_panel(id, |panel| {
                if panel.read().is_planned(planned) {
                    panel.refresh(false);
                }
            });
        });
    }

    /// Something in the folder changed; reads again when HEAD moved.
    pub fn changed(&self) {
        if self.state().changed() {
            let id = self.id();
            main_after(HEAD_DEBOUNCE, move || with_panel(id, CiPanel::check_head));
        }
    }

    fn check_head(&self) {
        let root = self.root().to_path_buf();
        let id = self.id();
        let queued = work::run(
            move || load::head(&root).ok().map(|(_, head)| head),
            move |head| {
                with_panel(id, |panel| {
                    let moved = panel.state().head_read(head.flatten());
                    if moved {
                        panel.refresh(true);
                    }
                });
            },
        );
        if !queued {
            self.state().head_check_failed();
        }
    }

    /// Puts every view in line with the state and the activity bar's dot with the runs.
    pub fn show(&self) {
        let phase = self.read().phase.clone();
        self.ui().show_phase(self, &phase);
        if phase == Phase::Ready {
            self.ui().show_ready(self);
            self.reload_outline();
        }
        self.ui().show_problem(self.read().problem.as_deref());
        let summary = RunState::summary(self.read().runs.iter().map(|r| r.run.state));
        if let Some(controller) = self.controller() {
            match summary.filter(|_| phase == Phase::Ready) {
                Some(state) => controller.set_symbol_status(Panel::Ci, Some(text::word(state))),
                None => controller.set_symbol_status(Panel::Ci, None),
            }
        }
    }

    pub fn set_problem(&self, problem: Option<String>) {
        self.state().set_problem(problem);
        self.ui().show_problem(self.read().problem.as_deref());
    }

    /// Shows `tab` and reads what it lists.
    pub fn choose_tab(&self, tab: Tab) {
        if self.state().choose_tab(tab) {
            self.show();
            self.refresh(true);
        }
    }
}
