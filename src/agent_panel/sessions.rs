//! The panel's two lists: the folder's sessions in groups, and the tree of what runs. One data source class serves
//! both; the sessions list also takes sessions dragged onto groups and has a context menu.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;

use ide_thread::{Filter, IndexEntry, Status, TaskStatus, ThreadIndex};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSControlTextEditingDelegate, NSDragOperation, NSDraggingInfo, NSMenu, NSMenuDelegate, NSOutlineView,
    NSOutlineViewDataSource, NSOutlineViewDelegate, NSPasteboardItem, NSPasteboardWriting, NSTableColumn,
    NSTableViewStyle, NSView,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSString};
use splat_i18n::{t, tf};

use super::rows::{self, Look};
use crate::agent::{AgentTab, Resume};
use crate::util::{guarded, item, ns, set_action, set_target};

/// The pasteboard type of a session dragged within the list.
const DRAG_TYPE: &str = "local.agentide.thread";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sessions,
    Running,
}

/// A row of either list.
/// How long ago a session was active, with its agent's name when it is not Claude.
fn agent_and_age(agent: &str, at: SystemTime) -> String {
    let age = crate::agent::age(at);
    if agent == ide_claude::AGENT {
        age
    } else {
        format!("{} · {age}", crate::agent::agent_name(agent))
    }
}

#[derive(Clone)]
pub enum Node {
    Group {
        name: String,
        /// A group the app makes, such as the sessions started in a terminal, which takes no drops.
        fixed: bool,
    },
    Thread {
        entry: IndexEntry,
        tab: Option<u64>,
        status: Option<Status>,
        unseen: bool,
        bookmarked: bool,
    },
    Past {
        resume: Resume,
        when: SystemTime,
    },
    Tab {
        key: u64,
        title: String,
        status: Status,
    },
    /// A CLI thread (I5): a coding agent's CLI in a terminal tab of the window.
    Cli(crate::terminal::CliThread),
    Task {
        tab: u64,
        /// The subagent's key, for revealing its card.
        reveal: Option<String>,
        title: String,
        detail: String,
        symbol: &'static str,
    },
}

/// The rows of a list by id, and the ids under each parent; `""` is the top.
#[derive(Default)]
pub struct Model {
    nodes: HashMap<String, Node>,
    children: HashMap<String, Vec<String>>,
}

/// What the sessions list is built from.
pub struct Inputs<'a> {
    pub index: &'a ThreadIndex,
    pub root: &'a Path,
    pub tabs: &'a [Rc<AgentTab>],
    pub query: &'a str,
    pub archived: bool,
    pub groups: &'a BTreeSet<String>,
    pub terminal: Vec<ide_claude::history::PastSession>,
    /// The window's CLI threads, which run in its terminal tabs.
    pub cli: Vec<crate::terminal::CliThread>,
    pub bookmarks: &'a BTreeSet<String>,
}

impl Model {
    fn add(&mut self, parent: &str, id: String, node: Node) {
        self.children.entry(parent.to_string()).or_default().push(id.clone());
        self.nodes.insert(id, node);
    }

    /// The group of what started in a terminal: the window's CLI threads, then the CLI's sessions saved from a
    /// terminal that the index does not know.
    fn add_terminal(&mut self, inputs: &Inputs<'_>) {
        let known: HashSet<&str> = inputs
            .index
            .list(&Filter::default())
            .iter()
            .filter_map(|e| e.session.as_deref())
            .collect();
        let past: Vec<&ide_claude::history::PastSession> = inputs
            .terminal
            .iter()
            .filter(|s| !known.contains(s.id.as_str()))
            .filter(|s| {
                matches_query(
                    s.title.as_deref().or(s.first_prompt.as_deref()).unwrap_or_default(),
                    inputs.query,
                )
            })
            .take(30)
            .collect();
        if !past.is_empty() || !inputs.cli.is_empty() {
            let group = t("Im Terminal gestartet").to_string();
            self.add(
                "",
                "fixed:terminal".into(),
                Node::Group {
                    name: group,
                    fixed: true,
                },
            );
            for thread in inputs
                .cli
                .iter()
                .filter(|thread| matches_query(&thread.name, inputs.query))
            {
                self.add(
                    "fixed:terminal",
                    format!("cli:{}", thread.key),
                    Node::Cli(thread.clone()),
                );
            }
            for session in past {
                let resume = Resume {
                    session: session.id.clone(),
                    thread: None,
                    title: session.title.clone().or_else(|| session.first_prompt.clone()),
                    transcript: Some(session.path.clone()),
                };
                self.add(
                    "fixed:terminal",
                    format!("past:{}", session.id),
                    Node::Past {
                        resume,
                        when: session.modified,
                    },
                );
            }
        }
    }

