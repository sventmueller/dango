//! The reader thread: PTY output into the emulator and the scanner, typed input and the emulator's replies out.
//!
//! Every chunk also goes through `Scanner`, which needs the raw bytes for OSC 7, OSC 133 and server addresses. The
//! thread reserves the screen for a batch, parses at most `MAX_LOCKED_READ` bytes under the lock and keeps reading
//! into its buffer while the view holds the lock, so neither side waits long for the other.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::num::NonZeroUsize;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ide_core::LockIgnoringPoison;
use ide_process::{Ending, ProcessId, Supervisor};
use polling::{Event as Interest, Events, Poller};
use term_vt::Event as TermEvent;
use term_vt::term::Modes;

use crate::event::{Clipboard, Event};
use crate::pty::Pty;
use crate::scan::Scanner;
use crate::session::{ClipboardReply, Shared};

/// The keys the poller reports the PTY and the program's end under.
const PTY_KEY: usize = 0;
const CHILD_KEY: usize = 1;

/// Bytes read before the screen must take them, even if the view holds it.
const READ_BUFFER: usize = 0x10_0000;

/// Bytes parsed under one hold of the lock.
const MAX_LOCKED_READ: usize = u16::MAX as usize;

/// The longest a synchronized update (mode 2026) holds the screen back.
const SYNC_TIMEOUT: Duration = Duration::from_millis(150);

/// How long a full-screen program's redraw may pause between two pieces and still show as one.
const BRIDGE_WAIT_MS: i32 = 1;

/// The longest a batch waits for further pieces of a redraw, well under a display frame.
const BRIDGE_BUDGET: Duration = Duration::from_millis(3);

/// How long the program has to be quiet before the history is packed.
const IDLE: Duration = Duration::from_secs(1);

/// How many history rows one step packs; about a millisecond under the lock, so a keystroke's echo barely waits.
const PACK_STEP: usize = 2000;

/// Writes waiting for the PTY beyond which the emulator's answers to the program are dropped: a program that leaves
/// this many unread does not read its input, and output asking for answers would otherwise grow the queue without end.
const MAX_QUEUED_WRITES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    Open,
    Closed,
}

/// The reader thread's state.
pub(crate) struct Reader {
    pty: Pty,
    shared: Arc<Shared>,
    inbox: Receiver<Vec<u8>>,
    poller: Arc<Poller>,
    scanner: Scanner,
    pending: VecDeque<Vec<u8>>,
    written: usize,
    /// Since when the program has been quiet, while there may be history left to pack.
    quiet_since: Option<Instant>,
}

impl Reader {
    pub(crate) fn new(pty: Pty, shared: Arc<Shared>, inbox: Receiver<Vec<u8>>, poller: Arc<Poller>) -> Reader {
        Reader {
            pty,
            shared,
            inbox,
            poller,
            scanner: Scanner::new(),
            pending: VecDeque::new(),
            written: 0,
            quiet_since: None,
        }
    }

    /// Runs the reader on a thread of its own until the program ends, then reports that and forgets it.
    pub(crate) fn start(self, supervisor: Supervisor, process: ProcessId) -> std::io::Result<()> {
        std::thread::Builder::new()
            .name("terminal-reader".into())
            .spawn(move || {
                ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
                let mut reader = self;
                let status = reader.serve();
                reader.finish(status, &supervisor, process);
            })
            .map(drop)
    }

