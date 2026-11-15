//! What the self-test reads from and does with the parts of `view.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl BrowserView {
    pub(crate) fn address_field(&self) -> Retained<objc2_app_kit::NSTextField> {
        self.parts().controls.field.clone()
    }

    pub(crate) fn seen(&self) -> tabs::Seen {
        let parts = self.parts();
        tabs::Seen {
            url: self.current_url(),
            address: parts.controls.field.stringValue().to_string(),
            editing: parts.controls.field.currentEditor().is_some(),
            inspector: web::has_inspector(&parts.web),
            failure: parts.failure.is_shown().then(|| parts.failure.heading()),
            proceed: parts.failure.offers_proceed(),
            notice: parts.notice.text(),
        }
    }
}
