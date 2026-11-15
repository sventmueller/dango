//! The folder a window shows (E22, E56): reading it, asking once whether the user trusts it, naming it in the title
//! bar, and what reads it while the window shows it, its tree, the project search and Open Quickly. The parts that
//! live as long as the folder are in `lifecycle`, finding in it in `finding`, putting another folder in its place in
//! `replace`.

mod finding;
pub(super) mod lifecycle;
mod replace;
#[cfg(feature = "selftest")]
mod selftest_probe;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_ui_common::alert::{Alert, Role, first};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::NSWindow;
use splat_i18n::t;

use super::files::FileTree;
use super::quick_open::QuickOpenPanel;
use super::{Panel, SearchPanel, WindowController};
use crate::project::{Listing, Project};
use crate::util::ns;

/// The window's folder and what reads it.
pub(super) struct State {
    /// The folder the window shows, as it was asked to.
    project: RefCell<Option<PathBuf>>,
    /// The folder once it is read.
    workspace: RefCell<Option<Rc<Project>>>,
    files: RefCell<Option<FileTree>>,
    search: RefCell<Option<Retained<SearchPanel>>>,
    quick_open: RefCell<Option<Retained<QuickOpenPanel>>>,
}

impl State {
    pub(super) fn new(project: Option<PathBuf>) -> State {
        State {
            project: RefCell::new(project),
            workspace: RefCell::default(),
            files: RefCell::default(),
            search: RefCell::default(),
            quick_open: RefCell::default(),
        }
    }
}

impl WindowController {
    /// Reads the folder, shows its tree and asks once whether the user trusts it.
    pub(super) fn load_project(&self) {
        let state = &self.ivars().folder;
        if let Some(search) = state.search.borrow_mut().take() {
            search.stop();
        }
        if let Some(quick_open) = state.quick_open.take() {
            quick_open.close();
        }
        *state.files.borrow_mut() = None;
        *state.workspace.borrow_mut() = None;
        let Some(root) = self.project() else { return };
        let project = match Project::open(&root) {
            Ok(project) => Rc::new(project),
            Err(error) => {
                crate::diag::report("opening a folder", error);
                return;
            }
        };
        let tree = FileTree::new(self.mtm(), self, project.clone());
        let search = SearchPanel::new(self.mtm(), self, project.root());
        self.set_files_page(&tree.page(), &tree.header_buttons(self.mtm()));
        if let Some(page) = search.page() {
            self.set_panel_page(Panel::Search, page);
        }
        if let Some(header) = search.header() {
            self.set_panel_accessory(Panel::Search, header);
        }
        *state.files.borrow_mut() = Some(tree);
        *state.search.borrow_mut() = Some(search);
        *state.workspace.borrow_mut() = Some(project);
        self.folder_attached();
        if !crate::project::is_trusted(&root) {
            self.ask_trust(root);
        }
    }

    /// The sheet that asks whether the folder may run what it configures itself (E22).
    fn ask_trust(&self, root: PathBuf) {
        let Some(window) = self.window() else { return };
        let message = splat_i18n::tf("Vertraust du den Dateien in „{}“?", &[&folder_name(&root)]);
        Alert::new(self.mtm(), &message)
            .informative(t(
                "In einem vertrauten Ordner dürfen Git-Hooks, Sprachserver aus dem Projekt und Agenten laufen. \
                 Eingeschränkt kannst du lesen und bearbeiten, aber nichts davon startet.",
            ))
            .button(t("Vertrauen"), Role::Plain)
            .button(t("Eingeschränkt öffnen"), Role::Plain)
            .sheet(&window, move |response| {
                if first(response) {
                    crate::project::trust(&root);
                    lifecycle::trust_changed(&root);
                }
            });
    }

    /// Asks again whether the user trusts the folder, for a panel that waits for it.
    pub(crate) fn ask_trust_again(&self) {
        if let Some(root) = self.project_root()
            && self.window().is_some_and(|w| w.attachedSheet().is_none())
        {
            self.ask_trust(root);
        }
    }

    /// The folder's root, if the window has one open.
    pub(crate) fn project_root(&self) -> Option<PathBuf> {
        self.ivars()
            .folder
            .workspace
            .borrow()
            .as_ref()
            .map(|p| p.root().to_path_buf())
    }

    /// Whether the window has its folder open and read.
    pub(super) fn has_workspace(&self) -> bool {
        self.ivars().folder.workspace.borrow().is_some()
    }

    /// Whether the user trusts this window's folder (E22).
    pub(crate) fn is_trusted(&self) -> bool {
        self.project_root()
            .is_some_and(|root| crate::project::is_trusted(&root))
    }

    /// The folder this window shows.
    pub(crate) fn project(&self) -> Option<PathBuf> {
        self.ivars().folder.project.borrow().clone()
    }

    pub(super) fn set_project(&self, project: Option<PathBuf>) {
        *self.ivars().folder.project.borrow_mut() = project;
        self.update_title();
        self.panels_follow_project();
        self.load_project();
    }

    fn update_title(&self) {
        for tab in self.open_tabs() {
            self.update_title_of(&tab.window);
        }
    }

    /// Names the folder in the title bar of `window`; the tabs name their documents.
    pub(super) fn update_title_of(&self, window: &NSWindow) {
        match self.project() {
            Some(path) => {
                window.setTitle(&ns(&folder_name(&path)));
                window.setSubtitle(&ns(&crate::app::display_path(&path)));
                window.setRepresentedURL(Some(&objc2_foundation::NSURL::fileURLWithPath(&ns(
                    &path.to_string_lossy()
                ))));
            }
            None => {
                window.setTitle(&ns(ide_product::NAME));
                window.setSubtitle(&ns(""));
            }
        }
    }

    /// Shows the folders the tree of `root` read, if the window has `root` open.
    pub(super) fn tree_listed(&self, root: &Path, listings: &[Listing]) {
        let state = &self.ivars().folder;
        let same = state.workspace.borrow().as_ref().is_some_and(|p| p.root() == root);
        if same && let Some(files) = state.files.borrow().as_ref() {
            files.listed(listings);
        }
    }

    /// Lets the tree list what Git ignores and the system hides, or leave it out again.
    pub(super) fn show_ignored_files(&self, on: bool) {
        if let Some(project) = self.ivars().folder.workspace.borrow().as_ref() {
            project.set_options(ide_project::TreeOptions { show_ignored: on });
        }
    }

    /// Open Quickly of the window, once it opened.
    pub(super) fn quick_open_panel(&self) -> Option<Retained<QuickOpenPanel>> {
        self.ivars().folder.quick_open.borrow().clone()
    }

    /// Runs `f` with the folder's tree, if the window shows a folder.
    pub(super) fn with_file_tree<T>(&self, f: impl FnOnce(&FileTree) -> T) -> Option<T> {
        self.ivars().folder.files.borrow().as_ref().map(f)
    }
}

/// The name of the folder at `path`, which its window bears.
pub(super) fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::folder_name;

    #[test]
    fn a_window_is_named_after_its_folder_and_the_root_after_itself() {
        assert_eq!(folder_name(Path::new("/Users/a/dango")), "dango");
        assert_eq!(folder_name(Path::new("/")), "/");
    }
}
