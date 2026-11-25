//! The view of a split of a window tab: its parts side by side or one above the other, each holding its share of the
//! room as the tab changes size; only a drag of a divider changes the shares, which go back into the tree (E63, E145).

use std::cell::RefCell;
use std::rc::{Rc, Weak as RcWeak};

use ide_core::panes::{PaneTree, SplitId};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSLayoutConstraint, NSSplitViewDividerStyle, NSView};

use super::{is_in, place_in_order};
use crate::util::activate;
use crate::window::TabId;
use crate::window::document_area::DocumentArea;

/// The priority that holds a share: above the split view's holding priority, below a drag of its divider.
const SHARE_PRIORITY: f32 = 260.0;
/// The narrowest a part gets beside another.
const MIN_WIDTH: f64 = 220.0;
/// The lowest a part gets above or below another.
const MIN_HEIGHT: f64 = 120.0;

pub(super) struct Split {
    pub(super) area: Retained<DocumentArea>,
    /// The views of the parts, in their order, as the split holds them now.
    children: RefCell<Vec<Retained<NSView>>>,
    holding: RefCell<Vec<Retained<NSLayoutConstraint>>>,
}

impl Split {
    /// The view of the split `id` of `tree`, which takes the shares the user drags its divider to.
    pub(super) fn new(
        mtm: MainThreadMarker,
        side_by_side: bool,
        id: SplitId,
        tree: RcWeak<RefCell<PaneTree<TabId>>>,
    ) -> Rc<Split> {
        let area = DocumentArea::new(mtm);
        area.setVertical(side_by_side);
        area.setDividerStyle(NSSplitViewDividerStyle::Thin);
        let split = Rc::new(Split {
            area,
            children: RefCell::new(Vec::new()),
            holding: RefCell::new(Vec::new()),
        });
        let dragged: RcWeak<Split> = Rc::downgrade(&split);
        split.area.on_drag(move || {
            let (Some(split), Some(tree)) = (dragged.upgrade(), tree.upgrade()) else {
                return;
            };
            let shares = tree.borrow().split(id).map(|split| split.shares().to_vec());
            let Some(dragged) = shares.and_then(|shares| split.dragged_shares(&shares)) else {
                return;
            };
            tree.borrow_mut().set_shares(id, &dragged);
            let shares = tree.borrow().split(id).map(|split| split.shares().to_vec());
            if let Some(shares) = shares {
                split.hold(&shares);
            }
        });
        split
    }

    pub(super) fn view(&self) -> Retained<NSView> {
        Retained::into_super(Retained::into_super(self.area.clone()))
    }

    fn side_by_side(&self) -> bool {
        self.area.isVertical()
    }

    /// Holds `children` in their order: views that left the split go, new ones come in at their place.
    pub(super) fn arrange(&self, children: &[Retained<NSView>]) {
        let area: &NSView = &self.area;
        let before = self.children.borrow().clone();
        let same = before.len() == children.len()
            && before.iter().zip(children).all(|(a, b)| std::ptr::eq(&**a, &**b))
            && children.iter().all(|child| is_in(child, area));
        if same {
            return;
        }
        for old in &before {
            if !children.iter().any(|child| std::ptr::eq(&**child, &**old)) && is_in(old, area) {
                old.removeFromSuperview();
            }
        }
        place_in_order(area, children);
        *self.children.borrow_mut() = children.to_vec();
    }

    /// Holds the children that show at their `shares` among one another, and at their least size.
    pub(super) fn hold(&self, shares: &[f64]) {
        for constraint in self.holding.borrow_mut().drain(..) {
            constraint.setActive(false);
        }
        let shown: Vec<(Retained<NSView>, f64)> = self
            .children
            .borrow()
            .iter()
            .zip(shares.iter())
            .map(|(child, share)| (child.clone(), *share))
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
        activate(holding.iter().cloned());
        *self.holding.borrow_mut() = holding;
        self.area.adjustSubviews();
    }

    /// The shares the children that show have now, as the user dragged a divider, from `shares` before; hidden ones
    /// keep theirs.
    fn dragged_shares(&self, shares: &[f64]) -> Option<Vec<f64>> {
        let side_by_side = self.side_by_side();
        let length = |view: &NSView| {
            let size = view.frame().size;
            if side_by_side { size.width } else { size.height }
        };
        let measured: Vec<Option<f64>> = self
            .children
            .borrow()
            .iter()
            .map(|view| (!view.isHidden()).then(|| length(view)))
            .collect();
        let shown_share: f64 = shares
            .iter()
            .zip(&measured)
            .filter(|(_, length)| length.is_some())
            .map(|(share, _)| share)
            .sum();
        let shown_length: f64 = measured.iter().flatten().sum();
        if shown_length <= 1.0 || measured.len() != shares.len() {
            return None;
        }
        Some(
            shares
                .iter()
                .zip(&measured)
                .map(|(share, length)| match length {
                    Some(length) => (length / shown_length * shown_share).max(0.02),
                    None => *share,
                })
                .collect(),
        )
    }
}
