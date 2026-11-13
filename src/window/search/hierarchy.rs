//! The call or type hierarchy of a symbol in the search panel, as Xcode's find navigator shows it (E85): a tree whose
//! children the language server is asked for only when an entry opens, a switch between the two directions, and a
//! menu of the text search and the hierarchies shown before, each of which keeps its state.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeSet;
use std::path::PathBuf;

use ide_lsp::lsp_types::{Range, SymbolKind};
use ide_lsp::{HierarchyItem, Neighbour, Ticket};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSColor, NSControlSize, NSControlTextEditingDelegate, NSFont,
    NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation, NSLineBreakMode, NSOutlineView,
    NSOutlineViewDataSource, NSOutlineViewDelegate, NSPopUpButton, NSScrollView, NSSegmentSwitchTracking,
    NSSegmentedControl, NSStackView, NSTableColumn, NSTableColumnResizingOptions, NSTableViewColumnAutoresizingStyle,
    NSTableViewStyle, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use super::SearchPanel;
use crate::lsp::{Direction, Hierarchy, HierarchyKind};
use crate::util::{guarded, label, ns, set_action, set_label, symbol};

/// How many hierarchies the menu keeps.
const HISTORY: usize = 10;
const SMALL_FONT: f64 = 11.0;

/// The call and type hierarchy panels of every window and the id the next one gets.
pub struct HierarchyPanels {
    panels: RefCell<Vec<Weak<HierarchyPanel>>>,
    next_id: Cell<u64>,
}

impl Default for HierarchyPanels {
    fn default() -> Self {
        HierarchyPanels {
            panels: RefCell::new(Vec::new()),
            next_id: Cell::new(1),
        }
    }
}

fn with_panels<R>(f: impl FnOnce(&RefCell<Vec<Weak<HierarchyPanel>>>) -> R) -> R {
    f(&crate::context::app().hierarchy_panels.panels)
}

fn with_next_id<R>(f: impl FnOnce(&Cell<u64>) -> R) -> R {
    f(&crate::context::app().hierarchy_panels.next_id)
}

fn next_id() -> u64 {
    with_next_id(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    })
}

struct Node {
    item: HierarchyItem,
    /// Where a click leads: for a caller the first call in it, else the item's name.
    target: (PathBuf, Range),
    /// `None` until the server was asked.
    children: Option<Vec<usize>>,
    asked: bool,
    key: Retained<NSString>,
}

/// One hierarchy as shown, kept in the menu with its direction and what is open.
struct Entry {
    id: u64,
    hierarchy: Hierarchy,
    direction: Direction,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    expanded: BTreeSet<usize>,
    /// Counts the times the tree started over; an answer for an earlier tree is dropped.
    epoch: u64,
    tickets: Vec<Ticket>,
}

impl Entry {
    fn new(hierarchy: Hierarchy) -> Entry {
        let mut entry = Entry {
            id: next_id(),
            direction: hierarchy.kind.directions()[0],
            hierarchy,
            nodes: vec![],
            roots: vec![],
            expanded: BTreeSet::new(),
            epoch: 0,
            tickets: vec![],
        };
        entry.start_over();
        entry
    }

    /// Forgets every answer and keeps the prepared items, with the roots open.
    fn start_over(&mut self) {
        for ticket in self.tickets.drain(..) {
            ticket.cancel();
        }
        self.epoch += 1;
        self.nodes.clear();
        let roots = self.hierarchy.roots.clone();
        self.roots = roots.into_iter().filter_map(|item| self.add(item, None)).collect();
        self.expanded = self.roots.iter().copied().collect();
    }

    /// Adds a node for `item`, reached through `call` if it is a caller; `None` for an item outside any file.
    fn add(&mut self, item: HierarchyItem, call: Option<(PathBuf, Range)>) -> Option<usize> {
        let path = ide_lsp::uri::to_path(&item.uri)?;
        let target = call.unwrap_or((path, item.selection_range));
        let index = self.nodes.len();
        self.nodes.push(Node {
            item,
            target,
            children: None,
            asked: false,
            // Unique across trees: the outline remembers what is open by equal items, and a new tree starts closed.
            key: ns(&format!("{}:{}:{index}", self.id, self.epoch)),
        });
        Some(index)
    }

    fn adopt(&mut self, parent: usize, found: Vec<Neighbour>) {
        let callers = self.direction == Direction::Callers;
        let children = found
            .into_iter()
            .filter_map(|neighbour| {
                let call = callers
                    .then(|| Some((ide_lsp::uri::to_path(&neighbour.calls_in)?, *neighbour.calls.first()?)))
                    .flatten();
                self.add(neighbour.item, call)
            })
            .collect();
        if let Some(node) = self.nodes.get_mut(parent) {
            node.children = Some(children);
        }
    }

