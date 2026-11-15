//! Opening a folder in a window that shows another one (E56): the new folder takes the old one's place in the window
//! and its tabs; if that would lose unsaved changes or stop what runs in any of its tabs, a sheet offers a new window
//! instead, closing everything, or cancelling.

use std::path::PathBuf;

use ide_ui_common::alert::{Alert, Role};
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAlertSecondButtonReturn};
use splat_i18n::{t, tf};

use super::folder_name;
use crate::window::{WindowController, layout};

/// What replacing a window's folder would lose.
#[derive(Default)]
struct Losses {
    /// The documents with unsaved changes.
    unsaved: Vec<String>,
    /// The agents and programs that run.
    running: Vec<String>,
}

impl Losses {
    fn of(controller: &WindowController) -> Losses {
        let mut running = ide_ui_agent::running_in(controller);
        running.extend(crate::terminal::running_in(controller));
        Losses {
            unsaved: crate::editor::unsaved_in(controller),
            running,
        }
    }

    fn is_empty(&self) -> bool {
        self.unsaved.is_empty() && self.running.is_empty()
    }

    /// What the sheet says is at stake, a line each.
    fn detail(&self) -> String {
        let mut lines = Vec::new();
        if !self.unsaved.is_empty() {
            lines.push(tf("Ungesicherte Änderungen: {}", &[&self.unsaved.join(", ")]));
        }
        if !self.running.is_empty() {
            lines.push(tf("Läuft noch: {}", &[&self.running.join(", ")]));
        }
        lines.push(t("„Alles schließen“ verwirft die Änderungen und beendet, was noch läuft.").to_string());
        lines.join("\n")
    }
}

impl WindowController {
    /// Shows `folder` in this window in place of its own folder: at once when nothing would be lost, else after the
    /// sheet that asks. A window that waits for the answer to a sheet keeps its folder, and `folder` gets a window of
    /// its own, since replacing would answer that sheet for the user.
    pub(crate) fn open_in_place(&self, folder: PathBuf) {
        let Some(window) = self.window() else { return };
        if window.attachedSheet().is_some() {
            return open_beside(self, folder);
        }
        let losses = Losses::of(self);
        if losses.is_empty() {
            return self.replace_project(folder);
        }
        let old = self.project().map(|p| folder_name(&p)).unwrap_or_default();
        let this = objc2::rc::Weak::from(self);
        Alert::new(
            self.mtm(),
            &tf("„{}“ statt „{}“ öffnen?", &[&folder_name(&folder), &old]),
        )
        .warning()
        .informative(&losses.detail())
        .button(t("In neuem Fenster öffnen"), Role::Plain)
        .button(t("Alles schließen"), Role::Destructive)
        .button(t("Abbrechen"), Role::CancelOnEscape)
        .sheet(&window, move |response| {
            let Some(this) = this.load() else { return };
            if response == NSAlertFirstButtonReturn {
                open_beside(&this, folder);
            } else if response == NSAlertSecondButtonReturn {
                this.replace_project(folder);
            }
        });
    }

    /// Closes what the window shows of its folder, keeping the folder's layout for next time, and opens `folder`
    /// with its own layout in the tab in front; the other tabs close. Closing a document ends its terminal or agent
    /// session.
    fn replace_project(&self, folder: PathBuf) {
        layout::save(self);
        crate::editor::discard_changes(self);
        self.clear_tabs();
        self.folder_closes();
        self.set_project(Some(folder));
        self.restore_layout();
        if self.tab_ids().is_empty() {
            self.open_welcome();
        }
        crate::session::changed();
    }
}

/// Opens `folder` in a window of its own beside `controller`'s.
fn open_beside(controller: &WindowController, folder: PathBuf) {
    let opened = WindowController::open_new(controller.mtm(), Some(folder));
    opened.restore_layout();
}
