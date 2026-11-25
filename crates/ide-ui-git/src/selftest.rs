//! What the self-test reads from and does with a window's Git panel, through the code its controls run.

use std::path::{Path, PathBuf};

use objc2::sel;

use crate::model::{RowKey, Section};
use crate::panel;
use ide_ui_common::window::{AsWindow, Window};

pub use crate::model::HISTORY_PAGE;

/// The panel's phase, its sections, the branch and what is running.
pub struct Seen {
    /// The panel's phase, by its name in the code.
    pub phase: String,
    /// The paths of the conflicts section.
    pub conflicts: Vec<PathBuf>,
    /// The paths of the staged section.
    pub staged: Vec<PathBuf>,
    /// The paths of the changes section.
    pub changes: Vec<PathBuf>,
    /// How many commits the history lists.
    pub commits: usize,
    /// The subject and the number of parents of the commit HEAD is at.
    pub top: Option<(String, usize)>,
    /// How many rows the outline view has.
    pub rows: isize,
    /// The message of the problem the panel shows.
    pub problem: Option<String>,
    /// The branch HEAD is on.
    pub branch: Option<String>,
    /// The branch's upstream.
    pub upstream: Option<String>,
    /// How many commits the branch is ahead of its upstream and behind it.
    pub ahead_behind: Option<(u32, u32)>,
    /// The merge, rebase or other operation in progress, by its name in the code.
    pub operation: Option<String>,
    /// Whether the banner of an operation in progress shows.
    pub banner: bool,
    /// Whether the banner's button to continue is enabled.
    pub can_resume: bool,
    /// Whether a git command of the panel runs.
    pub busy: bool,
}

/// What the self-test reads from the Git panel of `controller`'s window.
pub fn seen(controller: &(impl AsWindow + ?Sized)) -> Option<Seen> {
    let controller = &controller.as_window();
    let panel = panel::of(controller)?;
    let state = panel.read();
    let ui = panel.ui();
    let paths = |rows: &[crate::model::FileRow]| rows.iter().map(|r| r.path().to_path_buf()).collect();
    Some(Seen {
        phase: format!("{:?}", state.phase),
        conflicts: paths(&state.model.conflicts),
        staged: paths(&state.model.staged),
        changes: paths(&state.model.changes),
        commits: state.model.commits.len(),
        top: state
            .model
            .commits
            .iter()
            .zip(&state.model.graph)
            .find(|(_, row)| row.head)
            .map(|(c, _)| (c.subject.clone(), c.parents.len())),
        rows: ui.outline.numberOfRows(),
        problem: state.problem.as_ref().map(|p| p.message.clone()),
        branch: state.branch.head.clone(),
        upstream: state.branch.upstream.clone(),
        ahead_behind: state.branch.ahead_behind,
        operation: state.operation.map(|o| format!("{o:?}")),
        banner: !ui.banner.view.isHidden(),
        can_resume: ui.banner.resume.isEnabled(),
        busy: state.busy.is_some(),
    })
}

/// Stages `path` as its row's button does.
pub fn stage(controller: &(impl AsWindow + ?Sized), path: &Path) {
    let controller = &controller.as_window();
    row_action(controller, sel!(ideGitStage:), Section::Changes, path);
}

/// Takes back every staged file, as the button of the staged section does.
pub fn unstage_all(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.unstage_all();
    }
}

/// Asks to discard the changes of `path` as its row's button does.
pub fn discard(controller: &(impl AsWindow + ?Sized), path: &Path) {
    let controller = &controller.as_window();
    row_action(controller, sel!(ideGitDiscard:), Section::Changes, path);
}

/// Marks the conflicts of `path` resolved as its row's button does.
pub fn resolve(controller: &(impl AsWindow + ?Sized), path: &Path) {
    let controller = &controller.as_window();
    row_action(controller, sel!(ideGitResolve:), Section::Conflicts, path);
}

fn row_action(controller: &Window, action: objc2::runtime::Sel, section: Section, path: &Path) {
    if let Some(panel) = panel::of(controller) {
        panel.run_row_action(action, vec![RowKey::File(section, path.to_path_buf())]);
    }
}

/// Types `message` and commits as ⌘↩ does.
pub fn commit(controller: &(impl AsWindow + ?Sized), message: &str) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.ui().message.setString(&ide_ui_common::util::ns(message));
        panel.update_placeholder();
        panel.commit();
    }
}

