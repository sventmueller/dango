//! One AI tab: its session, the views showing it, and how session events reach them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use ide_claude::{Capabilities, Event, Launch, Session, SessionError};
use ide_git::Checkpoint;
use ide_process::Supervisor;
use ide_thread::{Attachment, Change, Status, Thread, ThreadId, ThreadIndex, UserMessage};
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSEvent, NSLayoutConstraint, NSResponder, NSStackView, NSView};
use objc2_foundation::{NSObjectProtocol, NSRect};
use splat_i18n::{t, tf};

use super::acp::Agent;
use super::checkpoint::Store;
use super::composer::Composer;
use super::header::Header;
use super::rewind::Fork;
use super::source::{self, Problem, Resume, Source};
use super::status::StatusLine;
use super::suggest::Suggestions;
use super::target::AgentTarget;
use super::timeline::Timeline;
use super::{notify, overlay, ui};
use crate::util::{guarded, main_after, main_soon};
use crate::window::{TabKind, WindowController};

/// The room on either side of the conversation, which takes the rest of the tab's width.
pub(crate) const SIDE_MARGIN: f64 = 16.0;
/// The widest the composer gets; it stands in the middle below a wider conversation.
pub(crate) const INPUT_WIDTH: f64 = 852.0;
/// The room between the composer and the conversation's edges.
pub(crate) const INPUT_INSET: f64 = 14.0;
/// How strongly the composer fills the conversation's width up to `INPUT_WIDTH`: above the content's hugging and the
/// holding priority of the window's content, below the sidebar's, so a wide window neither narrows nor moves it.
const FILL_PRIORITY: f32 = 255.0;

static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

/// What a session reported, from either kind of connection.
pub enum Incoming {
    Claude(Event),
    Acp(ide_acp::Event),
    Own(ide_agent::Event),
}

/// Session events waiting for the main thread.
#[derive(Default)]
pub struct Inbox {
    events: Mutex<super::inbox::Pending>,
    scheduled: AtomicBool,
}

pub struct RootIvars {
    key: u64,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `AgentRoot` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentRoot"]
    #[ivars = RootIvars]
    pub struct AgentRoot;

    unsafe impl NSObjectProtocol for AgentRoot {}

    impl AgentRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            guarded((), || {
                NSColor::textBackgroundColor().setFill();
                objc2_app_kit::NSBezierPath::fillRect(self.bounds());
            });
        }

        /// A click on the composer's card that no control in it took, such as on its margin, goes into its text.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let taken = guarded(false, || {
                let Some(tab) = super::get(self.ivars().key) else {
                    return false;
                };
                let card = &tab.composer.card;
                let inside = card.convertPoint_fromView(event.locationInWindow(), None);
                if !objc2_foundation::NSPointInRect(inside, card.bounds()) {
                    return false;
                }
                tab.focus_composer();
                true
            });
            if !taken {
                // SAFETY: The superclass's implementation, with this method's signature.
                let () = unsafe { msg_send![super(self), mouseDown: event] };
            }
        }

        /// A tab that leaves its window for good was closed: its session ends with it.
        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            guarded((), || {
                if self.window().is_some() {
                    return;
                }
                let key = self.ivars().key;
                main_soon(move || {
                    if super::get(key).is_some_and(|tab| tab.root.window().is_none()) {
                        super::remove(key);
                    }
                });
            });
        }
    }
);

/// What the composer's menus offered, so the picked item's tag can be read.
#[derive(Default)]
pub struct Offered {
    pub commands: Vec<String>,
    pub models: Vec<String>,
    pub efforts: Vec<String>,
    pub modes: Vec<String>,
    pub past: Vec<Resume>,
}

pub struct State {
    pub source: Source,
    pub thread: Rc<Thread>,
    pub caps: Capabilities,
    /// What the ACP agent and its session offer.
    pub acp: Option<ide_acp::Capabilities>,
    /// The endpoint and model of the own agent's session.
    pub own: Option<crate::own_agent::ModelChoice>,
    pub problem: Option<Problem>,
    pub custom_title: Option<String>,
    pub queue: Vec<UserMessage>,
    pub attachments: Vec<Attachment>,
    pub sandbox: bool,
    /// Whether the user connected the thread to Remote Control or ended it; `None` follows their Claude settings.
    pub remote: Option<bool>,
    pub mode: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub bookmarked: bool,
    pub resume: Option<Resume>,
    /// The checkpoint taken before each user message, by the CLI's id for the message.
    pub checkpoints: HashMap<String, Checkpoint>,
    /// The checkpoint of the turn that runs.
    pub turn: Option<Checkpoint>,
    pub status: Status,
    /// A checkpoint is being taken before a prompt goes out.
    pub sending: bool,
    pub offered: Offered,
    /// Where the keyboard focus was before ⌘Esc moved it into the composer.
    pub previous_focus: Option<Weak<NSResponder>>,
    /// The rewind the session started with, until the agent accepted it.
    pub fork: Option<Fork>,
    /// The prompt sent last, which goes out again when the agent refuses a rewind.
    pub last_sent: Option<UserMessage>,
    /// The editor's selection, offered as context while the composer has the focus.
    pub offer: Option<Attachment>,
    /// The session runs with the model and mode the user picked last for its agent, where it offers them.
    pub choice_applied: bool,
}