    fn serve(&mut self) -> Option<ExitStatus> {
        let mut interest = Interest::readable(PTY_KEY);
        let mut ended_early = match self.pty.register(&self.poller, interest, CHILD_KEY) {
            Ok(watched) => !watched,
            Err(_) => {
                self.pty.deregister(&self.poller);
                return None;
            }
        };
        let mut events = Events::with_capacity(NonZeroUsize::MIN.saturating_add(15));
        let mut buffer = vec![0u8; READ_BUFFER];
        let mut registered = (interest.readable, interest.writable);
        let status = loop {
            let deadline = self.sync_deadline();
            let packing = self.quiet_since.map(|since| since + IDLE);
            let wake_at = [deadline, packing].into_iter().flatten().min();
            let timeout = wake_at.map(|at| at.saturating_duration_since(Instant::now()));
            events.clear();
            if let Err(error) = self.poller.wait(&mut events, timeout) {
                if error.kind() == ErrorKind::Interrupted {
                    continue;
                }
                break None;
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                self.end_synchronized_update();
            }
            if packing.is_some_and(|at| Instant::now() >= at) {
                self.pack_history();
            }
            while let Ok(bytes) = self.inbox.try_recv() {
                self.pending.push_back(bytes);
            }
            let mut exited = None;
            if std::mem::take(&mut ended_early) || events.iter().any(|event| event.key == CHILD_KEY) {
                exited = self.pty.exit_status().map(Some);
            }
            for event in events.iter() {
                if event.key == PTY_KEY
                    && !event.is_interrupt()
                    && event.readable
                    && self.read(&mut buffer) == Output::Closed
                {
                    interest.readable = false;
                }
            }
            if let Some(status) = exited {
                // The program's last words may still be in the PTY.
                self.read(&mut buffer);
                break status;
            }
            self.flush();
            interest.writable = !self.pending.is_empty();
            if (interest.readable, interest.writable) != registered {
                registered = (interest.readable, interest.writable);
                if self.pty.reregister(&self.poller, interest).is_err() {
                    break None;
                }
            }
        };
        self.pty.deregister(&self.poller);
        status
    }

    fn finish(self, status: Option<ExitStatus>, supervisor: &Supervisor, process: ProcessId) {
        let ending = match status {
            Some(status) => match (status.code(), status.signal()) {
                (Some(code), _) => Ending::Code(code),
                (None, Some(signal)) => Ending::Signal(signal),
                (None, None) => Ending::Unknown,
            },
            None => Ending::Unknown,
        };
        self.shared.running.store(false, std::sync::atomic::Ordering::Release);
        self.shared.wake();
        self.shared.emit(Event::Exited(ending));
        // Dropping the PTY hangs up and waits for the program, which has ended already unless reading failed.
        drop(self.pty);
        supervisor.forget(process);
    }

    /// Reads what the PTY has, parsing it into the screen and the scanner; whether the PTY can still be read.
    fn read(&mut self, buffer: &mut [u8]) -> Output {
        let shared = Arc::clone(&self.shared);
        let lease = shared.screen.lease();
        let mut terminal = None;
        let mut unprocessed = 0;
        let mut processed = 0;
        let mut output = Output::Open;
        let mut bridging = None;
        loop {
            let mut caught_up = false;
            match self.pty.file().read(&mut buffer[unprocessed..]) {
                Ok(0) => output = Output::Closed,
                Ok(count) => unprocessed += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == ErrorKind::WouldBlock => caught_up = true,
                // EIO once the other side is gone.
                Err(_) => output = Output::Closed,
            }
            let stop = caught_up || output == Output::Closed;
            if unprocessed > 0 {
                let screen = match &mut terminal {
                    Some(screen) => screen,
                    None => match shared.screen.try_lock_unfair() {
                        Some(screen) => terminal.insert(screen),
                        None if stop || unprocessed == buffer.len() => terminal.insert(shared.screen.lock_unfair()),
                        None => continue,
                    },
                };
                let chunk = &buffer[..unprocessed];
                screen.terminal.advance(chunk);
                self.scanner.feed(chunk);
                processed += unprocessed;
                unprocessed = 0;
            }
            if processed >= MAX_LOCKED_READ {
                break;
            }
            // A program redrawing its screen, as htop does, writes it in dozens of small pieces microseconds apart;
            // shown between two of them, half the screen flashes, so the lease keeps the view waiting for the rest.
            let redrawing = caught_up
                && terminal
                    .as_ref()
                    .is_some_and(|screen| screen.terminal.modes().contains(Modes::ALT_SCREEN))
                && more_within_budget(self.pty.file(), &mut bridging);
            if stop && !redrawing {
                break;
            }
        }
        let show = terminal.as_mut().is_none_or(|screen| {
            if screen.terminal.modes().contains(Modes::SYNC) {
                let since = *screen.sync_since.get_or_insert_with(Instant::now);
                since.elapsed() >= SYNC_TIMEOUT
            } else {
                screen.sync_since = None;
                true
            }
        });
        drop(terminal);
        drop(lease);
        if processed > 0 {
            self.quiet_since = Some(Instant::now());
            if show {
                self.shared.wake();
            }
            self.dispatch();
        }
        output
    }

