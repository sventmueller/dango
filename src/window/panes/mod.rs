//! The panes of a window tab (E145) as views: they mirror the tree of `ide_core::panes`, which holds every rule of
//! splitting, closing and moving documents, keyed by its pane and split ids. Each pane holds its documents behind one
//! another with a tab bar of its own while the tab is split, in place of the window's. A preview beside its document is
//! no split; it keeps its badge (E63).

mod bar;
mod drag;
mod pane;
#[cfg(feature = "selftest")]
mod selftest_probe;
mod split;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ide_core::panes::{Node, PaneTree, SplitId, Spread};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindowOrderingMode};
use splat_i18n::tf;

pub(crate) use ide_core::panes::{PaneId, Side};
pub(crate) use pane::Pane;
use split::Split;

use super::tabs::Document;
use super::{TabId, WindowController};
use crate::util::pin_edges;

/// Where a document goes in a window tab.
pub(crate) type Place = ide_core::panes::Place<TabId>;

/// The panes of a window tab: the tree and the views that show it.
pub(crate) struct Panes {
    mtm: MainThreadMarker,
    /// The view the root's view fills.
    frame: Retained<NSView>,
    tree: Rc<RefCell<PaneTree<TabId>>>,
    /// The documents the panes hold.
    documents: RefCell<HashMap<TabId, Rc<Document>>>,
    panes: RefCell<HashMap<PaneId, Rc<Pane>>>,
    splits: RefCell<HashMap<SplitId, Rc<Split>>>,
    /// The view that fills the frame now.
    root: RefCell<Retained<NSView>>,
}

impl Panes {
    /// One empty pane, filling `frame`.
    pub(crate) fn new(mtm: MainThreadMarker, frame: Retained<NSView>) -> Panes {
        let tree = PaneTree::new();
        let first = tree.active();
        let pane = Pane::new(mtm, first.number());
        let root = pane.host.clone();
        frame.addSubview(&root);
        pin_edges(&root, &frame);
        Panes {
            mtm,
            frame,
            tree: Rc::new(RefCell::new(tree)),
            documents: RefCell::new(HashMap::new()),
            panes: RefCell::new(HashMap::from([(first, pane)])),
            splits: RefCell::new(HashMap::new()),
            root: RefCell::new(root),
        }
    }

    /// Changes the tree with `change` and brings the views in line with it; what `change` answers.
    fn change<T>(&self, change: impl FnOnce(&mut PaneTree<TabId>) -> T) -> T {
        let answer = change(&mut self.tree.borrow_mut());
        let focus = self.focus_inside();
        self.mirror();
        if let Some(view) = focus {
            self.keep_focus(&view);
        }
        answer
    }

    /// The view in the panes that has the keyboard focus.
    fn focus_inside(&self) -> Option<Retained<NSView>> {
        let responder = self.frame.window()?.firstResponder()?;
        let view = Retained::downcast::<NSView>(responder).ok()?;
        view.isDescendantOf(&self.frame).then_some(view)
    }

    /// Gives the keyboard focus back to `view` while it is in the document commands act on.
    fn keep_focus(&self, view: &NSView) {
        let Some(window) = self.frame.window() else { return };
        // A view that leaves its superview, as a pane does when it moves into a split, leaves the window itself the
        // first responder even when it comes straight back.
        let lost = window
            .firstResponder()
            .is_none_or(|responder| responder.downcast_ref::<NSView>().is_none());
        let in_front = self.focused().is_some_and(|doc| view.isDescendantOf(&doc.view));
        if lost && in_front && view.isDescendantOf(&self.frame) {
            window.makeFirstResponder(Some(view));
        }
    }

    fn read<T>(&self, read: impl FnOnce(&PaneTree<TabId>) -> T) -> T {
        read(&self.tree.borrow())
    }

    fn view(&self, pane: PaneId) -> Option<Rc<Pane>> {
        self.panes.borrow().get(&pane).cloned()
    }

    fn document_of(&self, id: TabId) -> Option<Rc<Document>> {
        self.documents.borrow().get(&id).cloned()
    }

    fn documents_by_id(&self, ids: &[TabId]) -> Vec<Rc<Document>> {
        let documents = self.documents.borrow();
        ids.iter().filter_map(|id| documents.get(id).cloned()).collect()
    }

    /// Every pane, left to right and top to bottom.
    pub(crate) fn all(&self) -> Vec<PaneId> {
        self.read(PaneTree::all)
    }

    /// Whether the window tab has the pane `pane`.
    pub(crate) fn has(&self, pane: PaneId) -> bool {
        self.read(|tree| tree.pane(pane).is_some())
    }