pub struct AgentTab {
    pub key: u64,
    /// The agent the tab runs, for good.
    pub(super) agent: Agent,
    pub(super) mtm: MainThreadMarker,
    controller: Weak<WindowController>,
    tab_id: std::cell::Cell<isize>,
    pub(super) cwd: PathBuf,
    pub(super) thread_id: ThreadId,
    pub(super) target: Retained<AgentTarget>,
    root: Retained<AgentRoot>,
    pub(super) header: Header,
    timeline: Timeline,
    pub(super) composer: Composer,
    pub(super) status: StatusLine,
    empty: Retained<NSStackView>,
    problem_holder: Retained<NSView>,
    pub(super) queue_view: Retained<NSStackView>,
    pub(super) suggestions: Suggestions,
    pub(super) state: RefCell<State>,
    inbox: RefCell<Arc<Inbox>>,
    pub(super) store: Store,
    ticking: std::cell::Cell<bool>,
}

impl AgentTab {
    /// A new tab in `controller`'s window running `agent`, not started yet.
    pub fn open(controller: &WindowController, agent: Agent, resume: Option<Resume>) -> Option<Rc<AgentTab>> {
        let cwd = controller.project_root().or_else(|| controller.project())?;
        AgentTab::open_in(controller, agent, cwd, resume)
    }

    /// A new tab in `controller`'s window running `agent` in `cwd`, such as a thread's worktree, not started yet.
    pub fn open_in(
        controller: &WindowController,
        agent: Agent,
        cwd: PathBuf,
        resume: Option<Resume>,
    ) -> Option<Rc<AgentTab>> {
        let thread_id = resume.as_ref().and_then(|r| r.thread.clone()).unwrap_or_default();
        let thread = Thread::new(thread_id.clone(), agent.id(), &cwd);
        let tab = Rc::new(AgentTab::build(controller, agent, cwd, thread_id, thread, Source::Idle));
        {
            let mut state = tab.state.borrow_mut();
            state.custom_title = resume.as_ref().and_then(|r| r.title.clone());
            state.bookmarked = super::super::agent_panel::is_bookmarked(&tab.thread_id);
            state.resume = resume;
        }
        tab.add_to_window(controller);
        Some(tab)
    }

    fn build(
        controller: &WindowController,
        agent: Agent,
        cwd: PathBuf,
        thread_id: ThreadId,
        thread: Thread,
        source: Source,
    ) -> AgentTab {
        let mtm = controller.mtm();
        let thread = Rc::new(thread);
        let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
        let target = AgentTarget::new(mtm, key);
        let this = AgentRoot::alloc(mtm).set_ivars(RootIvars { key });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let root: Retained<AgentRoot> = unsafe { msg_send![super(this), init] };
        let header = Header::new(mtm, &target);
        let timeline = Timeline::new(mtm, key, &target, thread.clone(), cwd.clone());
        let composer = Composer::new(mtm, key, &target);
        let status = StatusLine::new(mtm, key);
        #[cfg(feature = "dictation")]
        crate::dictation::register(
            crate::dictation::Kind::Composer(key),
            &composer.text,
            &composer.mic,
            &status.dictation,
        );
        let empty = overlay::empty(mtm, &agent);
        let problem_holder = NSView::new(mtm);
        let queue_view = ui::column(mtm, 4.0, &[]);
        let suggestions = Suggestions::new(mtm, &target);
        // The own agent and ACP agents take the user's last picks once their session runs.
        let choice = if matches!(agent, Agent::Claude) {
            super::choices::of(agent.id())
        } else {
            ide_core::settings::AgentChoice::default()
        };
        let tab = AgentTab {
            key,
            mtm,
            controller: Weak::from(controller),
            tab_id: std::cell::Cell::new(-1),
            store: Store::new(cwd.clone()),
            cwd,
            thread_id,
            target,
            root,
            header,
            timeline,
            composer,
            status,
            empty,
            problem_holder,
            queue_view,
            suggestions,
            agent,
            state: RefCell::new(State {
                source,
                thread,
                caps: Capabilities::default(),
                acp: None,
                own: None,
                problem: None,
                custom_title: None,
                queue: Vec::new(),
                attachments: Vec::new(),
                sandbox: false,
                remote: None,
                mode: choice.mode.unwrap_or_else(|| "default".into()),
                model: choice.model,
                effort: choice.effort,
                bookmarked: false,
                resume: None,
                checkpoints: HashMap::new(),
                turn: None,
                status: Status::Idle,
                sending: false,
                offered: Offered::default(),
                previous_focus: None,
                fork: None,
                last_sent: None,
                offer: None,
                choice_applied: false,
            }),
            inbox: RefCell::new(Arc::new(Inbox::default())),
            ticking: std::cell::Cell::new(false),
        };
        tab.lay_out();
        tab
    }

