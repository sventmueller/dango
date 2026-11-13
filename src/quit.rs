//! Quitting while agents work (E23): one question names what runs before anything stops; the supervisor then ends it
//! as it ends every process.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertSecondButtonReturn, NSAlertStyle};
use splat_i18n::{t, tf};

use crate::util::ns;

/// What quitting would stop, by name: AI tabs with a turn, a subagent or background work that runs, and coding agents
/// in terminals.
pub fn running() -> Vec<String> {
    let mut running = crate::agent::running();
    running.extend(crate::terminal::agents_running());
    running
}

/// Whether the app may quit as far as agents go: none works, or the user chose to quit anyway.
pub fn agents_may_stop(mtm: MainThreadMarker) -> bool {
    #[cfg(feature = "selftest")]
    if !asks_in_selftest() {
        return true;
    }
    let running = running();
    if running.is_empty() {
        return true;
    }
    question(mtm, &running).runModal() == NSAlertSecondButtonReturn
}

/// The question before quitting while `running` works; Return keeps the app open (HIG, E101).
fn question(mtm: MainThreadMarker, running: &[String]) -> objc2::rc::Retained<NSAlert> {
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&ns(t("Beenden, während Agenten noch arbeiten?")));
    alert.setInformativeText(&ns(&format!(
        "{}\n{}",
        tf("Läuft noch: {}", &[&running.join(", ")]),
        t("Beenden stoppt ihre Züge und Hintergrundprozesse.")
    )));
    // NSAlert gives a first button titled Cancel the Escape key instead of Return, which leaves no default button.
    alert
        .addButtonWithTitle(&ns(t("Abbrechen")))
        .setKeyEquivalent(&ns("\r"));
    let quit = alert.addButtonWithTitle(&ns(t("Beenden")));
    quit.setHasDestructiveAction(true);
    alert
}

/// Every self-test quits at its end with whatever its tabs left running; only the scenario about this question asks.
#[cfg(feature = "selftest")]
fn asks_in_selftest() -> bool {
    std::env::var("IDE_SELFTEST").map_or(true, |scenario| scenario == "quit")
}
