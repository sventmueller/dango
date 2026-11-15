//! One AI tab: its session, the views showing it, and how session events reach them.

use ide_core::LockIgnoringPoison;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use ide_claude::{Event, Session};
use ide_thread::{Change, Status, Thread, ThreadId};
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSEvent, NSLayoutConstraint, NSResponder, NSStackView, NSView};
use objc2_foundation::{NSObjectProtocol, NSRect};
use splat_i18n::t;

use super::AgentKey;
use super::acp::Agent;
use super::checkpoint::Store;
use super::composer::Composer;
use super::header::Header;
use super::model::State;
use super::source::{Problem, Resume, Source};
use super::status::StatusLine;
use super::suggest::Suggestions;
use super::target::AgentTarget;
use super::timeline::Timeline;
use super::{overlay, ui};
use ide_ui_common::util::{guarded, main_after, main_soon};
use ide_ui_common::window::{Notices, TabDecor, TabId, TabKind, Tabs, Window};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// The room on either side of the conversation, which takes the rest of the tab's width.
pub const SIDE_MARGIN: f64 = 16.0;
/// The widest the composer gets; it stands in the middle below a wider conversation.
pub const INPUT_WIDTH: f64 = 852.0;
/// The room between the composer and the conversation's edges.
pub const INPUT_INSET: f64 = 14.0;
/// How strongly the composer fills the conversation's width up to `INPUT_WIDTH`: above the content's hugging and the
/// holding priority of the window's content, below the sidebar's, so a wide window neither narrows nor moves it.
const FILL_PRIORITY: f32 = 255.0;

/// What a session reported, from either kind of connection.
pub enum Incoming {
    Claude(Event),
    Acp(ide_acp::Event),
    Own(ide_agent::Event),
}

/// Session events waiting for the main thread.
#[derive(Default)]
pub struct Inbox {
    pub(super) events: Mutex<super::inbox::Pending>,
    pub(super) scheduled: AtomicBool,
}

pub struct RootIvars {
    key: AgentKey,
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
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
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

pub struct AgentTab {
    pub key: AgentKey,
    /// The agent the tab runs, for good.
    pub(super) agent: Agent,
    pub(super) mtm: MainThreadMarker,
    pub(super) controller: Window,
    pub(super) tab_id: std::cell::Cell<Option<TabId>>,
    pub(super) cwd: PathBuf,
    pub(super) thread_id: ThreadId,
    pub(super) target: Retained<AgentTarget>,
    pub(super) root: Retained<AgentRoot>,
    pub(super) header: Header,
    pub(super) timeline: Timeline,
    pub(super) composer: Composer,
    pub(super) status: StatusLine,
    pub(super) empty: Retained<NSStackView>,
    pub(super) problem_holder: Retained<NSView>,
    pub(super) queue_view: Retained<NSStackView>,
    pub(super) suggestions: Suggestions,
    pub(super) state: RefCell<State>,
    pub(super) inbox: RefCell<Arc<Inbox>>,
    pub(super) store: Store,
    pub(super) ticking: std::cell::Cell<bool>,
    /// Where the keyboard focus was before ⌘Esc moved it into the composer.
    pub(super) previous_focus: RefCell<Option<Weak<NSResponder>>>,
}

impl AgentTab {
    /// A new tab in `controller`'s window running `agent`, not started yet.
    pub fn open(controller: &Window, agent: Agent, resume: Option<Resume>) -> Option<Rc<AgentTab>> {
        let cwd = controller.project_root().or_else(|| controller.project())?;
        AgentTab::open_in(controller, agent, cwd, resume)
    }