/// A button of the panel.
#[derive(Debug, Clone, Copy)]
#[expect(missing_docs, reason = "the names say it")]
pub enum Control {
    Fetch,
    Pull,
    Push,
    Cancel,
    Resume,
    Abort,
}

/// Clicks `control`.
pub fn click(controller: &(impl AsWindow + ?Sized), control: Control) {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else { return };
    let ui = panel.ui();
    let button = match control {
        Control::Fetch => &ui.fetch,
        Control::Pull => &ui.pull,
        Control::Push => &ui.push,
        Control::Cancel => &ui.cancel,
        Control::Resume => &ui.banner.resume,
        Control::Abort => &ui.banner.abort,
    };
    // SAFETY: A nil sender is what AppKit passes for a click made in code.
    unsafe { button.performClick(None) };
}

/// Chooses the item titled `title` in the branch popup; whether there is one.
pub fn choose_branch(controller: &(impl AsWindow + ?Sized), title: &str) -> bool {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return false;
    };
    let Some(menu) = panel.ui().branch.menu() else {
        return false;
    };
    let found = menu
        .itemArray()
        .iter()
        .position(|item| item.title().to_string() == title);
    if let Some(index) = found.and_then(|i| isize::try_from(i).ok()) {
        menu.performActionForItemAtIndex(index);
    }
    found.is_some()
}

/// Says the ages of the commits again, as the minute tick does.
pub fn refresh_ages(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.refresh_ages();
    }
}

/// Opens the diff of `path` as a double click on its row does.
pub fn show_diff(controller: &(impl AsWindow + ?Sized), path: &Path, staged: bool) {
    let controller = &controller.as_window();
    let section = if staged { Section::Staged } else { Section::Changes };
    if let Some(panel) = panel::of(controller) {
        panel.show_diff(&RowKey::File(section, path.to_path_buf()), false);
    }
}

/// Opens the diff of `path` as a single click on its row does.
pub fn preview_diff(controller: &(impl AsWindow + ?Sized), path: &Path, staged: bool) {
    let controller = &controller.as_window();
    let section = if staged { Section::Staged } else { Section::Changes };
    if let Some(panel) = panel::of(controller) {
        panel.show_diff(&RowKey::File(section, path.to_path_buf()), true);
    }
}

/// Selects every row of the diff tab of `path` and copies them, as ⌘A and ⌘C do.
pub fn copy_diff(controller: &(impl AsWindow + ?Sized), path: &Path, staged: bool) {
    let controller = &controller.as_window();
    let (Some(tab), Some(mtm)) = (
        super::diff::find(controller, &super::diff::key(path, staged)),
        objc2::MainThreadMarker::new(),
    ) else {
        return;
    };
    tab.select_all();
    send(mtm, &tab, sel!(copy:));
}

/// Sends `action` to the Git panel, as its button does.
pub fn press(controller: &(impl AsWindow + ?Sized), action: objc2::runtime::Sel) {
    let controller = &controller.as_window();
    if let (Some(panel), Some(mtm)) = (panel::of(controller), objc2::MainThreadMarker::new()) {
        send(mtm, &panel, action);
    }
}

fn send(mtm: objc2::MainThreadMarker, target: &objc2::runtime::AnyObject, action: objc2::runtime::Sel) {
    // SAFETY: The target's own action with no sender, as a click made in code sends it.
    unsafe { objc2_app_kit::NSApplication::sharedApplication(mtm).sendAction_to_from(action, Some(target), None) };
}

/// Rows and hunks of the diff tab of `path`, once it has read its diff.
pub fn diff_counts(controller: &(impl AsWindow + ?Sized), path: &Path, staged: bool) -> Option<(usize, usize)> {
    let controller = &controller.as_window();
    super::diff::find(controller, &super::diff::key(path, staged)).map(|tab| tab.counts())
}

/// A thread worktree as its row shows it: folder, branch, ahead, behind and the files it changed.
pub(crate) type TreeSeen = (PathBuf, String, usize, usize, Vec<PathBuf>);

/// The thread worktrees the panel lists.
pub fn worktrees(controller: &(impl AsWindow + ?Sized)) -> Vec<TreeSeen> {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return Vec::new();
    };
    let state = panel.read();
    state
        .model
        .worktrees
        .iter()
        .map(|w| {
            let files = w.changed.iter().map(|f| f.path.clone()).collect();
            (w.tree.path.clone(), w.tree.branch.clone(), w.ahead, w.behind, files)
        })
        .collect()
}

