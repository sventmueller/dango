//! The CI panel (E12, E77, I8): the runs of HEAD on GitHub Actions or GitLab CI with their jobs and steps, logs in
//! tabs, re-running, starting and cancelling runs and jobs, a failed job's log for an agent, and the open merge
//! requests or pull requests and issues. It talks to the service only for folders the user
//! trusts (E22), since their remote decides where the app connects; tokens come from `ide_ci::Auth`, per host.

mod clock;
mod load;
mod log_tab;
mod login;
mod model {
    pub(crate) use ide_ci::panel::*;

    /// The panel's state, with a sign-in through the browser while one waits.
    pub(crate) type State = ide_ci::panel::State<super::login::Login>;
}
mod ops;
mod panel;
mod refresh;
mod rows;
mod text;
mod ui;
mod work;

use std::path::Path;

pub(crate) use load::{access, client, default_branch, remote_client};
pub(crate) use log_tab::{LogTabs, came_to_front};
pub(crate) use login::CI_OAUTH_CLIENTS;
pub(crate) use ops::{open_agent, open_url};
pub(crate) use panel::CiPanels;
pub(crate) use text::problem;
pub(crate) use work::run as on_lane;
pub(crate) use work::{CI_HOSTS, use_secrets};

use crate::context::AppContext;
use crate::window::WindowController;

/// Puts the CI panel of the window's folder in place.
pub(crate) fn attach(controller: &WindowController) {
    panel::CiPanel::attach(controller);
}

/// The client the CI panel of `controller`'s window found for its folder, once it read the service.
pub(crate) fn client_of(controller: &WindowController) -> Option<ide_ci::Client> {
    panel::of(controller).and_then(|p| p.read().client.clone())
}

/// The window shows its CI panel, or the panel's symbol again: a panel that waited for that starts reading.
pub(crate) fn shown(controller: &WindowController) {
    if let Some(panel) = panel::of(controller) {
        panel.wake();
    }
}

/// Something changed below `root`; the panel reads again when HEAD moved.
pub(crate) fn changed(root: &Path) {
    for panel in crate::context::app().ci_panels().panels.for_root(root) {
        panel.changed();
    }
}

/// The window of `controller` closes: a login it waits for stops and lets the callback port go, and its panel goes.
pub(crate) fn window_closed(controller: &WindowController) {
    if let Some(panel) = panel::of(controller) {
        panel.cancel_login();
    }
    controller.context().ci_panels().panels.detach(controller.handle());
}

/// The user changed the CI settings, so every panel reads again.
pub(crate) fn settings_changed() {
    for panel in crate::context::app().ci_panels().panels.live() {
        panel.refresh(true);
    }
}

/// The user trusts `root` now, so the panel may talk to its service.
pub(crate) fn trust_changed(cx: &AppContext, root: &Path) {
    for panel in cx.ci_panels().panels.for_root(root) {
        panel.refresh(true);
    }
}

/// What the self-test reads from and does with a window's CI panel, through the code its controls run.
#[cfg(feature = "selftest")]
pub(crate) mod selftest {
    use ide_ci::State;
    use objc2::rc::Retained;

    pub(crate) use super::model::{Key, Tab};
    use super::panel::{self, CiPanel};
    use crate::window::WindowController;

    /// A job with its state and number of steps.
    pub(crate) type JobSeen = (String, u64, State, usize);

    /// The panel's phase, account and runs.
    pub(crate) struct Seen {
        pub(crate) phase: String,
        pub(crate) forge: Option<String>,
        pub(crate) account: Option<String>,
        pub(crate) source: Option<String>,
        pub(crate) problem: Option<String>,
        pub(crate) runs: Vec<(String, u64, State, Vec<JobSeen>)>,
        pub(crate) rows: isize,
        pub(crate) tab: Tab,
        /// The open merge requests or issues by number and title.
        pub(crate) items: Vec<(u64, String)>,
        /// The note below the list, when it shows.
        pub(crate) note: Option<String>,
        pub(crate) workflows: Vec<String>,
        /// A re-run, start or cancel is on its way.
        pub(crate) busy: bool,
    }

    fn panel(controller: &WindowController) -> Option<Retained<CiPanel>> {
        panel::of(controller)
    }

    pub(crate) fn seen(controller: &WindowController) -> Option<Seen> {
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
    pub(crate) fn choose_tab(controller: &WindowController, tab: Tab) {
        if let Some(panel) = panel(controller) {
            panel.choose_tab(tab);
        }
    }

    /// Asks to start a run for the branch, as the header's button does.
    pub(crate) fn start_run(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.start_run();
        }
    }

    /// Whether the header's start button can be pressed.
    pub(crate) fn can_start_run(controller: &WindowController) -> bool {
        panel(controller).is_some_and(|p| p.ui().buttons.start.isEnabled() && !p.ui().buttons.start.isHidden())
    }

