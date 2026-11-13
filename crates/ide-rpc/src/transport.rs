//! The streams, the writer queue, the table of calls waiting for an answer, and the timer that expires them.

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ide_core::{IgnoringPoison, LockIgnoringPoison};
use ide_process::framing;

/// Frames that may wait for the writer; more means the other side stopped reading.
const QUEUE: usize = 4096;

/// A deadline far enough away to mean none, for timeouts that overflow `Instant`.
const NEVER: Duration = Duration::from_secs(365 * 24 * 3600);

/// What waits for an answer; exactly one of its ends happens: the receiver [`take`](Transport::take)s it, it
/// expires, or the transport closes.
pub trait Call: Send + Sized + 'static {
    /// The transport closed before an answer came.
    fn closed(self);
    /// The deadline passed before an answer came; the call `id` is already off the table.
    fn expired(self, id: i64, transport: &Transport<Self>);
}

/// Hears what the other side sends.
pub trait Receiver<C: Call>: Send + Sync + 'static {
    /// One frame's body, in the order the frames came.
    fn receive(&self, transport: &Arc<Transport<C>>, body: &[u8]);
    /// The stream ended or broke; every waiting call has been closed.
    fn ended(&self);
}

/// Why a frame was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// The writer queue is full: the other side stopped reading.
    Congested,
    /// The transport is closed.
    Closed,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SendError::Congested => "the other side stopped reading",
            SendError::Closed => "the connection is closed",
        })
    }
}

impl std::error::Error for SendError {}

struct Waiting<C> {
    call: C,
    deadline: Instant,
}

struct Pending<C> {
    calls: HashMap<i64, Waiting<C>>,
    closed: bool,
}

enum Out {
    Frame(Vec<u8>),
    EndOfInput,
}

/// One transport.
pub struct Transport<C> {
    out: SyncSender<Out>,
    pending: Mutex<Pending<C>>,
    wake: Condvar,
    next: AtomicI64,
}

impl<C: Call> Transport<C> {
    /// Starts reading `input` and writing `output`; `receiver` hears every frame. The threads are named like
    /// `lsp-read-gopls`, after `protocol` and `label`.
    pub fn start(
        input: impl Read + Send + 'static,
        output: impl Write + Send + 'static,
        receiver: Arc<dyn Receiver<C>>,
        protocol: &str,
        label: &str,
    ) -> io::Result<Arc<Transport<C>>> {
        let (out, queue) = sync_channel(QUEUE);
        let transport = Arc::new(Transport {
            out,
            pending: Mutex::new(Pending {
                calls: HashMap::new(),
                closed: false,
            }),
            wake: Condvar::new(),
            next: AtomicI64::new(1),
        });
        let writer_name = format!("{protocol}-write-{label}");
        let writer = std::thread::Builder::new().name(writer_name.clone()).spawn(move || {
            ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
            write_all(output, &queue, &writer_name);
        });
        let timer = Arc::clone(&transport);
        let started = writer.and_then(|_| {
            std::thread::Builder::new()
                .name(format!("{protocol}-timer-{label}"))
                .spawn(move || timer.expire())
        });
        let reader = Arc::clone(&transport);
        let read_name = format!("{protocol}-read-{label}");
        let started = started.and_then(|_| {
            std::thread::Builder::new().name(read_name.clone()).spawn(move || {
                ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
                read_all(input, &reader, &*receiver, &read_name);
            })
        });
        if let Err(error) = started {
            transport.close();
            return Err(error);
        }
        Ok(transport)
    }

    /// A number no other call or message of this transport has had.
    pub fn next_id(&self) -> i64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// Queues `frame` and keeps `call` until its answer, `timeout` or the end of the transport. When this returns an
    /// error, `call` is dropped without any of its ends.
    pub fn call(&self, id: i64, frame: Vec<u8>, timeout: Duration, call: C) -> Result<(), SendError> {
        {
            let mut pending = self.lock();
            if pending.closed {
                return Err(SendError::Closed);
            }
            let deadline = Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(|| Instant::now() + NEVER);
            pending.calls.insert(id, Waiting { call, deadline });
        }
        self.wake.notify_all();
        if let Err(error) = self.send(frame)
            && self.lock().calls.remove(&id).is_some()
        {
            return Err(error);
        }
        Ok(())
    }

