//! The timeline: one table row per entry of the thread's own lane.
//!
//! Only rows in view get a card (E25). A row's height is the measured height of its card once it was built at the
//! current width, else an estimate from the entry; a card that turns out taller or shorter than the table thought
//! corrects its row afterwards. A plain stack of cards would build all of them; `usesAutomaticRowHeights` would
//! measure text views before they know their width.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use ide_thread::{Change, Entry, EntryId, EntryKind, Lane, Thread};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAnimationContext, NSEvent, NSScrollView, NSTableCellView, NSTableColumn, NSTableRowView,
    NSTableView, NSTableViewSelectionHighlightStyle, NSTableViewStyle, NSView,
};
use objc2_foundation::{NSIndexSet, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::cards::{self, Ctx, Inputs, Live, Part};
use super::row::TimelineRow;
use super::target::AgentTarget;
use crate::util::{guarded, main_soon, ns};

/// Room on the leading side for the line and its dots.
const LEADING: f64 = 30.0;
/// The leading room of a user message, which stands apart from the line.
const USER_LEADING: f64 = 8.0;
const TRAILING: f64 = 12.0;
const TOP: f64 = 3.0;
const BOTTOM: f64 = 10.0;

pub struct TableIvars {
    key: u64,
}

define_class!(
    // SAFETY: NSTableView has no subclassing requirements and `TimelineTable` does not implement Drop.
    #[unsafe(super(NSTableView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentTimelineTable"]
    #[ivars = TableIvars]
    pub struct TimelineTable;

    unsafe impl NSObjectProtocol for TimelineTable {}

    impl TimelineTable {
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            let key = self.ivars().key;
            if !self.inLiveResize() {
                main_soon(move || {
                    if let Some(tab) = super::get(key) {
                        tab.timeline().width_changed();
                    }
                });
            }
        }

        #[unsafe(method(viewDidEndLiveResize))]
        fn did_end_live_resize(&self) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let _: () = unsafe { msg_send![super(self), viewDidEndLiveResize] };
            guarded((), || {
                if let Some(tab) = super::get(self.ivars().key) {
                    tab.timeline().width_changed();
                }
            });
        }

        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(tab) = super::get(self.ivars().key) {
                    tab.stop_turn();
                }
            });
        }

        // The plan in the selected row comes next for Tab: its text is selectable, so the keyboard marks passages
        // of it to comment on; the views in a table's rows are not in the window's key view loop.
        #[unsafe(method_id(nextValidKeyView))]
        fn next_valid_key_view(&self) -> Option<Retained<NSView>> {
            let plan = guarded(None, || {
                let tab = super::get(self.ivars().key)?;
                let text = tab.timeline().plan_text_of_row(self.selectedRow())?;
                let view: Retained<NSView> = Retained::into_super(Retained::into_super(text));
                (view.window().is_some() && !view.isHiddenOrHasHiddenAncestor()).then_some(view)
            });
            plan.or_else(|| self.following_key_view())
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let handled = guarded(false, || {
                let characters = event.charactersIgnoringModifiers().map(|s| s.to_string()).unwrap_or_default();
                if characters != " " && characters != "\r" {
                    return false;
                }
                let row = self.selectedRow();
                let tab = super::get(self.ivars().key);
                tab.is_some_and(|tab| tab.timeline().toggle_row(row))
            });
            if !handled {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl TimelineTable {
    /// The view Tab goes to after the timeline and the plan in it.
    pub fn following_key_view(&self) -> Option<Retained<NSView>> {
        // SAFETY: The superclass's implementation, which takes nothing and returns a view or nil.
        unsafe { msg_send![super(self), nextValidKeyView] }
    }
}

pub struct Timeline {
    mtm: MainThreadMarker,
    key: u64,
    scroll: Retained<NSScrollView>,
    table: Retained<TimelineTable>,
    target: Retained<AgentTarget>,
    cwd: PathBuf,
    thread: RefCell<Thread>,
    rows: RefCell<Vec<EntryId>>,
    /// The entries of the rows, in their order.
    list: RefCell<Vec<std::sync::Arc<Entry>>>,
    /// The measured height of each entry's row, with the width it was measured at.
    heights: RefCell<HashMap<u64, (f64, f64)>>,
    /// The width the rows were last laid out at.
    width: Cell<f64>,
    expanded: RefCell<HashSet<(u64, Part)>>,
    inputs: RefCell<Inputs>,
    live: RefCell<Vec<Live>>,
    /// Rows whose card turned out another height than the table had, with that height.
    corrections: RefCell<BTreeMap<usize, f64>>,
    built: Cell<usize>,
    /// Only the conversation shows: no tools, thinking or status lines.
    focus: Cell<bool>,
}

impl Timeline {
    pub fn new(mtm: MainThreadMarker, key: u64, target: &AgentTarget, thread: Thread, cwd: PathBuf) -> Timeline {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
        let this = TimelineTable::alloc(mtm).set_ivars(TableIvars { key });
        // SAFETY: NSTableView's initializer, called once on the newly allocated object.
        let table: Retained<TimelineTable> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("entry"));
        column.setResizingMask(objc2_app_kit::NSTableColumnResizingOptions::AutoresizingMask);
        table.addTableColumn(&column);
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::Plain);
        table.setIntercellSpacing(NSSize::new(0.0, 0.0));
        table.setBackgroundColor(&objc2_app_kit::NSColor::clearColor());
        table.setSelectionHighlightStyle(NSTableViewSelectionHighlightStyle::Regular);
        table.setAllowsEmptySelection(true);
        table.setRowHeight(40.0);
        table.setColumnAutoresizingStyle(
            objc2_app_kit::NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle,
        );
        table.setAccessibilityLabel(Some(&ns(t("Verlauf der Sitzung"))));
        // SAFETY: The table holds its data source and delegate weakly; the tab keeps the target alive as long as the
        // table.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(target)));
            table.setDelegate(Some(ProtocolObject::from_ref(target)));
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&table));
        scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
        table.sizeLastColumnToFit();
        let list = shown(&thread, false);
        let rows = list.iter().map(|e| e.id).collect();
        Timeline {
            mtm,
            key,
            scroll,
            table,
            target: target.retain(),
            cwd,
            thread: RefCell::new(thread),
            rows: RefCell::new(rows),
            list: RefCell::new(list),
            heights: RefCell::new(HashMap::new()),
            width: Cell::new(0.0),
            expanded: RefCell::new(HashSet::new()),
            inputs: RefCell::new(Inputs::default()),
            live: RefCell::new(Vec::new()),
            corrections: RefCell::new(BTreeMap::new()),
            built: Cell::new(0),
            focus: Cell::new(false),
        }
    }

    pub fn view(&self) -> &NSScrollView {
        &self.scroll
    }

    #[cfg(feature = "selftest")]
    pub fn table(&self) -> &NSTableView {
        &self.table
    }

    pub fn inputs(&self) -> &RefCell<Inputs> {
        &self.inputs
    }

    pub fn count(&self) -> usize {
        self.rows.borrow().len()
    }

    /// The width the table has now.
    fn table_width(&self) -> f64 {
        self.scroll.contentSize().width.max(200.0)
    }

    fn entry(&self, row: usize) -> Option<std::sync::Arc<Entry>> {
        self.list.borrow().get(row).cloned()
    }

    fn leading(entry: &Entry) -> f64 {
        if matches!(entry.kind, EntryKind::UserMessage(_)) {
            USER_LEADING
        } else {
            LEADING
        }
    }

    pub fn row_height(&self, row: usize) -> f64 {
        let Some(entry) = self.entry(row) else { return 20.0 };
        self.known_height(&entry)
    }

    fn known_height(&self, entry: &Entry) -> f64 {
        let width = self.table_width();
        if let Some((measured, height)) = self.heights.borrow().get(&entry.id.get())
            && (measured - width).abs() < 0.5
        {
            return *height;
        }
        cards::estimate(entry, width - Self::leading(entry) - TRAILING) + TOP + BOTTOM
    }

    pub fn row_view(&self, row: usize) -> Retained<NSTableRowView> {
        let mark = cards::mark(&self.list.borrow(), row);
        Retained::into_super(TimelineRow::new(self.mtm, mark))
    }

    /// The card of `row` in a cell, measured; its row is corrected afterwards when the table had another height.
    pub fn cell(&self, row: usize) -> Option<Retained<NSView>> {
        let entry = self.entry(row)?;
        let width = self.table_width();
        self.fit_column(width);
        let before = self.known_height(&entry);
        let leading = Self::leading(&entry);
        let card = {
            let thread = self.thread.borrow();
            let expanded = self.expanded.borrow();
            let ctx = Ctx {
                mtm: self.mtm,
                width: width - leading - TRAILING,
                thread: &thread,
                expanded: &expanded,
                target: &self.target,
                cwd: &self.cwd,
                inputs: &self.inputs,
                live: &self.live,
                nested: false,
                now: SystemTime::now(),
            };
            cards::build(&ctx, &entry)
        };
        self.built.set(self.built.get() + 1);
        let cell = NSTableCellView::new(self.mtm);
        cell.addSubview(&card);
        card.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            card.leadingAnchor()
                .constraintEqualToAnchor_constant(&cell.leadingAnchor(), leading),
            card.trailingAnchor()
                .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -TRAILING),
            card.topAnchor()
                .constraintEqualToAnchor_constant(&cell.topAnchor(), TOP),
        ] {
            constraint.setActive(true);
        }
        let bottom = card
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&cell.bottomAnchor(), -BOTTOM);
        // Below required, so a row the table still has at its estimated height stretches the card, not the rules.
        bottom.setPriority(999.0);
        bottom.setActive(true);
        let height = measure(&cell, width);
        self.heights.borrow_mut().insert(entry.id.get(), (width, height));
        if (height - before).abs() > 0.5 {
            self.correct(row, before);
        }
        Some(Retained::into_super(cell))
    }

    /// Tells the table, once it is done with what it does now, that `row` changed its height from `before`.
    fn correct(&self, row: usize, before: f64) {
        let first = self.corrections.borrow().is_empty();
        self.corrections.borrow_mut().entry(row).or_insert(before);
        if first {
            let key = self.key;
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.timeline().flush_corrections();
                }
            });
        }
    }

    fn flush_corrections(&self) {
        let corrections = std::mem::take(&mut *self.corrections.borrow_mut());
        if corrections.is_empty() {
            return;
        }
        let following = self.is_at_bottom();
        let visible = self.scroll.contentView().bounds();
        let mut shift = 0.0;
        let set = objc2_foundation::NSMutableIndexSet::new();
        for (row, before) in &corrections {
            if *row >= self.count() {
                continue;
            }
            let rect = self.table.rectOfRow(isize::try_from(*row).unwrap_or(0));
            if rect.origin.y + rect.size.height <= visible.origin.y + 1.0 {
                shift += self.row_height(*row) - before;
            }
            set.addIndex(*row);
        }
        NSAnimationContext::beginGrouping();
        NSAnimationContext::currentContext().setDuration(0.0);
        self.table.noteHeightOfRowsWithIndexesChanged(&set);
        NSAnimationContext::endGrouping();
        if following {
            self.scroll_to_end();
        } else if shift.abs() > 0.5 {
            let clip = self.scroll.contentView();
            let origin = clip.bounds().origin;
            clip.scrollToPoint(NSPoint::new(origin.x, (origin.y + shift).max(0.0)));
            self.scroll.reflectScrolledClipView(&clip);
        }
    }

    /// Takes in a new state of the thread and redraws the rows `changes` name.
    pub fn apply(&self, thread: Thread, changes: &[Change]) {
        let following = self.is_at_bottom() || self.count() == 0;
        let old: Vec<EntryId> = self.rows.borrow().clone();
        let list = shown(&thread, self.focus.get());
        let new: Vec<EntryId> = list.iter().map(|e| e.id).collect();
        let mut reload: HashSet<usize> = HashSet::new();
        let mut everything = false;
        for change in changes {
            match change {
                Change::Updated { lane: Lane::Main, id } => {
                    if let Some(i) = new.iter().position(|e| e == id) {
                        reload.insert(i);
                    }
                }
                Change::Updated {
                    lane: Lane::Subagent(key),
                    ..
                }
                | Change::Inserted {
                    lane: Lane::Subagent(key),
                    ..
                }
                | Change::Removed {
                    lane: Lane::Subagent(key),
                    ..
                }
                | Change::Subagent(key) => {
                    if let Some(i) = owner_row(&thread, &new, key) {
                        reload.insert(i);
                    }
                }
                Change::Cleared => everything = true,
                _ => {}
            }
        }
        // A hook that ran fine has no row of its own; the row that counts its turn's hooks changes with it.
        let hidden_changed = changes.iter().any(|c| match c {
            Change::Inserted {
                lane: Lane::Main, id, ..
            }
            | Change::Updated { lane: Lane::Main, id } => !new.contains(id),
            _ => false,
        });
        if hidden_changed {
            reload.extend(
                list.iter()
                    .enumerate()
                    .filter(|(_, e)| is_quiet_hook(e))
                    .map(|(i, _)| i),
            );
        }
        for id in old.iter().filter(|id| !new.contains(id)) {
            self.inputs.borrow_mut().forget(id.get());
        }
        *self.thread.borrow_mut() = thread;
        *self.rows.borrow_mut() = new.clone();
        *self.list.borrow_mut() = list;
        let appended = new.len() >= old.len() && new[..old.len()] == old[..];
        if everything || !appended {
            self.table.reloadData();
        } else {
            if new.len() > old.len() {
                let added = NSIndexSet::indexSetWithIndexesInRange(objc2_foundation::NSRange::new(
                    old.len(),
                    new.len() - old.len(),
                ));
                self.table
                    .insertRowsAtIndexes_withAnimation(&added, objc2_app_kit::NSTableViewAnimationOptions::empty());
            }
            self.reload_rows(reload.into_iter().filter(|i| *i < old.len()));
        }
        self.refresh_marks();
        if following {
            self.scroll_to_end();
        }
    }

    fn reload_rows(&self, rows: impl Iterator<Item = usize>) {
        let set = objc2_foundation::NSMutableIndexSet::new();
        for row in rows {
            set.addIndex(row);
        }
        if set.count() > 0 {
            self.table
                .reloadDataForRowIndexes_columnIndexes(&set, &NSIndexSet::indexSetWithIndex(0));
        }
    }

    /// Builds the card of entry `id` anew.
    pub fn reload_entry(&self, id: u64) {
        if let Some(row) = self.rows.borrow().iter().position(|e| e.get() == id) {
            self.reload_rows(std::iter::once(row));
        }
    }

    /// Gives the rows in view the marks of their place, which change as rows are added after them.
    fn refresh_marks(&self) {
        let entries = self.list.borrow();
        let range = self.table.rowsInRect(self.table.visibleRect());
        for row in range.location..range.location + range.length {
            let Some(view) = self
                .table
                .rowViewAtRow_makeIfNecessary(isize::try_from(row).unwrap_or(0), false)
            else {
                continue;
            };
            if let Ok(view) = view.downcast::<TimelineRow>() {
                view.set_mark(cards::mark(&entries, row));
            }
        }
    }

    /// Opens or closes `part` of entry `id`.
    pub fn toggle(&self, id: u64, part: Part) {
        {
            let mut expanded = self.expanded.borrow_mut();
            if !expanded.remove(&(id, part)) {
                expanded.insert((id, part));
            }
        }
        self.reload_entry(id);
    }

    /// The text of the plan in `row`, which the keyboard selects passages of.
    pub fn plan_text_of_row(&self, row: isize) -> Option<Retained<objc2_app_kit::NSTextView>> {
        let entry = usize::try_from(row).ok().and_then(|r| self.entry(r))?;
        self.inputs.borrow().plan_text.get(&entry.id.get()).cloned()
    }

    /// Opens or closes the main part of the card in `row`, as Space or Return on a selected row do.
    pub fn toggle_row(&self, row: isize) -> bool {
        let Some(entry) = usize::try_from(row).ok().and_then(|r| self.entry(r)) else {
            return false;
        };
        let part = match &entry.kind {
            EntryKind::Thought(_) => Part::Thought,
            EntryKind::ToolCall(call) if matches!(call.view, ide_thread::ToolView::Agent { .. }) => Part::Subagent,
            EntryKind::ToolCall(_) => Part::Output,
            EntryKind::PlanApproval(_) => Part::Plan,
            EntryKind::Notice(_) => Part::Detail,
            _ => return false,
        };
        self.toggle(entry.id.get(), part);
        true
    }

    pub fn focus_view(&self) -> bool {
        self.focus.get()
    }

    /// Shows only the conversation, or everything again.
    pub fn set_focus_view(&self, on: bool) {
        self.focus.set(on);
        let thread = self.thread.borrow().clone();
        let list = shown(&thread, on);
        *self.rows.borrow_mut() = list.iter().map(|e| e.id).collect();
        *self.list.borrow_mut() = list;
        self.table.reloadData();
        self.scroll_to_end();
    }

    /// Whether the view shows the end of the timeline.
    pub fn is_at_bottom(&self) -> bool {
        let clip = self.scroll.contentView().bounds();
        let height = self.table.frame().size.height;
        clip.origin.y + clip.size.height >= height - 40.0
    }

    pub fn scroll_to_end(&self) {
        let count = self.count();
        if count == 0 {
            return;
        }
        self.table.scrollRowToVisible(isize::try_from(count - 1).unwrap_or(0));
        let clip = self.scroll.contentView();
        let height = self.table.frame().size.height;
        let y = (height - clip.bounds().size.height).max(0.0);
        clip.scrollToPoint(NSPoint::new(0.0, y));
        self.scroll.reflectScrolledClipView(&clip);
    }

    /// Scrolls to entry `key` and selects its row.
    pub fn reveal(&self, key: &str) {
        let thread = self.thread.borrow().clone();
        let rows = self.rows.borrow().clone();
        let Some(row) = owner_row(&thread, &rows, key) else {
            return;
        };
        self.table.scrollRowToVisible(isize::try_from(row).unwrap_or(0));
        self.table
            .selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row), false);
    }

    /// Makes the one column as wide as the table; a table does not do that for a column it was given in code.
    fn fit_column(&self, width: f64) {
        let columns = self.table.tableColumns();
        if let Some(column) = columns.firstObject()
            && (column.width() - width).abs() > 0.5
        {
            column.setWidth(width);
        }
    }

    /// Builds the cards anew, as formulas and diagrams they show as code were drawn.
    pub fn figures_arrived(&self) {
        let following = self.is_at_bottom();
        self.table.reloadData();
        if following {
            self.scroll_to_end();
        }
    }

    /// Lays the cards out anew after the width changed.
    pub fn width_changed(&self) {
        let width = self.table_width();
        self.fit_column(width);
        if (width - self.width.get()).abs() < 1.0 || self.table.inLiveResize() {
            return;
        }
        let following = self.is_at_bottom();
        self.width.set(width);
        self.table.reloadData();
        if following {
            self.scroll_to_end();
        }
    }

    /// Advances the running times of the cards in view; whether any runs.
    pub fn tick(&self, now: SystemTime) -> bool {
        let mut live = self.live.borrow_mut();
        live.retain(|l| l.label.window().is_some());
        for l in live.iter() {
            l.label.setStringValue(&ns(&super::format::elapsed(
                now.duration_since(l.since).unwrap_or_default(),
            )));
        }
        !live.is_empty()
    }

    /// How many cards were built so far.
    #[cfg(feature = "selftest")]
    pub fn built(&self) -> usize {
        self.built.get()
    }

    /// The rows in view whose height differs from what their card needs, as `row: table height / card height`.
    #[cfg(feature = "selftest")]
    pub fn misfits(&self) -> Vec<String> {
        let range = self.table.rowsInRect(self.table.visibleRect());
        (range.location..range.location + range.length)
            .filter_map(|row| {
                let rect = self.table.rectOfRow(isize::try_from(row).ok()?);
                let view = self
                    .table
                    .viewAtColumn_row_makeIfNecessary(0, isize::try_from(row).ok()?, false)?;
                let card = view.subviews().firstObject()?.fittingSize().height + TOP + BOTTOM;
                ((rect.size.height - card).abs() > 1.5).then(|| format!("{row}: {} / {card}", rect.size.height))
            })
            .collect()
    }

    /// What the permission editors hold.
    #[cfg(feature = "selftest")]
    pub fn editor_texts(&self) -> Vec<String> {
        let inputs = self.inputs.borrow();
        inputs.editors.keys().filter_map(|id| inputs.editor_text(*id)).collect()
    }

    /// The cards in view, as their VoiceOver summaries.
    #[cfg(feature = "selftest")]
    pub fn visible_summaries(&self) -> Vec<String> {
        let range = self.table.rowsInRect(self.table.visibleRect());
        (range.location..range.location + range.length)
            .filter_map(|row| {
                let view = self
                    .table
                    .viewAtColumn_row_makeIfNecessary(0, isize::try_from(row).ok()?, false)?;
                let card = view.subviews().firstObject()?;
                card.accessibilityLabel().map(|l| l.to_string())
            })
            .collect()
    }
}

