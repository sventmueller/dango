//! The view of the files panel: the outline view's data source and delegate, which draws the rows and passes clicks,
//! folds and menu commands on to the controller.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use ide_project::Entry;
use ide_ui_common::panel::{RowItem, RowItems};
use ide_ui_common::ui_size;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSColor, NSControlTextEditingDelegate, NSImageAlignment, NSImageView, NSLineBreakMode, NSMenu,
    NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSProgressIndicator,
    NSProgressIndicatorStyle, NSTableCellView, NSTableColumn, NSTextField, NSUserInterfaceItemIdentification,
    NSUserInterfaceLayoutDirection, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};

use super::model::{Command, Kind, MenuEntry, Model, Row};
use crate::project::Project;
use crate::util::{activate, auto_layout, guarded, item, ns, separator, set_target};
use crate::window::WindowController;

pub(crate) struct TreeIvars {
    pub(super) project: Rc<Project>,
    pub(super) controller: Weak<WindowController>,
    pub(super) model: RefCell<Model>,
    /// The objects that stand for rows in the outline view, one per path for as long as the tree lives, since the
    /// view keeps no reference of its own.
    items: RowItems<PathBuf>,
    /// The entries of each open folder as the outline view was last told them.
    pub(super) shown: RefCell<HashMap<PathBuf, Arc<[Entry]>>>,
    /// The folders opened whose entries are still being read.
    pub(super) pending: RefCell<HashMap<PathBuf, Pending>>,
    /// Set while the tree opens the folders inside one the user opened with all its folders.
    pub(super) cascade: Cell<bool>,
}

/// A folder that opened before its entries were read.
pub(super) struct Pending {
    /// The row that shows a spinner in the folder once reading takes long enough to be seen.
    pub(super) spinner: Option<Retained<NSObject>>,
    /// The folders inside open as well, as Option asks.
    pub(super) deep: bool,
}

/// What a folder's rows are as the outline view asks for them.
enum Listed {
    Entries(Arc<[Entry]>),
    Loading(Retained<NSObject>),
    Nothing,
}

/// The identifier of the rows the outline view keeps for reuse.
const ROW: &str = "file";

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `FileTreeSource` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFileTreeSource"]
    #[ivars = TreeIvars]
    pub(crate) struct FileTreeSource;

    unsafe impl NSObjectProtocol for FileTreeSource {}

    unsafe impl NSOutlineViewDataSource for FileTreeSource {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || match self.children_of(item) {
                Listed::Entries(entries) => isize::try_from(entries.len()).unwrap_or(0),
                Listed::Loading(_) => 1,
                Listed::Nothing => 0,
            })
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found: Option<Retained<AnyObject>> = guarded(None, || match self.children_of(item) {
                Listed::Entries(entries) => Some(self.item_for(entries.get(usize::try_from(index).ok()?)?).into()),
                Listed::Loading(spinner) => Some(spinner.into()),
                Listed::Nothing => None,
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
                let Some(path) = path_of(item) else {
                    return Some(spinner_view(self.mtm()));
                };
                let row = self.ivars().model.borrow().row(&path).unwrap_or_default();
                // SAFETY: The item came from this outline view, which knows it.
                let open = row.expandable.then(|| unsafe { view.isItemExpanded(Some(item)) });
                // SAFETY: The identifier names cells `row_view` made; no owner is involved.
                let reused = unsafe { view.makeViewWithIdentifier_owner(&ns(ROW), None) };
                let cell = reused.and_then(|cell| cell.downcast::<NSTableCellView>().ok());
                let cell = cell.unwrap_or_else(|| row_view(self.mtm()));
                fill_row(&cell, &path, row.ignored, open);
                Some(cell.into_super())
            })
        }

        #[unsafe(method(outlineViewItemWillExpand:))]
        fn will_expand(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(path) = expanded_path(notification) {
                    let deep = self.ivars().cascade.get() || option_held(self.mtm());
                    self.folder_opens(&path, deep);
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
                let row = view.clickedRow();
                let handled = match pane_click(self.mtm()) {
                    Some(choose) => self.row_alt_clicked(view, row, choose),
                    None => false,
                };
                if !handled && !extends_selection(self.mtm()) {
                    self.row_clicked(view, row);
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

        #[unsafe(method(ideOpenBelowClicked:))]
        fn open_below(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.command(Command::OpenBelow));
        }

        #[unsafe(method(ideOpenInPaneChosen:))]
        fn open_in_pane_chosen(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender
                    .and_then(|sender| sender.downcast_ref::<objc2_app_kit::NSMenuItem>())
                    .map_or(0, |item| item.tag());
                self.open_in_chosen_pane(tag);
            });
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
            guarded((), || self.create_from_header(Kind::File));
        }

        #[unsafe(method(ideTreeNewFolder:))]
        fn header_new_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.create_from_header(Kind::Folder));
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
            items: RowItems::default(),
            shown: RefCell::new(HashMap::new()),
            pending: RefCell::new(HashMap::new()),
            cascade: Cell::new(false),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(FileTreeSource::alloc(mtm).set_ivars(ivars)), init] }
    }

    /// The object that stands for `path` in the outline view, if the view was handed one.
    pub(super) fn item_of(&self, path: &Path) -> Option<Retained<RowItem>> {
        self.ivars().items.get(&path.to_path_buf())
    }

    fn children_of(&self, item: Option<&AnyObject>) -> Listed {
        let dir = match item {
            Some(item) => match path_of(item) {
                Some(path) => path,
                None => return Listed::Nothing,
            },
            None => self.ivars().project.root().to_path_buf(),
        };
        if let Some(entries) = self.ivars().shown.borrow().get(&dir) {
            return Listed::Entries(entries.clone());
        }
        match self.ivars().pending.borrow().get(&dir).and_then(|p| p.spinner.clone()) {
            Some(spinner) => Listed::Loading(spinner),
            None => Listed::Nothing,
        }
    }

    pub(super) fn item_for(&self, entry: &Entry) -> Retained<RowItem> {
        self.ivars().model.borrow_mut().note(&entry.path, Row::of(entry));
        self.ivars().items.item(self.mtm(), &entry.path)
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
        Command::OpenBelow => sel!(ideOpenBelowClicked:),
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

/// Whether the click that is being handled opens a file in a pane: beside the one in front with Option, or in a pane
/// the user chooses with Option and Shift (E145).
fn pane_click(mtm: MainThreadMarker) -> Option<bool> {
    use objc2_app_kit::NSEventModifierFlags as Flags;
    let event = NSApplication::sharedApplication(mtm).currentEvent()?;
    let flags = event.modifierFlags() & (Flags::Option | Flags::Shift | Flags::Command | Flags::Control);
    if flags == Flags::Option {
        Some(false)
    } else if flags == Flags::Option | Flags::Shift {
        Some(true)
    } else {
        None
    }
}

pub(super) fn path_of(item: &AnyObject) -> Option<PathBuf> {
    RowItems::<PathBuf>::key_of(item)
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

/// Whether the event being handled holds Option, which opens a folder with all the folders inside.
fn option_held(mtm: MainThreadMarker) -> bool {
    let event = NSApplication::sharedApplication(mtm).currentEvent();
    event.is_some_and(|event| {
        event
            .modifierFlags()
            .contains(objc2_app_kit::NSEventModifierFlags::Option)
    })
}

/// The row of a folder whose entries take a while to read.
fn spinner_view(mtm: MainThreadMarker) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let spinner = NSProgressIndicator::new(mtm);
    spinner.setStyle(NSProgressIndicatorStyle::Spinning);
    spinner.setControlSize(objc2_app_kit::NSControlSize::Small);
    cell.addSubview(&spinner);
    spinner.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        spinner.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        spinner.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ]);
    // SAFETY: A nil sender is what AppKit's own callers pass.
    unsafe { spinner.startAnimation(None) };
    cell.into_super()
}

