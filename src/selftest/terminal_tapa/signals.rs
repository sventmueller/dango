//! What a terminal's program asks of a user who looks elsewhere: notifications by OSC 9 and OSC 777 and progress by
//! OSC 9;4 from a terminal behind another tab, shown as the red dot and the ring on its tab and the bar on the Dock
//! icon, until the next prompt ends the progress and the keyboard in the terminal takes the dot away.

use term_session::event::{Progress, ProgressState};

use super::super::{check, helpers};
use super::{Run, bring_back, controller_of, inspect, stop, wait};
use crate::terminal::{Signals, Tone};
use crate::window::TabId;

/// A notification by OSC 9, one with a title by OSC 777 and progress at 40 % by OSC 9;4, then a program that waits
/// for a line, during which the progress lasts.
const SIGNALS: &str = "printf '\\033]9;tapa-osc9\\007\\033]777;notify;tapa-title;tapa-osc777\\007\\033]9;4;1;40\\007'; \
                       head -n 1 >/dev/null\r";

fn signals(run: &Run) -> Option<Signals> {
    crate::terminal::signals_of_view(run.id)
}

/// The notifications the terminal of `run` asked for after the first `before` of all of them.
fn heard_since(run: &Run, before: usize) -> Vec<crate::terminal::Heard> {
    crate::terminal::heard_notifications()
        .into_iter()
        .skip(before)
        .filter(|heard| heard.view == run.id)
        .collect()
}

/// Puts the AI tab `other` in front of the terminal and has its program send the notifications and the progress.
pub(super) fn behind(run: Run, other: TabId) {
    let (Some(controller), Some(view)) = (controller_of(&run), inspect::view(run.id)) else {
        return stop(&run, "the terminal and the folder's window are open");
    };
    controller.select_tab_by_id(other);
    let hidden = controller.selected_tab() == Some(other);
    let before = (
        crate::terminal::heard_notifications().len(),
        crate::terminal::notifications_posted(),
    );
    let prompts = view.prompts();
    // Behind the other tab the terminal's window is off screen, so the line goes to the shell directly, not as keys.
    view.with_session(|session| session.write(SIGNALS.as_bytes().to_vec()));
    let asked = run.clone();
    let reported = move || {
        heard_since(&asked, before.0).len() >= 2 && signals(&asked).is_some_and(|shown| shown.progress.is_some())
    };
    helpers::wait_within(std::time::Duration::from_secs(10), reported, move |_| {
        notified(&run, hidden, before);
        progressed(&run);
        if let Some(view) = inspect::view(run.id) {
            view.with_session(|session| session.write(b"\r".to_vec()));
        }
        ended(run, prompts);
    });
}

/// Both notifications were heard, posted while the app is not in front, and put the red dot on the terminal's tab.
fn notified(run: &Run, hidden: bool, before: (usize, usize)) {
    let heard = heard_since(run, before.0);
    let posted = crate::terminal::notifications_posted() - before.1;
    let shown = signals(run);
    let said = |title: Option<&str>, body: &str| {
        heard
            .iter()
            .any(|heard| heard.body == body && title.is_none_or(|title| heard.title == title) && heard.posted)
    };
    check(
        "OSC 9 and OSC 777 from a terminal behind another tab become notifications and put the red dot on its tab",
        hidden
            && said(None, "tapa-osc9")
            && said(Some("tapa-title"), "tapa-osc777")
            && posted >= 2
            && shown.as_ref().is_some_and(|shown| shown.waits && shown.red_dot),
        format!("behind {hidden}, {posted} posted, {heard:?}, tab {shown:?}"),
    );
}

/// The progress shows as the bar on the Dock icon, and on the terminal's tab as the ring unless the red dot of its
/// notification stands there, which comes first.
fn progressed(run: &Run) {
    let shown = signals(run);
    let dock = crate::terminal::dock_progress();
    let forty = Progress {
        state: ProgressState::Normal,
        percent: Some(40),
    };
    let at_forty = |ring: Option<crate::terminal::Ring>| {
        ring.is_some_and(|ring| ring.tone == Tone::Normal && ring.percent == Some(40) && ring.filled.is_some())
    };
    check(
        "OSC 9;4 shows the program's progress as a bar on the Dock icon and on the tab, where the red dot of a \
         notification comes before the ring",
        shown.as_ref().is_some_and(|shown| {
            shown.progress == Some(forty) && (at_forty(shown.ring) || (shown.red_dot && shown.ring.is_none()))
        }) && at_forty(dock),
        format!("tab {shown:?}, Dock {dock:?}"),
    );
}

/// The next prompt ends the progress on the tab and the Dock icon; back in front with the keyboard, the terminal's
/// tab loses its dot.
fn ended(run: Run, prompts: usize) {
    let over = move |view: &crate::terminal::TerminalView| {
        view.prompts() > prompts
            && crate::terminal::signals_of_view(view.id())
                .is_some_and(|shown| shown.progress.is_none() && shown.ring.is_none())
            && crate::terminal::dock_progress().is_none()
    };
    wait(&run.clone(), 10, over, move |over| {
        let after = signals(&run);
        let dock = crate::terminal::dock_progress();
        bring_back(&run);
        let seen = run.clone();
        helpers::wait_within(
            std::time::Duration::from_secs(5),
            move || signals(&seen).is_some_and(|shown| !shown.waits && !shown.red_dot),
            move |seen| {
                check(
                    "the progress ends with the next prompt, and the red dot goes once the terminal is in front with \
                     the keyboard",
                    over && seen,
                    format!("at the prompt {after:?}, Dock {dock:?}; in front {:?}", signals(&run)),
                );
                super::selection::smart(run);
            },
        );
    });
}
