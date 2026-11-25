//! The privacy permissions the comparison needs, which macOS grants to the app it runs in, such as the terminal.

use objc2_application_services::AXIsProcessTrusted;
use objc2_core_graphics::{CGPreflightPostEventAccess, CGPreflightScreenCaptureAccess};

/// The missing permissions, each with where System Settings grants it.
pub(crate) fn missing() -> Vec<&'static str> {
    let mut missing = Vec::new();
    // SAFETY: a query without arguments.
    if !unsafe { AXIsProcessTrusted() } {
        missing.push(
            "Accessibility, to place and raise the windows: System Settings > Privacy & Security > Accessibility \
             (Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen)",
        );
    }
    if !CGPreflightPostEventAccess() {
        missing.push(
            "posting events, to type and scroll: System Settings > Privacy & Security > Accessibility \
             (Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen)",
        );
    }
    if !CGPreflightScreenCaptureAccess() {
        missing.push(
            "Screen Recording, to see the frames: System Settings > Privacy & Security > Screen & System Audio \
             Recording (Systemeinstellungen > Datenschutz & Sicherheit > Bildschirm- & Systemaudioaufnahme)",
        );
    }
    missing
}

/// An error naming every missing permission, if one is.
pub(crate) fn require() -> Result<(), String> {
    let missing = missing();
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the app this runs in lacks permissions; grant them, then restart it:\n  - {}",
        missing.join("\n  - ")
    ))
}