/// Whether `entry` is a hook that ran fine, which shows only in its turn's count of such hooks.
pub fn is_quiet_hook(entry: &Entry) -> bool {
    matches!(
        &entry.kind,
        EntryKind::Notice(ide_thread::Notice::Hook {
            outcome: ide_thread::HookOutcome::Success,
            ..
        })
    )
}

/// Whether the focus view leaves `entry` out: tools, thinking, to-dos, settled requests and status lines but errors.
fn hidden_in_focus(entry: &Entry) -> bool {
    match &entry.kind {
        EntryKind::UserMessage(_) | EntryKind::AgentMessage(_) => false,
        EntryKind::PermissionRequest(r) => r.state != ide_thread::RequestState::Pending,
        EntryKind::Question(_) | EntryKind::PlanApproval(_) => false,
        EntryKind::Notice(ide_thread::Notice::Error { .. }) => false,
        EntryKind::ToolCall(_) | EntryKind::Thought(_) | EntryKind::Todos(_) | EntryKind::Notice(_) => true,
    }
}

/// The entries of the main lane that get a row: the hooks of a turn that ran fine share the row of the first of
/// them, and the focus view leaves out what is not conversation.
fn shown(thread: &Thread, focus: bool) -> Vec<std::sync::Arc<Entry>> {
    let mut hook_row = false;
    let mut out = Vec::new();
    for entry in thread.entries(&Lane::Main) {
        if matches!(entry.kind, EntryKind::UserMessage(_)) {
            hook_row = false;
        }
        if focus && hidden_in_focus(entry) {
            continue;
        }
        if is_quiet_hook(entry) {
            if hook_row {
                continue;
            }
            hook_row = true;
        }
        out.push(entry.clone());
    }
    out
}

/// The row of the main lane whose card shows the subagent or tool call `key`.
fn owner_row(thread: &Thread, rows: &[EntryId], key: &str) -> Option<usize> {
    let mut key = key.to_string();
    for _ in 0..8 {
        let entry = thread.entry_by_key(&key)?;
        if let Some(i) = rows.iter().position(|id| *id == entry.id) {
            return Some(i);
        }
        match thread.place(entry.id) {
            Some((Lane::Subagent(parent), _)) => key = parent.clone(),
            _ => return None,
        }
    }
    None
}

/// The height `cell` needs at `width`.
fn measure(cell: &NSTableCellView, width: f64) -> f64 {
    cell.setFrameSize(NSSize::new(width, 100.0));
    let constraint = cell.widthAnchor().constraintEqualToConstant(width);
    constraint.setActive(true);
    let fitting = cell.fittingSize();
    constraint.setActive(false);
    fitting.height.ceil()
}