    fn lay_out(&self) {
        let mtm = self.mtm;
        let root: &NSView = &self.root;
        let line = ui::separator(mtm);
        let column = NSView::new(mtm);
        column.setTranslatesAutoresizingMaskIntoConstraints(false);
        let bottom = ui::column(
            mtm,
            6.0,
            &[
                &self.queue_view,
                &self.suggestions.view,
                &self.composer.card,
                &self.status.view,
            ],
        );
        self.status.view.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 0.0,
            left: 4.0,
            bottom: 0.0,
            right: 4.0,
        });
        for view in [&*self.header.view as &NSView, &line, &column] {
            root.addSubview(view);
        }
        for view in [
            self.timeline.view() as &NSView,
            &self.empty,
            &self.problem_holder,
            &bottom,
        ] {
            column.addSubview(view);
        }
        self.empty.setTranslatesAutoresizingMaskIntoConstraints(false);
        self.problem_holder.setTranslatesAutoresizingMaskIntoConstraints(false);
        let scroll = self.timeline.view();
        let constraints: Vec<Retained<NSLayoutConstraint>> = vec![
            self.header.view.topAnchor().constraintEqualToAnchor(&root.topAnchor()),
            self.header
                .view
                .leadingAnchor()
                .constraintEqualToAnchor(&root.leadingAnchor()),
            self.header
                .view
                .trailingAnchor()
                .constraintEqualToAnchor(&root.trailingAnchor()),
            line.topAnchor()
                .constraintEqualToAnchor(&self.header.view.bottomAnchor()),
            line.leadingAnchor().constraintEqualToAnchor(&root.leadingAnchor()),
            line.trailingAnchor().constraintEqualToAnchor(&root.trailingAnchor()),
            column.topAnchor().constraintEqualToAnchor(&line.bottomAnchor()),
            column.bottomAnchor().constraintEqualToAnchor(&root.bottomAnchor()),
            column
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&root.leadingAnchor(), SIDE_MARGIN),
            column
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&root.trailingAnchor(), -SIDE_MARGIN),
            scroll.topAnchor().constraintEqualToAnchor(&column.topAnchor()),
            scroll.leadingAnchor().constraintEqualToAnchor(&column.leadingAnchor()),
            scroll
                .trailingAnchor()
                .constraintEqualToAnchor(&column.trailingAnchor()),
            scroll
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&bottom.topAnchor(), -6.0),
            self.empty
                .centerXAnchor()
                .constraintEqualToAnchor(&scroll.centerXAnchor()),
            self.empty
                .centerYAnchor()
                .constraintEqualToAnchor(&scroll.centerYAnchor()),
            self.problem_holder
                .centerXAnchor()
                .constraintEqualToAnchor(&scroll.centerXAnchor()),
            self.problem_holder
                .centerYAnchor()
                .constraintEqualToAnchor(&scroll.centerYAnchor()),
            self.problem_holder
                .widthAnchor()
                .constraintLessThanOrEqualToAnchor(&scroll.widthAnchor()),
        ];
        for constraint in constraints.into_iter().chain(input_constraints(&bottom, &column)) {
            constraint.setActive(true);
        }
    }

    /// Adds the tab to the window's tab bar.
    fn add_to_window(&self, controller: &WindowController) {
        let view: Retained<NSView> = self.root();
        let id = controller.add_tab(TabKind::Agent, Some(self.tab_key()), self.agent.name(), view);
        self.tab_id.set(id);
        controller.set_tab_tint(id, &self.agent.colour());
        if let Some(colors) = self.agent.four_colours() {
            controller.set_tab_four_colours(id, colors);
        }
        self.header.set_agent(self.agent.name());
        self.composer.set_agent(self.agent.name());
        self.refresh();
    }

    /// The texts of the empty state, top to bottom, with whether it holds anything else, such as an image.
    #[cfg(feature = "selftest")]
    pub fn empty_state(&self) -> (Vec<String>, bool) {
        let views = self.empty.arrangedSubviews();
        let texts = views
            .iter()
            .filter_map(|view| view.downcast::<objc2_app_kit::NSTextField>().ok())
            .map(|field| field.stringValue().to_string())
            .collect();
        let other = views
            .iter()
            .any(|view| view.downcast_ref::<objc2_app_kit::NSTextField>().is_none());
        (texts, other)
    }

    pub fn tab_key(&self) -> String {
        format!("agent:{}", self.key)
    }

    pub fn root(&self) -> Retained<NSView> {
        Retained::into_super(self.root.clone())
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// Clicks across the composer and on its margin: whether each put the keyboard focus into its text.
    #[cfg(feature = "selftest")]
    pub fn composer_clicks(&self) -> Vec<bool> {
        self.composer.clicks()
    }

    /// The width of the column the conversation reads in.
    #[cfg(feature = "selftest")]
    pub fn column_width(&self) -> f64 {
        // SAFETY: Asking for the superview only reads; the column holds the timeline while the tab lives.
        unsafe { self.timeline.view().superview() }.map_or(0.0, |column| column.frame().size.width)
    }

    /// The width of the composer's card and how far its middle is off the column's.
    #[cfg(feature = "selftest")]
    pub fn composer_place(&self) -> (f64, f64) {
        let card = self.composer.card.frame();
        // SAFETY: Asking for the superview only reads; the column holds the timeline while the tab lives.
        let column = unsafe { self.timeline.view().superview() };
        let middle = |frame: NSRect| frame.origin.x + frame.size.width / 2.0;
        let off = column.map_or(f64::NAN, |column| {
            let card = self
                .composer
                .card
                .convertRect_toView(self.composer.card.bounds(), Some(&column));
            middle(card) - column.bounds().size.width / 2.0
        });
        (card.size.width, off)
    }

    pub fn controller(&self) -> Option<Retained<WindowController>> {
        self.controller.load()
    }

    pub fn belongs_to(&self, controller: &WindowController) -> bool {
        self.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller))
    }

    /// The CLI's id for the conversation, once it has one.
    pub fn session_id(&self) -> Option<String> {
        let state = self.state.borrow();
        state
            .thread
            .session()
            .map(str::to_string)
            .or_else(|| state.resume.as_ref().map(|r| r.session.clone()))
    }

    pub fn thread_id(&self) -> &ThreadId {
        &self.thread_id
    }

    /// The folder the session works in: the window's folder, or a thread's worktree.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Names the thread `title`, as the user would rename it.
    pub fn set_custom_title(&self, title: &str) {
        self.state.borrow_mut().custom_title = Some(title.to_string());
        self.refresh();
    }

    /// The title the tab and the session list show.
    pub fn title(&self) -> String {
        self_title(&self.state.borrow(), &self.agent)
    }

    /// The thread as it is now.
    pub fn snapshot(&self) -> Thread {
        self.state.borrow().thread.as_ref().clone()
    }

    /// The last state received by the UI, shared without copying its history.
    pub fn view_snapshot(&self) -> Rc<Thread> {
        self.state.borrow().thread.clone()
    }

    /// The running session, if there is one.
    pub fn session(&self) -> Option<Session> {
        self.state.borrow().source.session().filter(|s| !s.is_ended()).cloned()
    }

    /// Whether a `claude` process runs for the tab.
    #[cfg(feature = "selftest")]
    pub fn is_live(&self) -> bool {
        self.state.borrow().source.is_live()
    }

    pub fn status(&self) -> Status {
        self.state.borrow().status
    }

    /// Whether the tab is the one showing in its window.
    pub fn is_shown(&self) -> bool {
        self.root.window().is_some_and(|w| crate::window::in_front(&w)) && !self.root.isHiddenOrHasHiddenAncestor()
    }

    pub fn select(&self) {
        if let Some(controller) = self.controller() {
            controller.select_tab(&self.tab_key());
            if let Some(window) = controller.window() {
                window.makeKeyAndOrderFront(None);
            }
        }
    }

    /// Resumes presentation and live labels after a background tab comes back into view.
    pub fn show_pending(&self) {
        self.timeline.show_pending();
        self.start_ticking();
    }

    /// Looks for `claude` and the login shell's environment on a worker thread, then starts the session.
    pub fn start(&self) {
        let Some(controller) = self.controller() else { return };
        if !controller.is_trusted() {
            self.set_problem(Some(Problem::Untrusted));
            return;
        }
        if self.agent.is_own() {
            {
                let mut state = self.state.borrow_mut();
                state.source = Source::Starting;
                state.problem = None;
            }
            self.refresh();
            self.start_own();
            return;
        }
        if let Some(agent) = self.acp_agent() {
            let thread = {
                let mut state = self.state.borrow_mut();
                state.source = Source::Starting;
                state.problem = None;
                state.choice_applied = false;
                state.thread.as_ref().clone()
            };
            self.refresh();
            self.start_acp(agent, thread);
            return;
        }
        let transcript = {
            let mut state = self.state.borrow_mut();
            state.source = Source::Starting;
            state.problem = None;
            state.resume.as_ref().and_then(|r| r.transcript.clone())
        };
        self.refresh();
        let key = self.key;
        let thread = self.snapshot();
        let _ = std::thread::Builder::new().name("claude-start".into()).spawn(move || {
            let env = crate::environment::wait(Duration::from_secs(15))
                .map(<[(String, String)]>::to_vec)
                .unwrap_or_default();
            let exe = source::find_claude(&env);
            let thread = match transcript {
                Some(path) if thread.entries(&ide_thread::Lane::Main).is_empty() => {
                    ide_claude::history::rebuild(thread.clone(), &path).unwrap_or(thread)
                }
                _ => thread,
            };
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.launch(exe, env, thread);
                }
            });
        });
    }

    fn launch(&self, exe: Option<PathBuf>, env: Vec<(String, String)>, thread: Thread) {
        let Some(exe) = exe else {
            self.state.borrow_mut().source = Source::Idle;
            self.set_problem(Some(Problem::NotFound));
            return;
        };
        let mut launch = Launch::new(exe, env, &self.cwd);
        {
            let state = self.state.borrow();
            launch.resume = thread
                .session()
                .map(str::to_string)
                .or_else(|| state.resume.as_ref().map(|r| r.session.clone()));
            launch.sandbox = state.sandbox;
            launch.remote_control = state.remote;
            launch.permission_mode = Some(state.mode.clone());
            launch.allow_bypass = true;
            launch.model.clone_from(&state.model);
            launch.effort.clone_from(&state.effort);
            launch.file_checkpoints = true;
            match &state.fork {
                Some(Fork {
                    session,
                    rewind: Some(rewind),
                    ..
                }) => launch.rewind(session, rewind),
                Some(Fork { rewind: None, .. }) => launch.resume = None,
                None => {}
            }
        }
        let sink = self.sink(Incoming::Claude);
        let started = Session::start(Supervisor::global(), &launch, thread.clone(), sink);
        match started {
            Ok(session) => {
                {
                    let mut state = self.state.borrow_mut();
                    state.source = Source::Live(session);
                    state.problem = None;
                }
                self.absorb(thread, &[Change::Cleared]);
                self.send_queued();
            }
            Err(error) => {
                self.state.borrow_mut().source = Source::Idle;
                let problem = match error {
                    ide_claude::SessionError::NotFound(_) => Problem::NotFound,
                    other => Problem::Start(other.to_string()),
                };
                self.set_problem(Some(problem));
            }
        }
    }

    /// Where a new session reports to: a fresh inbox, emptied on the main thread shortly after something arrives.
    pub(super) fn sink<E: Send + 'static>(&self, wrap: fn(E) -> Incoming) -> impl Fn(E) + Send + Sync + 'static {
        let inbox = Arc::new(Inbox::default());
        *self.inbox.borrow_mut() = inbox.clone();
        let key = self.key;
        move |event: E| {
            inbox.events.lock().unwrap_or_else(|e| e.into_inner()).push(wrap(event));
            if !inbox.scheduled.swap(true, Ordering::SeqCst) {
                let scheduled = inbox.clone();
                main_after(Duration::from_millis(30), move || {
                    if let Some(tab) = super::get(key) {
                        let current = Arc::ptr_eq(&tab.inbox.borrow(), &scheduled);
                        if current {
                            tab.flush();
                        }
                    }
                });
            }
        }
    }

    /// Takes in what the session reported since the last time.
    fn flush(&self) {
        let inbox = self.inbox.borrow().clone();
        inbox.scheduled.store(false, Ordering::SeqCst);
        let pending = std::mem::take(&mut *inbox.events.lock().unwrap_or_else(|e| e.into_inner()));
        if pending.is_empty() {
            return;
        }
        let changes: Vec<Change> = pending.changes.into_iter().collect();
        let mut problem = None;
        let mut learned = false;
        let mut remote = false;
        let mut refused = None;
        for event in pending.events {
            match &event {
                Incoming::Claude(Event::Changed(c))
                | Incoming::Acp(ide_acp::Event::Changed(c))
                | Incoming::Own(ide_agent::Event::Changed(c)) => {
                    debug_assert!(c.is_empty(), "invalidations are coalesced before scheduling");
                }
                Incoming::Own(other) => self.own_event(other),
                Incoming::Claude(Event::Capabilities) | Incoming::Acp(ide_acp::Event::Capabilities) => learned = true,
                Incoming::Claude(Event::RemoteControl) => remote = true,
                Incoming::Claude(Event::Failed(SessionError::RewindRejected(why))) => refused = Some(why.clone()),
                Incoming::Claude(event @ Event::Failed(_)) => problem = source::problem_of(event),
                Incoming::Acp(ide_acp::Event::Failed(error)) => problem = self.acp_problem(error),
                Incoming::Claude(Event::Ended { .. }) | Incoming::Acp(ide_acp::Event::Ended { .. }) => {}
            }
        }
        if let Some(why) = refused {
            self.rewind_refused(&why);
            return;
        }
        let snapshot = {
            let mut state = self.state.borrow_mut();
            if learned && let Some(caps) = state.source.capabilities() {
                state.caps = caps;
            }
            if learned && let Some(caps) = state.source.acp_capabilities() {
                state.acp = Some(caps);
            }
            state.source.snapshot()
        };
        if learned && self.agent.is_acp() {
            self.acp_apply_choice();
        }
        if problem.is_some() {
            self.set_problem(problem);
        }
        if let Some(snapshot) = snapshot {
            self.absorb(snapshot, &changes);
        }
        if learned {
            self.refresh();
        } else if remote {
            self.refresh_remote();
        }
    }

    /// What an ACP session's failure means for the tab; failures of a running session's requests are only logged.
    fn acp_problem(&self, error: &ide_acp::SessionError) -> Option<Problem> {
        let ready = self.state.borrow().source.is_live();
        match error {
            ide_acp::SessionError::SignIn => Some(Problem::AcpSignIn),
            ide_acp::SessionError::Version(v) => Some(Problem::Start(tf(
                "{} spricht die Protokollversion {}, diese App Version 1",
                &[self.agent.name(), &v.to_string()],
            ))),
            other if !ready => Some(Problem::Start(other.to_string())),
            other => {
                crate::diag::report("a request to the agent", other);
                None
            }
        }
    }

    /// Shows the thread as `snapshot` has it and reacts to how its status moved.
    pub(super) fn absorb(&self, snapshot: Thread, changes: &[Change]) {
        let snapshot = Rc::new(snapshot);
        let refresh = super::refresh::Refresh::of(&snapshot, changes);
        let before = self.state.borrow().status;
        let now = snapshot.status();
        let title_changed = snapshot.title() != self.state.borrow().thread.title();
        {
            let mut state = self.state.borrow_mut();
            state.thread = snapshot.clone();
            state.status = now;
        }
        let had_rows = self.timeline.count() > 0;
        self.timeline.apply(snapshot, changes);
        if refresh.controls
            || changes.is_empty()
            || title_changed
            || before != now
            || had_rows != (self.timeline.count() > 0)
        {
            self.refresh();
        } else if refresh.status {
            self.status.show(&self.state.borrow().thread);
        }
        if refresh.plans {
            self.refresh_plan_docs();
        }
        if before != now {
            self.status_moved(before, now);
        }
        if title_changed || before != now {
            self.persist();
        }
        if now != Status::Idle {
            self.start_ticking();
        }
        if refresh.overview
            && let Some(controller) = self.controller()
        {
            super::changed(&controller);
        }
    }

    fn status_moved(&self, before: Status, now: Status) {
        let title = self.title();
        let elsewhere = notify::user_looks_elsewhere(self.is_shown());
        match now {
            Status::Idle if before != Status::Idle => {
                if let Some(controller) = self.controller() {
                    crate::worktrees::turn_ended(&controller, &self.cwd);
                }
                let turn = self.state.borrow_mut().turn.take();
                if let Some(checkpoint) = turn {
                    let key = self.key;
                    let started = checkpoint.commit.clone();
                    self.store.finish(checkpoint, move |finished| {
                        main_soon(move || {
                            if let Some(tab) = super::get(key) {
                                for kept in tab.state.borrow_mut().checkpoints.values_mut() {
                                    if kept.commit == started {
                                        *kept = finished.clone();
                                    }
                                }
                            }
                        });
                    });
                }
                if elsewhere {
                    notify::post(&self.tab_key(), &title, &tf("{} ist fertig.", &[self.agent.name()]));
                }
                self.send_queued();
            }
            Status::RequiresAction if elsewhere => {
                notify::post(
                    &self.tab_key(),
                    &title,
                    &tf("{} wartet auf deine Antwort.", &[self.agent.name()]),
                );
            }
            _ => {}
        }
        self.refresh_tab_mark();
    }

    /// Records the session in the index the AI panel lists.
    pub(super) fn persist(&self) {
        let state = self.state.borrow();
        if state.thread.session().is_none() && state.thread.entries(&ide_thread::Lane::Main).is_empty() {
            return;
        }
        let mut index = ThreadIndex::open(&ThreadIndex::default_dir());
        let _ = index.record(&state.thread, SystemTime::now());
        if let Some(title) = &state.custom_title {
            let _ = index.rename(&self.thread_id, title);
        }
    }

    /// Advances running times every second while anything runs.
    fn start_ticking(&self) {
        if self.ticking.replace(true) {
            return;
        }
        tick(self.key);
    }

    pub fn set_problem(&self, problem: Option<Problem>) {
        self.state.borrow_mut().problem = problem;
        self.refresh();
    }

    /// Brings the head, the composer, the status line and the overlays up to date with the state.
    pub fn refresh(&self) {
        let (problem, thread, title) = {
            let state = self.state.borrow();
            (
                state.problem.clone(),
                state.thread.clone(),
                self_title(&state, &self.agent),
            )
        };
        self.header.set_title(&title);
        self.header.set_bookmarked(self.state.borrow().bookmarked);
        self.refresh_remote();
        if let Some(controller) = self.controller() {
            controller.set_tab_title(self.tab_id.get(), &super::format::first_line(&title, 32));
        }
        if matches!(self.agent, Agent::Claude)
            && let Some(session) = self.session()
        {
            super::usage::fetch(&session);
        }
        self.status.show(&thread);
        self.refresh_composer();
        let empty = self.timeline.count() == 0;
        self.empty.setHidden(!empty || problem.is_some());
        for view in self.problem_holder.subviews().iter() {
            view.removeFromSuperview();
        }
        if let Some(problem) = &problem {
            let methods = self.state.borrow().acp.clone().unwrap_or_default().auth_methods;
            let view = overlay::problem(self.mtm, &self.target, problem, self.agent.name(), &methods);
            self.problem_holder.addSubview(&view);
            crate::util::pin_edges(&view, &self.problem_holder);
        }
        self.problem_holder.setHidden(problem.is_none());
        let blocked = matches!(problem, Some(Problem::Untrusted | Problem::NotFound));
        self.composer.card.setHidden(blocked);
        self.refresh_tab_mark();
    }

    fn refresh_remote(&self) {
        let remote = self.session().map(|s| s.remote_control()).unwrap_or_default();
        self.header.set_remote(&remote);
    }

    fn refresh_tab_mark(&self) {
        let Some(controller) = self.controller() else { return };
        controller.set_tab_status(self.tab_id.get(), self.tab_mark().as_deref());
    }

    /// The colour of the dot on the tab: green while the agent works, red while it waits for the user, none when idle.
    pub fn tab_mark(&self) -> Option<Retained<NSColor>> {
        match self.state.borrow().status {
            Status::Running => Some(NSColor::systemGreenColor()),
            Status::RequiresAction => Some(NSColor::systemRedColor()),
            Status::Idle => None,
        }
    }

    /// Ends the session, as the tab closes.
    pub fn close(&self) {
        self.persist();
        let source = std::mem::replace(&mut self.state.borrow_mut().source, Source::Idle);
        source.end();
    }

    /// Stops the current turn; background work goes on (E23).
    pub fn stop_turn(&self) {
        self.state.borrow().source.interrupt();
    }

    pub fn composer_has_focus(&self) -> bool {
        self.composer.has_focus()
    }

    /// Puts `text` into the composer for the user to read and send, and moves the keyboard focus there.
    pub fn set_draft(&self, text: &str) {
        self.composer.set_text(text);
        self.focus_composer();
    }

    /// Adds text from outside to the next message, such as a CI log.
    pub fn attach_outside(&self, attachment: ide_thread::Attachment) {
        self.state.borrow_mut().attachments.push(attachment);
        self.refresh_chips();
    }

    pub fn focus_composer(&self) {
        let Some(window) = self.root.window() else { return };
        let previous = window.firstResponder().map(|r| Weak::from(&*r));
        self.state.borrow_mut().previous_focus = previous;
        window.makeFirstResponder(Some(&self.composer.text));
    }

    pub fn leave_focus(&self) {
        let Some(window) = self.root.window() else { return };
        let previous = self.state.borrow_mut().previous_focus.take().and_then(|w| w.load());
        window.makeFirstResponder(previous.as_deref());
    }

    pub fn composer_focus(&self, focused: bool) {
        self.composer.show_focus(focused);
        if focused {
            self.offer_selection();
        }
    }
}

