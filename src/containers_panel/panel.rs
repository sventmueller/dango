//! The Containers panel's view and controller (E136): the tree of Compose projects, containers, images and volumes of
//! the window's Docker context, read again whenever `docker events` reports a change; an empty state without `docker`,
//! a note while the daemon does not run, retried with a growing wait; actions from a context menu and the Containers
//! menu, deleting only after a sheet whose destructive button is never the default.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Duration;

use ide_docker::board::{Board, Look, Node, Row, Section};
use ide_docker::{Action, Compose};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSControlTextEditingDelegate, NSImageView, NSMenu, NSMenuDelegate, NSPopUpButton,
    NSScrollView, NSStackView, NSStackViewGravity, NSTableColumn, NSTableView, NSTableViewDataSource,
    NSTableViewDelegate, NSTableViewStyle, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::docker;
use crate::util::{guarded, label, main_after, main_soon, ns, set_action, symbol};

/// How long a read waits for the login shell's environment, which the app reads once at launch.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// How long changes gather before the panel reads Docker again.
const SETTLE: Duration = Duration::from_millis(400);

/// Whether Docker can be used, as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
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

pub struct PanelIvars {
    window: u64,
    root: PathBuf,
    context: RefCell<String>,
    contexts: RefCell<Vec<String>>,
    pub(super) board: RefCell<Board>,
    rows: RefCell<Vec<Row>>,
    pub(super) reach: RefCell<Reach>,
    table: RefCell<Option<Retained<NSTableView>>>,
    note: RefCell<Option<Retained<NSTextField>>>,
    picker: RefCell<Option<Retained<NSPopUpButton>>>,
    events: Cell<Option<ide_process::ProcessId>>,
    /// Counts the starts of `docker events`, so the end of one stopped meanwhile is told from the end of the one now.
    watches: Cell<u64>,
    failures: Cell<u32>,
    asked: Cell<u64>,
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
    pub struct ContainersPanel;

    unsafe impl NSObjectProtocol for ContainersPanel {}

    unsafe impl NSControlTextEditingDelegate for ContainersPanel {}

