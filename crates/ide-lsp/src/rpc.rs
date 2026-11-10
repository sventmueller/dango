//! JSON-RPC 2.0 over a pair of byte streams: requests with deadlines and cancellation, notifications, and requests
//! from the other side.
//!
//! Three threads per connection: one reads and dispatches, one writes from a bounded queue so a caller never waits
//! on a full pipe, one fails requests whose deadline passed.

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::error::Error;
use crate::framing;

/// Messages that may wait for the writer; more means the server stopped reading.
const QUEUE: usize = 4096;

/// Called once with the answer to a request.
pub(crate) type Reply = Box<dyn FnOnce(Result<Value, Error>) + Send>;

/// What the other side sends on its own.
pub(crate) trait Handler: Send + Sync {
    fn notification(&self, method: &str, params: Value);
    fn request(&self, method: &str, params: Value, responder: Responder);
    /// The stream ended or broke; every waiting request has been failed.
    fn closed(&self);
}

struct Call {
    reply: Reply,
    deadline: Instant,
}

#[derive(Default)]
struct Pending {
    calls: HashMap<i32, Call>,
    closed: bool,
}

enum Out {
    Message(Vec<u8>),
    EndOfInput,
}

/// One JSON-RPC connection.
pub(crate) struct Connection {
    out: SyncSender<Out>,
    pending: Mutex<Pending>,
    wake: Condvar,
    next: AtomicI32,
}

impl Connection {
    /// Starts reading `input` and writing `output`; `handler` hears what the other side sends.
    pub(crate) fn start(
        input: impl Read + Send + 'static,
        output: impl Write + Send + 'static,
        handler: Arc<dyn Handler>,
        name: &str,
    ) -> io::Result<Arc<Connection>> {
        let (out, queue) = sync_channel(QUEUE);
        let connection = Arc::new(Connection {
            out,
            pending: Mutex::default(),
            wake: Condvar::new(),
            next: AtomicI32::new(1),
        });
        std::thread::Builder::new()
            .name(format!("lsp-write-{name}"))
            .spawn(move || write_all(output, &queue))?;
        let timer = connection.clone();
        std::thread::Builder::new()
            .name(format!("lsp-timer-{name}"))
            .spawn(move || timer.expire())?;
        let reader = Arc::downgrade(&connection);
        if let Err(error) = std::thread::Builder::new()
            .name(format!("lsp-read-{name}"))
            .spawn(move || read_all(input, &reader, &*handler))
        {
            connection.close();
            return Err(error);
        }
        Ok(connection)
    }

