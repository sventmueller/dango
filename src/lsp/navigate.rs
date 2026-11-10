//! Going places: to a definition (⌘-click or the menu), to the uses of a symbol in a list, to a symbol of the file
//! through a menu at the cursor, and from problem to problem.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ide_lsp::lsp_types::{DocumentSymbol, Location, Range as LspRange, SymbolKind};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSControlTextEditingDelegate, NSFont, NSLineBreakMode, NSMenu, NSPanel, NSScrollView,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTableViewStyle, NSView,
    NSWindowStyleMask,
};
use objc2_foundation::{NSIndexSet, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::decorate::Severity;
use super::{server_of, text};
use crate::editor::{self, EditorTextView};
use crate::util::{guarded, item, label, main_soon, ns, set_action};
use crate::window::WindowController;

/// Goes to the definition of the symbol at the cursor.
pub fn definition(view: &EditorTextView) {
    definition_at(view, view.selectedRange().location);
}

/// Goes to the definition of the symbol at UTF-16 offset `index`, or lists them when there are several.
pub fn definition_at(view: &EditorTextView, index: usize) {
    let Some((server, uri)) = server_of(view) else { return };
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let serial = view.ivars().lsp.borrow().serial;
    let _ = server.definition(&uri, position, move |answer| {
        let links = answer.unwrap_or_default();
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            let locations: Vec<Location> = links
                .into_iter()
                .map(|l| Location::new(l.target_uri, l.target_selection_range))
                .collect();
            match locations.as_slice() {
                [] => beep(),
                [one] => go(&view, one),
                _ => list(
                    &view,
                    &tf("{} Definitionen", &[&locations.len().to_string()]),
                    locations,
                ),
            }
        });
    });
}

/// Lists the uses of the symbol at the cursor.
pub fn references(view: &EditorTextView) {
    let Some((server, uri)) = server_of(view) else { return };
    let index = view.selectedRange().location;
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let serial = view.ivars().lsp.borrow().serial;
    let _ = server.references(&uri, position, true, move |answer| {
        let locations = answer.unwrap_or_default();
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            if locations.is_empty() {
                beep();
                return;
            }
            let title = match locations.len() {
                1 => t("1 Verwendung").to_string(),
                n => tf("{} Verwendungen", &[&n.to_string()]),
            };
            list(&view, &title, locations);
        });
    });
}

fn beep() {
    objc2_app_kit::NSBeep();
}

fn go(view: &EditorTextView, location: &Location) {
    let Some(path) = ide_lsp::uri::to_path(&location.uri) else {
        return;
    };
    let Some(controller) = view.controller() else { return };
    open_at(&controller, &path, location.range);
}

/// Opens `path` in `controller`, or brings its editor forward, and selects `range`.
fn open_at(controller: &WindowController, path: &Path, range: LspRange) -> bool {
    controller.open_file(path);
    let Some(target) = editor::editor_for(controller, path) else {
        return false;
    };
    let selection = text::range(target.document().text(), range);
    let range = NSRange::new(selection.start, selection.end - selection.start);
    target.setSelectedRange(range);
    target.scrollRangeToVisible(range);
    if let Some(window) = target.window() {
        window.makeFirstResponder(Some(&target));
    }
    true
}

/// Shows `path` with `selection` in the window of `folder`, as a server may ask.
pub fn show(folder: &Path, path: &Path, selection: Option<LspRange>) -> bool {
    let Some(controller) = crate::window::for_project(folder) else {
        return false;
    };
    open_at(&controller, path, selection.unwrap_or_default())
}

/// Jumps to the next problem after the cursor, or the one before it, and shows its message.
pub fn next_problem(view: &EditorTextView, forward: bool) {
    let cursor = view.selectedRange().location;
    let target = {
        let state = view.ivars().lsp.borrow();
        let marks: Vec<_> = state.marks.iter().filter(|m| m.severity >= Severity::Warning).collect();
        let found = if forward {
            marks.iter().find(|m| m.range.start > cursor).or(marks.first())
        } else {
            marks.iter().rev().find(|m| m.range.start < cursor).or(marks.last())
        };
        found.map(|m| (m.range.clone(), m.message.clone()))
    };
    let Some((range, message)) = target else {
        beep();
        return;
    };
    view.setSelectedRange(NSRange::new(range.start, 0));
    view.scrollRangeToVisible(NSRange::new(range.start, range.len()));
    super::hover::show(view, range, &message);
}

/// The symbols of the file in a menu at the cursor; choosing one goes there.
pub fn symbols(view: &EditorTextView) {
    let Some((server, uri)) = server_of(view) else { return };
    let serial = view.ivars().lsp.borrow().serial;
    let _ = server.document_symbols(&uri, move |answer| {
        let symbols = answer.unwrap_or_default();
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            if symbols.is_empty() {
                beep();
                return;
            }
            symbol_menu(&view, &symbols);
        });
    });
}

