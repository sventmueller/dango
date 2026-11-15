//! The panes of a window tab (E145): parts side by side or one above the other, nested as deep as the user splits,
//! each holding documents behind one another with a tab bar of its own while the tab is split, in place of the
//! window's. A preview beside its document is no split; it keeps its badge (E63).

mod bar;
mod drag;
mod pane;
#[cfg(feature = "selftest")]
mod selftest_probe;
mod split;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak as RcWeak};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use splat_i18n::tf;

pub use pane::Pane;
use split::Split;

use super::tabs::Document;
use super::{TabId, WindowController};
use crate::util::pin_edges;

/// Where a new pane goes beside another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Below,
}

/// Where a document goes in a window tab.
#[derive(Clone)]
pub enum Place {
    /// Into the active pane, after its document in front.
    Active,
    /// Into `pane` at an index, or after its document in front.
    Into(Rc<Pane>, Option<usize>),
    /// Into a new pane at `side` of the pane that holds `anchor`; `companion` for a preview that follows it.
    Beside { anchor: TabId, side: Side, companion: bool },
}

#[derive(Clone)]
enum Node {
    Pane(Rc<Pane>),
    Split(Rc<Split>),
}

impl Node {
    fn view(&self) -> Retained<NSView> {
        match self {
            Node::Pane(pane) => pane.host.clone(),
            Node::Split(split) => Retained::into_super(Retained::into_super(split.area.clone())),
        }
    }

    fn is_pane(&self, pane: &Rc<Pane>) -> bool {
        matches!(self, Node::Pane(p) if Rc::ptr_eq(p, pane))
    }

    fn is_split(&self, split: &Rc<Split>) -> bool {
        matches!(self, Node::Split(s) if Rc::ptr_eq(s, split))
    }

    fn panes(&self, into: &mut Vec<Rc<Pane>>) {
        match self {
            Node::Pane(pane) => into.push(pane.clone()),
            Node::Split(split) => split.children.borrow().iter().for_each(|child| child.panes(into)),
        }
    }
}

/// The panes of a window tab.
pub struct Panes {
    mtm: MainThreadMarker,
    /// The view the root's view fills.
    frame: Retained<NSView>,
    root: RefCell<Node>,
    active: RefCell<RcWeak<Pane>>,
    /// The pane that fills the window tab alone for now, the others kept hidden.
    zoomed: RefCell<RcWeak<Pane>>,
    next_number: Cell<usize>,
}

impl Panes {
    /// One empty pane, filling `frame`.
    pub fn new(mtm: MainThreadMarker, frame: Retained<NSView>) -> Panes {
        let first = Pane::new(mtm, 1);
        let panes = Panes {
            mtm,
            frame,
            root: RefCell::new(Node::Pane(first.clone())),
            active: RefCell::new(Rc::downgrade(&first)),
            zoomed: RefCell::new(RcWeak::new()),
            next_number: Cell::new(2),
        };
        panes.pin_root();
        panes
    }

    /// Puts the root's view into the frame, once it is out of where it was.
    fn pin_root(&self) {
        let view = self.root.borrow().view();
        self.frame.addSubview(&view);
        pin_edges(&view, &self.frame);
    }

    fn new_pane(&self) -> Rc<Pane> {
        let number = self.next_number.get();
        self.next_number.set(number + 1);
        Pane::new(self.mtm, number)
    }

    /// Every pane, left to right and top to bottom.
    pub fn all(&self) -> Vec<Rc<Pane>> {
        let mut panes = Vec::new();
        self.root.borrow().panes(&mut panes);
        panes
    }

    /// The documents, pane by pane.
    pub fn documents(&self) -> Vec<Rc<Document>> {
        self.all().iter().flat_map(|pane| pane.documents()).collect()
    }

    pub fn of_document(&self, id: TabId) -> Option<Rc<Pane>> {
        self.all().into_iter().find(|pane| pane.holds(id))
    }

    pub fn by_number(&self, number: usize) -> Option<Rc<Pane>> {
        self.all().into_iter().find(|pane| pane.number == number)
    }

