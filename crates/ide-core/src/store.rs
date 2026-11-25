//! A JSON object in a file, with a version number, replaced as a whole so a crash leaves the old file or the new one.
//! Every store of a file shares one snapshot of it, which a write replaces at once; a store written behind hands the
//! write to the disk lane, so the main thread never waits for `fsync`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};

use crate::stamp::Stamp;
use crate::sync::LockIgnoringPoison;

pub(crate) const VERSION_FIELD: &str = "version";

/// Held while a file is written, so two writers of one file never interleave.
static WRITING: Mutex<()> = Mutex::new(());

/// Held while a store reads what a file holds, changes it and puts the result in place as the file's snapshot.
static CHANGING: Mutex<()> = Mutex::new(());

/// The files read or written so far, as the app knows them: the main thread asks for the settings on every caret rest
/// and every language server event, and one `stat` answers instead of a read, a parse and a copy.
static FILES: Mutex<BTreeMap<PathBuf, Entry>> = Mutex::new(BTreeMap::new());

/// Turns an object of one version into the next.
pub(crate) type Migration = fn(&mut Map<String, Value>);

/// What a file holds, shared by every store of it.
#[derive(Debug)]
struct Values {
    values: Map<String, Value>,
    /// Why the file stays as it is: written by a newer version, whose fields this one would drop, or not readable as
    /// an object, which the user may be in the middle of editing.
    locked: Option<Locked>,
}

/// A file as the app knows it.
struct Entry {
    values: Arc<Values>,
    /// The file's stamp when `values` were read or last written; `None` until it is known.
    stamp: Option<Stamp>,
    /// How many changes were made, and how many of them are on disk.
    changed: u64,
    written: u64,
    /// A write of the file waits on the disk lane.
    scheduled: bool,
}

impl Entry {
    fn read(values: Arc<Values>, stamp: Option<Stamp>) -> Entry {
        Entry {
            values,
            stamp,
            changed: 0,
            written: 0,
            scheduled: false,
        }
    }

    /// Whether the snapshot holds changes the file does not have yet, so it wins over the file.
    fn unwritten(&self) -> bool {
        self.written < self.changed
    }
}

fn with_files<R>(f: impl FnOnce(&mut BTreeMap<PathBuf, Entry>) -> R) -> R {
    f(&mut FILES.lock_ignoring_poison())
}

/// A versioned JSON object read from and written to one file.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    path: PathBuf,
    values: Arc<Values>,
    /// Whether a change returns before it is on disk, which the disk lane then writes.
    behind: bool,
}

/// Why a settings or state file is not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locked {
    /// Written by a newer version of the app.
    Newer,
    /// Not a JSON object.
    Damaged,
    /// There, but not readable, such as without permission or on a failing disk.
    Unreadable,
}

/// Whether every change made by a store written behind is on disk.
pub(crate) fn all_written() -> bool {
    with_files(|files| files.values().all(|entry| !entry.unwritten()))
}

impl Store {
    /// The object in `path` brought to the version `migrations` lead to; empty if the file is missing or unreadable.
    pub(crate) fn load(path: PathBuf, migrations: &[Migration]) -> Store {
        let values = current(&path, migrations);
        Store {
            path,
            values,
            behind: false,
        }
    }

    /// `load`, whose changes are written on the disk lane: they show in every store of the file at once, a locked
    /// file still refuses them at once, and a failed write is reported.
    pub(crate) fn load_behind(path: PathBuf, migrations: &[Migration]) -> Store {
        Store {
            behind: true,
            ..Store::load(path, migrations)
        }
    }