/// The files the list shows below the worktree at `tree`.
pub fn tree_rows(controller: &(impl AsWindow + ?Sized), tree: &Path) -> Vec<PathBuf> {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return Vec::new();
    };
    let outline = &panel.ui().outline;
    (0..outline.numberOfRows())
        .filter_map(|row| outline.itemAtRow(row))
        .filter_map(|item| match panel::key_of(&item)? {
            RowKey::WorktreeFile(at, file) if at == tree => Some(file),
            _ => None,
        })
        .collect()
}

/// What a worktree row offers.
#[derive(Debug, Clone, Copy)]
#[expect(missing_docs, reason = "the names say it")]
pub enum TreeAction {
    TakeOver,
    OpenThread,
    KeepBranch,
    Remove,
    ShowOrder,
}

/// Runs `action` on the worktree at `tree`, as its menu item does.
pub fn tree_action(controller: &(impl AsWindow + ?Sized), tree: &Path, action: TreeAction) {
    let controller = &controller.as_window();
    let selector = match action {
        TreeAction::TakeOver => sel!(ideGitTakeOver:),
        TreeAction::OpenThread => sel!(ideGitOpenThread:),
        TreeAction::KeepBranch => sel!(ideGitKeepBranch:),
        TreeAction::Remove => sel!(ideGitRemoveTree:),
        TreeAction::ShowOrder => sel!(ideGitShowOrder:),
    };
    if let Some(panel) = panel::of(controller) {
        panel.run_row_action(selector, vec![RowKey::Worktree(tree.to_path_buf())]);
    }
}

/// Opens the worktree's row and shows the diff of its `file`, as a double click on the file's row does.
pub fn show_tree_diff(controller: &(impl AsWindow + ?Sized), tree: &Path, file: &Path) {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else { return };
    let item = panel.item_for(&RowKey::Worktree(tree.to_path_buf()));
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    unsafe { panel.ui().outline.expandItem(Some(&item)) };
    let key = RowKey::WorktreeFile(tree.to_path_buf(), file.to_path_buf());
    panel.run_row_action(sel!(ideGitDiff:), vec![key]);
}

/// Opens the worktree's `file` in the editor, as its menu item does.
pub fn open_tree_file(controller: &(impl AsWindow + ?Sized), tree: &Path, file: &Path) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        let key = RowKey::WorktreeFile(tree.to_path_buf(), file.to_path_buf());
        panel.run_row_action(sel!(ideGitOpen:), vec![key]);
    }
}

/// Rows and hunks of the diff tab of the worktree's `file`, once it has read its diff.
pub fn tree_diff_counts(controller: &(impl AsWindow + ?Sized), tree: &Path, file: &Path) -> Option<(usize, usize)> {
    let controller = &controller.as_window();
    super::diff::find(controller, &super::diff::since_key(tree, file)).map(|tab| tab.counts())
}

/// The hash of the commit titled `subject` in the graph.
fn commit_hash(controller: &Window, subject: &str) -> Option<ide_git::CommitId> {
    let panel = panel::of(controller)?;
    let state = panel.read();
    state
        .model
        .commits
        .iter()
        .find(|c| c.subject == subject)
        .map(|c| c.hash.clone())
}

/// Chooses `action` in the context menu of the commit `hash`'s row, as a click on its item does.
pub fn commit_action(controller: &(impl AsWindow + ?Sized), hash: &ide_git::CommitId, action: objc2::runtime::Sel) {
    let controller = &controller.as_window();
    menu_action(controller, RowKey::Commit(hash.clone()), action);
}

/// Chooses `action` in the context menu of the staged file `path`'s row, as a click on its item does.
pub fn staged_file_action(controller: &(impl AsWindow + ?Sized), path: &Path, action: objc2::runtime::Sel) {
    let controller = &controller.as_window();
    menu_action(controller, RowKey::File(Section::Staged, path.to_path_buf()), action);
}

fn menu_action(controller: &Window, key: RowKey, action: objc2::runtime::Sel) {
    let (Some(panel), Some(mtm)) = (panel::of(controller), objc2::MainThreadMarker::new()) else {
        return;
    };
    panel.state().menu_keys = vec![key];
    let item = objc2_app_kit::NSMenuItem::new(mtm);
    // SAFETY: The panel's own action with a menu item as the sender, as AppKit sends it from the context menu.
    unsafe {
        objc2_app_kit::NSApplication::sharedApplication(mtm).sendAction_to_from(action, Some(&panel), Some(&item))
    };
}

