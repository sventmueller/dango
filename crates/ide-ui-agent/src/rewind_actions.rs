//! Rewinding from a prompt's card: the menu of what to take back, restoring the checkpoint of the code, and the
//! conversation's rewind through the CLI. The self-test answers the questions this asks.

use ide_git::{Restore, RestoreOptions};
use ide_thread::{Entry, EntryKind, Status};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSMenu, NSView};
use splat_i18n::{t, tf};

use super::acp::Agent;
use super::actions::pop_up;
use super::cards::pack;
use super::tab::AgentTab;
use ide_ui_common::util::{main_soon, ns};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

impl AgentTab {
    /// The rewind menu of user message `id`: the conversation, the code, or both; only while the session rests.
    pub fn rewind_menu(&self, id: u64, view: Option<&NSView>) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return;
        };
        let Some(uuid) = &message.message_id else { return };
        let menu = self.rewind_items(&entry, uuid);
        if let Some(view) = view {
            pop_up(&menu, view);
        }
    }

    /// The rewind menu of the prompt `entry`, whose id is `uuid`, without showing it.
    pub(super) fn rewind_items(&self, entry: &Entry, uuid: &str) -> objc2::rc::Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let resting = self.status() == Status::Idle && !self.state.borrow().sending;
        for (title, what, possible) in self.rewind_choices(entry, uuid) {
            let item = self.menu_item(&menu, title, sel!(ideAgentRewindPick:), pack(entry.id, what));
            item.setEnabled(possible && resting);
            if !resting {
                item.setToolTip(Some(&ns(&tf("Erst wenn {} fertig ist", &[self.agent.name()]))));
            }
        }
        menu
    }

    /// What the rewind menu of user message `entry` offers: the conversation, the code (I6), both; an agent over ACP
    /// and the own agent can only take the code back, from the app's own checkpoint (E40).
    pub(super) fn rewind_choices(&self, entry: &Entry, uuid: &str) -> Vec<(&'static str, u8, bool)> {
        let state = self.state.borrow();
        if !matches!(self.agent, Agent::Claude) {
            let has_code = state.checkpoints.contains_key(uuid);
            return vec![(t("Code auf den Stand vor dieser Nachricht"), 2, has_code)];
        }
        let has_code = state.checkpoints.contains_key(uuid) || state.source.is_live();
        drop(state);
        let has_conversation = self.conversation_rewindable(entry);
        vec![
            (t("Unterhaltung bis hierher zurückspulen"), 1, has_conversation),
            (t("Code auf den Stand vor dieser Nachricht"), 2, has_code),
            (t("Beides zurückspulen"), 3, has_conversation && has_code),
        ]
    }

    /// Rewinds to before user message `id`: `what` 1 the conversation, 2 the code, 3 both.
    pub fn rewind(&self, id: u64, what: u8) {
        if self.status() != Status::Idle || (!matches!(self.agent, Agent::Claude) && what != 2) {
            return;
        }
        if what == 2 || what == 3 {
            self.rewind_code(id);
        }
        if what == 1 || what == 3 {
            self.rewind_conversation(id);
        }
    }

    /// Brings the files back to how they were before user message `id`: the app's checkpoint, else the CLI's own
    /// file history.
    fn rewind_code(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return;
        };
        let Some(uuid) = message.message_id.clone() else { return };
        let checkpoint = self.state.borrow().checkpoints.get(&uuid).cloned();
        match checkpoint {
            Some(checkpoint) => self.restore(checkpoint, RestoreOptions::default()),
            None if matches!(self.agent, Agent::Claude) => self.rewind_with_cli(&uuid),
            None => {}
        }
    }

    fn restore(&self, checkpoint: ide_git::Checkpoint, options: RestoreOptions) {
        let key = self.key;
        let again = checkpoint.clone();
        self.store.restore(checkpoint, options, move |outcome| {
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.restored(again, outcome);
                }
            });
        });
    }

    fn restored(&self, checkpoint: ide_git::Checkpoint, outcome: Result<Restore, super::checkpoint::RestoreError>) {
        let alert = NSAlert::new(self.mtm);
        match outcome {
            Ok(Restore::UserChanges(files)) => {
                alert.setMessageText(&ns(&tf(
                    "Du hast seitdem {} Dateien selbst geändert.",
                    &[&files.len().to_string()],
                )));
                let names: Vec<String> = files.iter().take(12).map(|f| f.display().to_string()).collect();
                alert.setInformativeText(&ns(&names.join("\n")));
                alert.addButtonWithTitle(&ns(t("Deine Änderungen behalten")));
                alert.addButtonWithTitle(&ns(t("Trotzdem überschreiben")));
                alert.addButtonWithTitle(&ns(t("Abbrechen")));
                let response = run_alert(&alert);
                if response == NSAlertFirstButtonReturn {
                    let options = RestoreOptions {
                        keep: files,
                        ..RestoreOptions::default()
                    };
                    self.restore(checkpoint, options);
                } else if response == NSAlertSecondButtonReturn {
                    let options = RestoreOptions {
                        force: true,
                        ..RestoreOptions::default()
                    };
                    self.restore(checkpoint, options);
                }
                return;
            }
            Ok(Restore::Restored(done)) => {
                if let Some(controller) = self.controller() {
                    crate::host::host().take_back_buffers(controller.id(), done.buffers.clone());
                }
                alert.setMessageText(&ns(t("Code zurückgespult")));
                alert.setInformativeText(&ns(&tf(
                    "{} Dateien wiederhergestellt, {} entfernt.",
                    &[&done.written.len().to_string(), &done.removed.len().to_string()],
                )));
            }
            Err(error) => {
                alert.setMessageText(&ns(t("Zurückspulen fehlgeschlagen")));
                alert.setInformativeText(&ns(&ide_core::explain::Explain::explain(&error)));
            }
        }
        run_alert(&alert);
    }

    /// Without a checkpoint of the app, as for a resumed session: the CLI's own file history (experimental).
    fn rewind_with_cli(&self, uuid: &str) {
        let Some(session) = self.state.borrow().source.session().cloned() else {
            return;
        };
        let key = self.key;
        session.rewind_files(uuid, false).then(move |result| {
            main_soon(move || {
                let Some(mtm) = MainThreadMarker::new() else { return };
                if super::get(key).is_none() {
                    return;
                }
                let alert = NSAlert::new(mtm);
                match result {
                    Ok(done) if done.can_rewind => {
                        alert.setMessageText(&ns(t("Code zurückgespult")));
                        alert.setInformativeText(&ns(&tf(
                            "{} Dateien geändert.",
                            &[&done.files_changed.len().to_string()],
                        )));
                    }
                    Ok(done) => {
                        alert.setMessageText(&ns(t("Zurückspulen fehlgeschlagen")));
                        alert.setInformativeText(&ns(&done.error.unwrap_or_default()));
                    }
                    Err(error) => {
                        alert.setMessageText(&ns(t("Zurückspulen fehlgeschlagen")));
                        alert.setInformativeText(&ns(&ide_core::explain::Explain::explain(&error)));
                    }
                }
                alert.runModal();
            });
        });
    }
}

/// Shows `alert` and waits for the answer; in the self-test, the answer it set up, if any.
fn run_alert(alert: &NSAlert) -> objc2_app_kit::NSModalResponse {
    #[cfg(feature = "selftest")]
    {
        let answer = with_alerts(|a| {
            let mut alerts = a.borrow_mut();
            let answer = alerts.0.pop_front();
            if answer.is_some() {
                alerts.1.push(alert.messageText().to_string());
            }
            answer
        });
        if let Some(answer) = answer {
            return answer;
        }
    }
    alert.runModal()
}
