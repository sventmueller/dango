//! The self-test's hooks of `login.rs`.

/// What the self-test would have opened or copied, in order.
pub(crate) fn opened() -> Vec<String> {
    crate::context::app().ci_panels().noted.borrow().clone()
}