    fn title(&self) -> String {
        let name = self.hierarchy.roots.first().map_or("", |r| r.name.as_str());
        match self.hierarchy.kind {
            HierarchyKind::Calls => tf("Aufrufhierarchie von {}", &[name]),
            HierarchyKind::Types => tf("Typhierarchie von {}", &[name]),
        }
    }
}

fn direction_title(direction: Direction) -> &'static str {
    match direction {
        Direction::Callers => t("Aufrufer"),
        Direction::Callees => t("Aufgerufene"),
        Direction::Supertypes => t("Obertypen"),
        Direction::Subtypes => t("Untertypen"),
    }
}

struct Views {
    page: Retained<NSView>,
    history: Retained<NSPopUpButton>,
    direction: Retained<NSSegmentedControl>,
    outline: Retained<NSOutlineView>,
}

pub struct HierarchyIvars {
    id: u64,
    search: Weak<SearchPanel>,
    entries: RefCell<Vec<Entry>>,
    current: Cell<Option<usize>>,
    views: OnceCell<Views>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `HierarchyPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEHierarchyPanel"]
    #[ivars = HierarchyIvars]
    pub struct HierarchyPanel;

    unsafe impl NSObjectProtocol for HierarchyPanel {}

    unsafe impl NSControlTextEditingDelegate for HierarchyPanel {}

    unsafe impl NSOutlineViewDataSource for HierarchyPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.children(item).len()).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || {
                let child = *self.children(item).get(usize::try_from(index).ok()?)?;
                self.with_entry(|entry| entry.nodes.get(child).map(|n| n.key.clone())).flatten()
            });
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || {
                let Some(index) = index_of(item) else { return false };
                self.with_entry(|entry| {
                    entry
                        .nodes
                        .get(index)
                        .is_some_and(|n| n.children.as_ref().is_none_or(|c| !c.is_empty()))
                })
                .unwrap_or(false)
            })
        }
    }

    unsafe impl NSOutlineViewDelegate for HierarchyPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.row_view(index_of(item)?))
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &NSNotification) {
            guarded((), || self.folded(notification, false));
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || self.folded(notification, true));
        }
    }

    impl HierarchyPanel {
        #[unsafe(method(ideHierarchyDirection:))]
        fn direction_changed(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.switch_direction());
        }

        #[unsafe(method(ideHierarchyHistory:))]
        fn history_chosen(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.choose_history());
        }

        #[unsafe(method(ideHierarchyRowClicked:))]
        fn row_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    self.open_row(views.outline.clickedRow());
                }
            });
        }
    }
);

fn index_of(item: &AnyObject) -> Option<usize> {
    item.downcast_ref::<NSString>()?
        .to_string()
        .rsplit(':')
        .next()?
        .parse()
        .ok()
}

fn panel(id: u64) -> Option<Retained<HierarchyPanel>> {
    with_panels(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|p| p.load().is_some());
        panels.iter().filter_map(Weak::load).find(|p| p.ivars().id == id)
    })
}

impl HierarchyPanel {
    pub(super) fn new(mtm: MainThreadMarker, search: &SearchPanel) -> Retained<HierarchyPanel> {
        let ivars = HierarchyIvars {
            id: next_id(),
            search: Weak::from(search),
            entries: RefCell::new(vec![]),
            current: Cell::new(None),
            views: OnceCell::new(),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<HierarchyPanel> =
            unsafe { msg_send![super(HierarchyPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = this.ivars().views.set(build(mtm, &this));
        with_panels(|panels| panels.borrow_mut().push(Weak::from(&*this)));
        this
    }

    /// The page with the menu, the direction switch and the tree, hidden while the text search shows.
    pub(super) fn page(&self) -> Option<&NSView> {
        self.ivars().views.get().map(|v| &*v.page)
    }

    /// Shows `hierarchy` as the newest entry of the menu, its roots open and their first neighbours asked for.
    pub(super) fn show(&self, hierarchy: Hierarchy) {
        {
            let mut entries = self.ivars().entries.borrow_mut();
            entries.push(Entry::new(hierarchy));
            while entries.len() > HISTORY {
                let mut gone = entries.remove(0);
                gone.start_over();
            }
            self.ivars().current.set(Some(entries.len() - 1));
        }
        self.present();
        if let Some(views) = self.ivars().views.get()
            && let Some(window) = views.page.window()
        {
            window.makeFirstResponder(Some(&views.outline));
        }
    }

    /// Whether a hierarchy is on show rather than the text search.
    pub(super) fn showing(&self) -> bool {
        self.ivars().current.get().is_some()
    }

    /// Leaves the hierarchies as they are for the text search.
    pub(super) fn hide(&self) {
        self.ivars().current.set(None);
        if let Some(views) = self.ivars().views.get() {
            views.page.setHidden(true);
        }
    }

    fn with_entry<R>(&self, f: impl FnOnce(&Entry) -> R) -> Option<R> {
        let entries = self.ivars().entries.borrow();
        entries.get(self.ivars().current.get()?).map(f)
    }

    fn with_entry_mut<R>(&self, f: impl FnOnce(&mut Entry) -> R) -> Option<R> {
        let mut entries = self.ivars().entries.borrow_mut();
        entries.get_mut(self.ivars().current.get()?).map(f)
    }

    fn children(&self, item: Option<&AnyObject>) -> Vec<usize> {
        self.with_entry(|entry| match item.map(index_of) {
            None => entry.roots.clone(),
            Some(Some(index)) => entry
                .nodes
                .get(index)
                .and_then(|n| n.children.clone())
                .unwrap_or_default(),
            Some(None) => vec![],
        })
        .unwrap_or_default()
    }

    /// Fills the menu and the switch for the entry on show and lays out its tree, opened as it was left.
    fn present(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some((titles, kind, direction, expanded)) = self.with_entry(|entry| {
            let titles: Vec<String> = self.ivars().entries.borrow().iter().map(Entry::title).collect();
            (titles, entry.hierarchy.kind, entry.direction, entry.expanded.clone())
        }) else {
            return;
        };
        views.history.removeAllItems();
        views.history.addItemWithTitle(&ns(t("Textsuche")));
        for title in titles.iter().rev() {
            views.history.addItemWithTitle(&ns(title));
        }
        let current = self.ivars().current.get().unwrap_or(0);
        views
            .history
            .selectItemAtIndex(isize::try_from(titles.len() - current).unwrap_or(0));
        for (segment, choice) in kind.directions().into_iter().enumerate() {
            let segment = isize::try_from(segment).unwrap_or(0);
            views
                .direction
                .setLabel_forSegment(&ns(direction_title(choice)), segment);
            if choice == direction {
                views.direction.setSelectedSegment(segment);
            }
        }
        let name = match kind {
            HierarchyKind::Calls => t("Aufrufhierarchie"),
            HierarchyKind::Types => t("Typhierarchie"),
        };
        views.outline.setAccessibilityLabel(Some(&ns(name)));
        views.page.setHidden(false);
        views.outline.reloadData();
        views.outline.sizeLastColumnToFit();
        for index in expanded {
            let key = self.with_entry(|e| e.nodes.get(index).map(|n| n.key.clone())).flatten();
            if let Some(key) = key {
                // SAFETY: The key is one the data source hands out, kept alive by its node.
                unsafe { views.outline.expandItem(Some(&key)) };
            }
        }
    }

    /// An entry opened or closed: the first time it opens, its neighbours are asked for.
    fn folded(&self, notification: &NSNotification, collapsed: bool) {
        let item = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")));
        let Some(index) = item.and_then(|item| index_of(&item)) else {
            return;
        };
        let ask = self
            .with_entry_mut(|entry| {
                if collapsed {
                    entry.expanded.remove(&index);
                    return Some(false);
                }
                entry.expanded.insert(index);
                let node = entry.nodes.get_mut(index)?;
                let ask = !node.asked;
                node.asked = true;
                Some(ask)
            })
            .flatten()
            .unwrap_or(false);
        if ask {
            self.ask(index);
        }
    }

    fn ask(&self, index: usize) {
        let Some((hierarchy, item, direction, entry_id, epoch)) = self
            .with_entry(|entry| {
                let node = entry.nodes.get(index)?;
                Some((
                    entry.hierarchy.clone(),
                    node.item.clone(),
                    entry.direction,
                    entry.id,
                    entry.epoch,
                ))
            })
            .flatten()
        else {
            return;
        };
        let id = self.ivars().id;
        let ticket = hierarchy.neighbours(&item, direction, move |found| {
            if let Some(panel) = panel(id) {
                panel.answered(entry_id, epoch, index, found);
            }
        });
        if let Some(ticket) = ticket {
            self.with_entry_mut(|entry| entry.tickets.push(ticket));
        } else {
            self.answered(entry_id, epoch, index, vec![]);
        }
    }

    /// The neighbours of node `index` of entry `entry_id` arrived; they show if that tree is still the same.
    fn answered(&self, entry_id: u64, epoch: u64, index: usize, found: Vec<Neighbour>) {
        let shown = {
            let mut entries = self.ivars().entries.borrow_mut();
            let Some(position) = entries.iter().position(|e| e.id == entry_id && e.epoch == epoch) else {
                return;
            };
            entries[position].adopt(index, found);
            self.ivars().current.get() == Some(position)
        };
        let Some(views) = self.ivars().views.get() else { return };
        if !shown {
            return;
        }
        let Some((key, open)) = self
            .with_entry(|e| Some((e.nodes.get(index)?.key.clone(), e.expanded.contains(&index))))
            .flatten()
        else {
            return;
        };
        // SAFETY: The key is one the data source hands out, kept alive by its node.
        unsafe { views.outline.reloadItem_reloadChildren(Some(&key), true) };
        if open {
            // SAFETY: The key is the data source's own object for that node, which the node keeps alive.
            unsafe { views.outline.expandItem(Some(&key)) };
        }
    }

    fn switch_direction(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let segment = usize::try_from(views.direction.selectedSegment()).unwrap_or(0);
        let changed = self
            .with_entry_mut(|entry| {
                let chosen = entry.hierarchy.kind.directions()[segment.min(1)];
                if chosen == entry.direction {
                    return false;
                }
                entry.direction = chosen;
                entry.start_over();
                true
            })
            .unwrap_or(false);
        if changed {
            self.present();
        }
    }

    fn choose_history(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let chosen = usize::try_from(views.history.indexOfSelectedItem()).unwrap_or(0);
        let count = self.ivars().entries.borrow().len();
        if chosen == 0 || chosen > count {
            if let Some(search) = self.ivars().search.load() {
                search.show_text();
            }
            return;
        }
        self.ivars().current.set(Some(count - chosen));
        self.present();
    }

    fn open_row(&self, row: isize) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(index) = views.outline.itemAtRow(row).and_then(|item| index_of(&item)) else {
            return;
        };
        let Some((hierarchy, (path, range))) = self
            .with_entry(|entry| Some((entry.hierarchy.clone(), entry.nodes.get(index)?.target.clone())))
            .flatten()
        else {
            return;
        };
        hierarchy.open(&path, range);
    }

    fn row_view(&self, index: usize) -> Option<Retained<NSView>> {
        let (name, kind, place) = self
            .with_entry(|entry| {
                let node = entry.nodes.get(index)?;
                let (path, range) = &node.target;
                let shown = path.strip_prefix(&entry.hierarchy.folder).unwrap_or(path);
                let place = format!("{}:{}", shown.display(), range.start.line + 1);
                Some((node.item.name.clone(), node.item.kind, place))
            })
            .flatten()?;
        Some(row(self.mtm(), &name, kind, &place))
    }
}

/// A row: the kind's symbol, the name and where a click leads.
fn row(mtm: MainThreadMarker, name: &str, kind: SymbolKind, place: &str) -> Retained<NSView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(ide_ui_common::style::space::SNUG);
    if let Some(image) = symbol(crate::lsp::kind_symbol(kind), "") {
        let icon = NSImageView::imageViewWithImage(&image, mtm);
        icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        icon.setAccessibilityElement(false);
        stack.addArrangedSubview(&icon);
    }
    let title = label(mtm, name);
    title.setFont(Some(&crate::editor::code_font(SMALL_FONT + 1.0)));
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    title.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    let where_ = label(mtm, place);
    where_.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    where_.setTextColor(Some(&NSColor::secondaryLabelColor()));
    where_.setLineBreakMode(NSLineBreakMode::ByTruncatingHead);
    where_.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    stack.addArrangedSubview(&title);
    stack.addArrangedSubview(&where_);
    stack.into_super()
}

fn build(mtm: MainThreadMarker, panel: &HierarchyPanel) -> Views {
    let page = NSView::new(mtm);
    let history = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 22.0)),
        false,
    );
    history.setControlSize(NSControlSize::Small);
    history.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    set_label(&history, Some(t("Verlauf")));
    set_action(&history, panel, sel!(ideHierarchyHistory:));
    history.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    let direction = NSSegmentedControl::new(mtm);
    direction.setSegmentCount(2);
    direction.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
    direction.setSegmentDistribution(objc2_app_kit::NSSegmentDistribution::FillEqually);
    direction.setControlSize(NSControlSize::Small);
    direction.setFont(Some(&ide_ui_common::style::small_system()));
    direction.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    set_label(&direction, Some(t("Was die Liste zeigt")));
    set_action(&direction, panel, sel!(ideHierarchyDirection:));
    let bar = NSStackView::new(mtm);
    bar.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    bar.setAlignment(NSLayoutAttribute::Leading);
    bar.setSpacing(ide_ui_common::style::space::S);
    bar.setEdgeInsets(NSEdgeInsets {
        top: 0.0,
        left: 8.0,
        bottom: 4.0,
        right: 8.0,
    });
    for view in [&*history as &NSView, &direction] {
        bar.addArrangedSubview(view);
        view.trailingAnchor()
            .constraintEqualToAnchor_constant(&bar.trailingAnchor(), -8.0)
            .setActive(true);
    }
    let (scroll, outline) = tree(mtm, panel);
    page.addSubview(&bar);
    page.addSubview(&scroll);
    for view in [&*bar as &NSView, &scroll] {
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
        view.leadingAnchor()
            .constraintEqualToAnchor(&page.leadingAnchor())
            .setActive(true);
        view.trailingAnchor()
            .constraintEqualToAnchor(&page.trailingAnchor())
            .setActive(true);
    }
    bar.topAnchor()
        .constraintEqualToAnchor(&page.topAnchor())
        .setActive(true);
    scroll
        .topAnchor()
        .constraintEqualToAnchor_constant(&bar.bottomAnchor(), 2.0)
        .setActive(true);
    scroll
        .bottomAnchor()
        .constraintEqualToAnchor(&page.bottomAnchor())
        .setActive(true);
    page.setHidden(true);
    Views {
        page,
        history,
        direction,
        outline,
    }
}

fn tree(mtm: MainThreadMarker, panel: &HierarchyPanel) -> (Retained<NSScrollView>, Retained<NSOutlineView>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 400.0));
    let outline = NSOutlineView::initWithFrame(NSOutlineView::alloc(mtm), frame);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("symbol"));
    column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
    outline.addTableColumn(&column);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { outline.setOutlineTableColumn(Some(&column)) };
    outline.setHeaderView(None);
    outline.setStyle(NSTableViewStyle::SourceList);
    outline.setIndentationPerLevel(12.0);
    outline.setRowHeight(22.0);
    outline.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
    // SAFETY: The outline view holds its data source and delegate weakly; the search panel keeps this panel alive.
    unsafe {
        outline.setDataSource(Some(ProtocolObject::from_ref(panel)));
        outline.setDelegate(Some(ProtocolObject::from_ref(panel)));
    }
    set_action(&outline, panel, sel!(ideHierarchyRowClicked:));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(&outline));
    outline.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    (scroll, outline)
}

/// What the self-test reads from and does with the hierarchy, through the code its controls run.
#[cfg(feature = "selftest")]
pub mod selftest {
    use objc2::DefinedClass;

    use super::{HierarchyPanel, index_of};

    /// The rows on show, each with its level and name.
    pub fn rows(panel: &HierarchyPanel) -> Vec<(isize, String)> {
        let Some(views) = panel.ivars().views.get() else {
            return vec![];
        };
        (0..views.outline.numberOfRows())
            .filter_map(|row| {
                let item = views.outline.itemAtRow(row)?;
                let index = index_of(&item)?;
                let name = panel
                    .with_entry(|e| e.nodes.get(index).map(|n| n.item.name.clone()))
                    .flatten()?;
                Some((views.outline.levelForRow(row), name))
            })
            .collect()
    }

    /// Opens row `row` as its disclosure triangle does.
    pub fn expand(panel: &HierarchyPanel, row: isize) {
        if let Some(views) = panel.ivars().views.get()
            && let Some(item) = views.outline.itemAtRow(row)
        {
            // SAFETY: The item is one the data source handed out, kept alive by its node.
            unsafe { views.outline.expandItem(Some(&item)) };
        }
    }

    /// Clicks segment `segment` of the direction switch.
    pub fn choose_direction(panel: &HierarchyPanel, segment: isize) {
        if let Some(views) = panel.ivars().views.get() {
            views.direction.setSelectedSegment(segment);
        }
        panel.switch_direction();
    }

    /// The titles of the menu of earlier searches.
    pub fn history(panel: &HierarchyPanel) -> Vec<String> {
        panel.ivars().views.get().map_or_else(Vec::new, |views| {
            views.history.itemTitles().iter().map(|t| t.to_string()).collect()
        })
    }

    /// Chooses item `index` of the menu of earlier searches.
    pub fn choose_history(panel: &HierarchyPanel, index: isize) {
        if let Some(views) = panel.ivars().views.get() {
            views.history.selectItemAtIndex(index);
        }
        panel.choose_history();
    }

    /// Clicks row `row`, which opens its place.
    pub fn click(panel: &HierarchyPanel, row: isize) {
        panel.open_row(row);
    }
}
