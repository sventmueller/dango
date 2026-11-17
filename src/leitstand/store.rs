//! What the Leitstand keeps on this Mac (E167, E168, E190): each order's log as JSON lines and its specification, the
//! confirmations and the dates of the recurring orders of each project (B47), and across projects the daily account
//! and the quota book, all in the app's data
//! folder and never in a repository. Everything here blocks and belongs on the disk lane.

use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ide_factory::OrderId;
use ide_factory::budget::DailyAccount;
use ide_factory::log::{LogEntry, LogFailure};
use ide_factory::quota::QuotaBook;
use ide_factory::workflow::recurring::Schedule;
use ide_factory::workflow::rules::Confirmations;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The folder of everything the Leitstand keeps.
pub(super) fn base() -> PathBuf {
    ide_core::paths::data_dir().join("leitstand")
}

/// The folder of what the Leitstand keeps for the project at `root`, beside the folder of its worktrees.
pub(super) fn project(base: &Path, root: &Path) -> PathBuf {
    base.join(ide_core::paths::project_key(root))
}

/// The folder of the orders' logs and specifications.
pub(super) fn orders(project: &Path) -> PathBuf {
    project.join("orders")
}

/// The log of `order`.
pub(super) fn log_file(project: &Path, order: &OrderId) -> PathBuf {
    orders(project).join(format!("{}.jsonl", order.0))
}

/// The specification of `order` (E178).
pub(super) fn spec_file(project: &Path, order: &OrderId) -> PathBuf {
    orders(project).join(format!("{}.spec.md", order.0))
}

/// Where the agent configuration of `order` is frozen, outside anything its builder may write (E172).
pub(super) fn frozen(project: &Path, order: &OrderId) -> PathBuf {
    project.join("frozen").join(&order.0)
}

/// A folder of its own for one run of a gate or setup command of `order`: its temporary files and pictures.
pub(super) fn run_folder(project: &Path, order: &OrderId, run: u64) -> PathBuf {
    project.join("runs").join(&order.0).join(run.to_string())
}

fn confirmations_file(project: &Path) -> PathBuf {
    project.join("confirmations.json")
}

fn schedule_file(project: &Path) -> PathBuf {
    project.join("recurring.json")
}

fn day_file(base: &Path) -> PathBuf {
    base.join("day.json")
}

fn quotas_file(base: &Path) -> PathBuf {
    base.join("quotas.json")
}

/// The texts of every order log of the project, in the order of their names, as they were read; each log is put back
/// in shape first, so the next entry starts a line of its own and is read after a restart (E167).
pub(super) fn read_logs(project: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(orders(project)) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|path| match std::fs::read_to_string(path) {
            Ok(text) => {
                repair(path, &text);
                Some(text)
            }
            Err(error) => {
                ide_core::diag::report("reading an order log", error);
                None
            }
        })
        .collect()
}

/// Cuts a log back to the lines a restart reads, through a file beside it; what no longer counts is kept in a file
/// of its own for a look by hand.
fn repair(file: &Path, text: &str) {
    let Some(repair) = ide_factory::log::repair(text) else {
        return;
    };
    if !repair.dropped.trim().is_empty() {
        let aside = file.with_extension("dropped");
        if let Err(error) = std::fs::write(&aside, repair.dropped.as_bytes()) {
            ide_core::diag::report("keeping the damaged end of an order log", error);
        }
    }
    if let Err(error) = write_atomically(file, repair.kept.as_bytes()) {
        ide_core::diag::report("repairing an order log", error);
    }
}

/// One order's entries of a step, for its log.
pub(super) struct Group {
    order: OrderId,
    file: PathBuf,
    /// The place of the first entry.
    first: u64,
    /// The place after the last entry.
    next: u64,
    /// The lines; `None` when an entry cannot be written as JSON.
    text: Option<String>,
}

impl Group {
    /// The failure of writing it.
    pub(super) fn failure(&self) -> LogFailure {
        LogFailure {
            order: self.order.clone(),
            seq: self.first,
        }
    }
}

/// `entries` grouped by their order's log, each group's lines in order.
pub(super) fn group(project: &Path, entries: &[LogEntry]) -> Vec<Group> {
    let mut groups: BTreeMap<OrderId, Group> = BTreeMap::new();
    for entry in entries {
        let group = groups.entry(entry.order.clone()).or_insert_with(|| Group {
            order: entry.order.clone(),
            file: log_file(project, &entry.order),
            first: entry.seq,
            next: entry.seq,
            text: Some(String::new()),
        });
        group.next = entry.seq + 1;
        match (entry.line(), &mut group.text) {
            (Ok(line), Some(text)) => text.push_str(&line),
            (Err(error), text) => {
                ide_core::diag::report("writing an order log entry as JSON", error);
                *text = None;
            }
            (Ok(_), None) => {}
        }
    }
    groups.into_values().collect()
}

