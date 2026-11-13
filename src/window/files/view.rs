//! The view of the files panel: the outline view's data source and delegate, which draws the rows and passes clicks,
//! folds and menu commands on to the controller.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_project::Entry;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSColor, NSControlTextEditingDelegate, NSImageAlignment, NSImageView, NSLineBreakMode, NSMenu,
    NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSTableCellView, NSTableColumn,
    NSTextField, NSUserInterfaceLayoutDirection, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSString};

use super::model::{Command, MenuEntry, Model, Row};
use crate::project::Project;
use crate::util::{guarded, item, ns, separator, set_target};
use crate::window::WindowController;

pub struct TreeIvars {
    pub(super) project: Rc<Project>,
    pub(super) controller: Weak<WindowController>,
    pub(super) model: RefCell<Model>,
    /// The objects that stand for rows in the outline view, one per path for as long as the tree lives, since the
    /// view keeps no reference of its own.
    items: RefCell<HashMap<PathBuf, Retained<NSString>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `FileTreeSource` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFileTreeSource"]
    #[ivars = TreeIvars]
    pub struct FileTreeSource;

    unsafe impl NSObjectProtocol for FileTreeSource {}

    unsafe impl NSOutlineViewDataSource for FileTreeSource {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.children_of(item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let children = self.children_of(item);
                let entry = children.get(usize::try_from(index).ok()?)?;
                Some(self.item_for(entry))
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(None, || {
                let path = path_of(item)?;
                self.ivars().model.borrow().row(&path).map(|row| row.expandable)
            })
            .unwrap_or(false)
        }
    }

    unsafe impl NSControlTextEditingDelegate for FileTreeSource {}

    unsafe impl NSOutlineViewDelegate for FileTreeSource {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || {
                let path = path_of(item)?;
                let row = self.ivars().model.borrow().row(&path).unwrap_or_default();
                // SAFETY: The item came from this outline view, which knows it.
                let open = row.expandable.then(|| unsafe { view.isItemExpanded(Some(item)) });
                Some(row_view(self.mtm(), &path, row.ignored, open))
            })
        }

        #[unsafe(method(outlineViewItemWillExpand:))]
        fn will_expand(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(path) = expanded_path(notification) {
                    self.folder_opens(&path);
                }
            });
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &NSNotification) {
            guarded((), || turn_chevron(notification, true));
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(path) = expanded_path(notification) {
                    self.folder_closed(&path);
                }
                turn_chevron(notification, false);
            });
        }
    }

    unsafe impl NSMenuDelegate for FileTreeSource {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    impl FileTreeSource {
        #[unsafe(method(ideRowClicked:))]
        fn row_clicked_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(view) = sender.and_then(|s| s.downcast_ref::<NSOutlineView>()) else { return };
                if !extends_selection(self.mtm()) {
                    self.row_clicked(view, view.clickedRow());
                }
            });
        }

        #[unsafe(method(ideRowDoubleClicked:))]
        fn row_double_clicked_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(view) = sender.and_then(|s| s.downcast_ref::<NSOutlineView>()) else { return };
                self.row_double_clicked(view, view.clickedRow());
            });
        }

        #[unsafe(method(ideOpenBesideClicked:))]
        fn open_beside(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::OpenBeside));
        }

        #[unsafe(method(ideOpenAsPreview:))]
        fn open_as_preview(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::OpenAsPreview));
        }

        #[unsafe(method(ideOpenInBrowserTab:))]
        fn open_in_browser_tab(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::OpenInBrowserTab));
        }

        #[unsafe(method(ideRevealInFinder:))]
        fn reveal(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::Reveal));
        }

        #[unsafe(method(ideCopyPath:))]
        fn copy_path(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::CopyPath));
        }

        #[unsafe(method(ideCopyRelativePath:))]
        fn copy_relative_path(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::CopyRelativePath));
        }

        #[unsafe(method(ideMoveToTrash:))]
        fn move_to_trash(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::Trash));
        }

        #[unsafe(method(ideNewFile:))]
        fn new_file(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::NewFile));
        }

        #[unsafe(method(ideNewFolder:))]
        fn new_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::NewFolder));
        }

        #[unsafe(method(ideRename:))]
        fn rename(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::Rename));
        }

        #[unsafe(method(ideTreeNewFile:))]
        fn header_new_file(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create_from_header(false));
        }

        #[unsafe(method(ideTreeNewFolder:))]
        fn header_new_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create_from_header(true));
        }

        #[unsafe(method(ideTreeCollapseAll:))]
        fn collapse_all(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(view) = self.ivars().controller.load().and_then(|c| c.file_tree_view()) {
                    // SAFETY: Nil stands for the root, whose items and their children all fold.
                    unsafe { view.collapseItem_collapseChildren(None, true) };
                }
            });
        }
    }
);

