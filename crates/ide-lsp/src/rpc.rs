//! JSON-RPC 2.0 on the shared transport (`ide-rpc`): requests with deadlines and cancellation, notifications, and
//! requests from the other side.

use std::io::{self, Read, Write};
use std::sync::{Arc, Weak};
use std::time::Duration;

use ide_process::framing;
use ide_rpc::{Call, SendError, Transport, contained};
use serde_json::{Map, Value, json};

use crate::error::Error;

/// Called once with the answer to a request.
pub(crate) type Reply = Box<dyn FnOnce(Result<Value, Error>) + Send>;

/// What the other side sends on its own.
pub(crate) trait Handler: Send + Sync {
    fn notification(&self, method: &str, params: Value);
    fn request(&self, method: &str, params: Value, responder: Responder);
    /// The stream ended or broke; every waiting request has been failed.
    fn closed(&self);
}

/// A request waiting for its answer.
struct Waiting(Reply);

impl Call for Waiting {
    fn closed(self) {
        (self.0)(Err(Error::ServerGone));
    }

    fn expired(self, id: i64, transport: &Transport<Self>) {
        let _ = send(transport, &cancel_request(id));
        (self.0)(Err(Error::Timeout));
    }
}

/// One JSON-RPC connection.
pub(crate) struct Connection {
    transport: Arc<Transport<Waiting>>,
}

impl Connection {
    /// Starts reading `input` and writing `output`; `handler` hears what the other side sends.
    pub(crate) fn start(
        input: impl Read + Send + 'static,
        output: impl Write + Send + 'static,
        handler: Arc<dyn Handler>,
        name: &str,
    ) -> io::Result<Arc<Connection>> {
        let transport = Transport::start(input, output, Arc::new(Dispatch(handler)), "lsp", name)?;
        Ok(Arc::new(Connection { transport }))
    }

    /// Sends a request; `reply` hears the answer, an error, the timeout or the cancellation exactly once. When this
    /// returns an error, `reply` is dropped without a call.
    pub(crate) fn request(&self, method: &str, params: Value, timeout: Duration, reply: Reply) -> Result<i64, Error> {
        let id = self.transport.next_id();
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        self.transport
            .call(id, encode(&message)?, timeout, Waiting(reply))
            .map_err(error_of)?;
        Ok(id)
    }

    /// Sends a notification.
    pub(crate) fn notify(&self, method: &str, params: Value) -> Result<(), Error> {
        send(&self.transport, &notification(method, params))
    }

    /// Sends a notification whose parameters `build` makes on the writer thread, in order with every other message.
    pub(crate) fn notify_with(
        &self,
        method: &str,
        build: impl FnOnce() -> Value + Send + 'static,
    ) -> Result<(), Error> {
        let method = method.to_string();
        self.transport
            .send_built(Box::new(move || {
                encode(&notification(&method, build()))
                    .inspect_err(|error| ide_core::diag::report("lsp: notification", error))
                    .ok()
            }))
            .map_err(error_of)
    }

    /// Ends request `id` with `why` and asks the other side to stop working on it.
    pub(crate) fn cancel(&self, id: i64, why: Error) {
        if let Some(waiting) = self.transport.take(id) {
            let _ = send(&self.transport, &cancel_request(id));
            contained(|| (waiting.0)(Err(why)));
        }
    }

    /// Closes the other side's input once everything queued is written.
    pub(crate) fn end_input(&self) {
        self.transport.end_input();
    }
}

fn notification(method: &str, params: Value) -> Value {
    let mut message = json!({"jsonrpc": "2.0", "method": method});
    if !params.is_null() {
        message["params"] = params;
    }
    message
}

fn cancel_request(id: i64) -> Value {
    notification("$/cancelRequest", json!({"id": id}))
}

fn encode(message: &Value) -> Result<Vec<u8>, Error> {
    let body = serde_json::to_vec(message).map_err(|e| Error::Malformed(e.to_string()))?;
    Ok(framing::frame(&body))
}

fn send(transport: &Transport<Waiting>, message: &Value) -> Result<(), Error> {
    transport.send(encode(message)?).map_err(error_of)
}

fn error_of(error: SendError) -> Error {
    match error {
        SendError::Congested => Error::Congested,
        SendError::Closed => Error::ServerGone,
    }
}

/// Hands what arrives to the handler or to the request it answers.
struct Dispatch(Arc<dyn Handler>);

impl ide_rpc::Receiver<Waiting> for Dispatch {
    fn receive(&self, transport: &Arc<Transport<Waiting>>, body: &[u8]) {
        let mut message = match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(message)) => message,
            Ok(other) => return ide_core::diag::report("lsp: message is not an object", kind_of(&other)),
            Err(error) => return ide_core::diag::report("lsp: undecodable message", error),
        };
        let params = message.remove("params").unwrap_or(Value::Null);
        match (message.remove("method"), message.remove("id")) {
            (Some(Value::String(method)), Some(id)) => {
                let responder = Responder::new(Arc::downgrade(transport), id);
                contained(|| self.0.request(&method, params, responder));
            }
            (Some(Value::String(method)), None) => contained(|| self.0.notification(&method, params)),
            (None, Some(id)) => complete(transport, &id, message),
            _ => ide_core::diag::report("lsp: message neither request, notification nor response", ""),
        }
    }

    fn ended(&self) {
        self.0.closed();
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn complete(transport: &Transport<Waiting>, id: &Value, mut message: Map<String, Value>) {
    let Some(id) = id.as_i64() else {
        return ide_core::diag::report("lsp: response to an id never sent", id);
    };
    let Some(waiting) = transport.take(id) else {
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
    contained(|| (waiting.0)(answer));
}

/// Answers one request from the other side; dropped unanswered, it answers with an internal error.
pub(crate) struct Responder(ide_rpc::Responder<(i64, String)>);

impl Responder {
    fn new(transport: Weak<Transport<Waiting>>, id: Value) -> Responder {
        let answer = move |answer: Result<Value, (i64, String)>| {
            let Some(transport) = transport.upgrade() else {
                return;
            };
            let message = match answer {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                Err((code, message)) => {
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
                }
            };
            let _ = send(&transport, &message);
        };
        Responder(ide_rpc::Responder::new(
            answer,
            (-32603, "the request was not answered".to_string()),
        ))
    }

    pub(crate) fn ok(self, result: Value) {
        self.0.ok(result);
    }

    pub(crate) fn fail(self, code: i64, message: &str) {
        self.0.fail((code, message.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, PipeReader, PipeWriter};
    use std::sync::Mutex;
    use std::sync::mpsc::{Sender, channel};
    use std::time::Instant;

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
