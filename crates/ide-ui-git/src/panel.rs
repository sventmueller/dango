//! The Git panel of one window as AppKit sees it: the data source and delegate of its list, and the actions of its
//! buttons and menus, which go on to the panel's operations.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_git::Repository;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSTableRowView, NSTextDelegate, NSTextView, NSTextViewDelegate, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSUndoManager};

use super::model::{Read, RowKey, State, Task};
use super::ui::Ui;
use ide_ui_common::registry::WindowPart;
use ide_ui_common::util::{guarded, ns};
use ide_ui_common::window::{AsWindow, Trust, Window};

pub struct PanelIvars {
    id: u64,
    controller: Window,
    /// The window's folder, which may lie below the repository's root.
    root: PathBuf,
    ui: OnceCell<Ui>,
    state: RefCell<State>,
    /// The objects that stand for rows in the outline view, which keeps no reference of its own; the same object
    /// for a key for as long as the row exists, so the view keeps it open and selected.
    items: RefCell<HashMap<RowKey, Retained<RowItem>>>,
    /// The keys below each row the outline asked for, kept until the state changes.
    children: RefCell<HashMap<Option<RowKey>, Rc<[RowKey]>>>,
    /// The state changed while the page was out of view, so the list reloads when it comes into view.
    stale: Cell<bool>,
    /// The commit message's own undo, apart from the window's, so that a message a model wrote is one step to take
    /// back (E88).
    undo: Retained<NSUndoManager>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `RowItem` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitRowItem"]
    #[ivars = RowKey]
    pub struct RowItem;
);

define_class!(
    // SAFETY: NSView has no subclassing requirements and `GitPage` does not implement Drop.
    /// The panel's page, which tells the panel of the id it holds when it comes into view.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitPage"]
    #[ivars = u64]
    pub struct GitPage;

    unsafe impl NSObjectProtocol for GitPage {}

    impl GitPage {
        #[unsafe(method(viewDidUnhide))]
        fn did_unhide(&self) {
            guarded((), || with_panel(*self.ivars(), GitPanel::came_into_view));
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            guarded((), || with_panel(*self.ivars(), GitPanel::came_into_view));
        }
    }
);

