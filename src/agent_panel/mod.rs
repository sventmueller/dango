//! The AI panel of the sidebar: under its title, which offers a new session, one list of the folder's sessions, those
//! of AI tabs and of CLIs, with an archive behind the list's context menu (E68).

mod agents;
mod rows;
mod sessions;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use ide_thread::{ThreadId, ThreadIndex};
use objc2::MainThreadOnly;
use objc2::rc::{Retained, Weak};
#[cfg(feature = "selftest")]
use objc2_app_kit::NSView;
use objc2_app_kit::{NSAccessibility, NSLayoutAttribute, NSOutlineView, NSScrollView, NSStackView};
use splat_i18n::t;

use crate::agent::Resume;
use crate::util::{main_after, ns};
use crate::window::panels::ACCESSORY_INSET;
use crate::window::{Panel, WindowController};
pub use rows::{flipped, section_label, small_label, spacer};
use sessions::Source;

/// How far the panel's content keeps from the sidebar's leading edge: as far as the panel's title; at the trailing
/// edge it ends with the title's button.
const SIDE: f64 = 8.0;

/// The panel of one window.
pub struct AgentPanel {
    controller: Weak<WindowController>,
    root: std::path::PathBuf,
    view: Retained<NSStackView>,
    sessions: Retained<NSOutlineView>,
    sessions_source: Retained<Source>,
    show_archived: Cell<bool>,
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
        let title = session.heading();
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
#[cfg(feature = "selftest")]
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
        let agents = agents::Agents::new(controller);
        let (sessions, sessions_source) = sessions::outline(mtm);
        let view = NSStackView::new(mtm);
        view.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        view.setAlignment(NSLayoutAttribute::Leading);
        view.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 0.0,
            left: SIDE,
            bottom: 8.0,
            right: ACCESSORY_INSET,
        });
        let panel = Rc::new(AgentPanel {
            controller: Weak::from(controller),
            root,
            view,
            sessions,
            sessions_source,
            show_archived: Cell::new(false),
            scheduled: Cell::new(false),
            agents,
        });
        panel.sessions_source.set_panel(Rc::as_ptr(&panel) as usize);
        let scroll = scroll(mtm, &panel.sessions);
        panel.view.addArrangedSubview(&scroll);
        scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
        scroll
            .widthAnchor()
            .constraintEqualToAnchor_constant(&panel.view.widthAnchor(), -SIDE - ACCESSORY_INSET)
            .setActive(true);
        panel.sessions.setAccessibilityLabel(Some(&ns(t("Sitzungen"))));
        controller.set_panel_accessory(Panel::Agents, &panel.agents.new_session_button());
        panel
    }

    /// Reads the index and the open tabs again and shows them.
    fn reload(&self) {
        let Some(controller) = self.controller.load() else {
            return;
        };
        let tabs = crate::agent::of_window(&controller);
        let index = ThreadIndex::open(&ThreadIndex::default_dir());
        let archived = self.show_archived.get();
        let model = sessions::Model::build(&sessions::Inputs {
            index: &index,
            root: &self.root,
            tabs: &tabs,
            archived,
            terminal: if archived {
                Vec::new()
            } else {
                terminal_sessions(&self.root)
            },
            bookmarks: &bookmarks(),
        });
        self.sessions_source.show(&self.sessions, model);
    }

    fn controller(&self) -> Option<Retained<WindowController>> {
        self.controller.load()
    }
}

fn scroll(mtm: objc2::MainThreadMarker, outline: &NSOutlineView) -> Retained<NSScrollView> {
    let scroll = NSScrollView::new(mtm);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(outline));
    scroll
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
    /// The rows top to bottom: "AI" or "CLI", whether the session is open, and its title.
    pub rows: Vec<(&'static str, bool, String)>,
    /// The titles of the "new session" menu.
    pub new_session: Vec<String>,
}

/// In the panel's coordinates, where the content of each row of the list begins and where it ends.
#[cfg(feature = "selftest")]
pub fn edges(controller: &WindowController) -> Option<Vec<(f64, f64)>> {
    let panel = panel_of(controller)?;
    panel.reload();
    panel.view.layoutSubtreeIfNeeded();
    let frame = |view: &NSView| view.convertRect_toView(view.bounds(), Some(&panel.view));
    let mut rows = Vec::new();
    for row in 0..panel.sessions.numberOfRows() {
        let Some(cell) = panel.sessions.viewAtColumn_row_makeIfNecessary(0, row, true) else {
            continue;
        };
        cell.layoutSubtreeIfNeeded();
        let views = cell
            .subviews()
            .iter()
            .find_map(|view| view.downcast::<NSStackView>().ok())
            .map(|stack| stack.arrangedSubviews());
        if let Some((first, last)) = views.and_then(|v| v.firstObject().zip(v.lastObject())) {
            let last = frame(&last);
            rows.push((frame(&first).origin.x, last.origin.x + last.size.width));
        }
    }
    Some(rows)
}

#[cfg(feature = "selftest")]
pub fn inspect(controller: &WindowController) -> Option<Inspection> {
    let panel = panel_of(controller)?;
    panel.reload();
    Some(Inspection {
        rows: panel.sessions_source.rows(),
        new_session: panel.agents.menu_titles(),
    })
}
