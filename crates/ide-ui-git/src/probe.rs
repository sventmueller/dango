//! What the self-test reads from and does with the stash and the commit message of a window's Git panel, through the
//! code their controls run.

use std::path::{Path, PathBuf};

use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSAccessibility, NSTextInputClient};
use objc2_foundation::NSRange;

use super::model::RowKey;
use super::panel;
use ide_ui_common::util::ns;
use ide_ui_common::window::AsWindow;

/// A stash entry as its row shows it: commit, message.
pub type StashSeen = (String, String);

/// The stash entries the panel lists, newest first.
pub fn stashes(controller: &(impl AsWindow + ?Sized)) -> Vec<StashSeen> {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return Vec::new();
    };
    let state = panel.read();
    state
        .model
        .stashes
        .iter()
        .map(|s| (s.hash.clone(), s.message.clone()))
        .collect()
}

/// The count the section's pill shows, if the section shows.
pub fn stash_pill(controller: &(impl AsWindow + ?Sized)) -> Option<String> {
    let controller = &controller.as_window();
    let panel = panel::of(controller)?;
    let item = panel.item_for(&RowKey::Section(super::model::Section::Stashes));
    let outline = &panel.ui().outline;
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    let row = unsafe { outline.rowForItem(Some(&item)) };
    let view = outline.viewAtColumn_row_makeIfNecessary(0, row, true)?;
    Some(view.accessibilityLabel()?.to_string())
}

/// Whether a stash apply waits at its conflicts, and whether it was a pop.
pub fn stash_conflict(controller: &(impl AsWindow + ?Sized)) -> Option<bool> {
    let controller = &controller.as_window();
    panel::of(controller)?.read().stash_apply.as_ref().map(|s| s.pop)
}

/// The banner's title, text and buttons, while it shows.
pub fn banner(controller: &(impl AsWindow + ?Sized)) -> Option<[String; 4]> {
    let controller = &controller.as_window();
    let panel = panel::of(controller)?;
    let banner = &panel.ui().banner;
    (!banner.view.isHidden()).then(|| {
        [
            banner.title.stringValue().to_string(),
            banner.detail.stringValue().to_string(),
            banner.resume.title().to_string(),
            banner.abort.title().to_string(),
        ]
    })
}

/// "Stash …" from the menu beside "Commit".
pub fn stash(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.ask_stash();
    }
}

/// "Nur Vorgemerktes stashen".
pub fn stash_staged(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.stash_staged();
    }
}

/// What a stash row offers.
#[derive(Debug, Clone, Copy)]
pub enum StashAction {
    Apply,
    Pop,
    Drop,
}

/// Runs `action` on the stash entry `hash`, as its button does.
pub fn stash_action(controller: &(impl AsWindow + ?Sized), hash: &str, action: StashAction) {
    let controller = &controller.as_window();
    let selector = match action {
        StashAction::Apply => sel!(ideGitApplyStash:),
        StashAction::Pop => sel!(ideGitPopStash:),
        StashAction::Drop => sel!(ideGitDropStash:),
    };
    if let Some(panel) = panel::of(controller) {
        panel.run_row_action(selector, vec![RowKey::Stash(hash.to_string())]);
    }
}

/// "Fertig" or "Abbrechen …" in the banner of a stash conflict.
pub fn end_conflict(controller: &(impl AsWindow + ?Sized), finish: bool) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        if finish {
            panel.finish_stash_conflict();
        } else {
            panel.abort_stash_conflict();
        }
    }
}

/// Opens the row of the stash entry `hash`, as a click on it does.
pub fn open_stash(controller: &(impl AsWindow + ?Sized), hash: &str) {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else { return };
    let item = panel.item_for(&RowKey::Stash(hash.to_string()));
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    let row = unsafe { panel.ui().outline.rowForItem(Some(&item)) };
    panel.preview_row(row);
}

