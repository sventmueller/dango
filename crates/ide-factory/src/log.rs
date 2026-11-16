//! The order log as JSON lines (E167): one file per order in the app's data folder, appended only, each line versioned.
//! A cut last line, as a crash in the middle of a write leaves it, is dropped; reading stops at a damaged line in the
//! middle, since the lines after it build on what it said.

use serde::{Deserialize, Serialize};

use crate::event::Event;
use crate::ids::OrderId;
use crate::time::Stamp;

/// The version of the log entries this build writes.
pub const LOG_VERSION: u32 = 1;

/// One line of an order's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// The entry's version.
    pub v: u32,
    /// The order.
    pub order: OrderId,
    /// Its place in the order's log, from zero.
    pub seq: u64,
    /// When.
    pub at: Stamp,
    /// What.
    pub event: Event,
}

impl LogEntry {
    /// The entry as one line with its line break.
    pub fn line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_default();
        line.push('\n');
        line
    }
}

/// What reading a log found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Read {
    /// The entries that could be read, in order.
    pub entries: Vec<LogEntry>,
    /// The last line was cut and dropped.
    pub cut: bool,
    /// Reading stopped at this line, from one, because it was damaged or from a newer version.
    pub stopped_at: Option<usize>,
}

/// Lifts an entry of an older version to [`LOG_VERSION`]; there is none older yet.
fn upgrade(value: serde_json::Value, _from: u64) -> serde_json::Value {
    value
}

fn entry(line: &str) -> Option<LogEntry> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let version = value.get("v")?.as_u64()?;
    if version > u64::from(LOG_VERSION) {
        return None;
    }
    let value = if version < u64::from(LOG_VERSION) {
        upgrade(value, version)
    } else {
        value
    };
    let mut entry: LogEntry = serde_json::from_value(value).ok()?;
    entry.v = LOG_VERSION;
    Some(entry)
}

/// Reads a log's text.
pub fn read(text: &str) -> Read {
    let mut out = Read::default();
    let ends_with_break = text.ends_with('\n');
    let lines: Vec<&str> = text.split('\n').collect();
    let count = lines.len();
    for (index, line) in lines.into_iter().enumerate() {
        let last = index + 1 == count;
        if line.trim().is_empty() {
            continue;
        }
        match entry(line) {
            Some(entry) => out.entries.push(entry),
            None if last && !ends_with_break => out.cut = true,
            None => {
                out.stopped_at = Some(index + 1);
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, Phase};

    fn line(seq: u64) -> String {
        LogEntry {
            v: LOG_VERSION,
            order: OrderId("o1".into()),
            seq,
            at: Stamp::new(seq, 1000 + seq),
            event: Event::Phase { phase: Phase::Refs },
        }
        .line()
    }

    #[test]
    fn a_cut_last_line_is_dropped() {
        let text = format!("{}{}", line(0), line(1));
        let cut = &text[..text.len() - 5];
        let got = read(cut);
        assert_eq!((got.entries.len(), got.cut, got.stopped_at), (1, true, None));
        let whole = read(&text);
        assert_eq!((whole.entries.len(), whole.cut), (2, false));
    }

    #[test]
    fn a_damaged_middle_line_stops_reading() {
        let text = format!("{}garbage\n{}", line(0), line(2));
        let got = read(&text);
        assert_eq!((got.entries.len(), got.stopped_at), (1, Some(2)));
    }

    #[test]
    fn a_newer_entry_is_not_read() {
        let newer = line(1).replace("\"v\":1", "\"v\":9");
        let got = read(&format!("{}{newer}", line(0)));
        assert_eq!((got.entries.len(), got.stopped_at), (1, Some(2)));
    }
}
