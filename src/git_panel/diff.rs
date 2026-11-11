//! The diff tab: one file's staged or unstaged changes, read only, with buttons that stage, unstage or discard a single
//! hunk (E11); or, without buttons, what a thread's worktree changed in a file since its branch left the base (I1) or
//! what a commit of the history changed in it.

use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ide_git::{Change, DiffSource, Error, FileDiff, FileState, LineKind, StatusOptions};
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSPasteboard, NSPasteboardTypeString, NSTableColumn, NSTableRowView,
    NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::diff_view::{self, Row, Ui};
use super::text::{Problem, problem};
use super::{sheets, work};
use crate::util::{constant, guarded, ns};
use crate::window::{TabKind, WindowController};

/// What a hunk button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkAction {
    Stage,
    Unstage,
    Discard,
}

#[derive(Default)]
pub struct State {
    pub diff: Option<FileDiff>,
    pub rows: Vec<Row>,
    loading: bool,
    again: bool,
    busy: bool,
    loaded: bool,
}

pub struct DiffIvars {
    id: u64,
    controller: Weak<WindowController>,
    /// The window's folder, whose Git panel learns of changes made here.
    window_root: PathBuf,
    repo_root: PathBuf,
    /// The file, relative to the repository's root.
    path: PathBuf,
    staged: bool,
    /// The commit a worktree's changes are shown against, which moves as its branch and the base do, and the thread's
    /// title; `None` for the index's sides.
    since: Option<(RefCell<String>, String)>,
    /// The commit whose change of the file shows.
    commit: Option<String>,
    ui: OnceCell<Ui>,
    state: RefCell<State>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `DiffTab` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitDiffTab"]
    #[ivars = DiffIvars]
    pub struct DiffTab;

    unsafe impl NSObjectProtocol for DiffTab {}

    unsafe impl NSTableViewDataSource for DiffTab {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().state.borrow().rows.len()).unwrap_or(0))
        }
    }

    unsafe impl NSControlTextEditingDelegate for DiffTab {}

    unsafe impl NSTableViewDelegate for DiffTab {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let state = self.ivars().state.borrow();
                let entry = *state.rows.get(usize::try_from(row).ok()?)?;
                let diff = state.diff.as_ref()?;
                let ui = self.ivars().ui.get()?;
                match (entry, column) {
                    (Row::Hunk(index), _) => Some(diff_view::hunk_header(
                        self,
                        diff,
                        index,
                        self.ivars().staged,
                        self.ivars().since.is_some() || self.ivars().commit.is_some(),
                    )),
                    (Row::Line(hunk, line), Some(column)) => {
                        let line = diff.hunks.get(hunk)?.lines.get(line)?;
                        Some(ui.line_cell(table, &column.identifier().to_string(), line))
                    }
                    (Row::Line(..), None) => None,
                }
            })
        }

        #[unsafe(method(tableView:isGroupRow:))]
        fn is_group_row(&self, _table: &NSTableView, row: isize) -> bool {
            guarded(false, || matches!(self.row(row), Some(Row::Hunk(_))))
        }

        #[unsafe(method(tableView:heightOfRow:))]
        fn height_of_row(&self, _table: &NSTableView, row: isize) -> f64 {
            guarded(20.0, || {
                let line = self.ivars().ui.get().map_or(18.0, Ui::line_height);
                match self.row(row) {
                    Some(Row::Hunk(_)) => diff_view::HUNK_HEIGHT,
                    _ => line,
                }
            })
        }

        #[unsafe(method(tableView:didAddRowView:forRow:))]
        fn did_add_row_view(&self, _table: &NSTableView, view: &NSTableRowView, row: isize) {
            guarded((), || {
                let kind = self.line_kind(row);
                if let Some(color) = kind.and_then(diff_view::background) {
                    view.setBackgroundColor(&color);
                }
            });
        }
    }

    impl DiffTab {
        #[unsafe(method(ideDiffStageHunk:))]
        fn stage_hunk_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.hunk_action(HunkAction::Stage, tag(sender)));
        }

        #[unsafe(method(ideDiffUnstageHunk:))]
        fn unstage_hunk_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.hunk_action(HunkAction::Unstage, tag(sender)));
        }

        #[unsafe(method(ideDiffDiscardHunk:))]
        fn discard_hunk_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.hunk_action(HunkAction::Discard, tag(sender)));
        }

        #[unsafe(method(ideDiffOpenFile:))]
        fn open_file_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let full = self.ivars().repo_root.join(&self.ivars().path);
                if let Some(controller) = self.ivars().controller.load()
                    && full.is_file()
                {
                    controller.open_file(&full);
                }
            });
        }

        #[unsafe(method(copy:))]
        fn copy(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.copy_selection());
        }
    }
);

