//! The activity view: a toolbar item with the number of things that run, and a popover listing them — agent
//! sessions, their subagents and background work, and the processes the supervisor keeps — each with Stop and a way
//! to its tab.

use std::cell::{Cell, RefCell};
use std::sync::Once;
use std::time::{Duration, SystemTime};

use ide_process::{Kind, ProcessId, Supervisor};
use ide_thread::{BackgroundKind, Status, TaskStatus};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSLayoutAttribute, NSPopover, NSPopoverBehavior, NSScrollView, NSStackView,
    NSToolbarItem, NSView, NSViewController,
};
use objc2_foundation::{NSObjectProtocol, NSSize};
use splat_i18n::{t, tf};

use crate::agent::AgentTab;
use crate::util::{guarded, main_after, main_soon, ns, set_action, symbol};
use crate::window::WindowController;
use crate::window::toolbar::kind_name;

const ID: &str = "ide.activity";
const WIDTH: f64 = 420.0;

/// What a button of the popover does.
#[derive(Clone, PartialEq)]
enum Action {
    ShowCli(u64),
    StopCli(u64),
    ShowTab(u64),
    StopTurn(u64),
    Reveal(u64, String),
    StopTask(u64, String),
    ShowOutput(u64, String),
    StopProcess(ProcessId),
    /// Selects the terminal tab with this number.
    ShowTerminal(isize),
}

