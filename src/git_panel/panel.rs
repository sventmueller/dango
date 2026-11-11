//! The Git panel of one window: its state, the refresh after changes on disk, and the AppKit callbacks of its views.

use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ide_git::{Branch, BranchStatus, Cancel, Commit, Operation, Repository, Status, StatusOptions};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSTableRowView, NSTextDelegate, NSTextViewDelegate, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSString};

use super::model::{Model, RowKey, Section};
use super::text::{Problem, problem};
use super::ui::Ui;
use super::work;
use crate::util::{guarded, main_after, ns};
use crate::window::{Panel, WindowController};

/// How long changes on disk gather before the panel reads git again.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// How many commits the graph shows at first, and how many more each time the user asks.
pub const HISTORY_PAGE: usize = 50;

/// How often the ages of the commits on view are said again.
const AGE_INTERVAL: Duration = Duration::from_secs(60);

/// What the panel can show for its folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Loading,
    Untrusted,
    NoGit,
    NotRepository,
    Ready,
    Failed(String),
}

/// Something running that keeps other operations from starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    Commit,
    Scan,
    Fetch,
    Pull,
    Push,
    Switch,
    Continue,
    Abort,
    /// A thread's worktree is taken over, kept as a branch or removed.
    TakeOver,
}

pub struct Busy {
    pub task: Task,
    pub cancel: Option<Cancel>,
}

#[derive(Default)]
struct Refresh {
    scheduled: bool,
    running: bool,
    again: bool,
}

#[derive(Default)]
pub struct State {
    pub phase: Phase,
    pub repo: Option<Repository>,
    pub branch: BranchStatus,
    pub branches: Vec<Branch>,
    /// The merge or rebase that stopped at conflicts.
    pub operation: Option<Operation>,
    pub model: Model,
    pub collapsed: HashSet<Section>,
    /// The thread worktrees whose files show.
    pub open_trees: HashSet<PathBuf>,
    /// The commits whose files show.
    pub open_commits: HashSet<String>,
    /// What commits changed, by hash, as far as read.
    pub commit_changes: HashMap<String, ide_git::CommitChanges>,
    /// The commits whose changes are being read.
    pub reading_changes: HashSet<String>,
    /// How many more pages of history the user asked for.
    pub more_pages: usize,
    /// What follows the commit that runs: a push, or a pull and then a push.
    pub after_commit: Option<Task>,
    /// Whether a push follows the pull that runs.
    pub push_after_pull: bool,
    pub busy: Option<Busy>,
    pub problem: Option<Problem>,
    /// The rows the open context menu acts on.
    pub menu_keys: Vec<RowKey>,
    refresh: Refresh,
}

pub struct PanelIvars {
    id: u64,
    controller: Weak<WindowController>,
    /// The window's folder, which may lie below the repository's root.
    root: PathBuf,
    ui: OnceCell<Ui>,
    state: RefCell<State>,
    /// The objects that stand for rows in the outline view, which keeps no reference of its own.
    items: RefCell<HashMap<String, Retained<NSString>>>,
}