impl FileTreeSource {
    pub(super) fn new(mtm: MainThreadMarker, controller: &WindowController, project: Rc<Project>) -> Retained<Self> {
        let ivars = TreeIvars {
            model: RefCell::new(Model::new(project.root().to_path_buf())),
            project,
            controller: Weak::from(controller),
            items: RefCell::new(HashMap::new()),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(FileTreeSource::alloc(mtm).set_ivars(ivars)), init] }
    }

    /// The object that stands for `path` in the outline view, if the view was handed one.
    pub(super) fn item_of(&self, path: &Path) -> Option<Retained<NSString>> {
        self.ivars().items.borrow().get(path).cloned()
    }

    /// The row of `path` in `view`, -1 if it shows none.
    #[cfg(feature = "selftest")]
    pub(super) fn row_of(&self, view: &NSOutlineView, path: &Path) -> isize {
        let Some(item) = self.item_of(path) else { return -1 };
        // SAFETY: The item is one the data source handed to this view, kept alive in `items`.
        unsafe { view.rowForItem(Some(&item)) }
    }

    fn children_of(&self, item: Option<&AnyObject>) -> Vec<Entry> {
        let project = &self.ivars().project;
        let dir = match item {
            Some(item) => match path_of(item) {
                Some(path) => path,
                None => return vec![],
            },
            None => project.root().to_path_buf(),
        };
        project.tree().children(&dir).map(<[Entry]>::to_vec).unwrap_or_default()
    }

    fn item_for(&self, entry: &Entry) -> Retained<NSString> {
        self.ivars().model.borrow_mut().note(
            &entry.path,
            Row {
                expandable: entry.can_expand(),
                ignored: entry.is_ignored || entry.is_hidden,
            },
        );
        self.ivars()
            .items
            .borrow_mut()
            .entry(entry.path.clone())
            .or_insert_with(|| ns(&entry.path.to_string_lossy()))
            .clone()
    }

    fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let mtm = self.mtm();
        let Some(view) = self.ivars().controller.load().and_then(|c| c.file_tree_view()) else {
            return;
        };
        let path = view.itemAtRow(view.clickedRow()).and_then(|item| path_of(&item));
        let entries = self.menu_opens(path.as_deref());
        for entry in entries {
            let MenuEntry::Item {
                title,
                command,
                enabled,
            } = entry
            else {
                menu.addItem(&separator(mtm));
                continue;
            };
            let entry = item(mtm, title, Some(action(command)), "");
            set_target(&entry, self);
            entry.setEnabled(enabled);
            menu.addItem(&entry);
        }
        if let Some(entry) = path
            .as_deref()
            .and_then(|path| crate::markdown::open_in_splat_item(mtm, path))
        {
            menu.addItem(&separator(mtm));
            menu.addItem(&entry);
        }
        menu.setAutoenablesItems(false);
    }
}

