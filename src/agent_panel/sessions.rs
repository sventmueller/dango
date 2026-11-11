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
use objc2_foundation::{NSObjectProtocol, NSString};
use splat_i18n::t;

use super::rows::{self, Look};
use crate::agent::{AgentTab, Resume};
use crate::util::{guarded, item, ns, set_action, set_target};

/// Where a row begins: its content, set in by the row's own inset, then starts where the panel's title does.
const ROW_LEADING: f64 = -2.0;

#[derive(Clone)]
pub enum Node {
    /// A session of an AI tab: from the index, or open in a tab and not saved yet.
    Thread {
        entry: IndexEntry,
        tab: Option<u64>,
        bookmarked: bool,
    },
    /// A session the CLI saved from a terminal, which the index does not know.
    Past { resume: Resume, when: SystemTime },
    /// A CLI thread (I5): a coding agent's CLI in a terminal tab of the window.
    Cli(crate::terminal::CliThread),
}

impl Node {
    /// Whether the session is open in a tab of the window.
    fn open(&self) -> bool {
        match self {
            Node::Thread { tab, .. } => tab.is_some(),
            Node::Past { .. } => false,
            Node::Cli(_) => true,
        }
    }
}

/// The rows by id, in their order.
#[derive(Default)]
pub struct Model {
    nodes: HashMap<String, Node>,
    order: Vec<String>,
}

/// What the list is built from.
pub struct Inputs<'a> {
    pub index: &'a ThreadIndex,
    pub root: &'a Path,
    pub tabs: &'a [Rc<AgentTab>],
    pub archived: bool,
    pub terminal: Vec<ide_claude::history::PastSession>,
    /// The window's CLI threads, which run in its terminal tabs.
    pub cli: Vec<crate::terminal::CliThread>,
    pub bookmarks: &'a BTreeSet<String>,
}

impl Model {
    pub fn build(inputs: &Inputs<'_>) -> Model {
        let mut rows: Vec<(bool, SystemTime, String, Node)> = Vec::new();
        let filter = Filter {
            archived: inputs.archived,
            ..Filter::default()
        };
        let entries: Vec<&IndexEntry> = inputs
            .index
            .list(&filter)
            .into_iter()
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
            for thread in &inputs.cli {
                rows.push((
                    false,
                    thread.started,
                    format!("cli:{}", thread.key),
                    Node::Cli(thread.clone()),
                ));
            }
            // A session an AI tab continues shows once, as that tab's.
            let open: Vec<String> = inputs.tabs.iter().filter_map(|t| t.session_id()).collect();
            let known: HashSet<&str> = inputs
                .index
                .list(&Filter::default())
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
                    title: session.title.clone().or_else(|| session.first_prompt.clone()),
                    transcript: Some(session.path.clone()),
                };
                let node = Node::Past {
                    resume,
                    when: session.modified,
                };
                rows.push((false, session.modified, format!("past:{}", session.id), node));
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
        let thread = tab.view_snapshot();
        entry.cost_usd = if thread.usage().subscription {
            None
        } else {
            thread.usage().cost_usd.or(entry.cost_usd)
        };
    }
    Node::Thread {
        bookmarked: bookmarks.contains(entry.id.as_str()),
        tab: tab.map(|t| t.key),
        entry,
    }
}