thread_local! {
    static TABS: RefCell<Vec<(u64, Weak<DiffTab>)>> = const { RefCell::new(Vec::new()) };
}

fn with_tab(id: u64, f: impl FnOnce(&DiffTab)) {
    let tab = TABS.with(|tabs| tabs.borrow().iter().find(|(i, _)| *i == id).and_then(|(_, t)| t.load()));
    if let Some(tab) = tab {
        f(&tab);
    }
}

fn live_tabs() -> Vec<Retained<DiffTab>> {
    TABS.with(|tabs| {
        let mut tabs = tabs.borrow_mut();
        tabs.retain(|(_, tab)| tab.load().is_some());
        tabs.iter().filter_map(|(_, tab)| tab.load()).collect()
    })
}

/// Keeps the worktree diff tabs of `controller`'s window in step with `trees` as the Git panel read them: a tab whose
/// worktree is gone or whose file it no longer changes, as after taking its work over, closes; the others compare
/// against where the branch and the base meet now.
pub fn follow_trees(controller: &WindowController, trees: &[crate::worktrees::TreeState]) {
    let mut gone = Vec::new();
    for tab in live_tabs() {
        let ivars = tab.ivars();
        let Some((base, _)) = &ivars.since else { continue };
        if !ivars.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller)) {
            continue;
        }
        let tree = trees.iter().find(|t| t.tree.path == ivars.repo_root);
        match tree.filter(|t| t.changed.iter().any(|f| f.path == ivars.path)) {
            Some(tree) => base.borrow_mut().clone_from(&tree.since),
            None => gone.push(since_key(&ivars.repo_root, &ivars.path)),
        }
    }
    for key in gone {
        if let Some(id) = controller.tab_with_key(&key) {
            controller.close_tab(id);
        }
    }
}

/// Reads the diffs of the tabs of the window that shows `window_root` again.
pub fn refresh_for(window_root: &Path) {
    for tab in live_tabs() {
        if tab.ivars().window_root == window_root {
            tab.reload();
        }
    }
}

/// The key of the tab that shows `path`'s staged or unstaged changes.
pub fn key(path: &Path, staged: bool) -> String {
    format!("diff:{}:{}", if staged { "staged" } else { "unstaged" }, path.display())
}

/// Shows the diff of `path`, relative to `repo_root`, in its tab, opening one if there is none; `preview` opens it
/// in the window's preview tab and leaves an open tab as it is, else an open preview tab is kept.
pub fn show(controller: &WindowController, repo_root: &Path, path: &Path, staged: bool, preview: bool) {
    let key = key(path, staged);
    if let Some(id) = controller.tab_with_key(&key) {
        if !preview {
            controller.keep_tab(id);
        }
        controller.select_tab(&key);
        return;
    }
    let Some(window_root) = controller.project_root() else {
        return;
    };
    let window_root = window_root.canonicalize().unwrap_or(window_root);
    let tab = DiffTab::new(
        controller,
        window_root,
        repo_root.to_path_buf(),
        path.to_path_buf(),
        staged,
        None,
        None,
    );
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let title = if staged {
        tf("{} (vorgemerkt)", &[&name])
    } else {
        tf("{} (Änderungen)", &[&name])
    };
    place(controller, tab, key, &title, preview);
}

/// The key of the tab that shows what the worktree at `tree` changed in `path`.
pub fn since_key(tree: &Path, path: &Path) -> String {
    format!("diff:since:{}:{}", tree.display(), path.display())
}