    unsafe impl NSTableViewDataSource for ContainersPanel {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().rows.borrow().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for ContainersPanel {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let rows = self.ivars().rows.borrow();
                Some(row_view(self.mtm(), rows.get(usize::try_from(row).ok()?)?))
            })
        }

        #[unsafe(method(tableView:isGroupRow:))]
        fn is_group_row(&self, _table: &NSTableView, row: isize) -> bool {
            guarded(false, || {
                let rows = self.ivars().rows.borrow();
                usize::try_from(row).ok().and_then(|r| rows.get(r)).is_some_and(|r| r.level == 0)
            })
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
    pub fn new(mtm: MainThreadMarker, window: u64, root: PathBuf) -> Retained<ContainersPanel> {
        let this = Self::alloc(mtm).set_ivars(PanelIvars {
            window,
            root,
            context: RefCell::new(String::new()),
            contexts: RefCell::new(Vec::new()),
            board: RefCell::new(Board::default()),
            rows: RefCell::new(Vec::new()),
            reach: RefCell::new(Reach::Unknown),
            table: RefCell::new(None),
            note: RefCell::new(None),
            picker: RefCell::new(None),
            events: Cell::new(None),
            watches: Cell::new(0),
            failures: Cell::new(0),
            asked: Cell::new(0),
            #[cfg(feature = "selftest")]
            path: RefCell::new(None),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    pub fn window(&self) -> u64 {
        self.ivars().window
    }

    pub fn root(&self) -> &std::path::Path {
        &self.ivars().root
    }

    pub fn context(&self) -> String {
        self.ivars().context.borrow().clone()
    }

    /// The page and its header.
    pub fn build(&self) -> (Retained<NSView>, Retained<NSStackView>) {
        let mtm = self.mtm();
        let me: &AnyObject = self.as_ref();
        let header = ide_ui_common::panel::header_row(mtm);
        let picker = NSPopUpButton::new(mtm);
        picker.setControlSize(objc2_app_kit::NSControlSize::Small);
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
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 400.0));
        let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), frame);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("containers"));
        column.setResizingMask(objc2_app_kit::NSTableColumnResizingOptions::AutoresizingMask);
        // The table scales its columns by their share of its width, so the one column starts as wide as the table.
        column.setWidth(frame.size.width);
        table.addTableColumn(&column);
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::SourceList);
        table.setAccessibilityLabel(Some(&ns(t("Container"))));
        // SAFETY: The table holds its data source and delegate weakly; the panel registry keeps the panel alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        // SAFETY: The table keeps the menu; its delegate, the panel, outlives the table through the registry.
        unsafe { table.setMenu(Some(&menu)) };
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&table));
        let note = NSTextField::wrappingLabelWithString(&ns(""), mtm);
        note.setTextColor(Some(&NSColor::secondaryLabelColor()));
        note.setContentCompressionResistancePriority_forOrientation(
            objc2_app_kit::NSLayoutPriorityDefaultLow,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        let page = NSStackView::new(mtm);
        page.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        page.setEdgeInsets(NSEdgeInsets {
            top: 4.0,
            left: 8.0,
            bottom: 6.0,
            right: 8.0,
        });
        page.addView_inGravity(&note, NSStackViewGravity::Top);
        page.addView_inGravity(&scroll, NSStackViewGravity::Top);
        *self.ivars().table.borrow_mut() = Some(table);
        *self.ivars().note.borrow_mut() = Some(note);
        *self.ivars().picker.borrow_mut() = Some(picker);
        (Retained::into_super(page), header)
    }

    /// Reads Docker again on a worker thread: contexts, version, containers, images and volumes; starts watching its
    /// events when it answers.
    pub fn refresh(&self) {
        let context = self.context();
        let window = self.window();
        let asked = self.ivars().asked.get() + 1;
        self.ivars().asked.set(asked);
        let root = self.root().to_path_buf();
        let path = self.path();
        let spawned = std::thread::Builder::new().name("docker-read".into()).spawn(move || {
            let env = environment(path, ENVIRONMENT_WAIT);
            let read = read_all(&env, &context, &root);
            main_soon(move || {
                let _ = super::with_panel(window, |panel| panel.read_done(asked, read));
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
        self.ivars().board.borrow_mut().files = ide_docker::compose_files(self.root());
        if let Some(board) = read.board {
            let mut mine = self.ivars().board.borrow_mut();
            mine.containers = board.containers;
            mine.images = board.images;
            mine.volumes = board.volumes;
        }
        match read.reach {
            Reach::Ready => {
                self.ivars().failures.set(0);
                self.watch();
            }
            Reach::NotRunning(_) => self.retry_later(),
            Reach::Unknown | Reach::Missing | Reach::TooOld(_) => {}
        }
        self.show();
    }

    /// Reads Docker again after a wait that grows with each failure in a row.
    fn retry_later(&self) {
        let attempt = self.ivars().failures.get();
        self.ivars().failures.set(attempt + 1);
        let asked = self.ivars().asked.get();
        let window = self.window();
        main_after(ide_docker::backoff(attempt), move || {
            let _ = super::with_panel(window, |panel| {
                if panel.ivars().asked.get() == asked {
                    panel.refresh();
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
        let window = self.window();
        let watch = self.ivars().watches.get() + 1;
        self.ivars().watches.set(watch);
        let started = docker::watch(
            &ide_docker::watch_events(&self.context()),
            &env,
            move |line| {
                if ide_docker::event(line).is_some() {
                    main_soon(move || {
                        let _ = super::with_panel(window, ContainersPanel::changed);
                    });
                }
            },
            move |error| {
                main_soon(move || {
                    let _ = super::with_panel(window, |panel| panel.watch_ended(watch, error));
                });
            },
        );
        match started {
            Ok(id) => self.ivars().events.set(Some(id)),
            Err(error) => {
                *self.ivars().reach.borrow_mut() = Reach::NotRunning(error);
                self.retry_later();
            }
        }
    }

    fn changed(&self) {
        let asked = self.ivars().asked.get();
        let window = self.window();
        main_after(SETTLE, move || {
            let _ = super::with_panel(window, |panel| {
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

    #[cfg(feature = "selftest")]
    fn path(&self) -> Option<String> {
        self.ivars().path.borrow().clone()
    }

    #[cfg(not(feature = "selftest"))]
    #[expect(clippy::unused_self, reason = "the self-test's build reads the panel's PATH")]
    fn path(&self) -> Option<String> {
        None
    }

    /// Gives the panel `path` in place of the login shell's PATH, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn set_path(&self, path: &std::path::Path) {
        self.stop_watching();
        *self.ivars().path.borrow_mut() = Some(format!("{}:/usr/bin:/bin", path.display()));
        *self.ivars().reach.borrow_mut() = Reach::Unknown;
        self.ivars().failures.set(0);
        self.refresh();
    }

    /// Whether Docker can be used, as the panel shows it; for the self-test.
    #[cfg(feature = "selftest")]
    pub fn reach(&self) -> Reach {
        self.ivars().reach.borrow().clone()
    }

    /// Whether the panel watches Docker's events, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn watching(&self) -> bool {
        self.ivars().events.get().is_some()
    }

    pub fn stop_watching(&self) {
        if let Some(id) = self.ivars().events.take() {
            ide_process::Supervisor::global().stop(id, Duration::from_secs(1));
        }
    }

    /// Shows the tree, or the empty state and the note.
    pub fn show(&self) {
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
            label.setHidden(note.is_empty());
        }
        if let Some(picker) = self.ivars().picker.borrow().as_ref() {
            picker.removeAllItems();
            for name in self.ivars().contexts.borrow().iter() {
                picker.addItemWithTitle(&ns(name));
            }
            picker.selectItemWithTitle(&ns(&self.context()));
            picker.setHidden(self.ivars().contexts.borrow().is_empty());
        }
        if let Some(table) = self.ivars().table.borrow().as_ref() {
            table.reloadData();
            let again = selected.and_then(|node| self.ivars().rows.borrow().iter().position(|r| r.node == node));
            match again {
                Some(index) => {
                    table.selectRowIndexes_byExtendingSelection(
                        &objc2_foundation::NSIndexSet::indexSetWithIndex(index),
                        false,
                    );
                }
                None => table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::new(), false),
            }
        }
    }

    /// What the selected row stands for, which stays selected when the rows are read again and shift.
    fn selected_node(&self) -> Option<Node> {
        let table = self.ivars().table.borrow().clone()?;
        let row = usize::try_from(table.selectedRow()).ok()?;
        self.ivars().rows.borrow().get(row).map(|r| r.node.clone())
    }

    /// The row the menu or the selection is about.
    pub fn chosen(&self) -> Option<Row> {
        let table = self.ivars().table.borrow().clone()?;
        let row = table.clickedRow();
        let row = if row < 0 { table.selectedRow() } else { row };
        self.ivars().rows.borrow().get(usize::try_from(row).ok()?).cloned()
    }

    /// Runs `command` and reads Docker again after it; an error shows in an alert.
    pub fn run(&self, command: ide_docker::Command) {
        let window = self.window();
        let path = self.path();
        let spawned = std::thread::Builder::new().name("docker-action".into()).spawn(move || {
            let env = environment(path, ENVIRONMENT_WAIT);
            let outcome = docker::run(&command, &env);
            main_soon(move || {
                if let Err(error) = outcome
                    && let Some(mtm) = MainThreadMarker::new()
                {
                    crate::editor::show_alert(mtm, t("Docker-Befehl fehlgeschlagen"), &error);
                }
                let _ = super::with_panel(window, ContainersPanel::refresh);
            });
        });
        if let Err(error) = spawned {
            crate::diag::report("running docker", error);
        }
    }

    /// Does `action` to container `id`.
    pub fn act(&self, action: Action, id: &str) {
        self.run(ide_docker::container(&self.context(), action, id));
    }

    /// Does `action` to the Compose project of `file`.
    pub fn compose(&self, action: Compose, file: &std::path::Path) {
        self.run(ide_docker::compose(&self.context(), action, file));
    }

    /// The panel's rows as their level, label, detail and look, and the note, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn shown(&self) -> (Vec<(usize, String, String, Look)>, String) {
        let rows = self
            .ivars()
            .rows
            .borrow()
            .iter()
            .map(|r| (r.level, r.label.clone(), r.detail.clone(), r.look))
            .collect();
        let note = self
            .ivars()
            .note
            .borrow()
            .as_ref()
            .map(|n| n.stringValue().to_string())
            .unwrap_or_default();
        (rows, note)
    }

    /// Selects the row labelled `label`, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn select(&self, label: &str) -> bool {
        let index = self.ivars().rows.borrow().iter().position(|r| r.label == label);
        match (index, self.ivars().table.borrow().as_ref()) {
            (Some(index), Some(table)) => {
                let set = objc2_foundation::NSIndexSet::indexSetWithIndex(index);
                table.selectRowIndexes_byExtendingSelection(&set, false);
                true
            }
            _ => false,
        }
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
}

fn read_all(env: &[(String, String)], context: &str, root: &std::path::Path) -> Read {
    if docker::program(env).is_none() {
        return Read {
            contexts: Vec::new(),
            reach: Reach::Missing,
            board: None,
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
                reach: Reach::NotRunning(error),
                board: None,
            };
        }
    };
    if !ide_docker::supported(&version) {
        return Read {
            contexts,
            reach: Reach::TooOld(version),
            board: None,
        };
    }
    let list = |command: ide_docker::Command| docker::run(&command, env).unwrap_or_default();
    let board = Board {
        containers: ide_docker::containers(&list(ide_docker::list_containers(&context))),
        images: ide_docker::images(&list(ide_docker::list_images(&context))),
        volumes: ide_docker::volumes(&list(ide_docker::list_volumes(&context))),
        files: ide_docker::compose_files(root),
    };
    Read {
        contexts,
        reach: Reach::Ready,
        board: Some(board),
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
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(ide_ui_common::style::space::S);
    if let Node::Section(section) = &row.node {
        let title = label(mtm, section_title(*section));
        title.setTextColor(Some(&NSColor::secondaryLabelColor()));
        stack.addView_inGravity(&title, NSStackViewGravity::Leading);
        return stack.into_super();
    }
    let (name, description, color) = look_symbol(row);
    let icon = NSImageView::new(mtm);
    icon.setImage(symbol(name, &description).as_deref());
    icon.setContentTintColor(Some(&color));
    let text = label(mtm, &row.label);
    text.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
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
    detail.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(
        ide_ui_common::style::SMALL,
    )));
    #[expect(clippy::cast_precision_loss, reason = "a nesting level of a few at most")]
    let indent = 2.0 + row.level as f64 * 12.0;
    stack.setEdgeInsets(NSEdgeInsets {
        top: 0.0,
        left: indent,
        bottom: 0.0,
        right: 4.0,
    });
    stack.addView_inGravity(&icon, NSStackViewGravity::Leading);
    stack.addView_inGravity(&text, NSStackViewGravity::Leading);
    stack.addView_inGravity(&detail, NSStackViewGravity::Trailing);
    stack.setAccessibilityElement(true);
    stack.setAccessibilityRole(Some(ide_ui_common::constant!(objc2_app_kit::NSAccessibilityGroupRole)));
    let spoken = if description.is_empty() {
        format!("{}, {}", row.label, row.detail)
    } else {
        format!("{}, {description}, {}", row.label, row.detail)
    };
    stack.setAccessibilityLabel(Some(&ns(&spoken)));
    stack.into_super()
}
