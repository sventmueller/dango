//! The panel's one list of the folder's sessions, bookmarked ones first, else newest first: those of AI tabs and those
//! of the CLIs alike, the open ones with a green circle. A click opens one; the context menu renames, bookmarks and
//! archives a session and shows the archive.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;

use ide_thread::{Filter, IndexEntry, ThreadIndex};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSTableViewStyle, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::rows::{self, Look};
use crate::AgentKey;
use crate::{AgentTab, Resume};
use ide_ui_common::panel::{KeptCells, RowItem, RowItems};
use ide_ui_common::util::{guarded, item, ns, set_action, set_target};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// Where a row begins: its content, set in by the row's own inset, then starts where the panel's title does.
const ROW_LEADING: f64 = -2.0;
/// Where a row ends, from the list's far edge: its age then ends with the title's button.
const ROW_TRAILING: f64 = 2.0;

#[derive(Clone)]
pub(super) enum Node {
    /// A session of an AI tab: from the index, or open in a tab and not saved yet.
    Thread {
        entry: IndexEntry,
        tab: Option<AgentKey>,
        bookmarked: bool,
    },
    /// A session the CLI saved from a terminal, which the index does not know.
    Past { resume: Resume, when: SystemTime },
}

impl Node {
    /// Whether the session is open in a tab of the window.
    fn open(&self) -> bool {
        match self {
            Node::Thread { tab, .. } => tab.is_some(),
            Node::Past { .. } => false,
        }
    }
}

/// The rows by id, in their order.
#[derive(Default)]
pub(super) struct Model {
    nodes: HashMap<String, Node>,
    order: Vec<String>,
}

/// What the list is built from.
pub(crate) struct Inputs<'a> {
    pub(crate) index: &'a ThreadIndex,
    pub(crate) root: &'a Path,
    pub(crate) tabs: &'a [Rc<AgentTab>],
    pub(crate) archived: bool,
    pub(crate) terminal: Vec<ide_claude::history::PastSession>,
    /// The Codex threads of the folder begun outside the app, as `thread/list` named them.
    pub(crate) codex: Vec<(Resume, SystemTime)>,
    pub(crate) bookmarks: &'a BTreeSet<String>,
}

impl Model {
    pub(super) fn build(inputs: &Inputs<'_>) -> Model {
        let mut rows: Vec<(bool, SystemTime, String, Node)> = Vec::new();
        let filter = Filter {
            archived: inputs.archived,
            ..Filter::default()
        };
        let listed = inputs.index.list(&filter);
        let entries: Vec<&IndexEntry> = listed
            .iter()
            .copied()
            .filter(|e| e.cwd.as_deref() == Some(inputs.root))
            .collect();
        let tab_of = |id: &ide_thread::ThreadId| inputs.tabs.iter().find(|t| t.thread_id() == id);
        for entry in &entries {
            let node = thread_node((*entry).clone(), tab_of(&entry.id), inputs.bookmarks);
            rows.push((
                bookmarked(&node),
                entry.last_activity,
                format!("thread:{}", entry.id),
                node,
            ));
        }
        if !inputs.archived {
            let shown: HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
            for tab in inputs.tabs.iter().filter(|t| !shown.contains(t.thread_id().as_str())) {
                let node = thread_node(
                    IndexEntry::of(&tab.view_snapshot(), SystemTime::now()),
                    Some(tab),
                    inputs.bookmarks,
                );
                rows.push((
                    bookmarked(&node),
                    SystemTime::now(),
                    format!("thread:{}", tab.thread_id()),
                    node,
                ));
            }
            // A session an AI tab continues shows once, as that tab's.
            let open: Vec<String> = inputs.tabs.iter().filter_map(|t| t.session_id()).collect();
            let known: HashSet<&str> = listed
                .iter()
                .filter_map(|e| e.session.as_deref())
                .chain(open.iter().map(String::as_str))
                .collect();
            for session in inputs
                .terminal
                .iter()
                .filter(|s| !known.contains(s.id.as_str()))
                .take(30)
            {
                let resume = Resume {
                    session: session.id.clone(),
                    thread: None,
                    title: session.heading(),
                    transcript: Some(session.path.clone()),
                    agent: None,
                };
                let node = Node::Past {
                    resume,
                    when: session.modified,
                };
                rows.push((false, session.modified, format!("past:{}", session.id), node));
            }
            for (resume, when) in inputs
                .codex
                .iter()
                .filter(|(r, _)| !known.contains(r.session.as_str()))
                .take(30)
            {
                let id = format!("past:{}", resume.session);
                let node = Node::Past {
                    resume: resume.clone(),
                    when: *when,
                };
                rows.push((false, *when, id, node));
            }
        }
        rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        let mut model = Model::default();
        for (_, _, id, node) in rows {
            model.order.push(id.clone());
            model.nodes.insert(id, node);
        }
        model
    }
}