    /// The pane numbered `number`.
    pub(crate) fn by_number(&self, number: usize) -> Option<PaneId> {
        let pane = PaneId::from_number(number);
        self.has(pane).then_some(pane)
    }

    pub(crate) fn count(&self, pane: PaneId) -> usize {
        self.read(|tree| tree.pane(pane).map_or(0, ide_core::panes::Pane::count))
    }

    /// The document in front of `pane`.
    pub(crate) fn front(&self, pane: PaneId) -> Option<Rc<Document>> {
        let id = self.read(|tree| tree.pane(pane)?.front())?;
        self.document_of(id)
    }

    /// The documents of `pane`, in the order of its tab bar.
    pub(crate) fn documents_in(&self, pane: PaneId) -> Vec<Rc<Document>> {
        let ids = self.read(|tree| {
            tree.pane(pane)
                .map(|pane| pane.documents().to_vec())
                .unwrap_or_default()
        });
        self.documents_by_id(&ids)
    }

    /// Whether `pane` holds the preview of a document beside it.
    pub(crate) fn is_companion(&self, pane: PaneId) -> bool {
        self.read(|tree| tree.pane(pane).is_some_and(|pane| pane.companion_of().is_some()))
    }

    /// The documents, pane by pane.
    pub(crate) fn documents(&self) -> Vec<Rc<Document>> {
        let ids = self.read(PaneTree::documents);
        self.documents_by_id(&ids)
    }

    pub(crate) fn document(&self, id: TabId) -> Option<Rc<Document>> {
        self.document_of(id)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.read(PaneTree::is_empty)
    }

    pub(crate) fn of_document(&self, id: TabId) -> Option<PaneId> {
        self.read(|tree| tree.pane_of(id))
    }

    /// The pane commands act on: the one the keyboard focus was in last.
    pub(crate) fn active(&self) -> PaneId {
        self.read(PaneTree::active)
    }

    pub(crate) fn set_active(&self, pane: PaneId) {
        self.tree.borrow_mut().set_active(pane);
    }

    /// The document commands act on: the one in front in the active pane.
    pub(crate) fn focused(&self) -> Option<Rc<Document>> {
        let id = self.read(PaneTree::focused)?;
        self.document_of(id)
    }

    /// The first pane that is not a preview's, which holds the documents of a tab that is not split.
    pub(crate) fn main(&self) -> PaneId {
        self.read(PaneTree::main)
    }

    /// Whether the user split the window tab: two panes or more besides the previews that follow a document.
    pub(crate) fn is_split(&self) -> bool {
        self.read(PaneTree::is_split)
    }

    pub(crate) fn is_zoomed(&self) -> bool {
        self.read(PaneTree::is_zoomed)
    }

    /// The "+" of the active pane while the window tab is split and the panes show their bars.
    pub(crate) fn active_plus(&self) -> Option<Retained<objc2_app_kit::NSButton>> {
        if !self.is_split() {
            return None;
        }
        self.view(self.active()).map(|pane| pane.bar.plus.clone())
    }

    /// The documents that get a native tab of their own, as the window tab is no longer split.
    pub(crate) fn spread(&self) -> Option<Spread<TabId>> {
        self.read(PaneTree::spread)
    }

    /// Puts `document` where `place` says and returns its pane.
    pub(crate) fn insert(&self, document: Rc<Document>, place: Place) -> PaneId {
        let id = document.id;
        self.documents.borrow_mut().insert(id, document);
        self.change(|tree| tree.insert(id, place))
    }

    /// Takes the document `id` out; a pane left empty goes, unless it is the last.
    pub(crate) fn take(&self, id: TabId) -> Option<Rc<Document>> {
        let document = self.documents.borrow_mut().remove(&id)?;
        self.change(|tree| tree.take(id));
        Some(document)
    }

    /// Brings the document `id` to the front of its pane.
    pub(crate) fn select(&self, id: TabId) {
        self.change(|tree| tree.select(id));
    }

    /// Brings the document `id` to the front of its pane and makes that pane the active one.
    pub(crate) fn mark_focused(&self, id: TabId) -> bool {
        self.change(|tree| tree.mark_focused(id))
    }

    /// Brings the document `step` places from the one in front of the active pane to the front; that one.
    pub(crate) fn select_relative(&self, step: isize) -> Option<TabId> {
        self.change(|tree| tree.select_relative(step))
    }

    /// Takes the empty `pane` away and closes up its split, unless it is the last pane.
    pub(crate) fn remove_pane(&self, pane: PaneId) {
        self.change(|tree| tree.remove_pane(pane));
    }