/// What appending a step's groups came to.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Appended {
    /// The orders whose log is on disk up to the entry before this place.
    pub(super) written: Vec<(OrderId, u64)>,
    /// The orders whose entries could not be written.
    pub(super) failed: Vec<LogFailure>,
    /// The orders whose entries were not written because they do not continue the log: they build on entries that
    /// failed before, whose order hears of it from that failure.
    pub(super) refused: Vec<OrderId>,
}

impl Appended {
    /// Whether the effects of `order` must not run.
    pub(super) fn holds_back(&self, order: &OrderId) -> bool {
        self.failed.iter().any(|f| f.order == *order) || self.refused.contains(order)
    }
}

/// Appends each group to its log, on disk before it returns.
pub(super) fn append_all(groups: &[Group]) -> Appended {
    let mut out = Appended::default();
    for group in groups {
        let Some(text) = &group.text else {
            out.failed.push(group.failure());
            continue;
        };
        match append(&group.file, group.first, text) {
            Ok(true) => out.written.push((group.order.clone(), group.next)),
            Ok(false) => {
                ide_core::diag::report(
                    "writing an order log",
                    format!(
                        "entries of {} from {} on do not continue its log",
                        group.order, group.first
                    ),
                );
                out.refused.push(group.order.clone());
            }
            Err(error) => {
                ide_core::diag::report("writing an order log", error);
                out.failed.push(group.failure());
            }
        }
    }
    out
}

/// Appends `text`, whose first entry is the `first`th, if the log ends with the entry before it; `false` if it does
/// not. A failed write is cut off again, so the log keeps ending with a whole line.
fn append(file: &Path, first: u64, text: &str) -> io::Result<bool> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(file)?;
    let (last, broken) = last_line(&mut log)?;
    let next = match last {
        None => 0,
        Some(line) => match ide_factory::log::read(&line).entries.as_slice() {
            [entry] => entry.seq + 1,
            _ => return Ok(false),
        },
    };
    if next != first {
        return Ok(false);
    }
    let length = log.metadata()?.len();
    let mut bytes = Vec::with_capacity(text.len() + 1);
    if !broken && length > 0 {
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(text.as_bytes());
    let written = log.write_all(&bytes).and_then(|()| log.sync_data());
    if let Err(error) = written {
        let _ = log.set_len(length).and_then(|()| log.sync_data());
        return Err(error);
    }
    Ok(true)
}

/// The log's last line that is not empty, and whether the log ends with a line break.
fn last_line(log: &mut std::fs::File) -> io::Result<(Option<String>, bool)> {
    const CHUNK: u64 = 4096;
    let length = log.metadata()?.len();
    let mut tail: Vec<u8> = Vec::new();
    let mut start = length;
    loop {
        let trimmed = tail.iter().rposition(|b| !b.is_ascii_whitespace());
        let line_start = trimmed.and_then(|end| tail[..end].iter().rposition(|b| *b == b'\n'));
        if line_start.is_some() || start == 0 {
            break;
        }
        let from = start.saturating_sub(CHUNK.max(length - start));
        let mut chunk = vec![0; usize::try_from(start - from).unwrap_or(0)];
        log.seek(SeekFrom::Start(from))?;
        log.read_exact(&mut chunk)?;
        chunk.extend_from_slice(&tail);
        tail = chunk;
        start = from;
    }
    let broken = tail.last().is_none_or(|b| *b == b'\n');
    let Some(end) = tail.iter().rposition(|b| !b.is_ascii_whitespace()) else {
        return Ok((None, broken));
    };
    let begin = tail[..end].iter().rposition(|b| *b == b'\n').map_or(0, |at| at + 1);
    Ok((Some(String::from_utf8_lossy(&tail[begin..=end]).into_owned()), broken))
}

/// Writes `text` to `file` whole, through a file beside it, so a crash or a power loss leaves the old or the new one.
pub(super) fn write_atomically(file: &Path, text: &[u8]) -> io::Result<()> {
    ide_core::atomic_write::write(file, text)
}

