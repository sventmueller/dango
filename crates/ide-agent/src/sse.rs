//! Server-sent events as the providers stream them: `event:` and `data:` lines up to a blank line.

use std::io::BufRead;

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
    line: String,
}

impl<R: BufRead> Events<R> {
    pub(crate) fn new(reader: R) -> Events<R> {
        Events {
            reader,
            line: String::new(),
        }
    }
}

impl<R: BufRead> Iterator for Events<R> {
    type Item = std::io::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut event: Option<String> = None;
        let mut data: Option<String> = None;
        loop {
            self.line.clear();
            match self.reader.read_line(&mut self.line) {
                Err(e) => return Some(Err(e)),
                Ok(0) => {
                    return data.map(|data| Ok(Event { event, data }));
                }
                Ok(_) => {}
            }
            let line = self.line.trim_end_matches(['\n', '\r']);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> Vec<Event> {
        Events::new(text.as_bytes()).map(Result::unwrap).collect()
    }

    #[test]
    fn events_end_at_blank_lines_and_keep_their_type() {
        let got = events("event: ping\ndata: {}\n\n: comment\ndata: a\r\ndata: b\r\n\r\ndata:[DONE]");
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
