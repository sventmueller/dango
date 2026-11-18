//! The AI panel of the sidebar: under its title, which offers a new session, one list of the folder's sessions, those
//! of AI tabs and of CLIs, with an archive behind the list's context menu (E68).

mod agents;
mod rows;
mod sessions;
mod store;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use ide_thread::ThreadIndex;
use objc2::rc::Retained;
#[cfg(feature = "selftest")]
use objc2_app_kit::NSView;
use objc2_app_kit::{NSAccessibility, NSLayoutAttribute, NSOutlineView, NSScrollView, NSStackView};
use splat_i18n::t;

use crate::Resume;
use ide_ui_common::panel::ACCESSORY_INSET;
use ide_ui_common::util::{main_after, main_soon, ns};
use ide_ui_common::window::{AsWindow, Panel, Panels, Window};
pub use rows::{flipped, section_label, small_label, spacer};
use sessions::Source;
pub use store::{is_bookmarked, past_session, wait_for_writes};
pub(crate) use store::{past_sessions_cached, read_past, record, remove, rename, set_archived, set_bookmarked, upsert};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// How far the panel's content keeps from the sidebar's leading edge: as far as the panel's title; at the trailing
/// edge it ends with the title's button.
const SIDE: f64 = 8.0;

/// The panel of one window.
pub(crate) struct AgentPanel {
    /// The panel's id among the open ones, which callbacks carry.
    id: usize,
    controller: Window,
    root: std::path::PathBuf,
    view: Retained<NSStackView>,
    sessions: Retained<NSOutlineView>,
    sessions_source: Retained<Source>,
    show_archived: Cell<bool>,
    scheduled: Cell<bool>,
    /// The latest reading of the lists, so that an older one a worker answers late is dropped.
    reading: Cell<u64>,
    agents: agents::Agents,
}

/// The agent panels of every window.
#[derive(Default)]
pub(crate) struct AgentPanels {
    panels: RefCell<Vec<Rc<AgentPanel>>>,
    last_id: Cell<usize>,
    offered_agents: agents::OfferedAgents,
    store: store::Store,
}

fn with_state<R>(f: impl FnOnce(&AgentPanels) -> R) -> R {
    f(&crate::host::state().agent_panels)
}

fn with_panels<R>(f: impl FnOnce(&RefCell<Vec<Rc<AgentPanel>>>) -> R) -> R {
    with_state(|state| f(&state.panels))
}

/// Reads every panel's lists again in the background.
fn reload_all() {
    for panel in with_panels(|panels| panels.borrow().clone()) {
        if panel.controller.is_open() {
            panel.reload_soon();
        }
    }
}

fn panel_of(controller: &Window) -> Option<Rc<AgentPanel>> {
    with_panels(|panels| {
        panels
            .borrow()
            .iter()
            .find(|p| p.controller.is_open() && p.controller == *controller)
            .cloned()
    })
}

fn panel_by_key(key: usize) -> Option<Rc<AgentPanel>> {
    with_panels(|panels| panels.borrow().iter().find(|p| p.id == key).cloned())
}

/// Puts the AI panel of the window's folder in place of the Agents page.
pub fn attach(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    with_panels(|panels| {
        panels
            .borrow_mut()
            .retain(|p| p.controller.is_open() && p.controller != *controller)
    });
    let Some(root) = controller.project_root() else { return };
    let panel = AgentPanel::new(controller, root);
    controller.set_panel_page(Panel::Agents, &panel.view);
    with_panels(|panels| panels.borrow_mut().push(panel.clone()));
    store::read_bookmarks();
    panel.reload_soon();
}

/// Shows what changed in the window's sessions, at most a few times a second.
pub(crate) fn refresh(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    let Some(panel) = panel_of(controller) else { return };
    if panel.scheduled.replace(true) {
        return;
    }
    let key = panel.id;
    main_after(Duration::from_millis(400), move || {
        if let Some(panel) = panel_by_key(key) {
            panel.scheduled.set(false);
            panel.reload_soon();
        }
    });
}

fn bookmarks_file() -> std::path::PathBuf {
    ThreadIndex::default_dir().join("bookmarks.json")
}

fn bookmarks() -> BTreeSet<String> {
    bookmarks_in(&bookmarks_file())
}

