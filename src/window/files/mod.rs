//! The folder tree of the files panel, read one folder at a time as the user opens it: the model decides, the view
//! draws, the controller carries out (E98).

mod controller;
mod hover;
mod pane_choice;
use ide_project::files_panel as model;
mod view;

use std::path::Path;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, sel};
use objc2_app_kit::{NSMenu, NSOutlineView, NSScrollView, NSView};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::WindowController;
use crate::project::{Listing, Project};
use crate::util::set_action;
use view::{FileTreeSource, Pending};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// The tree view with what feeds it.
pub(crate) struct FileTree {
    scroll: Retained<NSScrollView>,
    view: Retained<NSOutlineView>,
    source: Retained<FileTreeSource>,
    _hover: Retained<hover::TreeHover>,
}

impl FileTree {
    pub(crate) fn new(mtm: MainThreadMarker, controller: &WindowController, project: Rc<Project>) -> FileTree {
        let source = FileTreeSource::new(mtm, controller, project);
        let view: Retained<NSOutlineView> = Retained::into_super(super::file_outline::FileOutline::new(
            mtm,
            ide_ui_common::panel::LIST_FRAME,
        ));
        let scroll = ide_ui_common::panel::source_list_outline(mtm, &view, "name", t("Dateien"));
        view.setUsesAutomaticRowHeights(false);
        view.setAllowsMultipleSelection(true);
        ide_ui_common::ui_size::follow_rows(&view, ide_ui_common::style::ROW_HEIGHT);
        // SAFETY: The outline view holds its data source and delegate weakly; `FileTree` keeps the source alive.
        unsafe {
            view.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            view.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        }
        set_action(&view, &source, sel!(ideRowClicked:));
        assert!(source.respondsToSelector(sel!(ideRowDoubleClicked:)));
        // SAFETY: The source answers the double action with a method that takes the sender, checked above.
        unsafe { view.setDoubleAction(Some(sel!(ideRowDoubleClicked:))) };
        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        // SAFETY: The view keeps the menu; its delegate, the source, lives as long as the view through `FileTree`.
        unsafe { view.setMenu(Some(&menu)) };
        // `Project::open` started reading the root.
        let root = source.ivars().project.root().to_path_buf();
        let waiting = Pending {
            spinner: None,
            deep: false,
        };
        source.ivars().pending.borrow_mut().insert(root.clone(), waiting);
        source.spin_later(root);
        let _hover = hover::TreeHover::new(mtm, &view);
        FileTree {
            scroll,
            view,
            source,
            _hover,
        }
    }

    /// The tree in its scroll view, the files page.
    pub(crate) fn page(&self) -> Retained<NSView> {
        Retained::into_super(self.scroll.clone())
    }

    /// The buttons for the files panel's title row.
    pub(crate) fn header_buttons(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        Retained::into_super(super::files_header::buttons(mtm, &self.source))
    }

    pub(crate) fn outline(&self) -> Retained<NSOutlineView> {
        self.view.clone()
    }

    /// Shows the entries the tree read of the folders in `listings`, those the view shows or waits for.
    pub(crate) fn listed(&self, listings: &[Listing]) {
        for (dir, entries) in listings {
            self.source.listed(&self.view, dir, entries.as_ref());
        }
        self.source.forget_rows();
    }

    /// Has the outline view read the folders that closed anew, then lets go of the rows it no longer holds.
    fn settle(&self) {
        self.source.settle(&self.view);
    }

    /// Shows a spinner in `dir` while its entries are still being read.
    fn spin(&self, dir: &Path) {
        self.source.spin(&self.view, dir);
    }
}
