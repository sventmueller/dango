//! The streams, the writer queue, and the table of calls waiting for an answer, which the shared timer expires.

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use ide_core::LockIgnoringPoison;
use ide_process::framing;

/// Frames that may wait for the writer; more means the other side stopped reading.
const QUEUE: usize = 4096;

/// The number of a call or message of one transport, which its answer carries back; never reused while the transport
/// runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(i64);

impl RequestId {
    /// The id an answer named by `number` on the wire.
    pub const fn new(number: i64) -> RequestId {
        RequestId(number)
    }

    /// The number the id goes on the wire as.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// What waits for an answer; exactly one of its ends happens: the receiver [`take`](Transport::take)s it, it
/// expires, or the transport closes, which a failed write to the other side does too.
pub trait Call: Send + Sized + 'static {
    /// The transport closed before an answer came.
    fn closed(self);
    /// The deadline passed before an answer came; the call `id` is already off the table.
    fn expired(self, id: RequestId, transport: &Transport<Self>);
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

struct Pending<C> {
    calls: HashMap<RequestId, C>,
    closed: bool,
}

/// A frame the writer builds itself, in its turn; `None` writes nothing.
pub(crate) type Build = Box<dyn FnOnce() -> Option<Vec<u8>> + Send>;

enum Out {
    Frame(Vec<u8>),
    Built(Build),
    EndOfInput,
}

/// One transport.
pub struct Transport<C> {
    out: SyncSender<Out>,
    /// Set when the other side's input ends; the writer sees it even when the queue was too full for `EndOfInput`.
    ending: Arc<AtomicBool>,
    pending: Mutex<Pending<C>>,
    next: AtomicI64,
    /// The transport itself, for the timers of its calls, which must not keep it alive.
    me: Weak<Transport<C>>,
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
        let ending = Arc::new(AtomicBool::new(false));
        let ends = Arc::clone(&ending);
        let transport = Arc::new_cyclic(|me| Transport {
            out,
            ending,
            pending: Mutex::new(Pending {
                calls: HashMap::new(),
                closed: false,
            }),
            next: AtomicI64::new(1),
            me: me.clone(),
        });
        let writer_name = format!("{protocol}-write-{label}");
        let failed = Weak::clone(&transport.me);
        let started = std::thread::Builder::new().name(writer_name.clone()).spawn(move || {
            ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
            if write_all(output, &queue, &ends, &writer_name).is_err()
                && let Some(transport) = failed.upgrade()
            {
                transport.close();
            }
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
    pub fn next_id(&self) -> RequestId {
        RequestId(self.next.fetch_add(1, Ordering::Relaxed))
    }

    /// Queues `frame` and keeps `call` until its answer, `timeout` or the end of the transport. When this returns an
    /// error, `call` is dropped without any of its ends.
    pub fn call(&self, id: RequestId, frame: Vec<u8>, timeout: Duration, call: C) -> Result<(), SendError> {
        {
            let mut pending = self.lock();
            if pending.closed {
                return Err(SendError::Closed);
            }
            pending.calls.insert(id, call);
        }
        if let Err(error) = self.send(frame)
            && self.lock().calls.remove(&id).is_some()
        {
            return Err(error);
        }
        let transport = Weak::clone(&self.me);
        ide_core::timer::after(timeout, move || {
            if let Some(transport) = transport.upgrade() {
                transport.expire(id);
            }
        });
        Ok(())
    }

    /// Queues `frame` without waiting for anything.
    pub fn send(&self, frame: Vec<u8>) -> Result<(), SendError> {
        self.queue(Out::Frame(frame))
    }

    /// Queues a frame that `build` makes on the writer thread, in order with every other frame: for a large one,
    /// such as a whole document, which the caller should not spend the time on.
    pub fn send_built(&self, build: Build) -> Result<(), SendError> {
        self.queue(Out::Built(build))
    }

    fn queue(&self, out: Out) -> Result<(), SendError> {
        if self.ending.load(Ordering::Acquire) {
            return Err(SendError::Closed);
        }
        match self.out.try_send(out) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(SendError::Congested),
            Err(TrySendError::Disconnected(_)) => Err(SendError::Closed),
        }
    }

    /// The call `id` off the table, to answer or cancel it; `None` when it already ended.
    pub fn take(&self, id: RequestId) -> Option<C> {
        self.lock().calls.remove(&id)
    }

    /// Closes the other side's input once everything queued is written.
    pub fn end_input(&self) {
        self.ending.store(true, Ordering::Release);
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
            pending.calls.drain().map(|(_, call)| call).collect()
        };
        self.end_input();
        for call in calls {
            contained(|| call.closed());
        }
    }

