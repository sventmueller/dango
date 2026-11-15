//! The timeline: one table row per entry of the thread's own lane.
//!
//! Only rows in view get a card (E25). A row's height is the measured height of its card once it was built at the
//! current width, else an estimate from the entry; a card that turns out taller or shorter than the table thought
//! corrects its row afterwards. A plain stack of cards would build all of them; `usesAutomaticRowHeights` would
//! measure text views before they know their width.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

use ide_thread::{Change, Entry, EntryId, EntryKind, Lane, Thread};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAnimationContext, NSEvent, NSScrollView, NSTableCellView, NSTableColumn, NSTableRowView,
    NSTableView, NSTableViewSelectionHighlightStyle, NSTableViewStyle, NSView,
};
use objc2_foundation::{NSIndexSet, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::cards::{self, Ctx, Inputs, Live, Part};
use super::row::TimelineRow;
use super::target::AgentTarget;
use crate::AgentKey;
use ide_ui_common::util::{activate, guarded, live, main_soon, ns};

/// Room on the leading side for the line and its dots.
const LEADING: f64 = 30.0;
/// The leading room of a user message, which stands apart from the line.
const USER_LEADING: f64 = 8.0;
const TRAILING: f64 = 12.0;
pub(super) const TOP: f64 = 3.0;
pub(super) const BOTTOM: f64 = 10.0;

pub(crate) struct TableIvars {
    key: AgentKey,
}

define_class!(
    // SAFETY: NSTableView has no subclassing requirements and `TimelineTable` does not implement Drop.
    #[unsafe(super(NSTableView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentTimelineTable"]
    #[ivars = TableIvars]
    pub(crate) struct TimelineTable;

    unsafe impl NSObjectProtocol for TimelineTable {}

    impl TimelineTable {
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
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
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
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
                if let Some(tab) = super::get(self.ivars().key)
                    && super::stop::stop_key_pressed(self.mtm())
                {
                    tab.stop_turn(super::Stop::Key);
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
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl TimelineTable {
    /// The view Tab goes to after the timeline and the plan in it.
    pub(crate) fn following_key_view(&self) -> Option<Retained<NSView>> {
        // SAFETY: The superclass's implementation, which takes nothing and returns a view or nil.
        unsafe { msg_send![super(self), nextValidKeyView] }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Correction {
    id: EntryId,
    width: f64,
    before: f64,
    after: f64,
}

pub struct Timeline {
    mtm: MainThreadMarker,
    key: AgentKey,
    pub(super) scroll: Retained<NSScrollView>,
    pub(super) table: Retained<TimelineTable>,
    target: Retained<AgentTarget>,
    cwd: PathBuf,
    thread: RefCell<Rc<Thread>>,
    rows: RefCell<Vec<EntryId>>,
    pub(super) positions: RefCell<HashMap<EntryId, usize>>,
    pending: Cell<bool>,
    pending_follow: Cell<bool>,
    figures: RefCell<HashMap<EntryId, Vec<splat_render::js::Key>>>,
    /// The entries of the rows, in their order.
    list: RefCell<Vec<std::sync::Arc<Entry>>>,
    /// The height published to AppKit for each entry, at the committed width, initially estimated.
    heights: RefCell<HashMap<u64, (f64, f64)>>,
    /// The width the rows were last laid out at.
    width: Cell<f64>,
    /// A divider drag changed the width, and the rows are laid out anew once it ends.
    after_drag: Cell<bool>,
    pub(super) expanded: RefCell<HashSet<(u64, Part)>>,
    pub(super) inputs: RefCell<Inputs>,
    live: RefCell<Vec<Live>>,
    /// Rows whose card turned out another height than the table had, with that height.
    pub(super) corrections: RefCell<BTreeMap<usize, Correction>>,
    pub(super) built: Cell<usize>,
    /// Only the conversation shows: no tools, thinking or status lines.
    focus: Cell<bool>,
    follow: Rc<follow::Follow>,
    glide: follow::Glide,
    scrolled: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for Timeline {
    fn drop(&mut self) {
        // SAFETY: An observer this center handed out.
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(self.scrolled.as_ref()) };
    }
}

impl Timeline {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        key: AgentKey,
        target: &AgentTarget,
        thread: Rc<Thread>,
        cwd: PathBuf,
    ) -> Timeline {
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
        table.setUsesAutomaticRowHeights(false);
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
        let width = scroll.contentSize().width.max(200.0);
        let list = shown(&thread, false);
        let rows: Vec<_> = list.iter().map(|e| e.id).collect();
        let positions = rows.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let follow = follow::Follow::new();
        let scrolled = follow::observe(&scroll, &follow);
        Timeline {
            mtm,
            key,
            scroll,
            table,
            target: target.retain(),
            cwd,
            thread: RefCell::new(thread),
            rows: RefCell::new(rows),
            positions: RefCell::new(positions),
            pending: Cell::new(false),
            pending_follow: Cell::new(false),
            figures: RefCell::new(HashMap::new()),
            list: RefCell::new(list),
            heights: RefCell::new(HashMap::new()),
            width: Cell::new(width),
            after_drag: Cell::new(false),
            expanded: RefCell::new(HashSet::new()),
            inputs: RefCell::new(Inputs::default()),
            live: RefCell::new(Vec::new()),
            corrections: RefCell::new(BTreeMap::new()),
            built: Cell::new(0),
            focus: Cell::new(false),
            follow,
            glide: follow::Glide::default(),
            scrolled,
        }
    }

    pub fn view(&self) -> &NSScrollView {
        &self.scroll
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

    pub(crate) fn row_height(&self, row: usize) -> f64 {
        let Some(entry) = self.entry(row) else { return 20.0 };
        self.known_height(&entry)
    }

    fn known_height(&self, entry: &Entry) -> f64 {
        let width = self.width.get();
        if let Some((measured, height)) = self.heights.borrow().get(&entry.id.get())
            && (measured - width).abs() < 0.5
        {
            return *height;
        }
        let height = cards::estimate(entry, width - Self::leading(entry) - TRAILING) + TOP + BOTTOM;
        self.heights.borrow_mut().insert(entry.id.get(), (width, height));
        height
    }

    pub(crate) fn row_view(&self, row: usize) -> Retained<NSTableRowView> {
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
            let (card, figures) = super::figures::collect(|| cards::build(&ctx, &entry));
            self.figures.borrow_mut().insert(entry.id, figures);
            card
        };
        self.built.set(self.built.get() + 1);
        let cell = NSTableCellView::new(self.mtm);
        cell.addSubview(&card);
        card.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            card.leadingAnchor()
                .constraintEqualToAnchor_constant(&cell.leadingAnchor(), leading),
            card.trailingAnchor()
                .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -TRAILING),
            card.topAnchor()
                .constraintEqualToAnchor_constant(&cell.topAnchor(), TOP),
        ]);
        let bottom = card
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&cell.bottomAnchor(), -BOTTOM);
        // Below required, so a row the table still has at its estimated height stretches the card, not the rules.
        bottom.setPriority(999.0);
        bottom.setActive(true);
        let height = measure(&cell, width);
        self.record_height(row, entry.id, width, before, height);
        Some(Retained::into_super(cell))
    }

    /// Keeps delegate heights stable until AppKit is notified; geometry queries may otherwise corrupt its cache.
    fn record_height(&self, row: usize, id: EntryId, width: f64, before: f64, after: f64) {
        if (width - self.width.get()).abs() >= 0.5 {
            return;
        }
        if (after - before).abs() <= 0.5 {
            self.corrections.borrow_mut().remove(&row);
            self.heights.borrow_mut().insert(id.get(), (width, before));
            return;
        }
        let first = self.corrections.borrow().is_empty();
        self.corrections
            .borrow_mut()
            .entry(row)
            .and_modify(|correction| {
                correction.width = width;
                correction.after = after;
            })
            .or_insert(Correction {
                id,
                width,
                before,
                after,
            });
        if first {
            let key = self.key;
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.timeline().flush_corrections();
                }
            });
        }
    }

    /// Gives the rows rebuilt outside AppKit's own row building their height before the window draws again; the
    /// deferred flush would leave one frame with each changed card in its old row.
    fn correct_now(&self) {
        for _ in 0..4 {
            if self.corrections.borrow().is_empty() {
                break;
            }
            self.flush_corrections();
        }
    }

    pub(super) fn flush_corrections(&self) {
        let corrections = std::mem::take(&mut *self.corrections.borrow_mut());
        if corrections.is_empty() {
            return;
        }
        let following = self.follows();
        let visible = self.scroll.contentView().bounds();
        let mut shift = 0.0;
        let set = objc2_foundation::NSMutableIndexSet::new();
        for (row, correction) in &corrections {
            if self.rows.borrow().get(*row) != Some(&correction.id) {
                continue;
            }
            let rect = self.table.rectOfRow(isize::try_from(*row).unwrap_or(0));
            if rect.origin.y + rect.size.height <= visible.origin.y + 1.0 {
                shift += correction.after - correction.before;
            }
            set.addIndex(*row);
        }
        // All old rectangles must be read before publishing any new heights.
        for (row, correction) in corrections {
            if self.rows.borrow().get(row) == Some(&correction.id) {
                self.heights
                    .borrow_mut()
                    .insert(correction.id.get(), (correction.width, correction.after));
            }
        }
        NSAnimationContext::beginGrouping();
        NSAnimationContext::currentContext().setDuration(0.0);
        self.table.noteHeightOfRowsWithIndexesChanged(&set);
        NSAnimationContext::endGrouping();
        if following {
            self.follow_end();
        } else if shift.abs() > 0.5 {
            self.shift_by(shift);
        }
    }

    /// Takes in a new state; content-only changes keep the row index and untouched cards.
    pub(crate) fn apply(&self, thread: Rc<Thread>, changes: &[Change]) {
        let following = self.follows() || self.count() == 0;
        let structural = self.structure_changed(&thread, changes);
        let old_len = self.count();
        let appended = !structural || self.reindex(&thread);
        *self.thread.borrow_mut() = thread;
        let reload = self.changed_rows(changes);
        if !structural && reload.is_empty() {
            return;
        }
        if !self.is_visible() {
            if !self.pending.replace(true) {
                self.pending_follow.set(following);
            }
            return;
        }
        if self.pending.get() {
            self.catch_up();
            self.refresh_marks();
            return;
        }
        let reloaded = !appended || changes.contains(&Change::Cleared);
        if reloaded {
            self.table.reloadData();
        } else {
            if self.count() > old_len {
                let added = NSIndexSet::indexSetWithIndexesInRange(objc2_foundation::NSRange::new(
                    old_len,
                    self.count() - old_len,
                ));
                self.table
                    .insertRowsAtIndexes_withAnimation(&added, objc2_app_kit::NSTableViewAnimationOptions::empty());
            }
            self.refresh_rows(reload.into_iter().filter(|i| *i < old_len));
        }
        self.correct_now();
        self.refresh_marks();
        if following && reloaded {
            self.scroll_to_end();
        } else if following {
            self.follow_end();
        }
    }

    fn structure_changed(&self, thread: &Thread, changes: &[Change]) -> bool {
        if self.thread.borrow().entries(&Lane::Main).len() != thread.entries(&Lane::Main).len() {
            return true;
        }
        changes.iter().any(|change| match change {
            Change::Cleared | Change::Inserted { lane: Lane::Main, .. } | Change::Removed { lane: Lane::Main, .. } => {
                true
            }
            Change::Updated { lane: Lane::Main, id } => {
                let new = thread.entry(*id);
                let old_thread = self.thread.borrow();
                let old = old_thread.entry(*id);
                match (old, new) {
                    (Some(old), Some(new)) => {
                        is_quiet_hook(old) != is_quiet_hook(new)
                            || is_running_hook(old) != is_running_hook(new)
                            || matches!(old.kind, EntryKind::UserMessage(_))
                                != matches!(new.kind, EntryKind::UserMessage(_))
                            || (self.focus.get() && hidden_in_focus(old) != hidden_in_focus(new))
                    }
                    _ => true,
                }
            }
            _ => false,
        })
    }

    /// Rebuilds the filtered index only for structural edits, in linear time.
    fn reindex(&self, thread: &Thread) -> bool {
        let list = shown(thread, self.focus.get());
        let new: Vec<_> = list.iter().map(|e| e.id).collect();
        let positions: HashMap<_, _> = new.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let old = self.rows.borrow();
        let appended = new.starts_with(&old);
        for id in old.iter().filter(|id| !positions.contains_key(id)) {
            self.inputs.borrow_mut().forget(id.get());
            self.heights.borrow_mut().remove(&id.get());
            self.figures.borrow_mut().remove(id);
        }
        drop(old);
        // A correction waiting for its flush would otherwise be lost, and its card stay squeezed into the old height.
        let corrections = std::mem::take(&mut *self.corrections.borrow_mut());
        *self.corrections.borrow_mut() = corrections
            .into_values()
            .filter_map(|correction| positions.get(&correction.id).map(|row| (*row, correction)))
            .collect();
        *self.rows.borrow_mut() = new;
        *self.positions.borrow_mut() = positions;
        *self.list.borrow_mut() = list;
        appended
    }

    fn changed_rows(&self, changes: &[Change]) -> HashSet<usize> {
        let thread = self.thread.borrow();
        let positions = self.positions.borrow();
        let mut list = self.list.borrow_mut();
        let mut reload = HashSet::new();
        for change in changes {
            match change {
                Change::Updated { lane: Lane::Main, id } => {
                    if let Some(&i) = positions.get(id) {
                        if let Some((_, index)) = thread.place(*id) {
                            list[i] = thread.entries(&Lane::Main)[index].clone();
                        }
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
                    if let Some(i) = owner_row(&thread, &positions, key) {
                        reload.insert(i);
                    }
                }
                _ => {}
            }
        }
        // Successful hooks share one row per turn.
        if changes.iter().any(|c| match c {
            Change::Inserted {
                lane: Lane::Main, id, ..
            }
            | Change::Updated { lane: Lane::Main, id } => {
                !positions.contains_key(id) && thread.entry(*id).is_some_and(is_quiet_hook)
            }
            Change::Removed { lane: Lane::Main, .. } => true,
            _ => false,
        }) {
            reload.extend(
                list.iter()
                    .enumerate()
                    .filter(|(_, e)| is_quiet_hook(e))
                    .map(|(i, _)| i),
            );
        }
        reload
    }

    fn is_visible(&self) -> bool {
        !self.scroll.isHiddenOrHasHiddenAncestor()
            && self.scroll.window().is_some_and(|w| ide_ui_common::panel::in_front(&w))
    }

    /// A background tab catches up once, when it becomes visible again.
    pub(crate) fn show_pending(&self) {
        if self.is_visible() && self.pending.get() {
            self.catch_up();
        }
    }

    /// Builds the rows of a timeline that was hidden at the width it has now, and shows the end if it did before.
    fn catch_up(&self) {
        // A native tab has just adopted its sidebar; hydrate at the final viewport width.
        if let Some(window) = self.scroll.window() {
            window.layoutIfNeeded();
        }
        self.pending.set(false);
        let width = self.table_width();
        self.fit_column(width);
        self.width.set(width);
        self.corrections.borrow_mut().clear();
        self.table.reloadData();
        if self.pending_follow.get() {
            self.scroll_to_end();
        }
    }

    fn refresh_rows(&self, rows: impl Iterator<Item = usize>) {
        let mut reload = Vec::new();
        for row in rows {
            if !self.update_stream(row) {
                reload.push(row);
            }
        }
        self.reload_rows(reload.into_iter());
    }

    fn update_stream(&self, row: usize) -> bool {
        let Some(entry) = self.entry(row) else { return false };
        let EntryKind::AgentMessage(message) = &entry.kind else {
            return false;
        };
        if !message.streaming || message.markdown.trim().is_empty() {
            return false;
        }
        let Some(cell) = self
            .table
            .viewAtColumn_row_makeIfNecessary(0, isize::try_from(row).unwrap_or(0), false)
        else {
            return false;
        };
        let Some(text) = cell
            .subviews()
            .firstObject()
            .and_then(|v| v.downcast::<super::text::CardText>().ok())
        else {
            return false;
        };
        let before = self.known_height(&entry);
        let (updated, figures) = super::figures::collect(|| text.update_markdown(&message.markdown));
        if updated {
            self.figures.borrow_mut().insert(entry.id, figures);
            ui_summary(&text, &entry, &self.thread.borrow());
            // `measure` sizes a detached cell. Resizing a live table cell would invalidate other rows.
            let height = text.intrinsicContentSize().height.ceil() + TOP + BOTTOM;
            self.record_height(row, entry.id, self.table_width(), before, height);
        }
        true
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
    pub(crate) fn reload_entry(&self, id: u64) {
        if let Some(row) = self
            .positions
            .borrow()
            .iter()
            .find_map(|(entry, row)| (entry.get() == id).then_some(*row))
        {
            self.reload_rows(std::iter::once(row));
            self.correct_now();
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
    pub(crate) fn toggle(&self, id: u64, part: Part) {
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
    pub(crate) fn toggle_row(&self, row: isize) -> bool {
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

    pub(crate) fn focus_view(&self) -> bool {
        self.focus.get()
    }

    /// Shows only the conversation, or everything again.
    pub(crate) fn set_focus_view(&self, on: bool) {
        self.focus.set(on);
        self.reindex(&self.thread.borrow());
        self.table.reloadData();
        self.scroll_to_end();
    }

    /// Whether the view shows the end of the timeline.
    pub fn is_at_bottom(&self) -> bool {
        at_bottom(&self.scroll)
    }

    /// Scrolls to entry `key` and selects its row.
    pub fn reveal(&self, key: &str) {
        let thread = self.thread.borrow().clone();
        let Some(row) = owner_row(&thread, &self.positions.borrow(), key) else {
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
    pub(crate) fn figures_arrived(&self, keys: &[splat_render::js::Key]) {
        let rows: Vec<_> = self
            .figures
            .borrow()
            .iter()
            .filter(|(_, wanted)| wanted.iter().any(|key| keys.contains(key)))
            .filter_map(|(id, _)| self.positions.borrow().get(id).copied())
            .collect();
        if rows.is_empty() {
            return;
        }
        if !self.is_visible() {
            if !self.pending.replace(true) {
                self.pending_follow.set(self.follows());
            }
            return;
        }
        self.reload_rows(rows.into_iter());
        self.correct_now();
        self.follow_end();
    }

    /// Lays the cards out anew after the width changed.
    pub(crate) fn width_changed(&self) {
        let width = self.table_width();
        // `record_height` drops measurements half a point off the committed width, so a smaller change must commit.
        if (width - self.width.get()).abs() < 0.5 || self.table.inLiveResize() {
            return;
        }
        // The column follows the table by itself, so the rows in view take the width and only the heights wait.
        if live::dragging(&self.table) {
            if !self.after_drag.replace(true) {
                let key = self.key;
                live::after_drag(move || {
                    if let Some(tab) = super::get(key) {
                        tab.timeline().after_drag.set(false);
                        tab.timeline().width_changed();
                    }
                });
            }
            return;
        }
        let following = self.follows();
        if !self.is_visible() {
            if !self.pending.replace(true) {
                self.pending_follow.set(following);
            }
            return;
        }
        self.fit_column(width);
        self.width.set(width);
        self.corrections.borrow_mut().clear();
        self.table.reloadData();
        if following {
            self.scroll_to_end();
        }
    }

    /// Advances the running times of the cards in view; whether any runs.
    pub(crate) fn tick(&self, now: SystemTime) -> bool {
        if !self.is_visible() {
            return false;
        }
        let mut live = self.live.borrow_mut();
        live.retain(|l| l.label.window().is_some());
        for l in live.iter() {
            l.label.setStringValue(&ns(&super::format::elapsed(
                now.duration_since(l.since).unwrap_or_default(),
            )));
        }
        !live.is_empty()
    }
}

/// Whether `entry` is a hook that ran fine, which shows only in its turn's count of such hooks.
pub(crate) fn is_quiet_hook(entry: &Entry) -> bool {
    matches!(
        &entry.kind,
        EntryKind::Notice(ide_thread::Notice::Hook {
            outcome: ide_thread::HookOutcome::Success,
            ..
        })
    )
}

/// Whether `entry` is a hook still running; it gets a row only once it failed, so hooks firing around every tool call
/// do not add and drop a row at the end of the timeline.
fn is_running_hook(entry: &Entry) -> bool {
    matches!(
        &entry.kind,
        EntryKind::Notice(ide_thread::Notice::Hook {
            outcome: ide_thread::HookOutcome::Running,
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
/// them, running hooks have none, and the focus view leaves out what is not conversation.
fn shown(thread: &Thread, focus: bool) -> Vec<std::sync::Arc<Entry>> {
    let mut hook_row = false;
    let mut out = Vec::new();
    for entry in thread.entries(&Lane::Main) {
        if matches!(entry.kind, EntryKind::UserMessage(_)) {
            hook_row = false;
        }
        if is_running_hook(entry) || (focus && hidden_in_focus(entry)) {
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
fn owner_row(thread: &Thread, rows: &HashMap<EntryId, usize>, key: &str) -> Option<usize> {
    let mut key = key.to_string();
    for _ in 0..8 {
        let entry = thread.entry_by_key(&key)?;
        if let Some(i) = rows.get(&entry.id) {
            return Some(*i);
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

fn ui_summary(view: &NSView, entry: &Entry, thread: &Thread) {
    super::ui::group(view, &cards::summary(thread, entry));
}

fn at_bottom(scroll: &NSScrollView) -> bool {
    let clip = scroll.contentView().bounds();
    let height = scroll.documentView().map_or(0.0, |table| table.frame().size.height);
    clip.origin.y + clip.size.height >= height - 40.0
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;

mod follow;

#[cfg(feature = "selftest")]
mod selftest_probe;
