//! The order log as JSON lines (E167): one file per order in the app's data folder, appended only, each line versioned
//! and numbered. A cut last line, as a crash in the middle of a write leaves it, is dropped; reading stops at a damaged
//! line in the middle, and at a line whose number does not follow the one before, since the lines after it build on
//! what is missing.

use std::collections::BTreeMap;

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
    /// The entry as one line with its line break; an entry that cannot be written as JSON cannot be logged.
    pub fn line(&self) -> serde_json::Result<String> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

/// An order's log entries from `seq` on could not be written, so none of the effects that followed them ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFailure {
    /// The order.
    pub order: OrderId,
    /// The first entry that is not on disk.
    pub seq: u64,
}

/// What reading a log found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Read {
    /// The entries that could be read, in order.
    pub entries: Vec<LogEntry>,
    /// The last line was cut and dropped.
    pub cut: bool,
    /// Reading stopped at this line, from one, because it was damaged, from a newer version, or did not follow the
    /// entry before it.
    pub stopped_at: Option<usize>,
    /// How many bytes of the text the entries read span, up to the line break after the last one.
    pub kept: usize,
    /// The last entry read ends the text without a line break.
    pub unbroken: bool,
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
    let mut next: BTreeMap<OrderId, u64> = BTreeMap::new();
    let mut start = 0;
    let mut number = 0;
    while start < text.len() {
        number += 1;
        let (line, end, broken) = match text[start..].find('\n') {
            Some(at) => (&text[start..start + at], start + at + 1, true),
            None => (&text[start..], text.len(), false),
        };
        if !line.trim().is_empty() {
            let read = entry(line).filter(|e| next.get(&e.order).is_none_or(|seq| *seq == e.seq));
            match read {
                Some(entry) => {
                    next.insert(entry.order.clone(), entry.seq + 1);
                    out.entries.push(entry);
                    out.kept = end;
                    out.unbroken = !broken;
                }
                None if !broken && entry(line).is_none() => out.cut = true,
                None => {
                    out.stopped_at = Some(number);
                    break;
                }
            }
        }
        start = end;
    }
    out
}

/// A log put back in shape before anything is appended to it: what to keep, and what no longer counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    /// The log as it is to be: the lines read, each ending with its line break.
    pub kept: String,
    /// What came after them: a cut line, or a damaged line and everything after it.
    pub dropped: String,
}

/// How to put a log's `text` back in shape, or `None` when it is: an append after a cut line or without a line break
/// would glue the next entry to it, and entries after a damaged line would never be read again.
pub fn repair(text: &str) -> Option<Repair> {
    let read = read(text);
    if read.kept == text.len() && !read.unbroken {
        return None;
    }
    let mut kept = text[..read.kept].to_string();
    if read.unbroken {
        kept.push('\n');
    }
    Some(Repair {
        kept,
        dropped: text[read.kept..].to_string(),
    })
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
        .unwrap()
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
        let text = format!("{}garbage\n{}", line(0), line(1));
        let got = read(&text);
        assert_eq!((got.entries.len(), got.stopped_at), (1, Some(2)));
    }

    #[test]
    fn a_newer_entry_is_not_read() {
        let newer = line(1).replace("\"v\":1", "\"v\":9");
        let got = read(&format!("{}{newer}", line(0)));
        assert_eq!((got.entries.len(), got.stopped_at), (1, Some(2)));
    }

    #[test]
    fn a_gap_in_the_numbers_stops_reading() {
        let got = read(&format!("{}{}{}", line(0), line(2), line(3)));
        assert_eq!((got.entries.len(), got.stopped_at), (1, Some(2)));
        let repeated = read(&format!("{}{}", line(0), line(0)));
        assert_eq!((repeated.entries.len(), repeated.stopped_at), (1, Some(2)));
    }

    #[test]
    fn a_log_in_shape_needs_no_repair() {
        assert_eq!(repair(&format!("{}{}", line(0), line(1))), None);
        assert_eq!(repair(""), None);
    }

    #[test]
    fn a_crash_in_the_middle_of_a_line_is_cut_back_to_the_last_line_break() {
        let whole = format!("{}{}", line(0), line(1));
        let cut = &whole[..whole.len() - 7];
        let repaired = repair(cut).unwrap();
        assert_eq!(repaired.kept, line(0));
        assert_eq!(format!("{}{}", repaired.kept, repaired.dropped), cut);
        let appended = format!("{}{}", repaired.kept, line(1));
        let read = read(&appended);
        assert_eq!((read.entries.len(), read.cut, read.stopped_at), (2, false, None));
    }

    #[test]
    fn a_crash_before_the_line_break_keeps_the_entry_and_adds_the_break() {
        let whole = format!("{}{}", line(0), line(1));
        let unbroken = &whole[..whole.len() - 1];
        assert_eq!(read(unbroken).entries.len(), 2);
        let repaired = repair(unbroken).unwrap();
        assert_eq!(repaired.kept, whole);
        assert!(repaired.dropped.is_empty());
        let read = read(&format!("{}{}", repaired.kept, line(2)));
        assert_eq!((read.entries.len(), read.stopped_at), (3, None));
    }

    #[test]
    fn everything_after_a_damaged_line_is_set_aside() {
        let text = format!("{}garbage\n{}", line(0), line(1));
        let repaired = repair(&text).unwrap();
        assert_eq!(repaired.kept, line(0));
        assert_eq!(repaired.dropped, format!("garbage\n{}", line(1)));
    }
}
