//! The view of the search panel: the data source and delegate of its list of matches, which draws the rows from the
//! model, and the actions of its fields, buttons and menu, which go on to the controller.

use std::cell::{Cell, OnceCell, RefCell, RefMut};

use ide_ui_common::panel::{RowItem, RowItems};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSEvent, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSTableColumn, NSTextFieldDelegate, NSTextView, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSRect};

use super::model::{Model, Spot};
use super::{ESCAPE_KEY, OUTDENT, RETURN_KEYS, hierarchy, rows, views};
use crate::util::{guarded, ns};
use crate::window::WindowController;

pub(crate) struct SearchIvars {
    pub(super) id: u64,
    pub(super) controller: Weak<WindowController>,
    pub(super) views: OnceCell<views::Views>,
    pub(super) hierarchy: OnceCell<Retained<hierarchy::HierarchyPanel>>,
    pub(super) deferred_start: Cell<Option<u64>>,
    pub(super) deferred_refresh: Cell<Option<u64>>,
    pub(super) deferred_show: Cell<bool>,
    model: RefCell<Model>,
    /// The objects that stand for rows in the outline view, which keeps no reference of its own.
    items: RowItems<Spot>,
}

impl SearchIvars {
    pub(super) fn new(id: u64, controller: Weak<WindowController>, model: Model) -> SearchIvars {
        SearchIvars {
            id,
            controller,
            views: OnceCell::new(),
            hierarchy: OnceCell::new(),
            deferred_start: Cell::new(None),
            deferred_refresh: Cell::new(None),
            deferred_show: Cell::new(false),
            model: RefCell::new(model),
            items: RowItems::default(),
        }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `SearchPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESearchPanel"]
    #[ivars = SearchIvars]
    pub(crate) struct SearchPanel;

    unsafe impl NSObjectProtocol for SearchPanel {}

    unsafe impl NSControlTextEditingDelegate for SearchPanel {
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn do_command(&self, control: &NSControl, _view: &NSTextView, command: Sel) -> bool {
            guarded(false, || self.field_command(control, command))
        }

        #[unsafe(method(controlTextDidChange:))]
        fn text_changed(&self, notification: &NSNotification) {
            guarded((), || {
                if self.is_replace_field(notification) {
                    self.show();
                } else {
                    self.schedule();
                }
            });
        }
    }

    unsafe impl NSMenuDelegate for SearchPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    unsafe impl NSTextFieldDelegate for SearchPanel {}

    unsafe impl NSOutlineViewDataSource for SearchPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.children(item)).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || self.child_item(usize::try_from(index).ok()?, item));
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || matches!(spot_of(item), Some(Spot::File(_))))
        }
    }

    unsafe impl NSOutlineViewDelegate for SearchPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.row_view(item))
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || self.folded(notification, true));
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &NSNotification) {
            guarded((), || self.folded(notification, false));
        }
    }

    impl SearchPanel {
        #[unsafe(method(ideSearchOptionChanged:))]
        fn option_changed(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    views.paint_toggles();
                }
                self.schedule();
            });
        }

        #[unsafe(method(ideSearchDisclose:))]
        fn disclose(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    views.set_globs_shown(views.globs.isHidden());
                }
            });
        }

        #[unsafe(method(ideSearchRowClicked:))]
        fn row_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(views) = self.ivars().views.get() else { return };
                let row = views.outline.clickedRow();
                let is_line = views.outline.itemAtRow(row).is_some_and(|i| matches!(spot_of(&i), Some(Spot::Line(..))));
                if is_line {
                    self.open_row(row, false);
                }
            });
        }

        #[unsafe(method(ideSearchRowDoubleClicked:))]
        fn row_double_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    self.open_row(views.outline.clickedRow(), true);
                }
            });
        }

        #[unsafe(method(ideSearchToggleReplace:))]
        fn toggle_replace(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    views.set_replace_shown(!views.replace_shown());
                    if views.replace_shown()
                        && let Some(window) = views.page.window()
                    {
                        window.makeFirstResponder(Some(&views.replace));
                    }
                }
                self.show();
            });
        }

        #[unsafe(method(ideSearchReplaceAll:))]
        fn replace_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_replace_all());
        }

        #[unsafe(method(ideSearchReplaceRow:))]
        fn replace_row_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(views) = self.ivars().views.get() else { return };
                let Some(view) = sender.and_then(|s| s.downcast_ref::<NSView>()) else { return };
                self.replace_at(views.outline.rowForView(view));
            });
        }

        #[unsafe(method(ideSearchMenuReplace:))]
        fn menu_replace(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.read().menu_row;
                self.replace_at(row);
            });
        }

        #[unsafe(method(ideSearchMenuOpen:))]
        fn menu_open(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.read().menu_row;
                self.open_row(row, true);
            });
        }
    }
);

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `SearchOutlineView` does not implement Drop.
    /// Return opens the selected match, Esc ends the search.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESearchOutlineView"]
    pub(crate) struct SearchOutlineView;

    unsafe impl NSObjectProtocol for SearchOutlineView {}

    impl SearchOutlineView {
        /// A file's disclosure triangle below the chevrons of the fields, not in the source list's margin.
        /// AppKit makes no disclosure button, whose observers would keep it for good; the row's own chevron stands
        /// where it would be.
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn no_outline_cell(&self, _row: isize) -> NSRect {
            NSRect::ZERO
        }

        #[unsafe(method(didAddRowView:forRow:))]
        fn did_add_row_view(&self, row_view: &objc2_app_kit::NSTableRowView, row: isize) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), didAddRowView: row_view, forRow: row] };
            ide_ui_common::util::guarded((), || {
                // SAFETY: This class's own method, with its signature.
                let frame: NSRect = unsafe { msg_send![self, ideDisclosureFrameAtRow: row] };
                ide_ui_common::panel::chevron::place(self, row_view, frame);
            });
        }

        #[unsafe(method(ideDisclosureFrameAtRow:))]
        fn outline_cell_frame(&self, row: isize) -> NSRect {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfOutlineCellAtRow: row] };
            crate::window::file_outline::outdent(self, frame, OUTDENT, false)
        }

        /// The rows moved along with the triangle; a match begins where its file's icon does.
        #[unsafe(method(frameOfCellAtColumn:row:))]
        fn cell_frame(&self, column: isize, row: isize) -> NSRect {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfCellAtColumn: column, row: row] };
            if column != 0 {
                return frame;
            }
            let by = OUTDENT + self.levelForRow(row).max(0) as f64 * self.indentationPerLevel();
            crate::window::file_outline::outdent(self, frame, by, true)
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if !guarded(false, || self.key(event)) {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl SearchOutlineView {
    pub(crate) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<SearchOutlineView> {
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![SearchOutlineView::alloc(mtm), initWithFrame: frame] }
    }

    fn panel(&self) -> Option<Retained<SearchPanel>> {
        // SAFETY: Only `views::Views::new` makes this view, and it sets the panel as the data source.
        let source = unsafe { self.dataSource() }?;
        let object: Retained<AnyObject> = source.into();
        object.downcast::<SearchPanel>().ok()
    }

    fn key(&self, event: &NSEvent) -> bool {
        let Some(panel) = self.panel() else { return false };
        let code = event.keyCode();
        if RETURN_KEYS.contains(&code) {
            panel.open_row(self.selectedRow(), true);
        } else if code == ESCAPE_KEY {
            panel.clear();
        } else {
            return false;
        }
        true
    }
}

impl SearchPanel {
    pub(super) fn model(&self) -> RefMut<'_, Model> {
        self.ivars().model.borrow_mut()
    }

    pub(super) fn read(&self) -> std::cell::Ref<'_, Model> {
        self.ivars().model.borrow()
    }

    /// The panel's page: the fields and their options above the matches.
    pub(crate) fn page(&self) -> Option<&NSView> {
        self.ivars().views.get().map(|views| &*views.page)
    }

    /// The option toggles for the panel's title row.
    pub(crate) fn header(&self) -> Option<&NSView> {
        self.ivars().views.get().map(|views| &*views.header as &NSView)
    }

    /// The object that stands for `spot`, the same one for as long as the model has the row.
    pub(super) fn item(&self, spot: &Spot) -> Retained<RowItem> {
        self.ivars().items.item(self.mtm(), spot)
    }

    /// Lets go of the objects of rows the model no longer has.
    pub(super) fn forget_items(&self) {
        let model = self.read();
        self.ivars().items.keep(|spot| model.has(spot));
    }

    fn children(&self, item: Option<&AnyObject>) -> usize {
        match item.map(spot_of) {
            None => self.read().children(None),
            Some(spot) => spot.map_or(0, |spot| self.read().children(Some(&spot))),
        }
    }

    fn child_item(&self, index: usize, item: Option<&AnyObject>) -> Option<Retained<RowItem>> {
        let spot = match item.map(spot_of) {
            None => self.read().child(index, None),
            Some(parent) => self.read().child(index, Some(&parent?)),
        }?;
        Some(self.item(&spot))
    }

    fn row_view(&self, item: &AnyObject) -> Option<Retained<NSView>> {
        let views = self.ivars().views.get()?;
        let with = views.replace.stringValue().to_string();
        let options = self.options();
        let replacing = views.replace_shown().then_some(rows::Replacing {
            panel: self,
            with: &with,
            options: &options,
        });
        let model = self.read();
        let mtm = self.mtm();
        match spot_of(item)? {
            Spot::File(path) => {
                let group = model.group(&path)?;
                Some(rows::file_row(
                    mtm,
                    &group.path,
                    &group.relative,
                    group.matches,
                    replacing.as_ref(),
                ))
            }
            Spot::Line(path, index) => {
                let group = model.group(&path)?;
                let widest = group.lines.iter().map(|l| l.line).max().unwrap_or_default();
                Some(rows::line_row(mtm, group.lines.get(index)?, widest, replacing.as_ref()))
            }
        }
    }
}

/// Where `item`, an outline item of a search panel, points.
pub(super) fn spot_of(item: &AnyObject) -> Option<Spot> {
    RowItems::<Spot>::key_of(item)
}