pub struct ActivityIvars {
    controller: Weak<WindowController>,
    /// The item in the toolbar of each of the window's tabs.
    items: RefCell<Vec<Weak<NSToolbarItem>>>,
    popover: RefCell<Option<Retained<NSPopover>>>,
    actions: RefCell<Vec<Action>>,
    scheduled: Cell<bool>,
    /// The tab whose subagents and background tasks alone the popover shows, as the session's own overview.
    only: Cell<Option<u64>>,
    /// The rows the popover shows, to change their texts in place.
    shown: RefCell<Option<Shown>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Activity` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEActivity"]
    #[ivars = ActivityIvars]
    pub struct Activity;

    unsafe impl NSObjectProtocol for Activity {}

    impl Activity {
        #[unsafe(method(ideShowActivity:))]
        fn show_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.toggle());
        }

        #[unsafe(method(ideActivityAction:))]
        fn action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender.and_then(|s| s.downcast_ref::<NSButton>()).map_or(-1, |b| b.tag());
                let action = usize::try_from(tag).ok().and_then(|i| self.ivars().actions.borrow().get(i).cloned());
                if let Some(action) = action {
                    self.perform(action);
                }
            });
        }
    }
);

thread_local! {
    static ACTIVITIES: RefCell<Vec<Retained<Activity>>> = const { RefCell::new(Vec::new()) };
}

fn of(controller: &WindowController) -> Option<Retained<Activity>> {
    ACTIVITIES.with(|all| {
        all.borrow()
            .iter()
            .find(|a| {
                a.ivars()
                    .controller
                    .load()
                    .is_some_and(|c| std::ptr::eq(&*c, controller))
            })
            .cloned()
    })
}

/// The toolbar item of `controller`'s window.
pub fn toolbar_item(controller: &WindowController) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    watch_supervisor();
    let activity = of(controller).unwrap_or_else(|| {
        let ivars = ActivityIvars {
            controller: Weak::from(controller),
            items: RefCell::new(Vec::new()),
            popover: RefCell::new(None),
            actions: RefCell::new(Vec::new()),
            scheduled: Cell::new(false),
            only: Cell::new(None),
            shown: RefCell::new(None),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let activity: Retained<Activity> = unsafe { msg_send![super(Activity::alloc(mtm).set_ivars(ivars)), init] };
        ACTIVITIES.with(|all| all.borrow_mut().push(activity.clone()));
        activity
    });
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(ID));
    item.setImage(symbol("list.bullet.rectangle", t("Aktivität")).as_deref());
    item.setLabel(&ns(t("Aktivität")));
    item.setToolTip(Some(&ns(t("Was gerade läuft"))));
    // SAFETY: The activity object answers ideShowActivity: with a method that takes the sender; the toolbar item
    // holds its target weakly and the window's list keeps the object.
    unsafe {
        item.setTarget(Some(&activity));
        item.setAction(Some(sel!(ideShowActivity:)));
    }
    activity.ivars().items.borrow_mut().push(Weak::from(&*item));
    activity.update_count();
    item
}

/// Hears from the supervisor when processes start or end, once for the app.
fn watch_supervisor() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Supervisor::global().subscribe(|_| {
            main_soon(|| {
                let all: Vec<Retained<Activity>> = ACTIVITIES.with(|a| a.borrow().clone());
                for activity in all {
                    activity.changed();
                }
            });
        });
    });
}

/// Tells the activity view of `controller`'s window that something changed.
pub fn refresh(controller: &WindowController) {
    if let Some(activity) = of(controller) {
        activity.changed();
    }
}

/// How a session stands, as the activity view and the AI panel say it.
pub fn session_status(status: Status) -> &'static str {
    match status {
        Status::Idle => t("Bereit"),
        Status::Running => t("Arbeitet"),
        Status::RequiresAction => t("Wartet auf dich"),
    }
}

/// How a task stands.
pub fn task_status(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => t("Wartet"),
        TaskStatus::Running => t("Läuft"),
        TaskStatus::Paused => t("Pausiert"),
        TaskStatus::Completed => t("Fertig"),
        TaskStatus::Failed => t("Fehlgeschlagen"),
        TaskStatus::Stopped => t("Gestoppt"),
    }
}

pub fn background_symbol(kind: &BackgroundKind) -> &'static str {
    match kind {
        BackgroundKind::Shell => "terminal",
        BackgroundKind::Monitor => "eye",
        BackgroundKind::Subagent => "person.2",
        BackgroundKind::Workflow => "flowchart",
        BackgroundKind::Other(_) => "gearshape.2",
    }
}

/// One line of the popover.
struct Line {
    symbol: &'static str,
    tint: Retained<NSColor>,
    title: String,
    detail: String,
    indent: bool,
    buttons: Vec<(&'static str, Action)>,
}

/// What decides the popover's rows: its sections and, per line, its symbol, indent and buttons. While it stays, the
/// rows keep their views and only their texts change, so the keyboard focus and VoiceOver's place stay where they are.
#[derive(PartialEq)]
enum Shape {
    Section(&'static str),
    Line {
        symbol: &'static str,
        indent: bool,
        buttons: Vec<(&'static str, Action)>,
    },
}

fn shape_of(sections: &[(&'static str, Vec<Line>)]) -> Vec<Shape> {
    let mut shape = Vec::new();
    for (title, lines) in sections.iter().filter(|(_, lines)| !lines.is_empty()) {
        shape.push(Shape::Section(title));
        shape.extend(lines.iter().map(|line| Shape::Line {
            symbol: line.symbol,
            indent: line.indent,
            buttons: line.buttons.clone(),
        }));
    }
    shape
}

/// The views of a line whose texts change in place.
struct LineViews {
    icon: Retained<objc2_app_kit::NSImageView>,
    title: Retained<objc2_app_kit::NSTextField>,
    detail: Retained<objc2_app_kit::NSTextField>,
    buttons: Vec<(Retained<NSButton>, &'static str)>,
}

impl LineViews {
    fn update(&self, line: &Line) {
        if self.title.stringValue().to_string() != line.title {
            self.title.setStringValue(&ns(&line.title));
            self.icon.setImage(symbol(line.symbol, &line.title).as_deref());
            for (button, title) in &self.buttons {
                crate::util::set_label(button, Some(&format!("{title}: {}", line.title)));
            }
        }
        if self.detail.stringValue().to_string() != line.detail {
            self.detail.setStringValue(&ns(&line.detail));
        }
        self.icon.setContentTintColor(Some(&line.tint));
    }
}

/// The popover's rows and what they were built from.
struct Shown {
    shape: Vec<Shape>,
    lines: Vec<LineViews>,
}

impl Shown {
    /// What tells the rows apart, in their order: the action of each one's first button, as a row keeps it while it
    /// shows.
    fn identities(&self) -> Vec<Option<Action>> {
        self.shape
            .iter()
            .filter_map(|shape| match shape {
                Shape::Line { buttons, .. } => Some(buttons.first().map(|(_, action)| action.clone())),
                Shape::Section(_) => None,
            })
            .collect()
    }

    /// The row and the button of it with the keyboard focus in `window`.
    fn focused(&self, window: &objc2_app_kit::NSWindow) -> Option<(usize, usize)> {
        let focus = window.firstResponder()?;
        let focus: &AnyObject = focus.as_ref();
        self.lines.iter().enumerate().find_map(|(row, line)| {
            line.buttons
                .iter()
                .position(|(button, _)| {
                    let button: &AnyObject = button.as_ref();
                    std::ptr::eq(button, focus)
                })
                .map(|at| (row, at))
        })
    }
}

/// The place in `new` of row `row` of `old`, or of the nearest row after it that is still there, else of the nearest
/// before it.
fn same_or_next<T: PartialEq>(old: &[T], row: usize, new: &[T]) -> Option<usize> {
    let place = |item: &T| new.iter().position(|other| other == item);
    old.get(row..)
        .into_iter()
        .flatten()
        .find_map(place)
        .or_else(|| old.get(..row).into_iter().flatten().rev().find_map(place))
}

/// How many things run, for the tooltip: busy sessions, running subagents and background work, and processes.
fn count(tabs: &[std::rc::Rc<AgentTab>], processes: &[ide_process::ProcessInfo]) -> usize {
    let agents: usize = tabs
        .iter()
        .map(|tab| {
            let thread = tab.view_snapshot();
            usize::from(thread.status() != Status::Idle)
                + thread
                    .subagents()
                    .iter()
                    .filter(|s| s.status == TaskStatus::Running)
                    .count()
                + thread
                    .background()
                    .iter()
                    .filter(|b| !b.status.is_final() && !b.ambient && b.kind != BackgroundKind::Subagent)
                    .count()
        })
        .sum();
    agents + processes.len()
}

/// The supervisor's processes the popover lists: not the agents and CLI threads, which have their own lines, and not
/// git.
fn processes() -> Vec<ide_process::ProcessInfo> {
    let clis = crate::terminal::cli_processes();
    Supervisor::global()
        .running()
        .into_iter()
        .filter(|p| !matches!(p.kind, Kind::Agent | Kind::Git) && !clis.contains(&p.id))
        .collect()
}

/// The CLI threads of the window (I5), each with its status, what it shows and its running time.
fn cli_lines(threads: &[crate::terminal::CliThread], now: SystemTime) -> Vec<Line> {
    threads
        .iter()
        .map(|thread| {
            let seconds = now.duration_since(thread.started).map_or(0, |d| d.as_secs());
            let mut detail = format!(
                "{} · {}",
                thread.status.text(),
                crate::window::toolbar::duration(seconds)
            );
            if let Some(shown) = &thread.detail {
                detail.push_str(&format!(" · {shown}"));
            }
            let mut buttons = vec![(t("Zeigen"), Action::ShowCli(thread.key))];
            if thread.status != crate::terminal::CliStatus::Finished {
                buttons.push((t("Stopp"), Action::StopCli(thread.key)));
            }
            Line {
                symbol: "apple.terminal",
                tint: thread.status.color(),
                title: thread.name.clone(),
                detail,
                indent: false,
                buttons,
            }
        })
        .collect()
}

/// The lines of `tabs`: each session, its running subagents, or all of them with `all`, and its background work.
fn agent_lines(tabs: &[std::rc::Rc<AgentTab>], now: SystemTime, all: bool) -> Vec<Line> {
    let mut lines = Vec::new();
    for tab in tabs {
        let thread = tab.view_snapshot();
        let mut buttons = vec![(t("Zeigen"), Action::ShowTab(tab.key))];
        if thread.status() != Status::Idle {
            buttons.push((t("Stopp"), Action::StopTurn(tab.key)));
        }
        let tint = match thread.status() {
            Status::Running => NSColor::controlAccentColor(),
            Status::RequiresAction => NSColor::systemOrangeColor(),
            Status::Idle => NSColor::tertiaryLabelColor(),
        };
        lines.push(Line {
            symbol: "sparkle",
            tint,
            title: tab.title(),
            detail: session_status(thread.status()).to_string(),
            indent: false,
            buttons,
        });
        for sub in thread
            .subagents()
            .iter()
            .filter(|s| all || s.status == TaskStatus::Running)
        {
            let elapsed = now.duration_since(sub.started).unwrap_or_default().max(sub.elapsed);
            let mut detail = crate::agent::elapsed(elapsed);
            if sub.tokens > 0 {
                detail.push_str(&format!(" · {}", tf("{} Tokens", &[&crate::agent::tokens(sub.tokens)])));
            }
            if let Some(tool) = &sub.last_tool {
                detail.push_str(&format!(" · {}", tf("zuletzt {}", &[tool])));
            }
            if sub.status != TaskStatus::Running {
                detail = format!("{} · {detail}", task_status(sub.status));
            }
            let mut buttons = vec![(t("Zeigen"), Action::Reveal(tab.key, sub.key.clone()))];
            if let Some(task) = sub.task_id.clone().filter(|_| sub.cancellable) {
                buttons.push((t("Stopp"), Action::StopTask(tab.key, task)));
            }
            lines.push(Line {
                symbol: "person.2",
                tint: NSColor::secondaryLabelColor(),
                title: sub.description.clone(),
                detail,
                indent: true,
                buttons,
            });
        }
        for item in thread
            .background()
            .iter()
            .filter(|b| !b.status.is_final() && b.kind != BackgroundKind::Subagent)
        {
            let mut detail = task_status(item.status).to_string();
            if let Some(started) = item.started {
                detail.push_str(&format!(
                    " · {}",
                    crate::agent::elapsed(now.duration_since(started).unwrap_or_default())
                ));
            }
            if let Some(last) = item
                .output
                .text
                .as_deref()
                .and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()))
            {
                detail.push_str(&format!(" · {}", last.trim()));
            }
            lines.push(Line {
                symbol: background_symbol(&item.kind),
                tint: if item.ambient {
                    NSColor::tertiaryLabelColor()
                } else {
                    NSColor::secondaryLabelColor()
                },
                title: item.label.clone(),
                detail,
                indent: true,
                buttons: vec![
                    (t("Ausgabe zeigen"), Action::ShowOutput(tab.key, item.id.clone())),
                    (t("Stopp"), Action::StopTask(tab.key, item.id.clone())),
                ],
            });
        }
    }
    lines
}

/// The lines of the supervisor's processes; those of a terminal, or started in one, can show its tab.
fn process_lines(controller: &WindowController, processes: &[ide_process::ProcessInfo], now: SystemTime) -> Vec<Line> {
    processes
        .iter()
        .map(|p| {
            let seconds = now.duration_since(p.started).map_or(0, |d| d.as_secs());
            let mut buttons = Vec::new();
            if matches!(p.kind, Kind::Terminal | Kind::DevServer)
                && let Some(tab) = crate::terminal::tab_of_process(controller, p.pid)
            {
                buttons.push((t("Zeigen"), Action::ShowTerminal(tab)));
            }
            buttons.push((t("Stopp"), Action::StopProcess(p.id)));
            Line {
                symbol: match p.kind {
                    Kind::Terminal => "terminal",
                    Kind::LanguageServer => "curlybraces",
                    Kind::DevServer => "server.rack",
                    _ => "gearshape",
                },
                tint: NSColor::secondaryLabelColor(),
                title: p.label.clone(),
                detail: tf(
                    "{} · {} · PID {}",
                    &[
                        kind_name(p.kind),
                        &crate::window::toolbar::duration(seconds),
                        &p.pid.to_string(),
                    ],
                ),
                indent: false,
                buttons,
            }
        })
        .collect()
}

impl Activity {
    fn controller(&self) -> Option<Retained<WindowController>> {
        self.ivars().controller.load()
    }

    /// Updates the count in the tooltip now and the popover soon, if it shows.
    fn changed(&self) {
        self.update_count();
        let shown = self.ivars().popover.borrow().as_ref().is_some_and(|p| p.isShown());
        if shown && !self.ivars().scheduled.replace(true) {
            let address = self as *const Activity as usize;
            main_after(Duration::from_millis(300), move || {
                let all: Vec<Retained<Activity>> = ACTIVITIES.with(|a| a.borrow().clone());
                if let Some(activity) = all.into_iter().find(|a| &**a as *const Activity as usize == address) {
                    activity.ivars().scheduled.set(false);
                    activity.fill();
                }
            });
        }
    }

    /// The toolbar items of the window's tabs that are still there.
    fn items(&self) -> Vec<Retained<NSToolbarItem>> {
        let mut items = self.ivars().items.borrow_mut();
        items.retain(|item| item.load().is_some());
        items.iter().filter_map(Weak::load).collect()
    }

    fn update_count(&self) {
        let Some(controller) = self.controller() else { return };
        let tabs = crate::agent::of_window(&controller);
        let clis = crate::terminal::cli_threads(&controller)
            .iter()
            .filter(|thread| thread.status != crate::terminal::CliStatus::Finished)
            .count();
        let count = count(&tabs, &processes()) + clis;
        for item in self.items() {
            item.setToolTip(Some(&ns(&tf("Was gerade läuft: {}", &[&count.to_string()]))));
        }
    }

    fn toggle(&self) {
        let shown = self.ivars().popover.borrow().as_ref().is_some_and(|p| p.isShown());
        if shown {
            if let Some(popover) = self.ivars().popover.borrow().as_ref() {
                // SAFETY: Closing takes any sender; none is given.
                unsafe { popover.performClose(None) };
            }
            return;
        }
        self.show();
    }

    /// Opens the popover below the toolbar item.
    /// Opens the popover with everything that runs, below the toolbar item.
    pub fn show(&self) {
        self.ivars().only.set(None);
        self.open(None);
    }

    /// Opens the popover below `anchor` or the toolbar item.
    fn open(&self, anchor: Option<&NSView>) {
        let Some(controller) = self.controller() else { return };
        let mtm = controller.mtm();
        let popover = self
            .ivars()
            .popover
            .borrow_mut()
            .get_or_insert_with(|| {
                let popover = NSPopover::new(mtm);
                popover.setBehavior(NSPopoverBehavior::Transient);
                popover.setContentViewController(Some(&NSViewController::new(mtm)));
                popover
            })
            .clone();
        self.fill();
        let toolbar = controller.window().and_then(|window| window.toolbar());
        let item = self.items().into_iter().find(|item| {
            item.toolbar()
                .zip(toolbar.as_ref())
                .is_some_and(|(own, front)| std::ptr::eq(&*own, &**front))
        });
        match (anchor, item) {
            (Some(anchor), _) => {
                popover.showRelativeToRect_ofView_preferredEdge(
                    anchor.bounds(),
                    anchor,
                    objc2_foundation::NSRectEdge::MaxY,
                );
            }
            (None, Some(item)) if item.toolbar().is_some() => popover.showRelativeToToolbarItem(&item),
            _ => {
                if let Some(view) = controller.window().and_then(|w| w.contentView()) {
                    let bounds = view.bounds();
                    popover.showRelativeToRect_ofView_preferredEdge(bounds, &view, objc2_foundation::NSRectEdge::MaxY);
                }
            }
        }
        relink(&popover);
        self.tick();
    }

    /// Keeps the running times of an open popover current.
    fn tick(&self) {
        let address = self as *const Activity as usize;
        main_after(Duration::from_secs(1), move || {
            let all: Vec<Retained<Activity>> = ACTIVITIES.with(|a| a.borrow().clone());
            if let Some(activity) = all.into_iter().find(|a| &**a as *const Activity as usize == address)
                && activity.ivars().popover.borrow().as_ref().is_some_and(|p| p.isShown())
            {
                activity.fill();
                activity.tick();
            }
        });
    }

    /// What runs now, by section.
    fn sections(&self, controller: &WindowController) -> Vec<(&'static str, Vec<Line>)> {
        let now = SystemTime::now();
        let only = self.ivars().only.get();
        let mut tabs = crate::agent::of_window(controller);
        tabs.retain(|tab| only.is_none_or(|key| tab.key == key));
        let processes = if only.is_some() { Vec::new() } else { processes() };
        let mut agents = agent_lines(&tabs, now, only.is_some());
        if only.is_none() {
            agents.extend(cli_lines(&crate::terminal::cli_threads(controller), now));
        }
        let heading = if only.is_some() {
            t("Aufgaben dieser Sitzung")
        } else {
            t("Agenten")
        };
        vec![
            (heading, agents),
            (t("Prozesse"), process_lines(controller, &processes, now)),
        ]
    }

    /// Shows what runs now: in the rows there are, while the same rows would show, else in rows built anew.
    fn fill(&self) {
        let Some(controller) = self.controller() else { return };
        let sections = self.sections(&controller);
        let shape = shape_of(&sections);
        if let Some(shown) = self.ivars().shown.borrow().as_ref()
            && shown.shape == shape
        {
            let lines = sections.iter().flat_map(|(_, lines)| lines);
            for (views, line) in shown.lines.iter().zip(lines) {
                views.update(line);
            }
            #[cfg(feature = "selftest")]
            UPDATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        self.build(controller.mtm(), &sections, shape);
    }

    /// Builds the popover's rows anew.
    fn build(&self, mtm: MainThreadMarker, sections: &[(&'static str, Vec<Line>)], shape: Vec<Shape>) {
        let mut actions = Vec::new();
        let mut shown = Vec::new();
        let stack = NSStackView::new(mtm);
        stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::Leading);
        stack.setSpacing(4.0);
        stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 12.0,
            left: 12.0,
            bottom: 12.0,
            right: 12.0,
        });
        for (title, lines) in sections.iter().filter(|(_, lines)| !lines.is_empty()) {
            let heading = crate::agent_panel::section_label(mtm, title);
            stack.addArrangedSubview(&heading);
            for line in lines {
                let (view, views) = self.line_view(mtm, line, &mut actions);
                shown.push(views);
                stack.addArrangedSubview(&view);
                view.setTranslatesAutoresizingMaskIntoConstraints(false);
                view.widthAnchor()
                    .constraintEqualToConstant(WIDTH - 24.0)
                    .setActive(true);
            }
            if let Some(last) = stack.arrangedSubviews().lastObject() {
                stack.setCustomSpacing_afterView(12.0, &last);
            }
        }
        if shape.is_empty() {
            let label = objc2_app_kit::NSTextField::labelWithString(&ns(t("Gerade läuft nichts.")), mtm);
            label.setTextColor(Some(&NSColor::secondaryLabelColor()));
            stack.addArrangedSubview(&label);
        }
        *self.ivars().actions.borrow_mut() = actions;
        let focused = self.focused_row();
        *self.ivars().shown.borrow_mut() = Some(Shown { shape, lines: shown });
        self.show_rows(mtm, &stack);
        if let Some(focused) = focused {
            self.refocus(focused);
        }
    }

    fn window(&self) -> Option<Retained<objc2_app_kit::NSWindow>> {
        let popover = self.ivars().popover.borrow();
        popover.as_ref()?.contentViewController()?.view().window()
    }

    /// The rows as they show, with the row and the button of it with the keyboard focus.
    fn focused_row(&self) -> Option<(Vec<Option<Action>>, usize, usize)> {
        let window = self.window()?;
        let shown = self.ivars().shown.borrow();
        let shown = shown.as_ref()?;
        let (row, button) = shown.focused(&window)?;
        Some((shown.identities(), row, button))
    }

    /// Gives the keyboard focus back to the row it was on before the rows were built anew, by what the row is, else
    /// to the nearest row after it: the popover never sends the focus back to its start.
    fn refocus(&self, (before, row, button): (Vec<Option<Action>>, usize, usize)) {
        let Some(window) = self.window() else { return };
        let shown = self.ivars().shown.borrow();
        let Some(shown) = shown.as_ref() else { return };
        let Some(line) = same_or_next(&before, row, &shown.identities()).and_then(|at| shown.lines.get(at)) else {
            return;
        };
        if let Some((button, _)) = line.buttons.get(button).or_else(|| line.buttons.last()) {
            window.makeFirstResponder(Some(button));
        }
    }

    /// Puts `stack` into the popover, scrolling beyond its greatest height.
    fn show_rows(&self, mtm: MainThreadMarker, stack: &NSStackView) {
        let height = stack.fittingSize().height.min(520.0);
        let scroll = NSScrollView::new(mtm);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let document = crate::agent_panel::flipped(mtm);
        document.addSubview(stack);
        crate::util::pin_edges(stack, &document);
        scroll.setDocumentView(Some(&document));
        document.setTranslatesAutoresizingMaskIntoConstraints(false);
        let clip = scroll.contentView();
        document
            .widthAnchor()
            .constraintEqualToAnchor(&clip.widthAnchor())
            .setActive(true);
        let size = NSSize::new(WIDTH, height.max(60.0));
        scroll.setFrameSize(size);
        if let Some(popover) = self.ivars().popover.borrow().as_ref()
            && let Some(controller) = popover.contentViewController()
        {
            controller.setView(&scroll);
            // A popover takes its size from its view controller, not from its own content size.
            controller.setPreferredContentSize(size);
            popover.setContentSize(size);
            relink(popover);
        }
        scroll.setAccessibilityLabel(Some(&ns(t("Aktivität"))));
    }

    fn line_view(
        &self,
        mtm: MainThreadMarker,
        line: &Line,
        actions: &mut Vec<Action>,
    ) -> (Retained<NSView>, LineViews) {
        let icon = objc2_app_kit::NSImageView::new(mtm);
        icon.setImage(symbol(line.symbol, &line.title).as_deref());
        icon.setContentTintColor(Some(&line.tint));
        icon.setContentHuggingPriority_forOrientation(750.0, objc2_app_kit::NSLayoutConstraintOrientation::Horizontal);
        let title = objc2_app_kit::NSTextField::labelWithString(&ns(&line.title), mtm);
        title.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
        title.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(12.0)));
        let detail = objc2_app_kit::NSTextField::labelWithString(&ns(&line.detail), mtm);
        detail.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
        detail.setFont(Some(&crate::agent::style::small()));
        detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
        for label in [&title, &detail] {
            label.setContentCompressionResistancePriority_forOrientation(
                250.0,
                objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
            );
        }
        let texts = NSStackView::new(mtm);
        texts.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        texts.setAlignment(NSLayoutAttribute::Leading);
        texts.setSpacing(1.0);
        texts.addArrangedSubview(&title);
        texts.addArrangedSubview(&detail);
        let row = NSStackView::new(mtm);
        row.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Horizontal);
        row.setAlignment(NSLayoutAttribute::CenterY);
        row.setSpacing(6.0);
        row.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 2.0,
            left: if line.indent { 18.0 } else { 0.0 },
            bottom: 2.0,
            right: 0.0,
        });
        row.addArrangedSubview(&icon);
        row.addArrangedSubview(&texts);
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Horizontal);
        row.addArrangedSubview(&spacer);
        let mut buttons = Vec::new();
        for (title, action) in &line.buttons {
            // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
            let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
            set_action(&button, self, sel!(ideActivityAction:));
            button.setTag(isize::try_from(actions.len()).unwrap_or(-1));
            button.setControlSize(objc2_app_kit::NSControlSize::Small);
            button.setBezelStyle(objc2_app_kit::NSBezelStyle::Push);
            button.setFont(Some(&crate::agent::style::small()));
            crate::util::set_label(&button, Some(&format!("{title}: {}", line.title)));
            actions.push(action.clone());
            row.addArrangedSubview(&button);
            buttons.push((button, *title));
        }
        row.setAccessibilityElement(false);
        let views = LineViews {
            icon,
            title,
            detail,
            buttons,
        };
        (Retained::into_super(row), views)
    }

    fn perform(&self, action: Action) {
        let Some(controller) = self.controller() else { return };
        let tab = |key: u64| crate::agent::of_window(&controller).into_iter().find(|t| t.key == key);
        match action {
            Action::ShowCli(key) => crate::terminal::show_cli_thread(&controller, key),
            Action::StopCli(key) => crate::terminal::stop_cli_thread(&controller, key),
            Action::ShowTab(key) => {
                if let Some(tab) = tab(key) {
                    tab.select();
                }
            }
            Action::StopTurn(key) => {
                if let Some(tab) = tab(key) {
                    tab.stop_turn();
                }
            }
            Action::Reveal(key, sub) => {
                if let Some(tab) = tab(key) {
                    tab.select();
                    tab.timeline().reveal(&sub);
                }
            }
            Action::StopTask(key, task) => {
                if let Some(tab) = tab(key) {
                    tab.stop_task(&task);
                }
            }
            Action::ShowOutput(key, task) => {
                if let Some(tab) = tab(key) {
                    tab.show_background_output(&task);
                }
            }
            Action::StopProcess(id) => Supervisor::global().stop(id, Duration::from_secs(2)),
            Action::ShowTerminal(tab) => controller.select_tab_by_id(tab),
        }
        self.changed();
    }
}

/// Opens the overview of the subagents and background tasks of the AI tab `key` below `anchor`.
pub fn show_tab(controller: &WindowController, key: u64, anchor: &NSView) {
    if let Some(activity) = of(controller) {
        activity.ivars().only.set(Some(key));
        activity.open(Some(anchor));
    }
}

/// What the self-test reads from the popover and the toolbar item: the popover's lines and the item's tooltip.
#[cfg(feature = "selftest")]
pub fn inspect(controller: &WindowController) -> (Vec<String>, String) {
    let Some(activity) = of(controller) else {
        return (Vec::new(), String::new());
    };
    activity.show();
    let tabs = crate::agent::of_window(controller);
    let lines: Vec<String> = agent_lines(&tabs, SystemTime::now(), false)
        .into_iter()
        .chain(cli_lines(&crate::terminal::cli_threads(controller), SystemTime::now()))
        .chain(process_lines(controller, &processes(), SystemTime::now()))
        .map(|l| {
            let buttons: Vec<&str> = l.buttons.iter().map(|(title, _)| *title).collect();
            format!("{} — {} [{}]", l.title, l.detail, buttons.join(", "))
        })
        .collect();
    let tooltip = activity
        .items()
        .first()
        .and_then(|i| i.toolTip())
        .map(|t| t.to_string())
        .unwrap_or_default();
    (lines, tooltip)
}

/// Closes the popover, for the self-test's later pictures.
#[cfg(feature = "selftest")]
pub fn close(controller: &WindowController) {
    if let Some(activity) = of(controller)
        && let Some(popover) = activity.ivars().popover.borrow().as_ref()
    {
        // SAFETY: Closing takes any sender; none is given.
        unsafe { popover.performClose(None) };
    }
}

/// The lines of the open popover, as the session overview shows them.
#[cfg(feature = "selftest")]
pub fn overview_lines(controller: &WindowController) -> Vec<String> {
    let Some(activity) = of(controller) else {
        return Vec::new();
    };
    let only = activity.ivars().only.get();
    let mut tabs = crate::agent::of_window(controller);
    tabs.retain(|tab| only.is_none_or(|key| tab.key == key));
    agent_lines(&tabs, SystemTime::now(), only.is_some())
        .into_iter()
        .map(|l| format!("{} — {}", l.title, l.detail))
        .collect()
}

/// How often a popover changed the texts of its rows in place.
#[cfg(feature = "selftest")]
static UPDATED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How often a popover changed the texts of its rows in place, for the self-test.
#[cfg(feature = "selftest")]
pub fn updated_in_place() -> usize {
    UPDATED.load(std::sync::atomic::Ordering::Relaxed)
}

/// The popover's content, for the self-test's picture.
#[cfg(feature = "selftest")]
pub fn popover_view(controller: &WindowController) -> Option<Retained<NSView>> {
    let activity = of(controller)?;
    let popover = activity.ivars().popover.borrow().clone()?;
    Some(popover.contentViewController()?.view())
}

/// Links the buttons of a shown popover into its key view loop: AppKit makes the loop once, when the popover opens,
/// and the rows are built anew each second.
fn relink(popover: &NSPopover) {
    if let Some(window) = popover.contentViewController().and_then(|c| c.view().window()) {
        window.recalculateKeyViewLoop();
    }
}

#[cfg(test)]
mod tests {
    use super::same_or_next;

    #[test]
    fn the_focus_keeps_its_row_else_takes_the_next_else_the_one_before() {
        let old = ["session", "subagent", "shell", "server"];
        assert_eq!(same_or_next(&old, 2, &["session", "shell", "server"]), Some(1));
        assert_eq!(same_or_next(&old, 1, &["session", "shell", "server"]), Some(1));
        assert_eq!(same_or_next(&old, 1, &["server", "session"]), Some(0));
        assert_eq!(same_or_next(&old, 3, &["session", "subagent"]), Some(1));
        assert_eq!(same_or_next(&old, 0, &["new"]), None);
    }
}
