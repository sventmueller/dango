//! The sync of the user's settings between their Macs (B28, E112–E114): a folder in iCloud Drive named like the app's
//! data folder, with a file per Mac that only that Mac writes. Reading and writing go through `NSFileCoordinator`, on
//! a worker thread with a time limit, since iCloud fetches evicted files only when they are read; an `NSFilePresenter`
//! hears when another Mac wrote. Changes reach this Mac through the settings' one writer, which tells the windows.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use block2::RcBlock;
use ide_core::explain::Explain;
use ide_core::settings::{Key, Settings};
use ide_core::settings_sync::{DeviceFile, Inputs, Round};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_foundation::{
    NSFileCoordinator, NSFileCoordinatorReadingOptions, NSFileCoordinatorWritingOptions, NSFileManager,
    NSFilePresenter, NSObject, NSObjectProtocol, NSOperationQueue, NSURL,
};
use serde_json::{Map, Value};
use splat_i18n::t;

use crate::context::AppContext;

/// Whether this Mac syncs its settings (E112); off at first.
pub(crate) const ENABLED: Key<bool> = Key::user("settingsSync", ide_core::settings::Sharing::PerMac, || false);
/// The folder the user picked in place of iCloud Drive's, after macOS refused access to it.
pub(crate) const FOLDER: Key<Option<PathBuf>> =
    Key::user("settingsSyncFolder", ide_core::settings::Sharing::PerMac, || None);

/// How long reading the folder may take before the round gives up.
const LIMIT: Duration = Duration::from_secs(20);
/// How long local changes rest before they go out.
const SETTLE: Duration = Duration::from_secs(1);
/// The prefix of a shortcut of `keybindings.json` among the synced settings, one per command.
const SHORTCUT: &str = "keybinding:";

/// What the sync does now, as the settings show it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Status {
    /// Turned off.
    #[default]
    Off,
    /// iCloud Drive is off on this Mac.
    NoICloud,
    /// It runs.
    Running,
    /// Another Mac runs a newer version of the app, whose files this one leaves out.
    Paused,
    /// The folder could not be read or written, with why.
    Failed(String),
}

/// The sync's state in the app.
#[derive(Default)]
pub(crate) struct SettingsSync {
    status: RefCell<Status>,
    presenter: RefCell<Option<Retained<Presenter>>>,
    /// Counts the requests for a round, so that a burst of changes makes one.
    asked: Cell<u64>,
    running: Cell<bool>,
}

/// The folder in iCloud Drive, named like the app's data folder, so debug and release builds keep apart (E18).
fn icloud_folder() -> PathBuf {
    let name = ide_core::paths::data_dir()
        .file_name()
        .map_or_else(|| ide_product::BUNDLE_ID.into(), |n| n.to_string_lossy().into_owned());
    ide_core::paths::home()
        .join("Library/Mobile Documents/com~apple~CloudDocs")
        .join(name)
}

/// Whether iCloud Drive is on for this Mac's user.
fn icloud_on() -> bool {
    let signed_in = NSFileManager::defaultManager().ubiquityIdentityToken().is_some();
    signed_in
        && ide_core::paths::home()
            .join("Library/Mobile Documents/com~apple~CloudDocs")
            .is_dir()
}

/// The folder the sync uses: the one the user picked, else iCloud Drive's.
fn folder() -> PathBuf {
    Settings::load_user().get(&FOLDER).unwrap_or_else(icloud_folder)
}

/// This Mac's id, made once and kept in the data folder, never synced.
fn device_id() -> String {
    let path = ide_core::paths::data_dir().join("device-id");
    if let Ok(id) = std::fs::read_to_string(&path)
        && !id.trim().is_empty()
    {
        return id.trim().to_string();
    }
    let mut bytes = [0u8; 8];
    if let Ok(mut random) = std::fs::File::open("/dev/urandom") {
        let _ = std::io::Read::read_exact(&mut random, &mut bytes);
    }
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(error) = std::fs::write(&path, &id) {
        crate::diag::report("keeping this Mac's id for the settings sync", error);
    }
    id
}

