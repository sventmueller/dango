//! The content area as a tree of panels (I10). Each panel has a tab bar of its own; panels sit side by side or one
//! above the other in nested split views; a tab moves between panels with its view, which keeps running.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak as RcWeak};

use objc2::rc::{Retained, Weak};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBox, NSResponder, NSSplitViewController, NSSplitViewDividerStyle, NSSplitViewItem, NSView, NSViewController,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::WindowController;
use super::tabs::{TabBar, drop_highlight};

/// The smallest a panel gets along a split.
const MIN_PANEL: f64 = 160.0;
/// The share of a panel's width or height at its right or bottom edge where a dropped tab opens a new panel.
const EDGE: f64 = 0.3;

/// Where a new panel goes beside another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Below,
}

/// One panel: a tab bar and the views of its tabs.
pub struct Pane {
    /// The panel's number in its window, which stays while the window lives.
    number: usize,
    pub tabs: TabBar,
    item: Retained<NSSplitViewItem>,
    container: Retained<NSView>,
    /// The accent over the part of the panel a dragged tab would drop into.
    drop: Retained<NSBox>,
    /// The view in the panel that had the keyboard focus last, which gets it back when the panel is activated.
    focus: RefCell<Option<Weak<NSView>>>,
}

impl Pane {
    fn new(
        mtm: MainThreadMarker,
        target: &WindowController,
        number: usize,
        next: Rc<Cell<isize>>,
        docked: bool,
    ) -> Rc<Pane> {
        let tabs = TabBar::new(mtm, target, number, next);
        let container = NSView::new(mtm);
        container.addSubview(tabs.content());
        if !docked {
            container.addSubview(tabs.view());
        }
        lay_out(&container, &tabs, docked);
        let drop = drop_highlight(mtm);
        drop.setTranslatesAutoresizingMaskIntoConstraints(true);
        container.addSubview(&drop);
        let controller = NSViewController::new(mtm);
        controller.setView(&container);
        let item = NSSplitViewItem::splitViewItemWithViewController(&controller);
        if docked {
            let accessory = objc2_app_kit::NSSplitViewItemAccessoryViewController::new(mtm);
            accessory.setView(tabs.view());
            item.addTopAlignedAccessoryViewController(&accessory);
        }
        item.setMinimumThickness(MIN_PANEL);
        Rc::new(Pane {
            number,
            tabs,
            item,
            container,
            drop,
            focus: RefCell::new(None),
        })
    }

    /// Whether `view` belongs to the panel.
    fn holds_view(&self, view: &NSView) -> bool {
        view.isDescendantOf(&self.container) || view.isDescendantOf(self.tabs.view())
    }

    /// `point`, in window coordinates, in the panel's content, if it lies there.
    fn content_point(&self, point: NSPoint) -> Option<NSPoint> {
        let content = self.tabs.content();
        let local = content.convertPoint_fromView(point, None);
        objc2_foundation::NSPointInRect(local, content.bounds()).then_some(local)
    }

    /// Shows the accent over the right or bottom part, or over the whole content, or hides it.
    fn show_drop(&self, part: Option<Option<Side>>) {
        let Some(side) = part else {
            self.drop.setHidden(true);
            return;
        };
        let content = self.tabs.content().frame();
        let (origin, size) = (content.origin, content.size);
        let frame = match side {
            Some(Side::Right) => NSRect::new(
                NSPoint::new(origin.x + size.width / 2.0, origin.y),
                NSSize::new(size.width / 2.0, size.height),
            ),
            Some(Side::Below) => NSRect::new(origin, NSSize::new(size.width, size.height / 2.0)),
            None => content,
        };
        self.drop.setFrame(frame.inset(6.0));
        self.drop.setHidden(false);
    }
}

trait Inset {
    fn inset(self, by: f64) -> Self;
}

impl Inset for NSRect {
    fn inset(self, by: f64) -> NSRect {
        NSRect::new(
            NSPoint::new(self.origin.x + by, self.origin.y + by),
            NSSize::new(
                (self.size.width - 2.0 * by).max(0.0),
                (self.size.height - 2.0 * by).max(0.0),
            ),
        )
    }
}