/// The composer and what stands above and below it at the foot of `column`: as wide as the column but for its inset,
/// at most `INPUT_WIDTH`, in the middle.
fn input_constraints(bottom: &NSView, column: &NSView) -> Vec<Retained<NSLayoutConstraint>> {
    let fill = bottom
        .widthAnchor()
        .constraintEqualToAnchor_constant(&column.widthAnchor(), -2.0 * INPUT_INSET);
    fill.setPriority(FILL_PRIORITY);
    vec![
        bottom.centerXAnchor().constraintEqualToAnchor(&column.centerXAnchor()),
        bottom
            .leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&column.leadingAnchor(), INPUT_INSET),
        bottom.widthAnchor().constraintLessThanOrEqualToConstant(INPUT_WIDTH),
        fill,
        bottom
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&column.bottomAnchor(), -8.0),
    ]
}

/// The session's title: the user's, else the agent's, else the first line of the first message.
fn self_title(state: &State, agent: &Agent) -> String {
    let first_message = || {
        state
            .thread
            .entries(&ide_thread::Lane::Main)
            .iter()
            .find_map(|e| match &e.kind {
                ide_thread::EntryKind::UserMessage(m) => {
                    Some(ide_thread::title_from(&m.text)).filter(|t| !t.is_empty())
                }
                _ => None,
            })
    };
    state
        .custom_title
        .clone()
        .filter(|t| !t.is_empty())
        .or_else(|| Some(state.thread.title().to_string()).filter(|t| !t.is_empty()))
        .or_else(first_message)
        .unwrap_or_else(|| match agent {
            Agent::Acp(agent) => agent.name.clone(),
            Agent::Claude | Agent::Own => t("Neue Sitzung").to_string(),
        })
}