    /// When a synchronized update must show even though the program has not ended it.
    fn sync_deadline(&self) -> Option<Instant> {
        self.shared.screen.lock().sync_since.map(|since| since + SYNC_TIMEOUT)
    }

    fn end_synchronized_update(&mut self) {
        self.shared.screen.lock().sync_since = None;
        self.shared.wake();
        self.dispatch();
    }

    /// Packs a step of the history; once nothing is left, waits for the program's next output.
    fn pack_history(&mut self) {
        if self.shared.screen.lock().terminal.pack_history(PACK_STEP) > 0 {
            return;
        }
        self.quiet_since = None;
    }

    /// Acts on what the emulator and the scanner reported while the screen was locked.
    fn dispatch(&mut self) {
        let reported = self.shared.screen.lock().terminal.take_events();
        for event in coalesce(reported) {
            self.on_term_event(event);
        }
        for event in self.scanner.take_events() {
            self.shared.emit(event);
        }
    }

    fn on_term_event(&mut self, event: TermEvent) {
        let shared = &self.shared;
        match event {
            TermEvent::Reply(bytes) => self.answer(bytes),
            TermEvent::Title(title) => shared.emit(Event::Title(title)),
            TermEvent::Bell => shared.emit(Event::Bell),
            TermEvent::ClipboardStore(target, text) => shared.emit(Event::ClipboardStore {
                clipboard: clipboard(target),
                text,
            }),
            TermEvent::ClipboardLoad(target, terminator) => {
                let reply: ClipboardReply = Arc::new(move |text: &str| {
                    format!("\x1b]52;{};{}{terminator}", char::from(target), base64(text.as_bytes()))
                });
                *shared.clipboard_reply.lock_ignoring_poison() = Some(reply);
                shared.emit(Event::ClipboardLoad(clipboard(target)));
            }
            TermEvent::TextAreaSizeRequest => {
                let (width, height) = shared.size.lock_ignoring_poison().pixels();
                self.answer(format!("\x1b[4;{height};{width}t").into_bytes());
            }
        }
    }

    /// Queues the emulator's answer to the program.
    fn answer(&mut self, bytes: Vec<u8>) {
        if self.pending.len() < MAX_QUEUED_WRITES {
            self.pending.push_back(bytes);
        }
    }