/// Shows what the thread `title` changed in `path` of its worktree at `tree` since `base`, like [`show`].
pub fn show_since(controller: &WindowController, tree: &Path, path: &Path, base: &str, title: &str, preview: bool) {
    let key = since_key(tree, path);
    if let Some(id) = controller.tab_with_key(&key) {
        if !preview {
            controller.keep_tab(id);
        }
        controller.select_tab(&key);
        return;
    }
    let Some(window_root) = controller.project_root() else {
        return;
    };
    let window_root = window_root.canonicalize().unwrap_or(window_root);
    let since = Some((RefCell::new(base.to_string()), title.to_string()));
    let tab = DiffTab::new(
        controller,
        window_root,
        tree.to_path_buf(),
        path.to_path_buf(),
        false,
        since,
        None,
    );
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    place(controller, tab, key, &tf("{} (Thread)", &[&name]), preview);
}

/// The key of the tab that shows what the commit `hash` changed in `path`.
pub fn commit_key(hash: &str, path: &Path) -> String {
    format!("diff:commit:{hash}:{}", path.display())
}

/// Shows what the commit `hash` changed in `path`, relative to `repo_root`, like [`show`].
pub fn show_commit(controller: &WindowController, repo_root: &Path, hash: &str, path: &Path, preview: bool) {
    let key = commit_key(hash, path);
    if let Some(id) = controller.tab_with_key(&key) {
        if !preview {
            controller.keep_tab(id);
        }
        controller.select_tab(&key);
        return;
    }
    let Some(window_root) = controller.project_root() else {
        return;
    };
    let window_root = window_root.canonicalize().unwrap_or(window_root);
    let tab = DiffTab::new(
        controller,
        window_root,
        repo_root.to_path_buf(),
        path.to_path_buf(),
        false,
        None,
        Some(hash.to_string()),
    );
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let short: String = hash.chars().take(7).collect();
    place(controller, tab, key, &tf("{} ({})", &[&name, &short]), preview);
}

fn place(controller: &WindowController, tab: Retained<DiffTab>, key: String, title: &str, preview: bool) {
    TABS.with(|tabs| tabs.borrow_mut().push((tab.ivars().id, Weak::from(&*tab))));
    tab.reload();
    if preview {
        controller.add_preview_tab(TabKind::Diff, Some(key), title, tab.into_super());
    } else {
        controller.add_tab(TabKind::Diff, Some(key), title, tab.into_super());
    }
}