fn load<T: DeserializeOwned + Default>(file: &Path) -> T {
    match std::fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            ide_core::diag::report("reading what the Leitstand keeps", error);
            T::default()
        }),
        Err(_) => T::default(),
    }
}

fn save<T: Serialize>(file: &Path, value: &T) {
    let written = serde_json::to_vec_pretty(value)
        .map_err(io::Error::other)
        .and_then(|bytes| write_atomically(file, &bytes));
    if let Err(error) = written {
        ide_core::diag::report("writing what the Leitstand keeps", error);
    }
}

/// The confirmations of the project (E190).
pub(super) fn load_confirmations(project: &Path) -> Confirmations {
    load(&confirmations_file(project))
}

/// Keeps the confirmations of the project.
pub(super) fn save_confirmations(project: &Path, confirmations: &Confirmations) {
    save(&confirmations_file(project), confirmations);
}

/// When each recurring order of the project last came (B47).
pub(super) fn load_schedule(project: &Path) -> Schedule {
    load(&schedule_file(project))
}

/// Keeps when each recurring order of the project last came.
pub(super) fn save_schedule(project: &Path, schedule: &Schedule) {
    save(&schedule_file(project), schedule);
}

/// The daily account across projects (E168).
pub(super) fn load_day(base: &Path) -> DailyAccount {
    load(&day_file(base))
}

/// Keeps the daily account.
pub(super) fn save_day(base: &Path, day: &DailyAccount) {
    save(&day_file(base), day);
}

/// The quota book across projects (C65).
pub(super) fn load_quotas(base: &Path) -> QuotaBook {
    load(&quotas_file(base))
}

/// Keeps the quota book.
pub(super) fn save_quotas(base: &Path, quotas: &QuotaBook) {
    save(&quotas_file(base), quotas);
}

#[cfg(test)]
mod tests {
    use ide_factory::budget::Spend;
    use ide_factory::event::{Event, Phase};
    use ide_factory::log::{LOG_VERSION, LogFailure, read};
    use ide_factory::{Stamp, workflow::rules::Confirmations};

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("leitstand-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn entry(order: &str, seq: u64) -> LogEntry {
        LogEntry {
            v: LOG_VERSION,
            order: OrderId(order.into()),
            seq,
            at: Stamp::new(seq, 1_700_000_000_000 + seq),
            event: Event::Phase { phase: Phase::Refs },
        }
    }

    fn line(order: &str, seq: u64) -> String {
        entry(order, seq).line().unwrap()
    }