    /// Writes as much pending input as the PTY takes without blocking.
    fn flush(&mut self) {
        while let Some(front) = self.pending.front() {
            match self.pty.file().write(&front[self.written..]) {
                Ok(0) => break,
                Ok(count) => {
                    self.written += count;
                    if self.written >= front.len() {
                        self.pending.pop_front();
                        self.written = 0;
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.pending.clear();
                    self.written = 0;
                }
            }
        }
    }
}

fn clipboard(target: u8) -> Clipboard {
    if target == b'c' {
        Clipboard::Clipboard
    } else {
        Clipboard::Selection
    }
}

/// `bytes` in standard base64, as OSC 52 carries them.
fn base64(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk.iter().fold(0u32, |acc, &b| (acc << 8) | u32::from(b)) << (8 * (3 - chunk.len()));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(
                    DIGITS[usize::try_from((value >> (18 - 6 * index)) & 63).unwrap_or(0)],
                ));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// What the emulator reported while parsing one read of the output, without what a later report of the same read
/// makes moot: the bells after the first, every title but the last, and every write to a clipboard but the last one
/// to it unless the program also asked to read one. Output that repeats these would otherwise send the main thread
/// one event apiece.
fn coalesce(reported: Vec<TermEvent>) -> Vec<TermEvent> {
    let reads = reported
        .iter()
        .any(|event| matches!(event, TermEvent::ClipboardLoad(..)));
    let mut later_title = false;
    let mut later_stores: Vec<Clipboard> = Vec::new();
    let mut kept = Vec::with_capacity(reported.len());
    for event in reported.into_iter().rev() {
        let moot = match &event {
            TermEvent::Title(_) => std::mem::replace(&mut later_title, true),
            TermEvent::ClipboardStore(target, _) if !reads => {
                let target = clipboard(*target);
                let seen = later_stores.contains(&target);
                if !seen {
                    later_stores.push(target);
                }
                seen
            }
            _ => false,
        };
        if !moot {
            kept.push(event);
        }
    }
    kept.reverse();
    let mut rung = false;
    kept.retain(|event| !matches!(event, TermEvent::Bell) || !std::mem::replace(&mut rung, true));
    kept
}

/// Whether more of a redraw came before the batch's budget, counted from its first wait in `started`, ran out.
fn more_within_budget(pty: &std::fs::File, started: &mut Option<Instant>) -> bool {
    let started = *started.get_or_insert_with(Instant::now);
    started.elapsed() < BRIDGE_BUDGET && crate::sys::wait_readable(pty, BRIDGE_WAIT_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(events: &[TermEvent]) -> Vec<String> {
        events
            .iter()
            .map(|event| match event {
                TermEvent::Title(Some(title)) => format!("title {title}"),
                TermEvent::Title(None) => "reset".to_string(),
                TermEvent::Bell => "bell".to_string(),
                TermEvent::ClipboardStore(target, text) => format!("store {:?} {text}", clipboard(*target)),
                TermEvent::ClipboardLoad(target, _) => format!("load {:?}", clipboard(*target)),
                _ => "other".to_string(),
            })
            .collect()
    }

    fn store(target: u8, text: &str) -> TermEvent {
        TermEvent::ClipboardStore(target, text.to_string())
    }

    #[test]
    fn one_read_keeps_the_first_bell_the_last_title_and_the_last_write_to_each_clipboard() {
        let reported = vec![
            TermEvent::Bell,
            TermEvent::Title(Some("one".into())),
            store(b'c', "a"),
            TermEvent::Reply(b"x".to_vec()),
            TermEvent::Bell,
            store(b's', "s"),
            TermEvent::Title(None),
            TermEvent::Title(Some("two".into())),
            store(b'c', "b"),
            TermEvent::Bell,
        ];
        assert_eq!(
            names(&coalesce(reported)),
            ["bell", "other", "store Selection s", "title two", "store Clipboard b"]
        );
        assert_eq!(names(&coalesce(vec![TermEvent::Title(None)])), ["reset"]);
    }

    #[test]
    fn writes_to_a_clipboard_all_stay_when_the_program_also_reads_one() {
        let reported = vec![
            store(b'c', "a"),
            TermEvent::ClipboardLoad(b'c', "\x07"),
            store(b'c', "b"),
        ];
        assert_eq!(
            names(&coalesce(reported)),
            ["store Clipboard a", "load Clipboard", "store Clipboard b"]
        );
    }

    fn pipe() -> (std::fs::File, std::io::PipeWriter) {
        let (reader, writer) = std::io::pipe().expect("a pipe");
        (std::fs::File::from(std::os::fd::OwnedFd::from(reader)), writer)
    }

    #[test]
    fn a_redraw_waits_for_its_next_piece_only_within_the_budget() {
        let (reader, mut writer) = pipe();
        let mut started = None;
        let late = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_micros(200));
            writer.write_all(b"x").expect("the pipe takes the piece");
            writer
        });
        assert!(more_within_budget(&reader, &mut started), "the next piece came in time");
        let _writer = late.join().expect("the writer ends");

        let (reader, _writer) = pipe();
        let mut started = None;
        let asked = Instant::now();
        assert!(!more_within_budget(&reader, &mut started), "nothing came");
        assert!(asked.elapsed() < BRIDGE_BUDGET * 10, "{:?}", asked.elapsed());

        let mut spent = Instant::now().checked_sub(BRIDGE_BUDGET);
        let asked = Instant::now();
        assert!(!more_within_budget(&reader, &mut spent), "the budget is spent");
        assert!(asked.elapsed() < Duration::from_millis(1), "{:?}", asked.elapsed());
    }

    #[test]
    fn base64_pads_as_osc_52_expects() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64("ü!".as_bytes()), "w7wh");
    }
}