/// The tab content below the tab bar, or below the docked bar's safe area.
fn lay_out(container: &NSView, tabs: &TabBar, docked: bool) {
    let guide = container.safeAreaLayoutGuide();
    tabs.content().setTranslatesAutoresizingMaskIntoConstraints(false);
    let top = if docked {
        tabs.content().topAnchor().constraintEqualToAnchor(&guide.topAnchor())
    } else {
        let bar = tabs.view();
        bar.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            bar.leadingAnchor().constraintEqualToAnchor(&container.leadingAnchor()),
            bar.trailingAnchor()
                .constraintEqualToAnchor(&container.trailingAnchor()),
            bar.topAnchor().constraintEqualToAnchor(&guide.topAnchor()),
        ] {
            constraint.setActive(true);
        }
        tabs.content().topAnchor().constraintEqualToAnchor(&bar.bottomAnchor())
    };
    let content = tabs.content();
    for constraint in [
        content
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor()),
        content
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        content
            .bottomAnchor()
            .constraintEqualToAnchor(&container.bottomAnchor()),
        top,
    ] {
        constraint.setActive(true);
    }
}

/// Panels split side by side or one above the other.
struct Split {
    controller: Retained<NSSplitViewController>,
    /// How the split sits in the split around it.
    item: Retained<NSSplitViewItem>,
    children: RefCell<Vec<Node>>,
}

#[derive(Clone)]
enum Node {
    Pane(Rc<Pane>),
    Split(Rc<Split>),
}

impl Node {
    fn item(&self) -> Retained<NSSplitViewItem> {
        match self {
            Node::Pane(pane) => pane.item.clone(),
            Node::Split(split) => split.item.clone(),
        }
    }

    fn is_pane(&self, pane: &Rc<Pane>) -> bool {
        matches!(self, Node::Pane(p) if Rc::ptr_eq(p, pane))
    }

    fn is_split(&self, split: &Rc<Split>) -> bool {
        matches!(self, Node::Split(s) if Rc::ptr_eq(s, split))
    }
}

impl Split {
    fn new(mtm: MainThreadMarker, side_by_side: bool) -> Rc<Split> {
        let controller = NSSplitViewController::new(mtm);
        let view = controller.splitView();
        view.setVertical(side_by_side);
        view.setDividerStyle(NSSplitViewDividerStyle::Thin);
        let item = NSSplitViewItem::splitViewItemWithViewController(&controller);
        item.setMinimumThickness(MIN_PANEL);
        Rc::new(Split {
            controller,
            item,
            children: RefCell::new(vec![]),
        })
    }

    fn side_by_side(&self) -> bool {
        self.controller.splitView().isVertical()
    }

    fn insert(&self, index: usize, node: Node) {
        let index = index.min(self.children.borrow().len());
        self.controller
            .insertSplitViewItem_atIndex(&node.item(), isize::try_from(index).unwrap_or(0));
        self.children.borrow_mut().insert(index, node);
    }

    fn remove(&self, index: usize) -> Node {
        let node = self.children.borrow_mut().remove(index);
        self.controller.removeSplitViewItem(&node.item());
        node
    }

    /// Each child's share of the room along the split.
    fn shares(&self) -> Vec<f64> {
        let side_by_side = self.side_by_side();
        let lengths: Vec<f64> = self
            .children
            .borrow()
            .iter()
            .map(|child| {
                let size = child.item().viewController(self.controller.mtm()).view().frame().size;
                if side_by_side { size.width } else { size.height }
            })
            .collect();
        let total: f64 = lengths.iter().sum();
        if total <= 0.0 {
            return vec![1.0 / lengths.len().max(1) as f64; lengths.len()];
        }
        lengths.iter().map(|length| length / total).collect()
    }

    /// Gives the children `shares` of the room, or equal ones.
    fn set_shares(&self, shares: Option<&[f64]>) {
        let count = self.children.borrow().len();
        if count < 2 {
            return;
        }
        let view = self.controller.splitView();
        view.layoutSubtreeIfNeeded();
        let size = view.frame().size;
        let length = if self.side_by_side() { size.width } else { size.height };
        let divider = view.dividerThickness();
        let room = length - divider * (count - 1) as f64;
        let equal = vec![1.0 / count as f64; count];
        let shares = shares.filter(|s| s.len() == count).unwrap_or(&equal);
        let mut position = 0.0;
        for (index, share) in shares.iter().take(count - 1).enumerate() {
            position += share * room;
            view.setPosition_ofDividerAtIndex(position, isize::try_from(index).unwrap_or(0));
            position += divider;
        }
    }
}

