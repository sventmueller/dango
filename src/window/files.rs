//! The folder tree of the files panel, read one folder at a time as the user opens it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_project::Entry;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSColor, NSControlTextEditingDelegate, NSImageView, NSLineBreakMode, NSMenu,
    NSMenuDelegate, NSModalResponse, NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSPasteboard,
    NSPasteboardTypeString, NSScrollView, NSTableCellView, NSTableColumn, NSTableViewStyle, NSTextField, NSView,
    NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSFileManager, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};
use splat_i18n::{t, tf};

use super::WindowController;
use crate::project::Project;
use crate::util::{constant, guarded, item, ns, separator, set_action, set_target};

/// What the tree knows of a row besides its path.
struct RowInfo {
    item: Retained<NSString>,
    expandable: bool,
    ignored: bool,
}

pub struct TreeIvars {
    project: Rc<Project>,
    controller: Weak<WindowController>,
    rows: RefCell<HashMap<PathBuf, RowInfo>>,
    /// The path the context menu was opened on.
    clicked: RefCell<Option<PathBuf>>,
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
                self.ivars().rows.borrow().get(&path).map(|row| row.expandable)
            })
            .unwrap_or(false)
        }
    }

    unsafe impl NSControlTextEditingDelegate for FileTreeSource {}

    unsafe impl NSOutlineViewDelegate for FileTreeSource {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || {
                let path = path_of(item)?;
                let ignored = self.ivars().rows.borrow().get(&path).is_some_and(|row| row.ignored);
                Some(row_view(self.mtm(), &path, ignored))
            })
        }

        #[unsafe(method(outlineViewItemWillExpand:))]
        fn will_expand(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(path) = expanded_path(notification) {
                    let _ = self.ivars().project.tree().expand(&path);
                }
            });
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(path) = expanded_path(notification) {
                    self.ivars().project.tree().collapse(&path);
                }
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

        #[unsafe(method(ideRevealInFinder:))]
        fn reveal(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(path) = self.clicked() {
                    let url = file_url(&path);
                    NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
                }
            });
        }

        #[unsafe(method(ideCopyPath:))]
        fn copy_path(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(path) = self.clicked() {
                    copy_text(&path.to_string_lossy());
                }
            });
        }

        #[unsafe(method(ideCopyRelativePath:))]
        fn copy_relative_path(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(path) = self.clicked() {
                    let root = self.ivars().project.root();
                    copy_text(&path.strip_prefix(root).unwrap_or(&path).to_string_lossy());
                }
            });
        }

        #[unsafe(method(ideMoveToTrash:))]
        fn move_to_trash(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(path) = self.clicked() {
                    let moved =
                        NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&file_url(&path), None);
                    if let Err(error) = moved {
                        crate::diag::report("moving to the Trash", error.localizedDescription());
                    }
                }
            });
        }

        #[unsafe(method(ideNewFile:))]
        fn new_file(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create(false));
        }

        #[unsafe(method(ideNewFolder:))]
        fn new_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create(true));
        }

        #[unsafe(method(ideRename:))]
        fn rename(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.rename_clicked());
        }

        #[unsafe(method(ideTreeNewFile:))]
        fn header_new_file(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create_in(self.selected_folder(), false));
        }

        #[unsafe(method(ideTreeNewFolder:))]
        fn header_new_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create_in(self.selected_folder(), true));
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

/// The tree view with what feeds it.
pub struct FileTree {
    scroll: Retained<NSScrollView>,
    view: Retained<NSOutlineView>,
    source: Retained<FileTreeSource>,
}

