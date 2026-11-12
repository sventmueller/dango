//! A recorded session played back as an adapter, for tests and the self-test: each request of the client is matched
//! to the next recorded request with the same command, and what the adapter sent after it in the recording is sent
//! again, its numbers changed to the client's.
//!
//! Recordings are written by `Adapter::record_to`: one JSON object a line, `{"from": "client"|"adapter",
//! "message": …}`.

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};

use serde_json::Value;

use crate::protocol::{self, Message};

/// One line of a recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Whether the client sent it.
    pub from_client: bool,
    /// The message.
    pub message: Message,
}

/// The lines of a recording; lines that cannot be read are skipped.
pub fn parse(text: &str) -> Vec<Line> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| {
            let from_client = value.get("from")?.as_str()? == "client";
            let body = serde_json::to_vec(value.get("message")?).ok()?;
            Some(Line {
                from_client,
                message: protocol::parse(&body).ok()?,
            })
        })
        .collect()
}

/// Plays `lines` on threads of its own and returns the streams to talk to it: what it writes, where to write.
pub(crate) fn start(lines: Vec<Line>) -> io::Result<(impl Read + Send + 'static, impl Write + Send + 'static)> {
    let (from_adapter, adapter_out) = std::io::pipe()?;
    let (adapter_in, to_adapter) = std::io::pipe()?;
    std::thread::Builder::new()
        .name("dap-replay".into())
        .spawn(move || Player::new(lines, adapter_out).play(adapter_in))?;
    Ok((from_adapter, to_adapter))
}

struct Player<W: Write> {
    lines: Vec<Line>,
    used: Vec<bool>,
    out: W,
    /// The client's numbers for the recorded client's.
    seqs: HashMap<i64, i64>,
    next_seq: i64,
}

impl<W: Write> Player<W> {
    fn new(lines: Vec<Line>, out: W) -> Player<W> {
        let used = vec![false; lines.len()];
        Player {
            lines,
            used,
            out,
            seqs: HashMap::new(),
            next_seq: 1,
        }
    }

    fn play(mut self, input: impl Read) {
        self.send_after(None);
        let mut input = BufReader::new(input);
        while let Ok(Some(body)) = ide_process::framing::read(&mut input) {
            let Ok(message) = protocol::parse(&body) else {
                continue;
            };
            let (seq, command, response) = match &message {
                Message::Request { seq, command, .. } => (*seq, command.clone(), false),
                Message::Response { seq, command, .. } => (*seq, command.clone(), true),
                Message::Event { .. } => continue,
            };
            let Some(index) = self.find(&command, response) else {
                if !response {
                    self.refuse(seq, &command);
                }
                continue;
            };
            self.used[index] = true;
            if let Message::Request { seq: recorded, .. } = self.lines[index].message {
                self.seqs.insert(recorded, seq);
            }
            self.send_after(Some(index));
            if command == "disconnect" {
                return;
            }
        }
    }

    /// The first unused recorded client message of this kind.
    fn find(&self, command: &str, response: bool) -> Option<usize> {
        self.lines.iter().enumerate().position(|(i, line)| {
            !self.used[i]
                && line.from_client
                && match &line.message {
                    Message::Request { command: c, .. } => !response && c == command,
                    Message::Response { command: c, .. } => response && c == command,
                    Message::Event { .. } => false,
                }
        })
    }

    /// Sends what the adapter sent after line `after` (or from the start) up to the client's next line.
    fn send_after(&mut self, after: Option<usize>) {
        let start = after.map_or(0, |i| i + 1);
        for index in start..self.lines.len() {
            if self.lines[index].from_client {
                break;
            }
            if std::mem::replace(&mut self.used[index], true) {
                continue;
            }
            let message = self.renumber(self.lines[index].message.clone());
            if self
                .out
                .write_all(&message.to_frame())
                .and_then(|()| self.out.flush())
                .is_err()
            {
                return;
            }
        }
    }

    fn renumber(&mut self, message: Message) -> Message {
        let seq = self.next_seq;
        self.next_seq += 1;
        match message {
            Message::Response {
                request_seq,
                success,
                command,
                message,
                body,
                ..
            } => Message::Response {
                seq,
                request_seq: self.seqs.get(&request_seq).copied().unwrap_or(request_seq),
                success,
                command,
                message,
                body,
            },
            Message::Request { command, arguments, .. } => Message::Request {
                seq,
                command,
                arguments,
            },
            Message::Event { event, body, .. } => Message::Event { seq, event, body },
        }
    }

    fn refuse(&mut self, request_seq: i64, command: &str) {
        let seq = self.next_seq;
        self.next_seq += 1;
        let message = Message::Response {
            seq,
            request_seq,
            success: false,
            command: command.to_string(),
            message: Some(format!("{command} is not in the recording")),
            body: Value::Null,
        };
        let _ = self.out.write_all(&message.to_frame());
        let _ = self.out.flush();
    }
}
