//! What the windows show, written down while the app runs, and put back at launch together with the unsaved text a
//! crash left behind (E24). The record follows each change of windows or tabs a moment later, so a crash leaves the
//! latest one behind; the copies of unsaved text are `editor::recovery`'s. The record names each window's folder and
//! editors; the tabs of every kind are the folder's layout (E60), which a launch builds first and then fills with the
//! copies.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_core::LockIgnoringPoison;

use ide_core::settings::{Settings, State, keys, remembered};
use ide_document::recovery::{Pending, Recovery, RecoveryId};
use objc2::MainThreadMarker;
use serde::{Deserialize, Serialize};
use splat_i18n::{t, tf};

use crate::context::app;
use crate::util::{main_after, main_on_run_loop};
use crate::window::layout::Document;
use crate::window::{TabId, WindowController};

/// How long the record waits after a change, so that a burst of changes writes it once.
const SETTLE: Duration = Duration::from_secs(1);

/// The longest quitting or closing a window waits for the record and the layouts to reach the disk.
const WRITE_LIMIT: Duration = Duration::from_secs(5);

/// The windows open now, in the order they opened.
const OPEN_WINDOWS: ide_core::settings::StateKey<Vec<WindowRecord>> =
    ide_core::settings::StateKey::new("openWindows", Vec::new);

/// What a window shows, as the next launch puts it back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowRecord {
    pub(crate) project: Option<PathBuf>,
    pub(crate) tabs: Vec<TabRecord>,
    /// The tab in front, as an index into `tabs`.
    pub(crate) selected: Option<usize>,
}

/// A tab that comes back at the next launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum TabRecord {
    Welcome,
    /// An editor: its file, if it has one, and the name of its copy against a crash, which exists while its text is
    /// unsaved.
    Editor {
        path: Option<PathBuf>,
        recovery: String,
    },
}

/// Where writing down the record stands.
#[derive(Default)]
pub(crate) struct Saving {
    write_pending: Cell<bool>,
    quitting: Cell<bool>,
    /// Writes of the record and the layouts queued on the disk lane that have not ended yet.
    writing: Arc<AtomicUsize>,
}

/// The app quits: what it wrote down last stays, as the terminals and agents that end now change their tabs.
pub(crate) fn quitting() {
    app().session().quitting.set(true);
}

/// Whether the app quits, after which nothing more is written down.
pub(crate) fn is_quitting() -> bool {
    app().session().quitting.get()
}

/// The windows or their tabs changed; the record follows a moment later.
pub(crate) fn changed() {
    let cx = app();
    if cx.session().quitting.get() || cx.session().write_pending.replace(true) {
        return;
    }
    main_after(SETTLE, || {
        let cx = app();
        cx.session().write_pending.set(false);
        remember(&cx);
    });
}

/// Writes down what the windows show now, and each folder's panels.
pub(crate) fn remember(cx: &crate::context::AppContext) {
    if cx.session().quitting.get() {
        return;
    }
    crate::window::save_layouts();
    let windows = crate::window::records();
    cx.session().write_soon("remembering the windows", move || {
        State::load_user().set(&OPEN_WINDOWS, &windows)
    });
}

impl Saving {
    /// Runs `write` on the disk lane after the writes queued before it, or here when the lane is full; a failure is
    /// reported as `what`.
    pub(crate) fn write_soon(&self, what: &'static str, write: impl FnOnce() -> std::io::Result<()> + Send + 'static) {
        let job = Arc::new(Mutex::new(Some(write)));
        let queued = job.clone();
        self.writing.fetch_add(1, Ordering::SeqCst);
        let counted = self.writing.clone();
        let sent = ide_core::lane::DISK.try_run(move || {
            if let Some(write) = queued.lock_ignoring_poison().take() {
                written(what, ide_core::lane::contained(write));
            }
            counted.fetch_sub(1, Ordering::SeqCst);
        });
        if sent.is_err() {
            self.writing.fetch_sub(1, Ordering::SeqCst);
            let write = job.lock_ignoring_poison().take();
            if let Some(write) = write {
                written(what, Some(write()));
            }
        }
    }