impl DiffTab {
    fn new(
        controller: &WindowController,
        window_root: PathBuf,
        repo_root: PathBuf,
        path: PathBuf,
        staged: bool,
        since: Option<(RefCell<String>, String)>,
        commit: Option<String>,
    ) -> Retained<DiffTab> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mtm: MainThreadMarker = controller.mtm();
        let ivars = DiffIvars {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            controller: Weak::from(controller),
            window_root,
            repo_root,
            path,
            staged,
            since,
            commit,
            ui: OnceCell::new(),
            state: RefCell::new(State::default()),
        };
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let tab: Retained<DiffTab> =
            unsafe { msg_send![super(DiffTab::alloc(mtm).set_ivars(ivars)), initWithFrame: frame] };
        let ivars = tab.ivars();
        let side = match (&ivars.since, &ivars.commit) {
            (_, Some(hash)) => tf("Commit {}", &[&hash.chars().take(7).collect::<String>()]),
            (Some((_, title)), None) => tf("Was der Thread „{}“ geändert hat", &[title]),
            (None, None) if ivars.staged => t("Vorgemerkte Änderungen").to_string(),
            (None, None) => t("Nicht vorgemerkte Änderungen").to_string(),
        };
        let _ = ivars.ui.set(Ui::new(mtm, &tab, &ivars.path, &side));
        tab
    }

    fn ui(&self) -> &Ui {
        self.ivars().ui.get().expect("the tab builds its views in `new`")
    }

    fn row(&self, row: isize) -> Option<Row> {
        let state = self.ivars().state.borrow();
        usize::try_from(row).ok().and_then(|r| state.rows.get(r).copied())
    }

    fn line_kind(&self, row: isize) -> Option<LineKind> {
        let Some(Row::Line(hunk, line)) = self.row(row) else {
            return None;
        };
        let state = self.ivars().state.borrow();
        Some(state.diff.as_ref()?.hunks.get(hunk)?.lines.get(line)?.kind)
    }

    /// Reads the diff again, or once more after the read that runs.
    pub fn reload(&self) {
        {
            let mut state = self.ivars().state.borrow_mut();
            if state.loading {
                state.again = true;
                return;
            }
            state.loading = true;
        }
        let ivars = self.ivars();
        let (root, path, staged, id) = (ivars.repo_root.clone(), ivars.path.clone(), ivars.staged, ivars.id);
        let since = ivars.since.as_ref().map(|(base, _)| base.borrow().clone());
        let commit = ivars.commit.clone();
        let queued = work::local(
            move || match (since, commit) {
                (_, Some(hash)) => load_commit(&root, &path, &hash),
                (Some(base), None) => load_since(&root, &path, &base),
                (None, None) => load(&root, &path, staged),
            },
            move |result| with_tab(id, |tab| tab.loaded(result)),
        );
        if !queued {
            self.ivars().state.borrow_mut().loading = false;
        }
    }

    fn loaded(&self, result: Option<Result<Option<FileDiff>, Problem>>) {
        let again = {
            let mut state = self.ivars().state.borrow_mut();
            state.loading = false;
            std::mem::take(&mut state.again)
        };
        match result {
            Some(Ok(diff)) => self.show_diff(diff),
            Some(Err(problem)) => self.ui().show_message(Some(&problem.message)),
            None => self.ui().show_message(Some(t("Git ist unerwartet abgebrochen."))),
        }
        if again {
            self.reload();
        }
    }

    fn show_diff(&self, diff: Option<FileDiff>) {
        {
            let mut state = self.ivars().state.borrow_mut();
            if state.loaded && state.diff == diff {
                return;
            }
            state.loaded = true;
            state.rows = diff.as_ref().map(diff_view::rows).unwrap_or_default();
            state.diff = diff;
        }
        let state = self.ivars().state.borrow();
        let ui = self.ui();
        ui.show_notice(diff_view::notice(state.diff.as_ref(), self.ivars().staged));
        ui.fit_text_column(state.diff.as_ref());
        drop(state);
        ui.table.reloadData();
    }

    fn hunk_action(&self, action: HunkAction, index: Option<usize>) {
        let Some(index) = index else { return };
        if action != HunkAction::Discard {
            self.run_hunk(action, index);
            return;
        }
        let Some(window) = self.window() else { return };
        let id = self.ivars().id;
        sheets::confirm(
            &window,
            t("Abschnitt verwerfen?"),
            t("Die Änderungen dieses Abschnitts gehen verloren. Das lässt sich nicht widerrufen."),
            t("Verwerfen"),
            move || with_tab(id, |tab| tab.run_hunk(HunkAction::Discard, index)),
        );
    }

    /// Stages, unstages or discards the hunk at `index` of the diff on view.
    pub fn run_hunk(&self, action: HunkAction, index: usize) {
        let diff = {
            let mut state = self.ivars().state.borrow_mut();
            if state.busy {
                return;
            }
            let Some(diff) = state.diff.clone() else { return };
            state.busy = true;
            diff
        };
        let root = self.ivars().repo_root.clone();
        let id = self.ivars().id;
        let queued = work::local(
            move || {
                let repo = work::git().ok_or(Error::GitMissing)?.discover(&root)?;
                match action {
                    HunkAction::Stage => repo.stage_hunks(&diff, &[index]),
                    HunkAction::Unstage => repo.unstage_hunks(&diff, &[index]),
                    HunkAction::Discard => repo.discard_hunks(&diff, &[index]),
                }
            },
            move |result| with_tab(id, |tab| tab.hunk_done(result)),
        );
        if !queued {
            self.ivars().state.borrow_mut().busy = false;
        }
    }

    fn hunk_done(&self, result: Option<ide_git::Result<()>>) {
        self.ivars().state.borrow_mut().busy = false;
        match result {
            Some(Ok(())) => self.ui().show_message(None),
            Some(Err(error)) => self.ui().show_message(Some(&problem(&error).message)),
            None => self.ui().show_message(Some(t("Git ist unerwartet abgebrochen."))),
        }
        self.reload();
        for panel in super::panel::for_root(&self.ivars().window_root) {
            panel.start_refresh();
        }
    }

    /// Copies the selected lines with their markers.
    fn copy_selection(&self) {
        let ui = self.ui();
        let state = self.ivars().state.borrow();
        let Some(diff) = state.diff.as_ref() else { return };
        let mut text = String::new();
        for (row, entry) in state.rows.iter().enumerate() {
            if !isize::try_from(row).is_ok_and(|r| ui.table.isRowSelected(r)) {
                continue;
            }
            match *entry {
                Row::Hunk(index) => text.push_str(&diff_view::hunk_title(&diff.hunks[index])),
                Row::Line(hunk, line) => {
                    let line = &diff.hunks[hunk].lines[line];
                    text.push_str(diff_view::marker(line.kind));
                    text.push_str(&line.text());
                }
            }
            text.push('\n');
        }
        if text.is_empty() {
            return;
        }
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        pasteboard.setString_forType(&ns(&text), constant!(NSPasteboardTypeString));
    }
}