    fn read(path: &Path, migrations: &[Migration]) -> Values {
        let (values, problem) = match std::fs::read(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Map::new(), None),
            Err(_) => (Map::new(), Some(Locked::Unreadable)),
            Ok(bytes) if crate::atomic_write::is_torn(&bytes) => (Map::new(), None),
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(map)) => (map, None),
                _ => (Map::new(), Some(Locked::Damaged)),
            },
        };
        let current = current_version(migrations);
        let found = values.get(VERSION_FIELD).and_then(Value::as_u64).unwrap_or(current);
        let locked = if problem.is_some() {
            problem
        } else if found > current {
            Some(Locked::Newer)
        } else {
            None
        };
        let mut read = Values { values, locked };
        if read.locked.is_none() {
            migrate(&mut read.values, found, migrations);
        }
        read
    }

    /// Why the file is not written, if it is not.
    pub(crate) fn locked(&self) -> Option<Locked> {
        self.values.locked
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn values(&self) -> &Map<String, Value> {
        &self.values.values
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        self.values.values.get(name)
    }

    /// Sets `name` and writes the file.
    pub(crate) fn set(&mut self, name: &str, value: Value, migrations: &[Migration]) -> io::Result<()> {
        self.update(migrations, |values| {
            values.insert(name.to_string(), value);
        })
    }

    /// Takes `name` out and writes the file, so that it falls back to its default.
    pub(crate) fn remove(&mut self, name: &str, migrations: &[Migration]) -> io::Result<()> {
        self.update(migrations, |values| {
            values.remove(name);
        })
    }

    /// Makes `change` to what the file holds now, so that a write from another place of the app between this
    /// store's load and now is kept, and writes it: here, or on the disk lane for a store written behind.
    fn update(&mut self, migrations: &[Migration], change: impl FnOnce(&mut Map<String, Value>)) -> io::Result<()> {
        let _writing = (!self.behind).then(|| WRITING.lock_ignoring_poison());
        let values = {
            let _changing = CHANGING.lock_ignoring_poison();
            let fresh = current(&self.path, migrations);
            if let Some(locked) = fresh.locked.or(self.values.locked) {
                return Err(io::Error::other(format!(
                    "{} stays as it is: {locked:?}",
                    self.path.display()
                )));
            }
            let mut values = fresh.values.clone();
            change(&mut values);
            values.insert(VERSION_FIELD.to_string(), Value::from(current_version(migrations)));
            let values = Arc::new(Values { values, locked: None });
            with_files(|files| {
                let entry = files
                    .entry(self.path.clone())
                    .or_insert_with(|| Entry::read(values.clone(), None));
                entry.values = values.clone();
                entry.changed += 1;
            });
            values
        };
        if self.behind {
            schedule(&self.path);
        } else {
            // The store takes the change only once it is on disk, so a failed write leaves it as the file is.
            write_out(&self.path)?;
        }
        self.values = values;
        Ok(())
    }
}

/// What `path` holds now: the snapshot while it has changes the file lacks or the file kept its stamp, else the file
/// read again.
fn current(path: &Path, migrations: &[Migration]) -> Arc<Values> {
    let unwritten = with_files(|files| {
        files
            .get(path)
            .filter(|entry| entry.unwritten())
            .map(|entry| entry.values.clone())
    });
    if let Some(values) = unwritten {
        return values;
    }
    let stamp = Stamp::of(path);
    let kept = with_files(|files| {
        files
            .get(path)
            .filter(|entry| entry.unwritten() || (stamp.is_some() && entry.stamp == stamp))
            .map(|entry| entry.values.clone())
    });
    if let Some(values) = kept {
        return values;
    }
    let read = Arc::new(Store::read(path, migrations));
    // A file that changed while it was read is not kept, so the next question reads it again; without a file nothing
    // is kept either.
    if stamp.is_none() || Stamp::of(path) != stamp {
        return read;
    }
    with_files(|files| match files.get_mut(path) {
        Some(entry) if entry.unwritten() => entry.values.clone(),
        Some(entry) => {
            entry.values = read.clone();
            entry.stamp = stamp;
            read
        }
        None => {
            files.insert(path.to_path_buf(), Entry::read(read.clone(), stamp));
            read
        }
    })
}

/// Writes `path`'s snapshot if it has changes the file lacks; the caller holds `WRITING`.
fn write_out(path: &Path) -> io::Result<()> {
    let pending = with_files(|files| {
        files
            .get(path)
            .filter(|entry| entry.unwritten())
            .map(|entry| (entry.values.clone(), entry.changed))
    });
    let Some((values, changed)) = pending else {
        return Ok(());
    };
    let result = crate::atomic_write::write_json(path, &values.values);
    with_files(|files| match &result {
        Ok(()) => {
            if let Some(entry) = files.get_mut(path) {
                entry.written = entry.written.max(changed);
                if !entry.unwritten() {
                    entry.stamp = Stamp::of(path);
                }
            }
        }
        // The app then knows the file as it is on disk again, unless a later change is still to be written.
        Err(_) => {
            if files.get(path).is_some_and(|entry| entry.changed == changed) {
                files.remove(path);
            }
        }
    });
    result
}