    pub fn build(inputs: &Inputs<'_>) -> Model {
        let mut model = Model::default();
        let filter = Filter {
            archived: inputs.archived,
            group: None,
            query: inputs.query.to_string(),
        };
        let entries: Vec<&IndexEntry> = inputs
            .index
            .list(&filter)
            .into_iter()
            .filter(|e| e.cwd.as_deref() == Some(inputs.root))
            .collect();
        let tab_of = |id: &ide_thread::ThreadId| inputs.tabs.iter().find(|t| t.thread_id() == id);
        let mut groups: BTreeSet<String> = inputs.groups.clone();
        groups.extend(entries.iter().filter_map(|e| e.group.clone()));
        if !inputs.archived {
            for group in &groups {
                model.add(
                    "",
                    format!("group:{group}"),
                    Node::Group {
                        name: group.clone(),
                        fixed: false,
                    },
                );
            }
        }
        let shown: HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        if !inputs.archived {
            for tab in inputs.tabs.iter().filter(|t| !shown.contains(t.thread_id().as_str())) {
                let entry = IndexEntry::of(&tab.snapshot(), SystemTime::now());
                if matches_query(&tab.title(), inputs.query) {
                    let node = thread_node(entry, Some(tab), inputs.bookmarks);
                    model.add("", format!("thread:{}", tab.thread_id()), node);
                }
            }
        }
        let mut sorted = entries.clone();
        sorted.sort_by_key(|e| !inputs.bookmarks.contains(e.id.as_str()));
        for entry in sorted {
            let parent = match (&entry.group, inputs.archived) {
                (Some(group), false) => format!("group:{group}"),
                _ => String::new(),
            };
            let node = thread_node(entry.clone(), tab_of(&entry.id), inputs.bookmarks);
            model.add(&parent, format!("thread:{}", entry.id), node);
        }
        model.add_terminal(inputs);
        model
    }

    /// The tree of what runs: each session that works or waits, its running subagents and its background work.
    pub fn running(tabs: &[Rc<AgentTab>]) -> Model {
        let mut model = Model::default();
        for tab in tabs {
            let thread = tab.snapshot();
            let subagents: Vec<_> = thread
                .subagents()
                .iter()
                .filter(|s| s.status == TaskStatus::Running)
                .collect();
            // A subagent in the background has its own row above, from the subagents.
            let background: Vec<_> = thread
                .background()
                .iter()
                .filter(|b| !b.status.is_final() && b.kind != ide_thread::BackgroundKind::Subagent)
                .collect();
            if thread.status() == Status::Idle && subagents.is_empty() && background.is_empty() {
                continue;
            }
            let id = format!("tab:{}", tab.key);
            model.add(
                "",
                id.clone(),
                Node::Tab {
                    key: tab.key,
                    title: tab.title(),
                    status: thread.status(),
                },
            );
            for sub in subagents {
                let elapsed = SystemTime::now()
                    .duration_since(sub.started)
                    .unwrap_or_default()
                    .max(sub.elapsed);
                let mut detail = crate::agent::elapsed(elapsed);
                if sub.tokens > 0 {
                    detail.push_str(&format!(" · {}", tf("{} Tokens", &[&crate::agent::tokens(sub.tokens)])));
                }
                if let Some(tool) = &sub.last_tool {
                    detail.push_str(&format!(" · {tool}"));
                }
                model.add(
                    &id,
                    format!("sub:{}:{}", tab.key, sub.key),
                    Node::Task {
                        tab: tab.key,
                        reveal: Some(sub.key.clone()),
                        title: sub.description.clone(),
                        detail,
                        symbol: "person.2",
                    },
                );
            }
            for item in background {
                let detail = item
                    .output
                    .text
                    .as_deref()
                    .and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()))
                    .map(str::to_string)
                    .unwrap_or_else(|| crate::activity::task_status(item.status).to_string());
                model.add(
                    &id,
                    format!("bg:{}:{}", tab.key, item.id),
                    Node::Task {
                        tab: tab.key,
                        reveal: item.tool_call.clone(),
                        title: item.label.clone(),
                        detail,
                        symbol: crate::activity::background_symbol(&item.kind),
                    },
                );
            }
        }
        model
    }
}

