//! Open Quickly (⌘P): a panel over the window that finds files of the folder by a few typed letters, or with a `#`
//! before them the project's symbols, which the language servers search (E85).

use std::cell::RefCell;
use std::path::PathBuf;

use ide_project::Hit;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSBackingStoreType, NSColor, NSControl, NSControlTextEditingDelegate, NSFont,
    NSFontWeightSemibold, NSForegroundColorAttributeName, NSPanel, NSScrollView, NSSearchField, NSSearchFieldDelegate,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTableViewStyle, NSTextField,
    NSTextFieldDelegate, NSTextView, NSView, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{NSMutableAttributedString, NSNotification, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize};
use splat_i18n::t;

use super::WindowController;
use crate::util::{constant, guarded, main_soon, ns, set_action};
#[cfg(feature = "selftest")]
mod selftest_probe;

const WIDTH: f64 = 620.0;
const HEIGHT: f64 = 380.0;
const RESULTS: usize = 60;
/// How often a symbol search that found nothing asks again while a server still indexes.
const RETRY: f64 = 1.0;

/// What the list shows.
enum Rows {
    Files(Vec<Hit>),
    Symbols(Vec<crate::lsp::FoundSymbol>),
}

impl Rows {
    fn len(&self) -> usize {
        match self {
            Rows::Files(hits) => hits.len(),
            Rows::Symbols(symbols) => symbols.len(),
        }
    }
}

/// The query after the `#` that asks for symbols, or `None` for a file query.
fn symbol_query(query: &str) -> Option<&str> {
    query.strip_prefix('#')
}