/// The sync's status now.
pub(crate) fn status(cx: &AppContext) -> Status {
    let status = cx.settings_sync().status.borrow().clone();
    if !Settings::load_user().get(&ENABLED) {
        return Status::Off;
    }
    if status == Status::Off && Settings::load_user().get(&FOLDER).is_none() && !icloud_on() {
        return Status::NoICloud;
    }
    status
}

/// Whether the sync can be turned on: iCloud Drive is on, or the user picked a folder.
pub(crate) fn available() -> bool {
    icloud_on() || Settings::load_user().get(&FOLDER).is_some()
}

/// At launch: starts the sync when it is on.
pub(crate) fn launched(cx: &AppContext, mtm: MainThreadMarker) {
    if Settings::load_user().get(&ENABLED) {
        start(cx, mtm);
    }
}

/// Turns the sync on or off and remembers it.
pub(crate) fn set_enabled(cx: &AppContext, mtm: MainThreadMarker, on: bool) {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&ENABLED, &on) {
        crate::diag::report("saving the settings sync's switch", error);
    }
    if on {
        start(cx, mtm);
    } else {
        if let Some(presenter) = cx.settings_sync().presenter.borrow_mut().take() {
            NSFileCoordinator::removeFilePresenter(ProtocolObject::from_ref(&*presenter));
        }
        *cx.settings_sync().status.borrow_mut() = Status::Off;
    }
}

/// Uses `picked` in place of iCloud Drive's folder, as the way around a refused access.
pub(crate) fn use_folder(cx: &AppContext, mtm: MainThreadMarker, picked: &Path) {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&FOLDER, &Some(picked.to_path_buf())) {
        crate::diag::report("saving the settings sync's folder", error);
    }
    if settings.get(&ENABLED) {
        start(cx, mtm);
    }
}

/// Watches the sync's folder and asks for a round.
fn start(cx: &AppContext, mtm: MainThreadMarker) {
    if cx.settings_sync().presenter.borrow().is_none() {
        let folder = folder();
        if let Err(error) = std::fs::create_dir_all(&folder) {
            *cx.settings_sync().status.borrow_mut() = Status::Failed(error.explain());
            return;
        }
        let presenter = Presenter::new(&folder);
        NSFileCoordinator::addFilePresenter(ProtocolObject::from_ref(&*presenter));
        *cx.settings_sync().presenter.borrow_mut() = Some(presenter);
    }
    *cx.settings_sync().status.borrow_mut() = Status::Running;
    request(mtm);
}

/// A setting changed here, or another Mac wrote: a round follows once things rest.
pub(crate) fn request(mtm: MainThreadMarker) {
    if !Settings::load_user().get(&ENABLED) {
        return;
    }
    let cx = AppContext::get(mtm);
    let asked = cx.settings_sync().asked.get() + 1;
    cx.settings_sync().asked.set(asked);
    crate::util::main_after(SETTLE, move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let cx = AppContext::get(mtm);
        if cx.settings_sync().asked.get() != asked {
            return;
        }
        if cx.settings_sync().running.replace(true) {
            return crate::util::main_after(SETTLE, request_again);
        }
        run_round(folder(), device_id());
    });
}

fn request_again() {
    if let Some(mtm) = MainThreadMarker::new() {
        request(mtm);
    }
}

/// Reads the folder, decides and writes this Mac's file on a worker thread, within `LIMIT`; the main thread applies.
fn run_round(folder: PathBuf, device: String) {
    let (sent, received) = mpsc::channel();
    let worker = crate::util::spawn("settings-sync", move || {
        let _ = sent.send(round_in(&folder, &device));
    });
    let waited = crate::util::spawn("settings-sync-wait", move || {
        let outcome = received.recv_timeout(LIMIT).unwrap_or(Err(SyncError::TimedOut));
        crate::util::main_soon(move || finished(outcome));
    });
    if let Err(why) = worker.and(waited) {
        finished(Err(SyncError::NoThread(why)));
    }
}