fn matches_query(title: &str, query: &str) -> bool {
    let title = title.to_lowercase();
    query.split_whitespace().all(|w| title.contains(&w.to_lowercase()))
}

fn thread_node(entry: IndexEntry, tab: Option<&Rc<AgentTab>>, bookmarks: &BTreeSet<String>) -> Node {
    let mut entry = entry;
    if let Some(tab) = tab {
        entry.title = tab.title();
        let thread = tab.snapshot();
        entry.cost_usd = if thread.usage().subscription {
            None
        } else {
            thread.usage().cost_usd.or(entry.cost_usd)
        };
    }
    Node::Thread {
        bookmarked: bookmarks.contains(entry.id.as_str()),
        tab: tab.map(|t| t.key),
        status: tab.map(|t| t.status()),
        unseen: tab.is_some_and(|t| t.wants_attention()),
        entry,
    }
}

pub struct SourceIvars {
    kind: Kind,
    panel: Cell<usize>,
    model: RefCell<Model>,
    items: RefCell<HashMap<String, Retained<NSString>>>,
    collapsed: RefCell<HashSet<String>>,
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
            guarded(0, || isize::try_from(self.children(item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let id = self.children(item).get(usize::try_from(index).ok()?)?.clone();
                Some(self.item(&id))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || !self.children(Some(item)).is_empty())
        }

        #[unsafe(method_id(outlineView:pasteboardWriterForItem:))]
        fn writer(&self, _view: &NSOutlineView, item: &AnyObject) -> Option<Retained<ProtocolObject<dyn NSPasteboardWriting>>> {
            guarded(None, || {
                let id = id_of(item)?;
                let thread = id.strip_prefix("thread:")?;
                let pasteboard_item = NSPasteboardItem::new();
                pasteboard_item.setString_forType(&ns(thread), &ns(DRAG_TYPE));
                Some(ProtocolObject::from_retained(pasteboard_item))
            })
        }

        #[unsafe(method(outlineView:validateDrop:proposedItem:proposedChildIndex:))]
        fn validate_drop(
            &self,
            view: &NSOutlineView,
            _info: &ProtocolObject<dyn NSDraggingInfo>,
            item: Option<&AnyObject>,
            _index: isize,
        ) -> NSDragOperation {
            guarded(NSDragOperation::None, || {
                let target = item.and_then(id_of);
                let group = target.as_deref().and_then(|id| match self.ivars().model.borrow().nodes.get(id) {
                    Some(Node::Group { fixed: false, .. }) => Some(id.to_string()),
                    _ => None,
                });
                match (target, group) {
                    (None, _) => {
                        // SAFETY: Nil and -1 retarget the drop onto the list itself, which takes a session out of its
                        // group.
                        unsafe { view.setDropItem_dropChildIndex(None, -1) };
                        NSDragOperation::Move
                    }
                    (Some(_), Some(group)) => {
                        let item = self.item(&group);
                        // SAFETY: The item is one this data source handed to the view.
                        unsafe { view.setDropItem_dropChildIndex(Some(&item), -1) };
                        NSDragOperation::Move
                    }
                    _ => NSDragOperation::None,
                }
            })
        }

        #[unsafe(method(outlineView:acceptDrop:item:childIndex:))]
        fn accept_drop(
            &self,
            _view: &NSOutlineView,
            info: &ProtocolObject<dyn NSDraggingInfo>,
            item: Option<&AnyObject>,
            _index: isize,
        ) -> bool {
            guarded(false, || {
                let pasteboard = info.draggingPasteboard();
                let Some(thread) = pasteboard.stringForType(&ns(DRAG_TYPE)) else {
                    return false;
                };
                let group = item.and_then(id_of).and_then(|id| id.strip_prefix("group:").map(str::to_string));
                self.move_to_group(&thread.to_string(), group.as_deref());
                true
            })
        }
    }