fn symbol_menu(view: &EditorTextView, symbols: &[DocumentSymbol]) {
    let mtm = view.mtm();
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns(t("Symbole")));
    let mut entries = vec![];
    flatten(symbols, 0, &mut entries);
    let text = view.document();
    for (depth, symbol) in entries {
        let title = format!("{} {}", kind_name(symbol.kind), symbol.name);
        let entry = item(mtm, &title, Some(sel!(ideJumpToOffset:)), "");
        entry.setIndentationLevel(isize::try_from(depth.min(15)).unwrap_or(0));
        let offset = text::offset(text.text(), symbol.selection_range.start);
        entry.setTag(isize::try_from(offset).unwrap_or(0));
        menu.addItem(&entry);
    }
    drop(text);
    let cursor = view.selectedRange().location;
    let at = super::hover::rect_of(view, cursor..cursor + 1).map_or(NSPoint::new(0.0, 0.0), |r| {
        NSPoint::new(r.origin.x, r.origin.y + r.size.height)
    });
    menu.popUpMenuPositioningItem_atLocation_inView(None, at, Some(view));
}

fn flatten<'a>(symbols: &'a [DocumentSymbol], depth: usize, out: &mut Vec<(usize, &'a DocumentSymbol)>) {
    for symbol in symbols {
        out.push((depth, symbol));
        if let Some(children) = &symbol.children {
            flatten(children, depth + 1, out);
        }
    }
}

/// A short word for the kind of symbol, which code does not translate.
fn kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR => "ƒ",
        SymbolKind::STRUCT | SymbolKind::CLASS | SymbolKind::INTERFACE | SymbolKind::ENUM => "◆",
        SymbolKind::MODULE | SymbolKind::NAMESPACE | SymbolKind::PACKAGE => "▢",
        SymbolKind::FIELD | SymbolKind::PROPERTY | SymbolKind::ENUM_MEMBER => "·",
        SymbolKind::CONSTANT | SymbolKind::VARIABLE => "•",
        _ => "‣",
    }
}

/// Selects UTF-16 offset `offset` of `view`, as a symbol's menu item asks.
pub fn jump_to(view: &EditorTextView, offset: usize) {
    let range = NSRange::new(offset.min(view.document().text().len_utf16()), 0);
    view.setSelectedRange(range);
    view.scrollRangeToVisible(range);
}

struct Entry {
    path: PathBuf,
    range: LspRange,
    title: String,
}

pub struct SheetIvars {
    controller: Weak<WindowController>,
    entries: RefCell<Vec<Entry>>,
    sheet: RefCell<Option<Retained<NSPanel>>>,
    table: RefCell<Option<Retained<NSTableView>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `LocationSheet` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDELocationSheet"]
    #[ivars = SheetIvars]
    pub struct LocationSheet;

    unsafe impl NSObjectProtocol for LocationSheet {}

    unsafe impl NSControlTextEditingDelegate for LocationSheet {}

    unsafe impl NSTableViewDataSource for LocationSheet {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn rows(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().entries.borrow().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for LocationSheet {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, _column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let entries = self.ivars().entries.borrow();
                let entry = entries.get(usize::try_from(row).ok()?)?;
                let field = label(self.mtm(), &entry.title);
                field.setFont(Some(&crate::editor::code_font(12.0)));
                field.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
                Some(field.into_super().into_super())
            })
        }
    }

    impl LocationSheet {
        #[unsafe(method(ideOpenLocation:))]
        fn open_location(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.open_selected());
        }

        #[unsafe(method(ideCloseLocations:))]
        fn close_locations(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.close());
        }
    }
);

thread_local! {
    static SHEET: RefCell<Option<Retained<LocationSheet>>> = const { RefCell::new(None) };
}

/// The line `range` starts on, from the open editor of `path` or else from the file, read once per file.
fn line_text(path: &Path, line: u32, files: &mut HashMap<PathBuf, Option<String>>) -> String {
    let text = files.entry(path.to_path_buf()).or_insert_with(|| {
        editor::all_editors()
            .into_iter()
            .find(|v| v.document().path() == Some(path))
            .map(|v| v.document().text().to_string())
            .or_else(|| std::fs::read_to_string(path).ok())
    });
    text.as_deref()
        .and_then(|t| t.lines().nth(usize::try_from(line).unwrap_or(usize::MAX)))
        .map(|l| l.trim().chars().take(140).collect())
        .unwrap_or_default()
}

