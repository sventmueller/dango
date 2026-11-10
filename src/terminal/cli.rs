//! CLI threads (I5): a coding agent's command-line interface in a terminal tab of a trusted folder, with a status the
//! terminal reads from the outside, as it cannot ask the CLI.
//!
//! The status errs towards "working", so a CLI is never shown waiting while it still works:
//! - finished: the CLI ended;
//! - waiting for input: it rang the bell after the last key and changed no text since, its window title shows an
//!   idle mark and its text has been still for `AFTER_IDLE_TITLE`, or no text changed for `IDLE` while no program it
//!   started runs in front;
//! - working: changed text within `IDLE` (within `ECHO` of a key it is the echo of typing, not work), a spinner in
//!   the window title, or a program in front that was not there when the CLI first went quiet.
//!
//! Only output that changes the text counts (`ide_terminal::OutputWatch`): Codex animates its title while it waits
//! for an approval. The title marks are those Claude Code and Codex use in a terminal without the OSC 7501 Program
//! Status Protocol; `fixtures/` holds sessions of the three CLIs this is tested against.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use ide_process::ProcessId;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, sel};
use objc2_app_kit::{NSColor, NSMenu};
use splat_i18n::t;

use super::view::TerminalView;
use crate::util::{item, main_after, ns, symbol};
use crate::window::{TabKind, WindowController};

/// The CLIs offered, by command; gemini only when it is installed.
const CLIS: [&str; 4] = ["claude", "codex", "agy", "gemini"];
const OPTIONAL: [&str; 1] = ["gemini"];

/// Quiet this long, a CLI waits.
const IDLE: Duration = Duration::from_secs(4);
/// Output this soon after a key is the echo of typing.
const ECHO: Duration = Duration::from_millis(400);
/// Output this soon after the bell belongs to it, as a CLI redraws once it rang.
const AFTER_BELL: Duration = Duration::from_secs(1);
/// Still this long under an idle title, a CLI waits; the time lets its last frame arrive.
const AFTER_IDLE_TITLE: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_secs(1);

/// How a CLI thread stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliStatus {
    Working,
    Waiting,
    Finished,
}

impl CliStatus {
    pub fn text(self) -> &'static str {
        match self {
            CliStatus::Working => t("Arbeitet"),
            CliStatus::Waiting => t("Wartet auf dich"),
            CliStatus::Finished => t("Beendet"),
        }
    }

    /// The colour of its dot, as agent tabs colour theirs.
    pub fn color(self) -> Retained<NSColor> {
        match self {
            CliStatus::Working => crate::agent::style::terracotta(),
            CliStatus::Waiting => NSColor::systemOrangeColor(),
            CliStatus::Finished => NSColor::tertiaryLabelColor(),
        }
    }
}

/// A CLI thread as the activity view and the AI panel list it.
#[derive(Debug, Clone)]
pub struct CliThread {
    /// The terminal's number, which `show_cli_thread` and `stop_cli_thread` take.
    pub key: u64,
    /// The CLI's command, such as `claude`.
    pub name: String,
    pub status: CliStatus,
    pub started: SystemTime,
    pub process: Option<ProcessId>,
    /// What it shows in its window title, or the program it runs in front.
    pub detail: Option<String>,
}

/// What a CLI is started from.
pub struct Launch {
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// What the terminal saw of a CLI, for its status.
#[derive(Debug, Clone, Copy, Default)]
struct Signals {
    key: Option<Instant>,
    output: Option<Instant>,
    bell: Option<Instant>,
    spinner: bool,
    idle_title: bool,
    exited: bool,
}

/// The status `signals` stand for at `now`; `foreign` tells whether a program the CLI started runs in front.
fn decide(signals: &Signals, foreign: bool, now: Instant) -> CliStatus {
    if signals.exited {
        return CliStatus::Finished;
    }
    if let Some(bell) = signals.bell {
        let after_key = signals.key.is_none_or(|key| bell > key);
        let quiet_since = signals.output.is_none_or(|output| output <= bell + AFTER_BELL);
        if after_key && quiet_since {
            return CliStatus::Waiting;
        }
    }
    let quiet_for = |limit: Duration| signals.output.is_none_or(|output| now.duration_since(output) >= limit);
    if signals.spinner || foreign {
        CliStatus::Working
    } else if signals.idle_title && quiet_for(AFTER_IDLE_TITLE) || quiet_for(IDLE) {
        CliStatus::Waiting
    } else {
        CliStatus::Working
    }
}

/// What the first character of a window title says about the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// A spinner frame: Codex's braille dots (or `◌` with reduced motion), Claude Code's alternating half circles.
    Working,
    /// Claude Code's `✳`, which stands where its spinner was once it stops.
    Idle,
    None,
}

