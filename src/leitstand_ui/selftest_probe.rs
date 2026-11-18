//! The self-test's hooks of `leitstand_ui.rs`.

use super::*;

/// The views' host as the app installs it, answering through `back`, for the self-test to call as the views do.
pub(crate) fn views_host(back: Back) -> Orders {
    Orders(back)
}
