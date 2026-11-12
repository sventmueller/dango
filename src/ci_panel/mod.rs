//! The CI panel (E12, E77, I8): the runs of HEAD on GitHub Actions or GitLab CI with their jobs and steps, logs in
//! tabs, re-running, starting and cancelling runs and jobs, a failed job's log for an agent, and the open merge
//! requests or pull requests and issues. It talks to the service only for folders the user
//! trusts (E22), since their remote decides where the app connects; tokens come from `ide_ci::Auth`, per host.

mod load;
mod log_tab;
mod login;
mod model;
mod ops;
mod panel;
mod refresh;
mod rows;
mod text;
mod ui;
mod work;

use std::path::Path;

pub use log_tab::LogTabs;
pub use login::CI_OAUTH_CLIENTS;
pub use ops::sheet;
pub use panel::CiPanels;
pub use work::CI_HOSTS;

use crate::window::WindowController;

/// Puts the CI panel of the window's folder in place.
pub fn attach(controller: &WindowController) {
    panel::CiPanel::attach(controller);
}

/// Something changed below `root`; the panel reads again when HEAD moved.
pub fn changed(root: &Path) {
    for panel in panel::for_root(root) {
        panel.changed();
    }
}

/// The window of `controller` closes: a login it waits for stops and lets the callback port go.
pub fn window_closed(controller: &WindowController) {
    if let Some(panel) = panel::of(controller) {
        panel.cancel_login();
    }
}

/// The user changed the CI settings, so every panel reads again.
pub fn settings_changed() {
    for panel in panel::all() {
        panel.refresh(true);
    }
}

/// The user trusts `root` now, so the panel may talk to its service.
pub fn trust_changed(root: &Path) {
    for panel in panel::for_root(root) {
        panel.refresh(true);
    }
}

/// What the self-test reads from and does with a window's CI panel, through the code its controls run.
#[cfg(feature = "selftest")]
pub mod selftest {
    use ide_ci::State;
    use objc2::rc::Retained;

    pub use super::model::Tab;
    use super::panel::{self, CiPanel};
    use crate::window::WindowController;

    /// A job with its state and number of steps.
    pub type JobSeen = (String, u64, State, usize);

    /// The panel's phase, account and runs.
    pub struct Seen {
        pub phase: String,
        pub forge: Option<String>,
        pub account: Option<String>,
        pub source: Option<String>,
        pub problem: Option<String>,
        pub runs: Vec<(String, u64, State, Vec<JobSeen>)>,
        pub rows: isize,
        pub tab: Tab,
        /// The open merge requests or issues by number and title.
        pub items: Vec<(u64, String)>,
        /// The note below the list, when it shows.
        pub note: Option<String>,
        pub workflows: Vec<String>,
        /// A re-run, start or cancel is on its way.
        pub busy: bool,
    }

    fn panel(controller: &WindowController) -> Option<Retained<CiPanel>> {
        panel::of(controller)
    }

    pub fn seen(controller: &WindowController) -> Option<Seen> {
        let panel = panel(controller)?;
        let state = panel.read();
        Some(Seen {
            phase: format!("{:?}", state.phase)
                .split([' ', '{', '('])
                .next()
                .unwrap_or("")
                .to_string(),
            forge: state.forge.as_ref().map(ToString::to_string),
            account: state.account.clone(),
            source: state.source.as_ref().map(super::text::source),
            problem: state.problem.clone(),
            runs: state
                .runs
                .iter()
                .map(|r| {
                    let jobs = r
                        .jobs
                        .iter()
                        .map(|j| (j.name.clone(), j.id, j.state, j.steps.len()))
                        .collect();
                    (r.run.name.clone(), r.run.id, r.run.state, jobs)
                })
                .collect(),
            rows: panel.ui().outline.numberOfRows(),
            tab: state.tab,
            items: state.items.iter().map(|i| (i.number, i.title.clone())).collect(),
            note: panel.ui().note(),
            workflows: state.workflows.iter().map(|w| w.file.clone()).collect(),
            busy: state.busy,
        })
    }

    /// Shows `tab`, as its segment does.
    pub fn choose_tab(controller: &WindowController, tab: Tab) {
        if let Some(panel) = panel(controller) {
            panel.choose_tab(tab);
        }
    }

    /// Re-runs every job of `run`, as its menu item does.
    pub fn rerun_all(controller: &WindowController, run: u64) {
        if let Some(panel) = panel(controller) {
            panel.rerun_all(panel.find_run(run));
        }
    }

    /// Starts the finished `job` again, as its menu item does.
    pub fn retry_job(controller: &WindowController, run: u64, job: u64) {
        if let Some(panel) = panel(controller) {
            panel.retry_job(panel.find_job(run, job));
        }
    }

