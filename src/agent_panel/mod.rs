//! The AI panel of the sidebar: the account and its usage, the sessions of the folder with groups and an archive,
//! and what runs now.

mod agents;
mod rows;
mod sessions;
mod usage;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use ide_thread::{ThreadId, ThreadIndex};
use objc2::rc::{Retained, Weak};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSLayoutAttribute, NSOutlineView, NSScrollView, NSSearchField, NSStackView,
    NSTextField, NSView,
};
use splat_i18n::{t, tf};

use crate::agent::Resume;
use crate::util::{main_after, ns};
use crate::window::{Panel, WindowController};
pub use rows::{flipped, section_label, small_label, spacer};
use sessions::{Kind, Source};

/// How far the panel's content keeps from the sidebar's edges: as far as the panel's title.
const SIDE: f64 = 8.0;

/// The panel of one window.
pub struct AgentPanel {
    controller: Weak<WindowController>,
    root: std::path::PathBuf,
    view: Retained<NSStackView>,
    usage: usage::Usage,
    active: Retained<NSTextField>,
    archive: Retained<NSButton>,
    search: Retained<NSSearchField>,
    sessions: Retained<NSOutlineView>,
    sessions_source: Retained<Source>,
    running: Retained<NSOutlineView>,
    running_source: Retained<Source>,
    show_archived: Cell<bool>,
    /// Groups the user made that hold no session yet; the index only knows groups in use.
    groups: RefCell<BTreeSet<String>>,
    scheduled: Cell<bool>,
    agents: agents::Agents,
}

thread_local! {
    static PANELS: RefCell<Vec<Rc<AgentPanel>>> = const { RefCell::new(Vec::new()) };
}

fn panel_of(controller: &WindowController) -> Option<Rc<AgentPanel>> {
    PANELS.with(|panels| {
        panels
            .borrow()
            .iter()
            .find(|p| p.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller)))
            .cloned()
    })
}

fn panel_by_key(key: usize) -> Option<Rc<AgentPanel>> {
    PANELS.with(|panels| panels.borrow().iter().find(|p| Rc::as_ptr(p) as usize == key).cloned())
}

/// Puts the AI panel of the window's folder in place of the Agents page.
pub fn attach(controller: &WindowController) {
    PANELS.with(|panels| {
        panels
            .borrow_mut()
            .retain(|p| p.controller.load().is_some_and(|c| !std::ptr::eq(&*c, controller)))
    });
    let Some(root) = controller.project_root() else { return };
    let panel = AgentPanel::new(controller, root);
    controller.set_panel_page(Panel::Agents, &panel.view);
    PANELS.with(|panels| panels.borrow_mut().push(panel.clone()));
    panel.reload();
}

/// Shows what changed in the window's sessions, at most a few times a second.
pub fn refresh(controller: &WindowController) {
    let Some(panel) = panel_of(controller) else { return };
    if panel.scheduled.replace(true) {
        return;
    }
    let key = Rc::as_ptr(&panel) as usize;
    main_after(Duration::from_millis(400), move || {
        if let Some(panel) = panel_by_key(key) {
            panel.scheduled.set(false);
            panel.reload();
        }
    });
}

/// Shows what changed in every window's panel.
fn refresh_all() {
    let controllers: Vec<Retained<WindowController>> =
        PANELS.with(|panels| panels.borrow().iter().filter_map(|p| p.controller.load()).collect());
    for controller in controllers {
        refresh(&controller);
    }
}

fn bookmarks_file() -> std::path::PathBuf {
    ThreadIndex::default_dir().join("bookmarks.json")
}

fn bookmarks() -> BTreeSet<String> {
    std::fs::read(bookmarks_file())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Whether the user bookmarked the thread `id`.
pub fn is_bookmarked(id: &ThreadId) -> bool {
    bookmarks().contains(id.as_str())
}

/// Bookmarks the thread `id` or takes the bookmark away.
pub fn set_bookmarked(id: &ThreadId, on: bool) {
    let mut marks = bookmarks();
    if on {
        marks.insert(id.as_str().to_string());
    } else {
        marks.remove(id.as_str());
    }
    let file = bookmarks_file();
    let written = file
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&file, serde_json::to_vec(&marks).unwrap_or_default()));
    if let Err(error) = written {
        crate::diag::report("bookmarks", error);
    }
}

/// The sessions of `cwd` that can be resumed, newest first: the index's, then — experimental — those the CLI saved
/// from a terminal.
pub fn past_sessions(cwd: &Path, limit: usize) -> Vec<(Resume, SystemTime)> {
    let index = ThreadIndex::open(&ThreadIndex::default_dir());
    let mut past: Vec<(Resume, SystemTime)> = index
        .list(&ide_thread::Filter::default())
        .into_iter()
        .filter(|e| e.cwd.as_deref() == Some(cwd))
        .filter_map(|e| {
            let session = e.session.clone()?;
            Some((
                Resume {
                    transcript: transcript(cwd, &session),
                    session,
                    thread: Some(e.id.clone()),
                    title: Some(e.title.clone()).filter(|t| !t.is_empty()),
                },
                e.last_activity,
            ))
        })
        .collect();
    let known: BTreeSet<String> = past.iter().map(|(r, _)| r.session.clone()).collect();
    for session in terminal_sessions(cwd) {
        if known.contains(&session.id) {
            continue;
        }
        let title = session.title.clone().or_else(|| session.first_prompt.clone());
        past.push((
            Resume {
                session: session.id.clone(),
                thread: None,
                title,
                transcript: Some(session.path.clone()),
            },
            session.modified,
        ));
    }
    past.sort_by_key(|(_, when)| std::cmp::Reverse(*when));
    past.truncate(limit);
    past
}

