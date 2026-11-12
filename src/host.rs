//! The app as the `ide-ui-*` crates see it: each crate's `Host`, answered from the app context (E99).

use std::rc::Rc;

/// The running app, as the crates' hooks reach it.
pub struct AppHost;

impl ide_ui_export::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_export::State> {
        crate::context::app().exports.clone()
    }
}