fn mark(title: &str) -> Mark {
    match title.trim_start().chars().next() {
        Some('\u{2801}'..='\u{28ff}' | '◐' | '◑' | '◒' | '◓' | '◌') => Mark::Working,
        Some('✳') => Mark::Idle,
        _ => Mark::None,
    }
}

/// What a CLI thread's view keeps besides the terminal.
pub struct CliState {
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub started: SystemTime,
    status: Cell<CliStatus>,
    signals: Cell<Signals>,
    /// The programs in front when the CLI last waited, such as helpers it keeps running.
    baseline: RefCell<Option<BTreeSet<String>>>,
    /// A program the CLI started that runs in front.
    foreign: RefCell<Option<String>>,
    title: RefCell<Option<String>>,
    /// The session's count of text-changing output when it was last looked at.
    content: Cell<u64>,
}

impl CliState {
    pub fn new(launch: Launch) -> CliState {
        CliState {
            name: launch.name,
            program: launch.program,
            args: launch.args,
            started: SystemTime::now(),
            status: Cell::new(CliStatus::Working),
            signals: Cell::new(Signals::default()),
            baseline: RefCell::new(None),
            foreign: RefCell::new(None),
            title: RefCell::new(None),
            content: Cell::new(0),
        }
    }

    pub fn status(&self) -> CliStatus {
        self.status.get()
    }

    fn update(&self, change: impl FnOnce(&mut Signals)) {
        let mut signals = self.signals.get();
        change(&mut signals);
        self.signals.set(signals);
    }

    pub fn key(&self, now: Instant) {
        self.update(|s| s.key = Some(now));
    }

    /// Output arrived; `content` counts the chunks so far that changed the text (`Session::content_changes`).
    pub fn output(&self, content: u64, now: Instant) {
        if self.content.replace(content) == content {
            return;
        }
        self.update(|s| {
            if s.key.is_none_or(|key| now.duration_since(key) > ECHO) {
                s.output = Some(now);
            }
        });
    }

    pub fn bell(&self, now: Instant) {
        self.update(|s| s.bell = Some(now));
    }

    pub fn title(&self, title: Option<&str>) {
        let mark = title.map_or(Mark::None, mark);
        self.update(|s| {
            s.spinner = mark == Mark::Working;
            s.idle_title = mark == Mark::Idle;
        });
        *self.title.borrow_mut() = title
            .map(|t| {
                let t = t.trim_start();
                let shown = if mark == Mark::None {
                    t
                } else {
                    t.trim_start_matches(|c: char| !c.is_whitespace())
                };
                shown.trim().to_string()
            })
            .filter(|t| !t.is_empty());
    }

    pub fn exited(&self) {
        self.update(|s| s.exited = true);
    }

    /// The status at `now`, given the programs in front if they were looked at; whether it changed.
    fn evaluate(&self, programs: Option<Vec<String>>, now: Instant) -> bool {
        let signals = self.signals.get();
        if let Some(programs) = programs {
            let programs: BTreeSet<String> = programs.into_iter().collect();
            let mut baseline = self.baseline.borrow_mut();
            let known = baseline.get_or_insert_with(|| programs.clone());
            *self.foreign.borrow_mut() = programs.difference(known).next().cloned();
        }
        let status = decide(&signals, self.foreign.borrow().is_some(), now);
        self.status.replace(status) != status
    }

    /// Whether the status would be waiting at `now` were it not for the programs in front, which are then worth a look.
    fn quiet(&self, now: Instant) -> bool {
        decide(&self.signals.get(), false, now) == CliStatus::Waiting && !self.signals.get().exited
    }

