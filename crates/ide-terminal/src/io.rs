//! The reader thread: PTY output into the emulator and the scanner, typed input and the emulator's replies out.
//!
//! This is alacritty's event loop with one addition it has no room for: every chunk also goes through `Scanner`,
//! which needs the raw bytes for OSC 7, OSC 133 and server addresses. Its locking is kept: the thread reserves the
//! screen for a batch, parses at most `MAX_LOCKED_READ` bytes under the lock and keeps reading into its buffer while
//! the view holds the lock, so neither side waits long for the other.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::num::NonZeroUsize;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use ide_core::LockIgnoringPoison;
use std::time::Instant;

use alacritty_terminal::event::Event as TermEvent;
use alacritty_terminal::term::color::COUNT;
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
use alacritty_terminal::vte::ansi::{Processor, Rgb as TermRgb};
use ide_process::{Ending, ProcessId, Supervisor};
use polling::{Event as Interest, Events, PollMode, Poller};

use crate::event::{Clipboard, Event};
use crate::scan::Scanner;
use crate::session::{Shared, TermEvents};
use crate::sys;

/// The keys alacritty's tty registers the PTY and its child-exit signal under.
const PTY_KEY: usize = 0;
const CHILD_KEY: usize = 1;

/// Bytes read before the screen must take them, even if the view holds it.
const READ_BUFFER: usize = 0x10_0000;

/// Bytes parsed under one hold of the lock.
const MAX_LOCKED_READ: usize = u16::MAX as usize;

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
    term_events: Arc<Mutex<Vec<TermEvent>>>,
    parser: Processor,
    scanner: Scanner,
    pending: VecDeque<Vec<u8>>,
    written: usize,
}

impl Reader {
    pub(crate) fn new(
        pty: Pty,
        shared: Arc<Shared>,
        inbox: Receiver<Vec<u8>>,
        poller: Arc<Poller>,
        term_events: TermEvents,
    ) -> Reader {
        Reader {
            pty,
            shared,
            inbox,
            poller,
            term_events: term_events.0,
            parser: Processor::new(),
            scanner: Scanner::new(),
            pending: VecDeque::new(),
            written: 0,
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
        if sys::register(&mut self.pty, &self.poller, interest).is_err() {
            return None;
        }
        let mut events = Events::with_capacity(NonZeroUsize::MIN.saturating_add(15));
        let mut buffer = vec![0u8; READ_BUFFER];
        let mut registered = (interest.readable, interest.writable);
        let status = loop {
            let deadline = self.parser.sync_timeout().sync_timeout();
            let timeout = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
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
            while let Ok(bytes) = self.inbox.try_recv() {
                self.pending.push_back(bytes);
            }
            let mut exited = None;
            for event in events.iter() {
                if event.key == CHILD_KEY {
                    if let Some(ChildEvent::Exited(status)) = self.pty.next_child_event() {
                        exited = Some(status);
                    }
                } else if event.key == PTY_KEY
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
                if self.pty.reregister(&self.poller, interest, PollMode::Level).is_err() {
                    break None;
                }
            }
        };
        let _ = self.pty.deregister(&self.poller);
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
        let lease = shared.term.lease();
        let mut terminal = None;
        let mut unprocessed = 0;
        let mut processed = 0;
        let mut output = Output::Open;
        loop {
            let mut caught_up = false;
            match self.pty.reader().read(&mut buffer[unprocessed..]) {
                Ok(0) => output = Output::Closed,
                Ok(count) => unprocessed += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == ErrorKind::WouldBlock => caught_up = true,
                // EIO once the other side is gone.
                Err(_) => output = Output::Closed,
            }
            let stop = caught_up || output == Output::Closed;
            if unprocessed == 0 {
                if stop {
                    break;
                }
                continue;
            }
            let term = match &mut terminal {
                Some(term) => term,
                None => match shared.term.try_lock_unfair() {
                    Some(term) => terminal.insert(term),
                    None if stop || unprocessed == buffer.len() => terminal.insert(shared.term.lock_unfair()),
                    None => continue,
                },
            };
            let chunk = &buffer[..unprocessed];
            self.parser.advance(&mut **term, chunk);
            self.scanner.feed(chunk);
            processed += unprocessed;
            unprocessed = 0;
            if stop || processed >= MAX_LOCKED_READ {
                break;
            }
        }
        drop(terminal);
        drop(lease);
        if processed > 0 {
            if self.parser.sync_bytes_count() < processed {
                self.shared.wake();
            }
            self.dispatch();
        }
        output
    }

    fn end_synchronized_update(&mut self) {
        self.parser.stop_sync(&mut *self.shared.term.lock());
        self.shared.wake();
        self.dispatch();
    }

    /// Acts on what the emulator and the scanner reported while the screen was locked.
    fn dispatch(&mut self) {
        let reported = std::mem::take(&mut *self.term_events.lock_ignoring_poison());
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
            TermEvent::Title(title) => shared.emit(Event::Title(Some(title))),
            TermEvent::ResetTitle => shared.emit(Event::Title(None)),
            TermEvent::Bell => shared.emit(Event::Bell),
            TermEvent::ClipboardStore(target, text) => shared.emit(Event::ClipboardStore {
                clipboard: clipboard(&target),
                text,
            }),
            TermEvent::ClipboardLoad(target, reply) => {
                *shared.clipboard_reply.lock_ignoring_poison() = Some(reply);
                shared.emit(Event::ClipboardLoad(clipboard(&target)));
            }
            TermEvent::ColorRequest(index, reply) => {
                let color = self.color(index);
                self.answer(reply(color).into_bytes());
            }
            TermEvent::PtyWrite(text) => self.answer(text.into_bytes()),
            TermEvent::TextAreaSizeRequest(_) => {
                // Answered here because alacritty's own answer multiplies in 16 bits.
                let (width, height) = shared.size.lock_ignoring_poison().pixels();
                self.answer(format!("\x1b[4;{height};{width}t").into_bytes());
            }
            TermEvent::Wakeup | TermEvent::CursorBlinkingChange | TermEvent::MouseCursorDirty => shared.wake(),
            TermEvent::Exit | TermEvent::ChildExit(_) => {}
        }
    }

