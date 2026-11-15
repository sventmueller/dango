//! Parts of a window tab side by side or one above the other, each holding its share of the room as the tab changes
//! size; only a drag of a divider changes the shares (E63, E145).

use std::cell::RefCell;
use std::rc::{Rc, Weak as RcWeak};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSLayoutConstraint, NSSplitViewDividerStyle, NSView, NSWindowOrderingMode};

use super::Node;
use crate::window::document_area::DocumentArea;

/// The priority that holds a share: above the split view's holding priority, below a drag of its divider.
const SHARE_PRIORITY: f32 = 260.0;
/// The narrowest a part gets beside another.
const MIN_WIDTH: f64 = 220.0;
/// The lowest a part gets above or below another.
const MIN_HEIGHT: f64 = 120.0;

pub(super) struct Split {
    pub(super) area: Retained<DocumentArea>,
    pub(super) children: RefCell<Vec<Node>>,
    /// Each child's share of the room along the split, adding up to 1.
    shares: RefCell<Vec<f64>>,
    holding: RefCell<Vec<Retained<NSLayoutConstraint>>>,
}

impl Split {
    pub(super) fn new(mtm: MainThreadMarker, side_by_side: bool) -> Rc<Split> {
        let area = DocumentArea::new(mtm);
        area.setVertical(side_by_side);
        area.setDividerStyle(NSSplitViewDividerStyle::Thin);
        let split = Rc::new(Split {
            area,
            children: RefCell::new(Vec::new()),
            shares: RefCell::new(Vec::new()),
            holding: RefCell::new(Vec::new()),
        });
        let dragged: RcWeak<Split> = Rc::downgrade(&split);
        split.area.on_drag(move || {
            if let Some(split) = dragged.upgrade() {
                split.take_dragged_shares();
            }
        });
        split
    }

    pub(super) fn side_by_side(&self) -> bool {
        self.area.isVertical()
    }

    pub(super) fn shares(&self) -> Vec<f64> {
        self.shares.borrow().clone()
    }

    /// Puts `node` at `index` with `share` of the room, taken from the others in proportion.
    pub(super) fn insert(&self, index: usize, node: Node, share: f64) {
        let index = index.min(self.children.borrow().len());
        let view = node.view();
        match self.children.borrow().get(index) {
            Some(after) => {
                self.area
                    .addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, Some(&after.view()))
            }
            None => self.area.addSubview(&view),
        }
        self.children.borrow_mut().insert(index, node);
        let mut shares = self.shares.borrow_mut();
        let share = if shares.is_empty() {
            1.0
        } else {
            share.clamp(0.05, 0.95)
        };
        for other in shares.iter_mut() {
            *other *= 1.0 - share;
        }
        shares.insert(index, share);
    }

    /// Takes the child at `index` out; its share goes to the neighbour before it, else the one after.
    pub(super) fn remove(&self, index: usize) -> Node {
        let node = self.children.borrow_mut().remove(index);
        node.view().removeFromSuperview();
        let mut shares = self.shares.borrow_mut();
        let freed = shares.remove(index);
        let neighbour = index.checked_sub(1).or((!shares.is_empty()).then_some(0));
        if let Some(neighbour) = neighbour.and_then(|at| shares.get_mut(at)) {
            *neighbour += freed;
        }
        node
    }

    /// Puts `node` in the place of the child at `index`, with its share.
    pub(super) fn replace(&self, index: usize, node: Node) -> Node {
        let old = std::mem::replace(&mut self.children.borrow_mut()[index], node.clone());
        self.area.replaceSubview_with(&old.view(), &node.view());
        old
    }

    pub(super) fn set_shares(&self, shares: &[f64]) {
        let count = self.children.borrow().len();
        let total: f64 = shares.iter().sum();
        if shares.len() == count && total > 0.0 && shares.iter().all(|share| share.is_finite() && *share > 0.0) {
            *self.shares.borrow_mut() = shares.iter().map(|share| share / total).collect();
        }
    }

    /// Holds the children that show at their shares among one another, and at their least size.
    pub(super) fn hold(&self) {
        for constraint in self.holding.borrow_mut().drain(..) {
            constraint.setActive(false);
        }
        let children = self.children.borrow();
        let shares = self.shares.borrow();
        let shown: Vec<(Retained<NSView>, f64)> = children
            .iter()
            .zip(shares.iter())
            .map(|(child, share)| (child.view(), *share))
            .filter(|(view, _)| !view.isHidden())
            .collect();
        if shown.len() < 2 {
            return;
        }
        let total: f64 = shown.iter().map(|(_, share)| share).sum();
        let side_by_side = self.side_by_side();
        let mut holding = Vec::with_capacity(shown.len() * 2);
        for (index, (view, share)) in shown.iter().enumerate() {
            if index + 1 < shown.len() {
                let held = if side_by_side {
                    view.widthAnchor()
                        .constraintEqualToAnchor_multiplier(&self.area.widthAnchor(), share / total)
                } else {
                    view.heightAnchor()
                        .constraintEqualToAnchor_multiplier(&self.area.heightAnchor(), share / total)
                };
                held.setPriority(SHARE_PRIORITY);
                holding.push(held);
            }
            let least = if side_by_side {
                view.widthAnchor().constraintGreaterThanOrEqualToConstant(MIN_WIDTH)
            } else {
                view.heightAnchor().constraintGreaterThanOrEqualToConstant(MIN_HEIGHT)
            };
            least.setPriority(SHARE_PRIORITY + 10.0);
            holding.push(least);
        }
        for constraint in &holding {
            constraint.setActive(true);
        }
        *self.holding.borrow_mut() = holding;
        drop(children);
        self.area.adjustSubviews();
    }

    /// The shares the children that show have now, as the user dragged a divider; hidden ones keep theirs.
    fn take_dragged_shares(&self) {
        let side_by_side = self.side_by_side();
        let length = |view: &NSView| {
            let size = view.frame().size;
            if side_by_side { size.width } else { size.height }
        };
        let measured: Vec<Option<f64>> = self
            .children
            .borrow()
            .iter()
            .map(|child| {
                let view = child.view();
                (!view.isHidden()).then(|| length(&view))
            })
            .collect();
        let mut shares = self.shares.borrow_mut();
        let shown_share: f64 = shares
            .iter()
            .zip(&measured)
            .filter(|(_, length)| length.is_some())
            .map(|(share, _)| share)
            .sum();
        let shown_length: f64 = measured.iter().flatten().sum();
        if shown_length <= 1.0 {
            return;
        }
        for (share, length) in shares.iter_mut().zip(&measured) {
            if let Some(length) = length {
                *share = (length / shown_length * shown_share).max(0.02);
            }
        }
        drop(shares);
        self.hold();
    }
}