    /// The pane commands act on: the one the keyboard focus was in last, else the first.
    pub fn active(&self) -> Rc<Pane> {
        let panes = self.all();
        let remembered = self.active.borrow().upgrade();
        remembered
            .filter(|pane| panes.iter().any(|p| Rc::ptr_eq(p, pane)))
            .or_else(|| panes.first().cloned())
            .expect("a window tab keeps a pane")
    }

    pub fn set_active(&self, pane: &Rc<Pane>) {
        *self.active.borrow_mut() = Rc::downgrade(pane);
    }

    /// The first pane that is not a preview's, which holds the documents of a tab that is not split.
    pub fn main(&self) -> Rc<Pane> {
        let panes = self.all();
        panes
            .iter()
            .find(|pane| pane.companion_of.get().is_none())
            .or_else(|| panes.first())
            .cloned()
            .expect("a window tab keeps a pane")
    }

    /// The "+" of the active pane while the window tab is split and the panes show their bars.
    pub fn active_plus(&self) -> Option<Retained<objc2_app_kit::NSButton>> {
        self.is_split().then(|| self.active().bar.plus.clone())
    }

    /// The pane of the preview that follows the document `owner`.
    pub fn companion_of(&self, owner: TabId) -> Option<Rc<Pane>> {
        self.all()
            .into_iter()
            .find(|pane| pane.companion_of.get() == Some(owner))
    }

    /// Whether the user split the window tab: two panes or more besides the previews that follow a document.
    pub fn is_split(&self) -> bool {
        self.all()
            .iter()
            .filter(|pane| pane.companion_of.get().is_none())
            .count()
            > 1
    }

    /// Puts `document` where `place` says and returns its pane.
    pub fn insert(&self, document: Rc<Document>, place: Place) -> Rc<Pane> {
        let pane = match place {
            Place::Active => self.active_for_documents(),
            Place::Into(pane, index) => {
                pane.insert(document, index);
                return pane;
            }
            Place::Beside {
                anchor,
                side,
                companion,
            } => match self.of_document(anchor) {
                Some(beside) => {
                    let pane = self.add_beside(&beside, side);
                    pane.companion_of.set(companion.then_some(anchor));
                    pane
                }
                None => self.active_for_documents(),
            },
        };
        pane.insert(document, None);
        pane
    }

    /// The active pane, or the one whose document a preview in the active pane follows, which a preview never shares.
    fn active_for_documents(&self) -> Rc<Pane> {
        let active = self.active();
        active
            .companion_of
            .get()
            .and_then(|owner| self.of_document(owner))
            .unwrap_or(active)
    }

    /// Takes the document `id` out; a pane left empty goes, unless it is the last.
    pub fn take(&self, id: TabId) -> Option<Rc<Document>> {
        let pane = self.of_document(id)?;
        let document = pane.take(id);
        if pane.count() == 0 {
            self.remove_pane(&pane);
        }
        document
    }

    /// The split that holds the node `matches` picks and its place there; none for the root.
    fn parent_of(&self, matches: &dyn Fn(&Node) -> bool) -> Option<(Rc<Split>, usize)> {
        fn walk(node: &Node, matches: &dyn Fn(&Node) -> bool) -> Option<(Rc<Split>, usize)> {
            let Node::Split(split) = node else { return None };
            let children = split.children.borrow();
            if let Some(index) = children.iter().position(matches) {
                return Some((split.clone(), index));
            }
            children.iter().find_map(|child| walk(child, matches))
        }
        walk(&self.root.borrow(), matches)
    }

    /// A new, empty pane at `side` of `pane`: it joins a split that runs that way, else `pane` and the new one split
    /// the place of `pane` half and half.
    pub fn add_beside(&self, pane: &Rc<Pane>, side: Side) -> Rc<Pane> {
        self.unzoom();
        let side_by_side = side == Side::Right;
        let new = self.new_pane();
        match self.parent_of(&|node| node.is_pane(pane)) {
            Some((parent, index)) if parent.side_by_side() == side_by_side => {
                let mut shares = parent.shares();
                parent.insert(index + 1, Node::Pane(new.clone()), 0.5);
                let half = shares.get(index).copied().unwrap_or(0.5) / 2.0;
                if let Some(own) = shares.get_mut(index) {
                    *own = half;
                }
                shares.insert((index + 1).min(shares.len()), half);
                parent.set_shares(&shares);
            }
            Some((parent, index)) => {
                let split = Split::new(self.mtm, side_by_side);
                parent.replace(index, Node::Split(split.clone()));
                split.insert(0, Node::Pane(pane.clone()), 1.0);
                split.insert(1, Node::Pane(new.clone()), 0.5);
            }
            None => {
                let split = Split::new(self.mtm, side_by_side);
                let old = self.root.replace(Node::Split(split.clone()));
                old.view().removeFromSuperview();
                self.pin_root();
                split.insert(0, old, 1.0);
                split.insert(1, Node::Pane(new.clone()), 0.5);
            }
        }
        new
    }