    /// Queues `frame` without waiting for anything.
    pub fn send(&self, frame: Vec<u8>) -> Result<(), SendError> {
        match self.out.try_send(Out::Frame(frame)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(SendError::Congested),
            Err(TrySendError::Disconnected(_)) => Err(SendError::Closed),
        }
    }

    /// The call `id` off the table, to answer or cancel it; `None` when it already ended.
    pub fn take(&self, id: i64) -> Option<C> {
        self.lock().calls.remove(&id).map(|waiting| waiting.call)
    }

    /// Closes the other side's input once everything queued is written.
    pub fn end_input(&self) {
        let _ = self.out.try_send(Out::EndOfInput);
    }

    /// Whether the transport still carries messages.
    pub fn is_open(&self) -> bool {
        !self.lock().closed
    }

    /// Closes every waiting call and ends the other side's input; nothing more can be called.
    pub fn close(&self) {
        let calls: Vec<C> = {
            let mut pending = self.lock();
            pending.closed = true;
            pending.calls.drain().map(|(_, waiting)| waiting.call).collect()
        };
        self.wake.notify_all();
        let _ = self.out.try_send(Out::EndOfInput);
        for call in calls {
            contained(|| call.closed());
        }
    }

    /// Expires calls whose deadline passed, until the transport closes.
    fn expire(&self) {
        let mut pending = self.lock();
        loop {
            if pending.closed {
                return;
            }
            let now = Instant::now();
            let due: Vec<i64> = pending
                .calls
                .iter()
                .filter(|(_, waiting)| waiting.deadline <= now)
                .map(|(id, _)| *id)
                .collect();
            if !due.is_empty() {
                let calls: Vec<(i64, C)> = due
                    .into_iter()
                    .filter_map(|id| pending.calls.remove(&id).map(|waiting| (id, waiting.call)))
                    .collect();
                drop(pending);
                for (id, call) in calls {
                    contained(|| call.expired(id, self));
                }
                pending = self.lock();
                continue;
            }
            pending = match pending.calls.values().map(|waiting| waiting.deadline).min() {
                Some(deadline) => {
                    self.wake
                        .wait_timeout(pending, deadline.saturating_duration_since(now))
                        .ignoring_poison()
                        .0
                }
                None => self.wake.wait(pending).ignoring_poison(),
            };
        }
    }

    fn lock(&self) -> MutexGuard<'_, Pending<C>> {
        self.pending.lock_ignoring_poison()
    }
}

fn read_all<C: Call>(input: impl Read, transport: &Arc<Transport<C>>, receiver: &dyn Receiver<C>, name: &str) {
    let mut input = BufReader::with_capacity(64 * 1024, input);
    loop {
        match framing::read(&mut input) {
            Ok(Some(body)) => contained(|| receiver.receive(transport, &body)),
            Ok(None) => break,
            Err(error) => {
                ide_core::diag::report(&format!("{name}: frame"), error);
                break;
            }
        }
    }
    transport.close();
    contained(|| receiver.ended());
}

fn write_all(mut output: impl Write, queue: &std::sync::mpsc::Receiver<Out>, name: &str) {
    while let Ok(Out::Frame(bytes)) = queue.recv() {
        if let Err(error) = output.write_all(&bytes).and_then(|()| output.flush()) {
            if error.kind() != io::ErrorKind::BrokenPipe {
                ide_core::diag::report(name, error);
            }
            return;
        }
    }
}