    /// Expires the call `id` when it still waits for its answer.
    fn expire(&self, id: RequestId) {
        if let Some(call) = self.take(id) {
            contained(|| call.expired(id, self));
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

/// Writes what is queued until the input ends; an error when the output failed, so the transport closes.
fn write_all(
    mut output: impl Write,
    queue: &std::sync::mpsc::Receiver<Out>,
    ending: &AtomicBool,
    name: &str,
) -> io::Result<()> {
    while let Ok(out) = queue.recv() {
        if !write_out(&mut output, out, name)? {
            return Ok(());
        }
        // The flag is set before `EndOfInput` is offered, so a queue that was full for it still holds this frame.
        if ending.load(Ordering::Acquire) {
            while let Ok(out) = queue.try_recv() {
                if !write_out(&mut output, out, name)? {
                    return Ok(());
                }
            }
            return Ok(());
        }
    }
    Ok(())
}

/// Writes one queued item; whether the writer goes on.
fn write_out(output: &mut impl Write, out: Out, name: &str) -> io::Result<bool> {
    match out {
        Out::Frame(bytes) => write_frame(output, &bytes, name).map(|()| true),
        Out::Built(build) => {
            let mut built = None;
            contained(|| built = build());
            built
                .map_or(Ok(()), |bytes| write_frame(output, &bytes, name))
                .map(|()| true)
        }
        Out::EndOfInput => Ok(false),
    }
}

/// Writes one frame.
fn write_frame(output: &mut impl Write, bytes: &[u8], name: &str) -> io::Result<()> {
    output
        .write_all(bytes)
        .and_then(|()| output.flush())
        .inspect_err(|error| {
            if error.kind() != io::ErrorKind::BrokenPipe {
                ide_core::diag::report(name, error);
            }
        })
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
        Expired(RequestId),
        Closed,
    }

    struct Probe(Sender<End>);

    impl Call for Probe {
        fn closed(self) {
            let _ = self.0.send(End::Closed);
        }
        fn expired(self, id: RequestId, transport: &Transport<Self>) {
            let _ = transport.send(framing::frame(format!("cancel {}", id.get()).as_bytes()));
            let _ = self.0.send(End::Expired(id));
        }
    }

    /// Answers the call whose number is the body's first byte; a body starting with `!` panics.
    struct ByFirstByte(Sender<()>);

    impl Receiver<Probe> for ByFirstByte {
        fn receive(&self, transport: &Arc<Transport<Probe>>, body: &[u8]) {
            assert_ne!(body.first(), Some(&b'!'), "a receiver that panics");
            if let Some(probe) = transport.take(RequestId::new(i64::from(body[0]))) {
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
    fn a_panicking_builder_skips_only_its_frame_and_the_writer_continues() {
        let mut bytes = Vec::new();
        let broken = Out::Built(Box::new(|| panic!("builder failed")));
        assert!(write_out(&mut bytes, broken, "test").unwrap());
        assert!(bytes.is_empty());
        let frame = framing::frame(b"next");
        assert!(write_out(&mut bytes, Out::Frame(frame.clone()), "test").unwrap());
        assert_eq!(bytes, frame);
        assert!(!write_out(&mut bytes, Out::EndOfInput, "test").unwrap());
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
        assert_eq!(peer.next(), format!("cancel {}", id.get()).as_bytes());
        peer.send(&[u8::try_from(id.get()).unwrap(), b'x']);
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
    fn the_writer_ends_after_the_queued_frames_when_the_queue_was_too_full_for_the_end() {
        let (out, queue) = sync_channel(2);
        out.try_send(Out::Frame(b"a".to_vec())).unwrap();
        out.try_send(Out::Frame(b"b".to_vec())).unwrap();
        assert!(out.try_send(Out::EndOfInput).is_err());
        let ending = AtomicBool::new(true);
        let mut written = Vec::new();
        write_all(&mut written, &queue, &ending, "test").unwrap();
        assert_eq!(written, b"ab");
    }

    #[test]
    fn a_frame_built_by_the_writer_keeps_its_place_in_the_queue() {
        let (transport, mut peer, _) = connect();
        transport.send(framing::frame(b"first")).unwrap();
        transport
            .send_built(Box::new(|| Some(framing::frame(b"built"))))
            .unwrap();
        transport.send_built(Box::new(|| None)).unwrap();
        transport.send(framing::frame(b"last")).unwrap();
        assert_eq!(peer.next(), b"first");
        assert_eq!(peer.next(), b"built");
        assert_eq!(peer.next(), b"last");
    }

    #[test]
    fn nothing_is_sent_once_the_transport_closed() {
        let (transport, _peer, _) = connect();
        transport.close();
        assert_eq!(transport.send(framing::frame(b"late")), Err(SendError::Closed));
    }

    #[test]
    fn closing_twice_and_then_the_end_of_the_stream_end_each_waiting_call_once() {
        let (transport, mut peer, ends) = connect();
        let (tx, rx) = channel();
        let id = transport.next_id();
        transport.call(id, framing::frame(b"a"), WAIT, Probe(tx)).unwrap();
        transport.close();
        transport.close();
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), End::Closed);
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(!transport.is_open());
        assert_eq!(peer.next(), b"a");
        peer.to_us = None;
        ends.recv_timeout(WAIT).unwrap();
        assert!(ends.recv_timeout(Duration::from_millis(100)).is_err());
        assert_eq!(transport.send(framing::frame(b"late")), Err(SendError::Closed));
    }

    #[test]
    fn an_output_that_fails_closes_its_call_at_once_and_refuses_what_follows() {
        let (input, to_us) = std::io::pipe().unwrap();
        let output = ide_core::testing::FailingWriter::after(0, io::ErrorKind::BrokenPipe);
        let (ended, _ends) = channel();
        let transport = Transport::start(input, output.clone(), Arc::new(ByFirstByte(ended)), "rpc", "test").unwrap();
        let (tx, rx) = channel();
        let id = transport.next_id();
        transport
            .call(id, framing::frame(b"lost"), Duration::from_secs(600), Probe(tx.clone()))
            .unwrap();
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), End::Closed);
        assert!(!transport.is_open());
        assert_eq!(
            transport.call(transport.next_id(), framing::frame(b"next"), WAIT, Probe(tx)),
            Err(SendError::Closed)
        );
        assert_eq!(transport.send(framing::frame(b"next")), Err(SendError::Closed));
        assert!(output.taken().is_empty());
        drop(to_us);
    }

    #[test]
    fn a_congested_writer_refuses_calls_at_once_and_keeps_the_transport() {
        let (input, _to_us) = std::io::pipe().unwrap();
        let (_unread, output) = std::io::pipe().unwrap();
        let (ended, _ends) = channel();
        let transport = Transport::start(input, output, Arc::new(ByFirstByte(ended)), "rpc", "test").unwrap();
        let frame = framing::frame(&[b'x'; 1024]);
        let mut sent = 0;
        while transport.send(frame.clone()) == Ok(()) {
            sent += 1;
            assert!(sent < 100_000, "the queue never filled");
        }
        assert!(sent >= QUEUE);
        let (tx, rx) = channel();
        let id = transport.next_id();
        assert_eq!(transport.call(id, frame, WAIT, Probe(tx)), Err(SendError::Congested));
        assert!(transport.take(id).is_none());
        assert!(rx.try_recv().is_err());
        assert!(transport.is_open());
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