    /// Takes `pane` away and closes up its split, unless it is the last pane.
    pub fn remove_pane(&self, pane: &Rc<Pane>) {
        if self.all().len() < 2 {
            return;
        }
        if self
            .zoomed
            .borrow()
            .upgrade()
            .is_some_and(|zoomed| Rc::ptr_eq(&zoomed, pane))
        {
            self.unzoom();
        }
        let Some((parent, index)) = self.parent_of(&|node| node.is_pane(pane)) else {
            return;
        };
        parent.remove(index);
        let order = self.all();
        if self
            .active
            .borrow()
            .upgrade()
            .is_none_or(|active| Rc::ptr_eq(&active, pane))
        {
            let neighbour = parent.children.borrow().get(index.saturating_sub(1)).cloned();
            let mut inside = Vec::new();
            if let Some(neighbour) = neighbour {
                neighbour.panes(&mut inside);
            }
            if let Some(next) = inside.last().cloned().or_else(|| order.first().cloned()) {
                self.set_active(&next);
            }
        }
        self.close_up(&parent);
    }

    /// Puts the only child of `split` in its place, merging it into the split around when it runs the same way.
    fn close_up(&self, split: &Rc<Split>) {
        if split.children.borrow().len() != 1 {
            return;
        }
        let child = split.remove(0);
        match self.parent_of(&|node| node.is_split(split)) {
            None => {
                split.area.removeFromSuperview();
                *self.root.borrow_mut() = child;
                self.pin_root();
            }
            Some((parent, index)) => match child {
                Node::Split(inner) if inner.side_by_side() == parent.side_by_side() => {
                    let shares = parent.shares();
                    let own = shares.get(index).copied().unwrap_or(0.5);
                    let inner_shares = inner.shares();
                    parent.remove(index);
                    let count = inner.children.borrow().len();
                    let mut merged: Vec<f64> = shares.clone();
                    merged.remove(index);
                    for offset in 0..count {
                        let grandchild = inner.remove(0);
                        parent.insert(index + offset, grandchild, 0.5);
                        merged.insert(index + offset, own * inner_shares.get(offset).copied().unwrap_or(0.5));
                    }
                    parent.set_shares(&merged);
                }
                child => {
                    parent.replace(index, child);
                }
            },
        }
    }

    /// Fills the window tab with `pane` alone for now, or shows the others again.
    pub fn toggle_zoom(&self, pane: &Rc<Pane>) {
        let zoomed = self.zoomed.borrow().upgrade();
        *self.zoomed.borrow_mut() = match zoomed {
            Some(zoomed) if Rc::ptr_eq(&zoomed, pane) => RcWeak::new(),
            _ => Rc::downgrade(pane),
        };
    }

    pub fn is_zoomed(&self) -> bool {
        self.zoomed.borrow().upgrade().is_some()
    }

    fn unzoom(&self) {
        *self.zoomed.borrow_mut() = RcWeak::new();
    }

    /// Shows what holds now: which panes show, every pane's tab bar while the window tab is split, else the badge of
    /// a preview beside its document, and which pane is the active one.
    pub fn settle(&self, target: &WindowController) {
        let zoomed = self.zoomed.borrow().upgrade();
        let following = |pane: &Pane| {
            pane.companion_of.get().is_none_or(|owner| {
                self.of_document(owner)
                    .is_none_or(|of| of.front().is_some_and(|front| front.id == owner))
            })
        };
        let shown = |pane: &Rc<Pane>| match &zoomed {
            Some(zoomed) => Rc::ptr_eq(zoomed, pane),
            None => following(pane),
        };
        show_node(&self.root.borrow(), &shown);
        let panes = self.all();
        let bars = self.is_split();
        let active = self.active();
        let several = panes.len() > 1;
        let mut first = true;
        for (index, pane) in panes.iter().enumerate() {
            let visible = !pane.host.isHidden();
            pane.show(target, bars, !first, several && Rc::ptr_eq(pane, &active));
            if visible {
                first = false;
            }
            let label = if several {
                tf(
                    "Bereich {} von {}",
                    &[&(index + 1).to_string(), &panes.len().to_string()],
                )
            } else {
                splat_i18n::t("Tabs").to_string()
            };
            pane.bar.set_label(&label);
        }
    }

