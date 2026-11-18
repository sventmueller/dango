//! The Containers panel's view and controller (E136): the tree of Compose projects, containers, images and volumes of
//! the window's Docker context, read again whenever `docker events` reports a change; an empty state without `docker`,
//! a note while the daemon does not run, retried with a growing wait; actions from a context menu and the Containers
//! menu, deleting only after a sheet whose destructive button is never the default.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::explain::Explain;
use ide_docker::board::{Board, Look, Node, Row, Section};
use ide_docker::{Action, Compose};
use ide_ui_common::panel::{Nesting, PanelOutline, RowItem, RowItems};
use ide_ui_common::registry::WindowPart;
use ide_ui_common::ui_size;
use ide_ui_common::window::{Window, WindowId};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSPopUpButton, NSStackView, NSStackViewGravity, NSTableColumn, NSTextField, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};
use splat_i18n::{t, tf};

use super::docker;
use crate::util::{FlagGuard, guarded, label, main_after, main_soon, ns, set_action};

/// How long a read waits for the login shell's environment, which the app reads once at launch.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// How long changes gather before the panel reads Docker again.
const SETTLE: Duration = Duration::from_millis(400);

/// Whether Docker can be used, as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reach {
    /// Not looked yet.
    Unknown,
    /// No `docker` on the login shell's PATH.
    Missing,
    /// The daemon does not answer; the reason.
    NotRunning(String),
    /// The server is older than 24.0.6, whose events are no JSON (A10).
    TooOld(String),
    /// It answers.
    Ready,
}