/// The configuration folder of Claude Code, where it keeps its sessions.
pub(crate) fn claude_config_dir() -> std::path::PathBuf {
    config_dir()
}

fn config_dir() -> std::path::PathBuf {
    ide_claude::history::config_dir(crate::environment::get().unwrap_or_default())
}

/// The sessions the CLI saved for `cwd`, as its own session list shows them; experimental.
fn terminal_sessions(cwd: &Path) -> Vec<ide_claude::history::PastSession> {
    ide_claude::history::list(&config_dir(), cwd)
}

fn transcript(cwd: &Path, session: &str) -> Option<std::path::PathBuf> {
    let path = config_dir()
        .join("projects")
        .join(ide_claude::history::project_folder(cwd))
        .join(format!("{session}.jsonl"));
    path.is_file().then_some(path)
}

impl AgentPanel {
    fn new(controller: &WindowController, root: std::path::PathBuf) -> Rc<AgentPanel> {
        let mtm = controller.mtm();
        let usage = usage::Usage::new(mtm);
        let agents = agents::Agents::new(controller);
        let active = rows::small_label(mtm, "");
        let search = NSSearchField::new(mtm);
        search.setPlaceholderString(Some(&ns(t("Sitzungen durchsuchen"))));
        search.setControlSize(objc2_app_kit::NSControlSize::Small);
        search.setTranslatesAutoresizingMaskIntoConstraints(false);
        let archive = crate::util::borderless_button(mtm, "", None, sel!(ideAgentPanelArchive:));
        archive.setFont(Some(&crate::agent::style::small()));
        archive.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let (sessions, sessions_source) = sessions::outline(mtm, Kind::Sessions);
        let (running, running_source) = sessions::outline(mtm, Kind::Running);
        let view = NSStackView::new(mtm);
        view.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        view.setAlignment(NSLayoutAttribute::Leading);
        view.setSpacing(6.0);
        view.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 0.0,
            left: SIDE,
            bottom: 8.0,
            right: SIDE,
        });
        let panel = Rc::new(AgentPanel {
            controller: Weak::from(controller),
            root,
            view,
            usage,
            active,
            archive,
            search,
            sessions,
            sessions_source,
            running,
            running_source,
            show_archived: Cell::new(false),
            groups: RefCell::new(BTreeSet::new()),
            scheduled: Cell::new(false),
            agents,
        });
        let key = Rc::as_ptr(&panel) as usize;
        panel.sessions_source.set_panel(key);
        panel.running_source.set_panel(key);
        crate::util::set_action(&panel.search, &panel.sessions_source, sel!(ideAgentPanelSearch:));
        crate::util::set_action(&panel.archive, &panel.sessions_source, sel!(ideAgentPanelArchive:));
        panel.lay_out();
        panel
    }

    fn lay_out(&self) {
        let Some(controller) = self.controller.load() else {
            return;
        };
        let new_session = self.agents.new_session_button();
        let mtm = controller.mtm();
        let section = |title: &str, trailing: Option<&NSView>| {
            let label = rows::section_label(mtm, title);
            let spacer = rows::spacer(mtm);
            let mut views: Vec<&NSView> = vec![&label, &spacer];
            if let Some(trailing) = trailing {
                views.push(trailing);
            }
            row(mtm, &views)
        };
        let account = section(t("Konto & Nutzung"), None);
        let sessions_head = section(t("Sitzungen"), Some(&new_session));
        let line = row(mtm, &[&self.active, &rows::spacer(mtm), &self.archive]);
        let running_head = section(t("Laufend"), None);
        let sessions_scroll = scroll(mtm, &self.sessions);
        let running_scroll = scroll(mtm, &self.running);
        for view in [
            &*account as &NSView,
            &self.usage.view,
            &sessions_head,
            &line,
            &self.search,
            &sessions_scroll,
            &running_head,
            &running_scroll,
        ] {
            self.view.addArrangedSubview(view);
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
            view.widthAnchor()
                .constraintEqualToAnchor_constant(&self.view.widthAnchor(), -2.0 * SIDE)
                .setActive(true);
        }
        self.view.setCustomSpacing_afterView(14.0, &self.usage.view);
        self.view.setCustomSpacing_afterView(14.0, &sessions_scroll);
        self.view.setCustomSpacing_afterView(14.0, &running_scroll);
        sessions_scroll
            .heightAnchor()
            .constraintGreaterThanOrEqualToConstant(140.0)
            .setActive(true);
        let running_height = running_scroll.heightAnchor().constraintEqualToConstant(150.0);
        running_height.setActive(true);
        self.sessions.setAccessibilityLabel(Some(&ns(t("Sitzungen"))));
        self.running
            .setAccessibilityLabel(Some(&ns(t("Laufende Sitzungen und Aufgaben"))));
    }

    /// Reads the index and the open tabs again and shows them.
    fn reload(&self) {
        let Some(controller) = self.controller.load() else {
            return;
        };
        let tabs = crate::agent::of_window(&controller);
        let live = tabs.iter().filter(|t| t.is_live()).count();
        self.active.setStringValue(&ns(&tf("Aktiv · {}", &[&live.to_string()])));
        let index = ThreadIndex::open(&ThreadIndex::default_dir());
        let archived = index
            .list(&ide_thread::Filter {
                archived: true,
                ..ide_thread::Filter::default()
            })
            .into_iter()
            .filter(|e| e.cwd.as_deref() == Some(self.root.as_path()))
            .count();
        let archive_title = if self.show_archived.get() {
            t("Zurück zu den Sitzungen").to_string()
        } else {
            tf("Archiv ({})", &[&archived.to_string()])
        };
        self.archive.setTitle(&ns(&archive_title));
        let query = self.search.stringValue().to_string();
        let model = sessions::Model::build(&sessions::Inputs {
            index: &index,
            root: &self.root,
            tabs: &tabs,
            query: &query,
            archived: self.show_archived.get(),
            groups: &self.groups.borrow(),
            terminal: if self.show_archived.get() {
                Vec::new()
            } else {
                terminal_sessions(&self.root)
            },
            cli: if self.show_archived.get() {
                Vec::new()
            } else {
                crate::terminal::cli_threads(&controller)
            },
            bookmarks: &bookmarks(),
        });
        self.sessions_source.show(&self.sessions, model);
        self.running_source.show(&self.running, sessions::Model::running(&tabs));
        self.usage.show(&tabs);
    }

    fn controller(&self) -> Option<Retained<WindowController>> {
        self.controller.load()
    }
}

