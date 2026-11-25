//! The self-test's hooks of `install.rs`.

use super::*;

/// Installs the slot's missing server, as the status bar's button does, for the self-test.
pub(crate) fn install_now(folder: &Path, slot: &Slot) {
    install(&crate::host::state(), folder, slot);
}
