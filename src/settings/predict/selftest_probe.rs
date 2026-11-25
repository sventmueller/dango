//! What the self-test reads from and does with the parts of `predict.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// What the tab shows, for the self-test: the provider's title, the Copilot line and the consent line.
pub(crate) fn shown() -> Option<(String, String, String, bool)> {
    let this = crate::context::app().settings().prediction.shown.borrow().clone()?;
    let views = this.ivars().views.get()?;
    Some((
        views.provider.titleOfSelectedItem()?.to_string(),
        views.copilot.stringValue().to_string(),
        views.allowed.stringValue().to_string(),
        views.revoke.isEnabled(),
    ))
}

/// What the tab shows of the local server, for the self-test.
pub(crate) struct LocalShown {
    pub(crate) about: String,
    pub(crate) url: String,
    pub(crate) model: String,
    /// The models the model field offers.
    pub(crate) offered: Vec<String>,
    pub(crate) note: String,
    pub(crate) exclude: String,
}

/// What the tab shows of the local server, for the self-test.
pub(crate) fn local_shown() -> Option<LocalShown> {
    let this = crate::context::app().settings().prediction.shown.borrow().clone()?;
    let views = this.ivars().views.get()?;
    let offered = (0..views.model.numberOfItems())
        .filter_map(|index| {
            let item = views.model.itemObjectValueAtIndex(index);
            item.downcast::<objc2_foundation::NSString>()
                .ok()
                .map(|s| s.to_string())
        })
        .collect();
    Some(LocalShown {
        about: views.about.stringValue().to_string(),
        url: views.url.stringValue().to_string(),
        model: views.model.stringValue().to_string(),
        offered,
        note: views.model_note.stringValue().to_string(),
        exclude: views.exclude.stringValue().to_string(),
    })
}
