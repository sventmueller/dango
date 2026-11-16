//! The worktree threads in the Git panel (I1): a row per worktree with its branch, how far it is ahead of and behind
//! its base and how many files it changed, the files below it with their diff against the base, and taking its work
//! over, keeping its branch or removing it. A work order's worktree is the orchestrator's to take over or remove
//! (F50); its row only leads to the order in the Leitstand.

use ide_ui_common::window::Files;
use std::path::Path;

use block2::RcBlock;
use ide_git::{Change, SecretScan};
use objc2::rc::Retained;
use objc2::runtime::{Bool, Sel};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityCustomAction, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSButton,
    NSColor, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSLineBreakMode, NSStackView, NSTableCellView,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSArray;
use splat_i18n::{t, tf};

use super::model::RowKey;
use super::model::Task;
use super::panel::{GitPanel, with_panel};
use super::text::Problem;
use super::{sheets, work};
use crate::worktrees::{TakeOver, ThreadTree, TreeState};
use ide_ui_common::alert::{Alert, Role, first};
use ide_ui_common::util::{activate, label, ns, symbol};

/// The menu of a worktree row; `ordered` when a work order owns the worktree.
pub(crate) fn menu(keys: &[RowKey], ordered: bool) -> Vec<(&'static str, Option<Sel>)> {
    match keys {
        [RowKey::Worktree(_)] if ordered => vec![(t("Im Leitstand zeigen"), Some(sel!(ideGitShowOrder:)))],
        [RowKey::Worktree(_)] => vec![
            (t("Übernehmen …"), Some(sel!(ideGitTakeOver:))),
            (t("Thread öffnen"), Some(sel!(ideGitOpenThread:))),
            ("", None),
            (t("Als Branch behalten"), Some(sel!(ideGitKeepBranch:))),
            (t("Entfernen …"), Some(sel!(ideGitRemoveTree:))),
        ],
        [RowKey::WorktreeFile(..)] => vec![
            (t("Diff zeigen"), Some(sel!(ideGitDiff:))),
            (t("Im Editor öffnen"), Some(sel!(ideGitOpen:))),
        ],
        _ => vec![],
    }
}

/// The letter and the words for `change`.
pub(super) fn mark(change: Change) -> super::model::Mark {
    use super::model::Mark;
    match change {
        Change::Added => Mark::Added,
        Change::Deleted => Mark::Deleted,
        Change::Renamed { .. } => Mark::Renamed,
        Change::Copied { .. } => Mark::Copied,
        Change::Unmerged => Mark::Conflict,
        Change::Modified => Mark::Modified,
    }
}

/// What the row of `state` says, in one line.
fn summary(state: &TreeState) -> String {
    let files = match state.changed.len() {
        1 => t("1 Datei geändert").to_string(),
        n => tf("{} Dateien geändert", &[&n.to_string()]),
    };
    tf(
        "↑{} ↓{} · {} · {}",
        &[
            &state.ahead.to_string(),
            &state.behind.to_string(),
            &files,
            &state.tree.branch,
        ],
    )
}