/// Why a round of syncing did not finish.
#[derive(Debug)]
enum SyncError {
    TimedOut,
    Io(std::io::Error),
    Encode(serde_json::Error),
    /// The file coordinator refused, in the system's words.
    Coordinator(String),
    /// The coordinator never handed the file over.
    NotWritten,
    NoThread(ide_core::explain::Explained),
}

impl Explain for SyncError {
    fn explain(&self) -> String {
        match self {
            SyncError::TimedOut => t("iCloud Drive hat nicht rechtzeitig geantwortet").to_string(),
            SyncError::Io(error) => error.explain(),
            SyncError::Encode(error) => error.to_string(),
            SyncError::Coordinator(why) => why.clone(),
            SyncError::NotWritten => t("iCloud Drive hat die Datei nicht zum Schreiben freigegeben").to_string(),
            SyncError::NoThread(why) => why.explain(),
        }
    }
}

fn round_in(folder: &Path, device: &str) -> Result<Round, SyncError> {
    let mut own = None;
    let mut others = Vec::new();
    let entries = std::fs::read_dir(folder).map_err(SyncError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Some(bytes) = coordinated_read(&path) else { continue };
        let Ok(file) = serde_json::from_slice::<DeviceFile>(&bytes) else {
            continue;
        };
        if file.device == device {
            own = Some(file);
        } else {
            others.push(file);
        }
    }
    let local = local_values();
    let shared = |name: &str| name.starts_with(SHORTCUT) || registry::is_shared(name);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    let inputs = Inputs {
        device,
        others: &others,
        local: &local,
        shared: &shared,
        schema: Settings::schema_version(),
        migrate: &Settings::migrate_values,
        now,
    };
    let round = ide_core::settings_sync::round(own, &inputs);
    let bytes = serde_json::to_vec_pretty(&round.own).map_err(SyncError::Encode)?;
    coordinated_write(&folder.join(format!("{device}.json")), &bytes)?;
    Ok(round)
}

/// The synced settings of this Mac: the settings file's values and a shortcut per command of `keybindings.json`.
fn local_values() -> Map<String, Value> {
    let mut values = Settings::load_user().user_values();
    let shortcuts = std::fs::read_to_string(ide_core::keymap::Keymap::user_path())
        .ok()
        .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
        .unwrap_or_default();
    for (command, key) in shortcuts {
        values.insert(format!("{SHORTCUT}{command}"), key);
    }
    values
}

fn coordinated_read(path: &Path) -> Option<Vec<u8>> {
    let url = NSURL::fileURLWithPath(&crate::util::ns(&path.to_string_lossy()));
    let read: RefCell<Option<Vec<u8>>> = RefCell::new(None);
    let block = RcBlock::new(|at: NonNull<NSURL>| {
        crate::util::guarded((), || {
            // SAFETY: The coordinator hands a valid URL for the call.
            let at = unsafe { at.as_ref() };
            if let Some(path) = at.path() {
                *read.borrow_mut() = std::fs::read(path.to_string()).ok();
            }
        });
    });
    let coordinator = NSFileCoordinator::initWithFilePresenter(NSFileCoordinator::alloc(), None);
    let mut error = None;
    coordinator.coordinateReadingItemAtURL_options_error_byAccessor(
        &url,
        NSFileCoordinatorReadingOptions::empty(),
        Some(&mut error),
        &block,
    );
    drop(block);
    read.into_inner()
}

