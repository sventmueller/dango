//! A terminal session: a program in a PTY, read on a thread of its own, its screen kept for the view.

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event as TermEvent, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll as TermScroll};
use alacritty_terminal::index::{Column, Direction, Line, Point as TermPoint};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Osc52, Term};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::Handler;
use ide_process::{Kind, ProcessId, Supervisor};
use polling::Poller;

use crate::event::Event;
use crate::foreground::{self, Foreground};
use crate::input::{
    InputModes, Key, KeyCode, Modifiers, MouseEvent, Wheel, encode_focus, encode_key, encode_mouse, encode_paste,
    encode_wheel,
};
use crate::paste::{self, PasteAction, QuoteStyle};
use crate::snapshot::{self, Palette, Point, Snapshot};
use crate::{integration, io as reader, sys};

/// The most history lines a session keeps, whatever the settings say.
pub const MAX_SCROLLBACK: usize = 100_000;

/// What runs in the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// The user's shell as a login shell, with shell integration for zsh, bash and fish.
    LoginShell(PathBuf),
    /// A program with its arguments, run as it is.
    Command {
        /// The executable.
        program: PathBuf,
        /// Its arguments.
        args: Vec<String>,
    },
}

/// The size of the terminal in cells and of one cell in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    /// Columns.
    pub columns: u16,
    /// Lines.
    pub lines: u16,
    /// Width of a cell in pixels.
    pub cell_width: u16,
    /// Height of a cell in pixels.
    pub cell_height: u16,
}