/// The hunk a button stands for, from its tag.
fn tag(sender: Option<&AnyObject>) -> Option<usize> {
    sender
        .and_then(|s| s.downcast_ref::<NSControl>())
        .and_then(|c| usize::try_from(c.tag()).ok())
}

/// The diff of `path` on the git lane; `None` when the file has no changes on this side any more.
fn load(root: &Path, path: &Path, staged: bool) -> Result<Option<FileDiff>, Problem> {
    let read = || -> ide_git::Result<Option<FileDiff>> {
        let repo = work::git().ok_or(Error::GitMissing)?.discover(root)?;
        let status = repo.status(StatusOptions::default())?;
        let Some(entry) = status.entries.iter().find(|e| e.path == path) else {
            return Ok(None);
        };
        let side = if staged { entry.index } else { entry.worktree };
        let listed = !staged
            && matches!(
                entry.kind,
                ide_git::EntryKind::Untracked | ide_git::EntryKind::Unmerged(_)
            );
        if side == FileState::Unmodified && !listed {
            return Ok(None);
        }
        let source = if staged {
            DiffSource::Staged
        } else {
            DiffSource::Unstaged
        };
        repo.diff_entry(entry, source)
    };
    read().map_err(|error| problem(&error))
}

/// What the worktree at `root` changed in `path` since `base`; a file new to it shows as new.
fn load_since(root: &Path, path: &Path, base: &str) -> Result<Option<FileDiff>, Problem> {
    let read = || -> ide_git::Result<Option<FileDiff>> {
        let repo = work::git().ok_or(Error::GitMissing)?.discover(root)?;
        let diffs = repo.diff_since(base, &[root.join(path)])?;
        Ok(diffs.into_iter().find(|d| d.path() == path))
    };
    read().map_err(|error| problem(&error))
}

/// What the commit `hash` changed in `path`.
fn load_commit(root: &Path, path: &Path, hash: &str) -> Result<Option<FileDiff>, Problem> {
    let read = || -> ide_git::Result<Option<FileDiff>> {
        let repo = work::git().ok_or(Error::GitMissing)?.discover(root)?;
        repo.diff_commit(hash, path)
    };
    read().map_err(|error| problem(&error))
}

/// Whether the unstaged side of `diff` can drop a hunk: a new file goes to the Trash as a whole instead.
pub fn can_discard(diff: &FileDiff) -> bool {
    diff.change != Change::Added
}

#[cfg(feature = "selftest")]
impl DiffTab {
    /// Rows and hunks on view.
    pub fn counts(&self) -> (usize, usize) {
        let state = self.ivars().state.borrow();
        (state.rows.len(), state.diff.as_ref().map_or(0, |d| d.hunks.len()))
    }
}

/// The diff tab showing `key` in `controller`'s window.
#[cfg(feature = "selftest")]
pub fn find(controller: &WindowController, key: &str) -> Option<Retained<DiffTab>> {
    live_tabs().into_iter().find(|tab| {
        let ivars = tab.ivars();
        let own = match (&ivars.since, &ivars.commit) {
            (_, Some(hash)) => commit_key(hash, &ivars.path),
            (Some(_), None) => since_key(&ivars.repo_root, &ivars.path),
            (None, None) => self::key(&ivars.path, ivars.staged),
        };
        own == key && ivars.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller))
    })
}