    /// A new tab in `controller`'s window running `agent` in `cwd`, such as a thread's worktree, not started yet.
    pub fn open_in(controller: &Window, agent: Agent, cwd: PathBuf, resume: Option<Resume>) -> Option<Rc<AgentTab>> {
        let thread_id = resume.as_ref().and_then(|r| r.thread.clone()).unwrap_or_default();
        let thread = Thread::new(thread_id.clone(), agent.id(), &cwd);
        let tab = Rc::new(AgentTab::build(controller, agent, cwd, thread_id, thread, Source::Idle));
        let bookmarked = crate::agent_panel::is_bookmarked(&tab.thread_id);
        tab.state.borrow_mut().opened(resume, bookmarked);
        crate::agent_panel::read_past(&tab.cwd);
        tab.add_to_window(controller);
        Some(tab)
    }

    pub(super) fn build(
        controller: &Window,
        agent: Agent,
        cwd: PathBuf,
        thread_id: ThreadId,
        thread: Thread,
        source: Source,
    ) -> AgentTab {
        let mtm = controller.mtm();
        let thread = Rc::new(thread);
        let key = super::next_key();
        let target = AgentTarget::new(mtm, key);
        let this = AgentRoot::alloc(mtm).set_ivars(RootIvars { key });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let root: Retained<AgentRoot> = unsafe { msg_send![super(this), init] };
        let header = Header::new(mtm, &target);
        let timeline = Timeline::new(mtm, key, &target, thread.clone(), cwd.clone());
        let composer = Composer::new(mtm, key, &target);
        let status = StatusLine::new(mtm, key);
        #[cfg(feature = "dictation")]
        ide_ui_dictation::register(
            Rc::new(ComposerField(key)),
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
        let state = RefCell::new(State::new(&agent, source, thread, choice));
        let tab = AgentTab {
            key,
            mtm,
            controller: *controller,
            tab_id: std::cell::Cell::new(None),
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
            state,
            inbox: RefCell::new(Arc::new(Inbox::default())),
            ticking: std::cell::Cell::new(false),
            previous_focus: RefCell::new(None),
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
    pub(super) fn add_to_window(&self, controller: &Window) {
        let view: Retained<NSView> = self.root();
        let id = controller.add_tab(TabKind::Agent, Some(self.tab_key()), self.agent.name(), view);
        self.tab_id.set(id);
        if let Some(id) = id {
            controller.set_tab_tint(id, &self.agent.colour());
            if let Some(colors) = self.agent.four_colours() {
                controller.set_tab_four_colours(id, colors);
            }
        }
        self.composer.set_agent(self.agent.name());
        self.refresh();
    }

    pub fn tab_key(&self) -> String {
        format!("agent:{}", self.key.get())
    }

    pub fn root(&self) -> Retained<NSView> {
        Retained::into_super(self.root.clone())
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn controller(&self) -> Option<Window> {
        self.controller.load()
    }

    pub fn belongs_to(&self, controller: &Window) -> bool {
        self.controller.is_open() && self.controller == *controller
    }

    /// The CLI's id for the conversation, once it has one.
    pub fn session_id(&self) -> Option<String> {
        let state = self.state.borrow();
        state
            .thread()
            .session()
            .map(str::to_string)
            .or_else(|| state.resume().map(|r| r.session.clone()))
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
        self.state.borrow_mut().set_custom_title(title.to_string());
        self.refresh();
    }

    /// The title the tab and the session list show.
    pub fn title(&self) -> String {
        self.state.borrow().title(&untitled(&self.agent))
    }

    /// The thread as it is now.
    pub fn snapshot(&self) -> Thread {
        self.state.borrow().thread().as_ref().clone()
    }

    /// The last state received by the UI, shared without copying its history.
    pub fn view_snapshot(&self) -> Rc<Thread> {
        self.state.borrow().thread().clone()
    }

    /// The running session, if there is one.
    pub fn session(&self) -> Option<Session> {
        self.state
            .borrow()
            .source()
            .session()
            .filter(|s| !s.is_ended())
            .cloned()
    }

    pub fn status(&self) -> Status {
        self.state.borrow().status()
    }

    /// Whether the tab is the one showing in its window.
    pub fn is_shown(&self) -> bool {
        self.root.window().is_some_and(|w| ide_ui_common::panel::in_front(&w))
            && !self.root.isHiddenOrHasHiddenAncestor()
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

    /// Where a new session reports to: a fresh inbox, emptied on the main thread shortly after something arrives.
    pub(super) fn sink<E: Send + 'static>(&self, wrap: fn(E) -> Incoming) -> impl Fn(E) + Send + Sync + 'static {
        let inbox = Arc::new(Inbox::default());
        *self.inbox.borrow_mut() = inbox.clone();
        let key = self.key;
        move |event: E| {
            inbox.events.lock_ignoring_poison().push(wrap(event));
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

    /// Shows the thread as `snapshot` has it and reacts to how its status moved.
    pub(super) fn absorb(&self, snapshot: Thread, changes: &[Change]) {
        let _interval = ide_process::signpost::begin(ide_process::signpost::name!("agent update"));
        let snapshot = Rc::new(snapshot);
        let refresh = super::refresh::Refresh::of(&snapshot, changes);
        let before = self.state.borrow().status();
        let now = snapshot.status();
        let delegated_before = delegates(self.state.borrow().thread());
        let delegated_now = delegates(&snapshot);
        let title_changed = snapshot.title() != self.state.borrow().thread().title();
        self.state.borrow_mut().absorbed(snapshot.clone());
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
            self.status.show(self.state.borrow().thread());
        }
        if refresh.plans {
            self.refresh_plan_docs();
        }
        if before != now {
            self.status_moved(before, now);
        }
        if delegated_before != delegated_now {
            self.refresh_tab_mark();
        }
        self.work_moved(
            before != Status::Idle || delegated_before,
            now != Status::Idle || delegated_now,
        );
        if title_changed || before != now {
            self.persist();
        }
        if now != Status::Idle {
            self.start_ticking();
        }
        if refresh.overview {
            self.refresh_work();
            if let Some(controller) = self.controller() {
                super::changed(&controller);
            }
        }
    }

    /// Records the session in the index the AI panel lists.
    pub(super) fn persist(&self) {
        let state = self.state.borrow();
        if state.thread().session().is_none() && state.thread().entries(&ide_thread::Lane::Main).is_empty() {
            return;
        }
        crate::agent_panel::record(
            ide_thread::IndexEntry::of(state.thread(), SystemTime::now()),
            state.thread().usage().subscription,
            state.custom_title().map(str::to_string),
        );
    }

    /// Takes in the bookmarks, read after the tab opened.
    pub(super) fn bookmarks_read(&self) {
        let on = crate::agent_panel::is_bookmarked(&self.thread_id);
        self.state.borrow_mut().set_bookmarked(on);
        self.header.set_bookmarked(on);
    }

    pub fn set_problem(&self, problem: Option<Problem>) {
        self.state.borrow_mut().set_problem(problem);
        self.refresh();
    }

    /// Shows that the session did not start, for `problem`.
    pub(super) fn start_failed(&self, problem: Problem) {
        self.state.borrow_mut().failed(problem);
        self.refresh();
    }

    /// Brings the head, the composer, the status line and the overlays up to date with the state.
    pub fn refresh(&self) {
        let (problem, thread, title) = {
            let state = self.state.borrow();
            (
                state.problem().cloned(),
                state.thread().clone(),
                state.title(&untitled(&self.agent)),
            )
        };
        self.header.set_title(&title);
        self.header.set_bookmarked(self.state.borrow().bookmarked());
        self.refresh_remote();
        self.refresh_work();
        if let (Some(controller), Some(tab)) = (self.controller(), self.tab_id.get()) {
            controller.set_tab_title(tab, &super::format::first_line(&title, 32));
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
            let methods = self.state.borrow().acp_caps().cloned().unwrap_or_default().auth_methods;
            let view = overlay::problem(self.mtm, &self.target, problem, self.agent.name(), &methods);
            self.problem_holder.addSubview(&view);
            ide_ui_common::util::pin_edges(&view, &self.problem_holder);
        }
        self.problem_holder.setHidden(problem.is_none());
        let blocked = matches!(problem, Some(Problem::Untrusted | Problem::NotFound));
        self.composer.card.setHidden(blocked);
        self.refresh_tab_mark();
    }

    pub(super) fn refresh_remote(&self) {
        let remote = self.session().map(|s| s.remote_control()).unwrap_or_default();
        self.header.set_remote(&remote);
    }

    pub(super) fn refresh_tab_mark(&self) {
        let (Some(controller), Some(tab)) = (self.controller(), self.tab_id.get()) else {
            return;
        };
        controller.set_tab_status(tab, self.tab_mark().as_deref());
    }

    /// The colour of the dot on the tab: green while the agent or one of its subagents works, red while it waits for
    /// the user, none when idle.
    pub fn tab_mark(&self) -> Option<Retained<NSColor>> {
        let state = self.state.borrow();
        match state.status() {
            Status::Running => Some(NSColor::systemGreenColor()),
            Status::RequiresAction => Some(NSColor::systemRedColor()),
            Status::Idle if delegates(state.thread()) => Some(NSColor::systemGreenColor()),
            Status::Idle => None,
        }
    }

    /// Ends the session, as the tab closes.
    pub fn close(&self) {
        self.persist();
        let source = self.state.borrow_mut().end();
        source.end();
    }

    /// Stops the current turn, as `by` asked; background work goes on (E23). The window's foot says what stopped it.
    pub fn stop_turn(&self, by: super::Stop) {
        if self.status() == Status::Idle {
            return;
        }
        ide_ui_common::diag::report("stopping a turn", format!("{by:?}"));
        super::stop::record(by, &self.title());
        self.controller.show_notice(by.notice());
        self.state.borrow().source().interrupt();
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
        self.state.borrow_mut().attach([attachment]);
        self.refresh_chips();
    }

    pub fn focus_composer(&self) {
        let Some(window) = self.root.window() else { return };
        let previous = window.firstResponder().map(|r| Weak::from(&*r));
        *self.previous_focus.borrow_mut() = previous;
        window.makeFirstResponder(Some(&self.composer.text));
    }

    pub fn leave_focus(&self) {
        let Some(window) = self.root.window() else { return };
        let previous = self.previous_focus.borrow_mut().take().and_then(|w| w.load());
        window.makeFirstResponder(previous.as_deref());
    }

    pub fn composer_focus(&self, focused: bool) {
        self.composer.show_focus(focused);
        if focused {
            self.offer_selection();
        }
    }
}

/// Whether a subagent or workflow of `thread` still runs; background shells and monitors are no part of the session's
/// work, as a server may run in one for hours.
pub(crate) fn delegates(thread: &Thread) -> bool {
    thread.subagents().iter().any(|s| !s.status.is_final())
        || thread.background().iter().any(|b| {
            !b.status.is_final()
                && !b.ambient
                && matches!(
                    b.kind,
                    ide_thread::BackgroundKind::Subagent | ide_thread::BackgroundKind::Workflow
                )
        })
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

/// The composer of the AI tab with this key, as a field that dictation goes into.
#[cfg(feature = "dictation")]
struct ComposerField(AgentKey);

#[cfg(feature = "dictation")]
impl ide_ui_dictation::DictationField for ComposerField {
    fn notice(&self, text: &str) -> bool {
        let controller = super::controller_of(self.0);
        controller.inspect(|controller| controller.show_notice(text)).is_some()
    }
}

/// What a session is called before anything names it.
fn untitled(agent: &Agent) -> String {
    match agent {
        Agent::Acp(agent) => agent.name.clone(),
        Agent::Claude | Agent::Own => t("Neue Sitzung").to_string(),
    }
}