impl Default for Size {
    fn default() -> Size {
        Size {
            columns: 80,
            lines: 24,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

impl Size {
    fn grid(self) -> GridSize {
        GridSize {
            columns: usize::from(self.columns.max(2)),
            lines: usize::from(self.lines.max(1)),
        }
    }

    /// The size for alacritty's PTY, whose pixel arithmetic must stay within 16 bits.
    fn window(self) -> WindowSize {
        let grid = self.grid();
        let columns = u16::try_from(grid.columns).unwrap_or(u16::MAX);
        let lines = u16::try_from(grid.lines).unwrap_or(u16::MAX);
        WindowSize {
            num_cols: columns,
            num_lines: lines,
            cell_width: self.cell_width.min(u16::MAX / columns),
            cell_height: self.cell_height.min(u16::MAX / lines),
        }
    }

    fn winsize(self) -> libc::winsize {
        let window = self.window();
        libc::winsize {
            ws_row: window.num_lines,
            ws_col: window.num_cols,
            ws_xpixel: window.num_cols.saturating_mul(window.cell_width),
            ws_ypixel: window.num_lines.saturating_mul(window.cell_height),
        }
    }

    pub(crate) fn pixels(self) -> (u32, u32) {
        (
            u32::from(self.columns) * u32::from(self.cell_width),
            u32::from(self.lines) * u32::from(self.cell_height),
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct GridSize {
    columns: usize,
    lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// How a session starts.
#[derive(Debug, Clone)]
pub struct Options {
    /// What runs in it.
    pub program: Program,
    /// The working directory it starts in.
    pub cwd: PathBuf,
    /// Its size.
    pub size: Size,
    /// How many lines of history it keeps, at most `MAX_SCROLLBACK`.
    pub scrollback: usize,
    /// Where the shell integration scripts are written; `None` turns shell integration off.
    pub shell_integration: Option<PathBuf>,
    /// The colours.
    pub palette: Palette,
    /// Programs may read the pasteboard (OSC 52); off by default because any program, also one on a remote host, could
    /// read whatever was copied.
    pub allow_clipboard_read: bool,
    /// Programs may switch on the kitty keyboard protocol.
    pub kitty_keyboard: bool,
    /// How long closing waits at each step before it escalates: hang-up, SIGTERM, SIGKILL.
    pub close_grace: Duration,
}

impl Options {
    /// The user's login shell in `cwd`, with shell integration and the defaults.
    pub fn new(cwd: impl Into<PathBuf>) -> Options {
        Options {
            program: Program::LoginShell(ide_process::shell_env::user_shell()),
            cwd: cwd.into(),
            size: Size::default(),
            scrollback: 10_000,
            shell_integration: Some(integration::default_dir()),
            palette: Palette::default(),
            allow_clipboard_read: false,
            kitty_keyboard: true,
            close_grace: Duration::from_secs(2),
        }
    }
}

/// How to scroll through the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scroll {
    /// By lines; positive towards older output.
    Lines(i32),
    /// One screen towards older output.
    PageUp,
    /// One screen towards newer output.
    PageDown,
    /// To the oldest line.
    Top,
    /// Back to the live screen.
    Bottom,
}

/// How a selection grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionKind {
    /// Cell by cell.
    Simple,
    /// A rectangle.
    Block,
    /// Whole words, as after a double click.
    Word,
    /// Whole lines, as after a triple click.
    Line,
}

/// Which half of a cell the pointer is in, which decides whether the cell belongs to a selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(missing_docs, reason = "the names say it")]
pub enum CellSide {
    Left,
    Right,
}

type Listener = Box<dyn Fn(Event) + Send + Sync>;
type ClipboardReply = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// What the session and its reader thread share.
pub(crate) struct Shared {
    pub(crate) term: FairMutex<Term<TermEvents>>,
    pub(crate) palette: Mutex<Palette>,
    pub(crate) size: Mutex<Size>,
    pub(crate) clipboard_reply: Mutex<Option<ClipboardReply>>,
    redraw_pending: AtomicBool,
    full_redraw: AtomicBool,
    pub(crate) running: AtomicBool,
    closed: AtomicBool,
    listener: Listener,
}

impl Shared {
    /// Hands `event` to the owner unless the session was closed.
    pub(crate) fn emit(&self, event: Event) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.listener)(event)));
    }

    /// Asks for a redraw unless one is already pending.
    pub(crate) fn wake(&self) {
        if !self.redraw_pending.swap(true, Ordering::AcqRel) {
            self.emit(Event::Redraw);
        }
    }
}

pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Collects what the emulator reports while it parses, for the reader thread to act on once the lock is released.
#[derive(Clone, Default)]
pub(crate) struct TermEvents(pub(crate) Arc<Mutex<Vec<TermEvent>>>);

impl EventListener for TermEvents {
    fn send_event(&self, event: TermEvent) {
        if !matches!(event, TermEvent::MouseCursorDirty) {
            locked(&self.0).push(event);
        }
    }
}

/// A program running in a terminal. Dropping the session closes it.
pub struct Session {
    shared: Arc<Shared>,
    input: Sender<Vec<u8>>,
    poller: Arc<Poller>,
    master: File,
    pid: u32,
    process: ProcessId,
    supervisor: Supervisor,
    close_grace: Duration,
    closed: AtomicBool,
}

impl Session {
    /// Starts `options.program` in a new PTY with `env`, the user's login-shell environment, and registers it with
    /// `supervisor`. `listener` hears every `Event` on the session's reader thread and must not block it.
    ///
    /// ```no_run
    /// use std::sync::mpsc::channel;
    ///
    /// use ide_process::Supervisor;
    /// use ide_terminal::{Event, Options, Program, Session};
    ///
    /// let mut options = Options::new("/tmp");
    /// options.program = Program::Command { program: "/bin/echo".into(), args: vec!["hello".into()] };
    /// options.shell_integration = None;
    /// let (tx, rx) = channel();
    /// let session = Session::spawn(Supervisor::global(), &[], options, move |event| drop(tx.send(event)))?;
    /// while !matches!(rx.recv()?, Event::Exited(_)) {}
    /// assert_eq!(session.visible_text()[0].trim_end(), "hello");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spawn(
        supervisor: &Supervisor,
        env: &[(String, String)],
        options: Options,
        listener: impl Fn(Event) + Send + Sync + 'static,
    ) -> io::Result<Session> {
        let poller = Arc::new(Poller::new()?);
        let pty = tty::new(&pty_options(&options, env), options.size.window(), 0)?;
        let pid = pty.child().id();
        let master = pty.file().try_clone()?;
        let hang_up_master = pty.file().try_clone()?;
        let term_events = TermEvents::default();
        let config = Config {
            scrolling_history: options.scrollback.min(MAX_SCROLLBACK),
            kitty_keyboard: options.kitty_keyboard,
            osc52: if options.allow_clipboard_read {
                Osc52::CopyPaste
            } else {
                Osc52::OnlyCopy
            },
            ..Config::default()
        };
        let term = Term::new(config, &options.size.grid(), term_events.clone());
        let shared = Arc::new(Shared {
            term: FairMutex::new(term),
            palette: Mutex::new(options.palette.clone()),
            size: Mutex::new(options.size),
            clipboard_reply: Mutex::new(None),
            redraw_pending: AtomicBool::new(false),
            full_redraw: AtomicBool::new(true),
            running: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            listener: Box::new(listener),
        });
        let process = supervisor.register(pid, Kind::Terminal, label(&options.program));
        let shell = i32::try_from(pid).unwrap_or(0);
        let grace = options.close_grace;
        supervisor.on_stop(process, move || hang_up(&hang_up_master, shell, grace));
        let (input, inbox) = channel();
        let reader = reader::Reader::new(pty, Arc::clone(&shared), inbox, Arc::clone(&poller), term_events);
        let thread = reader.start(supervisor.clone(), process);
        if let Err(error) = thread {
            supervisor.stop(process, grace);
            return Err(error);
        }
        Ok(Session {
            shared,
            input,
            poller,
            master,
            pid,
            process,
            supervisor: supervisor.clone(),
            close_grace: grace,
            closed: AtomicBool::new(false),
        })
    }