pub(crate) struct PanelIvars {
    id: u64,
    window: WindowId,
    root: PathBuf,
    context: RefCell<String>,
    contexts: RefCell<Vec<String>>,
    pub(super) board: RefCell<Board>,
    /// Every row of the tree, a section or project before its rows.
    rows: RefCell<Vec<Row>>,
    /// The object that stands for each row in the outline, by its node, the same one for as long as the row shows.
    items: RowItems<Node>,
    /// The sections and projects the user closed; all others show open.
    closed: RefCell<HashSet<Node>>,
    /// Set while the panel opens rows itself, so that does not count as the user's choice.
    unfolding: Cell<bool>,
    pub(super) reach: RefCell<Reach>,
    table: RefCell<Option<Retained<NSOutlineView>>>,
    note: RefCell<Option<Retained<NSTextField>>>,
    /// The row that keeps the note off the sidebar's edges, hidden while there is no note.
    note_row: RefCell<Option<Retained<NSStackView>>>,
    picker: RefCell<Option<Retained<NSPopUpButton>>>,
    events: Cell<Option<ide_process::ProcessId>>,
    /// Counts the starts of `docker events`, so the end of one stopped meanwhile is told from the end of the one now.
    watches: Cell<u64>,
    failures: Cell<u32>,
    asked: Cell<u64>,
    started: Cell<bool>,
    /// A retry came while the panel was hidden and waits for it to show.
    waiting: Cell<bool>,
    /// The PATH the self-test gives in place of the login shell's, to a stand-in `docker` or none.
    #[cfg(feature = "selftest")]
    path: RefCell<Option<String>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `ContainersPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEContainersPanel"]
    #[ivars = PanelIvars]
    pub(crate) struct ContainersPanel;

    unsafe impl NSObjectProtocol for ContainersPanel {}

    unsafe impl NSControlTextEditingDelegate for ContainersPanel {}

    unsafe impl NSOutlineViewDataSource for ContainersPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.children(item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let at = *self.children(item).get(usize::try_from(index).ok()?)?;
                let row = self.ivars().rows.borrow().get(at)?.clone();
                Some(self.item_for(&row))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || !self.children(Some(item)).is_empty())
        }
    }

    unsafe impl NSOutlineViewDelegate for ContainersPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(&self, _view: &NSOutlineView, _column: Option<&NSTableColumn>, item: &AnyObject) -> Option<Retained<NSView>> {
            guarded(None, || {
                let at = self.index_of(item)?;
                let row = self.ivars().rows.borrow().get(at)?.clone();
                Some(row_view(self.mtm(), &row))
            })
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

    unsafe impl NSMenuDelegate for ContainersPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || super::fill_menu(self, menu));
        }
    }

    impl ContainersPanel {
        #[unsafe(method(ideContainersRefresh:))]
        fn refresh_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.refresh());
        }

        #[unsafe(method(ideContainersContext:))]
        fn context_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let chosen = sender
                    .and_then(|s| s.downcast_ref::<NSPopUpButton>())
                    .and_then(|p| p.titleOfSelectedItem())
                    .map(|t| t.to_string());
                if let Some(chosen) = chosen {
                    *self.ivars().context.borrow_mut() = chosen;
                    self.stop_watching();
                    self.refresh();
                }
            });
        }
    }
);

impl ContainersPanel {
    /// The panel `id` of the window `window` for the folder `root`.
    pub(crate) fn new(mtm: MainThreadMarker, id: u64, window: WindowId, root: PathBuf) -> Retained<ContainersPanel> {
        let this = Self::alloc(mtm).set_ivars(PanelIvars {
            id,
            window,
            root,
            context: RefCell::new(String::new()),
            contexts: RefCell::new(Vec::new()),
            board: RefCell::new(Board::default()),
            rows: RefCell::new(Vec::new()),
            items: RowItems::default(),
            closed: RefCell::new(HashSet::new()),
            unfolding: Cell::new(false),
            reach: RefCell::new(Reach::Unknown),
            table: RefCell::new(None),
            note: RefCell::new(None),
            note_row: RefCell::new(None),
            picker: RefCell::new(None),
            events: Cell::new(None),
            watches: Cell::new(0),
            failures: Cell::new(0),
            asked: Cell::new(0),
            started: Cell::new(false),
            waiting: Cell::new(false),
            #[cfg(feature = "selftest")]
            path: RefCell::new(None),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn id(&self) -> u64 {
        self.ivars().id
    }

    pub(crate) fn window(&self) -> WindowId {
        self.ivars().window
    }

    pub(crate) fn root(&self) -> &std::path::Path {
        &self.ivars().root
    }

    pub(crate) fn context(&self) -> String {
        self.ivars().context.borrow().clone()
    }

    /// The page and its header.
    pub(crate) fn build(&self) -> (Retained<NSView>, Retained<NSStackView>) {
        let mtm = self.mtm();
        let me: &AnyObject = self.as_ref();
        let header = ide_ui_common::panel::header_row(mtm);
        let picker = NSPopUpButton::new(mtm);
        ide_ui_common::ui_size::follow_control_size(&picker, objc2_app_kit::NSControlSize::Small);
        if let Some(font) = picker.font() {
            ui_size::set_font(&picker, &font);
        }
        set_action(&picker, me, sel!(ideContainersContext:));
        picker.setToolTip(Some(&ns(t("Docker-Kontext dieses Fensters"))));
        ide_ui_common::util::set_label(&picker, Some(t("Docker-Kontext dieses Fensters")));
        let refresh = ide_ui_common::panel::header_button(
            mtm,
            "arrow.clockwise",
            t("Aktualisieren"),
            Some(me),
            sel!(ideContainersRefresh:),
        );
        header.addArrangedSubview(&picker);
        header.addArrangedSubview(&refresh);
        let table: Retained<NSOutlineView> = PanelOutline::new(mtm, Nesting::UnderTitle).into_super();
        let scroll = ide_ui_common::panel::source_list_outline(mtm, &table, "containers", t("Container"));
        // SAFETY: The table holds its data source and delegate weakly; the panel registry keeps the panel alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        // SAFETY: The table keeps the menu; its delegate, the panel, outlives the table through the registry.
        unsafe { table.setMenu(Some(&menu)) };
        let note = NSTextField::wrappingLabelWithString(&ns(""), mtm);
        ui_size::set_system_font(&note, ide_ui_common::style::BODY);
        note.setTextColor(Some(&NSColor::secondaryLabelColor()));
        note.setContentCompressionResistancePriority_forOrientation(
            objc2_app_kit::NSLayoutPriorityDefaultLow,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        let page = ide_ui_common::panel::panel_page(mtm);
        let note_row = ide_ui_common::panel::padded(mtm, &note);
        page.addView_inGravity(&note_row, NSStackViewGravity::Top);
        page.addView_inGravity(&scroll, NSStackViewGravity::Top);
        *self.ivars().table.borrow_mut() = Some(table);
        *self.ivars().note.borrow_mut() = Some(note);
        *self.ivars().note_row.borrow_mut() = Some(note_row);
        *self.ivars().picker.borrow_mut() = Some(picker);
        (Retained::into_super(page), header)
    }

    /// The panel shows: it reads Docker the first time, and again if a retry waited for it.
    pub(crate) fn start(&self) {
        let first = !self.ivars().started.replace(true);
        if first || self.ivars().waiting.take() {
            self.refresh();
        }
    }

    /// Reads Docker once while the panel is hidden, so it opens with its list; watching starts only when it shows.
    pub(crate) fn preload(&self) {
        if !self.ivars().started.get() && self.may_preload() {
            self.refresh();
        }
    }

    /// Whether its window shows the panel now.
    fn shown_now(&self) -> bool {
        crate::context::AppContext::get(self.mtm())
            .windows()
            .get(self.window().get())
            .is_some_and(|c| c.shown_panel() == Some(crate::window::Panel::Containers))
    }

    /// Reads Docker again on a worker thread: contexts, version, containers, images and volumes; starts watching its
    /// events when it answers.
    pub(crate) fn refresh(&self) {
        let context = self.context();
        let id = self.id();
        let asked = self.ivars().asked.get() + 1;
        self.ivars().asked.set(asked);
        let root = self.root().to_path_buf();
        let path = self.path();
        let spawned = crate::util::spawn("docker-read", move || {
            ide_process::priority::set_qos(ide_process::priority::Qos::Utility);
            let env = environment(path, ENVIRONMENT_WAIT);
            let read = read_all(&env, &context, &root);
            main_soon(move || {
                super::with_panel(id, |panel| panel.read_done(asked, read));
            });
        });
        if let Err(error) = spawned {
            crate::diag::report("reading Docker", error);
        }
    }

    fn read_done(&self, asked: u64, read: Read) {
        if self.ivars().asked.get() != asked {
            return;
        }
        if !read.contexts.is_empty() {
            *self.ivars().contexts.borrow_mut() = read.contexts.iter().map(|c| c.name.clone()).collect();
            if self.ivars().context.borrow().is_empty() {
                let current = read.contexts.iter().find(|c| c.current).or(read.contexts.first());
                *self.ivars().context.borrow_mut() = current.map(|c| c.name.clone()).unwrap_or_default();
            }
        }
        *self.ivars().reach.borrow_mut() = read.reach.clone();
        self.ivars().board.borrow_mut().files = read.files;
        if let Some(board) = read.board {
            let mut mine = self.ivars().board.borrow_mut();
            mine.containers = board.containers;
            mine.images = board.images;
            mine.volumes = board.volumes;
        }
        let started = self.ivars().started.get();
        match read.reach {
            Reach::Ready => {
                self.ivars().failures.set(0);
                if started {
                    self.watch();
                }
            }
            Reach::NotRunning(_) if started => self.retry_later(),
            Reach::NotRunning(_) | Reach::Unknown | Reach::Missing | Reach::TooOld(_) => {}
        }
        self.show();
    }

    /// Reads Docker again after a wait that grows with each failure in a row.
    fn retry_later(&self) {
        let attempt = self.ivars().failures.get();
        self.ivars().failures.set(attempt + 1);
        let asked = self.ivars().asked.get();
        let id = self.id();
        main_after(ide_docker::backoff(attempt), move || {
            super::with_panel(id, |panel| {
                if panel.ivars().asked.get() != asked {
                    return;
                }
                if panel.shown_now() {
                    panel.refresh();
                } else {
                    panel.ivars().waiting.set(true);
                }
            });
        });
    }

    /// Starts `docker events` for the context, unless it runs; a change reads Docker again once things settle, an end
    /// with an error tries again after a growing wait.
    fn watch(&self) {
        if self.ivars().events.get().is_some() {
            return;
        }
        let env = environment(self.path(), Duration::ZERO);
        let id = self.id();
        let watch = self.ivars().watches.get() + 1;
        self.ivars().watches.set(watch);
        let started = docker::watch(
            &ide_docker::watch_events(&self.context()),
            &env,
            move |line| {
                if ide_docker::event(line).is_some_and(|e| e.changes_listing()) {
                    main_soon(move || {
                        super::with_panel(id, ContainersPanel::changed);
                    });
                }
            },
            move |error| {
                main_soon(move || {
                    super::with_panel(id, |panel| panel.watch_ended(watch, error));
                });
            },
        );
        match started {
            Ok(id) => self.ivars().events.set(Some(id)),
            Err(error) => {
                *self.ivars().reach.borrow_mut() = Reach::NotRunning(error.explain());
                self.retry_later();
            }
        }
    }

    fn changed(&self) {
        let asked = self.ivars().asked.get();
        let id = self.id();
        main_after(SETTLE, move || {
            super::with_panel(id, |panel| {
                if panel.ivars().asked.get() == asked {
                    panel.refresh();
                }
            });
        });
    }

    fn watch_ended(&self, watch: u64, error: String) {
        if watch != self.ivars().watches.get() || self.ivars().events.take().is_none() {
            return;
        }
        *self.ivars().reach.borrow_mut() = Reach::NotRunning(error);
        self.show();
        self.retry_later();
    }

    #[cfg(not(feature = "selftest"))]
    #[expect(clippy::unused_self, reason = "the self-test's build reads the panel's PATH")]
    fn path(&self) -> Option<String> {
        None
    }

    #[cfg(not(feature = "selftest"))]
    #[expect(clippy::unused_self, reason = "the self-test needs a stand-in docker")]
    fn may_preload(&self) -> bool {
        true
    }

    pub(crate) fn stop_watching(&self) {
        if let Some(id) = self.ivars().events.take() {
            ide_process::Supervisor::global().stop(id, Duration::from_secs(1));
        }
    }

    /// Shows the tree, or the empty state and the note.
    pub(crate) fn show(&self) {
        let reach = self.ivars().reach.borrow().clone();
        let rows = if reach == Reach::Ready || matches!(reach, Reach::NotRunning(_)) {
            self.ivars().board.borrow().rows()
        } else {
            Vec::new()
        };
        let selected = self.selected_node();
        *self.ivars().rows.borrow_mut() = rows;
        let note = match &reach {
            Reach::Unknown => String::new(),
            Reach::Missing => t("Docker ist nicht installiert. Installiere Docker Desktop oder die docker-Befehlszeile, dann zeigt dieses Panel Container, Images und Volumes.").to_string(),
            Reach::NotRunning(_) => t("Docker läuft nicht. Starte Docker; das Panel versucht es von selbst wieder.").to_string(),
            Reach::TooOld(version) => tf("Docker {} ist zu alt; das Panel braucht 24.0.6 oder neuer.", &[version]),
            Reach::Ready => String::new(),
        };
        if let Some(label) = self.ivars().note.borrow().as_ref() {
            label.setStringValue(&ns(&note));
        }
        if let Some(row) = self.ivars().note_row.borrow().as_ref() {
            row.setHidden(note.is_empty());
        }
        if let Some(picker) = self.ivars().picker.borrow().as_ref() {
            picker.removeAllItems();
            for name in self.ivars().contexts.borrow().iter() {
                picker.addItemWithTitle(&ns(name));
            }
            picker.selectItemWithTitle(&ns(&self.context()));
            picker.setHidden(self.ivars().contexts.borrow().is_empty());
        }
        let Some(table) = self.ivars().table.borrow().clone() else {
            return;
        };
        table.reloadData();
        self.unfold(&table);
        let again = selected.and_then(|node| self.ivars().rows.borrow().iter().find(|r| r.node == node).cloned());
        self.select_row(&table, again.as_ref());
    }

    /// Opens every section and project the user did not close, and forgets the items of rows that went away.
    fn unfold(&self, table: &NSOutlineView) {
        let rows = self.ivars().rows.borrow().clone();
        {
            let _unfolding = FlagGuard::raise(&self.ivars().unfolding);
            for row in &rows {
                if !self.ivars().closed.borrow().contains(&row.node) {
                    // SAFETY: The item is one the data source handed the outline for this row.
                    unsafe { table.expandItem(Some(&self.item_for(row))) };
                }
            }
        }
        let live: HashSet<&Node> = rows.iter().map(|row| &row.node).collect();
        self.ivars().items.keep(|node| live.contains(node));
    }

    /// Selects the outline's row of `row`, or none.
    fn select_row(&self, table: &NSOutlineView, row: Option<&Row>) {
        // SAFETY: The item is one the data source handed the outline for this row.
        let index = row.map_or(-1, |row| unsafe { table.rowForItem(Some(&self.item_for(row))) });
        match usize::try_from(index) {
            Ok(index) => {
                table.selectRowIndexes_byExtendingSelection(
                    &objc2_foundation::NSIndexSet::indexSetWithIndex(index),
                    false,
                );
            }
            Err(_) => table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::new(), false),
        }
    }

    /// The rows one level under the row `item` stands for, or the sections for none, as indices into `rows`.
    fn children(&self, item: Option<&AnyObject>) -> Vec<usize> {
        let rows = self.ivars().rows.borrow();
        let Some(item) = item else {
            return (0..rows.len()).filter(|&i| rows[i].level == 0).collect();
        };
        let Some(parent) = self.index_of(item) else {
            return Vec::new();
        };
        let level = rows[parent].level;
        rows.iter()
            .enumerate()
            .skip(parent + 1)
            .take_while(|(_, row)| row.level > level)
            .filter(|(_, row)| row.level == level + 1)
            .map(|(i, _)| i)
            .collect()
    }

    /// The index in `rows` of the row `item` stands for.
    fn index_of(&self, item: &AnyObject) -> Option<usize> {
        let node = RowItems::<Node>::key_of(item)?;
        self.ivars().rows.borrow().iter().position(|row| row.node == node)
    }

    fn item_for(&self, row: &Row) -> Retained<RowItem> {
        self.ivars().items.item(self.mtm(), &row.node)
    }

    /// Notes that the user opened or closed a section or project, which stays so when Docker is read again.
    fn note_expansion(&self, notification: &NSNotification, open: bool) {
        if self.ivars().unfolding.get() {
            return;
        }
        let Some(at) = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")))
            .and_then(|item| self.index_of(&item))
        else {
            return;
        };
        let key = self.ivars().rows.borrow()[at].node.clone();
        let mut closed = self.ivars().closed.borrow_mut();
        if open {
            closed.remove(&key);
        } else {
            closed.insert(key);
        }
    }

    /// The row at `row` of the outline.
    fn row_at(&self, table: &NSOutlineView, row: isize) -> Option<Row> {
        let item = table.itemAtRow(row)?;
        let at = self.index_of(&item)?;
        self.ivars().rows.borrow().get(at).cloned()
    }

    /// What the selected row stands for, which stays selected when the rows are read again and shift.
    fn selected_node(&self) -> Option<Node> {
        let table = self.ivars().table.borrow().clone()?;
        self.row_at(&table, table.selectedRow()).map(|r| r.node)
    }

    /// The row the menu or the selection is about.
    pub(crate) fn chosen(&self) -> Option<Row> {
        let table = self.ivars().table.borrow().clone()?;
        let row = table.clickedRow();
        let row = if row < 0 { table.selectedRow() } else { row };
        self.row_at(&table, row)
    }

    /// Runs `command` and reads Docker again after it; an error shows in an alert.
    pub(crate) fn run(&self, command: ide_docker::Command) {
        let id = self.id();
        let path = self.path();
        let spawned = crate::util::spawn("docker-action", move || {
            let env = environment(path, ENVIRONMENT_WAIT);
            let outcome = docker::run(&command, &env);
            main_soon(move || {
                if let Err(error) = outcome
                    && let Some(mtm) = MainThreadMarker::new()
                {
                    ide_ui_common::alert::warn(mtm, t("Docker-Befehl fehlgeschlagen"), &error.explain());
                }
                super::with_panel(id, ContainersPanel::refresh);
            });
        });
        if let Err(error) = spawned {
            crate::diag::report("running docker", error);
        }
    }

    /// Does `action` to container `id`.
    pub(crate) fn act(&self, action: Action, id: &str) {
        self.run(ide_docker::container(&self.context(), action, id));
    }

    /// Does `action` to the Compose project of `file`.
    pub(crate) fn compose(&self, action: Compose, file: &std::path::Path) {
        self.run(ide_docker::compose(&self.context(), action, file));
    }
}

impl WindowPart for ContainersPanel {
    fn window(&self) -> Option<Window> {
        Window::open(self.ivars().window)
    }

    fn root(&self) -> &Path {
        ContainersPanel::root(self)
    }
}

/// The login shell's environment, waiting up to `wait` for it, with `path` as its PATH if given.
fn environment(path: Option<String>, wait: Duration) -> Vec<(String, String)> {
    let mut env = ide_ui_common::window::environment(wait);
    if let Some(path) = path {
        env.retain(|(k, _)| k != "PATH");
        env.push(("PATH".into(), path));
    }
    env
}

/// What a read of Docker found.
struct Read {
    contexts: Vec<ide_docker::Context>,
    reach: Reach,
    board: Option<Board>,
    /// The Compose files of the folder, whatever Docker answered.
    files: Vec<PathBuf>,
}

fn read_all(env: &[(String, String)], context: &str, root: &std::path::Path) -> Read {
    let files = ide_docker::compose_files(root);
    if docker::program(env).is_none() {
        return Read {
            contexts: Vec::new(),
            reach: Reach::Missing,
            board: None,
            files,
        };
    }
    let contexts = docker::run(&ide_docker::list_contexts(), env)
        .map(|out| ide_docker::contexts(&out))
        .unwrap_or_default();
    let context = if context.is_empty() {
        contexts
            .iter()
            .find(|c| c.current)
            .or(contexts.first())
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "default".into())
    } else {
        context.to_string()
    };
    let version = match docker::run(&ide_docker::server_version(&context), env) {
        Ok(version) => version.trim().to_string(),
        Err(error) => {
            return Read {
                contexts,
                reach: Reach::NotRunning(error.explain()),
                board: None,
                files,
            };
        }
    };
    if !ide_docker::supported(&version) {
        return Read {
            contexts,
            reach: Reach::TooOld(version),
            board: None,
            files,
        };
    }
    let list = |command: ide_docker::Command| docker::run(&command, env).unwrap_or_default();
    let board = Board {
        containers: ide_docker::containers(&list(ide_docker::list_containers(&context))),
        images: ide_docker::images(&list(ide_docker::list_images(&context))),
        volumes: ide_docker::volumes(&list(ide_docker::list_volumes(&context))),
        files: files.clone(),
    };
    Read {
        contexts,
        reach: Reach::Ready,
        board: Some(board),
        files,
    }
}

/// The symbol, its description and colour for a row; the shapes differ, so the state reads without colour (E101).
fn look_symbol(row: &Row) -> (&'static str, String, Retained<NSColor>) {
    match (&row.node, row.look) {
        (Node::Section(_), _) => ("", String::new(), NSColor::secondaryLabelColor()),
        (Node::Image(_), _) => (
            "square.stack.3d.up",
            t("Docker-Image").to_string(),
            NSColor::secondaryLabelColor(),
        ),
        (Node::Volume(_), _) => (
            "externaldrive",
            t("Docker-Volume").to_string(),
            NSColor::secondaryLabelColor(),
        ),
        (_, Look::Running) => ("play.circle.fill", t("Läuft").to_string(), NSColor::systemGreenColor()),
        (_, Look::Partly) => (
            "circle.lefthalf.filled",
            t("Teilweise").to_string(),
            NSColor::systemOrangeColor(),
        ),
        (_, Look::Stopped | Look::Plain) => ("stop.circle", t("Gestoppt").to_string(), NSColor::secondaryLabelColor()),
    }
}

fn section_title(section: Section) -> &'static str {
    match section {
        Section::Compose => t("Compose"),
        Section::Containers => t("Container"),
        Section::Images => t("Images"),
        Section::Volumes => t("Volumes"),
    }
}

fn row_view(mtm: MainThreadMarker, row: &Row) -> Retained<NSView> {
    if let Node::Section(section) = &row.node {
        return ide_ui_common::panel::section_cell(mtm, section_title(*section)).into_super();
    }
    let (name, description, color) = look_symbol(row);
    let spoken = if description.is_empty() {
        format!("{}, {}", row.label, row.detail)
    } else {
        format!("{}, {description}, {}", row.label, row.detail)
    };
    let (stack, text) = ide_ui_common::panel::icon_row(mtm, (name, &description, &color), &row.label, 0, &spoken);
    text.setContentCompressionResistancePriority_forOrientation(
        objc2_app_kit::NSLayoutPriorityDefaultLow,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    let detail = label(mtm, &row.detail);
    detail.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
    detail.setContentCompressionResistancePriority_forOrientation(
        objc2_app_kit::NSLayoutPriorityDefaultLow - 1.0,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
    ui_size::set_system_font(&detail, ide_ui_common::style::SMALL);
    stack.addView_inGravity(&detail, NSStackViewGravity::Trailing);
    stack.into_super()
}

#[cfg(feature = "selftest")]
mod selftest_probe;