    #[test]
    fn each_order_log_gets_its_own_lines_in_order_and_reads_back() {
        let project = temp("logs");
        let entries = [entry("a", 0), entry("b", 0), entry("a", 1)];
        let groups = group(&project, &entries);
        assert_eq!(groups.len(), 2);
        let appended = append_all(&groups);
        assert_eq!(appended.written, [(OrderId("a".into()), 2), (OrderId("b".into()), 1)]);
        assert!(append_all(&group(&project, &[entry("a", 2)])).failed.is_empty());
        let logs = read_logs(&project);
        assert_eq!(logs.len(), 2);
        let a = read(&logs[0]);
        assert_eq!(a.entries.iter().map(|e| e.seq).collect::<Vec<_>>(), [0, 1, 2]);
        assert!(!a.cut && a.stopped_at.is_none());
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn a_log_that_cannot_be_written_is_named() {
        let project = temp("unwritable");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(orders(&project), "a file where the folder should be").unwrap();
        let appended = append_all(&group(&project, &[entry("a", 0), entry("a", 1)]));
        let failure = LogFailure {
            order: OrderId("a".into()),
            seq: 0,
        };
        assert_eq!(appended.failed, [failure]);
        assert!(appended.holds_back(&OrderId("a".into())));
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn entries_that_do_not_continue_the_log_are_not_written() {
        let project = temp("gap");
        assert_eq!(append_all(&group(&project, &[entry("a", 0)])).written.len(), 1);
        let gap = append_all(&group(&project, &[entry("a", 2)]));
        assert_eq!(gap.refused, [OrderId("a".into())]);
        assert!(
            gap.failed.is_empty(),
            "the failure the gap follows was reported already"
        );
        let again = append_all(&group(&project, &[entry("a", 0)]));
        assert_eq!(again.refused.len(), 1, "an entry is never written twice");
        assert_eq!(append_all(&group(&project, &[entry("a", 1)])).written.len(), 1);
        let a = read(&std::fs::read_to_string(log_file(&project, &OrderId("a".into()))).unwrap());
        assert_eq!(a.entries.iter().map(|e| e.seq).collect::<Vec<_>>(), [0, 1]);
        let _ = std::fs::remove_dir_all(&project);
    }

    /// Restores the logs as the app does at its start, appends `seq` to order `a`, and reads its log back.
    fn restart_and_append(project: &Path, seq: u64) -> ide_factory::log::Read {
        let logs = read_logs(project);
        assert_eq!(logs.len(), 1);
        let appended = append_all(&group(project, &[entry("a", seq)]));
        assert_eq!(appended.written, [(OrderId("a".into()), seq + 1)]);
        read(&std::fs::read_to_string(log_file(project, &OrderId("a".into()))).unwrap())
    }

    #[test]
    fn a_crash_in_the_middle_of_a_line_is_cut_back_before_the_next_entry() {
        let project = temp("cut");
        let file = log_file(&project, &OrderId("a".into()));
        let whole = format!("{}{}", line("a", 0), line("a", 1));
        std::fs::create_dir_all(orders(&project)).unwrap();
        std::fs::write(&file, &whole[..whole.len() - 9]).unwrap();
        let back = restart_and_append(&project, 1);
        assert_eq!((back.entries.len(), back.cut, back.stopped_at), (2, false, None));
        assert!(file.with_extension("dropped").exists(), "the cut line is kept aside");
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn a_crash_before_the_line_break_keeps_the_entry_and_the_next_one_starts_a_line() {
        let project = temp("unbroken");
        let file = log_file(&project, &OrderId("a".into()));
        let whole = format!("{}{}", line("a", 0), line("a", 1));
        std::fs::create_dir_all(orders(&project)).unwrap();
        std::fs::write(&file, &whole[..whole.len() - 1]).unwrap();
        let back = restart_and_append(&project, 2);
        assert_eq!((back.entries.len(), back.cut, back.stopped_at), (3, false, None));
        std::fs::write(&file, &whole[..whole.len() - 1]).unwrap();
        let appended = append_all(&group(&project, &[entry("a", 2)]));
        assert_eq!(appended.written.len(), 1, "also without the repair at the start");
        let back = read(&std::fs::read_to_string(&file).unwrap());
        assert_eq!((back.entries.len(), back.stopped_at), (3, None));
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn a_damaged_line_is_set_aside_so_later_entries_are_read_again() {
        let project = temp("damaged");
        let file = log_file(&project, &OrderId("a".into()));
        std::fs::create_dir_all(orders(&project)).unwrap();
        std::fs::write(&file, format!("{}garbage\n{}", line("a", 0), line("a", 1))).unwrap();
        let back = restart_and_append(&project, 1);
        assert_eq!((back.entries.len(), back.stopped_at), (2, None));
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn a_last_line_longer_than_what_is_read_at_once_is_found() {
        let project = temp("long");
        let mut long = entry("a", 1);
        long.event = Event::Findings {
            text: "x".repeat(10_000),
            round: false,
        };
        let appended = append_all(&group(&project, &[entry("a", 0), long]));
        assert_eq!(appended.written, [(OrderId("a".into()), 2)]);
        assert_eq!(append_all(&group(&project, &[entry("a", 2)])).written.len(), 1);
        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn the_day_the_quotas_and_the_confirmations_come_back_as_kept() {
        let base = temp("kept");
        let mut day = DailyAccount::new("2026-11-16");
        day.add("2026-11-16", Some("anthropic-api"), &Spend::new(1.5, 100, 0.0));
        save_day(&base, &day);
        assert_eq!(load_day(&base), day);
        let quotas = QuotaBook::default();
        save_quotas(&base, &quotas);
        assert_eq!(load_quotas(&base), quotas);
        let project = project(&base, Path::new("/work/shop"));
        assert_eq!(load_confirmations(&project), Confirmations::default());
        let confirmed = Confirmations::new(["abc".to_string()]);
        save_confirmations(&project, &confirmed);
        assert!(load_confirmations(&project).has("abc"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn two_writers_of_one_file_each_leave_it_whole_and_nothing_beside_it() {
        let base = temp("whole");
        let file = base.join("intake.json");
        let writers: Vec<_> = [b'a', b'b']
            .into_iter()
            .map(|byte| {
                let file = file.clone();
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        write_atomically(&file, &[byte; 4096]).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let bytes = std::fs::read(&file).unwrap();
        assert!(bytes.len() == 4096 && bytes.iter().all(|b| *b == bytes[0]));
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }
}