/// Has the disk lane write `path`'s snapshot, unless a write of it waits there already; here if the lane takes none.
fn schedule(path: &Path) {
    let first = with_files(|files| {
        files
            .get_mut(path)
            .is_some_and(|entry| !std::mem::replace(&mut entry.scheduled, true))
    });
    if !first {
        return;
    }
    let queued = path.to_path_buf();
    if crate::lane::DISK.try_run(move || write_behind(&queued)).is_err() {
        write_behind(path);
    }
}

fn write_behind(path: &Path) {
    with_files(|files| {
        if let Some(entry) = files.get_mut(path) {
            entry.scheduled = false;
        }
    });
    let _writing = WRITING.lock_ignoring_poison();
    if let Err(error) = write_out(path) {
        crate::diag::report(&format!("writing {}", path.display()), error);
    }
}

fn current_version(migrations: &[Migration]) -> u64 {
    1 + migrations.len() as u64
}

/// Brings `values` of version `found` to the version `migrations` lead to.
pub(crate) fn migrate_from(values: &mut Map<String, Value>, found: u64, migrations: &[Migration]) {
    migrate(values, found, migrations);
}

/// Applies the migrations from `found` on; `migrations[n]` turns version `n + 1` into `n + 2`.
fn migrate(values: &mut Map<String, Value>, found: u64, migrations: &[Migration]) {
    let from = usize::try_from(found.saturating_sub(1)).unwrap_or(usize::MAX);
    for migration in migrations.iter().skip(from) {
        migration(values);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn a_failed_write_leaves_the_store_as_the_file_is() {
        let dir = scratch("failed-write");
        // A folder where the file should be makes the move over it fail.
        let path = dir.join("settings.json");
        let mut store = Store::load(path.clone(), &[]);
        std::fs::create_dir_all(path.join("blocker")).unwrap();
        assert!(store.set("a", Value::from(1), &[]).is_err());
        assert_eq!(store.get("a"), None);
    }

    #[test]
    fn a_write_the_disk_refuses_keeps_the_old_file_and_the_store_and_leaves_no_temporary_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("refused-write");
        let path = dir.join("settings.json");
        let mut store = Store::load(path.clone(), &[]);
        store.set("a", Value::from(1), &[]).unwrap();
        let before = std::fs::read(&path).unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let refused = store.set("a", Value::from(2), &[]);
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(refused.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(store.get("a"), Some(&Value::from(1)));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_write_keeps_what_another_store_of_the_file_wrote_since_its_load() {
        let dir = scratch("read-modify-write");
        let path = dir.join("settings.json");
        let mut first = Store::load(path.clone(), &[]);
        let mut second = Store::load(path.clone(), &[]);
        first.set("a", Value::from(1), &[]).unwrap();
        second.set("b", Value::from(2), &[]).unwrap();
        let after = Store::load(path.clone(), &[]);
        assert_eq!(after.get("a"), Some(&Value::from(1)));
        assert_eq!(after.get("b"), Some(&Value::from(2)));
        second.remove("a", &[]).unwrap();
        assert_eq!(Store::load(path, &[]).get("a"), None);
    }

    pub(crate) use crate::testing::scratch;

    fn rename_size(values: &mut Map<String, Value>) {
        if let Some(size) = values.remove("size") {
            values.insert("fontSize".into(), size);
        }
    }

    #[test]
    fn a_missing_file_is_an_empty_store_that_writes_itself() {
        let scratch_dir = scratch("missing");
        let path = scratch_dir.join("nested/settings.json");
        let mut store = Store::load(path.clone(), &[]);
        assert!(store.get("anything").is_none());
        store.set("a", Value::from(1), &[]).unwrap();
        let again = Store::load(path, &[]);
        assert_eq!(again.get("a"), Some(&Value::from(1)));
        assert_eq!(again.get(VERSION_FIELD), Some(&Value::from(1)));
    }

    #[test]
    fn an_older_file_is_migrated_and_written_with_the_new_version() {
        let scratch_dir = scratch("migrate");
        let path = scratch_dir.join("settings.json");
        std::fs::write(&path, r#"{"version": 1, "size": 13}"#).unwrap();
        let migrations: &[Migration] = &[rename_size];
        let mut store = Store::load(path.clone(), migrations);
        assert_eq!(store.get("fontSize"), Some(&Value::from(13)));
        assert!(store.get("size").is_none());
        store.set("other", Value::Bool(true), migrations).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written[VERSION_FIELD], Value::from(2));
    }

    #[test]
    fn a_file_from_a_newer_version_is_read_but_never_overwritten() {
        let scratch_dir = scratch("newer");
        let path = scratch_dir.join("settings.json");
        let original = r#"{"version": 9, "future": "kept"}"#;
        std::fs::write(&path, original).unwrap();
        let mut store = Store::load(path.clone(), &[]);
        assert_eq!(store.get("future"), Some(&Value::from("kept")));
        assert_eq!(store.locked(), Some(Locked::Newer));
        assert!(store.set("x", Value::from(1), &[]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn a_damaged_file_reads_as_empty_and_is_left_for_the_user_to_mend() {
        let scratch_dir = scratch("damaged");
        let path = scratch_dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let mut store = Store::load(path.clone(), &[]);
        assert!(store.get("anything").is_none());
        assert_eq!(store.locked(), Some(Locked::Damaged));
        assert!(store.set("a", Value::from("b"), &[]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn an_empty_file_left_by_a_power_loss_reads_as_empty_and_is_written_again() {
        let scratch_dir = scratch("empty");
        let path = scratch_dir.join("settings.json");
        for torn in ["", "\n  \n", "\0\0\0\0"] {
            std::fs::write(&path, torn).unwrap();
            let mut store = Store::load(path.clone(), &[]);
            assert_eq!(store.locked(), None);
            store.set("a", Value::from(1), &[]).unwrap();
            assert_eq!(Store::load(path.clone(), &[]).get("a"), Some(&Value::from(1)));
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_is_never_overwritten() {
        use std::os::unix::fs::PermissionsExt;
        let scratch_dir = scratch("unreadable");
        let path = scratch_dir.join("settings.json");
        let original = r#"{"version": 1, "kept": true}"#;
        std::fs::write(&path, original).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o200)).unwrap();
        let mut store = Store::load(path.clone(), &[]);
        assert_eq!(store.locked(), Some(Locked::Unreadable));
        assert!(store.set("a", Value::from("b"), &[]).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn a_missing_file_is_written() {
        let scratch_dir = scratch("missing");
        let path = scratch_dir.join("settings.json");
        let mut store = Store::load(path.clone(), &[]);
        assert_eq!(store.locked(), None);
        store.set("a", Value::from("b"), &[]).unwrap();
        assert!(path.exists());
    }

    fn written_soon(path: &Path) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while with_files(|files| files.get(path).is_some_and(Entry::unwritten)) {
            assert!(std::time::Instant::now() < until, "{} was not written", path.display());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_change_written_behind_shows_at_once_reaches_the_disk_and_yields_to_a_later_outside_change() {
        let dir = scratch("behind");
        let path = dir.join("state.json");
        let mut store = Store::load_behind(path.clone(), &[]);
        for n in 0..20 {
            store.set("n", Value::from(n), &[]).unwrap();
        }
        assert_eq!(store.get("n"), Some(&Value::from(19)));
        assert_eq!(Store::load(path.clone(), &[]).get("n"), Some(&Value::from(19)));
        written_soon(&path);
        let on_disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(on_disk["n"], Value::from(19));
        std::fs::write(&path, r#"{"version": 1, "n": "from outside"}"#).unwrap();
        assert_eq!(
            Store::load(path.clone(), &[]).get("n"),
            Some(&Value::from("from outside"))
        );
        let mut other = Store::load_behind(path.clone(), &[]);
        other.set("m", Value::from(1), &[]).unwrap();
        written_soon(&path);
        let on_disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            (&on_disk["n"], &on_disk["m"]),
            (&Value::from("from outside"), &Value::from(1))
        );
    }

    #[test]
    fn a_locked_file_refuses_a_change_written_behind_at_once() {
        let dir = scratch("behind-locked");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let mut store = Store::load_behind(path.clone(), &[]);
        assert!(store.set("a", Value::from(1), &[]).is_err());
        assert_eq!(Store::load(path.clone(), &[]).get("a"), None);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn stores_of_one_file_share_its_snapshot() {
        let dir = scratch("shared");
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{"version": 1, "a": 1}"#).unwrap();
        let first = Store::load(path.clone(), &[]);
        let second = Store::load(path, &[]);
        assert!(Arc::ptr_eq(&first.values, &second.values));
    }
}
