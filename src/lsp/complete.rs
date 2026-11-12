//! Completion: a list below the cursor that typing filters, while the editor keeps the keyboard. Return or Tab
//! inserts the chosen item with its extra edits, such as an import, as one undoable step; a snippet's tab stops are
//! then visited with Tab.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;

use ide_document::Change;
use ide_lsp::lsp_types::{CompletionContext, CompletionItem, CompletionTriggerKind};
use ide_lsp::{Completions, Ticket};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{NSObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSControlTextEditingDelegate, NSFont, NSLineBreakMode, NSPanel, NSScrollView,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTableViewStyle, NSTextView, NSView,
    NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{NSIndexSet, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::{ViewState, server_of, text};
use crate::editor::{self, EditorTextView};
use crate::util::{label, main_soon, ns};

const WIDTH: f64 = 440.0;
const ROW: f64 = 20.0;
const VISIBLE_ROWS: f64 = 9.0;
const MOST: usize = 200;

/// The tab stops of an inserted snippet still to visit, in order; the last is where the cursor ends.
pub struct Session {
    stops: Vec<Range<usize>>,
}

pub struct ListIvars {
    panel: RefCell<Option<Retained<NSPanel>>>,
    table: RefCell<Option<Retained<NSTableView>>>,
    view: RefCell<Weak<EditorTextView>>,
    items: RefCell<Vec<CompletionItem>>,
    /// The items shown, as indices into `items`, best first.
    shown: RefCell<Vec<usize>>,
    /// Where the word being completed starts, in UTF-16 code units.
    start: Cell<usize>,
    /// The server said that typing further needs a new list.
    incomplete: Cell<bool>,
    /// Counts requests, so that an answer to an older one is dropped.
    asked: Cell<u64>,
    resolved: RefCell<HashMap<usize, CompletionItem>>,
    resolving: RefCell<Option<Ticket>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `CompletionList` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECompletionList"]
    #[ivars = ListIvars]
    pub struct CompletionList;

    unsafe impl NSObjectProtocol for CompletionList {}

    unsafe impl NSControlTextEditingDelegate for CompletionList {}

    unsafe impl NSTableViewDataSource for CompletionList {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn rows(&self, _table: &NSTableView) -> isize {
            crate::util::guarded(0, || isize::try_from(self.ivars().shown.borrow().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for CompletionList {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(
            &self,
            _table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: isize,
        ) -> Option<Retained<NSView>> {
            crate::util::guarded(None, || {
                let index = *self.ivars().shown.borrow().get(usize::try_from(row).ok()?)?;
                let items = self.ivars().items.borrow();
                Some(row_view(self.mtm(), items.get(index)?))
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &NSNotification) {
            crate::util::guarded((), || self.resolve_selected());
        }
    }
);

/// The completion list on view.
#[derive(Default)]
pub struct Completion {
    list: RefCell<Option<Retained<CompletionList>>>,
}

fn with_list<R>(f: impl FnOnce(&RefCell<Option<Retained<CompletionList>>>) -> R) -> R {
    f(&crate::context::app().completion.list)
}

fn list(mtm: MainThreadMarker) -> Retained<CompletionList> {
    with_list(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                let ivars = ListIvars {
                    panel: RefCell::new(None),
                    table: RefCell::new(None),
                    view: RefCell::new(Weak::default()),
                    items: RefCell::new(vec![]),
                    shown: RefCell::new(vec![]),
                    start: Cell::new(0),
                    incomplete: Cell::new(false),
                    asked: Cell::new(0),
                    resolved: RefCell::new(HashMap::new()),
                    resolving: RefCell::new(None),
                };
                // SAFETY: NSObject's initializer, called once on the newly allocated object.
                unsafe { msg_send![super(CompletionList::alloc(mtm).set_ivars(ivars)), init] }
            })
            .clone()
    })
}

/// The list, if it is shown for `view`.
fn open_for(view: &EditorTextView) -> Option<Retained<CompletionList>> {
    let list = with_list(|cell| cell.borrow().clone())?;
    let showing = list
        .ivars()
        .view
        .borrow()
        .load()
        .is_some_and(|v| std::ptr::eq(&*v, view))
        && list.ivars().panel.borrow().as_ref().is_some_and(|p| p.isVisible());
    showing.then_some(list)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Where the word before UTF-16 offset `at` starts.
fn word_start(view: &EditorTextView, at: usize) -> usize {
    let string = view.string();
    let mut start = at;
    while start > 0 {
        let unit = string.characterAtIndex(start - 1);
        if !char::from_u32(u32::from(unit)).is_some_and(is_word) {
            break;
        }
        start -= 1;
    }
    start
}

/// Asks the server for completions at the cursor, as the menu command and trigger characters do.
pub fn complete(view: &EditorTextView) {
    request(view, None);
}

fn request(view: &EditorTextView, trigger: Option<String>) {
    let Some((server, uri)) = server_of(view) else { return };
    let cursor = view.selectedRange().location;
    let Some(position) = text::position(view.document().text(), cursor) else {
        return;
    };
    let shared = list(view.mtm());
    let asked = shared.ivars().asked.get() + 1;
    shared.ivars().asked.set(asked);
    let serial = view.ivars().lsp.borrow().serial;
    let context = CompletionContext {
        trigger_kind: if trigger.is_some() {
            CompletionTriggerKind::TRIGGER_CHARACTER
        } else {
            CompletionTriggerKind::INVOKED
        },
        trigger_character: trigger,
    };
    let _ = server.completion(&uri, position, Some(context), move |answer| {
        let Ok(completions) = answer else { return };
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            let list = list(view.mtm());
            if list.ivars().asked.get() == asked {
                list.show(&view, completions);
            }
        });
    });
}

/// The user typed `typed` into `view`: the list filters or closes, a trigger character asks anew.
pub fn typed(view: &EditorTextView, typed: &str) {
    if let Some(list) = open_for(view) {
        if typed.chars().all(is_word) {
            if list.ivars().incomplete.get() {
                request(view, None);
            } else {
                list.refilter();
            }
            return;
        }
        list.close();
    }
    let Some((server, _)) = server_of(view) else { return };
    let triggers = server
        .capabilities()
        .and_then(|c| c.completion_provider)
        .and_then(|p| p.trigger_characters)
        .unwrap_or_default();
    let cursor = view.selectedRange().location;
    let from = cursor.saturating_sub(2);
    let before = view
        .string()
        .substringWithRange(objc2_foundation::NSRange::new(from, cursor - from))
        .to_string();
    if let Some(trigger) = triggers.iter().find(|t| before.ends_with(t.as_str())) {
        request(view, Some(trigger.clone()));
    }
}

/// A command of the keyboard reached `view`; whether the list or a snippet took it.
pub fn command(view: &EditorTextView, command: Sel) -> bool {
    if let Some(list) = open_for(view) {
        let step = |by: isize| list.step(by);
        match command {
            c if c == sel!(moveDown:) => step(1),
            c if c == sel!(moveUp:) => step(-1),
            c if c == sel!(pageDown:) || c == sel!(scrollPageDown:) => step(8),
            c if c == sel!(pageUp:) || c == sel!(scrollPageUp:) => step(-8),
            c if c == sel!(insertNewline:) || c == sel!(insertTab:) => list.accept(),
            c if c == sel!(cancelOperation:) => list.close(),
            c if c == sel!(deleteBackward:) => return false,
            _ => {
                list.close();
                return false;
            }
        }
        return true;
    }
    if command == sel!(insertTab:) {
        return next_stop(view);
    }
    if command == sel!(cancelOperation:) {
        return view.ivars().lsp.borrow_mut().snippet.take().is_some();
    }
    false
}

/// After a command the editor ran itself, such as deleting backwards: the list follows or closes.
pub fn after_command(view: &EditorTextView, command: Sel) {
    if let Some(list) = open_for(view)
        && command == sel!(deleteBackward:)
    {
        if view.selectedRange().location < list.ivars().start.get() {
            list.close();
        } else {
            list.refilter();
        }
    }
}

/// Closes the list if it belongs to `view`.
pub fn dismiss_for(view: &EditorTextView) {
    if let Some(list) = open_for(view) {
        list.close();
    }
}

/// Keeps the tab stops of a snippet on their text while it is edited.
pub fn shift(state: &mut ViewState, change: &Change) {
    if let Some(session) = &mut state.snippet {
        for stop in &mut session.stops {
            super::decorate::follow(stop, change);
        }
    }
}

/// Selects the next tab stop of the snippet being filled in; whether there was one.
fn next_stop(view: &EditorTextView) -> bool {
    let stop = {
        let mut state = view.ivars().lsp.borrow_mut();
        let Some(session) = &mut state.snippet else {
            return false;
        };
        if session.stops.is_empty() {
            state.snippet = None;
            return false;
        }
        let stop = session.stops.remove(0);
        if session.stops.is_empty() {
            state.snippet = None;
        }
        stop
    };
    view.setSelectedRange(objc2_foundation::NSRange::new(stop.start, stop.end - stop.start));
    true
}

impl CompletionList {
    fn view(&self) -> Option<Retained<EditorTextView>> {
        self.ivars().view.borrow().load()
    }

    fn show(&self, view: &EditorTextView, completions: Completions) {
        let cursor = view.selectedRange().location;
        let start = completions
            .items
            .iter()
            .find_map(|item| ide_lsp::insertion(item).insert)
            .map(|range| text::offset(view.document().text(), range.start))
            .filter(|start| *start <= cursor)
            .unwrap_or_else(|| word_start(view, cursor));
        *self.ivars().view.borrow_mut() = Weak::from(view);
        *self.ivars().items.borrow_mut() = completions.items;
        self.ivars().resolved.borrow_mut().clear();
        self.ivars().start.set(start);
        self.ivars().incomplete.set(completions.incomplete);
        if self.ivars().panel.borrow().is_none() {
            self.build(view.mtm());
        }
        self.refilter();
    }

    /// Shows the items that match what was typed since the start of the word.
    fn refilter(&self) {
        let Some(view) = self.view() else { return };
        let start = self.ivars().start.get();
        let cursor = view.selectedRange().location;
        if cursor < start {
            self.close();
            return;
        }
        let typed = view
            .string()
            .substringWithRange(objc2_foundation::NSRange::new(start, cursor - start))
            .to_string()
            .to_lowercase();
        let items = self.ivars().items.borrow();
        let mut ranked: Vec<(u8, &str, usize)> = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let key = item.filter_text.as_deref().unwrap_or(&item.label).to_lowercase();
                let rank = if key.starts_with(&typed) {
                    0
                } else if subsequence(&typed, &key) {
                    1
                } else {
                    return None;
                };
                Some((rank, item.sort_text.as_deref().unwrap_or(&item.label), index))
            })
            .collect();
        ranked.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let shown: Vec<usize> = ranked.into_iter().take(MOST).map(|(_, _, index)| index).collect();
        drop(items);
        let empty = shown.is_empty();
        *self.ivars().shown.borrow_mut() = shown;
        if empty {
            self.close();
            return;
        }
        if let Some(table) = self.ivars().table.borrow().as_ref() {
            table.reloadData();
            table.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(0), false);
            table.scrollRowToVisible(0);
        }
        self.place(&view);
    }

    fn build(&self, mtm: MainThreadMarker) {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, ROW * VISIBLE_ROWS)),
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );
        // SAFETY: Switching it off keeps AppKit from releasing the panel behind Rust's back; the ivars own it.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setHasShadow(true);
        panel.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
        let table = NSTableView::new(mtm);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("item"));
        column.setWidth(WIDTH);
        table.addTableColumn(&column);
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::Plain);
        table.setRowHeight(ROW);
        table.setIntercellSpacing(NSSize::new(0.0, 0.0));
        table.setRefusesFirstResponder(true);
        // SAFETY: The table holds its data source and delegate weakly; the thread-local keeps this list alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        let scroll = NSScrollView::new(mtm);
        scroll.setDocumentView(Some(&table));
        scroll.setHasVerticalScroller(true);
        scroll.setDrawsBackground(false);
        if let Some(content) = panel.contentView() {
            content.addSubview(&scroll);
            crate::util::pin_edges(&scroll, &content);
        }
        *self.ivars().panel.borrow_mut() = Some(panel);
        *self.ivars().table.borrow_mut() = Some(table);
    }

    /// Puts the panel below the start of the word, or above it where the screen ends.
    fn place(&self, view: &EditorTextView) {
        let Some(panel) = self.ivars().panel.borrow().clone() else {
            return;
        };
        let Some(window) = view.window() else { return };
        let start = self.ivars().start.get();
        let rows = f64::from(u32::try_from(self.ivars().shown.borrow().len()).unwrap_or(9)).min(VISIBLE_ROWS);
        let height = rows * ROW + 4.0;
        let text_view: &NSTextView = view;
        // SAFETY: A range within the text; no actual range is asked for.
        let caret = unsafe {
            objc2_app_kit::NSTextInputClient::firstRectForCharacterRange_actualRange(
                text_view,
                objc2_foundation::NSRange::new(start, 0),
                std::ptr::null_mut(),
            )
        };
        let mut origin = NSPoint::new(caret.origin.x - 4.0, caret.origin.y - height - 2.0);
        if let Some(screen) = window.screen()
            && origin.y < screen.visibleFrame().origin.y
        {
            origin.y = caret.origin.y + caret.size.height + 2.0;
        }
        panel.setFrame_display(NSRect::new(origin, NSSize::new(WIDTH, height)), true);
        if panel.parentWindow().is_none() {
            // SAFETY: The window keeps a weak reference to the panel, which the thread-local owns and `close` detaches.
            unsafe { window.addChildWindow_ordered(&panel, NSWindowOrderingMode::Above) };
        }
        panel.orderFront(None);
    }

    fn step(&self, by: isize) {
        let Some(table) = self.ivars().table.borrow().clone() else {
            return;
        };
        let count = isize::try_from(self.ivars().shown.borrow().len()).unwrap_or(0);
        let row = (table.selectedRow() + by).clamp(0, (count - 1).max(0));
        table.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row.cast_unsigned()), false);
        table.scrollRowToVisible(row);
    }

    fn selected(&self) -> Option<usize> {
        let row = self.ivars().table.borrow().as_ref()?.selectedRow();
        self.ivars().shown.borrow().get(usize::try_from(row).ok()?).copied()
    }

    /// Asks the server for the details of the selected item, such as the import it needs.
    fn resolve_selected(&self) {
        let (Some(index), Some(view)) = (self.selected(), self.view()) else {
            return;
        };
        if self.ivars().resolved.borrow().contains_key(&index) {
            return;
        }
        let Some((server, uri)) = server_of(&view) else { return };
        let Some(item) = self.ivars().items.borrow().get(index).cloned() else {
            return;
        };
        if let Some(previous) = self.ivars().resolving.borrow_mut().take() {
            previous.cancel();
        }
        let asked = self.ivars().asked.get();
        let ticket = server.resolve_completion(&uri, item, move |answer| {
            let Ok(item) = answer else { return };
            main_soon(move || {
                let Some(mtm) = MainThreadMarker::new() else { return };
                let list = list(mtm);
                if list.ivars().asked.get() == asked {
                    list.ivars().resolved.borrow_mut().insert(index, item);
                }
            });
        });
        *self.ivars().resolving.borrow_mut() = ticket.ok();
    }

    fn accept(&self) {
        let (Some(index), Some(view)) = (self.selected(), self.view()) else {
            self.close();
            return;
        };
        let item = self
            .ivars()
            .resolved
            .borrow()
            .get(&index)
            .cloned()
            .or_else(|| self.ivars().items.borrow().get(index).cloned());
        let start = self.ivars().start.get();
        self.close();
        if let Some(item) = item {
            insert(&view, &item, start);
        }
    }

    pub fn close(&self) {
        if let Some(panel) = self.ivars().panel.borrow().as_ref() {
            if let Some(parent) = panel.parentWindow() {
                parent.removeChildWindow(panel);
            }
            panel.orderOut(None);
        }
        if let Some(ticket) = self.ivars().resolving.borrow_mut().take() {
            ticket.cancel();
        }
        self.ivars().asked.set(self.ivars().asked.get() + 1);
    }
}

