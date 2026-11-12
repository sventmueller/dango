//! The CI panel of one window: what it read, when it reads again, and the AppKit callbacks of its views.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ide_ci::{Client, Forge, Item, Source, State as RunState, Workflow};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint, NSString};

use super::load::{self, Head, Loaded, RunRow};
use super::rows::Key;
use super::ui::Ui;
use super::{text, work};
use crate::util::{guarded, main_after, ns};
use crate::window::{Panel, WindowController};

/// The first wait before reading a running run again; it doubles up to `MAX_WAIT`.
pub const FIRST_WAIT: Duration = Duration::from_secs(5);
const MAX_WAIT: Duration = Duration::from_secs(60);
/// The longest wait for a spent rate limit before trying again.
const MAX_LIMIT_WAIT: Duration = Duration::from_secs(15 * 60);
/// How long changes in `.git` gather before HEAD is read again.
const HEAD_DEBOUNCE: Duration = Duration::from_millis(1500);

/// What the panel can show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Loading,
    Untrusted,
    NoGit,
    NoRepository,
    NoCommit,
    NoRemote,
    Unknown(String),
    SignIn {
        forge: Forge,
        refused: bool,
    },
    Ready,
    Failed(String),
}

/// What the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    Runs,
    /// Open merge requests, pull requests on GitHub.
    Changes,
    Issues,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Runs, Tab::Changes, Tab::Issues];
}

#[derive(Default)]
pub struct State {
    pub phase: Phase,
    pub forge: Option<Forge>,
    pub client: Option<Client>,
    pub head: Head,
    pub source: Option<Source>,
    pub account: Option<String>,
    pub runs: Vec<RunRow>,
    pub tab: Tab,
    /// The open merge requests or issues, when the list shows them.
    pub items: Vec<Item>,
    /// Why they could not be read.
    pub items_problem: Option<String>,
    /// The GitHub workflows of the checkout that may be started by hand.
    pub workflows: Vec<Workflow>,
    pub problem: Option<String>,
    /// The problem came from reading, so the next good read clears it.
    problem_from_read: bool,
    /// A re-run, start or cancel is on its way.
    pub busy: bool,
    /// A login waits for the browser.
    pub login: Option<super::login::Login>,
    pub menu_keys: Vec<Key>,
    /// The rows the user opened or closed.
    pub chosen: HashMap<Key, bool>,
    loading: bool,
    again: bool,
    /// Which planned read is the current one; an older one does nothing.
    planned: u64,
    wait: Duration,
    head_check: bool,
}

