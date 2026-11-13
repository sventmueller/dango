//! A provider that answers from a script over HTTP on the loopback interface, for the tests and the self-test, which
//! make no paid requests. It keeps every request it got, so a test can check what was sent.

use ide_core::LockIgnoringPoison;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;

/// A request the stub got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// The method, such as `POST`.
    pub method: String,
    /// The path with its query.
    pub path: String,
    /// The headers, names in lower case.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: String,
}

impl Seen {
    /// The first header called `name`.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The body as JSON, or null.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// What the stub answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The status.
    pub status: u16,
    /// The headers besides `connection: close`.
    pub headers: Vec<(String, String)>,
    /// The body, in parts written after a pause each.
    pub parts: Vec<(Duration, String)>,
}

impl Answer {
    /// A stream of server-sent events, written at once.
    pub fn events(text: &str) -> Answer {
        Answer {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            parts: vec![(Duration::ZERO, text.to_string())],
        }
    }

    /// A stream of server-sent events, written event by event with `pause` before each.
    pub fn events_slowly(text: &str, pause: Duration) -> Answer {
        let parts = text
            .split_inclusive("\n\n")
            .map(|event| (pause, event.to_string()))
            .collect();
        Answer {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            parts,
        }
    }

    /// JSON with `status`.
    pub fn json(status: u16, body: &Value) -> Answer {
        Answer {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            parts: vec![(Duration::ZERO, body.to_string())],
        }
    }

    /// The same with one more header.
    pub fn with_header(mut self, name: &str, value: &str) -> Answer {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

type Script = Box<dyn Fn(&Seen) -> Answer + Send + Sync>;

/// The running stub; it stops when dropped.
pub struct Stub {
    address: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Stub {
    /// A stub that answers each request with what `script` makes of it.
    pub fn start(script: impl Fn(&Seen) -> Answer + Send + Sync + 'static) -> std::io::Result<Stub> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let script: Arc<Script> = Arc::new(Box::new(script));
        let thread = {
            let (seen, stop) = (seen.clone(), stop.clone());
            std::thread::Builder::new()
                .name("provider-stub".into())
                .spawn(move || {
                    for connection in listener.incoming() {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        let Ok(connection) = connection else { continue };
                        let (seen, script) = (seen.clone(), script.clone());
                        std::thread::spawn(move || serve(connection, &seen, &script));
                    }
                })?
        };
        Ok(Stub {
            address,
            seen,
            stop,
            thread: Some(thread),
        })
    }

    /// A stub that gives `answers` one after the other and then a 500 for each further request.
    pub fn script(answers: Vec<Answer>) -> std::io::Result<Stub> {
        let answers = Mutex::new(answers.into_iter());
        Stub::start(move |_| {
            answers
                .lock_ignoring_poison()
                .next()
                .unwrap_or_else(|| Answer::json(500, &serde_json::json!({"error": {"message": "the script ended"}})))
        })
    }

    /// Its address, such as `http://127.0.0.1:50123`.
    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// The requests it got so far.
    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock_ignoring_poison().clone()
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(connection: TcpStream, seen: &Mutex<Vec<Seen>>, script: &Script) {
    let Ok(writer) = connection.try_clone() else { return };
    let mut reader = BufReader::new(connection);
    let Some(request) = read_request(&mut reader) else {
        return;
    };
    let answer = script(&request);
    seen.lock_ignoring_poison().push(request);
    let _ = write_answer(writer, &answer);
}

fn read_request(reader: &mut impl BufRead) -> Option<Seen> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut words = line.split_whitespace();
    let method = words.next()?.to_string();
    let path = words.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line).ok()?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Seen {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn write_answer(mut writer: TcpStream, answer: &Answer) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} Stub\r\nconnection: close\r\n", answer.status);
    for (name, value) in &answer.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    writer.write_all(head.as_bytes())?;
    writer.flush()?;
    for (pause, part) in &answer.parts {
        std::thread::sleep(*pause);
        writer.write_all(part.as_bytes())?;
        writer.flush()?;
    }
    writer.shutdown(std::net::Shutdown::Both)
}
