//! The CI panel of one window: what it read, when it reads again, and the AppKit callbacks of its views.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ide_ci::State as RunState;
use ide_ui_common::panel::{HeldReload, KeptCells, RowItem, RowItems};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint};

use super::model::{Key, State, Tab};
use super::ui::Ui;
use crate::context::{Registry, WindowPart};
use crate::util::{guarded, ns};
use crate::window::WindowController;

pub(crate) struct PanelIvars {
    id: u64,
    controller: Weak<WindowController>,
    root: PathBuf,
    ui: OnceCell<Ui>,
    state: RefCell<State>,
    /// The panel itself opens and closes rows, so the user's choices are not touched.
    pub(crate) unfolding: Cell<bool>,
    items: RowItems<Key>,
    pub(crate) cells: KeptCells<Key>,
    pub(crate) held: HeldReload,
    /// Whether the panel reads nothing until it shows, as its symbol is hidden (E80) and the state of the runs, which
    /// its tooltip names (E69), is seen nowhere else.
    pub(crate) idle: Cell<bool>,
    /// The revision of the rows the list last reloaded with.
    pub(crate) shown_revision: Cell<Option<u64>>,
    /// Counts the ticks that bring the times the rows show up to now.
    pub(crate) clock: Cell<u64>,
    /// The tick each kept cell's times are as of.
    pub(crate) timed: RefCell<HashMap<Key, u64>>,
    /// Whether ticks are planned.
    pub(crate) ticking: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `CiPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiPanel"]
    #[ivars = PanelIvars]
    pub(crate) struct CiPanel;

    unsafe impl NSObjectProtocol for CiPanel {}

    unsafe impl NSOutlineViewDataSource for CiPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.child_count(item)).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let index = usize::try_from(index).ok()?;
                let key = self.with_children(item, |children| children.get(index).copied())?;
                Some(self.item_for(&key))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || self.child_count(Some(item)) > 0)
        }
    }

    unsafe impl NSControlTextEditingDelegate for CiPanel {}

    unsafe impl NSOutlineViewDelegate for CiPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.kept_row_view(&super::rows::key_of(item)?))
        }

        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn row_height(&self, _view: &NSOutlineView, item: &AnyObject) -> f64 {
            guarded(super::rows::height(None), || super::rows::height(super::rows::key_of(item).as_ref()))
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

    unsafe impl NSMenuDelegate for CiPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    impl CiPanel {
        #[unsafe(method(ideCiRefresh:))]
        fn refresh_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.refresh(true));
        }

        #[unsafe(method(ideCiRerun:))]
        fn rerun_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.rerun(self.run_for(sender, RunState::Failed)));
        }

        #[unsafe(method(ideCiRerunAll:))]
        fn rerun_all_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.rerun_all(self.run_for(sender, RunState::Neutral)));
        }

        #[unsafe(method(ideCiRetryJob:))]
        fn retry_job_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.retry_job(self.job_for(sender)));
        }

        #[unsafe(method(ideCiStartJob:))]
        fn start_job_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.start_job(self.job_for(sender)));
        }

        #[unsafe(method(ideCiStartRun:))]
        fn start_run_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.start_run());
        }

        #[unsafe(method(ideCiNewItem:))]
        fn new_item_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.new_item());
        }

        #[unsafe(method(ideCiTakeAsOrder:))]
        fn take_as_order_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.take_as_order(sender));
        }

        #[unsafe(method(ideCiCopyLink:))]
        fn copy_link_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.copy_link(sender));
        }

        #[unsafe(method(ideCiTab:))]
        fn tab_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let segment = sender
                    .and_then(|s| s.downcast_ref::<objc2_app_kit::NSSegmentedControl>())
                    .map_or(0, objc2_app_kit::NSSegmentedControl::selectedSegment);
                let tab = usize::try_from(segment).ok().and_then(|i| Tab::ALL.get(i).copied());
                self.choose_tab(tab.unwrap_or_default());
            });
        }

        #[unsafe(method(ideCiCancel:))]
        fn cancel_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.cancel(self.run_for(sender, RunState::Running)));
        }

        #[unsafe(method(ideCiOpenLog:))]
        fn open_log_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.open_log(self.job_for(sender)));
        }

        #[unsafe(method(ideCiSendToAgent:))]
        fn agent_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.send_to_agent(self.job_for(sender)));
        }

        #[unsafe(method(ideCiOpenWeb:))]
        fn web_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.open_web(sender));
        }

        #[unsafe(method(ideCiDoubleClicked:))]
        fn double_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.ui().outline.clickedRow();
                self.activate_row(row);
            });
        }

        #[unsafe(method(ideCiAccount:))]
        fn account_action(&self, sender: Option<&NSView>) {
            guarded((), || {
                if let Some(view) = sender {
                    let below = NSPoint::new(0.0, view.frame().size.height + 4.0);
                    self.ui()
                        .account_menu(self)
                        .popUpMenuPositioningItem_atLocation_inView(None, below, Some(view));
                }
            });
        }

        #[unsafe(method(ideCiSignIn:))]
        fn sign_in_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.sign_in());
        }

        #[unsafe(method(ideCiBrowserLogin:))]
        fn browser_login_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.browser_login());
        }

        #[unsafe(method(ideCiCancelLogin:))]
        fn cancel_login_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.cancel_login());
        }

        #[unsafe(method(ideCiOpenSettings:))]
        fn open_settings_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::settings::show_ci(self.mtm()));
        }

        #[unsafe(method(ideCiSignOut:))]
        fn sign_out_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.sign_out());
        }

        #[unsafe(method(ideCiTrust:))]
        fn trust_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(controller) = self.controller() {
                    controller.ask_trust_again();
                }
            });
        }

        #[unsafe(method(ideCiDismissProblem:))]
        fn dismiss_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.set_problem(None));
        }
    }
);

/// The CI panels of every window.
#[derive(Default)]
pub(crate) struct CiPanels {
    pub(super) panels: Registry<Retained<CiPanel>>,
    /// What the self-test would have opened in the browser or put on the pasteboard, which belong to the user.
    pub(super) noted: RefCell<Vec<String>>,
    pub(super) logins: crate::context::Serial,
}

/// Runs `f` with the panel `id`, if its window is still open.
pub(crate) fn with_panel(id: u64, f: impl FnOnce(&CiPanel)) {
    crate::context::app().ci_panels().panels.with(id, f);
}

/// The panels of windows that show `root`.
pub(crate) fn for_root(root: &Path) -> Vec<Retained<CiPanel>> {
    crate::context::app().ci_panels().panels.for_root(root)
}

/// The panels of all open windows.
pub(crate) fn all() -> Vec<Retained<CiPanel>> {
    crate::context::app().ci_panels().panels.live()
}

/// The panel of `controller`'s window.
pub(crate) fn of(controller: &WindowController) -> Option<Retained<CiPanel>> {
    controller.context().ci_panels().panels.of(controller.handle())
}

impl WindowPart for CiPanel {
    fn window(&self) -> Option<ide_ui_common::window::Window> {
        self.controller().map(|c| c.handle())
    }

    fn root(&self) -> &Path {
        CiPanel::root(self)
    }
}

impl CiPanel {
    pub(super) fn new(mtm: MainThreadMarker, controller: &WindowController, root: PathBuf) -> Retained<CiPanel> {
        let ivars = PanelIvars {
            id: controller.context().ci_panels().panels.next_id(),
            controller: Weak::from(controller),
            root,
            ui: OnceCell::new(),
            unfolding: Cell::new(false),
            state: RefCell::new(State::new()),
            items: RowItems::default(),
            cells: KeptCells::default(),
            held: HeldReload::default(),
            idle: Cell::new(false),
            shown_revision: Cell::new(None),
            clock: Cell::new(0),
            timed: RefCell::new(HashMap::new()),
            ticking: Cell::new(false),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let panel: Retained<CiPanel> = unsafe { msg_send![super(CiPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = panel.ivars().ui.set(Ui::new(mtm, &panel));
        panel
    }

    pub(crate) fn id(&self) -> u64 {
        self.ivars().id
    }

    pub(crate) fn root(&self) -> &Path {
        &self.ivars().root
    }

    pub(crate) fn ui(&self) -> &Ui {
        self.ivars().ui.get().expect("the panel builds its views in `new`")
    }

    pub(crate) fn state(&self) -> std::cell::RefMut<'_, State> {
        self.ivars().state.borrow_mut()
    }

    pub(crate) fn read(&self) -> std::cell::Ref<'_, State> {
        self.ivars().state.borrow()
    }

    pub(crate) fn controller(&self) -> Option<Retained<WindowController>> {
        self.ivars().controller.load()
    }

    /// The rows below `item`: runs at the top, then GitLab's stages, then jobs, then steps; or the open merge requests
    /// or issues.
    pub(crate) fn children(&self, item: Option<&AnyObject>) -> Vec<Key> {
        self.with_children(item, <[Key]>::to_vec)
    }

    /// How many rows lie below `item`.
    pub(crate) fn child_count(&self, item: Option<&AnyObject>) -> usize {
        self.with_children(item, <[Key]>::len)
    }

    /// Runs `f` with the rows below `item`, which the state keeps worked out.
    fn with_children<T>(&self, item: Option<&AnyObject>, f: impl FnOnce(&[Key]) -> T) -> T {
        match item.map(super::rows::key_of) {
            None => f(self.read().children(None)),
            Some(Some(key)) => f(self.read().children(Some(key))),
            Some(None) => f(&[]),
        }
    }

    /// The object that stands for `key`, the same one for as long as the panel lives.
    pub(crate) fn item_for(&self, key: &Key) -> Retained<RowItem> {
        self.ivars().items.item(self.mtm(), key)
    }
}