impl FileTree {
    pub fn new(mtm: MainThreadMarker, controller: &WindowController, project: Rc<Project>) -> FileTree {
        let ivars = TreeIvars {
            project,
            controller: Weak::from(controller),
            rows: RefCell::new(HashMap::new()),
            clicked: RefCell::new(None),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let source: Retained<FileTreeSource> =
            unsafe { msg_send![super(FileTreeSource::alloc(mtm).set_ivars(ivars)), init] };
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 600.0));
        let view = NSOutlineView::initWithFrame(NSOutlineView::alloc(mtm), frame);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("name"));
        column.setResizingMask(objc2_app_kit::NSTableColumnResizingOptions::AutoresizingMask);
        column.setWidth(frame.size.width);
        view.addTableColumn(&column);
        // SAFETY: The column was just added to this outline view, which keeps it.
        unsafe { view.setOutlineTableColumn(Some(&column)) };
        view.setHeaderView(None);
        view.setStyle(NSTableViewStyle::SourceList);
        view.setIndentationPerLevel(12.0);
        view.setAutoresizesOutlineColumn(false);
        view.setColumnAutoresizingStyle(
            objc2_app_kit::NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle,
        );
        view.setAccessibilityLabel(Some(&ns(t("Dateien"))));
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
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&view));
        FileTree { scroll, view, source }
    }

    /// The files page: the row with the folder's name and its buttons above the tree.
    pub fn page(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let name = self
            .source
            .ivars()
            .project
            .root()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        super::files_header::page(mtm, &name, &self.source, &self.scroll)
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
            let item = self.source.ivars().rows.borrow().get(dir).map(|row| row.item.clone());
            if let Some(item) = item {
                // SAFETY: The item is one the data source handed to this view, kept alive in `rows`.
                unsafe { self.view.reloadItem_reloadChildren(Some(&item), true) };
            }
        }
    }

    #[cfg(feature = "selftest")]
    pub fn row_count(&self) -> isize {
        self.view.numberOfRows()
    }

    /// Selects the row of `path`, as a click on it does.
    #[cfg(feature = "selftest")]
    pub fn select(&self, path: &Path) {
        let item = self.source.ivars().rows.borrow().get(path).map(|row| row.item.clone());
        // SAFETY: The item is one the data source handed to this view, kept alive in `rows`.
        let row = item.map_or(-1, |item| unsafe { self.view.rowForItem(Some(&item)) });
        if let Ok(row) = usize::try_from(row) {
            self.view
                .selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
    }

    /// Clicks the row of `path`, twice for a double click, as the mouse does.
    #[cfg(feature = "selftest")]
    pub fn click(&self, path: &Path, double: bool) {
        let item = self.source.ivars().rows.borrow().get(path).map(|row| row.item.clone());
        // SAFETY: The item is one the data source handed to this view, kept alive in `rows`.
        let row = item.map_or(-1, |item| unsafe { self.view.rowForItem(Some(&item)) });
        self.source.row_clicked(&self.view, row);
        if double {
            self.source.row_double_clicked(&self.view, row);
        }
    }
}

/// Whether the click that is being handled adds to the selection, with Shift or Command, rather than opening a file.
fn extends_selection(mtm: MainThreadMarker) -> bool {
    let event = objc2_app_kit::NSApplication::sharedApplication(mtm).currentEvent();
    event.is_some_and(|event| {
        event
            .modifierFlags()
            .intersects(objc2_app_kit::NSEventModifierFlags::Shift | objc2_app_kit::NSEventModifierFlags::Command)
    })
}

impl FileTreeSource {
    /// A single click on a file shows it in the preview tab.
    fn row_clicked(&self, view: &NSOutlineView, row: isize) {
        let Some(path) = (row >= 0)
            .then(|| view.itemAtRow(row))
            .flatten()
            .and_then(|item| path_of(&item))
        else {
            return;
        };
        if !path.is_dir()
            && let Some(controller) = self.ivars().controller.load()
        {
            controller.preview_file(&path);
        }
    }

