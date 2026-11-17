//! Server-sent events as the providers stream them: `event:` and `data:` lines up to a blank line. An event the
//! stream ends in before its blank line is dropped, as the HTML standard's event-stream parsing says.

use std::io::{self, BufRead};

use ide_process::framing::{self, Line};

/// The longest line of a stream; more means a broken or hostile server, not an answer.
const MAX_LINE: usize = 16 * 1024 * 1024;

/// One event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Event {
    /// Its type, when the stream names one.
    pub(crate) event: Option<String>,
    /// Its data, with several `data:` lines joined by newlines.
    pub(crate) data: String,
}

/// The events of a stream, one at a time.
pub(crate) struct Events<R> {
    reader: R,
    line: Vec<u8>,
}

impl<R: BufRead> Events<R> {
    pub(crate) fn new(reader: R) -> Events<R> {
        Events {
            reader,
            line: Vec::new(),
        }
    }
}

impl<R: BufRead> Iterator for Events<R> {
    type Item = io::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut event: Option<String> = None;
        let mut data: Option<String> = None;
        loop {
            match framing::read_line(&mut self.reader, &mut self.line, MAX_LINE) {
                Err(e) => return Some(Err(e)),
                Ok(Line::End) => return None,
                Ok(Line::TooLong) => return Some(Err(too_long())),
                Ok(Line::Read) => {}
            }
            let line = match std::str::from_utf8(&self.line) {
                Ok(line) => line.trim_end_matches(['\n', '\r']),
                Err(e) => return Some(Err(io::Error::new(io::ErrorKind::InvalidData, e))),
            };
            if line.is_empty() {
                if let Some(data) = data.take() {
                    return Some(Ok(Event { event, data }));
                }
                event = None;
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "event" => event = Some(value.to_string()),
                "data" => match &mut data {
                    Some(data) => {
                        data.push('\n');
                        data.push_str(value);
                    }
                    None => data = Some(value.to_string()),
                },
                _ => {}
            }
            // An event's data lines are one value, so a server cannot grow it without end line by line either.
            if data.as_ref().is_some_and(|data| data.len() > MAX_LINE) {
                return Some(Err(too_long()));
            }
        }
    }
}

/// The failure of a stream that sent more in one go than any answer does.
fn too_long() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the stream sent a line longer than {} MiB", MAX_LINE >> 20),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> Vec<Event> {
        Events::new(text.as_bytes()).map(Result::unwrap).collect()
    }

    #[test]
    fn events_end_at_blank_lines_and_keep_their_type() {
        let got = events("event: ping\ndata: {}\n\n: comment\ndata: a\r\ndata: b\r\n\r\ndata:[DONE]\n\n");
        assert_eq!(
            got,
            [
                Event {
                    event: Some("ping".into()),
                    data: "{}".into()
                },
                Event {
                    event: None,
                    data: "a\nb".into()
                },
                Event {
                    event: None,
                    data: "[DONE]".into()
                },
            ]
        );
    }

    #[test]
    fn an_event_cut_off_by_the_end_of_the_stream_is_dropped() {
        assert_eq!(
            events("data: whole\n\ndata: half"),
            [Event {
                event: None,
                data: "whole".into()
            }]
        );
        assert!(events("event: x\ndata: half\n").is_empty());
    }

    #[test]
    fn an_overlong_line_ends_the_stream_with_an_error() {
        let mut text = String::from("data: first\n\ndata: ");
        text.push_str(&"x".repeat(MAX_LINE + 1));
        text.push_str("\n\ndata: after\n\n");
        let mut events = Events::new(text.as_bytes());
        assert_eq!(events.next().unwrap().unwrap().data, "first");
        let error = events.next().unwrap().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("longer than 16 MiB"));
    }

    #[test]
    fn an_event_of_many_short_lines_cannot_grow_past_the_limit() {
        let line = format!("data: {}\n", "x".repeat(1 << 20));
        let mut text = String::from("data: first\n\n");
        for _ in 0..17 {
            text.push_str(&line);
        }
        text.push('\n');
        let mut events = Events::new(text.as_bytes());
        assert_eq!(events.next().unwrap().unwrap().data, "first");
        assert_eq!(events.next().unwrap().unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_type_without_data_does_not_carry_over() {
        let got = events("event: x\n\ndata: y\n\n");
        assert_eq!(
            got,
            [Event {
                event: None,
                data: "y".into()
            }]
        );
    }
}