/// Where a dragged tab would land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Landing {
    /// Between the tabs of its own bar, which it moves along.
    Reorder,
    /// Into the panel numbered so, at a place in its bar.
    Into(usize, Option<usize>),
    /// Into a new panel beside the one numbered so.
    Beside(usize, Side),
    Nowhere,
}

/// The panels of a window.
pub struct Panes {
    mtm: MainThreadMarker,
    target: Weak<WindowController>,
    root: Rc<Split>,
    active: RefCell<RcWeak<Pane>>,
    next_tab: Rc<Cell<isize>>,
    next_number: Cell<usize>,
    docked: bool,
}

impl Panes {
    /// One panel; `docked` puts tab bars above split view items, as macOS 26 and later offer.
    pub fn new(mtm: MainThreadMarker, target: &WindowController, docked: bool) -> Panes {
        let root = Split::new(mtm, true);
        let panes = Panes {
            mtm,
            target: Weak::from(target),
            root,
            active: RefCell::new(RcWeak::new()),
            next_tab: Rc::new(Cell::new(1)),
            next_number: Cell::new(1),
            docked,
        };
        let first = panes.new_pane(target);
        panes.root.insert(0, Node::Pane(first.clone()));
        panes.set_active(&first);
        panes.relabel();
        panes
    }

    /// The split view controller of the content area.
    pub fn controller(&self) -> &NSSplitViewController {
        &self.root.controller
    }

    fn new_pane(&self, target: &WindowController) -> Rc<Pane> {
        let number = self.next_number.get();
        self.next_number.set(number + 1);
        Pane::new(self.mtm, target, number, self.next_tab.clone(), self.docked)
    }

    /// Every panel, left to right and top to bottom.
    pub fn all(&self) -> Vec<Rc<Pane>> {
        fn walk(split: &Split, out: &mut Vec<Rc<Pane>>) {
            for child in split.children.borrow().iter() {
                match child {
                    Node::Pane(pane) => out.push(pane.clone()),
                    Node::Split(split) => walk(split, out),
                }
            }
        }
        let mut out = vec![];
        walk(&self.root, &mut out);
        out
    }

    /// The panel that holds the tab `id`.
    pub fn of_tab(&self, id: isize) -> Option<Rc<Pane>> {
        self.all().into_iter().find(|pane| pane.tabs.ids().contains(&id))
    }

    pub fn by_number(&self, number: usize) -> Option<Rc<Pane>> {
        self.all().into_iter().find(|pane| pane.number == number)
    }

    /// The panel commands act on: the one with the keyboard focus, else the one used last.
    pub fn active(&self) -> Rc<Pane> {
        let panes = self.all();
        let focused = self
            .target
            .load()
            .and_then(|target| target.window())
            .and_then(|window| window.firstResponder())
            .and_then(|responder| Retained::downcast::<NSView>(responder).ok());
        if let Some(view) = focused
            && let Some(pane) = panes.iter().find(|pane| pane.holds_view(&view))
        {
            *pane.focus.borrow_mut() = Some(Weak::from(&*view));
            self.set_active(pane);
            return pane.clone();
        }
        let remembered = self.active.borrow().upgrade();
        remembered
            .filter(|pane| panes.iter().any(|p| Rc::ptr_eq(p, pane)))
            .or_else(|| panes.first().cloned())
            .expect("a window keeps at least one panel")
    }

    pub fn set_active(&self, pane: &Rc<Pane>) {
        let changed = self.active.borrow().upgrade().is_none_or(|old| !Rc::ptr_eq(&old, pane));
        *self.active.borrow_mut() = Rc::downgrade(pane);
        if changed {
            self.mark();
        }
    }