    /// A double click opens a folder, or a file in a tab of its own.
    fn row_double_clicked(&self, view: &NSOutlineView, row: isize) {
        let Some(item) = (row >= 0).then(|| view.itemAtRow(row)).flatten() else {
            return;
        };
        let Some(path) = path_of(&item) else { return };
        if path.is_dir() {
            // SAFETY: The item came from this outline view, which knows it.
            unsafe {
                if view.isItemExpanded(Some(&item)) {
                    view.collapseItem(Some(&item));
                } else {
                    view.expandItem(Some(&item));
                }
            }
        } else if let Some(controller) = self.ivars().controller.load() {
            controller.open_file(&path);
        }
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

    /// The object that stands for `entry` in the outline view; one per path for as long as the tree lives, since the
    /// view keeps no reference of its own.
    fn item_for(&self, entry: &Entry) -> Retained<NSString> {
        let mut rows = self.ivars().rows.borrow_mut();
        let row = rows.entry(entry.path.clone()).or_insert_with(|| RowInfo {
            item: ns(&entry.path.to_string_lossy()),
            expandable: false,
            ignored: false,
        });
        row.expandable = entry.can_expand();
        row.ignored = entry.is_ignored;
        row.item.clone()
    }

    fn clicked(&self) -> Option<PathBuf> {
        self.ivars().clicked.borrow().clone()
    }

    fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let mtm = self.mtm();
        let Some(controller) = self.ivars().controller.load() else {
            return;
        };
        let Some(view) = controller.file_tree_view() else {
            return;
        };
        let row = view.clickedRow();
        let path = view.itemAtRow(row).and_then(|item| path_of(&item));
        *self.ivars().clicked.borrow_mut() = Some(
            path.clone()
                .unwrap_or_else(|| self.ivars().project.root().to_path_buf()),
        );
        let entries: Vec<(&str, objc2::runtime::Sel, bool)> = vec![
            (t("Neue Datei …"), sel!(ideNewFile:), true),
            (t("Neuer Ordner …"), sel!(ideNewFolder:), true),
            ("", sel!(ideNewFile:), false),
            (t("Im Finder zeigen"), sel!(ideRevealInFinder:), true),
            (t("Pfad kopieren"), sel!(ideCopyPath:), true),
            (t("Relativen Pfad kopieren"), sel!(ideCopyRelativePath:), true),
            ("", sel!(ideNewFile:), false),
            (t("Umbenennen …"), sel!(ideRename:), path.is_some()),
            (t("In den Papierkorb legen"), sel!(ideMoveToTrash:), path.is_some()),
        ];
        for (title, action, enabled) in entries {
            if title.is_empty() {
                menu.addItem(&separator(mtm));
                continue;
            }
            let entry = item(mtm, title, Some(action), "");
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

    /// Asks for a name and creates a file or folder in the clicked folder, or beside the clicked file.
    fn create(&self, folder: bool) {
        if let Some(clicked) = self.clicked() {
            self.create_in(clicked, folder);
        }
    }

    /// The folder of the selected row, the folder a selected file is in, or the root without a selection.
    fn selected_folder(&self) -> PathBuf {
        let root = self.ivars().project.root().to_path_buf();
        let view = self.ivars().controller.load().and_then(|c| c.file_tree_view());
        let selected = view
            .and_then(|view| view.itemAtRow(view.selectedRow()))
            .and_then(|item| path_of(&item));
        selected.unwrap_or(root)
    }

    /// Asks for a name and creates a file or folder in `at`, or beside it if it is a file.
    fn create_in(&self, at: PathBuf, folder: bool) {
        let parent = if at.is_dir() {
            at
        } else {
            at.parent().map(Path::to_path_buf).unwrap_or(at)
        };
        let title = if folder { t("Neuer Ordner") } else { t("Neue Datei") };
        let Some(name) = ask_name(self.mtm(), title, "") else {
            return;
        };
        let target = parent.join(&name);
        let result = if folder {
            std::fs::create_dir(&target)
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map(|_| ())
        };
        match result {
            Ok(()) if !folder => {
                if let Some(controller) = self.ivars().controller.load() {
                    controller.open_file(&target);
                }
            }
            Ok(()) => {}
            Err(error) => show_error(
                self.mtm(),
                &tf("„{}“ wurde nicht angelegt.", &[&name]),
                &error.to_string(),
            ),
        }
    }

    fn rename_clicked(&self) {
        let Some(path) = self.clicked() else { return };
        let current = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(name) = ask_name(self.mtm(), t("Umbenennen"), &current) else {
            return;
        };
        if name == current {
            return;
        }
        let target = path.with_file_name(&name);
        if target.exists() && !same_file_ignoring_case(&path, &target) {
            show_error(self.mtm(), &tf("„{}“ gibt es schon.", &[&name]), "");
            return;
        }
        if let Err(error) = std::fs::rename(&path, &target) {
            show_error(
                self.mtm(),
                &tf("„{}“ wurde nicht umbenannt.", &[&current]),
                &error.to_string(),
            );
        }
    }
}

/// Whether `a` and `b` name one file on a volume that ignores case, as a rename that only changes case does.
fn same_file_ignoring_case(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

fn path_of(item: &AnyObject) -> Option<PathBuf> {
    item.downcast_ref::<NSString>().map(|s| PathBuf::from(s.to_string()))
}

fn expanded_path(notification: &NSNotification) -> Option<PathBuf> {
    let info = notification.userInfo()?;
    let item = info.objectForKey(&ns("NSObject"))?;
    path_of(&item)
}

fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&ns(&path.to_string_lossy()))
}

fn copy_text(text: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(&ns(text), constant!(NSPasteboardTypeString));
}

fn row_view(mtm: MainThreadMarker, path: &Path, ignored: bool) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let image: Retained<NSImageView> = crate::file_icon::image_view(mtm, path, path.is_dir());
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

/// Asks for a file name; `None` if the user cancels or leaves it empty, or it holds a slash.
fn ask_name(mtm: MainThreadMarker, title: &str, current: &str) -> Option<String> {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(title));
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 24.0)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    let response: NSModalResponse = alert.runModal();
    if response != objc2_app_kit::NSAlertFirstButtonReturn {
        return None;
    }
    let name = field.stringValue().to_string().trim().to_string();
    (!name.is_empty() && !name.contains('/')).then_some(name)
}

fn show_error(mtm: MainThreadMarker, message: &str, detail: &str) {
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(objc2_app_kit::NSAlertStyle::Warning);
    alert.setMessageText(&ns(message));
    alert.setInformativeText(&ns(detail));
    alert.runModal();
}
