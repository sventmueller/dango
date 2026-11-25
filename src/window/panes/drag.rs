//! Dragging the tab of a document between the panes of its window tab: along its bar, into another pane or to a pane's
//! right or bottom edge, where it gets a new pane (E145). The views say what the tab is over; the tree where it lands.

use ide_core::panes::{Hit, Landing};
use objc2_foundation::NSPoint;

use super::{Panes, Side};
use crate::window::TabId;

/// The share of a pane's width or height at its right or bottom edge where a dropped tab opens a new pane.
const EDGE: f64 = 0.3;

impl Panes {
    /// What the tab of the document `id` is over at `point`, in window coordinates: the first pane, in reading order,
    /// whose bar or documents hold the point.
    fn hit(&self, id: TabId, point: NSPoint) -> Option<Hit> {
        for pane in self.all() {
            let Some(view) = self.view(pane) else { continue };
            if view.bar.holds(point) {
                return Some(Hit::Bar(pane, view.bar.index_at(point, id)));
            }
            let Some(local) = view.content_point(point) else {
                continue;
            };
            let size = view.content_size();
            let side = if local.x > size.width * (1.0 - EDGE) {
                Some(Side::Right)
            } else if local.y < size.height * EDGE {
                Some(Side::Below)
            } else {
                None
            };
            return Some(Hit::Content(pane, side));
        }
        None
    }

    fn landing(&self, id: TabId, point: NSPoint) -> Landing {
        let hit = self.hit(id, point);
        self.read(|tree| tree.landing(id, hit))
    }

    /// Follows the tab of the document `id` dragged to `point`: shows where it would land.
    pub(crate) fn drag(&self, id: TabId, point: NSPoint) {
        let landing = self.landing(id, point);
        for pane in self.all() {
            let Some(view) = self.view(pane) else { continue };
            let (bar, part) = match landing {
                Landing::Into(into, Some(_)) if into == pane => (true, None),
                Landing::Into(into, None) if into == pane => (false, Some(None)),
                Landing::Beside(beside, side) if beside == pane => (false, Some(Some(side))),
                _ => (false, None),
            };
            view.bar.highlight(bar);
            view.show_drop(part);
        }
    }

    /// Puts down the tab of the document `id` dragged to `point`; whether it moved.
    pub(crate) fn drop(&self, id: TabId, point: NSPoint) -> bool {
        let landing = self.landing(id, point);
        for pane in self.all() {
            if let Some(view) = self.view(pane) {
                view.bar.highlight(false);
                view.show_drop(None);
            }
        }
        self.change(|tree| tree.drop_at(id, landing))
    }
}