/// One second of running time for the tab `key`, again while anything runs.
fn tick(key: u64) {
    let Some(tab) = super::get(key) else { return };
    let live = tab.timeline.tick(SystemTime::now());
    let running = tab.status() != Status::Idle;
    if running && let Some(session) = tab.session() {
        super::usage::fetch(&session);
    }
    if live || running {
        main_after(Duration::from_secs(1), move || tick(key));
    } else {
        tab.ticking.set(false);
    }
    if running && let Some(controller) = tab.controller() {
        crate::activity::refresh(&controller);
    }
}

/// What the self-test reads from a tab.
#[cfg(feature = "selftest")]
pub struct Inspection {
    pub rows: usize,
    pub built: usize,
    pub visible: Vec<String>,
    pub pills: Vec<String>,
    pub composer: bool,
    pub status: Vec<String>,
    pub title: String,
    pub subagents: Vec<String>,
    pub background: Vec<String>,
    /// The tooltip of the head's Remote Control button, while it shows.
    pub remote: Option<String>,
}

#[cfg(feature = "selftest")]
impl AgentTab {
    /// A tab that plays recordings, with no process behind it.
    pub fn open_replay(controller: &WindowController) -> Option<Rc<AgentTab>> {
        let cwd = controller.project_root().or_else(|| controller.project())?;
        Some(AgentTab::open_replay_in(controller, cwd))
    }