    /// Colour `index` as the program sees it: its own change, else the palette's.
    fn color(&self, index: usize) -> TermRgb {
        let changed = self.shared.term.lock().colors()[index.min(COUNT - 1)];
        changed.unwrap_or_else(|| {
            let table = self
                .shared
                .palette
                .lock_ignoring_poison()
                .table(&alacritty_terminal::term::color::Colors::default());
            let rgb = table[index.min(COUNT - 1)];
            TermRgb {
                r: rgb.r,
                g: rgb.g,
                b: rgb.b,
            }
        })
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
            match self.pty.writer().write(&front[self.written..]) {
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

fn clipboard(target: &alacritty_terminal::term::ClipboardType) -> Clipboard {
    match target {
        alacritty_terminal::term::ClipboardType::Clipboard => Clipboard::Clipboard,
        alacritty_terminal::term::ClipboardType::Selection => Clipboard::Selection,
    }
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
            TermEvent::Title(_) | TermEvent::ResetTitle => std::mem::replace(&mut later_title, true),
            TermEvent::ClipboardStore(target, _) if !reads => {
                let target = clipboard(target);
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

#[cfg(test)]
mod tests {
    use alacritty_terminal::term::ClipboardType;

    use super::*;

    fn names(events: &[TermEvent]) -> Vec<String> {
        events
            .iter()
            .map(|event| match event {
                TermEvent::Title(title) => format!("title {title}"),
                TermEvent::ResetTitle => "reset".to_string(),
                TermEvent::Bell => "bell".to_string(),
                TermEvent::ClipboardStore(target, text) => format!("store {:?} {text}", clipboard(target)),
                TermEvent::ClipboardLoad(target, _) => format!("load {:?}", clipboard(target)),
                _ => "other".to_string(),
            })
            .collect()
    }

    fn store(target: ClipboardType, text: &str) -> TermEvent {
        TermEvent::ClipboardStore(target, text.to_string())
    }

    #[test]
    fn one_read_keeps_the_first_bell_the_last_title_and_the_last_write_to_each_clipboard() {
        let reported = vec![
            TermEvent::Bell,
            TermEvent::Title("one".into()),
            store(ClipboardType::Clipboard, "a"),
            TermEvent::Wakeup,
            TermEvent::Bell,
            store(ClipboardType::Selection, "s"),
            TermEvent::ResetTitle,
            TermEvent::Title("two".into()),
            store(ClipboardType::Clipboard, "b"),
            TermEvent::Bell,
        ];
        assert_eq!(
            names(&coalesce(reported)),
            ["bell", "other", "store Selection s", "title two", "store Clipboard b"]
        );
        assert_eq!(names(&coalesce(vec![TermEvent::ResetTitle])), ["reset"]);
    }

    #[test]
    fn writes_to_a_clipboard_all_stay_when_the_program_also_reads_one() {
        let load = TermEvent::ClipboardLoad(ClipboardType::Clipboard, Arc::new(|text: &str| text.to_string()));
        let reported = vec![
            store(ClipboardType::Clipboard, "a"),
            load,
            store(ClipboardType::Clipboard, "b"),
        ];
        assert_eq!(
            names(&coalesce(reported)),
            ["store Clipboard a", "load Clipboard", "store Clipboard b"]
        );
    }
}
