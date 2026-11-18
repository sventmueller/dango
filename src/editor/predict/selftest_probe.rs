//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The proposal `view` shows, as the range it replaces in bytes and its text, for the self-test.
pub(crate) fn shown_proposal(view: &EditorTextView) -> Option<(Range<usize>, String, Kind)> {
    let state = view.ivars().predict.borrow();
    state
        .shown
        .as_ref()
        .map(|(p, _)| (p.range.clone(), p.text.clone(), p.kind.clone()))
}

/// What the box below the line and the status bar show, for the self-test.
pub(crate) use super::shown::{boxed_text, marked, status_shown};

/// Takes away what a dropped proposal left on view now, which a keystroke otherwise leaves for the run loop's next
/// turn; for the self-test's keystroke budget.
pub(crate) fn settle_now(view: &EditorTextView) {
    if view.ivars().predict.borrow().shown.is_none() {
        shown::clear(view);
    }
}

/// The view's state for a proposal shown at once, as an answer would, for the self-test's keystroke budget.
pub(crate) fn show_now(view: &EditorTextView, provider: Arc<dyn Provider>, prediction: Prediction) -> bool {
    let (text, revision) = {
        let document = view.document();
        (document.text().to_string(), document.revision())
    };
    match Proposal::first(&text, revision, selection(view), vec![prediction]) {
        Some(proposal) => {
            show(view, proposal, provider);
            true
        }
        None => false,
    }
}
