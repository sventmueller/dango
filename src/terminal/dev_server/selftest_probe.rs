//! The self-test's hooks of `dev_server.rs`.

use super::*;

impl DevServerBanner {
    /// What the banner says while it shows.
    pub(crate) fn shown(&self) -> Option<String> {
        (!self.isHidden()).then(|| self.ivars().message.stringValue().to_string())
    }
}
