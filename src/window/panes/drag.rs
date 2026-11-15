//! Moving a document between the panes of its window tab: dragging its tab along the bar, into another pane or to a
//! pane's right or bottom edge, where it gets a new pane (E145).

use std::rc::Rc;

use objc2_foundation::NSPoint;

use super::{Pane, Panes, Side};
use crate::window::TabId;

/// The share of a pane's width or height at its right or bottom edge where a dropped tab opens a new pane.
const EDGE: f64 = 0.3;

/// Where a dragged tab would land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Landing {
    /// Into the pane numbered so, at a place in its bar, or after its document in front.
    Into(usize, Option<usize>),
    /// Into a new pane beside the one numbered so.
    Beside(usize, Side),
    Nowhere,
}

impl Panes {
    /// Moves the document `id` into `to` at `index`, or after its document in front, and drops the pane it leaves
    /// empty; along its own bar it only moves to `index`.
    pub fn move_document(&self, id: TabId, to: &Rc<Pane>, index: Option<usize>) {
        let Some(from) = self.of_document(id) else { return };
        let Some(document) = from.take(id) else { return };
        to.insert(document, index);
        self.set_active(to);
        if !Rc::ptr_eq(&from, to) && from.count() == 0 {
            self.remove_pane(&from);
        }
    }

    /// Moves the document `id` into a new pane at `side` of its own, which keeps its other documents; whether it did.
    pub fn split_off(&self, id: TabId, side: Side) -> bool {
        let Some(from) = self.of_document(id) else { return false };
        if from.count() < 2 {
            return false;
        }
        let new = self.add_beside(&from, side);
        self.move_document(id, &new, None);
        true
    }

    /// Where the document `id` lands when let go at `point`, in window coordinates.
    fn landing(&self, id: TabId, point: NSPoint) -> Landing {
        let Some(from) = self.of_document(id) else {
            return Landing::Nowhere;
        };
        for pane in self.all() {
            if pane.bar.holds(point) {
                return Landing::Into(pane.number, Some(pane.bar.index_at(point, id)));
            }
            let Some(local) = pane.content_point(point) else {
                continue;
            };
            let size = pane.content_size();
            let side = if local.x > size.width * (1.0 - EDGE) {
                Some(Side::Right)
            } else if local.y < size.height * EDGE {
                Some(Side::Below)
            } else {
                None
            };
            let own = Rc::ptr_eq(&pane, &from);
            return match side {
                Some(side) if !own || from.count() > 1 => Landing::Beside(pane.number, side),
                None if !own => Landing::Into(pane.number, None),
                _ => Landing::Nowhere,
            };
        }
        Landing::Nowhere
    }

    /// Follows the tab of the document `id` dragged to `point`: shows where it would land.
    pub fn drag(&self, id: TabId, point: NSPoint) {
        let landing = self.landing(id, point);
        for pane in self.all() {
            let (bar, part) = match landing {
                Landing::Into(number, Some(_)) if number == pane.number => (true, None),
                Landing::Into(number, None) if number == pane.number => (false, Some(None)),
                Landing::Beside(number, side) if number == pane.number => (false, Some(Some(side))),
                _ => (false, None),
            };
            pane.bar.highlight(bar);
            pane.show_drop(part);
        }
    }

    /// Puts down the tab of the document `id` dragged to `point`; whether it moved.
    pub fn drop(&self, id: TabId, point: NSPoint) -> bool {
        let landing = self.landing(id, point);
        for pane in self.all() {
            pane.bar.highlight(false);
            pane.show_drop(None);
        }
        match landing {
            Landing::Into(number, index) => match self.by_number(number) {
                Some(to) => {
                    self.move_document(id, &to, index);
                    true
                }
                None => false,
            },
            Landing::Beside(number, side) => {
                let Some(beside) = self.by_number(number) else {
                    return false;
                };
                let new = self.add_beside(&beside, side);
                self.move_document(id, &new, None);
                true
            }
            Landing::Nowhere => false,
        }
    }
}