    unsafe impl NSOutlineViewDelegate for Source {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(&self, _view: &NSOutlineView, _column: Option<&NSTableColumn>, item: &AnyObject) -> Option<Retained<NSView>> {
            guarded(None, || {
                let id = id_of(item)?;
                let node = self.ivars().model.borrow().nodes.get(&id)?.clone();
                Some(self.cell(&node, &id))
            })
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &objc2_foundation::NSNotification) {
            guarded((), || {
                if let Some(id) = notified(notification) {
                    self.ivars().collapsed.borrow_mut().insert(id);
                }
            });
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &objc2_foundation::NSNotification) {
            guarded((), || {
                if let Some(id) = notified(notification) {
                    self.ivars().collapsed.borrow_mut().remove(&id);
                }
            });
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

        #[unsafe(method(ideAgentPanelSearch:))]
        fn search(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_panel(|panel| panel.reload()));
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

        #[unsafe(method(ideAgentPanelMoveTo:))]
        fn move_to(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let group = sender
                    .and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>())
                    .and_then(|i| i.representedObject())
                    .and_then(|o| o.downcast::<NSString>().ok())
                    .map(|s| s.to_string());
                if let Some(thread) = self.clicked_thread() {
                    self.move_to_group(&thread, group.as_deref());
                }
            });
        }

        #[unsafe(method(ideAgentPanelNewGroup:))]
        fn new_group(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.make_group());
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

fn notified(notification: &objc2_foundation::NSNotification) -> Option<String> {
    let info = notification.userInfo()?;
    let item = info.objectForKey(&ns("NSObject"))?;
    id_of(&item)
}

