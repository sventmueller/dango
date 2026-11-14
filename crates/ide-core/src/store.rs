//! A JSON object in a file, with a version number, replaced as a whole so a crash leaves the old file or the new one.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use std::collections::HashMap;

use crate::stamp::Stamp;
use crate::sync::LockIgnoringPoison;
use std::sync::Mutex;

const VERSION_FIELD: &str = "version";

/// Held while a store reads, changes and writes its file, so two writers of one file never interleave.
static WRITING: Mutex<()> = Mutex::new(());

/// The files read last, as they were loaded, by path and stamp: the main thread asks for the settings on every caret
/// rest and every language server event, and one `stat` answers instead of a read and a parse.
static READ: Mutex<Option<HashMap<PathBuf, (Stamp, Store)>>> = Mutex::new(None);

/// Turns an object of one version into the next.
pub(crate) type Migration = fn(&mut Map<String, Value>);

/// A versioned JSON object read from and written to one file.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    path: PathBuf,
    values: Map<String, Value>,
    /// Why the file stays as it is: written by a newer version, whose fields this one would drop, or not readable as
    /// an object, which the user may be in the middle of editing.
    locked: Option<Locked>,
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

impl Store {
    /// The object in `path` brought to the version `migrations` lead to; empty if the file is missing or unreadable.
    pub(crate) fn load(path: PathBuf, migrations: &[Migration]) -> Store {
        let Some(stamp) = Stamp::of(&path) else {
            return Store::read(path, migrations);
        };
        if let Some((seen, store)) = READ.lock_ignoring_poison().get_or_insert_default().get(&path)
            && *seen == stamp
        {
            return store.clone();
        }
        let store = Store::read(path.clone(), migrations);
        // A file that changed while it was read keeps no entry, so the next load reads it again.
        if Stamp::of(&path) == Some(stamp) {
            READ.lock_ignoring_poison()
                .get_or_insert_default()
                .insert(path, (stamp, store.clone()));
        }
        store
    }

    fn read(path: PathBuf, migrations: &[Migration]) -> Store {
        let (values, problem) = match std::fs::read(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Map::new(), None),
            Err(_) => (Map::new(), Some(Locked::Unreadable)),
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
        let mut store = Store { path, values, locked };
        if store.locked.is_none() {
            migrate(&mut store.values, found, migrations);
        }
        store
    }

    /// Why the file is not written, if it is not.
    pub(crate) fn locked(&self) -> Option<Locked> {
        self.locked
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn values(&self) -> &Map<String, Value> {
        &self.values
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
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

    /// Reads the file again, makes `change` to what it holds now and writes it, all under one lock, so that a write
    /// from another place of the app between this store's load and now is kept.
    fn update(&mut self, migrations: &[Migration], change: impl FnOnce(&mut Map<String, Value>)) -> io::Result<()> {
        let _writing = WRITING.lock_ignoring_poison();
        let fresh = Store::load(self.path.clone(), migrations);
        if let Some(locked) = fresh.locked.or(self.locked) {
            return Err(io::Error::other(format!(
                "{} stays as it is: {locked:?}",
                self.path.display()
            )));
        }
        // The store takes the change only once it is on disk, so a failed write leaves it as the file is.
        let mut values = fresh.values;
        change(&mut values);
        values.insert(VERSION_FIELD.to_string(), Value::from(current_version(migrations)));
        let value = Value::Object(values);
        write_atomically(&self.path, &value)?;
        let Value::Object(values) = value else {
            unreachable!("built as an object above")
        };
        self.values = values;
        Ok(())
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

/// Writes `value` beside `path` and moves it over the old file.
pub(crate) fn write_atomically(path: &Path, value: &Value) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    bytes.push(b'\n');
    let temporary = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("store"),
        std::process::id()
    ));
    std::fs::write(&temporary, &bytes)?;
    std::fs::rename(&temporary, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })
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
}