pub struct PanelIvars {
    id: u64,
    controller: Weak<WindowController>,
    root: PathBuf,
    ui: OnceCell<Ui>,
    state: RefCell<State>,
    /// The panel itself opens and closes rows, so the user's choices are not touched.
    pub unfolding: Cell<bool>,
    items: RefCell<HashMap<String, Retained<NSString>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `CiPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiPanel"]
    #[ivars = PanelIvars]
    pub struct CiPanel;

    unsafe impl NSObjectProtocol for CiPanel {}

    unsafe impl NSOutlineViewDataSource for CiPanel {
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
            guarded(false, || !self.children(Some(item)).is_empty())
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
            guarded(None, || self.row_view(&Key::of(item)?))
        }

        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn row_height(&self, _view: &NSOutlineView, item: &AnyObject) -> f64 {
            guarded(24.0, || super::rows::height(Key::of(item).as_ref()))
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
pub struct CiPanels {
    panels: RefCell<Vec<Retained<CiPanel>>>,
}

fn with_panels<R>(f: impl FnOnce(&RefCell<Vec<Retained<CiPanel>>>) -> R) -> R {
    f(&crate::context::app().ci_panels.panels)
}

/// Runs `f` with the panel `id`, if its window is still open.
pub fn with_panel(id: u64, f: impl FnOnce(&CiPanel)) {
    let panel = with_panels(|panels| panels.borrow().iter().find(|p| p.ivars().id == id).cloned());
    if let Some(panel) = panel {
        f(&panel);
    }
}

/// The panels of windows that show `root`.
pub fn for_root(root: &Path) -> Vec<Retained<CiPanel>> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    with_panels(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|p| p.ivars().controller.load().is_some());
        panels.iter().filter(|p| p.ivars().root == root).cloned().collect()
    })
}

/// The panels of all open windows.
pub fn all() -> Vec<Retained<CiPanel>> {
    with_panels(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|p| p.ivars().controller.load().is_some());
        panels.clone()
    })
}

/// The panel of `controller`'s window.
pub fn of(controller: &WindowController) -> Option<Retained<CiPanel>> {
    with_panels(|panels| {
        panels
            .borrow()
            .iter()
            .find(|p| p.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
            .cloned()
    })
}

impl CiPanel {
    /// Builds the panel for `controller`'s folder, puts it on the CI page and starts reading.
    pub fn attach(controller: &WindowController) {
        let Some(root) = controller.project_root() else { return };
        let root = root.canonicalize().unwrap_or(root);
        with_panels(|panels| {
            panels
                .borrow_mut()
                .retain(|p| p.controller().is_some_and(|c| !std::ptr::eq(&*c, controller)));
        });
        let panel = CiPanel::new(controller.mtm(), controller, root);
        controller.set_panel_page(Panel::Ci, &panel.ui().page);
        controller.set_panel_accessory(Panel::Ci, &panel.ui().header);
        with_panels(|panels| panels.borrow_mut().push(panel.clone()));
        panel.refresh(true);
    }

    fn new(mtm: MainThreadMarker, controller: &WindowController, root: PathBuf) -> Retained<CiPanel> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let ivars = PanelIvars {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            controller: Weak::from(controller),
            root,
            ui: OnceCell::new(),
            unfolding: Cell::new(false),
            state: RefCell::new(State {
                wait: FIRST_WAIT,
                ..State::default()
            }),
            items: RefCell::new(HashMap::new()),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let panel: Retained<CiPanel> = unsafe { msg_send![super(CiPanel::alloc(mtm).set_ivars(ivars)), init] };
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

    /// Reads the service again now, or right after the read that runs; `fresh` starts the wait for running runs over.
    pub fn refresh(&self, fresh: bool) {
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        let (known, tab) = {
            let mut state = self.state();
            state.planned += 1;
            if fresh {
                state.wait = FIRST_WAIT;
            }
            if !trusted {
                state.phase = Phase::Untrusted;
                state.client = None;
                state.runs.clear();
            } else if state.loading {
                state.again = true;
                return;
            } else {
                state.loading = true;
            }
            (known(&state), state.tab)
        };
        if !trusted {
            self.show();
            return;
        }
        let root = self.root().to_path_buf();
        let id = self.id();
        let queued = work::run(
            move || load::load(&root, known, tab),
            move |loaded| with_panel(id, |p| p.loaded(loaded)),
        );
        if !queued {
            self.state().loading = false;
            self.plan(FIRST_WAIT);
        }
    }

    fn loaded(&self, loaded: Option<Loaded>) {
        let again = {
            let mut state = self.state();
            state.loading = false;
            std::mem::take(&mut state.again)
        };
        let loaded = loaded.unwrap_or(Loaded::Failed(ide_ci::Error::Decode("panic".into()), None));
        let wait = self.apply(loaded);
        self.show();
        super::log_tab::refresh_for(self.root(), &self.read().runs);
        if again {
            self.refresh(false);
        } else if let Some(wait) = wait {
            self.plan(wait);
        }
    }

    /// Takes what a read found; when to read again, if at all.
    fn apply(&self, loaded: Loaded) -> Option<Duration> {
        let mut state = self.state();
        let keep = |state: &mut State, phase: Phase, forge: Option<Forge>| {
            state.phase = phase;
            state.forge = forge;
            state.client = None;
            state.runs.clear();
            state.items.clear();
            state.items_problem = None;
        };
        match loaded {
            Loaded::NoGit => keep(&mut state, Phase::NoGit, None),
            Loaded::NoRepository => keep(&mut state, Phase::NoRepository, None),
            Loaded::NoCommit => keep(&mut state, Phase::NoCommit, None),
            Loaded::NoRemote => keep(&mut state, Phase::NoRemote, None),
            Loaded::Unknown(host) => keep(&mut state, Phase::Unknown(host), None),
            Loaded::SignIn { forge, refused } => {
                keep(
                    &mut state,
                    Phase::SignIn {
                        forge: forge.clone(),
                        refused,
                    },
                    Some(forge),
                );
            }
            Loaded::Failed(error, forge) => {
                let message = text::problem(&error);
                let limited = match error {
                    ide_ci::Error::RateLimited { until } => Some(until),
                    _ => None,
                };
                if state.phase == Phase::Ready {
                    state.problem = Some(message);
                    state.problem_from_read = true;
                } else {
                    keep(&mut state, Phase::Failed(message), forge);
                }
                return limited.map(|until| {
                    let seconds = until.map_or(60, |u| (u - ide_ci::now()).max(5));
                    Duration::from_secs(u64::try_from(seconds).unwrap_or(60)).min(MAX_LIMIT_WAIT)
                });
            }
            Loaded::Ready(snapshot) => {
                if std::mem::take(&mut state.problem_from_read) {
                    state.problem = None;
                }
                state.phase = Phase::Ready;
                state.forge = Some(snapshot.client.forge().clone());
                state.client = Some(snapshot.client);
                state.head = snapshot.head;
                state.source = snapshot.source;
                state.account = snapshot.account;
                state.runs = snapshot.runs;
                if snapshot.tab == state.tab {
                    state.items = snapshot.items;
                    state.items_problem = snapshot.items_problem;
                }
                state.workflows = snapshot.workflows;
                let active = state
                    .runs
                    .iter()
                    .any(|r| r.run.state.is_active() || r.jobs.iter().any(|j| j.state.is_active()));
                if !active {
                    state.wait = FIRST_WAIT;
                    return None;
                }
                let wait = state.wait;
                state.wait = (wait * 2).min(MAX_WAIT);
                return Some(wait);
            }
        }
        None
    }

    /// Reads again after `wait`, unless another read was planned or started since.
    pub fn plan(&self, wait: Duration) {
        let planned = {
            let mut state = self.state();
            state.planned += 1;
            state.planned
        };
        let id = self.id();
        main_after(wait, move || {
            with_panel(id, |panel| {
                if panel.read().planned == planned {
                    panel.refresh(false);
                }
            });
        });
    }

    /// Something in the folder changed; reads again when HEAD moved.
    pub fn changed(&self) {
        {
            let mut state = self.state();
            if state.head_check || state.phase == Phase::Untrusted {
                return;
            }
            state.head_check = true;
        }
        let id = self.id();
        main_after(HEAD_DEBOUNCE, move || with_panel(id, CiPanel::check_head));
    }

    fn check_head(&self) {
        let root = self.root().to_path_buf();
        let id = self.id();
        let queued = work::run(
            move || load::head(&root).ok().map(|(_, head)| head),
            move |head| {
                with_panel(id, |panel| {
                    let moved = {
                        let mut state = panel.state();
                        state.head_check = false;
                        head.flatten().is_some_and(|h| h != state.head) || state.phase != Phase::Ready
                    };
                    if moved {
                        panel.refresh(true);
                    }
                });
            },
        );
        if !queued {
            self.state().head_check = false;
        }
    }

    /// Puts every view in line with the state and the activity bar's dot with the runs.
    pub fn show(&self) {
        let phase = self.read().phase.clone();
        self.ui().show_phase(self, &phase);
        if phase == Phase::Ready {
            self.ui().show_ready(self);
            self.reload_outline();
        }
        self.ui().show_problem(self.read().problem.as_deref());
        let summary = RunState::summary(self.read().runs.iter().map(|r| r.run.state));
        if let Some(controller) = self.controller() {
            match summary.filter(|_| phase == Phase::Ready) {
                Some(state) => controller.set_symbol_status(Panel::Ci, Some(text::word(state))),
                None => controller.set_symbol_status(Panel::Ci, None),
            }
        }
    }

    pub fn set_problem(&self, problem: Option<String>) {
        {
            let mut state = self.state();
            state.problem = problem;
            state.problem_from_read = false;
        }
        self.ui().show_problem(self.read().problem.as_deref());
    }

    /// Shows `tab` and reads what it lists.
    pub fn choose_tab(&self, tab: Tab) {
        {
            let mut state = self.state();
            if state.tab == tab {
                return;
            }
            state.tab = tab;
            state.items.clear();
            state.items_problem = None;
        }
        self.show();
        self.refresh(true);
    }

    /// The rows below `item`: runs at the top, then GitLab's stages, then jobs, then steps; or the open merge requests
    /// or issues.
    pub fn children(&self, item: Option<&AnyObject>) -> Vec<Key> {
        let state = self.read();
        match item.map(Key::of) {
            None if state.tab != Tab::Runs => state.items.iter().map(|i| Key::Item(i.number)).collect(),
            None => state.runs.iter().map(|r| Key::Run(r.run.id)).collect(),
            Some(Some(Key::Run(run))) => state
                .runs
                .iter()
                .find(|r| r.run.id == run)
                .map(|r| match ide_ci::stages(&r.jobs).len() {
                    0 => r.jobs.iter().map(|j| Key::Job(run, j.id)).collect(),
                    stages => (0..u32::try_from(stages).unwrap_or(0))
                        .map(|stage| Key::Stage(run, stage))
                        .collect(),
                })
                .unwrap_or_default(),
            Some(Some(Key::Stage(run, stage))) => state
                .runs
                .iter()
                .find(|r| r.run.id == run)
                .and_then(|r| {
                    let stages = ide_ci::stages(&r.jobs);
                    let jobs = &stages.get(usize::try_from(stage).ok()?)?.jobs;
                    Some(jobs.iter().map(|j| Key::Job(run, j.id)).collect())
                })
                .unwrap_or_default(),
            Some(Some(Key::Job(run, job))) => state
                .runs
                .iter()
                .find(|r| r.run.id == run)
                .and_then(|r| r.jobs.iter().find(|j| j.id == job))
                .map(|j| j.steps.iter().map(|s| Key::Step(run, job, s.number)).collect())
                .unwrap_or_default(),
            _ => vec![],
        }
    }

    /// The object that stands for `key`, the same one for as long as the panel lives.
    pub fn item_for(&self, key: &Key) -> Retained<NSString> {
        self.ivars()
            .items
            .borrow_mut()
            .entry(key.encode())
            .or_insert_with_key(|text| ns(text))
            .clone()
    }
}

/// The account name found last time with its host and token source, so a read need not ask again.
fn known(state: &State) -> Option<(String, Source, String)> {
    Some((
        state.forge.as_ref()?.host.clone(),
        state.source.clone()?,
        state.account.clone()?,
    ))
}