/// Shows `path` in `cell`: its icon, or the chevron of a folder that is `open` or not, and its name.
fn fill_row(cell: &NSTableCellView, path: &Path, ignored: bool, open: Option<bool>) {
    let mtm = cell.mtm();
    // SAFETY: The cell is one `row_view` made, whose image view and text field it set.
    let (Some(image), Some(label)) = (unsafe { cell.imageView() }, unsafe { cell.textField() }) else {
        return;
    };
    crate::file_icon::show_scaled(&image, path, open.is_some(), ui_size::scaled(1.0));
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
    } else {
        image.setImageAlignment(NSImageAlignment::AlignCenter);
    }
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    label.setStringValue(&ns(&name));
    label.setTextColor(Some(&*if ignored {
        NSColor::tertiaryLabelColor()
    } else {
        NSColor::labelColor()
    }));
    cell.setToolTip(Some(&ns(&path.to_string_lossy())));
}

/// The side of a row's icon at the default interface size.
const ICON_SIDE: f64 = 16.0;
/// The room between a row's icon and its name at the default interface size.
const ICON_GAP: f64 = 5.0;

/// An empty row for `fill_row`, which the outline view keeps for reuse.
fn row_view(mtm: MainThreadMarker) -> Retained<NSTableCellView> {
    let cell = NSTableCellView::new(mtm);
    cell.setIdentifier(Some(&ns(ROW)));
    let image: Retained<NSImageView> = crate::file_icon::image_view(mtm, Path::new(""), true);
    let label = NSTextField::labelWithString(&ns(""), mtm);
    ui_size::set_system_font(&label, ide_ui_common::style::BODY);
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    cell.addSubview(&image);
    cell.addSubview(&label);
    // SAFETY: Both views are subviews of the cell, which keeps them.
    unsafe {
        cell.setImageView(Some(&image));
        cell.setTextField(Some(&label));
    }
    auto_layout(&[&image, &label]);
    let (width, height) = (
        image.widthAnchor().constraintEqualToConstant(ICON_SIDE),
        image.heightAnchor().constraintEqualToConstant(ICON_SIDE),
    );
    let gap = label
        .leadingAnchor()
        .constraintEqualToAnchor_constant(&image.trailingAnchor(), ICON_GAP);
    ui_size::follow_length(&width, ICON_SIDE);
    ui_size::follow_length(&height, ICON_SIDE);
    ui_size::follow_length(&gap, ICON_GAP);
    activate([
        image.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        image.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        width,
        height,
        gap,
        label
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor(&cell.trailingAnchor()),
        label.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ]);
    cell
}

#[cfg(feature = "selftest")]
mod selftest_probe;