impl GitPanel {
    /// The row of a thread's worktree: its title, then its branch, ahead and behind, and its changed files.
    pub(crate) fn tree_cell(&self, mtm: MainThreadMarker, state: &TreeState) -> Retained<NSView> {
        let cell = NSTableCellView::new(mtm);
        let icon = objc2_app_kit::NSImageView::new(mtm);
        if let Some(image) = symbol("arrow.triangle.branch", t("Worktree")) {
            icon.setImage(Some(&image));
        }
        icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let title = label(mtm, &state.tree.title);
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let detail = label(mtm, &summary(state));
        detail.setFont(Some(&ide_ui_common::style::small_system()));
        detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
        detail.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let lines = NSStackView::new(mtm);
        lines.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        lines.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        lines.setSpacing(ide_ui_common::style::space::HAIR);
        lines.addArrangedSubview(&title);
        lines.addArrangedSubview(&detail);
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(ide_ui_common::style::space::SNUG);
        row.addArrangedSubview(&icon);
        row.addArrangedSubview(&lines);
        cell.addSubview(&row);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            row.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
            row.trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
            row.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        ]);
        let described = tf("Worktree-Thread „{}“: {}", &[&state.tree.title, &summary(state)]);
        cell.setToolTip(Some(&ns(&format!("{described}\n{}", state.tree.path.display()))));
        cell.setAccessibilityLabel(Some(&ns(&described)));
        let key = RowKey::Worktree(state.tree.path.clone());
        let custom = self.custom_actions(&key, &menu(std::slice::from_ref(&key), state.tree.order.is_some()));
        cell.setAccessibilityCustomActions(Some(&NSArray::from_retained_slice(&custom)));
        // SAFETY: The label is a subview of the cell, which keeps it.
        unsafe { cell.setTextField(Some(&title)) };
        cell.into_super()
    }

    /// The row of a file a thread's worktree changed.
    pub(crate) fn tree_file_cell(
        &self,
        mtm: MainThreadMarker,
        tree: &Path,
        file: &Path,
        change: Change,
    ) -> Retained<NSView> {
        let cell = NSTableCellView::new(mtm);
        let mark = mark(change);
        let letter = label(mtm, mark.letter());
        letter.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
            ide_ui_common::style::SMALL,
            ide_ui_common::constant!(objc2_app_kit::NSFontWeightBold),
        )));
        letter.setTextColor(Some(&super::rows::mark_color(mark)));
        let name = label(
            mtm,
            &file
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        );
        name.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        let folder = label(
            mtm,
            &file.parent().map_or_else(String::new, |p| p.display().to_string()),
        );
        folder.setFont(Some(&ide_ui_common::style::small_system()));
        folder.setTextColor(Some(&NSColor::secondaryLabelColor()));
        folder.setLineBreakMode(NSLineBreakMode::ByTruncatingHead);
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(ide_ui_common::style::space::SNUG);
        for view in [&*letter as &NSView, &name, &folder] {
            row.addArrangedSubview(view);
        }
        letter.widthAnchor().constraintEqualToConstant(12.0).setActive(true);
        cell.addSubview(&row);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            row.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
            row.trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
            row.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        ]);
        let described = tf("{}, {}", &[&file.display().to_string(), mark.word()]);
        cell.setToolTip(Some(&ns(&format!("{described}\n{}", tree.join(file).display()))));
        cell.setAccessibilityLabel(Some(&ns(&described)));
        let key = RowKey::WorktreeFile(tree.to_path_buf(), file.to_path_buf());
        let custom = self.custom_actions(&key, &menu(std::slice::from_ref(&key), false));
        cell.setAccessibilityCustomActions(Some(&NSArray::from_retained_slice(&custom)));
        // SAFETY: The label is a subview of the cell, which keeps it.
        unsafe { cell.setTextField(Some(&name)) };
        cell.into_super()
    }

    /// The menu entries of `key` as actions for VoiceOver, which reaches no context menu.
    pub(super) fn custom_actions(
        &self,
        key: &RowKey,
        entries: &[(&'static str, Option<Sel>)],
    ) -> Vec<Retained<NSAccessibilityCustomAction>> {
        entries
            .iter()
            .filter_map(|(title, action)| Some((*title, (*action)?)))
            .map(|(title, action)| {
                let id = self.id();
                let key = key.clone();
                let handler = RcBlock::new(move || -> Bool {
                    ide_ui_common::util::guarded((), || {
                        with_panel(id, |p| p.run_row_action(action, vec![key.clone()]))
                    });
                    Bool::YES
                });
                NSAccessibilityCustomAction::initWithName_handler(
                    NSAccessibilityCustomAction::alloc(),
                    &ns(title),
                    Some(&handler),
                )
            })
            .collect()
    }

    /// The worktree a row belongs to, unless a work order owns it, whose orchestrator alone changes it (F50).
    fn thread_tree_of(&self, keys: &[RowKey]) -> Option<TreeState> {
        self.tree_of(keys).filter(|state| state.tree.order.is_none())
    }

    /// Shows the work order that owns the row's worktree in the Leitstand.
    pub(crate) fn show_order(&self, keys: &[RowKey]) {
        let Some(controller) = self.controller() else { return };
        if let Some(order) = self.tree_of(keys).and_then(|state| state.tree.order) {
            crate::host::host().show_order(controller.id(), &order);
        }
    }

    /// The thread worktree a row belongs to.
    pub(crate) fn tree_of(&self, keys: &[RowKey]) -> Option<TreeState> {
        let path = keys.iter().find_map(|key| match key {
            RowKey::Worktree(path) | RowKey::WorktreeFile(path, _) => Some(path.clone()),
            _ => None,
        })?;
        self.read().model.tree(&path).cloned()
    }

    /// Shows the diff of a worktree's file against where its branch left the base.
    pub(crate) fn show_tree_diff(&self, tree: &Path, file: &Path, preview: bool) {
        let (Some(controller), Some(state)) = (self.controller(), self.read().model.tree(tree).cloned()) else {
            return;
        };
        super::diff::show_since(&controller, tree, file, &state.since, &state.tree.title, preview);
    }

    /// Opens worktree files in the editor, which marks their tabs as the thread's copy.
    pub(crate) fn open_tree_files(&self, keys: &[RowKey]) {
        let Some(controller) = self.controller() else { return };
        for key in keys {
            if let RowKey::WorktreeFile(tree, file) = key {
                let full = tree.join(file);
                if full.is_file() {
                    controller.open_file(&full);
                }
            }
        }
    }

    /// Selects the thread that works in the worktree, or starts a new one there.
    pub(crate) fn open_thread(&self, keys: &[RowKey]) {
        let (Some(controller), Some(state)) = (self.controller(), self.thread_tree_of(keys)) else {
            return;
        };
        if crate::host::host().select_thread_in(&state.tree.path) {
            return;
        }
        crate::host::host().open_thread(controller.id(), &state.tree);
    }

    /// Asks how to take the thread's work over, then does it.
    pub(crate) fn take_over(&self, keys: &[RowKey]) {
        let (Some(window), Some(state)) = (self.controller().and_then(|c| c.window()), self.thread_tree_of(keys))
        else {
            return;
        };
        let into = self.read().branch.head.clone().unwrap_or_else(|| "HEAD".into());
        let id = self.id();
        let informative = tf(
            "Die Arbeit von {} kommt nach {}. Was der Thread nicht committet hat, committet die App vorher auf seinem Branch. Konflikte löst du danach wie bei jedem Merge.",
            &[&state.tree.branch, &into],
        );
        Alert::new(self.mtm(), &tf("„{}“ übernehmen?", &[&state.tree.title]))
            .informative(&informative)
            .button(t("Zusammenführen"), Role::Plain)
            .button(t("Commits einzeln übernehmen"), Role::Plain)
            .button(t("Abbrechen"), Role::Plain)
            .sheet(&window, move |response| {
                let how = if response == NSAlertFirstButtonReturn {
                    TakeOver::Merge
                } else if response == NSAlertSecondButtonReturn {
                    TakeOver::CherryPick
                } else {
                    return;
                };
                with_panel(id, |panel| panel.checked(state.tree, Step::TakeOver(how)));
            });
    }

    /// Removes the worktree but keeps its branch, after committing what the thread left there.
    pub(crate) fn keep_branch(&self, keys: &[RowKey]) {
        let Some(state) = self.thread_tree_of(keys) else { return };
        if self.refuse_while_open(&state.tree) {
            return;
        }
        self.checked(state.tree, Step::Keep);
    }

    /// Asks before removing the worktree and perhaps its branch (E27), then does it.
    pub(crate) fn remove_tree(&self, keys: &[RowKey]) {
        let (Some(window), Some(state)) = (self.controller().and_then(|c| c.window()), self.thread_tree_of(keys))
        else {
            return;
        };
        if self.refuse_while_open(&state.tree) {
            return;
        }
        // SAFETY: A checkbox without target and action only keeps its state, which the sheet reads at the end.
        let branch = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &ns(&tf("Auch den Branch {} löschen", &[&state.tree.branch])),
                None,
                None,
                self.mtm(),
            )
        };
        let merged = state.ahead == 0 && state.changed.is_empty();
        branch.setState(if merged {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        let id = self.id();
        let informative = t(
            "Der Ordner des Worktrees wird gelöscht, mit allem, was der Thread darin nicht committet hat. Das lässt sich nicht widerrufen.",
        );
        Alert::new(self.mtm(), &tf("Worktree „{}“ entfernen?", &[&state.tree.title]))
            .warning()
            .informative(informative)
            .button(t("Entfernen"), Role::Destructive)
            .button(t("Abbrechen"), Role::Plain)
            .accessory(&branch)
            .sheet(&window, move |response| {
                if first(response) {
                    let delete = branch.state() == NSControlStateValueOn;
                    with_panel(id, |panel| panel.removal_confirmed(state.tree, delete));
                }
            });
    }

    /// Refuses to take a worktree away from a thread that still works there; once it is closed, the refusal goes.
    fn refuse_while_open(&self, tree: &ThreadTree) -> bool {
        let open = self
            .controller()
            .is_some_and(|controller| crate::host::host().thread_works_in(controller.id(), &tree.path));
        let refusal = tf(
            "Der Thread „{}“ ist noch offen. Schließ seinen Tab, bevor sein Worktree verschwindet.",
            &[&tree.title],
        );
        if open {
            self.set_problem(Some(Problem::plain(refusal)));
        } else if self.read().problem.as_ref().is_some_and(|p| p.message == refusal) {
            self.set_problem(None);
        }
        open
    }

    /// Stages and checks for secrets what the thread left uncommitted, asks when something turns up, then goes on
    /// with `step`.
    fn checked(&self, tree: ThreadTree, step: Step) {
        if !self.begin(Task::TakeOver, None) {
            return;
        }
        let id = self.id();
        let scanned = tree.clone();
        work::long(
            move || {
                let git = work::git().ok_or(ide_git::Error::GitMissing)?;
                crate::worktrees::stage_pending(&git, &scanned)
            },
            move |scan| {
                with_panel(id, |panel| {
                    panel.end();
                    let action = match step {
                        Step::TakeOver(_) => t("Trotzdem übernehmen"),
                        Step::Keep => t("Trotzdem behalten"),
                    };
                    let go_on = move |agreed: bool| {
                        if agreed {
                            with_panel(id, |panel| panel.carry_out(tree, step));
                        }
                    };
                    let window = panel.controller().and_then(|c| c.window());
                    match (scan, window) {
                        (Some(Ok(SecretScan::NotInstalled | SecretScan::Clean)), _) => go_on(true),
                        (Some(Ok(SecretScan::Found(findings))), Some(window)) => {
                            sheets::secrets(&window, &findings, action, go_on);
                        }
                        (Some(Ok(SecretScan::Found(_))), None) => {}
                        (Some(Err(error)), _) => panel.report(&error),
                        (None, _) => panel.set_problem(Some(Problem::plain(t("Git ist unerwartet abgebrochen.")))),
                    }
                });
            },
        );
    }

    fn carry_out(&self, tree: ThreadTree, step: Step) {
        let Some(repo) = self.repo() else { return };
        if !self.begin(Task::TakeOver, None) {
            return;
        }
        let id = self.id();
        work::long(
            move || {
                let git = work::git().ok_or(ide_git::Error::GitMissing)?;
                match step {
                    Step::TakeOver(how) => crate::worktrees::take_over(&git, &repo, &tree, how),
                    Step::Keep => crate::worktrees::keep_branch(&git, &repo, &tree),
                }
            },
            move |result| {
                with_panel(id, |panel| {
                    panel.end();
                    panel.local_done(result);
                });
            },
        );
    }

    /// Makes a worktree for a thread of `agent` titled `title` and opens the thread there.
    pub(crate) fn create_tree(&self, title: String, agent: String) {
        let Some(repo) = self.repo() else { return };
        let root = self.root().to_path_buf();
        let id = self.id();
        let queued = work::local(
            move || crate::worktrees::create(&repo, &root, &title, &agent),
            move |made| with_panel(id, |panel| panel.tree_created(made)),
        );
        if !queued {
            self.set_problem(Some(Problem::plain(t(
                "Git ist gerade ausgelastet. Versuch es gleich noch einmal.",
            ))));
        }
    }

    fn tree_created(&self, made: Option<Result<ThreadTree, crate::worktrees::TreeError>>) {
        use crate::worktrees::TreeError;
        match made {
            Some(Ok(tree)) => {
                self.start_refresh();
                if let Some(controller) = self.controller() {
                    crate::host::host().open_thread(controller.id(), &tree);
                }
            }
            Some(Err(TreeError::NoCommit | TreeError::NoBranch(_))) => {
                self.set_problem(Some(Problem::plain(t("Ein Worktree braucht einen ersten Commit."))));
            }
            Some(Err(TreeError::Git(error))) => self.report(&error),
            Some(Err(TreeError::Io(error))) => {
                self.set_problem(Some(Problem::plain(ide_core::explain::Explain::explain(&error))))
            }
            None => self.set_problem(Some(Problem::plain(t("Git ist unerwartet abgebrochen.")))),
        }
    }

    fn removal_confirmed(&self, tree: ThreadTree, delete_branch: bool) {
        self.run_local(move |repo| crate::worktrees::remove(repo, &tree, delete_branch));
    }
}

/// What comes after the secrets check.
#[derive(Debug, Clone, Copy)]
enum Step {
    TakeOver(TakeOver),
    Keep,
}