    /// Shows which panel is the active one by its bar, once there are several; one panel looks as it always did.
    fn mark(&self) {
        let panes = self.all();
        let active = self.active.borrow().upgrade();
        for pane in &panes {
            let on = panes.len() > 1 && active.as_ref().is_some_and(|active| Rc::ptr_eq(active, pane));
            pane.tabs.set_active(on);
        }
    }

    /// Makes the panel with the keyboard focus the active one, as the focus moves.
    pub fn follow_focus(&self) {
        let _ = self.active();
    }

    /// Makes `pane` the active panel and gives the keyboard focus to its tab in front, or to the window.
    pub fn activate(&self, pane: &Rc<Pane>) {
        self.set_active(pane);
        let Some(window) = self.target.load().and_then(|target| target.window()) else {
            return;
        };
        let selected = pane.tabs.selected_view();
        let remembered = pane
            .focus
            .borrow()
            .as_ref()
            .and_then(Weak::load)
            .filter(|view| selected.as_deref().is_some_and(|tab| view.isDescendantOf(tab)));
        let view = remembered.or_else(|| selected.as_deref().and_then(first_responder_in));
        match view {
            Some(view) => window.makeFirstResponder(Some(&view)),
            None => window.makeFirstResponder(None),
        };
    }

    /// The split that holds `node` and its place there.
    fn parent_of(&self, matches: &dyn Fn(&Node) -> bool) -> Option<(Rc<Split>, usize)> {
        fn walk(split: &Rc<Split>, matches: &dyn Fn(&Node) -> bool) -> Option<(Rc<Split>, usize)> {
            let children = split.children.borrow();
            if let Some(index) = children.iter().position(matches) {
                return Some((split.clone(), index));
            }
            children.iter().find_map(|child| match child {
                Node::Split(inner) => walk(inner, matches),
                Node::Pane(_) => None,
            })
        }
        walk(&self.root, matches)
    }

    /// A new, empty panel beside `pane`; the panels of a split that runs the same way share it, else `pane` and
    /// the new one split its place.
    pub fn add_beside(&self, pane: &Rc<Pane>, side: Side) -> Option<Rc<Pane>> {
        let target = self.target.load()?;
        let (parent, index) = self.parent_of(&|node| node.is_pane(pane))?;
        let side_by_side = side == Side::Right;
        let new = self.new_pane(&target);
        if parent.children.borrow().len() == 1 {
            parent.controller.splitView().setVertical(side_by_side);
        }
        if parent.side_by_side() == side_by_side {
            parent.insert(index + 1, Node::Pane(new.clone()));
            self.settle();
            parent.set_shares(None);
        } else {
            let split = Split::new(self.mtm, side_by_side);
            let old = parent.remove(index);
            split.insert(0, old);
            split.insert(1, Node::Pane(new.clone()));
            parent.insert(index, Node::Split(split.clone()));
            self.settle();
            split.set_shares(None);
        }
        self.relabel();
        Some(new)
    }

    /// Moves the tab `id` into a new panel beside its own; nothing if it is its panel's only tab.
    pub fn split(&self, id: isize, side: Side) -> Option<Rc<Pane>> {
        let from = self.of_tab(id)?;
        if from.tabs.count() < 2 {
            return None;
        }
        let new = self.add_beside(&from, side)?;
        self.move_tab(id, &new, None);
        Some(new)
    }

    /// Whether the tab `id` can move into a panel of its own: its panel keeps another tab.
    pub fn can_split(&self, id: isize) -> bool {
        self.of_tab(id).is_some_and(|pane| pane.tabs.count() > 1)
    }

    /// Moves the tab `id` into `to` at `index`, or at the end, and drops the panel it leaves empty.
    pub fn move_tab(&self, id: isize, to: &Rc<Pane>, index: Option<usize>) {
        let Some(from) = self.of_tab(id) else { return };
        if Rc::ptr_eq(&from, to) {
            return;
        }
        let Some(tab) = from.tabs.take(id) else { return };
        to.tabs.adopt(tab, index);
        self.set_active(to);
        self.remove_if_empty(&from);
    }

