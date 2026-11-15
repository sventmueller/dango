//! What the self-test reads from and does with the parts of `rewind_actions.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The answers the self-test gives the rewind's questions, first first, and the questions it saw.
#[derive(Default)]
pub(crate) struct RewindAlerts {
    alerts: std::cell::RefCell<(std::collections::VecDeque<objc2_app_kit::NSModalResponse>, Vec<String>)>,
}

pub(super) fn with_alerts<R>(
    f: impl FnOnce(&std::cell::RefCell<(std::collections::VecDeque<objc2_app_kit::NSModalResponse>, Vec<String>)>) -> R,
) -> R {
    f(&crate::host::state().rewind_alerts.alerts)
}

/// Has the self-test answer the next questions of a code rewind with `answers` instead of the user.
pub fn answer_alerts(answers: &[objc2_app_kit::NSModalResponse]) {
    with_alerts(|a| a.borrow_mut().0.extend(answers.iter().copied()));
}

/// The questions a code rewind asked since the self-test began answering them.
pub fn alerts_seen() -> Vec<String> {
    with_alerts(|a| a.borrow().1.clone())
}