fn bookmarked(node: &Node) -> bool {
    matches!(node, Node::Thread { bookmarked: true, .. })
}

fn thread_node(entry: IndexEntry, tab: Option<&Rc<AgentTab>>, bookmarks: &BTreeSet<String>) -> Node {
    let mut entry = entry;
    if let Some(tab) = tab {
        entry.title = tab.title();
    }
    Node::Thread {
        bookmarked: bookmarks.contains(entry.id.as_str()),
        tab: tab.map(|t| t.key),
        entry,
    }
}

pub(crate) struct SourceIvars {
    panel: Cell<usize>,
    model: RefCell<Model>,
    items: RowItems<String>,
    cells: KeptCells<String>,
    /// The id of the row the context menu was opened on.
    clicked: RefCell<Option<String>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Source` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentPanelSource"]
    #[ivars = SourceIvars]
    pub(crate) struct Source;

    unsafe impl NSObjectProtocol for Source {}

    unsafe impl NSControlTextEditingDelegate for Source {}

    unsafe impl NSOutlineViewDataSource for Source {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || match item {
                None => isize::try_from(self.ivars().model.borrow().order.len()).unwrap_or(0),
                Some(_) => 0,
            })
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, _item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let model = self.ivars().model.borrow();
                let id = model.order.get(usize::try_from(index).ok()?)?;
                Some(self.ivars().items.item(self.mtm(), id))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, _item: &AnyObject) -> bool {
            false
        }
    }

    unsafe impl NSOutlineViewDelegate for Source {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(&self, _view: &NSOutlineView, _column: Option<&NSTableColumn>, item: &AnyObject) -> Option<Retained<NSView>> {
            guarded(None, || {
                let id = id_of(item)?;
                let model = self.ivars().model.borrow();
                let shown = shown(model.nodes.get(id)?);
                let look = (shown.open, shown.title, shown.dim, minutes_ago(shown.when), shown.bookmarked);
                self.ivars().cells.cell(id, &look, || Some(self.cell(&shown)))
            })
        }
    }

    unsafe impl NSMenuDelegate for Source {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    impl Source {
        #[unsafe(method(ideAgentPanelOpen:))]
        fn open(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(view) = sender.and_then(|s| s.downcast_ref::<NSOutlineView>()) else { return };
                let Some(item) = view.itemAtRow(view.clickedRow()) else { return };
                if let Some(id) = id_of(&item).cloned() {
                    self.open_id(&id);
                }
            });
        }

        #[unsafe(method(ideAgentPanelArchive:))]
        fn toggle_archive(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.with_panel(|panel| {
                    panel.show_archived.set(!panel.show_archived.get());
                    panel.reload_soon();
                });
            });
        }

        #[unsafe(method(ideAgentPanelRename:))]
        fn rename(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.rename_clicked());
        }

        #[unsafe(method(ideAgentPanelArchiveItem:))]
        fn archive_item(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.archive_clicked());
        }

        #[unsafe(method(ideAgentPanelBookmark:))]
        fn bookmark(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(thread) = self.clicked_thread() {
                    let id = ide_thread::ThreadId::from(thread.as_str());
                    super::set_bookmarked(&id, !super::is_bookmarked(&id));
                }
            });
        }
    }
);

/// What the row of a node shows, its age as of when the session was last used.
struct Shown<'a> {
    open: bool,
    title: &'a str,
    dim: bool,
    when: SystemTime,
    bookmarked: bool,
}

fn shown(node: &Node) -> Shown<'_> {
    match node {
        Node::Thread { entry, bookmarked, .. } => Shown {
            open: node.open(),
            title: if entry.title.is_empty() {
                t("Neue Sitzung")
            } else {
                entry.title.as_str()
            },
            dim: entry.archived,
            when: entry.last_activity,
            bookmarked: *bookmarked,
        },
        Node::Past { resume, when } => Shown {
            open: false,
            title: resume.title.as_deref().unwrap_or(&resume.session),
            dim: false,
            when: *when,
            bookmarked: false,
        },
    }
}

/// The whole minutes since `when`, finer than any age `short_age` tells apart, so a row's age is formatted only when
/// that changes.
fn minutes_ago(when: SystemTime) -> u64 {
    SystemTime::now().duration_since(when).unwrap_or_default().as_secs() / 60
}

fn id_of(item: &AnyObject) -> Option<&String> {
    item.downcast_ref::<RowItem>()?.ivars().downcast_ref::<String>()
}

/// The outline view of the list with its data source.
pub(super) fn outline(mtm: MainThreadMarker) -> (Retained<NSOutlineView>, Retained<Source>) {
    let ivars = SourceIvars {
        panel: Cell::new(0),
        model: RefCell::new(Model::default()),
        items: RowItems::default(),
        cells: KeptCells::default(),
        clicked: RefCell::new(None),
    };
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let source: Retained<Source> = unsafe { msg_send![super(Source::alloc(mtm).set_ivars(ivars)), init] };
    let view: Retained<NSOutlineView> = Retained::into_super(ide_ui_common::file_outline::FileOutline::with_insets(
        mtm,
        objc2_foundation::NSRect::ZERO,
        ROW_LEADING,
        Some(ROW_TRAILING),
    ));
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("row"));
    view.addTableColumn(&column);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { view.setOutlineTableColumn(Some(&column)) };
    view.setHeaderView(None);
    view.setStyle(NSTableViewStyle::SourceList);
    ide_ui_common::ui_size::follow_rows(&view, 22.0);
    view.setColumnAutoresizingStyle(
        objc2_app_kit::NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle,
    );
    // SAFETY: The view holds its data source and delegate weakly; the panel keeps the source alive with the view.
    unsafe {
        view.setDataSource(Some(ProtocolObject::from_ref(&*source)));
        view.setDelegate(Some(ProtocolObject::from_ref(&*source)));
    }
    set_action(&view, &source, sel!(ideAgentPanelOpen:));
    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(&*source)));
    // SAFETY: The view keeps the menu; its delegate, the source, lives as long as the view through the panel.
    unsafe { view.setMenu(Some(&menu)) };
    (view, source)
}

impl Source {
    pub(super) fn set_panel(&self, key: usize) {
        self.ivars().panel.set(key);
    }

    fn with_panel(&self, f: impl FnOnce(&super::AgentPanel)) {
        if let Some(panel) = super::panel_by_key(self.ivars().panel.get()) {
            f(&panel);
        }
    }

    pub(super) fn show(&self, view: &NSOutlineView, model: Model) {
        *self.ivars().model.borrow_mut() = model;
        self.ivars().cells.keep_shown();
        ide_ui_common::panel::reload(view);
        let model = self.ivars().model.borrow();
        self.ivars().items.keep(|id| model.nodes.contains_key(id));
        self.ivars().cells.keep(|id| model.nodes.contains_key(id));
        drop(model);
        view.sizeLastColumnToFit();
    }

    /// The row of what `shown` holds: whether it is open, its title and how long ago it was used.
    fn cell(&self, shown: &Shown<'_>) -> Retained<NSView> {
        rows::cell(
            self.mtm(),
            &Look {
                open: shown.open,
                title: shown.title,
                dim: shown.dim,
                detail: &crate::short_age(shown.when),
                badge: shown
                    .bookmarked
                    .then(|| ("bookmark.fill", NSColor::controlAccentColor())),
            },
        )
    }

    fn open_id(&self, id: &str) {
        let Some(node) = self.ivars().model.borrow().nodes.get(id).cloned() else {
            return;
        };
        let Some(panel) = super::panel_by_key(self.ivars().panel.get()) else {
            return;
        };
        let Some(controller) = panel.controller() else { return };
        match node {
            Node::Thread { tab: Some(key), .. } => {
                if let Some(tab) = crate::of_window(&controller).into_iter().find(|t| t.key == key) {
                    tab.select();
                }
            }
            Node::Thread { entry, tab: None, .. } => {
                let Some(session) = entry.session.clone() else { return };
                let resume = Resume {
                    transcript: super::transcript(&panel.root, &session),
                    session,
                    thread: Some(entry.id.clone()),
                    title: Some(entry.title.clone()).filter(|t| !t.is_empty()),
                    agent: None,
                };
                crate::open_agent(&controller, crate::Agent::of_thread(&entry.agent), Some(resume));
            }
            Node::Past { resume, .. } => match resume.agent.as_deref().and_then(crate::AcpAgent::find) {
                Some(agent) => {
                    crate::open_agent(&controller, crate::Agent::Acp(agent), Some(resume));
                }
                None => {
                    crate::open_tab(&controller, Some(resume));
                }
            },
        }
    }

    fn clicked_thread(&self) -> Option<String> {
        self.ivars()
            .clicked
            .borrow()
            .as_deref()
            .and_then(|id| id.strip_prefix("thread:"))
            .map(str::to_string)
    }

    fn fill_menu(&self, menu: &NSMenu) {
        let Some(panel) = super::panel_by_key(self.ivars().panel.get()) else {
            menu.removeAllItems();
            return;
        };
        let row = panel.sessions.clickedRow();
        self.fill_menu_for(menu, &panel, row);
    }

    /// The context menu of row `row` of the list, `-1` for none.
    fn fill_menu_for(&self, menu: &NSMenu, panel: &super::AgentPanel, row: isize) {
        menu.removeAllItems();
        menu.setAutoenablesItems(false);
        let id = panel.sessions.itemAtRow(row).and_then(|item| id_of(&item).cloned());
        *self.ivars().clicked.borrow_mut() = id;
        let mtm = self.mtm();
        let add = |title: &str, action: objc2::runtime::Sel| {
            let entry = item(mtm, title, Some(action), "");
            set_target(&entry, self);
            menu.addItem(&entry);
        };
        if let Some(thread) = self.clicked_thread() {
            let archived = matches!(
                self.ivars().model.borrow().nodes.get(&format!("thread:{thread}")),
                Some(Node::Thread { entry, .. }) if entry.archived
            );
            add(t("Umbenennen …"), sel!(ideAgentPanelRename:));
            let marked = super::is_bookmarked(&ide_thread::ThreadId::from(thread.as_str()));
            add(
                if marked {
                    t("Lesezeichen entfernen")
                } else {
                    t("Lesezeichen setzen")
                },
                sel!(ideAgentPanelBookmark:),
            );
            add(
                if archived {
                    t("Aus dem Archiv holen")
                } else {
                    t("Archivieren")
                },
                sel!(ideAgentPanelArchiveItem:),
            );
            menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(mtm));
        }
        add(
            if panel.show_archived.get() {
                t("Zurück zu den Sitzungen")
            } else {
                t("Archiv zeigen")
            },
            sel!(ideAgentPanelArchive:),
        );
    }

    fn rename_clicked(&self) {
        let Some(thread) = self.clicked_thread() else { return };
        let id = ide_thread::ThreadId::from(thread.as_str());
        let open = super::panel_by_key(self.ivars().panel.get())
            .and_then(|p| p.controller())
            .and_then(|c| crate::of_window(&c).into_iter().find(|t| t.thread_id() == &id));
        let current = match &open {
            Some(tab) => tab.title(),
            None => self.listed(&thread).map(|e| e.title).unwrap_or_default(),
        };
        let Some(name) = crate::ask_text(self.mtm(), t("Sitzung umbenennen"), &current) else {
            return;
        };
        match open {
            Some(tab) => tab.set_title(&name),
            None => crate::agent_panel::rename(&id, &name),
        }
    }

    fn archive_clicked(&self) {
        let Some(thread) = self.clicked_thread() else { return };
        let id = ide_thread::ThreadId::from(thread.as_str());
        let archived = self.listed(&thread).is_some_and(|e| e.archived);
        super::set_archived(&id, !archived);
    }

    /// The index's entry of the listed thread `thread`, as the list was read last.
    fn listed(&self, thread: &str) -> Option<ide_thread::IndexEntry> {
        match self.ivars().model.borrow().nodes.get(&format!("thread:{thread}")) {
            Some(Node::Thread { entry, .. }) => Some(entry.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    use ide_claude::history::PastSession;
    use ide_core::testing::Scratch;
    use ide_thread::{IndexEntry, Status, ThreadId, ThreadIndex};

    use super::*;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn entry(id: &str, cwd: &Path, when: u64, session: Option<&str>) -> IndexEntry {
        IndexEntry {
            id: ThreadId::from(id),
            title: id.to_string(),
            agent: "claude".into(),
            group: None,
            archived: false,
            last_activity: at(when),
            status: Status::default(),
            session: session.map(str::to_string),
            cwd: Some(cwd.to_path_buf()),
            cost_usd: None,
        }
    }

    fn past(id: &str, when: u64) -> PastSession {
        PastSession {
            id: id.into(),
            title: Some(format!("title {id}")),
            first_prompt: None,
            last_prompt: None,
            git_branch: None,
            modified: at(when),
            path: PathBuf::from(format!("/t/{id}.jsonl")),
        }
    }

    fn build(index: &ThreadIndex, root: &Path, archived: bool, terminal: Vec<PastSession>, marks: &[&str]) -> Model {
        let bookmarks = marks.iter().map(|m| m.to_string()).collect();
        Model::build(&Inputs {
            index,
            root,
            tabs: &[],
            archived,
            terminal,
            codex: Vec::new(),
            bookmarks: &bookmarks,
        })
    }

    #[test]
    fn bookmarked_sessions_come_first_then_the_newest() {
        let scratch = Scratch::new("sessions-order");
        let root = Path::new("/project");
        let mut index = ThreadIndex::open(scratch.path());
        for (id, when) in [("old", 1), ("new", 3), ("marked", 2)] {
            index.upsert(entry(id, root, when, None)).unwrap();
        }
        let model = build(&index, root, false, vec![past("cli", 4)], &["marked"]);
        assert_eq!(model.order, ["thread:marked", "past:cli", "thread:new", "thread:old"]);
    }

    #[test]
    fn only_sessions_of_the_folder_are_listed() {
        let scratch = Scratch::new("sessions-folder");
        let mut index = ThreadIndex::open(scratch.path());
        index.upsert(entry("here", Path::new("/project"), 1, None)).unwrap();
        index.upsert(entry("elsewhere", Path::new("/other"), 2, None)).unwrap();
        let model = build(&index, Path::new("/project"), false, Vec::new(), &[]);
        assert_eq!(model.order, ["thread:here"]);
    }

    #[test]
    fn a_terminal_session_the_index_knows_shows_once() {
        let scratch = Scratch::new("sessions-known");
        let root = Path::new("/project");
        let mut index = ThreadIndex::open(scratch.path());
        index.upsert(entry("tab", root, 1, Some("s1"))).unwrap();
        let model = build(&index, root, false, vec![past("s1", 5), past("s2", 6)], &[]);
        assert_eq!(model.order, ["past:s2", "thread:tab"]);
    }

    #[test]
    fn the_archive_lists_no_terminal_sessions() {
        let scratch = Scratch::new("sessions-archive");
        let root = Path::new("/project");
        let mut index = ThreadIndex::open(scratch.path());
        let mut gone = entry("gone", root, 1, None);
        gone.archived = true;
        index.upsert(gone).unwrap();
        index.upsert(entry("kept", root, 2, None)).unwrap();
        let model = build(&index, root, true, vec![past("cli", 3)], &[]);
        assert_eq!(model.order, ["thread:gone"]);
    }

    #[test]
    fn at_most_thirty_terminal_sessions_are_listed() {
        let scratch = Scratch::new("sessions-many");
        let index = ThreadIndex::open(scratch.path());
        let terminal = (0..40).map(|n| past(&format!("s{n}"), n)).collect();
        let model = build(&index, Path::new("/project"), false, terminal, &[]);
        assert_eq!(model.order.len(), 30);
    }
}