    /// The process number of the program, which leads the terminal's session.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The supervisor's number for the program.
    pub fn process(&self) -> ProcessId {
        self.process
    }

    /// Whether the program still runs.
    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }

    /// Sends `bytes` to the program as typed input.
    pub fn write(&self, bytes: impl Into<Vec<u8>>) {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return;
        }
        if self.input.send(bytes).is_ok() {
            let _ = self.poller.notify();
        }
    }

    /// Sends a key and returns whether it produced any input; typing scrolls back to the live screen.
    pub fn key(&self, key: &Key) -> bool {
        let bytes = encode_key(key, &self.input_modes());
        if bytes.is_empty() {
            return false;
        }
        self.scroll_to_bottom();
        self.write(bytes);
        true
    }

    /// Pastes `text`, in bracketed-paste markers when the program asked for them.
    pub fn paste(&self, text: &str) {
        let bytes = encode_paste(text, &self.input_modes());
        self.scroll_to_bottom();
        self.write(bytes);
    }

    /// Reports a mouse event if the program asked for it; `false` means the view handles it, such as for a selection.
    pub fn mouse(&self, event: &MouseEvent) -> bool {
        match encode_mouse(event, &self.input_modes()) {
            Some(report) => {
                self.write(report);
                true
            }
            None => false,
        }
    }

    /// Turns the wheel by `lines` (positive away from the user) over `point`.
    pub fn wheel(&self, lines: i32, point: Point, modifiers: Modifiers) {
        match encode_wheel(lines, point.line, point.column, modifiers, &self.input_modes()) {
            Wheel::Send(bytes) => self.write(bytes),
            Wheel::Scroll(lines) => self.scroll(Scroll::Lines(lines)),
        }
    }

    /// Tells the session whether its view has the keyboard focus.
    pub fn focus(&self, focused: bool) {
        self.shared.term.lock().is_focused = focused;
        self.shared.full_redraw.store(true, Ordering::Release);
        if let Some(report) = encode_focus(focused, &self.input_modes()) {
            self.write(report);
        }
    }

    /// Changes the size; the program hears of it through SIGWINCH.
    pub fn resize(&self, size: Size) {
        *locked(&self.shared.size) = size;
        self.shared.term.lock().resize(size.grid());
        let _ = sys::set_window_size(&self.master, &size.winsize());
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    /// The visible screen; also re-arms `Event::Redraw`.
    pub fn snapshot(&self) -> Snapshot {
        self.shared.redraw_pending.store(false, Ordering::Release);
        let palette = locked(&self.shared.palette).clone();
        let mut snapshot = snapshot::take(&mut *self.shared.term.lock(), &palette);
        if self.shared.full_redraw.swap(false, Ordering::AcqRel) {
            snapshot.damage = snapshot::Damage::Full;
        }
        snapshot
    }

    /// The visible lines as plain text, for VoiceOver.
    pub fn visible_text(&self) -> Vec<String> {
        snapshot::visible_text(&*self.shared.term.lock())
    }

    /// Scrolls through the history.
    pub fn scroll(&self, scroll: Scroll) {
        let scroll = match scroll {
            Scroll::Lines(lines) => TermScroll::Delta(lines),
            Scroll::PageUp => TermScroll::PageUp,
            Scroll::PageDown => TermScroll::PageDown,
            Scroll::Top => TermScroll::Top,
            Scroll::Bottom => TermScroll::Bottom,
        };
        self.shared.term.lock().scroll_display(scroll);
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    fn scroll_to_bottom(&self) {
        let mut term = self.shared.term.lock();
        if term.grid().display_offset() != 0 {
            term.scroll_display(TermScroll::Bottom);
            self.shared.full_redraw.store(true, Ordering::Release);
        }
    }

    /// Clears the history and, with `above_cursor`, the lines above the cursor's, which moves that line to the top as a
    /// terminal's Clear command does; the program is not told.
    pub fn clear(&self, above_cursor: bool) {
        let mut term = self.shared.term.lock();
        term.scroll_display(TermScroll::Bottom);
        let line = usize::try_from(term.grid().cursor.point.line.0).unwrap_or(0);
        if above_cursor && line > 0 {
            term.scroll_up(line);
            term.move_up(line);
        }
        term.grid_mut().clear_history();
        term.selection = None;
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    /// Starts a selection at `point`.
    pub fn select(&self, kind: SelectionKind, point: Point, side: CellSide) {
        let mut term = self.shared.term.lock();
        let kind = match kind {
            SelectionKind::Simple => SelectionType::Simple,
            SelectionKind::Block => SelectionType::Block,
            SelectionKind::Word => SelectionType::Semantic,
            SelectionKind::Line => SelectionType::Lines,
        };
        let point = grid_point(&term, point);
        term.selection = Some(Selection::new(kind, point, direction(side)));
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    /// Moves the end of the selection to `point`.
    pub fn extend_selection(&self, point: Point, side: CellSide) {
        let mut term = self.shared.term.lock();
        let point = grid_point(&term, point);
        if let Some(selection) = term.selection.as_mut() {
            selection.update(point, direction(side));
        }
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    /// Removes the selection.
    pub fn clear_selection(&self) {
        self.shared.term.lock().selection = None;
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    /// The selected text.
    pub fn selected_text(&self) -> Option<String> {
        self.shared.term.lock().selection_to_string()
    }

    /// The input modes the program has switched on.
    pub fn input_modes(&self) -> InputModes {
        InputModes::from_term(*self.shared.term.lock().mode())
    }

    /// Changes the colours.
    pub fn set_palette(&self, palette: Palette) {
        *locked(&self.shared.palette) = palette;
        self.shared.full_redraw.store(true, Ordering::Release);
    }

    /// What runs in front: an agent, a remote session, the shell or another program.
    pub fn foreground(&self) -> Foreground {
        foreground::of_terminal(&self.master)
    }

    /// Pastes the PNG `png` as `paste::paste_image` decides, storing it in `dir` when its path is pasted.
    pub fn paste_image(&self, png: &[u8], dir: &Path) -> io::Result<PasteAction> {
        let front = self.foreground();
        let action = paste::paste_image(&front, png, dir)?;
        match &action {
            PasteAction::SendCtrlV => {
                let ctrl_v = Key {
                    code: KeyCode::Char('v'),
                    modifiers: Modifiers {
                        control: true,
                        ..Modifiers::default()
                    },
                    text: Some("v".into()),
                    keypad: false,
                    action: crate::input::KeyAction::Press,
                };
                self.key(&ctrl_v);
            }
            PasteAction::InsertPath(path) => {
                self.paste(&paste::quote_path(path, QuoteStyle::for_foreground(&front)));
            }
            PasteAction::Refuse(_) => {}
        }
        Ok(action)
    }

    /// Pastes the quoted paths of files dropped onto the terminal.
    pub fn paste_paths(&self, paths: &[PathBuf]) {
        let style = QuoteStyle::for_foreground(&self.foreground());
        self.paste(&paste::quote_paths(paths, style));
    }

    /// Answers the program's last request to read the clipboard (`Event::ClipboardLoad`) with `text`.
    pub fn answer_clipboard(&self, text: &str) {
        let reply = locked(&self.shared.clipboard_reply).take();
        if let Some(reply) = reply {
            self.write(reply(text));
        }
    }

    /// Ends the program and everything it started in the terminal, without waiting: hang-up, then SIGTERM, then
    /// SIGKILL, each after `Options::close_grace`. No events follow.
    pub fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.shared.closed.store(true, Ordering::Release);
        self.supervisor.stop(self.process, self.close_grace);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

fn pty_options(options: &Options, env: &[(String, String)]) -> tty::Options {
    let (program, args, extra) = match &options.program {
        Program::LoginShell(shell) => {
            let scripts = options
                .shell_integration
                .as_deref()
                .filter(|dir| integration::install(dir).is_ok());
            let launch = integration::launch(shell, scripts, env);
            (shell.clone(), launch.args, launch.env)
        }
        Program::Command { program, args } => (program.clone(), args.clone(), Vec::new()),
    };
    let mut vars: HashMap<String, String> = env.iter().cloned().collect();
    vars.extend(extra);
    vars.insert("TERM".into(), "xterm-256color".into());
    vars.insert("COLORTERM".into(), "truecolor".into());
    vars.insert("TERM_PROGRAM".into(), ide_product::NAME.into());
    let has_locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .any(|name| vars.get(*name).is_some_and(|value| !value.is_empty()));
    if !has_locale {
        // Without a UTF-8 character type shells treat every non-ASCII byte as a character of its own.
        vars.insert("LC_CTYPE".into(), "UTF-8".into());
    }
    tty::Options {
        shell: Some(tty::Shell::new(program.to_string_lossy().into_owned(), args)),
        working_directory: Some(options.cwd.clone()),
        drain_on_exit: true,
        env: vars,
    }
}

fn label(program: &Program) -> String {
    let path = match program {
        Program::LoginShell(path) | Program::Command { program: path, .. } => path,
    };
    path.file_name()
        .map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy())
        .into_owned()
}

fn grid_point<T>(term: &Term<T>, point: Point) -> TermPoint {
    let offset = i32::try_from(term.grid().display_offset()).unwrap_or(0);
    let line = i32::try_from(point.line.min(term.screen_lines() - 1)).unwrap_or(0);
    TermPoint::new(Line(line - offset), Column(point.column.min(term.columns() - 1)))
}

fn direction(side: CellSide) -> Direction {
    match side {
        CellSide::Left => Direction::Left,
        CellSide::Right => Direction::Right,
    }
}

/// What closing a terminal window does: SIGHUP to the shell and to the job in front, which lives in a process group
/// of its own and so is beyond the supervisor's SIGTERM and SIGKILL for the shell's group.
fn hang_up(master: &File, shell: i32, grace: Duration) {
    let front = sys::foreground_group(master).filter(|&group| group != shell);
    sys::signal_group(shell, libc::SIGHUP);
    let Some(front) = front else { return };
    sys::signal_group(front, libc::SIGHUP);
    let _ = std::thread::Builder::new()
        .name("terminal-hang-up".into())
        .spawn(move || {
            for signal in [libc::SIGTERM, libc::SIGKILL] {
                if group_ends_within(front, grace) {
                    return;
                }
                sys::signal_group(front, signal);
            }
        });
}

fn group_ends_within(group: i32, grace: Duration) -> bool {
    let deadline = Instant::now() + grace;
    while sys::group_exists(group) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_sizes_stay_within_what_the_pty_can_hold() {
        let huge = Size {
            columns: 1000,
            lines: 500,
            cell_width: 200,
            cell_height: 300,
        };
        let window = huge.window();
        assert!(u32::from(window.num_cols) * u32::from(window.cell_width) <= u32::from(u16::MAX));
        assert!(u32::from(window.num_lines) * u32::from(window.cell_height) <= u32::from(u16::MAX));
        let tiny = Size {
            columns: 0,
            lines: 0,
            cell_width: 8,
            cell_height: 16,
        };
        assert_eq!((tiny.grid().columns, tiny.grid().lines), (2, 1));
    }

    #[test]
    fn the_program_gets_the_terminals_variables_on_top_of_the_users() {
        let options = Options {
            program: Program::Command {
                program: "/bin/echo".into(),
                args: vec!["x".into()],
            },
            ..Options::new("/tmp")
        };
        let env = [
            ("PATH".to_string(), "/u/bin".to_string()),
            ("TERM".to_string(), "dumb".to_string()),
        ];
        let pty = pty_options(&options, &env);
        assert_eq!(pty.env["PATH"], "/u/bin");
        assert_eq!(pty.env["TERM"], "xterm-256color");
        assert_eq!(pty.env["COLORTERM"], "truecolor");
        assert_eq!(pty.env["TERM_PROGRAM"], ide_product::NAME);
        assert_eq!(pty.env["LC_CTYPE"], "UTF-8");
        let with_lang = [("LANG".to_string(), "de_DE.UTF-8".to_string())];
        assert!(!pty_options(&options, &with_lang).env.contains_key("LC_CTYPE"));
        assert_eq!(label(&options.program), "echo");
    }
}