pub struct QuickIvars {
    controller: Weak<WindowController>,
    root: PathBuf,
    hits: RefCell<Rows>,
    status: RefCell<Option<Retained<NSTextField>>>,
    panel: RefCell<Option<Retained<NSPanel>>>,
    field: RefCell<Option<Retained<NSSearchField>>>,
    table: RefCell<Option<Retained<NSTableView>>>,
    /// Counts queries, so a late answer to an older one is dropped.
    asked: std::cell::Cell<u64>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `QuickOpenPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEQuickOpenPanel"]
    #[ivars = QuickIvars]
    pub struct QuickOpenPanel;

    unsafe impl NSObjectProtocol for QuickOpenPanel {}

    unsafe impl NSControlTextEditingDelegate for QuickOpenPanel {
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn do_command(&self, _control: &NSControl, _view: &NSTextView, command: Sel) -> bool {
            guarded(false, || self.command(command))
        }

        #[unsafe(method(controlTextDidChange:))]
        fn text_changed(&self, _notification: &NSNotification) {
            guarded((), || self.ask());
        }
    }

    unsafe impl NSTextFieldDelegate for QuickOpenPanel {}

    unsafe impl NSSearchFieldDelegate for QuickOpenPanel {}

    unsafe impl NSTableViewDataSource for QuickOpenPanel {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn rows(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().hits.borrow().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for QuickOpenPanel {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let row = usize::try_from(row).ok()?;
                match &*self.ivars().hits.borrow() {
                    Rows::Files(hits) => Some(hit_view(self.mtm(), hits.get(row)?)),
                    Rows::Symbols(symbols) => Some(symbol_view(self.mtm(), symbols.get(row)?, &self.ivars().root)),
                }
            })
        }
    }

    unsafe impl NSWindowDelegate for QuickOpenPanel {
        #[unsafe(method(windowDidResignKey:))]
        fn resigned(&self, _notification: &NSNotification) {
            guarded((), || self.close());
        }
    }

    impl QuickOpenPanel {
        #[unsafe(method(ideOpenHit:))]
        fn open_hit(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.open_selected());
        }

        #[unsafe(method(ideQuickOpenRetry))]
        fn retry(&self) {
            guarded((), || self.ask());
        }
    }
);

impl QuickOpenPanel {
    pub fn new(mtm: MainThreadMarker, controller: &WindowController, root: PathBuf) -> Retained<QuickOpenPanel> {
        let ivars = QuickIvars {
            controller: Weak::from(controller),
            root,
            hits: RefCell::new(Rows::Files(vec![])),
            status: RefCell::new(None),
            panel: RefCell::new(None),
            field: RefCell::new(None),
            table: RefCell::new(None),
            asked: std::cell::Cell::new(0),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(QuickOpenPanel::alloc(mtm).set_ivars(ivars)), init] }
    }

    /// Shows the panel over `parent`, with `query` in the field and the field focused.
    pub fn show(&self, parent: &objc2_app_kit::NSWindow, query: &str) {
        let mtm = self.mtm();
        if self.ivars().panel.borrow().is_none() {
            self.build(mtm);
        }
        let Some(panel) = self.ivars().panel.borrow().clone() else {
            return;
        };
        let frame = parent.frame();
        let origin = NSPoint::new(
            frame.origin.x + (frame.size.width - WIDTH) / 2.0,
            frame.origin.y + frame.size.height - HEIGHT - 90.0,
        );
        panel.setFrameOrigin(origin);
        if let Some(field) = self.ivars().field.borrow().as_ref() {
            field.setStringValue(&ns(query));
        }
        *self.ivars().hits.borrow_mut() = Rows::Files(vec![]);
        self.set_status(None);
        self.reload();
        // SAFETY: The parent keeps a weak reference to the panel, which the ivars own and `close` detaches.
        unsafe { parent.addChildWindow_ordered(&panel, objc2_app_kit::NSWindowOrderingMode::Above) };
        panel.makeKeyAndOrderFront(None);
        if let Some(field) = self.ivars().field.borrow().as_ref() {
            panel.makeFirstResponder(Some(field));
            if let Some(editor) = field.currentEditor() {
                editor.setSelectedRange(NSRange::new(query.encode_utf16().count(), 0));
            }
        }
        self.ask();
    }

    fn build(&self, mtm: MainThreadMarker) {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setTitlebarAppearsTransparent(true);
        panel.setTitleVisibility(objc2_app_kit::NSWindowTitleVisibility::Hidden);
        panel.setTitle(&ns(t("Schnell öffnen")));
        panel.setAutorecalculatesKeyViewLoop(true);
        panel.setMovable(false);
        // SAFETY: Switching it off keeps AppKit from releasing the panel behind Rust's back; the ivars own it.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setDelegate(Some(ProtocolObject::from_ref(self)));
        let field = NSSearchField::new(mtm);
        field.setPlaceholderString(Some(&ns(t("Datei suchen"))));
        field.setFont(Some(&NSFont::systemFontOfSize(ide_ui_common::style::FIELD_LARGE)));
        // SAFETY: The field holds its delegate weakly; the window controller keeps this object alive.
        unsafe { field.setDelegate(Some(ProtocolObject::from_ref(self))) };
        let table = NSTableView::new(mtm);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("hit"));
        table.addTableColumn(&column);
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::Inset);
        table.setRowHeight(34.0);
        table.setAccessibilityLabel(Some(&ns(t("Gefundene Dateien"))));
        // SAFETY: The table holds its data source and delegate weakly; the window controller keeps this panel alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        set_action(&table, self, sel!(ideOpenHit:));
        // SAFETY: The double action goes to this object, which answers it with a method that takes the sender.
        unsafe { table.setDoubleAction(Some(sel!(ideOpenHit:))) };
        let scroll = NSScrollView::new(mtm);
        scroll.setDocumentView(Some(&table));
        scroll.setHasVerticalScroller(true);
        scroll.setDrawsBackground(false);
        let status = NSTextField::labelWithString(&ns(""), mtm);
        status.setTextColor(Some(&NSColor::secondaryLabelColor()));
        status.setHidden(true);
        let Some(content) = panel.contentView() else { return };
        content.addSubview(&field);
        content.addSubview(&scroll);
        content.addSubview(&status);
        status.setTranslatesAutoresizingMaskIntoConstraints(false);
        status
            .centerXAnchor()
            .constraintEqualToAnchor(&content.centerXAnchor())
            .setActive(true);
        status
            .topAnchor()
            .constraintEqualToAnchor_constant(&field.bottomAnchor(), 24.0)
            .setActive(true);
        for view in [&*field as &NSView, &scroll] {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        for constraint in [
            field
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&content.leadingAnchor(), 14.0),
            field
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&content.trailingAnchor(), -14.0),
            field
                .topAnchor()
                .constraintEqualToAnchor_constant(&content.topAnchor(), 30.0),
            scroll.leadingAnchor().constraintEqualToAnchor(&content.leadingAnchor()),
            scroll
                .trailingAnchor()
                .constraintEqualToAnchor(&content.trailingAnchor()),
            scroll
                .topAnchor()
                .constraintEqualToAnchor_constant(&field.bottomAnchor(), 10.0),
            scroll.bottomAnchor().constraintEqualToAnchor(&content.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        *self.ivars().panel.borrow_mut() = Some(panel);
        *self.ivars().status.borrow_mut() = Some(status);
        *self.ivars().field.borrow_mut() = Some(field);
        *self.ivars().table.borrow_mut() = Some(table);
    }

    fn ask(&self) {
        let query = self
            .ivars()
            .field
            .borrow()
            .as_ref()
            .map(|f| f.stringValue().to_string())
            .unwrap_or_default();
        let Some(controller) = self.ivars().controller.load() else {
            return;
        };
        let asked = self.ivars().asked.get() + 1;
        self.ivars().asked.set(asked);
        let root = self.ivars().root.clone();
        let symbols = symbol_query(&query).map(str::to_string);
        if let Some(table) = self.ivars().table.borrow().as_ref() {
            let name = if symbols.is_some() {
                t("Gefundene Symbole")
            } else {
                t("Gefundene Dateien")
            };
            table.setAccessibilityLabel(Some(&ns(name)));
        }
        let Some(symbols) = symbols else {
            controller.query_files(&query, RESULTS, move |hits| {
                main_soon(move || super::registry::quick_open_answered(&root, asked, hits));
            });
            return;
        };
        let this = Weak::from(self);
        crate::lsp::search_symbols(&root, &symbols, move |answer| {
            if let Some(this) = this.load() {
                this.symbols_answered(asked, answer);
            }
        });
    }

    /// Shows `hits`, the answer to query number `asked`, unless a newer one was sent since.
    pub fn answered(&self, asked: u64, hits: Vec<Hit>) {
        if asked != self.ivars().asked.get() {
            return;
        }
        *self.ivars().hits.borrow_mut() = Rows::Files(hits);
        self.set_status(None);
        self.reload();
    }

    /// Shows the symbols found for query number `asked`; while nothing is found and a server still indexes, the list
    /// says so and asks again shortly.
    fn symbols_answered(&self, asked: u64, answer: crate::lsp::SymbolAnswer) {
        if asked != self.ivars().asked.get() {
            return;
        }
        let query = self
            .ivars()
            .field
            .borrow()
            .as_ref()
            .map(|f| f.stringValue().to_string())
            .unwrap_or_default();
        let typed = symbol_query(&query).is_some_and(|q| !q.trim().is_empty());
        let status = if answer.untrusted {
            Some(t("Sprachserver aus: Ordner nicht vertraut"))
        } else if !answer.symbols.is_empty() || !typed {
            None
        } else if answer.indexing {
            Some(t("Wird indexiert …"))
        } else {
            Some(t("Nichts gefunden"))
        };
        let open = self.ivars().panel.borrow().as_ref().is_some_and(|p| p.isVisible());
        if answer.indexing && answer.symbols.is_empty() && typed && open {
            use objc2_foundation::NSObjectNSDelayedPerforming;
            let object: &NSObject = self;
            // SAFETY: A selector without arguments that this class answers; the run loop retains the target until
            // the call.
            unsafe {
                NSObject::cancelPreviousPerformRequestsWithTarget_selector_object(
                    object,
                    sel!(ideQuickOpenRetry),
                    None,
                );
                object.performSelector_withObject_afterDelay(sel!(ideQuickOpenRetry), None, RETRY);
            }
        }
        *self.ivars().hits.borrow_mut() = Rows::Symbols(answer.symbols);
        self.set_status(status);
        self.reload();
    }

    fn set_status(&self, text: Option<&str>) {
        if let Some(status) = self.ivars().status.borrow().as_ref() {
            status.setStringValue(&ns(text.unwrap_or("")));
            status.setHidden(text.is_none());
        }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.ivars().root
    }

    fn reload(&self) {
        if let Some(table) = self.ivars().table.borrow().as_ref() {
            table.reloadData();
            if self.ivars().hits.borrow().len() > 0 {
                table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(0), false);
            }
        }
    }

    fn command(&self, command: Sel) -> bool {
        let Some(table) = self.ivars().table.borrow().clone() else {
            return false;
        };
        let count = isize::try_from(self.ivars().hits.borrow().len()).unwrap_or(0);
        let selected = table.selectedRow();
        let select = |row: isize| {
            if (0..count).contains(&row) {
                let index = objc2_foundation::NSIndexSet::indexSetWithIndex(usize::try_from(row).unwrap_or(0));
                table.selectRowIndexes_byExtendingSelection(&index, false);
                table.scrollRowToVisible(row);
            }
        };
        if command == sel!(moveDown:) {
            select(selected + 1);
        } else if command == sel!(moveUp:) {
            select(selected - 1);
        } else if command == sel!(insertNewline:) {
            self.open_selected();
        } else if command == sel!(cancelOperation:) {
            self.close();
        } else {
            return false;
        }
        true
    }

    fn open_selected(&self) {
        let row = self.ivars().table.borrow().as_ref().map_or(-1, |t| t.selectedRow());
        let Ok(row) = usize::try_from(row) else { return };
        let (hit, symbol) = match &*self.ivars().hits.borrow() {
            Rows::Files(hits) => (hits.get(row).cloned(), None),
            Rows::Symbols(symbols) => (None, symbols.get(row).cloned()),
        };
        self.close();
        if let Some(symbol) = symbol {
            crate::lsp::open_symbol(&self.ivars().root, &symbol);
        } else if let (Some(hit), Some(controller)) = (hit, self.ivars().controller.load()) {
            controller.open_file(&self.ivars().root.join(&hit.path));
        }
    }

    pub(super) fn close(&self) {
        if let Some(panel) = self.ivars().panel.borrow().as_ref() {
            if let Some(parent) = panel.parentWindow() {
                parent.removeChildWindow(panel);
            }
            panel.orderOut(None);
        }
    }
}

/// A symbol: the kind's symbol, the name with what contains it, and below them its file and line.
fn symbol_view(mtm: MainThreadMarker, found: &crate::lsp::FoundSymbol, root: &std::path::Path) -> Retained<NSView> {
    let symbol = &found.symbol;
    let mut title = symbol.name.clone();
    if let Some(container) = symbol.container.as_deref().filter(|c| !c.is_empty()) {
        title.push_str("  ");
        title.push_str(container);
    }
    let name = NSTextField::labelWithString(&ns(&title), mtm);
    // SAFETY: AppKit sets its font weight constants when it loads.
    name.setFont(Some(&NSFont::systemFontOfSize_weight(
        ide_ui_common::style::HEADING,
        unsafe { NSFontWeightSemibold },
    )));
    name.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
    let path = found.path.strip_prefix(root).unwrap_or(&found.path);
    let place = match symbol.range {
        Some(range) => format!("{}:{}", path.display(), range.start.line + 1),
        None => path.display().to_string(),
    };
    let place = NSTextField::labelWithString(&ns(&place), mtm);
    place.setFont(Some(&NSFont::systemFontOfSize(ide_ui_common::style::SMALL)));
    place.setTextColor(Some(&NSColor::secondaryLabelColor()));
    place.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingHead);
    let icon = objc2_app_kit::NSImageView::new(mtm);
    icon.setImage(crate::util::symbol(crate::lsp::kind_symbol(symbol.kind), "").as_deref());
    icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    icon.setAccessibilityElement(false);
    let cell = NSView::new(mtm);
    for view in [&*icon as &NSView, &name, &place] {
        cell.addSubview(view);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    for constraint in [
        icon.leadingAnchor()
            .constraintEqualToAnchor_constant(&cell.leadingAnchor(), 6.0),
        icon.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        icon.widthAnchor().constraintEqualToConstant(16.0),
        icon.heightAnchor().constraintEqualToConstant(16.0),
        name.leadingAnchor()
            .constraintEqualToAnchor_constant(&icon.trailingAnchor(), 6.0),
        name.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -6.0),
        name.topAnchor()
            .constraintEqualToAnchor_constant(&cell.topAnchor(), 1.0),
        place.leadingAnchor().constraintEqualToAnchor(&name.leadingAnchor()),
        place
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -6.0),
        place.topAnchor().constraintEqualToAnchor(&name.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    cell
}

/// The relative path with the file name first and the matched letters in the accent colour.
fn hit_view(mtm: MainThreadMarker, hit: &Hit) -> Retained<NSView> {
    let text = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns(&hit.path));
    let key = constant!(NSForegroundColorAttributeName);
    for range in &hit.ranges {
        // SAFETY: The attribute name is AppKit's foreground colour key and the value a colour.
        unsafe {
            text.addAttribute_value_range(
                key,
                &NSColor::controlAccentColor(),
                NSRange::new(range.start, range.end - range.start),
            );
        }
    }
    let label = NSTextField::labelWithAttributedString(&text, mtm);
    // SAFETY: AppKit sets its font weight constants when it loads.
    label.setFont(Some(&NSFont::systemFontOfSize_weight(
        ide_ui_common::style::HEADING,
        unsafe { NSFontWeightSemibold },
    )));
    label.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingHead);
    let icon = crate::file_icon::image_view(mtm, std::path::Path::new(&hit.path), false);
    let cell = NSView::new(mtm);
    cell.addSubview(&icon);
    cell.addSubview(&label);
    icon.setTranslatesAutoresizingMaskIntoConstraints(false);
    label.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        icon.leadingAnchor()
            .constraintEqualToAnchor_constant(&cell.leadingAnchor(), 6.0),
        icon.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        icon.widthAnchor().constraintEqualToConstant(16.0),
        icon.heightAnchor().constraintEqualToConstant(16.0),
        label
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&icon.trailingAnchor(), 6.0),
        label
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -6.0),
        label.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    cell
}

#[cfg(test)]
mod tests {
    use super::symbol_query;

    #[test]
    fn a_leading_hash_asks_for_symbols() {
        assert_eq!(symbol_query("#Model"), Some("Model"));
        assert_eq!(symbol_query("#"), Some(""));
        assert_eq!(symbol_query("main.rs"), None);
        assert_eq!(symbol_query("a#b"), None);
    }
}