    /// Takes `pane` away once its last tab is gone, unless it is the window's last panel, and closes up the split.
    pub fn remove_if_empty(&self, pane: &Rc<Pane>) {
        if pane.tabs.count() > 0 || self.all().len() < 2 {
            return;
        }
        let Some((parent, index)) = self.parent_of(&|node| node.is_pane(pane)) else {
            return;
        };
        parent.remove(index);
        self.close_up(&parent);
        if self
            .active
            .borrow()
            .upgrade()
            .is_none_or(|active| Rc::ptr_eq(&active, pane))
        {
            let neighbour = self.all().into_iter().next();
            if let Some(neighbour) = neighbour {
                self.set_active(&neighbour);
            }
        }
        self.relabel();
    }

    /// Puts the only child of `split` in its place, and lifts the panels of a lone split under the root into it.
    fn close_up(&self, split: &Rc<Split>) {
        if split.children.borrow().len() != 1 {
            return;
        }
        if Rc::ptr_eq(split, &self.root) {
            let only = split.children.borrow().first().cloned();
            if let Some(Node::Split(inner)) = only {
                split.remove(0);
                split.controller.splitView().setVertical(inner.side_by_side());
                let count = inner.children.borrow().len();
                let shares = inner.shares();
                for index in 0..count {
                    let child = inner.remove(0);
                    split.insert(index, child);
                }
                split.set_shares(Some(&shares));
            }
            return;
        }
        let Some((parent, index)) = self.parent_of(&|node| node.is_split(split)) else {
            return;
        };
        let child = split.remove(0);
        parent.remove(index);
        match child {
            Node::Split(inner) if inner.side_by_side() == parent.side_by_side() => {
                let count = inner.children.borrow().len();
                for offset in 0..count {
                    parent.insert(index + offset, inner.remove(0));
                }
                parent.set_shares(None);
            }
            child => parent.insert(index, child),
        }
        self.close_up(&parent);
    }

    /// Lays out the panels now, so a split knows its size before it shares it out.
    fn settle(&self) {
        self.root.controller.view().layoutSubtreeIfNeeded();
    }

    /// Names each panel's tab bar for VoiceOver, by its place among the window's panels, and marks the active one.
    fn relabel(&self) {
        self.mark();
        let panes = self.all();
        let count = panes.len();
        for (index, pane) in panes.iter().enumerate() {
            if count == 1 {
                pane.tabs.set_label(t("Tabs"));
            } else {
                pane.tabs
                    .set_label(&tf("Panel {} von {}", &[&(index + 1).to_string(), &count.to_string()]));
            }
        }
    }

    /// Where the tab `id` lands when let go at `point`, in window coordinates.
    fn landing(&self, id: isize, point: NSPoint) -> Landing {
        let Some(from) = self.of_tab(id) else {
            return Landing::Nowhere;
        };
        for pane in self.all() {
            if pane.tabs.holds(point) {
                return if Rc::ptr_eq(&pane, &from) {
                    Landing::Reorder
                } else {
                    Landing::Into(pane.number, Some(pane.tabs.index_at(point)))
                };
            }
            let Some(local) = pane.content_point(point) else {
                continue;
            };
            let size = pane.tabs.content().bounds().size;
            let side = if local.x > size.width * (1.0 - EDGE) {
                Some(Side::Right)
            } else if local.y < size.height * EDGE {
                Some(Side::Below)
            } else {
                None
            };
            let own = Rc::ptr_eq(&pane, &from);
            return match side {
                Some(side) if !own || from.tabs.count() > 1 => Landing::Beside(pane.number, side),
                None if !own => Landing::Into(pane.number, None),
                _ => Landing::Nowhere,
            };
        }
        Landing::Nowhere
    }

    /// Follows a tab dragged to `point`: moves it along its own bar, or shows where it would land.
    pub fn drag(&self, id: isize, point: NSPoint) {
        let landing = self.landing(id, point);
        for pane in self.all() {
            let (bar, part) = match landing {
                Landing::Into(number, Some(_)) if number == pane.number => (true, None),
                Landing::Into(number, None) if number == pane.number => (false, Some(None)),
                Landing::Beside(number, side) if number == pane.number => (false, Some(Some(side))),
                _ => (false, None),
            };
            pane.tabs.highlight(bar);
            pane.show_drop(part);
        }
        if landing == Landing::Reorder
            && let Some(from) = self.of_tab(id)
        {
            from.tabs.drag_to(id, point);
        }
    }

