//! A window's folder: its tree, kept current by the file system watcher, quick open and whether the user trusts it.

use ide_core::LockIgnoringPoison;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ide_core::settings::{Settings, keys};
use ide_project::{Entry, QuickOpen, Tree, TreeOptions, TrustStore, WatchEvent, Watcher};

use crate::util::main_soon;

/// A folder of the tree with its entries as last read, `None` once the tree no longer follows it.
pub(crate) type Listing = (PathBuf, Option<Arc<[Entry]>>);

pub(crate) struct Project {
    root: PathBuf,
    /// Locked only off the main thread, by the watcher and the disk lane, which read the disk while they hold it.
    tree: Arc<Mutex<Tree>>,
    quick_open: Option<Arc<QuickOpen>>,
    _watcher: Option<Watcher>,
}

impl Project {
    /// Starts reading the top of `root` and watching it; folders read reach `crate::window::tree_listed`, changed
    /// folders `crate::window::tree_changed` and the project search, changed files `ide_ui_lsp::files_changed`, every
    /// change `ide_ui_git::changed` and `crate::ci_panel::changed`, and a change in `.git` the editors' change marks,
    /// all on the main thread.
    pub(crate) fn open(root: &Path) -> std::io::Result<Project> {
        let options = TreeOptions {
            show_ignored: Settings::load_user().get(&keys::SHOW_IGNORED_FILES),
        };
        let tree = Tree::new(root, options)?;
        let root = tree.root().to_path_buf();
        let tree = Arc::new(Mutex::new(tree));
        let quick_open = match QuickOpen::start(&root, |_| ()) {
            Ok(quick_open) => Some(Arc::new(quick_open)),
            Err(error) => {
                crate::diag::report("quick open index", error);
                None
            }
        };
        let watcher = {
            let tree = tree.clone();
            let quick_open = quick_open.clone();
            let window_root = root.clone();
            Watcher::start(&root, move |event| {
                if let Some(quick_open) = &quick_open {
                    quick_open.changed(&event);
                }
                {
                    let mut tree = tree.lock_ignoring_poison();
                    let changed = tree.apply(&event);
                    if !changed.is_empty() {
                        let (root, listings) = (window_root.clone(), listings(&tree, changed));
                        // Queued while the lock is held, so the main thread gets the readings in the order they
                        // were made.
                        main_soon(move || crate::window::tree_changed(&root, &listings));
                    }
                }
                if let WatchEvent::Changed(changes) = &event
                    && !changes.files.is_empty()
                {
                    let (root, files) = (window_root.clone(), Arc::new(changes.files.clone()));
                    main_soon(move || {
                        ide_ui_lsp::files_changed(&root, &files);
                        if let Some(mtm) = objc2::MainThreadMarker::new() {
                            let paths: Vec<std::path::PathBuf> = files.keys().cloned().collect();
                            crate::site_preview::files_changed(mtm, &root, &paths);
                        }
                    });
                }
                let root = window_root.clone();
                main_soon(move || {
                    ide_ui_git::changed(&root);
                    crate::ci_panel::changed(&root);
                });
                let (git, files) = match &event {
                    WatchEvent::Changed(changes) => (
                        changes.git_changed,
                        !changes.dirs.is_empty() || !changes.subtrees.is_empty(),
                    ),
                    WatchEvent::Rescan(_) => (true, true),
                };
                if git {
                    let root = window_root.clone();
                    main_soon(move || crate::editor::git_changed(&root));
                }
                if files {
                    let root = window_root.clone();
                    main_soon(move || crate::window::files_changed_on_disk(&root));
                }
            })
        };
        let watcher = watcher
            .inspect_err(|error| crate::diag::report("file watcher", error))
            .ok();
        let project = Project {
            root,
            tree,
            quick_open,
            _watcher: watcher,
        };
        project.expand(&project.root);
        Ok(project)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn quick_open(&self) -> Option<&QuickOpen> {
        self.quick_open.as_deref()
    }

    /// Reads the folder `dir` unless the tree holds it, and hands its entries to `crate::window::tree_listed`.
    pub(crate) fn expand(&self, dir: &Path) {
        let dir = dir.to_path_buf();
        self.on_disk(move |tree| {
            // A folder that cannot be read lists as none.
            let _ = tree.expand(&dir);
            vec![dir]
        });
    }

    /// Forgets `dir` and every folder below it, after what was asked before.
    pub(crate) fn collapse(&self, dir: &Path) {
        let dir = dir.to_path_buf();
        self.on_disk(move |tree| {
            tree.collapse(&dir);
            Vec::new()
        });
    }

    /// Lists with `options` from now on, and hands the folders that changed to `crate::window::tree_listed`.
    pub(crate) fn set_options(&self, options: TreeOptions) {
        self.on_disk(move |tree| tree.set_options(options));
    }

    /// Runs `work` with the tree on the disk lane and hands the folders it names, as they are then, to the main thread.
    fn on_disk(&self, work: impl FnOnce(&mut Tree) -> Vec<PathBuf> + Send + 'static) {
        let (tree, root) = (self.tree.clone(), self.root.clone());
        let queued = ide_core::lane::DISK.try_run(move || {
            let mut tree = tree.lock_ignoring_poison();
            let dirs = work(&mut tree);
            if !dirs.is_empty() {
                let listings = listings(&tree, dirs);
                main_soon(move || crate::window::tree_listed(&root, &listings));
            }
        });
        if queued.is_err() {
            crate::diag::report("reading a folder", "the disk lane is full");
        }
    }
}

fn listings(tree: &Tree, dirs: Vec<PathBuf>) -> Vec<Listing> {
    dirs.into_iter()
        .map(|dir| {
            let entries = tree.listed(&dir);
            (dir, entries)
        })
        .collect()
}

/// Whether the user trusts `folder` or a folder above it.
pub(crate) fn is_trusted(folder: &Path) -> bool {
    TrustStore::open().is_trusted(folder)
}

pub(crate) fn trust(folder: &Path) {
    if let Err(error) = TrustStore::open().trust(folder) {
        crate::diag::report("trusting a folder", error);
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