    /// The titles of the context menu of `key`'s row and whether each can be chosen.
    pub(crate) fn menu_of_job(controller: &WindowController, run: u64, job: u64) -> Vec<(String, bool)> {
        panel(controller).map_or_else(Vec::new, |p| p.menu_titles(super::model::Key::Job(run, job)))
    }

    /// The titles of the context menu of `run`'s row and whether each can be chosen.
    pub(crate) fn menu_of_run(controller: &WindowController, run: u64) -> Vec<(String, bool)> {
        panel(controller).map_or_else(Vec::new, |p| p.menu_titles(super::model::Key::Run(run)))
    }

    /// The titles of the context menu of the row of the merge request or issue `number`.
    pub(crate) fn menu_of_item(controller: &WindowController, number: u64) -> Vec<(String, bool)> {
        panel(controller).map_or_else(Vec::new, |p| p.menu_titles(super::model::Key::Item(number)))
    }

    /// Chooses `action` in the context menu of `key`'s row, as a click on its item does.
    pub(crate) fn choose_in_menu(controller: &WindowController, key: Key, action: objc2::runtime::Sel) {
        let (Some(panel), Some(mtm)) = (panel(controller), objc2::MainThreadMarker::new()) else {
            return;
        };
        panel.state().menu_keys = vec![key];
        let item = objc2_app_kit::NSMenuItem::new(mtm);
        // SAFETY: The panel's own action with a menu item as the sender, as AppKit sends it from the context menu.
        unsafe {
            objc2_app_kit::NSApplication::sharedApplication(mtm).sendAction_to_from(action, Some(&panel), Some(&item))
        };
    }

    /// The lines of the account button's menu and whether each can be chosen, separators left out.
    pub(crate) fn account_menu(controller: &WindowController) -> Vec<(String, bool)> {
        let Some(panel) = panel(controller) else {
            return Vec::new();
        };
        let menu = panel.ui().account_menu(&panel);
        menu.itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| (item.title().to_string(), item.isEnabled()))
            .collect()
    }

    /// Re-runs the failed jobs of `run`, as its menu item does.
    pub(crate) fn rerun(controller: &WindowController, run: u64) {
        if let Some(panel) = panel(controller) {
            panel.rerun(panel.find_run(run));
        }
    }

    /// Asks to cancel `run`, as its menu item does.
    pub(crate) fn cancel(controller: &WindowController, run: u64) {
        if let Some(panel) = panel(controller) {
            panel.cancel(panel.find_run(run));
        }
    }

    /// Selects the row of `job` and presses Return in the list, through `keyDown:` as AppKit would.
    pub(crate) fn press_return_on(controller: &WindowController, run: u64, job: u64) {
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
    pub(crate) fn send_to_agent(controller: &WindowController, run: u64, job: u64) {
        if let Some(panel) = panel(controller) {
            panel.send_to_agent(panel.find_job(run, job));
        }
    }

    /// Asks for a token, as the panel's button does.
    pub(crate) fn sign_in(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.sign_in();
        }
    }

    /// Forgets the app's own token of the host, as the panel's button does.
    pub(crate) fn sign_out(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.sign_out();
        }
    }

    /// Starts signing in through the browser, as the panel's button does.
    pub(crate) fn browser_login(controller: &WindowController) {
        if let Some(panel) = panel(controller) {
            panel.browser_login();
        }
    }

    /// Whether a login waits for the browser, with the code to type on GitHub.
    pub(crate) fn login(controller: &WindowController) -> Option<Option<String>> {
        let panel = panel(controller)?;
        let state = panel.read();
        state.login.as_ref().map(|login| login.code.clone())
    }

    /// The URLs the panel would have opened in the browser.
    pub(crate) fn opened() -> Vec<String> {
        super::login::opened()
    }

    /// Whether the panel's browser button can be pressed.
    pub(crate) fn browser_button(controller: &WindowController) -> Option<bool> {
        panel(controller)?.ui().browser_button()
    }

    /// The lines of the open log tab of `job`, and whether its find bar shows after ⌘F.
    pub(crate) fn log(
        controller: &WindowController,
        job: u64,
        find: bool,
    ) -> Option<(Vec<ide_ci::log::LogLine>, bool)> {
        super::log_tab::inspect(controller, job, find)
    }

    /// The text in the composer of the window's newest AI tab, then the text it takes along from outside.
    pub(crate) fn agent_draft(controller: &WindowController) -> Option<String> {
        ide_ui_agent::of_window(controller).last().map(|tab| {
            let mut draft = tab.composer_text();
            for outside in tab.outside_texts() {
                draft.push('\n');
                draft.push_str(&outside);
            }
            draft
        })
    }

    /// Whether the window's newest AI tab takes text along from outside.
    pub(crate) fn agent_has_outside(controller: &WindowController) -> bool {
        ide_ui_agent::of_window(controller)
            .last()
            .is_some_and(|tab| !tab.outside_texts().is_empty())
    }
}