fn coordinated_write(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
    let url = NSURL::fileURLWithPath(&crate::util::ns(&path.to_string_lossy()));
    let outcome: RefCell<Result<(), SyncError>> = RefCell::new(Err(SyncError::NotWritten));
    let block = RcBlock::new(|at: NonNull<NSURL>| {
        crate::util::guarded((), || {
            // SAFETY: The coordinator hands a valid URL for the call.
            let at = unsafe { at.as_ref() };
            if let Some(path) = at.path() {
                *outcome.borrow_mut() = std::fs::write(path.to_string(), bytes).map_err(SyncError::Io);
            }
        });
    });
    let coordinator = NSFileCoordinator::initWithFilePresenter(NSFileCoordinator::alloc(), None);
    let mut error = None;
    coordinator.coordinateWritingItemAtURL_options_error_byAccessor(
        &url,
        NSFileCoordinatorWritingOptions::ForReplacing,
        Some(&mut error),
        &block,
    );
    drop(block);
    if let Some(error) = error {
        return Err(SyncError::Coordinator(error.localizedDescription().to_string()));
    }
    outcome.into_inner()
}

/// Applies what a round decided, through the settings' writer, which tells the windows.
fn finished(outcome: Result<Round, SyncError>) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let cx = AppContext::get(mtm);
    cx.settings_sync().running.set(false);
    let round = match outcome {
        Ok(round) => round,
        Err(why) => {
            *cx.settings_sync().status.borrow_mut() = Status::Failed(why.explain());
            return;
        }
    };
    *cx.settings_sync().status.borrow_mut() = if round.newer.is_empty() {
        Status::Running
    } else {
        Status::Paused
    };
    let mut settings = Settings::load_user();
    let mut shortcuts_changed = false;
    for (name, value) in round.apply {
        if let Some(command) = name.strip_prefix(SHORTCUT) {
            apply_shortcut(command, value);
            shortcuts_changed = true;
            continue;
        }
        let written = match value {
            Some(value) => settings.set_value(&name, value),
            None => settings.remove(&name),
        };
        if let Err(error) = written {
            crate::diag::report("taking over a synced setting", error);
        }
    }
    if shortcuts_changed {
        crate::menu::reinstall(mtm);
    }
}

fn apply_shortcut(command: &str, key: Option<Value>) {
    let path = ide_core::keymap::Keymap::user_path();
    let mut shortcuts = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
        .unwrap_or_default();
    match key {
        Some(key) => shortcuts.insert(command.to_string(), key),
        None => shortcuts.remove(command),
    };
    let written = serde_json::to_vec_pretty(&shortcuts)
        .map_err(std::io::Error::other)
        .and_then(|bytes| std::fs::write(&path, bytes));
    if let Err(error) = written {
        crate::diag::report("taking over a synced shortcut", error);
    }
}

pub(crate) struct PresenterIvars {
    url: Retained<NSURL>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Presenter` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[name = "IDESettingsSyncPresenter"]
    #[ivars = PresenterIvars]
    pub(crate) struct Presenter;

    unsafe impl NSObjectProtocol for Presenter {}

    unsafe impl NSFilePresenter for Presenter {
        #[unsafe(method_id(presentedItemURL))]
        fn presented_item_url(&self) -> Option<Retained<NSURL>> {
            Some(self.ivars().url.clone())
        }

        #[unsafe(method_id(presentedItemOperationQueue))]
        fn presented_item_operation_queue(&self) -> Retained<NSOperationQueue> {
            NSOperationQueue::mainQueue()
        }

        #[unsafe(method(presentedSubitemDidChangeAtURL:))]
        fn presented_subitem_did_change(&self, _url: &NSURL) {
            crate::util::guarded((), request_again);
        }

        #[unsafe(method(presentedItemDidChange))]
        fn presented_item_did_change(&self) {
            crate::util::guarded((), request_again);
        }
    }
);

impl Presenter {
    fn new(folder: &Path) -> Retained<Presenter> {
        let url = NSURL::fileURLWithPath_isDirectory(&crate::util::ns(&folder.to_string_lossy()), true);
        let this = Self::alloc().set_ivars(PresenterIvars { url });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

mod registry;
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