    /// Closes `pane` and moves its documents into the pane before it, else the one after; that pane, none for a
    /// preview's pane and the last one.
    pub(crate) fn close_pane(&self, pane: PaneId) -> Option<PaneId> {
        self.change(|tree| tree.close_pane(pane))
    }

    /// Fills the window tab with `pane` alone for now, or shows the others again.
    pub(crate) fn toggle_zoom(&self, pane: PaneId) {
        self.tree.borrow_mut().toggle_zoom(pane);
    }

    /// Shows every pane again and makes the one `step` places from the active one active; that one.
    pub(crate) fn step_pane(&self, step: isize) -> Option<PaneId> {
        self.tree.borrow_mut().step_pane(step)
    }

    /// Splits the active pane at `side`; the new pane and the document that moved into it.
    pub(crate) fn split_active(&self, side: Side) -> Option<(PaneId, Option<TabId>)> {
        self.change(|tree| tree.split_active(side))
    }

    /// Moves the document `id` into a new pane at `side` of its own, which keeps its other documents; whether it did.
    pub(crate) fn split_off(&self, id: TabId, side: Side) -> bool {
        self.change(|tree| tree.split_off(id, side)).is_some()
    }

    /// The pane `view` is in.
    pub(crate) fn pane_holding(&self, view: &NSView) -> Option<PaneId> {
        let panes = self.panes.borrow();
        self.all()
            .into_iter()
            .find(|id| panes.get(id).is_some_and(|pane| pane.holds_view(view)))
    }

    /// The view to give the keyboard focus to in `pane`: in its document in front, or the empty pane.
    pub(crate) fn focus_target(&self, pane: PaneId) -> Option<Retained<NSView>> {
        let front = self.front(pane);
        self.view(pane)?.focus_target(front.as_deref())
    }

    /// Makes the views match the tree: a view for every pane and split, each split holding its parts' views in their
    /// order, the root's view in the frame, and each pane its documents with the one in front showing. Views of parts
    /// the tree no longer has let go of what they hold.
    fn mirror(&self) {
        let (root, holdings) = self.read(|tree| {
            let holdings: Vec<(PaneId, Vec<TabId>, usize)> = tree
                .all()
                .into_iter()
                .filter_map(|id| {
                    tree.pane(id)
                        .map(|pane| (id, pane.documents().to_vec(), pane.selected()))
                })
                .collect();
            (tree.root().clone(), holdings)
        });
        let live: Vec<PaneId> = holdings.iter().map(|(id, _, _)| *id).collect();
        let gone: Vec<Rc<Pane>> = {
            let mut panes = self.panes.borrow_mut();
            let gone_ids: Vec<PaneId> = panes.keys().filter(|id| !live.contains(id)).copied().collect();
            for id in &live {
                panes.entry(*id).or_insert_with(|| Pane::new(self.mtm, id.number()));
            }
            gone_ids.iter().filter_map(|id| panes.remove(id)).collect()
        };
        for pane in &gone {
            pane.mirror(Vec::new(), 0);
        }
        let gone_splits = self.make_splits(&root);
        self.pin_root(&root);
        for split in root.splits() {
            let view = self.splits.borrow().get(&split.id()).cloned();
            if let Some(view) = view {
                let children: Vec<Retained<NSView>> = split
                    .children()
                    .iter()
                    .filter_map(|child| self.node_view(child))
                    .collect();
                view.arrange(&children);
            }
        }
        for pane in &gone {
            pane.host.removeFromSuperview();
        }
        for split in gone_splits {
            split.area.removeFromSuperview();
        }
        for (id, documents, selected) in holdings {
            if let Some(pane) = self.view(id) {
                pane.mirror(self.documents_by_id(&documents), selected);
            }
        }
    }

    /// Makes a view for each split of `root` that has none and lets go of those of splits gone, which it returns.
    fn make_splits(&self, root: &Node) -> Vec<Rc<Split>> {
        let live: Vec<SplitId> = root.splits().iter().map(|split| split.id()).collect();
        let mut splits = self.splits.borrow_mut();
        for split in root.splits() {
            splits
                .entry(split.id())
                .or_insert_with(|| Split::new(self.mtm, split.side_by_side(), split.id(), Rc::downgrade(&self.tree)));
        }
        let gone: Vec<SplitId> = splits.keys().filter(|id| !live.contains(id)).copied().collect();
        gone.iter().filter_map(|id| splits.remove(id)).collect()
    }

    fn node_view(&self, node: &Node) -> Option<Retained<NSView>> {
        match node {
            Node::Pane(id) => self.view(*id).map(|pane| pane.host.clone()),
            Node::Split(split) => self.splits.borrow().get(&split.id()).map(|split| split.view()),
        }
    }