/// Runs `f`; a panic in a caller's callback costs that callback, not the transport.
pub fn contained(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, PipeReader, PipeWriter};
    use std::sync::mpsc::{Sender, channel};

    use super::*;

    const WAIT: Duration = Duration::from_secs(5);

    #[derive(Debug, PartialEq)]
    enum End {
        Answered(Vec<u8>),
        Expired(i64),
        Closed,
    }

    struct Probe(Sender<End>);

    impl Call for Probe {
        fn closed(self) {
            let _ = self.0.send(End::Closed);
        }
        fn expired(self, id: i64, transport: &Transport<Self>) {
            let _ = transport.send(framing::frame(format!("cancel {id}").as_bytes()));
            let _ = self.0.send(End::Expired(id));
        }
    }

    /// Answers the call whose number is the body's first byte; a body starting with `!` panics.
    struct ByFirstByte(Sender<()>);

    impl Receiver<Probe> for ByFirstByte {
        fn receive(&self, transport: &Arc<Transport<Probe>>, body: &[u8]) {
            assert_ne!(body.first(), Some(&b'!'), "a receiver that panics");
            if let Some(probe) = transport.take(i64::from(body[0])) {
                let _ = probe.0.send(End::Answered(body[1..].to_vec()));
            }
        }
        fn ended(&self) {
            let _ = self.0.send(());
        }
    }

    struct Peer {
        from_us: BufReader<PipeReader>,
        to_us: Option<PipeWriter>,
    }

    impl Peer {
        fn next(&mut self) -> Vec<u8> {
            framing::read(&mut self.from_us).unwrap().unwrap()
        }
        fn send(&mut self, body: &[u8]) {
            self.to_us.as_mut().unwrap().write_all(&framing::frame(body)).unwrap();
        }
    }

    fn connect() -> (Arc<Transport<Probe>>, Peer, std::sync::mpsc::Receiver<()>) {
        let (input, to_us) = std::io::pipe().unwrap();
        let (from_us, output) = std::io::pipe().unwrap();
        let (ended, ends) = channel();
        let transport = Transport::start(input, output, Arc::new(ByFirstByte(ended)), "rpc", "test").unwrap();
        let peer = Peer {
            from_us: BufReader::new(from_us),
            to_us: Some(to_us),
        };
        (transport, peer, ends)
    }

    #[test]
    fn answers_reach_their_calls_in_any_order_and_a_panicking_receiver_costs_one_frame() {
        let (transport, mut peer, _) = connect();
        let (tx, rx) = channel();
        let (one, two) = (transport.next_id(), transport.next_id());
        transport
            .call(one, framing::frame(b"one"), WAIT, Probe(tx.clone()))
            .unwrap();
        transport.call(two, framing::frame(b"two"), WAIT, Probe(tx)).unwrap();
        assert_eq!(peer.next(), b"one");
        assert_eq!(peer.next(), b"two");
        peer.send(b"!");
        peer.send(&[2, b'b']);
        peer.send(&[1, b'a']);
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), End::Answered(vec![b'b']));
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), End::Answered(vec![b'a']));
    }

    #[test]
    fn a_late_call_expires_once_and_its_late_answer_is_ignored() {
        let (transport, mut peer, _) = connect();
        let (tx, rx) = channel();
        let id = transport.next_id();
        transport
            .call(id, framing::frame(b"slow"), Duration::from_millis(30), Probe(tx))
            .unwrap();
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), End::Expired(id));
        assert_eq!(peer.next(), b"slow");
        assert_eq!(peer.next(), format!("cancel {id}").as_bytes());
        peer.send(&[u8::try_from(id).unwrap(), b'x']);
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn the_end_of_the_stream_closes_waiting_calls_and_refuses_new_ones() {
        let (transport, mut peer, ends) = connect();
        let (tx, rx) = channel();
        let id = transport.next_id();
        transport
            .call(id, framing::frame(b"a"), WAIT, Probe(tx.clone()))
            .unwrap();
        peer.to_us = None;
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), End::Closed);
        ends.recv_timeout(WAIT).unwrap();
        assert!(!transport.is_open());
        assert_eq!(
            transport.call(transport.next_id(), framing::frame(b"b"), WAIT, Probe(tx)),
            Err(SendError::Closed)
        );
    }

    #[test]
    fn ending_the_input_writes_what_was_queued_first() {
        let (transport, mut peer, _) = connect();
        transport.send(framing::frame(b"last")).unwrap();
        transport.end_input();
        assert_eq!(peer.next(), b"last");
        let mut rest = String::new();
        assert_eq!(peer.from_us.read_line(&mut rest).unwrap(), 0);
    }

    #[test]
    fn taking_a_call_twice_finds_it_once() {
        let (transport, _peer, _) = connect();
        let (tx, _rx) = channel();
        let id = transport.next_id();
        transport.call(id, framing::frame(b"a"), WAIT, Probe(tx)).unwrap();
        assert!(transport.take(id).is_some());
        assert!(transport.take(id).is_none());
    }
}
