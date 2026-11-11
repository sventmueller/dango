//! A window's folder: its tree, kept current by the file system watcher, quick open and whether the user trusts it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ide_core::settings::{Settings, keys};
use ide_project::{QuickOpen, Tree, TreeOptions, TrustStore, WatchEvent, Watcher};

use crate::util::main_soon;

pub struct Project {
    root: PathBuf,
    tree: Arc<Mutex<Tree>>,
    quick_open: Option<Arc<QuickOpen>>,
    _watcher: Option<Watcher>,
}

impl Project {
    /// Reads the top of `root` and starts watching it; changed folders reach `crate::window::tree_changed` and the
    /// project search, changed files `crate::lsp::files_changed`, every change `crate::git_panel::changed` and
    /// `crate::ci_panel::changed`, and a change in `.git` the editors' change marks, all on the main thread.
    pub fn open(root: &Path) -> std::io::Result<Project> {
        let options = TreeOptions {
            show_ignored: Settings::load_user().get(&keys::SHOW_IGNORED_FILES),
        };
        let mut tree = Tree::new(root, options)?;
        let root = tree.root().to_path_buf();
        tree.expand(&root)?;
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
                let changed = tree.lock().unwrap_or_else(|e| e.into_inner()).apply(&event);
                if !changed.is_empty() {
                    let root = window_root.clone();
                    main_soon(move || crate::window::tree_changed(&root, &changed));
                }
                if let WatchEvent::Changed(changes) = &event
                    && !changes.files.is_empty()
                {
                    let (root, files) = (window_root.clone(), changes.files.clone());
                    main_soon(move || crate::lsp::files_changed(&root, &files));
                }
                let root = window_root.clone();
                main_soon(move || {
                    crate::git_panel::changed(&root);
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
        Ok(Project {
            root,
            tree,
            quick_open,
            _watcher: watcher,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tree(&self) -> MutexGuard<'_, Tree> {
        self.tree.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn quick_open(&self) -> Option<&QuickOpen> {
        self.quick_open.as_deref()
    }
}

/// Whether the user trusts `folder` or a folder above it.
pub fn is_trusted(folder: &Path) -> bool {
    TrustStore::open().is_trusted(folder)
}

/// Takes the trust in `folder` back, so a self-test can open an AI tab without its agent starting.
#[cfg(feature = "selftest")]
pub fn distrust(folder: &Path) {
    if let Err(error) = TrustStore::open().revoke(folder) {
        crate::diag::report("taking back the trust in a folder", error);
    }
}

pub fn trust(folder: &Path) {
    if let Err(error) = TrustStore::open().trust(folder) {
        crate::diag::report("trusting a folder", error);
    }
}
