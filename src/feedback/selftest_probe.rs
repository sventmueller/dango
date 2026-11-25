//! The self-test's hooks of `feedback.rs`.

use crate::context::AppContext;

/// The pages the self-test asked for, in order.
pub(crate) fn opened(cx: &AppContext) -> Vec<String> {
    cx.feedback().0.borrow().clone()
}