/// Whether the list ends in the row that reads more of the history.
pub fn has_more_row(controller: &(impl AsWindow + ?Sized)) -> bool {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else {
        return false;
    };
    let outline = &panel.ui().outline;
    let last = outline.numberOfRows() - 1;
    outline
        .itemAtRow(last)
        .and_then(|item| panel::key_of(&item))
        .is_some_and(|key| key == RowKey::More)
}

/// Opens the row of the commit titled `subject`, as a click on it does.
pub fn open_commit(controller: &(impl AsWindow + ?Sized), subject: &str) {
    let controller = &controller.as_window();
    let (Some(panel), Some(hash)) = (panel::of(controller), commit_hash(controller, subject)) else {
        return;
    };
    let item = panel.item_for(&RowKey::Commit(hash));
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    let row = unsafe { panel.ui().outline.rowForItem(Some(&item)) };
    panel.preview_row(row);
}

/// Selects the row of the commit titled `subject`, and only it.
pub fn select_commit(controller: &(impl AsWindow + ?Sized), subject: &str) {
    let controller = &controller.as_window();
    let (Some(panel), Some(hash)) = (panel::of(controller), commit_hash(controller, subject)) else {
        return;
    };
    let outline = &panel.ui().outline;
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    let row = unsafe { outline.rowForItem(Some(&panel.item_for(&RowKey::Commit(hash)))) };
    if let Ok(row) = usize::try_from(row) {
        outline.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
    }
}

/// Selects the row of the changed file `path`, and only it, in a list that has the keyboard focus.
pub fn select_file(controller: &(impl AsWindow + ?Sized), path: &Path) {
    let controller = &controller.as_window();
    let Some(panel) = panel::of(controller) else { return };
    let outline = &panel.ui().outline;
    let row = (0..outline.numberOfRows()).find(|&row| {
        outline
            .itemAtRow(row)
            .and_then(|item| panel::key_of(&item))
            .is_some_and(|key| matches!(key, RowKey::File(_, file) if file == path))
    });
    if let Some(row) = row.and_then(|row| usize::try_from(row).ok()) {
        outline.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
    }
    if let Some(window) = outline.window() {
        window.makeFirstResponder(Some(outline));
    }
}

/// The files the list shows below the commit titled `subject`.
pub fn commit_rows(controller: &(impl AsWindow + ?Sized), subject: &str) -> Vec<PathBuf> {
    let controller = &controller.as_window();
    let (Some(panel), Some(hash)) = (panel::of(controller), commit_hash(controller, subject)) else {
        return Vec::new();
    };
    let outline = &panel.ui().outline;
    (0..outline.numberOfRows())
        .filter_map(|row| outline.itemAtRow(row))
        .filter_map(|item| match panel::key_of(&item)? {
            RowKey::CommitFile(at, file) if at == hash => Some(file),
            _ => None,
        })
        .collect()
}

/// Shows what the commit titled `subject` changed in `file`, as a click on the file's row does.
pub fn show_commit_diff(controller: &(impl AsWindow + ?Sized), subject: &str, file: &Path) {
    let controller = &controller.as_window();
    let (Some(panel), Some(hash)) = (panel::of(controller), commit_hash(controller, subject)) else {
        return;
    };
    panel.show_diff(&RowKey::CommitFile(hash, file.to_path_buf()), true);
}

/// Rows and hunks of the diff tab of what the commit titled `subject` changed in `file`.
pub fn commit_diff_counts(controller: &(impl AsWindow + ?Sized), subject: &str, file: &Path) -> Option<(usize, usize)> {
    let controller = &controller.as_window();
    let hash = commit_hash(controller, subject)?;
    super::diff::find(controller, &super::diff::commit_key(&hash, file)).map(|tab| tab.counts())
}

/// Stages the hunk at `index` in the diff tab of `path`, as its button does.
pub fn stage_hunk(controller: &(impl AsWindow + ?Sized), path: &Path, index: usize) {
    let controller = &controller.as_window();
    if let Some(tab) = super::diff::find(controller, &super::diff::key(path, false)) {
        tab.run_hunk(super::diff::HunkAction::Stage, index);
    }
}