    /// A tab that plays recordings in `cwd`, for the self-test.
    pub fn open_replay_in(controller: &WindowController, cwd: PathBuf) -> Rc<AgentTab> {
        let thread_id = ThreadId::new();
        let thread = Thread::new(thread_id.clone(), ide_claude::AGENT, &cwd);
        let source = source::replay(thread.clone());
        let tab = Rc::new(AgentTab::build(
            controller,
            Agent::Claude,
            cwd,
            thread_id,
            thread,
            source,
        ));
        tab.add_to_window(controller);
        tab
    }

    /// Plays recorded lines.
    pub fn feed(&self, lines: &str) {
        let (changes, snapshot) = {
            let mut state = self.state.borrow_mut();
            let changes = state.source.feed(lines);
            (changes, state.source.snapshot())
        };
        if let Some(snapshot) = snapshot {
            self.absorb(snapshot, &changes);
        }
    }

    /// Shows `title` in the head for a moment and measures it as [`Header::title_widths`] does.
    pub fn title_widths(&self, title: &str) -> (f64, f64, f64) {
        self.header.set_title(title);
        let widths = self.header.title_widths();
        self.refresh();
        widths
    }

    pub fn inspect(&self) -> Inspection {
        let thread = self.snapshot();
        Inspection {
            rows: self.timeline.count(),
            built: self.timeline.built(),
            visible: self.timeline.visible_summaries(),
            pills: [&self.composer.model, &self.composer.mode]
                .iter()
                .map(|b| b.title().to_string())
                .collect(),
            composer: self.composer.card.window().is_some() && !self.composer.card.isHidden(),
            status: self.status.texts(),
            title: self.title(),
            subagents: thread.subagents().iter().map(|s| s.description.clone()).collect(),
            background: thread.background().iter().map(|b| b.label.clone()).collect(),
            remote: self.header.remote_tip(),
        }
    }