/// The outline view of one list with its data source.
pub fn outline(mtm: MainThreadMarker, kind: Kind) -> (Retained<NSOutlineView>, Retained<Source>) {
    let ivars = SourceIvars {
        kind,
        panel: Cell::new(0),
        model: RefCell::new(Model::default()),
        items: RefCell::new(HashMap::new()),
        collapsed: RefCell::new(HashSet::new()),
        clicked: RefCell::new(None),
    };
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let source: Retained<Source> = unsafe { msg_send![super(Source::alloc(mtm).set_ivars(ivars)), init] };
    let view = NSOutlineView::new(mtm);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("row"));
    view.addTableColumn(&column);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { view.setOutlineTableColumn(Some(&column)) };
    view.setHeaderView(None);
    view.setStyle(NSTableViewStyle::SourceList);
    view.setIndentationPerLevel(10.0);
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
    if kind == Kind::Sessions {
        view.registerForDraggedTypes(&NSArray::from_retained_slice(&[ns(DRAG_TYPE)]));
        view.setDraggingSourceOperationMask_forLocal(NSDragOperation::Move, true);
        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        // SAFETY: The view keeps the menu; its delegate, the source, lives as long as the view through the panel.
        unsafe { view.setMenu(Some(&menu)) };
    }
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

    fn children(&self, item: Option<&AnyObject>) -> Vec<String> {
        let parent = item.and_then(id_of).unwrap_or_default();
        self.ivars()
            .model
            .borrow()
            .children
            .get(&parent)
            .cloned()
            .unwrap_or_default()
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

    /// Shows `model`, keeping groups open unless the user closed them.
    pub fn show(&self, view: &NSOutlineView, model: Model) {
        let parents: Vec<String> = model.children.keys().filter(|k| !k.is_empty()).cloned().collect();
        *self.ivars().model.borrow_mut() = model;
        view.reloadData();
        view.sizeLastColumnToFit();
        let collapsed = self.ivars().collapsed.borrow().clone();
        for parent in parents.iter().filter(|p| !collapsed.contains(*p)) {
            let item = self.item(parent);
            // SAFETY: The item is one this data source handed to the view.
            unsafe { view.expandItem(Some(&item)) };
        }
    }

    fn cell(&self, node: &Node, id: &str) -> Retained<NSView> {
        let mtm = self.mtm();
        match node {
            Node::Group { name, fixed } => {
                let count = self.ivars().model.borrow().children.get(id).map_or(0, Vec::len);
                let symbol = if *fixed { "terminal" } else { "folder" };
                rows::cell(
                    mtm,
                    &Look {
                        symbol: Some((symbol, 10.0, NSColor::secondaryLabelColor())),
                        title: name,
                        bold: true,
                        dim: true,
                        detail: &count.to_string(),
                        badge: None,
                    },
                )
            }
            Node::Thread {
                entry,
                status,
                unseen,
                bookmarked,
                ..
            } => {
                let dot = dot(*status, *unseen);
                let title = if entry.title.is_empty() {
                    t("Neue Sitzung")
                } else {
                    &entry.title
                };
                rows::cell(
                    mtm,
                    &Look {
                        symbol: Some(dot),
                        title,
                        bold: false,
                        dim: entry.archived,
                        detail: &crate::agent::with_cost(
                            agent_and_age(&entry.agent, entry.last_activity),
                            entry.cost_usd,
                        ),
                        badge: bookmarked.then(|| ("bookmark.fill", crate::agent::style::terracotta())),
                    },
                )
            }
            Node::Past { resume, when } => rows::cell(
                mtm,
                &Look {
                    symbol: Some(("terminal", 9.0, NSColor::tertiaryLabelColor())),
                    title: resume.title.as_deref().unwrap_or(&resume.session),
                    bold: false,
                    dim: true,
                    detail: &crate::agent::age(*when),
                    badge: None,
                },
            ),
            Node::Cli(thread) => rows::cell(
                mtm,
                &Look {
                    symbol: Some(("circle.fill", 7.0, thread.status.color())),
                    title: &thread.name,
                    bold: false,
                    dim: false,
                    detail: thread.status.text(),
                    badge: None,
                },
            ),
            Node::Tab { title, status, .. } => rows::cell(
                mtm,
                &Look {
                    symbol: Some(dot(Some(*status), false)),
                    title,
                    bold: false,
                    dim: false,
                    detail: crate::activity::session_status(*status),
                    badge: None,
                },
            ),
            Node::Task {
                title, detail, symbol, ..
            } => rows::cell(
                mtm,
                &Look {
                    symbol: Some((symbol, 10.0, NSColor::secondaryLabelColor())),
                    title,
                    bold: false,
                    dim: false,
                    detail,
                    badge: None,
                },
            ),
        }
    }

    fn open_id(&self, id: &str) {
        let Some(node) = self.ivars().model.borrow().nodes.get(id).cloned() else {
            return;
        };
        let Some(panel) = super::panel_by_key(self.ivars().panel.get()) else {
            return;
        };
        let Some(controller) = panel.controller() else { return };
        let tab = |key: u64| crate::agent::of_window(&controller).into_iter().find(|t| t.key == key);
        match node {
            Node::Thread { tab: Some(key), .. } | Node::Tab { key, .. } => {
                if let Some(tab) = tab(key) {
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
            Node::Task { tab: key, reveal, .. } => {
                if let Some(tab) = tab(key) {
                    tab.select();
                    if let Some(reveal) = reveal {
                        tab.timeline().reveal(&reveal);
                    }
                }
            }
            Node::Group { .. } => {}
        }
        let _ = self.ivars().kind;
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
        *self.ivars().clicked.borrow_mut() = id.clone();
        let mtm = self.mtm();
        let add = |title: &str, action: objc2::runtime::Sel, enabled: bool| {
            let entry = item(mtm, title, Some(action), "");
            set_target(&entry, self);
            entry.setEnabled(enabled);
            menu.addItem(&entry);
            entry
        };
        let thread = self.clicked_thread();
        if let Some(thread) = &thread {
            let archived = matches!(
                self.ivars().model.borrow().nodes.get(&format!("thread:{thread}")),
                Some(Node::Thread { entry, .. }) if entry.archived
            );
            add(t("Umbenennen …"), sel!(ideAgentPanelRename:), true);
            let marked = super::is_bookmarked(&ide_thread::ThreadId::from(thread.as_str()));
            add(
                if marked {
                    t("Lesezeichen entfernen")
                } else {
                    t("Lesezeichen setzen")
                },
                sel!(ideAgentPanelBookmark:),
                true,
            );
            let groups = self.group_menu(&panel);
            menu.addItem(&groups);
            add(
                if archived {
                    t("Aus dem Archiv holen")
                } else {
                    t("Archivieren")
                },
                sel!(ideAgentPanelArchiveItem:),
                true,
            );
            menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(mtm));
        }
        add(t("Neue Gruppe …"), sel!(ideAgentPanelNewGroup:), true);
    }

    fn group_menu(&self, panel: &super::AgentPanel) -> Retained<objc2_app_kit::NSMenuItem> {
        let mtm = self.mtm();
        let index = ThreadIndex::open(&ThreadIndex::default_dir());
        let mut groups: BTreeSet<String> = index.groups().into_iter().collect();
        groups.extend(panel.groups.borrow().iter().cloned());
        let menu = NSMenu::new(mtm);
        for group in &groups {
            let entry = item(mtm, group, Some(sel!(ideAgentPanelMoveTo:)), "");
            // SAFETY: A menu item can represent an object of any type.
            unsafe { entry.setRepresentedObject(Some(&ns(group))) };
            set_target(&entry, self);
            menu.addItem(&entry);
        }
        if !groups.is_empty() {
            menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(mtm));
        }
        let out = item(mtm, t("Ohne Gruppe"), Some(sel!(ideAgentPanelMoveTo:)), "");
        set_target(&out, self);
        menu.addItem(&out);
        let parent = item(mtm, t("In Gruppe verschieben"), None, "");
        parent.setSubmenu(Some(&menu));
        parent
    }

    fn move_to_group(&self, thread: &str, group: Option<&str>) {
        let id = ide_thread::ThreadId::from(thread);
        let mut index = ThreadIndex::open(&ThreadIndex::default_dir());
        if index.get(&id).is_none()
            && let Some(panel) = super::panel_by_key(self.ivars().panel.get())
            && let Some(controller) = panel.controller()
            && let Some(tab) = crate::agent::of_window(&controller)
                .into_iter()
                .find(|t| t.thread_id() == &id)
        {
            let _ = index.record(&tab.snapshot(), SystemTime::now());
        }
        if let Err(error) = index.set_group(&id, group) {
            crate::diag::report("moving a session into a group", error);
        }
        self.with_panel(|panel| {
            if let Some(group) = group {
                panel.groups.borrow_mut().insert(group.to_string());
            }
            panel.reload();
        });
    }

    fn make_group(&self) {
        let Some(name) = crate::agent::ask_text(self.mtm(), t("Neue Gruppe"), "") else {
            return;
        };
        self.with_panel(|panel| {
            panel.groups.borrow_mut().insert(name.clone());
            panel.reload();
        });
        if let Some(thread) = self.clicked_thread() {
            self.move_to_group(&thread, Some(&name));
        }
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
                Node::Thread { entry, .. } => Some(crate::agent::with_cost(
                    agent_and_age(&entry.agent, entry.last_activity),
                    entry.cost_usd,
                )),
                _ => None,
            })
            .collect()
    }

    /// The titles of the rows, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn titles(&self) -> Vec<String> {
        let model = self.ivars().model.borrow();
        let mut out = Vec::new();
        let mut stack: Vec<(String, usize)> = vec![(String::new(), 0)];
        while let Some((parent, depth)) = stack.pop() {
            for id in model.children.get(&parent).into_iter().flatten().rev() {
                stack.push((id.clone(), depth + 1));
            }
            if let Some(node) = model.nodes.get(&parent) {
                let title = match node {
                    Node::Group { name, .. } => name.clone(),
                    Node::Thread { entry, .. } if entry.agent != ide_claude::AGENT => {
                        format!("{} · {}", entry.title, crate::agent::agent_name(&entry.agent))
                    }
                    Node::Thread { entry, .. } => entry.title.clone(),
                    Node::Past { resume, .. } => resume.title.clone().unwrap_or_default(),
                    Node::Tab { title, .. } | Node::Task { title, .. } => title.clone(),
                    Node::Cli(thread) => thread.name.clone(),
                };
                out.push(format!("{}{title}", "  ".repeat(depth.saturating_sub(1))));
            }
        }
        out
    }
}

/// The status dot of a session row.
fn dot(status: Option<Status>, unseen: bool) -> (&'static str, f64, Retained<NSColor>) {
    match status {
        Some(Status::Running) => ("circle.fill", 7.0, crate::agent::style::terracotta()),
        Some(Status::RequiresAction) => ("circle.fill", 7.0, NSColor::systemOrangeColor()),
        Some(Status::Idle) if unseen => ("circle.fill", 7.0, NSColor::controlAccentColor()),
        Some(Status::Idle) => ("circle", 7.0, NSColor::tertiaryLabelColor()),
        None => ("circle", 7.0, NSColor::quaternaryLabelColor()),
    }
}