/// Whether the letters of `needle` appear in `haystack` in order.
fn subsequence(needle: &str, haystack: &str) -> bool {
    let mut letters = haystack.chars();
    needle.chars().all(|c| letters.any(|h| h == c))
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Inserts `item` over the word from `start` to the cursor, with its extra edits, as one undoable step, and selects
/// the snippet's first tab stop.
fn insert(view: &EditorTextView, item: &CompletionItem, start: usize) {
    let insertion = ide_lsp::insertion(item);
    let cursor = view.selectedRange().location;
    let main_start = insertion
        .insert
        .map(|range| text::offset(view.document().text(), range.start))
        .unwrap_or(start)
        .min(cursor);
    let mut edits = vec![(main_start..cursor, insertion.snippet.text.clone())];
    let extra = super::edits::ranges(view, item.additional_text_edits.as_deref().unwrap_or_default());
    let shift: isize = extra
        .iter()
        .filter(|(range, _)| range.end <= main_start)
        .map(|(range, text)| isize::try_from(utf16_len(text)).unwrap_or(0) - isize::try_from(range.len()).unwrap_or(0))
        .sum();
    edits.extend(
        extra
            .into_iter()
            .filter(|(range, _)| range.end <= main_start || range.start >= cursor),
    );
    super::edits::apply_text_edits(view, edits);
    let base = main_start.saturating_add_signed(shift);
    let base = u32::try_from(base).unwrap_or(u32::MAX);
    let mut stops: Vec<Range<usize>> = insertion
        .snippet
        .tabstops
        .iter()
        .filter_map(|stop| stop.spans.first())
        .map(|span| usize::try_from(base + span.start).unwrap_or(0)..usize::try_from(base + span.end).unwrap_or(0))
        .collect();
    if stops.is_empty() {
        return;
    }
    let first = stops.remove(0);
    view.setSelectedRange(objc2_foundation::NSRange::new(first.start, first.end - first.start));
    view.ivars().lsp.borrow_mut().snippet = (!stops.is_empty()).then_some(Session { stops });
}

fn row_view(mtm: MainThreadMarker, item: &CompletionItem) -> Retained<NSView> {
    let name = label(mtm, &item.label);
    name.setFont(Some(&crate::editor::code_font(12.0)));
    name.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let detail_text = item
        .label_details
        .as_ref()
        .and_then(|d| d.description.clone().or_else(|| d.detail.clone()))
        .or_else(|| item.detail.clone())
        .unwrap_or_default();
    let detail = label(mtm, detail_text.lines().next().unwrap_or_default());
    detail.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
    detail.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let cell = NSView::new(mtm);
    for field in [&name, &detail] {
        cell.addSubview(field);
        field.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    detail.setContentCompressionResistancePriority_forOrientation(
        250.0,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    for constraint in [
        name.leadingAnchor()
            .constraintEqualToAnchor_constant(&cell.leadingAnchor(), 8.0),
        name.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        detail
            .leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&name.trailingAnchor(), 12.0),
        detail
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -8.0),
        detail.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    cell
}

/// What the self-test reads from and does with the completion list.
#[cfg(feature = "selftest")]
pub mod selftest {
    use objc2::DefinedClass;

    use crate::editor::EditorTextView;

    /// The labels the list shows for `view`, best first; empty when it is closed.
    pub fn shown(view: &EditorTextView) -> Vec<String> {
        let Some(list) = super::open_for(view) else {
            return vec![];
        };
        let items = list.ivars().items.borrow();
        list.ivars()
            .shown
            .borrow()
            .iter()
            .filter_map(|i| items.get(*i).map(|item| item.label.clone()))
            .collect()
    }

    /// Selects the first item whose label starts with `label` and accepts it as Return does.
    pub fn accept(view: &EditorTextView, label: &str) -> bool {
        let Some(list) = super::open_for(view) else {
            return false;
        };
        let row = {
            let items = list.ivars().items.borrow();
            list.ivars().shown.borrow().iter().position(|i| {
                items
                    .get(*i)
                    .is_some_and(|item| item.label.trim_start().starts_with(label))
            })
        };
        let Some(row) = row else { return false };
        if let Some(table) = list.ivars().table.borrow().as_ref() {
            table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
        super::command(view, objc2::sel!(insertNewline:))
    }

    /// The completion panel, for a picture.
    pub fn panel() -> Option<objc2::rc::Retained<objc2_app_kit::NSPanel>> {
        super::with_list(|cell| cell.borrow().as_ref().and_then(|l| l.ivars().panel.borrow().clone()))
    }
}