fn bookmarks_in(file: &Path) -> BTreeSet<String> {
    std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes the bookmarks `marks`.
fn write_bookmarks(marks: &BTreeSet<String>) -> std::io::Result<()> {
    write_bookmarks_to(&bookmarks_file(), marks)
}

fn write_bookmarks_to(file: &Path, marks: &BTreeSet<String>) -> std::io::Result<()> {
    ide_core::atomic_write::write(file, &serde_json::to_vec(marks).unwrap_or_default())
}

/// The sessions of `cwd` that can be resumed, newest first: the index's, then — experimental — those the CLI saved
/// from a terminal. Reads the disk; the main thread takes `past_sessions_cached`.
pub(crate) fn past_sessions(cwd: &Path, limit: usize) -> Vec<(Resume, SystemTime)> {
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
                    agent: None,
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
                agent: None,
            },
            session.modified,
        ));
    }
    let known: BTreeSet<String> = past.iter().map(|(r, _)| r.session.clone()).collect();
    past.extend(
        crate::codex::past_threads(cwd)
            .into_iter()
            .filter(|(r, _)| !known.contains(&r.session)),
    );
    past.sort_by_key(|(_, when)| std::cmp::Reverse(*when));
    past.truncate(limit);
    past
}

fn config_dir() -> std::path::PathBuf {
    ide_claude::history::config_dir(&ide_ui_common::window::environment(std::time::Duration::ZERO))
}

/// The sessions the CLI saved for `cwd`, as its own session list shows them; experimental.
fn terminal_sessions(cwd: &Path) -> Vec<ide_claude::history::PastSession> {
    ide_claude::history::list(&config_dir(), cwd)
}

fn transcript(cwd: &Path, session: &str) -> Option<std::path::PathBuf> {
    if session.contains('/') {
        return None;
    }
    let path = config_dir()
        .join("projects")
        .join(ide_claude::history::project_folder(cwd))
        .join(format!("{session}.jsonl"));
    path.is_file().then_some(path)
}

impl AgentPanel {
    fn new(controller: &Window, root: std::path::PathBuf) -> Rc<AgentPanel> {
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
        let state = crate::host::state();
        let ids = &state.agent_panels.last_id;
        ids.set(ids.get() + 1);
        let panel = Rc::new(AgentPanel {
            id: ids.get(),
            controller: *controller,
            root,
            view,
            sessions,
            sessions_source,
            show_archived: Cell::new(false),
            scheduled: Cell::new(false),
            reading: Cell::new(0),
            agents,
        });
        panel.sessions_source.set_panel(panel.id);
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
        self.reading.set(self.reading.get() + 1);
        self.show(Lists::read(&self.root, self.show_archived.get()));
    }

    /// `reload` with the files read on a worker thread, as the transcripts of a folder take long to read; the newest
    /// reading wins.
    fn reload_soon(&self) {
        let reading = self.reading.get() + 1;
        self.reading.set(reading);
        let (key, root, archived) = (self.id, self.root.clone(), self.show_archived.get());
        let spawned = ide_ui_common::util::spawn("agent-panel", move || {
            let lists = Lists::read(&root, archived);
            main_soon(move || {
                if let Some(panel) = panel_by_key(key)
                    && panel.reading.get() == reading
                {
                    panel.show(lists);
                }
            });
        });
        if spawned.is_err() {
            self.reload();
        }
    }

    fn show(&self, lists: Lists) {
        let Some(controller) = self.controller.load() else {
            return;
        };
        let tabs = crate::of_window(&controller);
        let model = sessions::Model::build(&sessions::Inputs {
            index: &lists.index,
            root: &self.root,
            tabs: &tabs,
            archived: lists.archived,
            terminal: lists.terminal,
            codex: lists.codex,
            bookmarks: &lists.bookmarks,
        });
        self.sessions_source.show(&self.sessions, model);
    }

    fn controller(&self) -> Option<Window> {
        self.controller.load()
    }
}

/// What the panel lists besides the open tabs, as read from the disk.
struct Lists {
    index: ThreadIndex,
    archived: bool,
    terminal: Vec<ide_claude::history::PastSession>,
    /// The Codex threads of the folder the running Codex last named.
    codex: Vec<(Resume, SystemTime)>,
    bookmarks: BTreeSet<String>,
}

impl Lists {
    fn read(root: &Path, archived: bool) -> Lists {
        Lists {
            index: ThreadIndex::open(&ThreadIndex::default_dir()),
            archived,
            terminal: if archived { Vec::new() } else { terminal_sessions(root) },
            codex: if archived {
                Vec::new()
            } else {
                crate::codex::past_threads(root)
            },
            bookmarks: bookmarks(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_replace_the_file_whole_and_leave_no_scratch_file() {
        let dir = ide_core::testing::scratch("bookmarks");
        let file = dir.path().join("threads").join("bookmarks.json");
        let first: BTreeSet<String> = ["a".to_string(), "b".to_string()].into();
        write_bookmarks_to(&file, &first).unwrap();
        assert_eq!(bookmarks_in(&file), first);
        let second: BTreeSet<String> = ["c".to_string()].into();
        write_bookmarks_to(&file, &second).unwrap();
        assert_eq!(bookmarks_in(&file), second);
        let names: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["bookmarks.json"]);
    }
}