/// Selects the row of the stash entry `hash`, and only it.
pub fn select_stash(controller: &(impl AsWindow + ?Sized), hash: &str) {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else { return };
    let outline = &panel.ui().outline;
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    let row = unsafe { outline.rowForItem(Some(&panel.item_for(&RowKey::Stash(hash.to_string())))) };
    if let Ok(row) = usize::try_from(row) {
        outline.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
    }
}

/// The files the list shows below the stash entry `hash`.
pub fn stash_rows(controller: &(impl AsWindow + ?Sized), hash: &str) -> Vec<PathBuf> {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return Vec::new();
    };
    let outline = &panel.ui().outline;
    (0..outline.numberOfRows())
        .filter_map(|row| outline.itemAtRow(row))
        .filter_map(|item| match panel::key_of(&item)? {
            RowKey::StashFile(at, file) if at == hash => Some(file),
            _ => None,
        })
        .collect()
}

/// Shows what the stash entry `hash` changed in `file`, as a double click on the file's row does.
pub fn show_stash_diff(controller: &(impl AsWindow + ?Sized), hash: &str, file: &Path) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.show_diff(&RowKey::StashFile(hash.to_string(), file.to_path_buf()), false);
    }
}

/// Rows and hunks of the diff tab of what the stash entry `hash` changed in `file`.
pub fn stash_diff_counts(controller: &(impl AsWindow + ?Sized), hash: &str, file: &Path) -> Option<(usize, usize)> {
    let controller = &controller.as_window();
    super::diff::find(controller, &super::diff::stash_key(hash, file)).map(|tab| tab.counts())
}

/// Whether the menu bar's Git command `action` can run now, on the selected rows.
pub fn can(controller: &(impl AsWindow + ?Sized), action: Sel) -> bool {
    let controller = &controller.as_window();
    panel::of(controller).is_some_and(|panel| panel.can(action, &panel.selected_keys()))
}

/// What the commit message and its controls show.
#[derive(Debug, Clone, Default)]
pub struct MessageSeen {
    pub text: String,
    pub editable: bool,
    /// Whether "Von AI erstellt, bitte prüfen" shows.
    pub written: bool,
    /// The checked request that went out last.
    pub sent: Option<String>,
    pub button: bool,
    pub tip: String,
    pub running: bool,
    pub progress: String,
}

pub fn message(controller: &(impl AsWindow + ?Sized)) -> MessageSeen {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return MessageSeen::default();
    };
    let ui = panel.ui();
    let state = panel.read();
    MessageSeen {
        text: ui.message.text(),
        editable: ui.message.isEditable(),
        written: !ui.ai_box.view.isHidden() && !ui.ai_box.written.isHidden(),
        sent: state.message.sent.as_ref().map(|(text, _)| text.clone()),
        button: ui.ai.isEnabled(),
        tip: ui.ai.toolTip().map(|t| t.to_string()).unwrap_or_default(),
        running: state.message.running.is_some(),
        progress: ui.progress_label.stringValue().to_string(),
    }
}

/// Clicks the button in the commit field.
pub fn generate(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.generate_message();
    }
}

/// Sets the message the user is writing, as typing it would leave it, without an AI note.
pub fn set_draft(controller: &(impl AsWindow + ?Sized), text: &str) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.ui().message.setString(&ns(text));
        panel.update_placeholder();
    }
}

/// Types `text` at the end of the message, as the keyboard does.
pub fn type_text(controller: &(impl AsWindow + ?Sized), text: &str) {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else { return };
    let view = &panel.ui().message;
    let end = view.string().length();
    // SAFETY: Inserting a string at the end of the text view's own text, which its range covers.
    unsafe { view.insertText_replacementRange(&ns(text), NSRange::new(end, 0)) };
}

/// "Widerrufen" under a message the model wrote, as ⌘Z in the field does.
pub fn undo(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.undo_message();
    }
}

/// "Gesendeten Inhalt zeigen".
pub fn show_sent(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.show_sent();
    }
}

/// The progress row's cancel button.
pub fn cancel(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.cancel_message();
    }
}
