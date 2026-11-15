//! Quitting while agents work (E23): one question names what runs before anything stops; the supervisor then ends it
//! as it ends every process.

use ide_ui_common::alert::{Alert, Role};
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertSecondButtonReturn;
use splat_i18n::{t, tf};

/// What quitting would stop, by name: AI tabs with a turn, a subagent or background work that runs, and coding agents
/// in terminals.
pub(crate) fn running() -> Vec<String> {
    let mut running = ide_ui_agent::running();
    running.extend(crate::terminal::agents_running());
    running
}

/// Whether the app may quit as far as agents go: none works, or the user chose to quit anyway.
pub(crate) fn agents_may_stop(mtm: MainThreadMarker) -> bool {
    #[cfg(feature = "selftest")]
    if !asks_in_selftest() {
        return true;
    }
    let running = running();
    if running.is_empty() {
        return true;
    }
    question(mtm, &running).run_modal() == NSAlertSecondButtonReturn
}

/// The question before quitting while `running` works; Return keeps the app open (HIG, E101).
fn question(mtm: MainThreadMarker, running: &[String]) -> Alert {
    Alert::new(mtm, t("Beenden, während Agenten noch arbeiten?"))
        .warning()
        .informative(&format!(
            "{}\n{}",
            tf("Läuft noch: {}", &[&running.join(", ")]),
            t("Beenden stoppt ihre Züge und Hintergrundprozesse.")
        ))
        // NSAlert gives a first button titled Cancel the Escape key instead of Return, which leaves no default button.
        .button(t("Abbrechen"), Role::CancelOnReturn)
        .button(t("Beenden"), Role::Destructive)
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