/// Lists `locations` in a sheet of `view`'s window; Return or a double click goes to one.
fn list(view: &EditorTextView, title: &str, mut locations: Vec<Location>) {
    let (Some(controller), Some(window)) = (view.controller(), view.window()) else {
        return;
    };
    locations.sort_by(|a, b| {
        (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(
            b.uri.as_str(),
            b.range.start.line,
            b.range.start.character,
        ))
    });
    let root = controller.project_root().unwrap_or_default();
    let mut files = HashMap::new();
    let entries: Vec<Entry> = locations
        .into_iter()
        .filter_map(|location| {
            let path = ide_lsp::uri::to_path(&location.uri)?;
            let shown = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
            let line = location.range.start.line;
            let title = format!("{shown}:{}   {}", line + 1, line_text(&path, line, &mut files));
            Some(Entry {
                path,
                range: location.range,
                title,
            })
        })
        .collect();
    let mtm = view.mtm();
    let sheet = LocationSheet::alloc(mtm).set_ivars(SheetIvars {
        controller: Weak::from(&*controller),
        entries: RefCell::new(entries),
        sheet: RefCell::new(None),
        table: RefCell::new(None),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let sheet: Retained<LocationSheet> = unsafe { msg_send![super(sheet), init] };
    let panel = sheet.build(mtm, title);
    SHEET.with(|cell| *cell.borrow_mut() = Some(sheet.clone()));
    window.beginSheet_completionHandler(&panel, None);
    if let Some(table) = sheet.ivars().table.borrow().as_ref() {
        panel.makeFirstResponder(Some(table));
    }
}

impl LocationSheet {
    fn build(&self, mtm: MainThreadMarker, title: &str) -> Retained<NSPanel> {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(640.0, 360.0)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        );
        // SAFETY: Switching it off keeps AppKit from releasing the panel behind Rust's back; the ivars own it.
        unsafe { panel.setReleasedWhenClosed(false) };
        let heading = label(mtm, title);
        heading.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        let table = NSTableView::new(mtm);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("location"));
        column.setWidth(600.0);
        table.addTableColumn(&column);
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::Inset);
        table.setRowHeight(22.0);
        // SAFETY: The table holds its data source, delegate and target weakly; the thread-local keeps the sheet
        // alive, and the double action is a method of this object that takes the sender.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
            table.setTarget(Some(self));
            table.setDoubleAction(Some(sel!(ideOpenLocation:)));
        }
        table.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(0), false);
        let scroll = NSScrollView::new(mtm);
        scroll.setDocumentView(Some(&table));
        scroll.setHasVerticalScroller(true);
        // SAFETY: A nil target until `set_action` below checks that this object answers the action.
        let open = unsafe { NSButton::buttonWithTitle_target_action(&ns(t("Öffnen")), None, None, mtm) };
        set_action(&open, self, sel!(ideOpenLocation:));
        open.setKeyEquivalent(&ns("\r"));
        // SAFETY: As above.
        let close = unsafe { NSButton::buttonWithTitle_target_action(&ns(t("Schließen")), None, None, mtm) };
        set_action(&close, self, sel!(ideCloseLocations:));
        close.setKeyEquivalent(&ns("\u{1b}"));
        let Some(content) = panel.contentView() else {
            return panel;
        };
        for view in [&*heading as &NSView, &scroll, &open, &close] {
            content.addSubview(view);
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        for constraint in [
            heading
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&content.leadingAnchor(), 16.0),
            heading
                .topAnchor()
                .constraintEqualToAnchor_constant(&content.topAnchor(), 14.0),
            scroll
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&content.leadingAnchor(), 12.0),
            scroll
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&content.trailingAnchor(), -12.0),
            scroll
                .topAnchor()
                .constraintEqualToAnchor_constant(&heading.bottomAnchor(), 10.0),
            scroll
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&open.topAnchor(), -12.0),
            open.trailingAnchor()
                .constraintEqualToAnchor_constant(&content.trailingAnchor(), -16.0),
            open.bottomAnchor()
                .constraintEqualToAnchor_constant(&content.bottomAnchor(), -14.0),
            close
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&open.leadingAnchor(), -10.0),
            close.centerYAnchor().constraintEqualToAnchor(&open.centerYAnchor()),
        ] {
            constraint.setActive(true);
        }
        *self.ivars().sheet.borrow_mut() = Some(panel.clone());
        *self.ivars().table.borrow_mut() = Some(table);
        panel
    }

    fn open_selected(&self) {
        let row = self.ivars().table.borrow().as_ref().map_or(-1, |t| t.selectedRow());
        let target = usize::try_from(row)
            .ok()
            .and_then(|i| self.ivars().entries.borrow().get(i).map(|e| (e.path.clone(), e.range)));
        self.close();
        if let (Some((path, range)), Some(controller)) = (target, self.ivars().controller.load()) {
            open_at(&controller, &path, range);
        }
    }

    fn close(&self) {
        if let Some(sheet) = self.ivars().sheet.borrow().as_ref()
            && let Some(parent) = sheet.sheetParent()
        {
            parent.endSheet(sheet);
        }
        SHEET.with(|cell| cell.borrow_mut().take());
    }
}

/// The titles the open location sheet lists, for the self-test.
#[cfg(feature = "selftest")]
pub fn listed() -> Vec<String> {
    SHEET.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|s| s.ivars().entries.borrow().iter().map(|e| e.title.clone()).collect())
            .unwrap_or_default()
    })
}

/// Closes the open location sheet, for the self-test.
#[cfg(feature = "selftest")]
pub fn close_list() {
    let sheet = SHEET.with(|cell| cell.borrow().clone());
    if let Some(sheet) = sheet {
        sheet.close();
    }
}