    /// The tree as the layout file keeps it, `reopen` saying how each document opens again; none without a document
    /// that does.
    pub fn layout(
        &self,
        reopen: &dyn Fn(&Document) -> Option<ide_core::layout::Tab>,
    ) -> Option<ide_core::layout::Node> {
        fn node(of: &Node, reopen: &dyn Fn(&Document) -> Option<ide_core::layout::Tab>) -> ide_core::layout::Node {
            match of {
                Node::Pane(pane) => {
                    let front = pane.front().map(|doc| doc.id);
                    let mut documents = Vec::new();
                    let mut selected = 0;
                    if pane.companion_of.get().is_none() {
                        for doc in pane.documents() {
                            if let Some(tab) = reopen(&doc) {
                                if Some(doc.id) == front {
                                    selected = documents.len();
                                }
                                documents.push(tab);
                            }
                        }
                    }
                    ide_core::layout::Node::Pane(ide_core::layout::Pane { documents, selected })
                }
                Node::Split(split) => ide_core::layout::Node::Split {
                    side_by_side: split.side_by_side(),
                    children: split
                        .children
                        .borrow()
                        .iter()
                        .map(|child| node(child, reopen))
                        .collect(),
                    shares: split.shares(),
                },
            }
        }
        node(&self.root.borrow(), reopen).normalised()
    }

    /// Divides the window tab, which holds one empty pane, as `layout` says; its panes, empty, each with the
    /// documents of the layout it is to show.
    pub fn build<'a>(&self, layout: &'a ide_core::layout::Node) -> Vec<(Rc<Pane>, &'a ide_core::layout::Pane)> {
        let mut panes = Vec::new();
        let first = self.active();
        match layout {
            ide_core::layout::Node::Pane(saved) => panes.push((first, saved)),
            ide_core::layout::Node::Split { .. } => {
                let built = self.build_node(layout, &mut panes);
                let old = self.root.replace(built);
                old.view().removeFromSuperview();
                self.pin_root();
                if let Some((front, _)) = panes.first() {
                    self.set_active(front);
                }
            }
        }
        panes
    }

    fn build_node<'a>(
        &self,
        layout: &'a ide_core::layout::Node,
        panes: &mut Vec<(Rc<Pane>, &'a ide_core::layout::Pane)>,
    ) -> Node {
        match layout {
            ide_core::layout::Node::Pane(saved) => {
                let pane = self.new_pane();
                panes.push((pane.clone(), saved));
                Node::Pane(pane)
            }
            ide_core::layout::Node::Split {
                side_by_side,
                children,
                shares,
            } => {
                let split = Split::new(self.mtm, *side_by_side);
                for (index, child) in children.iter().enumerate() {
                    let node = self.build_node(child, panes);
                    split.insert(index, node, 0.5);
                }
                split.set_shares(shares);
                Node::Split(split)
            }
        }
    }

    /// Takes away the panes left empty, as a document of the layout did not open again.
    pub fn prune(&self) {
        for pane in self.all() {
            if pane.count() == 0 {
                self.remove_pane(&pane);
            }
        }
    }
}

/// Shows the panes `shown` picks and the splits holding them, hides the rest, and holds each split's shares among
/// the parts that show; whether `node` shows.
fn show_node(node: &Node, shown: &dyn Fn(&Rc<Pane>) -> bool) -> bool {
    let visible = match node {
        Node::Pane(pane) => shown(pane),
        Node::Split(split) => {
            let children = split.children.borrow().clone();
            let mut any = false;
            for child in &children {
                any |= show_node(child, shown);
            }
            split.hold();
            any
        }
    };
    let view = node.view();
    if view.isHidden() == visible {
        view.setHidden(!visible);
    }
    visible
}