    /// Sends a request; `reply` hears the answer, an error, the timeout or the cancellation exactly once. When this
    /// returns an error, `reply` is dropped without a call.
    pub(crate) fn request(&self, method: &str, params: Value, timeout: Duration, reply: Reply) -> Result<i32, Error> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        {
            let mut pending = self.lock();
            if pending.closed {
                return Err(Error::ServerGone);
            }
            let deadline = Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(365 * 24 * 3600));
            pending.calls.insert(id, Call { reply, deadline });
        }
        self.wake.notify_all();
        if let Err(error) = self.send(&message)
            && self.lock().calls.remove(&id).is_some()
        {
            return Err(error);
        }
        Ok(id)
    }

    /// Sends a notification.
    pub(crate) fn notify(&self, method: &str, params: Value) -> Result<(), Error> {
        let mut message = json!({"jsonrpc": "2.0", "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(&message)
    }

    /// Ends request `id` with `why` and asks the other side to stop working on it.
    pub(crate) fn cancel(&self, id: i32, why: Error) {
        let call = self.lock().calls.remove(&id);
        if let Some(call) = call {
            let _ = self.notify("$/cancelRequest", json!({"id": id}));
            contained(|| (call.reply)(Err(why)));
        }
    }

    /// Closes the other side's input once everything queued is written.
    pub(crate) fn end_input(&self) {
        let _ = self.out.try_send(Out::EndOfInput);
    }

    fn send(&self, message: &Value) -> Result<(), Error> {
        let body = serde_json::to_vec(message).map_err(|e| Error::Malformed(e.to_string()))?;
        match self.out.try_send(Out::Message(framing::frame(&body))) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(Error::Congested),
            Err(TrySendError::Disconnected(_)) => Err(Error::ServerGone),
        }
    }

    fn respond(&self, id: Value, answer: Result<Value, (i64, String)>) {
        let message = match answer {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        };
        let _ = self.send(&message);
    }

    fn dispatch(self: &Arc<Self>, body: &[u8], handler: &dyn Handler) {
        let Ok(Value::Object(mut message)) = serde_json::from_slice::<Value>(body) else {
            return;
        };
        let params = message.remove("params").unwrap_or(Value::Null);
        match (message.remove("method"), message.remove("id")) {
            (Some(Value::String(method)), Some(id)) => {
                let responder = Responder {
                    id: Some(id),
                    connection: Arc::downgrade(self),
                };
                contained(|| handler.request(&method, params, responder));
            }
            (Some(Value::String(method)), None) => contained(|| handler.notification(&method, params)),
            (None, Some(id)) => self.complete(&id, message),
            _ => {}
        }
    }

    fn complete(&self, id: &Value, mut message: Map<String, Value>) {
        let Some(id) = id.as_i64().and_then(|id| i32::try_from(id).ok()) else {
            return;
        };
        let Some(call) = self.lock().calls.remove(&id) else {
            return;
        };
        let answer = match message.remove("error") {
            Some(error) => Err(Error::Server {
                code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }),
            None => Ok(message.remove("result").unwrap_or(Value::Null)),
        };
        contained(|| (call.reply)(answer));
    }

    /// Fails every waiting request; nothing more can be sent.
    fn close(&self) {
        let calls: Vec<Call> = {
            let mut pending = self.lock();
            pending.closed = true;
            pending.calls.drain().map(|(_, call)| call).collect()
        };
        self.wake.notify_all();
        let _ = self.out.try_send(Out::EndOfInput);
        for call in calls {
            contained(|| (call.reply)(Err(Error::ServerGone)));
        }
    }

    /// Fails requests whose deadline passed, until the connection closes.
    fn expire(&self) {
        let mut pending = self.lock();
        loop {
            if pending.closed {
                return;
            }
            let now = Instant::now();
            let due: Vec<i32> = pending
                .calls
                .iter()
                .filter(|(_, call)| call.deadline <= now)
                .map(|(id, _)| *id)
                .collect();
            if !due.is_empty() {
                let calls: Vec<(i32, Call)> = due
                    .into_iter()
                    .filter_map(|id| pending.calls.remove(&id).map(|call| (id, call)))
                    .collect();
                drop(pending);
                for (id, call) in calls {
                    let _ = self.notify("$/cancelRequest", json!({"id": id}));
                    contained(|| (call.reply)(Err(Error::Timeout)));
                }
                pending = self.lock();
                continue;
            }
            pending = match pending.calls.values().map(|call| call.deadline).min() {
                Some(deadline) => {
                    self.wake
                        .wait_timeout(pending, deadline.saturating_duration_since(now))
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                }
                None => self.wake.wait(pending).unwrap_or_else(|e| e.into_inner()),
            };
        }
    }

    fn lock(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Answers one request from the other side; dropped unanswered, it answers with an internal error.
pub(crate) struct Responder {
    id: Option<Value>,
    connection: Weak<Connection>,
}

impl Responder {
    pub(crate) fn ok(mut self, result: Value) {
        self.answer(Ok(result));
    }

    pub(crate) fn fail(mut self, code: i64, message: &str) {
        self.answer(Err((code, message.to_string())));
    }

    fn answer(&mut self, answer: Result<Value, (i64, String)>) {
        if let (Some(id), Some(connection)) = (self.id.take(), self.connection.upgrade()) {
            connection.respond(id, answer);
        }
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.answer(Err((-32603, "the request was not answered".to_string())));
    }
}

fn read_all(input: impl Read, connection: &Weak<Connection>, handler: &dyn Handler) {
    let mut input = BufReader::with_capacity(64 * 1024, input);
    while let Ok(Some(body)) = framing::read(&mut input) {
        let Some(connection) = connection.upgrade() else {
            return;
        };
        connection.dispatch(&body, handler);
    }
    if let Some(connection) = connection.upgrade() {
        connection.close();
    }
    contained(|| handler.closed());
}

fn write_all(mut output: impl Write, queue: &Receiver<Out>) {
    while let Ok(Out::Message(bytes)) = queue.recv() {
        if output.write_all(&bytes).and_then(|()| output.flush()).is_err() {
            return;
        }
    }
}

/// Runs `f`; a panic in a caller's callback costs that callback, not the connection.
fn contained(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, PipeReader, PipeWriter};
    use std::sync::mpsc::{Sender, channel};

    use super::*;

    const WAIT: Duration = Duration::from_secs(5);

    #[derive(Default)]
    struct Recorder {
        notifications: Mutex<Vec<(String, Value)>>,
        closed: Mutex<bool>,
    }

    impl Handler for Recorder {
        fn notification(&self, method: &str, params: Value) {
            self.notifications.lock().unwrap().push((method.to_string(), params));
        }

        fn request(&self, method: &str, params: Value, responder: Responder) {
            match method {
                "echo" => responder.ok(params),
                "ignored" => drop(responder),
                _ => responder.fail(-32601, "unknown"),
            }
        }

        fn closed(&self) {
            *self.closed.lock().unwrap() = true;
        }
    }

    /// The other end of a connection under test: what the client wrote, and a way to write to the client.
    struct Peer {
        from_client: BufReader<PipeReader>,
        to_client: Option<PipeWriter>,
    }

    impl Peer {
        fn next(&mut self) -> Value {
            serde_json::from_slice(&framing::read(&mut self.from_client).unwrap().unwrap()).unwrap()
        }

        fn send(&mut self, message: &Value) {
            let bytes = framing::frame(&serde_json::to_vec(message).unwrap());
            self.to_client.as_mut().unwrap().write_all(&bytes).unwrap();
        }
    }

    fn connect() -> (Arc<Connection>, Peer, Arc<Recorder>) {
        let (client_in, to_client) = std::io::pipe().unwrap();
        let (from_client, client_out) = std::io::pipe().unwrap();
        let recorder = Arc::new(Recorder::default());
        let connection = Connection::start(client_in, client_out, recorder.clone(), "test").unwrap();
        let peer = Peer {
            from_client: BufReader::new(from_client),
            to_client: Some(to_client),
        };
        (connection, peer, recorder)
    }

    fn collect(tx: Sender<Result<Value, Error>>) -> Reply {
        Box::new(move |answer| tx.send(answer).unwrap())
    }

    #[test]
    fn a_response_reaches_the_request_it_answers_whatever_the_order() {
        let (connection, mut peer, _) = connect();
        let (tx, rx) = channel();
        let first = connection
            .request("a", json!({"n": 1}), WAIT, collect(tx.clone()))
            .unwrap();
        let second = connection.request("b", Value::Null, WAIT, collect(tx)).unwrap();
        assert_eq!(peer.next()["params"], json!({"n": 1}));
        assert!(peer.next().get("params").is_none(), "no params rather than null");
        peer.send(&json!({"jsonrpc": "2.0", "id": second, "result": "second"}));
        peer.send(&json!({"jsonrpc": "2.0", "id": first, "error": {"code": -32801, "message": "modified"}}));
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Ok(json!("second")));
        assert_eq!(
            rx.recv_timeout(WAIT).unwrap(),
            Err(Error::Server {
                code: -32801,
                message: "modified".into()
            })
        );
    }

    #[test]
    fn a_late_request_times_out_and_the_server_is_asked_to_cancel() {
        let (connection, mut peer, _) = connect();
        let (tx, rx) = channel();
        let id = connection
            .request("slow", Value::Null, Duration::from_millis(50), collect(tx))
            .unwrap();
        assert_eq!(peer.next()["method"], "slow");
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Err(Error::Timeout));
        let cancel = peer.next();
        assert_eq!(cancel["method"], "$/cancelRequest");
        assert_eq!(cancel["params"]["id"], id);
        peer.send(&json!({"jsonrpc": "2.0", "id": id, "result": "too late"}));
        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "the reply came only once"
        );
    }

    #[test]
    fn a_cancelled_request_answers_cancelled_and_tells_the_server() {
        let (connection, mut peer, _) = connect();
        let (tx, rx) = channel();
        let id = connection.request("slow", Value::Null, WAIT, collect(tx)).unwrap();
        connection.cancel(id, Error::Cancelled);
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Err(Error::Cancelled));
        assert_eq!(peer.next()["method"], "slow");
        assert_eq!(peer.next()["method"], "$/cancelRequest");
    }

    #[test]
    fn requests_from_the_server_are_answered_even_when_the_handler_forgets() {
        let (_connection, mut peer, recorder) = connect();
        peer.send(&json!({"jsonrpc": "2.0", "id": "s1", "method": "echo", "params": [1, "ü"]}));
        assert_eq!(peer.next(), json!({"jsonrpc": "2.0", "id": "s1", "result": [1, "ü"]}));
        peer.send(&json!({"jsonrpc": "2.0", "id": 7, "method": "ignored"}));
        assert_eq!(peer.next()["error"]["code"], -32603);
        peer.send(&json!({"jsonrpc": "2.0", "id": 8, "method": "nope"}));
        assert_eq!(peer.next()["error"]["code"], -32601);
        peer.send(&json!({"jsonrpc": "2.0", "method": "note", "params": {"x": 1}}));
        peer.send(&json!({"jsonrpc": "2.0", "id": 9, "method": "echo"}));
        assert_eq!(peer.next()["result"], Value::Null);
        assert_eq!(
            recorder.notifications.lock().unwrap().as_slice(),
            &[("note".to_string(), json!({"x": 1}))]
        );
    }

    #[test]
    fn the_end_of_the_stream_fails_waiting_requests_and_refuses_new_ones() {
        let (connection, mut peer, recorder) = connect();
        let (tx, rx) = channel();
        connection.request("a", Value::Null, WAIT, collect(tx)).unwrap();
        peer.to_client = None;
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Err(Error::ServerGone));
        let deadline = Instant::now() + WAIT;
        while !*recorder.closed.lock().unwrap() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(*recorder.closed.lock().unwrap());
        let refused = connection.request("b", Value::Null, WAIT, Box::new(|_| panic!("not called")));
        assert_eq!(refused, Err(Error::ServerGone));
    }

    #[test]
    fn closing_the_input_ends_the_other_sides_stream_after_what_was_queued() {
        let (connection, mut peer, _) = connect();
        connection.notify("exit", Value::Null).unwrap();
        connection.end_input();
        assert_eq!(peer.next()["method"], "exit");
        let mut rest = String::new();
        assert_eq!(peer.from_client.read_line(&mut rest).unwrap(), 0);
    }
}