    /// Starts the waiting `job`, as its menu item does.
    pub fn start_job(controller: &WindowController, run: u64, job: u64) {
        if let Some(panel) = panel(controller) {
            panel.start_job(panel.find_job(run, job));
        }
    }

    /// Asks to start a run for the branch, as the header's button does.
    pub fn start_run(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.start_run();
        }
    }

    /// Whether the header's start button can be pressed.
    pub fn can_start_run(controller: &WindowController) -> bool {
        panel(controller).is_some_and(|p| p.ui().buttons.start.isEnabled() && !p.ui().buttons.start.isHidden())
    }

    /// The titles of the context menu of `key`'s row and whether each can be chosen.
    pub fn menu_of_job(controller: &WindowController, run: u64, job: u64) -> Vec<(String, bool)> {
        panel(controller).map_or_else(Vec::new, |p| p.menu_titles(super::model::Key::Job(run, job)))
    }

    /// Re-runs the failed jobs of `run`, as its menu item does.
    pub fn rerun(controller: &WindowController, run: u64) {
        if let Some(panel) = panel(controller) {
            panel.rerun(panel.find_run(run));
        }
    }

    /// Asks to cancel `run`, as its menu item does.
    pub fn cancel(controller: &WindowController, run: u64) {
        if let Some(panel) = panel(controller) {
            panel.cancel(panel.find_run(run));
        }
    }

    /// Selects the row of `job` and presses Return in the list, through `keyDown:` as AppKit would.
    pub fn press_return_on(controller: &WindowController, run: u64, job: u64) {
        let Some(panel) = panel(controller) else { return };
        let outline = &panel.ui().outline;
        let item = panel.item_for(&super::model::Key::Job(run, job));
        // SAFETY: The panel hands this outline view the same item for that job's row.
        let Ok(row) = usize::try_from(unsafe { outline.rowForItem(Some(&item)) }) else {
            return;
        };
        outline.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        let event = objc2_app_kit::NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            objc2_app_kit::NSEventType::KeyDown,
            objc2_foundation::NSPoint::ZERO,
            objc2_app_kit::NSEventModifierFlags::empty(),
            0.0,
            controller.window().map_or(0, |w| w.windowNumber()),
            None,
            &crate::util::ns("\r"),
            &crate::util::ns("\r"),
            false,
            36,
        );
        if let Some(event) = event {
            // SAFETY: `keyDown:` takes the event, which lives for the call.
            let () = unsafe { objc2::msg_send![&**outline, keyDown: &*event] };
        }
    }

    /// Hands the failure of `job` to a new AI tab, as its menu item does.
    pub fn send_to_agent(controller: &WindowController, run: u64, job: u64) {
        if let Some(panel) = panel(controller) {
            panel.send_to_agent(panel.find_job(run, job));
        }
    }

    /// Asks for a token, as the panel's button does.
    pub fn sign_in(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.sign_in();
        }
    }

    /// Forgets the app's own token of the host, as the panel's button does.
    pub fn sign_out(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.sign_out();
        }
    }

    /// Starts signing in through the browser, as the panel's button does.
    pub fn browser_login(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.browser_login();
        }
    }

    /// Whether a login waits for the browser, with the code to type on GitHub.
    pub fn login(controller: &WindowController) -> Option<Option<String>> {
        let panel = panel(controller)?;
        let state = panel.read();
        state.login.as_ref().map(|login| login.code.clone())
    }

    /// The URLs the panel would have opened in the browser.
    pub fn opened() -> Vec<String> {
        super::login::opened()
    }

    /// Whether the panel's browser button can be pressed.
    pub fn browser_button(controller: &WindowController) -> Option<bool> {
        panel(controller)?.ui().browser_button()
    }

    /// The lines of the open log tab of `job`, and whether its find bar shows after ⌘F.
    pub fn log(controller: &WindowController, job: u64, find: bool) -> Option<(Vec<ide_ci::log::LogLine>, bool)> {
        super::log_tab::inspect(controller, job, find)
    }

    /// The text in the composer of the window's newest AI tab, then the text it takes along from outside.
    pub fn agent_draft(controller: &WindowController) -> Option<String> {
        crate::agent::of_window(controller).last().map(|tab| {
            let mut draft = tab.composer_text();
            for outside in tab.outside_texts() {
                draft.push('\n');
                draft.push_str(&outside);
            }
            draft
        })
    }

    /// Whether the window's newest AI tab takes text along from outside.
    pub fn agent_has_outside(controller: &WindowController) -> bool {
        crate::agent::of_window(controller)
            .last()
            .is_some_and(|tab| !tab.outside_texts().is_empty())
    }
}
