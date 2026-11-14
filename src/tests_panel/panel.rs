//! The Tests panel's view and controller (E111): a tree of package, file and test with each one's state as a symbol
//! whose shape tells the state apart, not its colour alone (E101); a header to run everything, list the tests again,
//! show failed ones only and stop; a context menu to run, debug or show a test's output.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_process::Ending;
use ide_test::board::{Board, Node, Row};
use ide_test::{Scope, State, Tool};
use ide_ui_common::registry::WindowPart;
use ide_ui_common::window::Window;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOn, NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSOutlineView,
    NSOutlineViewDataSource, NSOutlineViewDelegate, NSProgressIndicator, NSProgressIndicatorStyle, NSStackView,
    NSStackViewGravity, NSTableColumn, NSTextField, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSRect};
use splat_i18n::{t, tf};

use super::outline::{self, Outline};
use super::run::{self, Running};
use crate::util::{guarded, main_after, main_soon, ns, set_action};

mod poll;

/// How often a running tool's output is read again.
const POLL: Duration = Duration::from_millis(300);
/// The most changed rows redrawn one by one; past it the whole outline loads again.
const RELOADED_ROWS: usize = 100;

pub struct PanelIvars {
    id: u64,
    window: u64,
    root: PathBuf,
    pub(super) board: RefCell<Board>,
    tools: RefCell<Vec<Tool>>,
    runs: RefCell<Vec<(Tool, Running)>>,
    /// Every row of the tree, packages before their files and tests.
    rows: RefCell<Vec<Row>>,
    outline: RefCell<Outline>,
    /// The key the next run gets.
    next_run: Cell<u64>,
    /// The runs handed to the disk lane to start.
    starting: Cell<usize>,
    /// The board's revision the editors' marks were last shown for.
    marked: Cell<Option<u64>>,
    /// The gutter's test states of each file it drew.
    pub(super) marks: RefCell<std::collections::HashMap<PathBuf, super::Marks>>,
    /// Set while the panel opens rows itself, so that does not count as the user's choice.
    unfolding: Cell<bool>,
    table: RefCell<Option<Retained<NSOutlineView>>>,
    scroll: RefCell<Option<Retained<NSView>>>,
    /// What the panel shows before the first run.
    empty: RefCell<Option<Retained<NSView>>>,
    spinner: RefCell<Option<Retained<NSProgressIndicator>>>,
    status: RefCell<Option<Retained<NSTextField>>>,
    /// The row of the spinner and the status, hidden while neither has anything to show.
    status_row: RefCell<Option<Retained<NSStackView>>>,
    failed: RefCell<Option<Retained<NSButton>>>,
    polling: Cell<bool>,
    /// The output of the last run of each test, by entry index, for "show output".
    pub(super) output: RefCell<String>,
}

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `TestsOutlineView` does not implement Drop.
    /// The tests' outline, its disclosure triangles where the panel's title begins as in the other panels.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETestsOutlineView"]
    struct TestsOutlineView;

    unsafe impl NSObjectProtocol for TestsOutlineView {}

    impl TestsOutlineView {
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn outline_cell_frame(&self, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfOutlineCellAtRow: row] };
            ide_ui_common::panel::outdent(self, frame, ide_ui_common::panel::OUTDENT, false)
        }

        #[unsafe(method(frameOfCellAtColumn:row:))]
        fn cell_frame(&self, column: isize, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfCellAtColumn: column, row: row] };
            if column != 0 {
                return frame;
            }
            ide_ui_common::panel::outdent(self, frame, ide_ui_common::panel::OUTDENT, true)
        }
    }
);

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `TestsPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETestsPanel"]
    #[ivars = PanelIvars]
    pub struct TestsPanel;

    unsafe impl NSObjectProtocol for TestsPanel {}

    unsafe impl NSControlTextEditingDelegate for TestsPanel {}

    unsafe impl NSOutlineViewDataSource for TestsPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || {
                let outline = self.ivars().outline.borrow();
                outline
                    .children_of(item)
                    .map_or(0, |rows| isize::try_from(rows.len()).unwrap_or(0))
            })
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let outline = self.ivars().outline.borrow();
                let at = *outline.children_of(item)?.get(usize::try_from(index).ok()?)?;
                outline.item(at)
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || {
                let outline = self.ivars().outline.borrow();
                outline.children_of(Some(item)).is_some_and(|rows| !rows.is_empty())
            })
        }
    }

    unsafe impl NSOutlineViewDelegate for TestsPanel {
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

    unsafe impl NSMenuDelegate for TestsPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    impl TestsPanel {
        #[unsafe(method(ideTestsRunAll:))]
        fn run_all(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.run_everything());
        }

        #[unsafe(method(ideTestsList:))]
        fn list_again(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.list(true));
        }

        #[unsafe(method(ideTestsFailedOnly:))]
        fn failed_only(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.ivars().board.borrow_mut().failed_only = on;
                self.show();
            });
        }

        #[unsafe(method(ideTestsStop:))]
        fn stop(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                for (_, running) in self.ivars().runs.borrow().iter() {
                    run::stop(running);
                }
            });
        }

        #[unsafe(method(ideTestsDoubleClicked:))]
        fn double_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.reveal_clicked());
        }

        #[unsafe(method(ideTestsRunRow:))]
        fn run_row(&self, sender: Option<&AnyObject>) {
            guarded((), || self.on_row(sender, false));
        }

        #[unsafe(method(ideTestsDebugRow:))]
        fn debug_row(&self, sender: Option<&AnyObject>) {
            guarded((), || self.on_row(sender, true));
        }

        #[unsafe(method(ideTestsShowOutput:))]
        fn show_output(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.output_sheet());
        }
    }
);

impl TestsPanel {
    /// The panel `id` of the window `window` for the folder `root`.
    pub fn new(mtm: MainThreadMarker, id: u64, window: u64, root: PathBuf) -> Retained<TestsPanel> {
        let this = Self::alloc(mtm).set_ivars(PanelIvars {
            id,
            window,
            root,
            board: RefCell::new(Board::default()),
            tools: RefCell::new(Vec::new()),
            runs: RefCell::new(Vec::new()),
            rows: RefCell::new(Vec::new()),
            outline: RefCell::new(Outline::default()),
            next_run: Cell::new(0),
            starting: Cell::new(0),
            marked: Cell::new(None),
            marks: RefCell::new(std::collections::HashMap::new()),
            unfolding: Cell::new(false),
            table: RefCell::new(None),
            scroll: RefCell::new(None),
            empty: RefCell::new(None),
            spinner: RefCell::new(None),
            status: RefCell::new(None),
            status_row: RefCell::new(None),
            failed: RefCell::new(None),
            polling: Cell::new(false),
            output: RefCell::new(String::new()),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    pub fn id(&self) -> u64 {
        self.ivars().id
    }

    pub fn window(&self) -> u64 {
        self.ivars().window
    }

    pub fn root(&self) -> &std::path::Path {
        &self.ivars().root
    }

    /// The page and its header, for the window's sidebar.
    pub fn build(&self) -> (Retained<NSView>, Retained<NSStackView>) {
        let mtm = self.mtm();
        let me: &AnyObject = self.as_ref();
        let button =
            |name: &str, title: &str, action| ide_ui_common::panel::header_button(mtm, name, title, Some(me), action);
        let run = button("play.fill", t("Alle Tests ausführen"), sel!(ideTestsRunAll:));
        let find = button("arrow.clockwise", t("Tests suchen"), sel!(ideTestsList:));
        let failed = button(
            "line.3.horizontal.decrease.circle",
            t("Nur fehlgeschlagene"),
            sel!(ideTestsFailedOnly:),
        );
        failed.setButtonType(objc2_app_kit::NSButtonType::PushOnPushOff);
        let stop = button("stop.fill", t("Testlauf stoppen"), sel!(ideTestsStop:));
        let header = ide_ui_common::panel::header_row(mtm);
        for button in [&run, &find, &failed, &stop] {
            header.addArrangedSubview(button);
        }
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        let table: Retained<TestsOutlineView> =
            unsafe { msg_send![TestsOutlineView::alloc(mtm), initWithFrame: ide_ui_common::panel::LIST_FRAME] };
        let table: Retained<NSOutlineView> = table.into_super();
        let scroll = ide_ui_common::panel::source_list_outline(mtm, &table, "tests", t("Tests"));
        // SAFETY: The table holds its data source and delegate weakly; the panel registry keeps the panel alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        set_action(&table, self, sel!(ideTestsDoubleClicked:));
        // SAFETY: The panel answers the double action with a method that takes the sender; `set_action` checked it.
        unsafe { table.setDoubleAction(Some(sel!(ideTestsDoubleClicked:))) };
        // SAFETY: A single click only selects; the double action reveals.
        unsafe { table.setAction(None) };
        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        // SAFETY: The table keeps the menu; its delegate, the panel, outlives the table through the registry.
        unsafe { table.setMenu(Some(&menu)) };
        let (status_row, status, spinner) = status_row(mtm);
        let empty = crate::window::panels::empty_state(
            mtm,
            "checklist",
            t("Noch keine Tests ausgeführt"),
            t("Führe die Tests aus, um ihre Ergebnisse hier zu sehen."),
            Some((t("Alle Tests ausführen"), sel!(ideTestsRunAll:))),
        );
        if let Some(link) = empty.arrangedSubviews().lastObject()
            && let Ok(link) = link.downcast::<NSButton>()
        {
            set_action(&link, self, sel!(ideTestsRunAll:));
        }
        let page = ide_ui_common::panel::panel_page(mtm);
        scroll.setHidden(true);
        page.addView_inGravity(&status_row, NSStackViewGravity::Top);
        page.addView_inGravity(&scroll, NSStackViewGravity::Top);
        page.addView_inGravity(&empty, NSStackViewGravity::Center);
        status_row
            .widthAnchor()
            .constraintEqualToAnchor(&page.widthAnchor())
            .setActive(true);
        *self.ivars().table.borrow_mut() = Some(table);
        *self.ivars().scroll.borrow_mut() = Some(scroll.into_super());
        *self.ivars().empty.borrow_mut() = Some(empty.into_super());
        *self.ivars().spinner.borrow_mut() = Some(spinner);
        *self.ivars().status.borrow_mut() = Some(status);
        *self.ivars().status_row.borrow_mut() = Some(status_row);
        *self.ivars().failed.borrow_mut() = Some(failed);
        (Retained::into_super(page), header)
    }

    /// Finds the folder's tools and lists their tests; with `all`, also those that list by building.
    pub fn list(&self, all: bool) {
        self.find_tools(move |panel| panel.list_found(all));
    }

    /// Lists the tests of the tools found; with `all`, also those that list by building.
    fn list_found(&self, all: bool) {
        let tools = self.ivars().tools.borrow().clone();
        if tools.is_empty() {
            self.say(t("Keine Tests gefunden"));
            return;
        }
        let waiting = !all && tools.iter().all(|tool| tool.lists_by_building());
        for tool in tools {
            if tool.lists_by_building() && !all {
                self.find_in_sources(tool);
                continue;
            }
            let command = tool.list(self.root(), &self.scratch());
            self.launch(tool, command, None);
        }
        self.show();
        if waiting {
            self.say(t("Auflisten baut das Projekt; dafür ↻ drücken"));
        }
    }

    /// Reads the tests of `tool` from the sources on the disk lane, to show them until a listing or a run comes.
    fn find_in_sources(&self, tool: Tool) {
        let id = self.id();
        let root = self.root().to_path_buf();
        let _ = ide_core::lane::DISK.ask(
            move || tool.from_source(&root),
            move |tests| {
                main_soon(move || {
                    super::with_panel(id, |panel| panel.found_in_sources(tool, tests.unwrap_or_default()));
                });
            },
        );
    }

    fn found_in_sources(&self, tool: Tool, tests: Vec<ide_test::Test>) {
        self.ivars().board.borrow_mut().guessed(tool, tests);
        self.show();
        super::marks_changed(self);
    }

    fn scratch(&self) -> PathBuf {
        std::env::temp_dir().join(format!("dango-tests-{}", self.window()))
    }

    /// Runs `scope` with `tool`.
    pub fn run(&self, tool: Tool, scope: Scope) {
        if !crate::project::is_trusted(self.root()) {
            self.say(t("Tests nur in vertrauten Ordnern"));
            return;
        }
        let command = tool.run(self.root(), &scope, Some(&self.scratch()));
        self.ivars().board.borrow_mut().started(tool, &scope);
        self.launch(tool, command, Some(scope));
        self.show();
    }

    /// Runs every test of every tool.
    pub fn run_everything(&self) {
        if self.ivars().tools.borrow().is_empty() {
            self.find_tools(|panel| {
                panel.list_found(false);
                panel.run_known();
            });
        } else {
            self.run_known();
        }
    }

    fn run_known(&self) {
        let tools = self.ivars().tools.borrow().clone();
        for tool in tools {
            self.run(tool, Scope::All);
        }
    }

    /// Runs the last run again.
    pub fn rerun(&self) {
        let last = self.ivars().board.borrow().last_run.clone();
        match last {
            Some((tool, scope)) => self.run(tool, scope),
            None => self.run_everything(),
        }
    }

    /// Draws the tree as the board is now: only the rows that changed where the tree kept its shape.
    pub fn show(&self) {
        let rows = self.ivars().board.borrow().rows();
        let redrawn = {
            let old = self.ivars().rows.borrow();
            outline::same_shape(&old, &rows).then(|| {
                (0..rows.len())
                    .filter(|&i| old[i].label != rows[i].label || old[i].state != rows[i].state)
                    .collect::<Vec<_>>()
            })
        };
        if redrawn.is_none() {
            self.ivars().outline.borrow_mut().index(&rows);
        }
        *self.ivars().rows.borrow_mut() = rows;
        self.show_state();
        let Some(table) = self.ivars().table.borrow().clone() else {
            return;
        };
        let item = |row: usize| self.ivars().outline.borrow().item(row);
        match &redrawn {
            Some(redrawn) if redrawn.len() <= RELOADED_ROWS => {
                for item in redrawn.iter().filter_map(|row| item(*row)) {
                    // SAFETY: The item is one the data source handed the outline for this row.
                    unsafe { table.reloadItem(Some(&item)) };
                }
            }
            _ => table.reloadData(),
        }
        let open: Vec<_> = {
            let board = self.ivars().board.borrow();
            let rows = self.ivars().rows.borrow();
            let outline = self.ivars().outline.borrow();
            (0..rows.len())
                .filter(|&row| !outline.children(Some(row)).is_empty() && board.is_open(&rows[row].node))
                .filter_map(|row| outline.item(row))
                .collect()
        };
        self.ivars().unfolding.set(true);
        for item in open {
            // SAFETY: The item is one the data source handed the outline for this row.
            unsafe {
                if !table.isItemExpanded(Some(&item)) {
                    table.expandItem(Some(&item));
                }
            }
        }
        self.ivars().unfolding.set(false);
    }

    /// Shows the tree or the hint before the first run, and the spinner while tests run.
    fn show_state(&self) {
        let known = {
            let board = self.ivars().board.borrow();
            board.last_run.is_some() || !board.entries().is_empty()
        };
        if let (Some(scroll), Some(empty)) = (
            self.ivars().scroll.borrow().as_ref(),
            self.ivars().empty.borrow().as_ref(),
        ) {
            scroll.setHidden(!known);
            empty.setHidden(known);
        }
        let busy = self.ivars().runs.borrow().iter().any(|(_, r)| !r.listing);
        if let Some(spinner) = self.ivars().spinner.borrow().as_ref() {
            spinner.setHidden(!busy);
            // SAFETY: A nil sender is what AppKit's own callers pass.
            unsafe {
                if busy {
                    spinner.startAnimation(None);
                } else {
                    spinner.stopAnimation(None);
                }
            }
        }
        self.show_status_row();
    }

    /// The index in `rows` of the row `item` stands for.
    fn index_of(&self, item: &AnyObject) -> Option<usize> {
        self.ivars().outline.borrow().row_of(item)
    }

    /// Notes that the user opened or closed a package or file, which stays so when the tree is built again.
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
        let node = self.ivars().rows.borrow()[at].node.clone();
        self.ivars().board.borrow_mut().set_open(node, open);
    }

    fn say(&self, text: &str) {
        if let Some(status) = self.ivars().status.borrow().as_ref() {
            status.setStringValue(&ns(text));
        }
        self.show_status_row();
    }

    fn show_status_row(&self) {
        let said = self
            .ivars()
            .status
            .borrow()
            .as_ref()
            .is_some_and(|status| status.stringValue().length() > 0);
        let busy = self
            .ivars()
            .spinner
            .borrow()
            .as_ref()
            .is_some_and(|spinner| !spinner.isHidden());
        if let Some(row) = self.ivars().status_row.borrow().as_ref() {
            row.setHidden(!said && !busy);
        }
    }

    fn clicked_row(&self) -> Option<Row> {
        let table = self.ivars().table.borrow().clone()?;
        let row = table.clickedRow();
        let row = if row < 0 { table.selectedRow() } else { row };
        let item = table.itemAtRow(row)?;
        let at = self.index_of(&item)?;
        self.ivars().rows.borrow().get(at).cloned()
    }

    fn on_row(&self, _sender: Option<&AnyObject>, debug: bool) {
        let Some(row) = self.clicked_row() else { return };
        let Some((tool, scope)) = scope_of(&self.ivars().board.borrow(), &row.node) else {
            return;
        };
        match (debug, scope) {
            (true, Scope::Test(id)) => super::debug(self, tool, &id),
            (_, scope) => self.run(tool, scope),
        }
    }

    fn reveal_clicked(&self) {
        let Some(Row {
            node: Node::Test(index),
            ..
        }) = self.clicked_row()
        else {
            return;
        };
        let place = {
            let board = self.ivars().board.borrow();
            let entry = board.entries().get(index);
            entry.and_then(|e| e.failure.clone().or_else(|| e.test.location.clone()))
        };
        if let (Some(place), Some(controller)) = (place, crate::host::window(self.window())) {
            crate::editor::reveal_position(&controller, &place.path, place.line as usize, 1);
        }
    }

    fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let Some(row) = self.clicked_row() else { return };
        let mtm = self.mtm();
        let me: &AnyObject = self.as_ref();
        let item = |title: &str, action: objc2::runtime::Sel| {
            let entry = crate::util::item(mtm, title, Some(action), "");
            crate::util::set_target(&entry, me);
            entry
        };
        for command in row_commands(&row.node) {
            menu.addItem(&item(command.title(), command.action()));
        }
    }

    fn output_sheet(&self) {
        let text = self.ivars().output.borrow().clone();
        let message = match self.clicked_row().and_then(|row| match row.node {
            Node::Test(index) => self
                .ivars()
                .board
                .borrow()
                .entries()
                .get(index)
                .and_then(|e| e.message.clone()),
            _ => None,
        }) {
            Some(message) => format!("{message}\n\n{text}"),
            None => text,
        };
        crate::editor::show_text(self.mtm(), t("Ausgabe des Testlaufs"), &message);
    }
}

impl WindowPart for TestsPanel {
    fn window(&self) -> Option<Window> {
        Window::open(self.ivars().window)
    }

    fn root(&self) -> &Path {
        TestsPanel::root(self)
    }
}

/// What the context menu offers on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowCommand {
    Run,
    Debug,
    ShowOutput,
}

impl RowCommand {
    fn title(self) -> &'static str {
        match self {
            RowCommand::Run => t("Ausführen"),
            RowCommand::Debug => t("Debuggen"),
            RowCommand::ShowOutput => t("Ausgabe zeigen"),
        }
    }

    fn action(self) -> objc2::runtime::Sel {
        match self {
            RowCommand::Run => sel!(ideTestsRunRow:),
            RowCommand::Debug => sel!(ideTestsDebugRow:),
            RowCommand::ShowOutput => sel!(ideTestsShowOutput:),
        }
    }
}

/// What the context menu offers on a row of `node`; only a single test can be debugged.
fn row_commands(node: &Node) -> Vec<RowCommand> {
    match node {
        Node::Test(_) => vec![RowCommand::Run, RowCommand::Debug, RowCommand::ShowOutput],
        Node::Package(..) | Node::File(..) => vec![RowCommand::Run, RowCommand::ShowOutput],
    }
}

/// The tool and scope that run the tests of `node`; a package without a name stands for all of its tool's tests.
fn scope_of(board: &Board, node: &Node) -> Option<(Tool, Scope)> {
    match node {
        Node::Package(tool, package) if package.is_empty() => Some((*tool, Scope::All)),
        Node::Package(tool, package) => Some((*tool, Scope::Package(package.clone()))),
        Node::File(tool, _, file) => Some((*tool, Scope::File(file.clone()))),
        Node::Test(index) => board.scope_of(*index),
    }
}

/// What the status line says after the runs ended.
fn summary(board: &Board) -> String {
    let count = |state: State| board.entries().iter().filter(|e| e.state == state).count().to_string();
    tf(
        "{} bestanden, {} fehlgeschlagen, {} übersprungen",
        &[&count(State::Passed), &count(State::Failed), &count(State::Skipped)],
    )
}

/// The symbol and its description for `state`; the shapes differ, so the state reads without colour (E101).
pub fn state_symbol(state: State) -> (&'static str, &'static str, Retained<NSColor>) {
    match state {
        State::NotRun => ("circle", t("Nicht gelaufen"), NSColor::tertiaryLabelColor()),
        State::Running => ("clock", t("Läuft"), NSColor::systemBlueColor()),
        State::Passed => ("checkmark.circle.fill", t("Bestanden"), NSColor::systemGreenColor()),
        State::Failed => ("xmark.octagon.fill", t("Fehlgeschlagen"), NSColor::systemRedColor()),
        State::Skipped => ("minus.circle", t("Übersprungen"), NSColor::secondaryLabelColor()),
    }
}

/// The line between the header and the list: a spinner while tests run, then what the panel last said, wrapped to
/// the sidebar's width so it never holds the sidebar wider.
fn status_row(
    mtm: MainThreadMarker,
) -> (
    Retained<NSStackView>,
    Retained<NSTextField>,
    Retained<NSProgressIndicator>,
) {
    let status = NSTextField::wrappingLabelWithString(&ns(""), mtm);
    status.setTextColor(Some(&NSColor::secondaryLabelColor()));
    status.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(
        ide_ui_common::style::SMALL,
    )));
    status.setContentCompressionResistancePriority_forOrientation(
        objc2_app_kit::NSLayoutPriorityDefaultLow,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    status.setContentHuggingPriority_forOrientation(
        objc2_app_kit::NSLayoutPriorityDefaultLow,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    let spinner = NSProgressIndicator::new(mtm);
    spinner.setStyle(NSProgressIndicatorStyle::Spinning);
    spinner.setControlSize(objc2_app_kit::NSControlSize::Small);
    spinner.setHidden(true);
    let row = ide_ui_common::panel::padded(mtm, &status);
    row.setSpacing(ide_ui_common::style::space::S);
    row.insertArrangedSubview_atIndex(&spinner, 0);
    row.setHidden(true);
    (row, status, spinner)
}

fn row_view(mtm: MainThreadMarker, row: &Row) -> Retained<NSView> {
    let (name, description, color) = state_symbol(row.state);
    let spoken = format!("{}, {description}", row.label);
    // The outline indents by level itself.
    let (view, _) = ide_ui_common::panel::icon_row(mtm, (name, description, &color), &row.label, 0, &spoken);
    view.into_super()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_single_test_offers_debugging() {
        let file = Node::File(Tool::Cargo, "app".into(), "src/lib.rs".into());
        assert_eq!(row_commands(&file), [RowCommand::Run, RowCommand::ShowOutput]);
        assert_eq!(
            row_commands(&Node::Test(0)),
            [RowCommand::Run, RowCommand::Debug, RowCommand::ShowOutput]
        );
    }

    #[test]
    fn a_row_runs_its_package_its_file_or_every_test_of_its_tool() {
        let board = Board::default();
        assert_eq!(
            scope_of(&board, &Node::Package(Tool::Go, String::new())),
            Some((Tool::Go, Scope::All))
        );
        assert_eq!(
            scope_of(&board, &Node::Package(Tool::Go, "pkg".into())),
            Some((Tool::Go, Scope::Package("pkg".into())))
        );
        assert_eq!(
            scope_of(
                &board,
                &Node::File(Tool::Pytest, "tests".into(), "tests/test_a.py".into())
            ),
            Some((Tool::Pytest, Scope::File("tests/test_a.py".into())))
        );
        assert_eq!(scope_of(&board, &Node::Test(3)), None);
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