    /// Waits until the record and the layouts queued so far are on disk, as the app quits or a window closes.
    pub(crate) fn wait_for_writes(&self) {
        let until = Instant::now() + WRITE_LIMIT;
        while self.writing.load(Ordering::SeqCst) > 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn written(what: &str, result: Option<std::io::Result<()>>) {
    match result {
        Some(Ok(())) => {}
        Some(Err(error)) => ide_core::diag::report(what, error),
        None => ide_core::diag::report(what, "the write panicked"),
    }
}

/// Opens the windows of the last run with their tabs if the user wants them back, and in any case every document a
/// crash left unsaved, each window with a note of how many came back; how many windows opened.
pub(crate) fn restore(mtm: MainThreadMarker) -> usize {
    let recovery = Recovery::in_data_dir();
    set_aside_damaged(&recovery);
    let pending = recovery.pending().unwrap_or_else(|error| {
        crate::diag::report("reading the recovery copies", error);
        Vec::new()
    });
    let settings = Settings::load_user();
    let state = State::load_user();
    let mut records = state.get(&OPEN_WINDOWS);
    if records.is_empty() {
        records = state
            .get(&remembered::OPEN_PROJECTS)
            .into_iter()
            .map(|project| WindowRecord {
                project: Some(project),
                ..WindowRecord::default()
            })
            .collect();
    }
    let reopen = settings.get(&keys::REOPEN_PROJECTS);
    let windows = plan(records, &pending, reopen, Path::is_dir);
    let mut restore = Restore {
        reopen,
        recovery,
        copies: pending.into_iter().map(|p| (p.id.as_str().to_string(), p)).collect(),
        font_size: settings.get(&keys::FONT_SIZE),
        files: Vec::new(),
    };
    for window in &windows {
        restore.window(mtm, window);
    }
    if !restore.files.is_empty() {
        let files = restore.files;
        main_on_run_loop(move || crate::editor::check_recovered(&files));
    }
    windows.len()
}

fn set_aside_damaged(recovery: &Recovery) {
    let damaged = recovery.damaged().unwrap_or_else(|error| {
        crate::diag::report("looking for damaged recovery copies", error);
        Vec::new()
    });
    for id in damaged {
        crate::diag::report("a damaged recovery copy was set aside", id.as_str());
        if let Err(error) = recovery.set_aside(&id) {
            crate::diag::report("setting aside a recovery copy", error);
        }
    }
}

/// What a launch puts back while it opens the windows.
struct Restore {
    /// Whether the windows come back with their tabs, or only with the copies.
    reopen: bool,
    recovery: Recovery,
    /// The copies not yet in a tab, by name.
    copies: HashMap<String, Pending>,
    font_size: f64,
    /// The copies of documents with a file, whose files are looked at once the windows show.
    files: Vec<RecoveryId>,
}

impl Restore {
    /// Opens the window of `record`: the folder's tabs, an editor's copy in place of its file, then the
    /// record's other editors and copies, such as untitled ones, in tabs of their own.
    fn window(&mut self, mtm: MainThreadMarker, record: &WindowRecord) {
        let shown = record.project.as_deref().and_then(crate::window::for_project);
        let controller = shown.unwrap_or_else(|| WindowController::open_new(mtm, record.project.clone()));
        let layout = self.reopen && record.project.is_some();
        if record.tabs.is_empty() && !layout {
            return;
        }
        controller.restore_editor_tabs(|| self.tabs(&controller, record, layout));
    }

    fn tabs(&mut self, controller: &WindowController, record: &WindowRecord, layout: bool) {
        controller.clear_tabs();
        let mut restored = 0;
        if layout {
            controller
                .restore_layout_with(&mut |controller, document| self.document(controller, document, &mut restored));
        }
        let mut opened = Vec::with_capacity(record.tabs.len());
        for tab in &record.tabs {
            let id = match tab {
                TabRecord::Welcome => Some(controller.open_welcome()),
                TabRecord::Editor { path, recovery } => {
                    let open = path
                        .as_deref()
                        .and_then(|path| controller.tab_with_key(&path.to_string_lossy()));
                    match (open, self.copies.remove(recovery)) {
                        (Some(id), _) => Some(id),
                        (None, Some(copy)) if self.recovered(controller, &copy) => {
                            restored += 1;
                            controller.selected_tab()
                        }
                        _ => self.reopen(controller, path.as_deref()),
                    }
                }
            };
            opened.push(id);
        }
        if controller.tab_ids().is_empty() {
            controller.open_welcome();
        }
        if let Some(id) = record.selected.and_then(|index| opened.get(index).copied().flatten()) {
            controller.select_tab_by_id(id);
        }
        if restored > 0 {
            controller.show_notice(&restored_text(restored));
        }
    }

    /// Opens the tab of an editor of the folder's layout, from the copy a crash left of its text if there is one;
    /// an untitled document comes back only from its copy. Whether it opened.
    fn document(&mut self, controller: &WindowController, document: Document, restored: &mut usize) -> bool {
        let name = match document {
            Document::Untitled(name) => Some(name.to_string()),
            Document::File(path) => self
                .copies
                .iter()
                .find(|(_, copy)| copy.path.as_deref() == Some(path))
                .map(|(name, _)| name.clone()),
        };
        if let Some(copy) = name.and_then(|name| self.copies.remove(&name))
            && self.recovered(controller, &copy)
        {
            *restored += 1;
            return true;
        }
        match document {
            Document::File(path) => self.reopen(controller, Some(path)).is_some(),
            Document::Untitled(_) => false,
        }
    }

    /// Opens the document of `copy`; a copy that cannot be read is set aside and its file, if any, opens as it is.
    fn recovered(&mut self, controller: &WindowController, copy: &Pending) -> bool {
        match self.recovery.restore(&copy.id) {
            Ok(recovered) => {
                if recovered.pending.path.is_some() {
                    self.files.push(copy.id.clone());
                }
                crate::editor::open_recovered(controller, recovered, self.font_size);
                true
            }
            Err(error) => {
                crate::diag::report("reading a recovery copy", error);
                if let Err(error) = self.recovery.set_aside(&copy.id) {
                    crate::diag::report("setting aside a recovery copy", error);
                }
                false
            }
        }
    }

    fn reopen(&self, controller: &WindowController, path: Option<&Path>) -> Option<TabId> {
        controller.restore_editor(path?, self.font_size)
    }
}

fn restored_text(count: usize) -> String {
    if count == 1 {
        t("Ein Dokument wiederhergestellt").to_string()
    } else {
        tf("{} Dokumente wiederhergestellt", &[&count.to_string()])
    }
}

/// The windows to open at launch, from the windows of the last run and the copies a crash left: every recorded window
/// if `reopen`, else only the tabs that hold a copy. A copy no record names joins the window of its folder, or one of
/// its own. A window whose folder is gone keeps only its copies, and a window without a folder comes back only with
/// editor tabs.
fn plan(
    records: Vec<WindowRecord>,
    copies: &[Pending],
    reopen: bool,
    is_folder: impl Fn(&Path) -> bool,
) -> Vec<WindowRecord> {
    let copy_names: HashSet<&str> = copies.iter().map(|copy| copy.id.as_str()).collect();
    let mut named: HashSet<String> = HashSet::new();
    let mut windows = Vec::new();
    for mut record in records {
        let gone = record.project.as_deref().is_some_and(|project| !is_folder(project));
        retain_tabs(&mut record, |tab| match tab {
            TabRecord::Editor { recovery, .. } if copy_names.contains(recovery.as_str()) => {
                named.insert(recovery.clone())
            }
            _ => reopen && !gone,
        });
        if gone {
            record.project = None;
        }
        let editors = record.tabs.iter().any(|tab| matches!(tab, TabRecord::Editor { .. }));
        let copied = record
            .tabs
            .iter()
            .any(|tab| matches!(tab, TabRecord::Editor { recovery, .. } if copy_names.contains(recovery.as_str())));
        let keep = if reopen {
            record.project.is_some() || editors
        } else {
            copied
        };
        if keep {
            windows.push(record);
        }
    }
    for copy in copies.iter().filter(|copy| !named.contains(copy.id.as_str())) {
        let tab = TabRecord::Editor {
            path: copy.path.clone(),
            recovery: copy.id.as_str().to_string(),
        };
        let project = copy.project.clone().filter(|project| is_folder(project));
        match windows.iter_mut().find(|window| window.project == project) {
            Some(window) => window.tabs.push(tab),
            None => windows.push(WindowRecord {
                project,
                tabs: vec![tab],
                selected: None,
            }),
        }
    }
    windows
}

/// Keeps the tabs `keep` says yes to, in order, with the selection on the same tab or on none.
fn retain_tabs(record: &mut WindowRecord, mut keep: impl FnMut(&TabRecord) -> bool) {
    let selected = record.selected.take();
    let mut kept = Vec::with_capacity(record.tabs.len());
    for (index, tab) in std::mem::take(&mut record.tabs).into_iter().enumerate() {
        if keep(&tab) {
            if selected == Some(index) {
                record.selected = Some(kept.len());
            }
            kept.push(tab);
        }
    }
    record.tabs = kept;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(name: &str, path: Option<&str>, project: Option<&str>) -> Pending {
        Pending {
            id: RecoveryId::parse(name).unwrap(),
            path: path.map(PathBuf::from),
            title: None,
            project: project.map(PathBuf::from),
            saved: std::time::UNIX_EPOCH,
        }
    }

    fn editor(path: Option<&str>, recovery: &str) -> TabRecord {
        TabRecord::Editor {
            path: path.map(PathBuf::from),
            recovery: recovery.into(),
        }
    }

    fn window(project: Option<&str>, tabs: Vec<TabRecord>, selected: Option<usize>) -> WindowRecord {
        WindowRecord {
            project: project.map(PathBuf::from),
            tabs,
            selected,
        }
    }

    fn folders(path: &Path) -> bool {
        path.starts_with("/p")
    }

    #[test]
    fn the_last_run_comes_back_as_it_was_with_the_copies_in_their_tabs() {
        let records = vec![
            window(
                Some("/p/a"),
                vec![TabRecord::Welcome, editor(Some("/p/a/x.rs"), "a1"), editor(None, "b2")],
                Some(1),
            ),
            window(Some("/p/b"), vec![], None),
        ];
        let copies = [copy("b2", None, Some("/p/a"))];
        assert_eq!(plan(records.clone(), &copies, true, folders), records);
    }

    #[test]
    fn without_reopening_only_the_tabs_with_a_copy_come_back() {
        let records = vec![
            window(
                Some("/p/a"),
                vec![TabRecord::Welcome, editor(Some("/p/a/x.rs"), "a1"), editor(None, "b2")],
                Some(2),
            ),
            window(Some("/p/b"), vec![editor(Some("/p/b/y.rs"), "c3")], Some(0)),
        ];
        let copies = [copy("b2", None, Some("/p/a"))];
        assert_eq!(
            plan(records, &copies, false, folders),
            [window(Some("/p/a"), vec![editor(None, "b2")], Some(0))]
        );
    }

    #[test]
    fn a_copy_no_window_names_joins_its_folder_or_a_window_of_its_own() {
        let records = vec![window(Some("/p/a"), vec![TabRecord::Welcome], Some(0))];
        let copies = [
            copy("a1", Some("/p/a/x.rs"), Some("/p/a")),
            copy("b2", None, None),
            copy("c3", Some("/elsewhere/z.txt"), Some("/gone")),
        ];
        assert_eq!(
            plan(records, &copies, true, folders),
            [
                window(
                    Some("/p/a"),
                    vec![TabRecord::Welcome, editor(Some("/p/a/x.rs"), "a1")],
                    Some(0)
                ),
                window(
                    None,
                    vec![editor(None, "b2"), editor(Some("/elsewhere/z.txt"), "c3")],
                    None
                ),
            ]
        );
    }

    #[test]
    fn a_window_whose_folder_is_gone_keeps_only_its_copies_and_a_copy_comes_back_once() {
        let records = vec![
            window(
                Some("/gone"),
                vec![TabRecord::Welcome, editor(Some("/gone/x"), "a1")],
                Some(0),
            ),
            window(Some("/gone/too"), vec![editor(Some("/gone/too/y"), "b2")], Some(0)),
            window(Some("/p/a"), vec![editor(Some("/gone/x"), "a1")], Some(0)),
            window(None, vec![TabRecord::Welcome], Some(0)),
        ];
        let copies = [copy("a1", Some("/gone/x"), Some("/gone"))];
        assert_eq!(
            plan(records, &copies, true, folders),
            [
                window(None, vec![editor(Some("/gone/x"), "a1")], None),
                window(Some("/p/a"), vec![], None),
            ]
        );
    }

    #[test]
    fn a_record_reads_back_as_written() {
        let records = vec![window(
            Some("/p/a"),
            vec![TabRecord::Welcome, editor(Some("/p/a/x.rs"), "a1"), editor(None, "b2")],
            Some(2),
        )];
        let json = serde_json::to_value(&records).unwrap();
        assert_eq!(json[0]["tabs"][1]["kind"], "editor");
        assert_eq!(serde_json::from_value::<Vec<WindowRecord>>(json).unwrap(), records);
    }

    #[test]
    fn the_note_counts_what_came_back() {
        assert_eq!(restored_text(1), t("Ein Dokument wiederhergestellt"));
        assert_eq!(restored_text(3), tf("{} Dokumente wiederhergestellt", &["3"]));
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