    /// Keeps the thread off Remote Control whatever the user's Claude settings say, so a live self-test registers
    /// nothing on claude.ai.
    #[cfg(feature = "selftest")]
    pub fn keep_remote_control_off(&self) {
        self.state.borrow_mut().remote = Some(false);
    }

    /// Picks the model for the next start.
    pub fn use_model(&self, model: &str) {
        self.state.borrow_mut().model = Some(model.to_string());
    }

    /// Picks `model`, `effort` and `mode` as the pills' menus do, which remembers them for the agent's next sessions.
    #[cfg(feature = "selftest")]
    pub fn pick_all(&self, model: &str, effort: &str, mode: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.offered.models = vec![model.to_string()];
            state.offered.efforts = vec![effort.to_string()];
            state.offered.modes = vec![mode.to_string()];
        }
        self.pick_model(0);
        self.pick_effort(0);
        self.pick_mode(0);
    }

    /// Picks the mode `mode` as the mode pill's menu does, which remembers it for the agent's next sessions.
    #[cfg(feature = "selftest")]
    pub fn pick_mode_id(&self, mode: &str) {
        self.state.borrow_mut().offered.modes = vec![mode.to_string()];
        self.pick_mode(0);
    }

    /// The model, effort and mode the session runs with or starts with.
    #[cfg(feature = "selftest")]
    pub fn picks(&self) -> (Option<String>, Option<String>, String) {
        let state = self.state.borrow();
        (state.model.clone(), state.effort.clone(), state.mode.clone())
    }

    /// Switches to the permission mode `mode`, as the mode pill's menu does.
    #[cfg(feature = "selftest")]
    pub fn use_mode(&self, mode: &str) {
        self.state.borrow().source.set_mode(mode, |_| ());
        self.state.borrow_mut().mode = mode.to_string();
        self.refresh_composer();
    }

    /// The views that show at the foot of the tab besides the composer and its status line, with their heights.
    #[cfg(feature = "selftest")]
    pub fn foot_extras(&self) -> Option<Vec<(String, f64)>> {
        // SAFETY: Asking for the superview only reads; the foot holds the composer while the tab lives.
        let foot = unsafe { self.composer.card.superview() }.and_then(|v| v.downcast::<NSStackView>().ok())?;
        foot.layoutSubtreeIfNeeded();
        let own: [&NSView; 2] = [&self.composer.card, &self.status.view];
        Some(
            foot.arrangedSubviews()
                .iter()
                .filter(|view| !view.isHidden() && !own.iter().any(|o| std::ptr::eq(*o, &**view)))
                .map(|view| {
                    (
                        view.class().name().to_string_lossy().into_owned(),
                        view.frame().size.height,
                    )
                })
                .collect(),
        )
    }

    /// Types `text` into the composer and sends it, as Return would.
    pub fn type_and_send(&self, text: &str) {
        self.composer.set_text(text);
        self.submit();
    }

    /// Attaches the files at `paths` as the `+` button's panel would.
    pub fn attach_files(&self, paths: &[PathBuf]) {
        let picked: Vec<ide_thread::Attachment> = paths
            .iter()
            .map(|path| super::outside::classify(super::actions::file_attachment(path), &self.cwd))
            .collect();
        self.state.borrow_mut().attachments.extend(picked);
        self.refresh_chips();
    }

    /// What the next message takes along.
    pub fn pending_attachments(&self) -> Vec<ide_thread::Attachment> {
        self.state.borrow().attachments.clone()
    }

    /// The text of the next message's attachments from outside.
    pub fn outside_texts(&self) -> Vec<String> {
        self.state
            .borrow()
            .attachments
            .iter()
            .filter_map(|a| match a {
                ide_thread::Attachment::External { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// What the composer holds.
    pub fn composer_text(&self) -> String {
        self.composer.text()
    }

    /// Types `text` into the composer, the caret at its end, as a user would.
    pub fn type_in_composer(&self, text: &str) {
        self.composer.set_text(text);
        let end = text.encode_utf16().count();
        self.composer
            .text
            .setSelectedRange(objc2_foundation::NSRange::new(end, 0));
        self.composer_changed();
    }

    /// The completions the list shows.
    pub fn suggestion_labels(&self) -> Vec<String> {
        if self.suggestions.is_shown() {
            self.suggestions.labels()
        } else {
            Vec::new()
        }
    }

    /// Whether the editor's selection is offered as context.
    pub fn has_offer(&self) -> bool {
        self.state.borrow().offer.is_some()
    }

    /// The attachments of the next message.
    pub fn attachment_names(&self) -> Vec<String> {
        self.state
            .borrow()
            .attachments
            .iter()
            .map(|a| match a {
                ide_thread::Attachment::Image { .. } => "image".to_string(),
                ide_thread::Attachment::File { path, .. } | ide_thread::Attachment::Selection { path, .. } => {
                    path.display().to_string()
                }
                ide_thread::Attachment::External { label, .. } => format!("{label} (external)"),
            })
            .collect()
    }

    /// The size of each picture tile above the composer's text, with the width of the picture it shows.
    #[cfg(feature = "selftest")]
    pub fn thumbnails(&self) -> Vec<(objc2_foundation::NSSize, f64)> {
        fn picture(view: &objc2_app_kit::NSView) -> Option<f64> {
            if let Some(image) = view
                .downcast_ref::<objc2_app_kit::NSImageView>()
                .and_then(|v| v.image())
                .filter(|image| !image.isTemplate())
            {
                return Some(image.size().width);
            }
            view.subviews().iter().find_map(|child| picture(&child))
        }
        let chips = &self.composer.chips;
        chips.layoutSubtreeIfNeeded();
        chips
            .arrangedSubviews()
            .iter()
            .filter_map(|chip| picture(&chip).map(|width| (chip.frame().size, width)))
            .collect()
    }

    /// Clicks the first picture tile above the composer's text; the size of the popover that opens, closed again.
    #[cfg(feature = "selftest")]
    pub fn enlarge_thumbnail(&self) -> Option<objc2_foundation::NSSize> {
        fn clear_button(view: &objc2_app_kit::NSView) -> Option<Retained<objc2_app_kit::NSButton>> {
            if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>()
                && button.isTransparent()
            {
                return Some(objc2::Message::retain(button));
            }
            view.subviews().iter().find_map(|child| clear_button(&child))
        }
        let button = clear_button(&self.composer.chips)?;
        // SAFETY: The button's target is the opener, which lives as long as the app.
        unsafe { button.performClick(None) };
        super::enlarge::take_shown()
    }

    /// Whether a rewind waits for the agent to accept it.
    pub fn rewind_pending(&self) -> bool {
        self.state.borrow().fork.is_some()
    }

    /// How many checkpoints were taken before prompts.
    pub fn checkpoint_count(&self) -> usize {
        self.state.borrow().checkpoints.len()
    }

    /// Opens every card part with `part`, as a user would click them.
    pub fn open_all(&self, part: super::cards::Part) {
        let ids: Vec<u64> = self
            .snapshot()
            .entries(&ide_thread::Lane::Main)
            .iter()
            .map(|e| e.id.get())
            .collect();
        for id in ids {
            self.timeline.toggle(id, part);
        }
    }
}