impl GitPage {
    /// The page of the panel `panel`.
    pub fn new(mtm: MainThreadMarker, panel: u64) -> Retained<GitPage> {
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(GitPage::alloc(mtm).set_ivars(panel)), init] }
    }
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
            guarded(false, || key_of(item).is_some_and(|key| self.read().expandable(&key)))
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
            guarded((), || {
                self.update_placeholder();
                self.message_edited();
            });
        }
    }

    unsafe impl NSTextViewDelegate for GitPanel {
        #[unsafe(method_id(undoManagerForTextView:))]
        fn undo_manager_for(&self, _view: &NSTextView) -> Option<Retained<NSUndoManager>> {
            Some(self.ivars().undo.clone())
        }
    }

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
            guarded((), || {
                if self.read().stash_apply.is_some() {
                    self.finish_stash_conflict();
                } else {
                    self.continue_operation();
                }
            });
        }

        #[unsafe(method(ideGitAbort:))]
        fn abort_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if self.read().stash_apply.is_some() {
                    self.abort_stash_conflict();
                } else {
                    self.abort_operation();
                }
            });
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

        #[unsafe(method(ideGitStash:))]
        fn stash_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_stash());
        }

        #[unsafe(method(ideGitStashStaged:))]
        fn stash_staged_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.stash_staged());
        }

        #[unsafe(method(ideGitApplyStash:))]
        fn apply_stash_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.apply_stash(&self.keys_for(sender), false));
        }

        #[unsafe(method(ideGitPopStash:))]
        fn pop_stash_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.apply_stash(&self.keys_for(sender), true));
        }

        #[unsafe(method(ideGitDropStash:))]
        fn drop_stash_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.drop_stash(&self.keys_for(sender)));
        }

        #[unsafe(method(ideGitMessage:))]
        fn message_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.generate_message());
        }

        #[unsafe(method(ideGitUndoMessage:))]
        fn undo_message_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.undo_message());
        }

        #[unsafe(method(ideGitShowSent:))]
        fn show_sent_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.show_sent());
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &objc2_app_kit::NSMenuItem) -> bool {
            guarded(false, || {
                let keys = self.read().menu_keys.clone();
                item.action().is_none_or(|action| self.can(action, &keys))
            })
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
                self.cancel_message();
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

/// Runs `f` with the panel `id`, if its window is still open.
pub fn with_panel(id: u64, f: impl FnOnce(&GitPanel)) {
    crate::host::state().panels.with(id, f);
}

/// The panels of windows that show `root`.
pub fn for_root(root: &Path) -> Vec<Retained<GitPanel>> {
    crate::host::state().panels.for_root(root)
}

/// The panel of `controller`'s window.
pub fn of(controller: &(impl AsWindow + ?Sized)) -> Option<Retained<GitPanel>> {
    let controller = &controller.as_window();
    crate::host::state().panels.of(*controller)
}

impl WindowPart for GitPanel {
    fn window(&self) -> Option<Window> {
        self.controller()
    }

    fn root(&self) -> &Path {
        GitPanel::root(self)
    }
}

impl GitPanel {
    pub(super) fn new(mtm: MainThreadMarker, controller: &Window, root: PathBuf) -> Retained<GitPanel> {
        let ivars = PanelIvars {
            id: crate::host::state().panels.next_id(),
            controller: *controller,
            root,
            ui: OnceCell::new(),
            state: RefCell::new(State::default()),
            items: RefCell::new(HashMap::new()),
            children: RefCell::new(HashMap::new()),
            stale: Cell::new(false),
            undo: NSUndoManager::new(mtm),
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
        let state = self.ivars().state.borrow_mut();
        self.ivars().children.borrow_mut().clear();
        state
    }

    /// Whether the page is out of view, where the list waits with reloading until it comes back.
    pub(super) fn out_of_view(&self) -> bool {
        let page = &self.ui().page;
        page.window().is_none() || page.isHiddenOrHasHiddenAncestor()
    }

    /// Notes whether the list waits to reload until the page comes into view.
    pub(super) fn set_stale(&self, stale: bool) {
        self.ivars().stale.set(stale);
    }

    /// The page came into view: the list catches up with what changed while it was away.
    fn came_into_view(&self) {
        if self.ivars().stale.get() && !self.out_of_view() {
            self.reload_outline();
        }
    }

    pub fn read(&self) -> std::cell::Ref<'_, State> {
        self.ivars().state.borrow()
    }

    /// The commit message's undo.
    pub fn message_undo(&self) -> &NSUndoManager {
        &self.ivars().undo
    }

    pub fn controller(&self) -> Option<Window> {
        Window::open(self.ivars().controller.id())
    }

    /// How many commits the graph reads.
    pub fn history_limit(&self) -> usize {
        self.read().history_limit()
    }

    pub fn repo(&self) -> Option<Repository> {
        self.ivars().state.borrow().repo.clone()
    }

    /// The rows below `item`, the sections for the top.
    pub fn children(&self, item: Option<&AnyObject>) -> Rc<[RowKey]> {
        let parent = match item {
            None => None,
            Some(item) => match key_of(item) {
                Some(key) => Some(key),
                None => return Rc::from([]),
            },
        };
        if let Some(found) = self.ivars().children.borrow().get(&parent) {
            return found.clone();
        }
        let found: Rc<[RowKey]> = self.read().children(parent.as_ref()).into();
        self.ivars().children.borrow_mut().insert(parent, found.clone());
        found
    }

    /// The object that stands for `key`, the same one for as long as its row exists.
    pub fn item_for(&self, key: &RowKey) -> Retained<RowItem> {
        if let Some(item) = self.ivars().items.borrow().get(key) {
            return item.clone();
        }
        let item = RowItem::alloc(self.mtm()).set_ivars(key.clone());
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let item: Retained<RowItem> = unsafe { msg_send![super(item), init] };
        self.ivars().items.borrow_mut().insert(key.clone(), item.clone());
        item
    }

    /// Lets go of the items of rows the panel no longer has, after the outline view read the rows anew.
    pub(super) fn forget_gone_items(&self) {
        let live = self.read().all_keys();
        self.ivars().items.borrow_mut().retain(|key, _| live.contains(key));
    }

    fn note_expansion(&self, notification: &NSNotification, expanded: bool) {
        let Some(info) = notification.userInfo() else { return };
        let Some(key) = info.objectForKey(&ns("NSObject")).and_then(|item| key_of(&item)) else {
            return;
        };
        let read = self.state().expansion(key, expanded);
        match read {
            Some(Read::CommitChanges(hash)) => self.read_changes(&hash),
            Some(Read::StashChanges(hash)) => self.read_stash_changes(&hash),
            None => {}
        }
    }
}

/// The key `item` stands for.
pub fn key_of(item: &AnyObject) -> Option<RowKey> {
    item.downcast_ref::<RowItem>().map(|item| item.ivars().clone())
}
