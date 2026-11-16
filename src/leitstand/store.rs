//! What the Leitstand keeps on this Mac (E167, E168, E190): each order's log as JSON lines and its specification, the
//! confirmations of each project, and across projects the daily account and the quota book, all in the app's data
//! folder and never in a repository. Everything here blocks and belongs on the disk lane.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use ide_factory::OrderId;
use ide_factory::budget::DailyAccount;
use ide_factory::log::LogEntry;
use ide_factory::quota::QuotaBook;
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

fn day_file(base: &Path) -> PathBuf {
    base.join("day.json")
}

fn quotas_file(base: &Path) -> PathBuf {
    base.join("quotas.json")
}

/// The texts of every order log of the project, in the order of their names.
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
            Ok(text) => Some(text),
            Err(error) => {
                ide_core::diag::report("reading an order log", error);
                None
            }
        })
        .collect()
}

/// `entries` grouped by their order's log, each group's lines in order.
pub(super) fn group(project: &Path, entries: &[LogEntry]) -> Vec<(OrderId, PathBuf, String)> {
    let mut groups: BTreeMap<OrderId, String> = BTreeMap::new();
    for entry in entries {
        groups.entry(entry.order.clone()).or_default().push_str(&entry.line());
    }
    groups
        .into_iter()
        .map(|(order, text)| {
            let file = log_file(project, &order);
            (order, file, text)
        })
        .collect()
}

/// Appends each group to its log, on disk before it returns; the orders whose log could not be written.
pub(super) fn append_all(groups: &[(OrderId, PathBuf, String)]) -> Vec<OrderId> {
    groups
        .iter()
        .filter_map(|(order, file, text)| match append(file, text) {
            Ok(()) => None,
            Err(error) => {
                ide_core::diag::report("writing an order log", error);
                Some(order.clone())
            }
        })
        .collect()
}

fn append(file: &Path, text: &str) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(file)?;
    log.write_all(text.as_bytes())?;
    log.sync_data()
}

/// Writes `text` to `file` whole, through a file beside it, so a crash leaves the old or the new one.
pub(super) fn write_atomically(file: &Path, text: &[u8]) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let beside = file.with_extension("partial");
    std::fs::write(&beside, text)?;
    std::fs::rename(&beside, file)
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
    use ide_factory::log::{LOG_VERSION, read};
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

    #[test]
    fn each_order_log_gets_its_own_lines_in_order_and_reads_back() {
        let project = temp("logs");
        let entries = [entry("a", 0), entry("b", 0), entry("a", 1)];
        let groups = group(&project, &entries);
        assert_eq!(groups.len(), 2);
        assert!(append_all(&groups).is_empty());
        assert!(append_all(&group(&project, &[entry("a", 2)])).is_empty());
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
        let failed = append_all(&group(&project, &[entry("a", 0)]));
        assert_eq!(failed, [OrderId("a".into())]);
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
}
