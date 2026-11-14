//! The Tests panel's view and controller (E111): a tree of package, file and test with each one's state as a symbol
//! whose shape tells the state apart, not its colour alone (E101); a header to run everything, list the tests again,
//! show failed ones only and stop; a context menu to run, debug or show a test's output.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_test::board::{Board, Node, Row};
use ide_test::{Scope, State, Tool};
use ide_ui_common::registry::WindowPart;
use ide_ui_common::window::Window;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOn, NSControlTextEditingDelegate, NSMenu, NSMenuDelegate, NSStackView,
    NSStackViewGravity, NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::{t, tf};

use super::run::{self, Running};
use crate::util::{guarded, label, main_after, ns, set_action};

/// How often a running tool's output is read again.
const POLL: Duration = Duration::from_millis(300);

pub struct PanelIvars {
    id: u64,
    window: u64,
    root: PathBuf,
    pub(super) board: RefCell<Board>,
    tools: RefCell<Vec<Tool>>,
    runs: RefCell<Vec<(Tool, Running)>>,
    rows: RefCell<Vec<Row>>,
    table: RefCell<Option<Retained<NSTableView>>>,
    status: RefCell<Option<Retained<NSTextField>>>,
    failed: RefCell<Option<Retained<NSButton>>>,
    polling: Cell<bool>,
    /// The output of the last run of each test, by entry index, for "show output".
    pub(super) output: RefCell<String>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `TestsPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETestsPanel"]
    #[ivars = PanelIvars]
    pub struct TestsPanel;

    unsafe impl NSObjectProtocol for TestsPanel {}

    unsafe impl NSControlTextEditingDelegate for TestsPanel {}

    unsafe impl NSTableViewDataSource for TestsPanel {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().rows.borrow().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for TestsPanel {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let rows = self.ivars().rows.borrow();
                let row = rows.get(usize::try_from(row).ok()?)?;
                Some(row_view(self.mtm(), row))
            })
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
            table: RefCell::new(None),
            status: RefCell::new(None),
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
        let (scroll, table) = ide_ui_common::panel::source_list_table(mtm, "tests", t("Tests"));
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
        let status = label(mtm, "");
        status.setTextColor(Some(&NSColor::secondaryLabelColor()));
        status.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(
            ide_ui_common::style::SMALL,
        )));
        let page = ide_ui_common::panel::panel_page(mtm);
        page.addView_inGravity(&scroll, NSStackViewGravity::Top);
        page.addView_inGravity(&status, NSStackViewGravity::Bottom);
        *self.ivars().table.borrow_mut() = Some(table);
        *self.ivars().status.borrow_mut() = Some(status);
        *self.ivars().failed.borrow_mut() = Some(failed);
        (Retained::into_super(page), header)
    }

    /// Finds the folder's tools and lists their tests; with `all`, also those that list by building.
    pub fn list(&self, all: bool) {
        if !crate::project::is_trusted(self.root()) {
            self.say(t("Tests nur in vertrauten Ordnern"));
            return;
        }
        let env = ide_ui_common::window::try_environment(Duration::ZERO).unwrap_or_default();
        let path = env.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.as_str());
        let tools = ide_test::tools(self.root(), path);
        *self.ivars().tools.borrow_mut() = tools.clone();
        if tools.is_empty() {
            self.say(t("Keine Tests gefunden"));
            return;
        }
        for tool in tools {
            if tool.lists_by_building() && !all {
                continue;
            }
            let command = tool.list(self.root(), &self.scratch());
            self.launch(tool, command, None);
        }
        self.show();
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
            self.list(false);
        }
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

    /// Starts `command` of `tool`, a listing without `scope`, and reads it until it ends.
    fn launch(&self, tool: Tool, command: ide_test::Command, scope: Option<Scope>) {
        let env = ide_ui_common::window::try_environment(Duration::ZERO).unwrap_or_default();
        let label = if scope.is_some() {
            tf("{} ausführen", &[tool.name()])
        } else {
            tf("{} auflisten", &[tool.name()])
        };
        match run::start(&command, &env, &label, scope.is_none()) {
            Ok(running) => {
                self.ivars().runs.borrow_mut().push((tool, running));
                self.say(&label);
                self.poll_later(scope.is_none());
            }
            Err(error) => {
                self.ivars().board.borrow_mut().finished(tool);
                self.say(&tf("{} startet nicht: {}", &[tool.name(), &error.to_string()]));
            }
        }
    }

    fn poll_later(&self, listing: bool) {
        if self.ivars().polling.replace(true) {
            return;
        }
        let id = self.id();
        main_after(POLL, move || {
            super::with_panel(id, |panel| panel.poll(listing));
        });
    }

    /// Reads the runs again: the tools that report starts as they go, every tool once it ended.
    fn poll(&self, listing: bool) {
        self.ivars().polling.set(false);
        let mut ended = Vec::new();
        for (index, (tool, running)) in self.ivars().runs.borrow().iter().enumerate() {
            let done = run::ended(running, Duration::ZERO).is_some();
            if done || tool.reports_starts() {
                let said = run::said(running);
                if done && listing_command(running) {
                    let tests = tool.tests(self.root(), &said);
                    self.ivars().board.borrow_mut().listed(*tool, tests);
                } else {
                    let reports = tool.reports(self.root(), &said);
                    self.ivars().board.borrow_mut().reported(*tool, reports);
                }
            }
            if done {
                *self.ivars().output.borrow_mut() = running.output.lock().map(|o| o.clone()).unwrap_or_default();
                ended.push(index);
            }
        }
        let still = {
            let mut runs = self.ivars().runs.borrow_mut();
            for index in ended.iter().rev() {
                let (tool, _) = runs.remove(*index);
                self.ivars().board.borrow_mut().finished(tool);
            }
            !runs.is_empty()
        };
        self.show();
        super::marks_changed(self);
        if still {
            self.poll_later(listing);
        } else {
            self.say(&summary(&self.ivars().board.borrow()));
        }
    }

    /// Draws the tree as the board is now.
    pub fn show(&self) {
        let rows = self.ivars().board.borrow().rows();
        *self.ivars().rows.borrow_mut() = rows;
        if let Some(table) = self.ivars().table.borrow().as_ref() {
            table.reloadData();
        }
    }

    fn say(&self, text: &str) {
        if let Some(status) = self.ivars().status.borrow().as_ref() {
            status.setStringValue(&ns(text));
        }
    }

    fn clicked_row(&self) -> Option<Row> {
        let table = self.ivars().table.borrow().clone()?;
        let row = table.clickedRow();
        let row = if row < 0 { table.selectedRow() } else { row };
        self.ivars().rows.borrow().get(usize::try_from(row).ok()?).cloned()
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

    /// The panel's rows as their indentation, label and state, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn shown(&self) -> Vec<(usize, String, State)> {
        self.ivars()
            .rows
            .borrow()
            .iter()
            .map(|r| (r.level, r.label.clone(), r.state))
            .collect()
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

/// Whether `running` lists rather than runs, which only a listing's command line says.
fn listing_command(running: &Running) -> bool {
    running.listing
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

fn row_view(mtm: MainThreadMarker, row: &Row) -> Retained<NSView> {
    let (name, description, color) = state_symbol(row.state);
    let spoken = format!("{}, {description}", row.label);
    let (view, _) = ide_ui_common::panel::icon_row(mtm, (name, description, &color), &row.label, row.level, &spoken);
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
