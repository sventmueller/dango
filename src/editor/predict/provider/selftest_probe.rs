//! The self-test's hooks of `provider.rs`.

use super::*;

/// Answers every editor from `provider` instead of the settings' provider, or from the settings' again with `None`;
/// for the self-test.
pub fn force(provider: Option<Arc<dyn Provider>>) {
    with(|p| {
        if provider.is_none() {
            p.forced_outside.set(false);
        }
        *p.forced.borrow_mut() = provider;
    });
    super::super::follow_settings();
}

/// Answers every editor from `provider` as from one outside the Mac, whose requests the secrets check sees first; for
/// the self-test.
pub fn force_outside(provider: Arc<dyn Provider>) {
    with(|p| p.forced_outside.set(true));
    force(Some(provider));
}