/// What a refresh read from git.
pub enum Loaded {
    NoGit,
    NotRepository,
    Failed(Problem),
    Ready {
        repo: Repository,
        status: Box<Status>,
        branches: Vec<Branch>,
        log: Vec<Commit>,
        operation: Option<Operation>,
        worktrees: Vec<crate::worktrees::TreeState>,
    },
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `GitPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitPanel"]
    #[ivars = PanelIvars]
    pub struct GitPanel;

    unsafe impl NSObjectProtocol for GitPanel {}

    unsafe impl NSOutlineViewDataSource for GitPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.children(item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let children = self.children(item);
                let key = children.get(usize::try_from(index).ok()?)?;
                Some(self.item_for(key))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || match key_of(item) {
                Some(RowKey::Section(_)) => true,
                Some(RowKey::Worktree(path)) => !self.read().model.tree_keys(&path).is_empty(),
                Some(RowKey::Commit(_)) => true,
                _ => false,
            })
        }
    }

    unsafe impl NSControlTextEditingDelegate for GitPanel {}

    unsafe impl NSOutlineViewDelegate for GitPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.row_view(&key_of(item)?))
        }

        #[unsafe(method_id(outlineView:rowViewForItem:))]
        fn row_view_for(&self, view: &NSOutlineView, item: &AnyObject) -> Option<Retained<NSTableRowView>> {
            guarded(None, || match key_of(item)? {
                RowKey::Section(_) => None,
                _ => Some(Retained::into_super(super::graph::GraphRowView::new(view.mtm()))),
            })
        }

        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn row_height(&self, _view: &NSOutlineView, item: &AnyObject) -> f64 {
            guarded(24.0, || super::rows::height(key_of(item).as_ref()))
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || self.note_expansion(notification, false));
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &NSNotification) {
            guarded((), || self.note_expansion(notification, true));
        }
    }

    unsafe impl NSMenuDelegate for GitPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    unsafe impl NSTextDelegate for GitPanel {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            guarded((), || self.update_placeholder());
        }
    }

    unsafe impl NSTextViewDelegate for GitPanel {}

    impl GitPanel {
        #[unsafe(method(ideGitStage:))]
        fn stage_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.stage(self.keys_for(sender)));
        }

        #[unsafe(method(ideGitUnstage:))]
        fn unstage_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.unstage(self.keys_for(sender)));
        }

        #[unsafe(method(ideGitDiscard:))]
        fn discard_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.discard(self.keys_for(sender)));
        }

        #[unsafe(method(ideGitOpen:))]
        fn open_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.open(self.keys_for(sender)));
        }

        #[unsafe(method(ideGitResolve:))]
        fn resolve_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.resolve(self.keys_for(sender)));
        }

        #[unsafe(method(ideGitContinue:))]
        fn continue_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.continue_operation());
        }

        #[unsafe(method(ideGitAbort:))]
        fn abort_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.abort_operation());
        }

        #[unsafe(method(ideGitDiff:))]
        fn diff_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(key) = self.keys_for(sender).first() {
                    self.show_diff(key, false);
                }
            });
        }

        #[unsafe(method(ideGitTakeOver:))]
        fn take_over_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.take_over(&self.keys_for(sender)));
        }

        #[unsafe(method(ideGitOpenThread:))]
        fn open_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.open_thread(&self.keys_for(sender)));
        }

        #[unsafe(method(ideGitKeepBranch:))]
        fn keep_branch_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.keep_branch(&self.keys_for(sender)));
        }

        #[unsafe(method(ideGitRemoveTree:))]
        fn remove_tree_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.remove_tree(&self.keys_for(sender)));
        }

        #[unsafe(method(ideGitCopyHash:))]
        fn copy_hash_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.run_history_action(objc2::sel!(ideGitCopyHash:), &self.keys_for(sender)));
        }

        #[unsafe(method(ideGitCopyMessage:))]
        fn copy_message_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.run_history_action(objc2::sel!(ideGitCopyMessage:), &self.keys_for(sender)));
        }

        #[unsafe(method(ideGitBranchHere:))]
        fn branch_here_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.run_history_action(objc2::sel!(ideGitBranchHere:), &self.keys_for(sender)));
        }

        #[unsafe(method(ideGitCommitPush:))]
        fn commit_push_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.commit_then(Some(Task::Push)));
        }

        #[unsafe(method(ideGitCommitSync:))]
        fn commit_sync_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.commit_then(Some(Task::Pull)));
        }

        #[unsafe(method(ideGitRowClicked:))]
        fn clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.preview_row(self.ui().outline.clickedRow()));
        }

        #[unsafe(method(ideGitRowDoubleClicked:))]
        fn double_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.activate_row(self.ui().outline.clickedRow()));
        }

        #[unsafe(method(ideGitStageAll:))]
        fn stage_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.stage_all());
        }

        #[unsafe(method(ideGitUnstageAll:))]
        fn unstage_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.unstage_all());
        }

        #[unsafe(method(ideGitCommit:))]
        fn commit_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.commit());
        }

        #[unsafe(method(ideGitAmend:))]
        fn amend_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.update_placeholder());
        }

        #[unsafe(method(ideGitFetch:))]
        fn fetch_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remote(Task::Fetch));
        }

        #[unsafe(method(ideGitPull:))]
        fn pull_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remote(Task::Pull));
        }

        #[unsafe(method(ideGitPush:))]
        fn push_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remote(Task::Push));
        }

        #[unsafe(method(ideGitCancel:))]
        fn cancel_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(cancel) = self.state().busy.as_ref().and_then(|b| b.cancel.clone()) {
                    cancel.cancel();
                }
            });
        }

        #[unsafe(method(ideGitBranch:))]
        fn branch_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.branch_chosen(sender));
        }

        #[unsafe(method(ideGitNewBranch:))]
        fn new_branch_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.new_branch());
        }

        #[unsafe(method(ideGitInit:))]
        fn init_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.init_repository());
        }

        #[unsafe(method(ideGitRetry:))]
        fn retry_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.start_refresh());
        }

        #[unsafe(method(ideGitTrust:))]
        fn trust_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(controller) = self.controller() {
                    controller.ask_trust_again();
                }
            });
        }

        #[unsafe(method(ideGitShowOutput:))]
        fn show_output_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.show_problem_output());
        }

        #[unsafe(method(ideGitDismissProblem:))]
        fn dismiss_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.set_problem(None));
        }
    }
);

thread_local! {
    static PANELS: RefCell<Vec<Retained<GitPanel>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` with the panel `id`, if its window is still open.
pub fn with_panel(id: u64, f: impl FnOnce(&GitPanel)) {
    let panel = PANELS.with(|panels| panels.borrow().iter().find(|p| p.ivars().id == id).cloned());
    if let Some(panel) = panel {
        f(&panel);
    }
}

/// The panels of windows that show `root`.
pub fn for_root(root: &Path) -> Vec<Retained<GitPanel>> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    PANELS.with(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|p| p.ivars().controller.load().is_some());
        panels.iter().filter(|p| p.ivars().root == root).cloned().collect()
    })
}