fn row(mtm: MainThreadMarker, views: &[&NSView]) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setAlignment(NSLayoutAttribute::CenterY);
    stack.setSpacing(6.0);
    for view in views {
        stack.addArrangedSubview(view);
    }
    stack
}

fn scroll(mtm: MainThreadMarker, outline: &NSOutlineView) -> Retained<NSScrollView> {
    let scroll = NSScrollView::new(mtm);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(outline));
    scroll
}

/// Shows `report` as the latest usage, for the self-test.
#[cfg(feature = "selftest")]
pub fn set_usage_report(report: ide_claude::control::UsageReport) {
    crate::agent::inject_usage(report.clone());
    usage::inject(report);
    refresh_all();
}

/// The quiet texts of the session rows of `controller`'s panel, for the self-test.
#[cfg(feature = "selftest")]
pub fn session_details(controller: &WindowController) -> Vec<String> {
    panel_of(controller).map_or_else(Vec::new, |panel| {
        panel.reload();
        panel.sessions_source.session_details()
    })
}

/// What the self-test reads from the panel.
#[cfg(feature = "selftest")]
pub struct Inspection {
    pub sessions: Vec<String>,
    pub running: Vec<String>,
    pub usage: Vec<String>,
    pub active: String,
    /// The titles of the "new session" menu.
    pub new_session: Vec<String>,
}

/// In the panel's coordinates: where the first section's title begins, and where the content of each row of the two
/// lists begins, with the row's level.
#[cfg(feature = "selftest")]
pub fn edges(controller: &WindowController) -> Option<(f64, Vec<(isize, f64)>)> {
    let panel = panel_of(controller)?;
    panel.reload();
    panel.view.layoutSubtreeIfNeeded();
    let x = |view: &NSView| view.convertRect_toView(view.bounds(), Some(&panel.view)).origin.x;
    let head = panel.view.arrangedSubviews().firstObject()?;
    let label = head.downcast_ref::<NSStackView>()?.arrangedSubviews().firstObject()?;
    let mut rows = Vec::new();
    for outline in [&panel.sessions, &panel.running] {
        for row in 0..outline.numberOfRows() {
            let Some(cell) = outline.viewAtColumn_row_makeIfNecessary(0, row, true) else {
                continue;
            };
            cell.layoutSubtreeIfNeeded();
            let first = cell
                .subviews()
                .iter()
                .find_map(|view| view.downcast::<NSStackView>().ok())
                .and_then(|stack| stack.arrangedSubviews().firstObject());
            if let Some(first) = first {
                rows.push((outline.levelForRow(row), x(&first)));
            }
        }
    }
    Some((x(&label), rows))
}

#[cfg(feature = "selftest")]
pub fn inspect(controller: &WindowController) -> Option<Inspection> {
    let panel = panel_of(controller)?;
    panel.reload();
    Some(Inspection {
        sessions: panel.sessions_source.titles(),
        running: panel.running_source.titles(),
        usage: panel.usage.texts(),
        active: panel.active.stringValue().to_string(),
        new_session: panel.agents.menu_titles(),
    })
}
