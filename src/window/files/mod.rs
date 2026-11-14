//! The folder tree of the files panel, read one folder at a time as the user opens it: the model decides, the view
//! draws, the controller carries out (E98).

mod controller;
use ide_project::files_panel as model;
mod view;

use std::path::PathBuf;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, sel};
use objc2_app_kit::{NSMenu, NSOutlineView, NSScrollView, NSView};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::WindowController;
use crate::project::Project;
use crate::util::set_action;
use view::FileTreeSource;
#[cfg(feature = "selftest")]
mod selftest_probe;

/// The tree view with what feeds it.
pub struct FileTree {
    scroll: Retained<NSScrollView>,
    view: Retained<NSOutlineView>,
    source: Retained<FileTreeSource>,
}

impl FileTree {
    pub fn new(mtm: MainThreadMarker, controller: &WindowController, project: Rc<Project>) -> FileTree {
        let source = FileTreeSource::new(mtm, controller, project);
        let view: Retained<NSOutlineView> = Retained::into_super(super::file_outline::FileOutline::new(
            mtm,
            ide_ui_common::panel::LIST_FRAME,
        ));
        let scroll = ide_ui_common::panel::source_list_outline(mtm, &view, "name", t("Dateien"));
        view.setUsesAutomaticRowHeights(false);
        view.setRowHeight(ide_ui_common::style::ROW_HEIGHT);
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
        FileTree { scroll, view, source }
    }

    /// The tree in its scroll view, the files page.
    pub fn page(&self) -> Retained<NSView> {
        Retained::into_super(self.scroll.clone())
    }

    /// The buttons for the files panel's title row.
    pub fn header_buttons(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        Retained::into_super(super::files_header::buttons(mtm, &self.source))
    }

    pub fn outline(&self) -> Retained<NSOutlineView> {
        self.view.clone()
    }

    /// Shows the new entries of `dirs`.
    pub fn reload(&self, dirs: &[PathBuf]) {
        let root = self.source.ivars().project.root().to_path_buf();
        for dir in dirs {
            if *dir == root {
                // SAFETY: Nil stands for the root, whose children the data source answers for.
                unsafe { self.view.reloadItem_reloadChildren(None, true) };
                continue;
            }
            if let Some(item) = self.source.item_of(dir) {
                // SAFETY: The item is one the data source handed to this view, kept alive by the source.
                unsafe { self.view.reloadItem_reloadChildren(Some(&item), true) };
            }
        }
    }
}