pub struct SourceIvars {
    panel: Cell<usize>,
    model: RefCell<Model>,
    items: RefCell<HashMap<String, Retained<NSString>>>,
    /// The id of the row the context menu was opened on.
    clicked: RefCell<Option<String>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Source` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentPanelSource"]
    #[ivars = SourceIvars]
    pub struct Source;

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
                let id = self.ivars().model.borrow().order.get(usize::try_from(index).ok()?)?.clone();
                Some(self.item(&id))
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
                let node = self.ivars().model.borrow().nodes.get(&id)?.clone();
                Some(self.cell(&node))
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
                if let Some(id) = id_of(&item) {
                    self.open_id(&id);
                }
            });
        }

        #[unsafe(method(ideAgentPanelArchive:))]
        fn toggle_archive(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.with_panel(|panel| {
                    panel.show_archived.set(!panel.show_archived.get());
                    panel.reload();
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
                    self.with_panel(|panel| panel.reload());
                }
            });
        }
    }
);

fn id_of(item: &AnyObject) -> Option<String> {
    item.downcast_ref::<NSString>().map(|s| s.to_string())
}

/// The outline view of the list with its data source.
pub fn outline(mtm: MainThreadMarker) -> (Retained<NSOutlineView>, Retained<Source>) {
    let ivars = SourceIvars {
        panel: Cell::new(0),
        model: RefCell::new(Model::default()),
        items: RefCell::new(HashMap::new()),
        clicked: RefCell::new(None),
    };
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let source: Retained<Source> = unsafe { msg_send![super(Source::alloc(mtm).set_ivars(ivars)), init] };
    let view: Retained<NSOutlineView> = Retained::into_super(crate::window::file_outline::FileOutline::with_leading(
        mtm,
        objc2_foundation::NSRect::ZERO,
        ROW_LEADING,
    ));
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("row"));
    view.addTableColumn(&column);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { view.setOutlineTableColumn(Some(&column)) };
    view.setHeaderView(None);
    view.setStyle(NSTableViewStyle::SourceList);
    view.setRowHeight(22.0);
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
    pub fn set_panel(&self, key: usize) {
        self.ivars().panel.set(key);
    }

    fn with_panel(&self, f: impl FnOnce(&super::AgentPanel)) {
        if let Some(panel) = super::panel_by_key(self.ivars().panel.get()) {
            f(&panel);
        }
    }

    /// The one object that stands for row `id` while the panel lives; the view keeps no reference of its own.
    fn item(&self, id: &str) -> Retained<NSString> {
        self.ivars()
            .items
            .borrow_mut()
            .entry(id.to_string())
            .or_insert_with(|| ns(id))
            .clone()
    }

    pub fn show(&self, view: &NSOutlineView, model: Model) {
        *self.ivars().model.borrow_mut() = model;
        view.reloadData();
        view.sizeLastColumnToFit();
    }

    /// The row of `node`: whether it is open, its title and how long ago it was used.
    fn cell(&self, node: &Node) -> Retained<NSView> {
        let open = node.open();
        let (title, dim, when, badge) = match node {
            Node::Thread { entry, bookmarked, .. } => (
                if entry.title.is_empty() {
                    t("Neue Sitzung")
                } else {
                    entry.title.as_str()
                },
                entry.archived,
                entry.last_activity,
                bookmarked.then(|| ("bookmark.fill", NSColor::controlAccentColor())),
            ),
            Node::Past { resume, when } => (resume.title.as_deref().unwrap_or(&resume.session), false, *when, None),
            Node::Cli(thread) => (thread.name.as_str(), false, thread.started, None),
        };
        rows::cell(
            self.mtm(),
            &Look {
                open,
                title,
                dim,
                detail: &crate::agent::age(when),
                badge,
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
                if let Some(tab) = crate::agent::of_window(&controller).into_iter().find(|t| t.key == key) {
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
                };
                crate::agent::open_agent(&controller, crate::agent::Agent::of_thread(&entry.agent), Some(resume));
            }
            Node::Past { resume, .. } => {
                crate::agent::open_tab(&controller, Some(resume));
            }
            Node::Cli(thread) => crate::terminal::show_cli_thread(&controller, thread.key),
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
        menu.removeAllItems();
        menu.setAutoenablesItems(false);
        let Some(panel) = super::panel_by_key(self.ivars().panel.get()) else {
            return;
        };
        let row = panel.sessions.clickedRow();
        let id = panel.sessions.itemAtRow(row).and_then(|item| id_of(&item));
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
            .and_then(|c| crate::agent::of_window(&c).into_iter().find(|t| t.thread_id() == &id));
        let current = match &open {
            Some(tab) => tab.title(),
            None => ThreadIndex::open(&ThreadIndex::default_dir())
                .get(&id)
                .map(|e| e.title.clone())
                .unwrap_or_default(),
        };
        let Some(name) = crate::agent::ask_text(self.mtm(), t("Sitzung umbenennen"), &current) else {
            return;
        };
        match open {
            Some(tab) => tab.set_title(&name),
            None => {
                let mut index = ThreadIndex::open(&ThreadIndex::default_dir());
                let _ = index.rename(&id, &name);
            }
        }
        self.with_panel(|panel| panel.reload());
    }

    fn archive_clicked(&self) {
        let Some(thread) = self.clicked_thread() else { return };
        let id = ide_thread::ThreadId::from(thread.as_str());
        let mut index = ThreadIndex::open(&ThreadIndex::default_dir());
        let archived = index.get(&id).is_some_and(|e| e.archived);
        if let Err(error) = index.set_archived(&id, !archived) {
            crate::diag::report("archiving a session", error);
        }
        self.with_panel(|panel| panel.reload());
    }

    /// The quiet texts at the end of the session rows, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn session_details(&self) -> Vec<String> {
        let model = self.ivars().model.borrow();
        model
            .nodes
            .values()
            .filter_map(|node| match node {
                Node::Thread { entry, .. } => Some(crate::agent::age(entry.last_activity)),
                _ => None,
            })
            .collect()
    }

    /// The rows top to bottom, for the self-test: "AI" or "CLI", whether open, and the title.
    #[cfg(feature = "selftest")]
    pub fn rows(&self) -> Vec<(&'static str, bool, String)> {
        let model = self.ivars().model.borrow();
        model
            .order
            .iter()
            .filter_map(|id| model.nodes.get(id))
            .map(|node| match node {
                Node::Thread { entry, .. } => ("AI", node.open(), entry.title.clone()),
                Node::Past { resume, .. } => ("CLI", false, resume.title.clone().unwrap_or_default()),
                Node::Cli(thread) => ("CLI", true, thread.name.clone()),
            })
            .collect()
    }
}
