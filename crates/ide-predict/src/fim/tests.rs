//! The local providers against a server on a loopback port that answers with recorded answers, or holds the
//! connection and watches it close.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::*;
use crate::Indent;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// What the fake server saw: the request line and the body.
struct Seen {
    line: String,
    body: Value,
}

/// A server that answers the first connection with `status` and `body`, or, without one, holds it open and reports
/// how long after the request the client closed it.
struct Fake {
    base: String,
    seen: Receiver<Seen>,
    closed: Receiver<Instant>,
}

fn fake(answer: Option<(u16, String)>) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (seen_tx, seen) = channel();
    let (closed_tx, closed) = channel();
    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else { return };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let (line, body) = read_request(&mut reader);
        let _ = seen_tx.send(Seen { line, body });
        let mut stream = stream;
        match answer {
            Some((status, body)) => {
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
            None => {
                let mut rest = [0u8; 64];
                while reader.get_mut().read(&mut rest).is_ok_and(|n| n > 0) {}
                let _ = closed_tx.send(Instant::now());
            }
        }
    });
    Fake { base, seen, closed }
}

fn read_request(reader: &mut BufReader<TcpStream>) -> (String, Value) {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    (
        line.trim().to_string(),
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

const TEXT: &str = "fn add(a: i32, b: i32) -> i32 {\n    \n}\n";

fn request() -> Request {
    let caret = TEXT.find("    \n").unwrap() + 4;
    Request {
        path: "/p/src/lib.rs".into(),
        language: "rust".into(),
        text: TEXT.into(),
        revision: 7,
        selection: caret..caret,
        indent: Indent::Spaces(4),
    }
}

fn ask(provider: &dyn Provider, cancel: Cancel) -> Receiver<Result<Vec<Prediction>, Error>> {
    let (tx, rx) = channel();
    provider.predict(
        request(),
        cancel,
        Box::new(move |answer| {
            let _ = tx.send(answer);
        }),
    );
    rx
}

const WAIT: Duration = Duration::from_secs(10);

#[test]
fn ollama_is_asked_with_prefix_and_suffix_and_its_answer_is_cut_before_the_next_line() {
    let server = fake(Some((200, fixture("ollama-generate.json"))));
    let ollama = Ollama::new(&format!("{}/v1/", server.base), "qwen2.5-coder:1.5b");
    let answer = ask(&ollama, Cancel::new()).recv_timeout(WAIT).unwrap();
    let caret = request().selection.start;
    assert_eq!(
        answer,
        Ok(vec![Prediction {
            range: caret..caret,
            text: "a + b".into(),
            feedback: None,
        }])
    );
    let seen = server.seen.recv_timeout(WAIT).unwrap();
    assert_eq!(seen.line, "POST /api/generate HTTP/1.1");
    assert_eq!(seen.body["model"], "qwen2.5-coder:1.5b");
    assert_eq!(seen.body["prompt"], &TEXT[..caret]);
    assert_eq!(seen.body["suffix"], &TEXT[caret..]);
    assert_eq!(seen.body["stream"], false);
    assert_eq!(seen.body["options"]["num_predict"], MAX_TOKENS);
}

#[test]
fn lm_studio_is_asked_without_a_template_in_the_marks_of_the_models_family() {
    let server = fake(Some((200, fixture("lmstudio-completions.json"))));
    let lmstudio = LmStudio::new(&server.base, "qwen2.5-coder-1.5b-instruct");
    let answer = ask(&lmstudio, Cancel::new()).recv_timeout(WAIT).unwrap();
    assert_eq!(answer.unwrap()[0].text, "a + b");
    let seen = server.seen.recv_timeout(WAIT).unwrap();
    let caret = request().selection.start;
    assert_eq!(seen.line, "POST /v1/completions HTTP/1.1");
    assert_eq!(
        seen.body["prompt"],
        format!(
            "<|fim_prefix|>{}<|fim_suffix|>{}<|fim_middle|>",
            &TEXT[..caret],
            &TEXT[caret..]
        )
    );
    assert_eq!(seen.body["stop"][0], "<|endoftext|>");
    assert_eq!(seen.body["stop"].as_array().map(Vec::len), Some(STOPS_SENT));
    assert!(seen.body.get("messages").is_none());
}

#[test]
fn failures_tell_what_to_do() {
    let unknown = fake(Some((404, fixture("ollama-not-found.json"))));
    let answer = ask(&Ollama::new(&unknown.base, "qwen2.5-coder:7b"), Cancel::new()).recv_timeout(WAIT);
    let expected = texts::unknown_model("Ollama", "qwen2.5-coder:7b");
    assert_eq!(answer.unwrap(), Err(Error::Unavailable(expected)));

    let no_insert = fake(Some((400, fixture("ollama-no-insert.json"))));
    let answer = ask(&Ollama::new(&no_insert.base, "llama3.2"), Cancel::new()).recv_timeout(WAIT);
    assert_eq!(answer.unwrap(), Err(Error::Unavailable(texts::no_fill_in("llama3.2"))));

    let not_loaded = fake(Some((404, fixture("lmstudio-no-model.json"))));
    let answer = ask(&LmStudio::new(&not_loaded.base, "starcoder2-3b"), Cancel::new()).recv_timeout(WAIT);
    let expected = texts::unknown_model("LM Studio", "starcoder2-3b");
    assert_eq!(answer.unwrap(), Err(Error::Unavailable(expected)));

    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let answer = ask(&Ollama::new(&base, "m"), Cancel::new()).recv_timeout(WAIT);
    assert_eq!(answer.unwrap(), Err(Error::Unavailable(texts::unreachable("Ollama"))));

    let answer = ask(&LmStudio::new(&base, "llama-3.2-3b-instruct"), Cancel::new()).recv_timeout(WAIT);
    let expected = texts::no_fill_in("llama-3.2-3b-instruct");
    assert_eq!(answer.unwrap(), Err(Error::Unavailable(expected)));
    let answer = ask(&Ollama::new(&base, " "), Cancel::new()).recv_timeout(WAIT);
    assert_eq!(answer.unwrap(), Err(Error::Unavailable(texts::no_model())));

    let odd = fake(Some((200, r#"{"choices": []}"#.into())));
    let answer = ask(&LmStudio::new(&odd.base, "codestral-22b"), Cancel::new()).recv_timeout(WAIT);
    assert!(matches!(answer.unwrap(), Err(Error::Malformed(_))));
}

#[test]
fn cancelling_closes_the_connection_at_once_so_the_server_stops_generating() {
    let server = fake(None);
    let ollama = Ollama::new(&server.base, "qwen2.5-coder:1.5b");
    let cancel = Cancel::new();
    let caller = std::thread::current().id();
    let (tx, rx) = channel();
    ollama.predict(
        request(),
        cancel.clone(),
        Box::new(move |answer| {
            let _ = tx.send((answer, std::thread::current().id()));
        }),
    );
    server.seen.recv_timeout(WAIT).unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "the server has not answered"
    );
    let cancelled = Instant::now();
    cancel.cancel();
    let closed = server
        .closed
        .recv_timeout(WAIT)
        .expect("the server saw the connection close");
    assert!(closed.duration_since(cancelled) < Duration::from_secs(1));
    let (answer, on) = rx.recv_timeout(WAIT).unwrap();
    assert_eq!(answer, Err(Error::Cancelled));
    assert_ne!(on, caller);
}

#[test]
fn a_request_cancelled_before_it_is_sent_never_connects() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let cancel = Cancel::new();
    cancel.cancel();
    let answer = ask(&Ollama::new(&base, "m"), cancel).recv_timeout(WAIT).unwrap();
    assert_eq!(answer, Err(Error::Cancelled));
    assert!(listener.accept().is_err());
}

#[test]
fn the_installed_models_are_listed() {
    let ollama = fake(Some((200, fixture("ollama-tags.json"))));
    assert_eq!(
        Ollama::models(&ollama.base),
        Ok(vec!["llama3.2:latest".to_string(), "qwen2.5-coder:1.5b".to_string()])
    );
    assert_eq!(ollama.seen.recv_timeout(WAIT).unwrap().line, "GET /api/tags HTTP/1.1");
    let lmstudio = fake(Some((200, fixture("lmstudio-models.json"))));
    let models = LmStudio::models(&format!("{}/v1", lmstudio.base)).unwrap();
    assert_eq!(models[0], "qwen2.5-coder-1.5b-instruct");
    assert_eq!(
        lmstudio.seen.recv_timeout(WAIT).unwrap().line,
        "GET /v1/models HTTP/1.1"
    );
}

#[test]
fn the_gap_carries_whole_lines_of_a_long_prefix_and_replaces_a_selection() {
    let long = format!("{}é\nlast line {{\n", "x".repeat(PREFIX_BYTES));
    let text = format!("{long}SELECTED tail");
    let start = long.len();
    let gap = Gap::of(&Request {
        selection: start..start + "SELECTED".len(),
        text: text.clone(),
        ..request()
    });
    assert_eq!(gap.prefix, "last line {\n");
    assert_eq!(gap.suffix, " tail");
    assert_eq!(gap.range, start..start + 8);
    let odd = Gap::of(&Request {
        selection: 2..9,
        text: "aé".into(),
        ..request()
    });
    assert_eq!((odd.prefix.as_str(), odd.suffix.as_str(), odd.range), ("a", "", 1..3));
}