    /// Puts down a tab dragged to `point`.
    pub fn drop(&self, id: isize, point: NSPoint) {
        let landing = self.landing(id, point);
        for pane in self.all() {
            pane.tabs.highlight(false);
            pane.show_drop(None);
        }
        match landing {
            Landing::Into(number, index) => {
                if let Some(to) = self.by_number(number) {
                    self.move_tab(id, &to, index);
                }
            }
            Landing::Beside(number, side) => {
                let Some(beside) = self.by_number(number) else { return };
                if let Some(new) = self.add_beside(&beside, side) {
                    self.move_tab(id, &new, None);
                }
            }
            Landing::Reorder | Landing::Nowhere => {}
        }
    }

    /// Moves the keyboard focus to the next panel's tab in front.
    pub fn focus_next(&self) {
        let panes = self.all();
        let active = self.active();
        let index = panes.iter().position(|pane| Rc::ptr_eq(pane, &active)).unwrap_or(0);
        let Some(next) = panes.get((index + 1) % panes.len()).cloned() else {
            return;
        };
        self.activate(&next);
    }

    /// The tree as the layout file keeps it, with `tab` saying how each tab opens again.
    pub fn layout(&self, tab: &dyn Fn(isize) -> Option<ide_core::layout::Tab>) -> ide_core::layout::Node {
        fn node(split: &Split, tab: &dyn Fn(isize) -> Option<ide_core::layout::Tab>) -> ide_core::layout::Node {
            let children: Vec<ide_core::layout::Node> = split
                .children
                .borrow()
                .iter()
                .map(|child| match child {
                    Node::Pane(pane) => panel(pane, tab),
                    Node::Split(inner) => node(inner, tab),
                })
                .collect();
            if children.len() == 1 {
                return children.into_iter().next().expect("one child");
            }
            ide_core::layout::Node::Split {
                side_by_side: split.side_by_side(),
                shares: split.shares(),
                children,
            }
        }
        fn panel(pane: &Pane, tab: &dyn Fn(isize) -> Option<ide_core::layout::Tab>) -> ide_core::layout::Node {
            let selected = pane.tabs.selected_id();
            let mut tabs = vec![];
            let mut index = None;
            for (id, _, _) in pane.tabs.entries() {
                if let Some(entry) = tab(id) {
                    if Some(id) == selected {
                        index = Some(tabs.len());
                    }
                    tabs.push(entry);
                }
            }
            ide_core::layout::Node::Panel { tabs, selected: index }
        }
        node(&self.root, tab)
    }

    /// Gives the splits the shares of `layout`, which the panels were built from.
    pub fn set_shares(&self, layout: &ide_core::layout::Node) {
        self.settle();
        fn apply(split: &Split, layout: &ide_core::layout::Node) {
            let ide_core::layout::Node::Split { shares, children, .. } = layout else {
                return;
            };
            split.set_shares(Some(shares));
            for (child, part) in split.children.borrow().iter().zip(children) {
                if let Node::Split(inner) = child {
                    apply(inner, part);
                }
            }
        }
        apply(&self.root, layout);
    }
}

/// The view at or under `view` to give the keyboard focus: a text view if there is one, as an editor or a composer
/// has, else the first that takes the focus.
pub(super) fn first_responder_in(view: &NSView) -> Option<Retained<NSView>> {
    fn find(view: &NSView, want: &dyn Fn(&NSView) -> bool) -> Option<Retained<NSView>> {
        if want(view) {
            return Some(objc2::Message::retain(view));
        }
        view.subviews().iter().find_map(|sub| find(&sub, want))
    }
    let takes = |view: &NSView| {
        let responder: &NSResponder = view;
        responder.acceptsFirstResponder()
    };
    let text = |view: &NSView| {
        let object: &objc2::runtime::AnyObject = view;
        object.downcast_ref::<objc2_app_kit::NSTextView>().is_some()
    };
    find(view, &|view| text(view) && takes(view)).or_else(|| find(view, &|view| takes(view)))
}