    /// Puts the view of `root` into the frame in place of the one there, once it differs.
    fn pin_root(&self, root: &Node) {
        let Some(view) = self.node_view(root) else { return };
        let old = self.root.borrow().clone();
        if std::ptr::eq(&*old, &*view) && is_in(&view, &self.frame) {
            return;
        }
        if is_in(&old, &self.frame) {
            old.removeFromSuperview();
        }
        self.frame.addSubview(&view);
        pin_edges(&view, &self.frame);
        *self.root.borrow_mut() = view;
    }

    /// Shows what holds now: which panes show, every pane's tab bar while the window tab is split, else the badge of
    /// a preview beside its document, and which pane is the active one.
    pub(crate) fn settle(&self, target: &WindowController) {
        let (root, shown, bars, active) = self.read(|tree| {
            let shown: Vec<(PaneId, bool)> = tree.all().into_iter().map(|id| (id, tree.is_shown(id))).collect();
            (tree.root().clone(), shown, tree.is_split(), tree.active())
        });
        self.show_node(&root, &|id| shown.iter().any(|(pane, on)| *pane == id && *on));
        let several = shown.len() > 1;
        let mut first = true;
        for (index, (id, _)) in shown.iter().enumerate() {
            let Some(pane) = self.view(*id) else { continue };
            let (documents, selected) = self.read(|tree| {
                tree.pane(*id)
                    .map(|pane| (pane.documents().to_vec(), pane.selected()))
                    .unwrap_or_default()
            });
            let visible = !pane.host.isHidden();
            let documents = self.documents_by_id(&documents);
            pane.show(target, &documents, selected, bars, !first, several && *id == active);
            if visible {
                first = false;
            }
            let label = if several {
                tf(
                    "Bereich {} von {}",
                    &[&(index + 1).to_string(), &shown.len().to_string()],
                )
            } else {
                splat_i18n::t("Tabs").to_string()
            };
            pane.bar.set_label(&label);
        }
    }

    /// Shows the panes `shown` picks and the splits holding them, hides the rest, and holds each split's shares among
    /// the parts that show; whether `node` shows.
    fn show_node(&self, node: &Node, shown: &dyn Fn(PaneId) -> bool) -> bool {
        let (visible, view) = match node {
            Node::Pane(id) => (shown(*id), self.view(*id).map(|pane| pane.host.clone())),
            Node::Split(split) => {
                let mut any = false;
                for child in split.children() {
                    any |= self.show_node(child, shown);
                }
                let view = self.splits.borrow().get(&split.id()).cloned();
                if let Some(view) = &view {
                    view.hold(split.shares());
                }
                (any, view.map(|view| view.view()))
            }
        };
        if let Some(view) = view
            && view.isHidden() == visible
        {
            view.setHidden(!visible);
        }
        visible
    }

    /// The tree as the layout file keeps it, `reopen` saying how each document opens again; none without a document
    /// that does.
    pub(crate) fn layout(
        &self,
        reopen: &dyn Fn(&Document) -> Option<ide_core::layout::Tab>,
    ) -> Option<ide_core::layout::Node> {
        let tree = self.read(PaneTree::clone);
        tree.to_layout(&|id| self.document_of(id).and_then(|document| reopen(&document)))
    }

    /// Divides the window tab, which holds one empty pane, as `layout` says; its panes, empty, each with the
    /// documents of the layout it is to show.
    pub(crate) fn build<'a>(&self, layout: &'a ide_core::layout::Node) -> Vec<(PaneId, &'a ide_core::layout::Pane)> {
        self.change(|tree| tree.build(layout))
    }

    /// Takes away the panes left empty, as a document of the layout did not open again.
    pub(crate) fn prune(&self) {
        self.change(PaneTree::prune);
    }
}

/// Whether `view` sits directly in `parent`.
fn is_in(view: &NSView, parent: &NSView) -> bool {
    // SAFETY: Asking a view for its superview on the main thread, where views live.
    let superview = unsafe { view.superview() };
    superview.is_some_and(|superview| std::ptr::eq(&*superview, parent))
}

/// Puts `views` into `parent` in their order where they are not in it yet, each before the next one that is.
fn place_in_order(parent: &NSView, views: &[Retained<NSView>]) {
    let mut next: Option<&Retained<NSView>> = None;
    for view in views.iter().rev() {
        if !is_in(view, parent) {
            match next {
                Some(after) => parent.addSubview_positioned_relativeTo(view, NSWindowOrderingMode::Below, Some(after)),
                None => parent.addSubview(view),
            }
        }
        next = Some(view);
    }
}
