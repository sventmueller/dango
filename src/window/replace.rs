//! Opening a folder in a window that shows another one (E56): the new folder takes the old one's place in the window
//! and its tab group; if that would lose unsaved changes or stop what runs in any of its tabs, a sheet offers a new
//! window instead, closing everything, or cancelling.

use std::path::PathBuf;

use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle};
use splat_i18n::{t, tf};

use super::{WINDOWS, WindowController, folder_name, layout};
use crate::util::{guarded, ns};

/// What replacing a window's folder would lose.
#[derive(Default)]
struct Losses {
    /// The documents with unsaved changes.
    unsaved: Vec<String>,
    /// The agents and programs that run.
    running: Vec<String>,
}

impl Losses {
    fn of(controllers: &[&WindowController]) -> Losses {
        let mut losses = Losses::default();
        for controller in controllers {
            losses.unsaved.extend(crate::editor::unsaved_in(controller));
            losses.running.extend(crate::agent::running_in(controller));
            losses.running.extend(crate::terminal::running_in(controller));
        }
        losses
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
        let siblings = self.tab_siblings();
        let mut group: Vec<&WindowController> = siblings.iter().map(|s| &**s).collect();
        group.insert(0, self);
        let losses = Losses::of(&group);
        if losses.is_empty() {
            return self.replace_project(folder);
        }
        let alert = NSAlert::new(self.mtm());
        alert.setAlertStyle(NSAlertStyle::Warning);
        let old = self.project().map(|p| folder_name(&p)).unwrap_or_default();
        alert.setMessageText(&ns(&tf("„{}“ statt „{}“ öffnen?", &[&folder_name(&folder), &old])));
        alert.setInformativeText(&ns(&losses.detail()));
        alert.addButtonWithTitle(&ns(t("In neuem Fenster öffnen")));
        let close = alert.addButtonWithTitle(&ns(t("Alles schließen")));
        close.setHasDestructiveAction(true);
        let cancel = alert.addButtonWithTitle(&ns(t("Abbrechen")));
        cancel.setKeyEquivalent(&ns("\u{1b}"));
        let this = objc2::rc::Weak::from(self);
        let handler = block2::RcBlock::new(move |response: objc2_app_kit::NSModalResponse| {
            guarded((), || {
                let Some(this) = this.load() else { return };
                if response == NSAlertFirstButtonReturn {
                    open_beside(&this, folder.clone());
                } else if response == NSAlertSecondButtonReturn {
                    this.replace_project(folder.clone());
                }
            });
        });
        alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler));
    }

    /// Closes what the window shows of its folder, keeping the folder's layout for next time, and opens `folder`
    /// with its own layout. Closing a tab ends its terminal or agent session.
    fn replace_project(&self, folder: PathBuf) {
        for sibling in self.tab_siblings() {
            crate::editor::discard_changes(&sibling);
            if let Some(window) = sibling.window() {
                window.close();
            }
        }
        layout::save(self);
        crate::editor::discard_changes(self);
        for id in self.tab_ids() {
            self.close_tab_now(id);
        }
        crate::agent::window_closed(self);
        crate::ci_panel::window_closed(self);
        if let Some(root) = self.project_root() {
            crate::lsp::window_closed(&root);
        }
        self.set_project(Some(folder));
        self.restore_layout();
        if self.tab_ids().is_empty() {
            self.open_welcome();
        }
        crate::session::changed();
    }
}

impl WindowController {
    /// The other windows in this window's tab group.
    pub(crate) fn tab_siblings(&self) -> Vec<Retained<WindowController>> {
        let Some(own) = self.window() else { return vec![] };
        let Some(tabbed) = own.tabbedWindows() else {
            return vec![];
        };
        WINDOWS
            .with(|windows| windows.borrow().clone())
            .into_iter()
            .filter(|other| {
                other.window().is_some_and(|window| {
                    !std::ptr::eq(&*window, &*own) && tabbed.iter().any(|tab| std::ptr::eq(&*tab, &*window))
                })
            })
            .collect()
    }
}

/// Opens `folder` in a window of its own beside `controller`'s.
fn open_beside(controller: &WindowController, folder: PathBuf) {
    let opened = WindowController::open_new(controller.mtm(), Some(folder));
    opened.restore_layout();
}