/// The panel of `controller`'s window.
pub fn of(controller: &WindowController) -> Option<Retained<GitPanel>> {
    PANELS.with(|panels| {
        panels
            .borrow()
            .iter()
            .find(|p| p.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
            .cloned()
    })
}

impl GitPanel {
    /// Builds the panel for `controller`'s folder, puts it on the Git page and starts reading the repository.
    pub fn attach(controller: &WindowController) {
        let Some(root) = controller.project_root() else { return };
        let root = root.canonicalize().unwrap_or(root);
        PANELS.with(|panels| {
            panels
                .borrow_mut()
                .retain(|p| p.controller().is_some_and(|c| !std::ptr::eq(&*c, controller)));
        });
        let panel = GitPanel::new(controller.mtm(), controller, root);
        controller.set_panel_page(Panel::Git, &panel.ui().page);
        panel.update_placeholder();
        PANELS.with(|panels| panels.borrow_mut().push(panel.clone()));
        panel.start_refresh();
        panel.schedule_ages();
    }

    /// Says the ages of the commits again every `AGE_INTERVAL` while the panel shows, until its window closes.
    fn schedule_ages(&self) {
        let id = self.id();
        main_after(AGE_INTERVAL, move || {
            with_panel(id, |panel| {
                panel.refresh_ages();
                panel.schedule_ages();
            });
        });
    }

    /// Reloads the commit rows, whose views compute their age when they are made.
    pub fn refresh_ages(&self) {
        let page = &self.ui().page;
        let showing = page.window().is_some_and(|w| w.isVisible()) && !page.isHiddenOrHasHiddenAncestor();
        if !showing || self.read().model.commits.is_empty() {
            return;
        }
        let item = self.item_for(&RowKey::Section(Section::Commits));
        // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
        unsafe { self.ui().outline.reloadItem_reloadChildren(Some(&item), true) };
    }

    fn new(mtm: MainThreadMarker, controller: &WindowController, root: PathBuf) -> Retained<GitPanel> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let ivars = PanelIvars {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            controller: Weak::from(controller),
            root,
            ui: OnceCell::new(),
            state: RefCell::new(State::default()),
            items: RefCell::new(HashMap::new()),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let panel: Retained<GitPanel> = unsafe { msg_send![super(GitPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = panel.ivars().ui.set(Ui::new(mtm, &panel));
        panel
    }

    pub fn id(&self) -> u64 {
        self.ivars().id
    }

    pub fn root(&self) -> &Path {
        &self.ivars().root
    }

    pub fn ui(&self) -> &Ui {
        self.ivars().ui.get().expect("the panel builds its views in `new`")
    }

    pub fn state(&self) -> std::cell::RefMut<'_, State> {
        self.ivars().state.borrow_mut()
    }

    pub fn read(&self) -> std::cell::Ref<'_, State> {
        self.ivars().state.borrow()
    }

    pub fn controller(&self) -> Option<Retained<WindowController>> {
        self.ivars().controller.load()
    }

    /// How many commits the graph reads.
    pub fn history_limit(&self) -> usize {
        HISTORY_PAGE * (self.read().more_pages + 1)
    }

    pub fn repo(&self) -> Option<Repository> {
        self.ivars().state.borrow().repo.clone()
    }

    /// Reads git again once changes on disk have settled.
    pub fn schedule_refresh(&self) {
        {
            let mut state = self.state();
            if state.refresh.scheduled {
                return;
            }
            state.refresh.scheduled = true;
        }
        let id = self.id();
        main_after(DEBOUNCE, move || with_panel(id, GitPanel::start_refresh));
    }

    /// Reads git now, or right after the read that is running.
    pub fn start_refresh(&self) {
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        {
            let mut state = self.state();
            state.refresh.scheduled = false;
            if !trusted {
                state.phase = Phase::Untrusted;
                state.repo = None;
            } else if state.refresh.running {
                state.refresh.again = true;
                return;
            } else {
                state.refresh.running = true;
            }
        }
        if !trusted {
            self.show_phase();
            return;
        }
        let root = self.root().to_path_buf();
        let id = self.id();
        let limit = self.history_limit();
        let queued = work::local(
            move || load(&root, limit),
            move |loaded| with_panel(id, |p| p.loaded(loaded)),
        );
        if !queued {
            self.state().refresh.running = false;
            self.schedule_refresh();
        }
    }

    fn loaded(&self, loaded: Option<Loaded>) {
        let again = {
            let mut state = self.state();
            state.refresh.running = false;
            std::mem::take(&mut state.refresh.again)
        };
        let loaded =
            loaded.unwrap_or_else(|| Loaded::Failed(Problem::plain(splat_i18n::t("Git ist unerwartet abgebrochen."))));
        self.apply(loaded);
        if again {
            self.schedule_refresh();
        }
        if let Some(controller) = self.controller()
            && self.read().phase == Phase::Ready
        {
            let trees = self.read().model.worktrees.clone();
            super::diff::follow_trees(&controller, &trees);
        }
        super::diff::refresh_for(self.root());
    }

    fn apply(&self, loaded: Loaded) {
        {
            let mut state = self.state();
            let was_ready = state.phase == Phase::Ready;
            if was_ready
                && let Loaded::Ready {
                    status,
                    branches,
                    log,
                    operation,
                    worktrees,
                    ..
                } = &loaded
            {
                let model = Model::new(
                    status,
                    log.clone(),
                    worktrees.clone(),
                    HISTORY_PAGE * (state.more_pages + 1),
                );
                if state.model == model
                    && state.branch == status.branch
                    && state.branches == *branches
                    && state.operation == *operation
                {
                    return;
                }
            }
            match loaded {
                Loaded::NoGit => (state.phase, state.repo) = (Phase::NoGit, None),
                Loaded::NotRepository => (state.phase, state.repo) = (Phase::NotRepository, None),
                Loaded::Failed(problem) if was_ready => state.problem = Some(problem),
                Loaded::Failed(problem) => (state.phase, state.repo) = (Phase::Failed(problem.message), None),
                Loaded::Ready {
                    repo,
                    status,
                    branches,
                    log,
                    operation,
                    worktrees,
                } => {
                    state.phase = Phase::Ready;
                    state.operation = operation;
                    state.repo = Some(repo);
                    state.branch = status.branch.clone();
                    state.branches = branches;
                    state.model = Model::new(&status, log, worktrees, HISTORY_PAGE * (state.more_pages + 1));
                }
            }
            if state.phase != Phase::Ready {
                state.model = Model::default();
            }
            if state.model.conflicts.is_empty() && state.problem.as_ref().is_some_and(|p| p.conflicts) {
                state.problem = None;
            }
        }
        self.show_phase();
    }

    /// Puts every view in line with the state.
    pub fn show_phase(&self) {
        let (phase, count) = {
            let state = self.ivars().state.borrow();
            (state.phase.clone(), state.model.changed_files())
        };
        self.ui().show_phase(self, &phase);
        if phase == Phase::Ready {
            self.update_header();
            self.reload_outline();
            self.update_busy();
            self.update_banner();
            self.update_problem();
        }
        self.refresh_toolbar();
        if let Some(controller) = self.controller() {
            controller.set_symbol_count(Panel::Git, count);
        }
    }

    /// The rows below `item`, the sections for the top.
    pub fn children(&self, item: Option<&AnyObject>) -> Vec<RowKey> {
        let state = self.ivars().state.borrow();
        match item.and_then(key_of) {
            None if item.is_none() => state.model.sections().into_iter().map(RowKey::Section).collect(),
            Some(RowKey::Section(section)) => state.model.keys(section),
            Some(RowKey::Worktree(path)) => state.model.tree_keys(&path),
            Some(RowKey::Commit(hash)) => state.commit_changes.get(&hash).map_or_else(Vec::new, |changes| {
                changes
                    .files
                    .iter()
                    .map(|f| RowKey::CommitFile(hash.clone(), f.path.clone()))
                    .collect()
            }),
            _ => vec![],
        }
    }

    /// The object that stands for `key`, the same one for as long as the panel lives.
    pub fn item_for(&self, key: &RowKey) -> Retained<NSString> {
        let text = key.encode();
        self.ivars()
            .items
            .borrow_mut()
            .entry(text)
            .or_insert_with_key(|text| ns(text))
            .clone()
    }

    fn note_expansion(&self, notification: &NSNotification, expanded: bool) {
        let Some(info) = notification.userInfo() else { return };
        let Some(item) = info.objectForKey(&ns("NSObject")) else {
            return;
        };
        match key_of(&item) {
            Some(RowKey::Section(section)) => {
                let mut state = self.state();
                if expanded {
                    state.collapsed.remove(&section);
                } else {
                    state.collapsed.insert(section);
                }
            }
            Some(RowKey::Worktree(path)) => {
                let mut state = self.state();
                if expanded {
                    state.open_trees.insert(path);
                } else {
                    state.open_trees.remove(&path);
                }
            }
            Some(RowKey::Commit(hash)) => {
                if expanded {
                    self.state().open_commits.insert(hash.clone());
                    self.read_changes(&hash);
                } else {
                    self.state().open_commits.remove(&hash);
                }
            }
            _ => {}
        }
    }

    /// Shows `problem` above the list, or hides it.
    pub fn set_problem(&self, problem: Option<Problem>) {
        self.state().problem = problem;
        self.update_problem();
    }

    fn update_problem(&self) {
        let problem = self.ivars().state.borrow().problem.clone();
        self.ui().show_problem(problem.as_ref());
    }

    pub fn report(&self, error: &ide_git::Error) {
        if matches!(error, ide_git::Error::Cancelled) {
            return;
        }
        let problem = problem(error);
        let hook = problem.hook;
        self.set_problem(Some(problem));
        if hook {
            self.show_problem_output();
        }
    }
}

/// The key `item` stands for.
pub fn key_of(item: &AnyObject) -> Option<RowKey> {
    item.downcast_ref::<NSString>()
        .and_then(|text| RowKey::decode(&text.to_string()))
}

/// Everything the panel shows, with up to `limit` commits, read on the git lane.
fn load(root: &Path, limit: usize) -> Loaded {
    let Some(git) = work::git() else { return Loaded::NoGit };
    if git.program().is_none() {
        return Loaded::NoGit;
    }
    let repo = match git.discover(root) {
        Ok(repo) => repo,
        Err(ide_git::Error::NotARepository { .. }) => return Loaded::NotRepository,
        Err(ide_git::Error::GitMissing) => return Loaded::NoGit,
        Err(error) => return Loaded::Failed(problem(&error)),
    };
    let status = match repo.status(StatusOptions::default()) {
        Ok(status) => status,
        Err(error) => return Loaded::Failed(problem(&error)),
    };
    let branches = repo.branches().unwrap_or_default();
    let log = repo
        .history(limit, status.branch.upstream.as_deref())
        .unwrap_or_default();
    let operation = repo.operation().unwrap_or(None);
    let worktrees = crate::worktrees::read(&git, &repo, root, &branches);
    Loaded::Ready {
        repo,
        status: Box::new(status),
        branches,
        log,
        operation,
        worktrees,
    }
}