/// The action of the menu item for `command`.
fn action(command: Command) -> Sel {
    match command {
        Command::OpenBeside => sel!(ideOpenBesideClicked:),
        Command::OpenAsPreview => sel!(ideOpenAsPreview:),
        Command::OpenInBrowserTab => sel!(ideOpenInBrowserTab:),
        Command::NewFile => sel!(ideNewFile:),
        Command::NewFolder => sel!(ideNewFolder:),
        Command::Reveal => sel!(ideRevealInFinder:),
        Command::CopyPath => sel!(ideCopyPath:),
        Command::CopyRelativePath => sel!(ideCopyRelativePath:),
        Command::Rename => sel!(ideRename:),
        Command::Trash => sel!(ideMoveToTrash:),
    }
}

/// Whether the click that is being handled adds to the selection, with Shift or Command, rather than opening a file.
fn extends_selection(mtm: MainThreadMarker) -> bool {
    let event = NSApplication::sharedApplication(mtm).currentEvent();
    event.is_some_and(|event| {
        event
            .modifierFlags()
            .intersects(objc2_app_kit::NSEventModifierFlags::Shift | objc2_app_kit::NSEventModifierFlags::Command)
    })
}

pub(super) fn path_of(item: &AnyObject) -> Option<PathBuf> {
    item.downcast_ref::<NSString>().map(|s| PathBuf::from(s.to_string()))
}

/// Points the chevron of the folder that opened or closed down or right.
fn turn_chevron(notification: &NSNotification, open: bool) {
    let Some(view) = notification
        .object()
        .and_then(|object| object.downcast::<NSOutlineView>().ok())
    else {
        return;
    };
    let Some(item) = notification
        .userInfo()
        .and_then(|info| info.objectForKey(&ns("NSObject")))
    else {
        return;
    };
    // SAFETY: The item came from this outline view's notification, so the view knows it.
    let row = unsafe { view.rowForItem(Some(&item)) };
    let cell = (row >= 0)
        .then(|| view.viewAtColumn_row_makeIfNecessary(0, row, false))
        .flatten()
        .and_then(|cell| cell.downcast::<NSTableCellView>().ok());
    // SAFETY: The cell is one this tree made, whose image view is the icon it set.
    if let Some(image) = cell.and_then(|cell| unsafe { cell.imageView() }) {
        image.setImage(crate::window::file_outline::chevron(open).as_deref());
    }
}

fn expanded_path(notification: &NSNotification) -> Option<PathBuf> {
    let info = notification.userInfo()?;
    let item = info.objectForKey(&ns("NSObject"))?;
    path_of(&item)
}

/// The row of `path`: its icon, or the chevron of a folder that is `open` or not, and its name.
fn row_view(mtm: MainThreadMarker, path: &Path, ignored: bool, open: Option<bool>) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let image: Retained<NSImageView> = crate::file_icon::image_view(mtm, path, open.is_some());
    if let Some(open) = open {
        image.setImage(crate::window::file_outline::chevron(open).as_deref());
        // At the start of the icon's place, where the search panel's chevrons sit under the panel's title.
        let rtl = NSApplication::sharedApplication(mtm).userInterfaceLayoutDirection()
            == NSUserInterfaceLayoutDirection::RightToLeft;
        image.setImageAlignment(if rtl {
            NSImageAlignment::AlignRight
        } else {
            NSImageAlignment::AlignLeft
        });
    }
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let label = NSTextField::labelWithString(&ns(&name), mtm);
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    if ignored {
        label.setTextColor(Some(&NSColor::tertiaryLabelColor()));
    }
    cell.addSubview(&image);
    cell.addSubview(&label);
    // SAFETY: Both views are subviews of the cell, which keeps them.
    unsafe {
        cell.setImageView(Some(&image));
        cell.setTextField(Some(&label));
    }
    image.setTranslatesAutoresizingMaskIntoConstraints(false);
    label.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        image.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        image.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        image.widthAnchor().constraintEqualToConstant(16.0),
        image.heightAnchor().constraintEqualToConstant(16.0),
        label
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&image.trailingAnchor(), 5.0),
        label
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor(&cell.trailingAnchor()),
        label.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    cell.setToolTip(Some(&ns(&path.to_string_lossy())));
    cell.into_super()
}