    pub fn detail(&self) -> Option<String> {
        self.foreign.borrow().clone().or_else(|| self.title.borrow().clone())
    }
}

/// Where `name` is on `path`, a PATH variable.
fn find(path: &str, name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path.split(':').map(|dir| Path::new(dir).join(name)).find(|candidate| {
        std::fs::metadata(candidate).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

fn login_path() -> String {
    crate::environment::get()
        .and_then(|env| {
            env.iter()
                .find(|(name, _)| name == "PATH")
                .map(|(_, value)| value.clone())
        })
        .unwrap_or_default()
}

/// An entry of the CLI-Thread menu.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    /// Why the CLIs cannot start here.
    Note(&'static str),
    Cli {
        name: &'static str,
        found: bool,
        enabled: bool,
    },
}

/// The menu's entries for a window with or without a folder, trusted or not, and the CLIs `found` on the login PATH.
fn entries(folder: bool, trusted: bool, found: &[&str]) -> Vec<Entry> {
    let mut entries = Vec::new();
    if !folder {
        entries.push(Entry::Note(t("Kein Ordner geöffnet")));
    } else if !trusted {
        entries.push(Entry::Note(t(
            "Nur in vertrauten Ordnern: CLIs führen Hooks und Einstellungen des Projekts aus.",
        )));
    }
    for name in CLIS {
        let installed = found.contains(&name);
        if OPTIONAL.contains(&name) && !installed {
            continue;
        }
        entries.push(Entry::Cli {
            name,
            found: installed,
            enabled: installed && folder && trusted,
        });
    }
    entries
}

/// Fills the "+" menu's CLI-Thread submenu for `controller`'s window.
pub fn fill_menu(controller: &WindowController, menu: &NSMenu) {
    let mtm = controller.mtm();
    menu.removeAllItems();
    menu.setAutoenablesItems(false);
    let path = login_path();
    let found: Vec<&str> = CLIS.into_iter().filter(|name| find(&path, name).is_some()).collect();
    for entry in entries(controller.project_root().is_some(), controller.is_trusted(), &found) {
        let entry = match entry {
            Entry::Note(text) => {
                let note = item(mtm, text, None, "");
                note.setEnabled(false);
                note
            }
            Entry::Cli { name, found, enabled } => {
                let cli = item(mtm, name, Some(sel!(ideNewCliThread:)), "");
                cli.setImage(symbol("apple.terminal", name).as_deref());
                // SAFETY: A menu item can represent an object of any type.
                unsafe { cli.setRepresentedObject(Some(&ns(name))) };
                cli.setEnabled(enabled);
                if !found {
                    cli.setToolTip(Some(&ns(t("Nicht im PATH der Login-Shell gefunden"))));
                }
                cli
            }
        };
        menu.addItem(&entry);
    }
}

/// Opens a CLI thread of `name` in `controller`'s folder, if the folder is trusted and the CLI on the login PATH.
pub fn open(controller: &WindowController, name: &str) {
    if !controller.is_trusted() {
        return;
    }
    if let Some(program) = find(&login_path(), name) {
        open_program(controller, name, program);
    }
}

/// Opens a CLI thread running `program` as the CLI `name` in `controller`'s folder.
pub fn open_program(controller: &WindowController, name: &str, program: PathBuf) {
    open_with(controller, name, program, Vec::new());
}

/// The arguments that start the CLI `name` continuing a session: `session` for Claude Code, else the CLI's latest in
/// the folder; none for a CLI that cannot continue one.
fn continuing(name: &str, session: Option<&str>) -> Option<Vec<String>> {
    match (name, session) {
        ("claude", Some(session)) => Some(vec!["--resume".into(), session.into()]),
        ("codex", _) => Some(vec!["resume".into(), "--last".into()]),
        ("gemini", _) => Some(vec!["--resume".into(), "latest".into()]),
        _ => None,
    }
}

/// Opens the CLI thread of `name` again in `controller`'s trusted folder, continuing its session without sending it
/// anything; whether it did.
pub fn reopen(controller: &WindowController, name: &str, session: Option<&str>) -> bool {
    if !controller.is_trusted() {
        return false;
    }
    let Some(args) = continuing(name, session) else {
        return false;
    };
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        let stand_in = ide_core::paths::home().join(format!("stand-in-{name}"));
        let there = stand_in.is_file();
        if there {
            open_with(controller, name, stand_in, args);
        }
        return there;
    }
    let Some(program) = find(&login_path(), name) else {
        return false;
    };
    open_with(controller, name, program, args);
    true
}

/// The CLI thread in the tab `tab` of `controller`'s window: its CLI and, for Claude Code, the session it shows.
pub fn of_tab(controller: &WindowController, tab: isize) -> Option<(String, Option<String>)> {
    let (name, cwd, started) = super::cli_of_tab(controller, tab)?;
    let session = (name == "claude").then(|| claude_session(&cwd, started)).flatten();
    Some((name, session))
}

/// The Claude Code session of a CLI started at `started` in `cwd`: of the transcripts begun since, the one that
/// changed last, as `/clear` begins a new one.
fn claude_session(cwd: &Path, started: SystemTime) -> Option<String> {
    let since = started.checked_sub(Duration::from_secs(2)).unwrap_or(started);
    ide_claude::history::list(&crate::agent_panel::claude_config_dir(), cwd)
        .into_iter()
        .find(|session| {
            std::fs::metadata(&session.path)
                .and_then(|meta| meta.created())
                .is_ok_and(|created| created >= since)
        })
        .map(|session| session.id)
}

fn open_with(controller: &WindowController, name: &str, program: PathBuf, args: Vec<String>) {
    let Some(root) = controller.project_root() else { return };
    let launch = Launch {
        name: name.to_string(),
        program,
        args,
    };
    let (view, page) = TerminalView::new(controller.mtm(), controller, root, Some(launch));
    let tab = controller.add_tab(TabKind::Cli, None, name, page);
    view.set_tab(tab);
    if let Some(window) = controller.window() {
        window.makeFirstResponder(Some(&view));
    }
    super::start(&view);
    view.cli_changed(true);
    tick(view.id());
}

/// Looks at a CLI thread every `TICK` while it runs: the time since its output, and the programs in front once it
/// is quiet.
fn tick(id: u64) {
    main_after(TICK, move || {
        let Some(view) = super::find(id) else { return };
        let Some(cli) = view.ivars().cli.as_ref() else { return };
        let now = Instant::now();
        let programs = cli
            .quiet(now)
            .then(|| view.with_session(ide_terminal::Session::foreground_programs))
            .flatten();
        if cli.evaluate(programs, now) {
            view.cli_changed(false);
        }
        if cli.status() != CliStatus::Finished {
            tick(id);
        }
    });
}

impl TerminalView {
    /// Re-reads the CLI's status after an event and shows a change on the tab, in the activity view and the panel;
    /// `force` shows it even when it stayed.
    pub(super) fn cli_changed(&self, force: bool) {
        let Some(cli) = self.ivars().cli.as_ref() else { return };
        if !cli.evaluate(None, Instant::now()) && !force {
            return;
        }
        let Some(controller) = self.ivars().controller.load() else {
            return;
        };
        if let Some(tab) = self.ivars().tab.get() {
            controller.set_tab_status(tab, Some(&cli.status().color()));
        }
        crate::activity::refresh(&controller);
        crate::agent_panel::refresh(&controller);
    }

    fn cli_thread(&self) -> Option<CliThread> {
        let cli = self.ivars().cli.as_ref()?;
        Some(CliThread {
            key: self.id(),
            name: cli.name.clone(),
            status: cli.status(),
            started: cli.started,
            process: self.with_session(ide_terminal::Session::process),
            detail: cli.detail(),
        })
    }

    fn in_window(&self, controller: &WindowController) -> bool {
        self.ivars()
            .controller
            .load()
            .is_some_and(|c| std::ptr::eq(&*c, controller))
    }
}

/// The CLI threads of `controller`'s window, oldest first.
pub fn threads(controller: &WindowController) -> Vec<CliThread> {
    let mut threads: Vec<CliThread> = super::all_views()
        .into_iter()
        .filter(|view| view.in_window(controller))
        .filter_map(|view| view.cli_thread())
        .collect();
    threads.sort_by_key(|thread| thread.key);
    threads
}

/// The supervisor's numbers of every CLI thread's program.
pub fn processes() -> Vec<ProcessId> {
    super::all_views()
        .into_iter()
        .filter_map(|view| view.cli_thread()?.process)
        .collect()
}

/// Brings the tab of CLI thread `key` forward.
pub fn show(controller: &WindowController, key: u64) {
    if let Some(tab) = super::find(key).and_then(|view| view.ivars().tab.get()) {
        controller.select_tab_by_id(tab);
    }
}

/// Closes the tab of CLI thread `key`, which ends the CLI: Ctrl+C, then SIGTERM to its group.
pub fn stop(controller: &WindowController, key: u64) {
    if let Some(tab) = super::find(key).and_then(|view| view.ivars().tab.get()) {
        controller.close_tab(tab);
    }
}

#[cfg(test)]
mod replay;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_follows_output_typing_the_bell_and_the_end() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut signals = Signals {
            output: Some(at(0)),
            ..Signals::default()
        };
        assert_eq!(decide(&signals, false, at(1_000)), CliStatus::Working);
        assert_eq!(decide(&signals, false, at(5_000)), CliStatus::Waiting, "quiet for long");
        assert_eq!(
            decide(&signals, true, at(5_000)),
            CliStatus::Working,
            "a program in front"
        );
        signals.spinner = true;
        assert_eq!(
            decide(&signals, false, at(5_000)),
            CliStatus::Working,
            "a spinner in the title"
        );
        signals.spinner = false;
        signals.bell = Some(at(1_000));
        assert_eq!(decide(&signals, false, at(1_100)), CliStatus::Waiting, "the bell");
        signals.output = Some(at(1_500));
        assert_eq!(
            decide(&signals, false, at(1_600)),
            CliStatus::Waiting,
            "redrawn after the bell"
        );
        signals.output = Some(at(3_000));
        assert_eq!(
            decide(&signals, false, at(3_100)),
            CliStatus::Working,
            "working again after the bell"
        );
        signals.key = Some(at(2_000));
        signals.output = Some(at(1_500));
        assert_eq!(
            decide(&signals, false, at(2_100)),
            CliStatus::Working,
            "a key after the bell"
        );
        signals.exited = true;
        assert_eq!(decide(&signals, false, at(2_100)), CliStatus::Finished);
    }

    #[test]
    fn title_marks_of_claude_code_and_codex_are_read() {
        assert_eq!(mark("⠋ Reading files"), Mark::Working);
        assert_eq!(mark("◑ Fix the failing test"), Mark::Working);
        assert_eq!(mark("✳ Claude Code"), Mark::Idle);
        assert_eq!(mark("codex"), Mark::None);
    }

    #[test]
    fn an_idle_title_lets_a_cli_wait_soon_after_its_last_frame() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let signals = Signals {
            output: Some(at(0)),
            idle_title: true,
            ..Signals::default()
        };
        assert_eq!(decide(&signals, false, at(500)), CliStatus::Working);
        assert_eq!(decide(&signals, false, at(1_200)), CliStatus::Waiting);
        assert_eq!(
            decide(&signals, true, at(1_200)),
            CliStatus::Working,
            "a program in front"
        );
    }

    #[test]
    fn the_menu_offers_installed_clis_in_trusted_folders_and_says_why_not_otherwise() {
        let cli = |name, found, enabled| Entry::Cli { name, found, enabled };
        assert_eq!(
            entries(true, true, &["claude", "agy"]),
            vec![
                cli("claude", true, true),
                cli("codex", false, false),
                cli("agy", true, true)
            ]
        );
        let untrusted = entries(true, false, &["claude", "gemini"]);
        assert!(matches!(untrusted[0], Entry::Note(_)));
        assert_eq!(untrusted.last(), Some(&cli("gemini", true, false)));
        assert!(matches!(entries(false, false, &[])[0], Entry::Note(_)));
    }
}
