//! The commit graph beside the history, as VS Code draws it: every commit a dot in its lane, lines from each commit
//! down to its parents, a lane's colour kept for as long as the lane runs.

use std::cell::RefCell;

use ide_git::Commit;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBackgroundStyle, NSBezierPath, NSColor, NSTableCellView, NSTableRowView,
    NSTableViewSelectionHighlightStyle, NSUserInterfaceLayoutDirection, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};

use crate::util::guarded;

/// The width of one lane.
pub const LANE: f64 = 12.0;
const DOT_RADIUS: f64 = 3.5;
const LINE_WIDTH: f64 = 1.5;

/// A line in one half of a row: from a lane at one edge to a lane at the other, in the colour of `color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub from: usize,
    pub to: usize,
    pub color: usize,
}

/// How one commit's row of the graph looks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphRow {
    /// The lane of the commit's dot.
    pub lane: usize,
    pub color: usize,
    /// How many lanes the row needs.
    pub width: usize,
    /// Lines from the top edge to the middle; those that end in the dot's lane end at the dot.
    pub top: Vec<Segment>,
    /// Lines from the middle to the bottom edge; those that start in the dot's lane start at the dot.
    pub bottom: Vec<Segment>,
    pub merge: bool,
    /// Whether HEAD points at the commit.
    pub head: bool,
}

impl GraphRow {
    /// The lanes that run on below the commit, as straight lines through rows shown beneath it.
    pub fn through(&self) -> GraphRow {
        let mut lanes: Vec<Segment> = self
            .bottom
            .iter()
            .map(|s| Segment {
                from: s.to,
                to: s.to,
                color: s.color,
            })
            .collect();
        lanes.sort_by_key(|s| s.to);
        lanes.dedup_by_key(|s| s.to);
        GraphRow {
            lane: usize::MAX,
            color: 0,
            width: self.width,
            top: lanes.clone(),
            bottom: lanes,
            merge: false,
            head: false,
        }
    }
}

/// The rows of `commits`, which come children before parents.
pub fn layout(commits: &[Commit]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<(String, usize)>> = Vec::new();
    let mut next_color = 0;
    let mut rows = Vec::with_capacity(commits.len());
    for commit in commits {
        let before = lanes.clone();
        let found = lanes
            .iter()
            .position(|l| l.as_ref().is_some_and(|(h, _)| *h == commit.hash));
        let (lane, color) = match found {
            Some(lane) => (lane, before[lane].as_ref().map_or(0, |(_, c)| *c)),
            None => {
                let color = next_color;
                next_color += 1;
                (free_lane(&mut lanes, None), color)
            }
        };
        let mut top = Vec::new();
        for (index, entry) in before.iter().enumerate() {
            let Some((hash, lane_color)) = entry else { continue };
            let to = if *hash == commit.hash { lane } else { index };
            top.push(Segment {
                from: index,
                to,
                color: *lane_color,
            });
        }
        for entry in &mut lanes {
            if entry.as_ref().is_some_and(|(h, _)| *h == commit.hash) {
                *entry = None;
            }
        }
        let mut bottom = Vec::new();
        for (index, parent) in commit.parents.iter().enumerate() {
            // The first parent keeps the commit's lane even when another lane waits for it already: both lanes run
            // into the parent's dot, and the line of the branch being followed stays straight.
            let existing = lanes
                .iter()
                .position(|l| l.as_ref().is_some_and(|(h, _)| h == parent))
                .filter(|_| index > 0);
            let (to, segment_color) = match existing {
                Some(to) => (to, lanes[to].as_ref().map_or(0, |(_, c)| *c)),
                None if index == 0 => {
                    lanes[lane] = Some((parent.clone(), color));
                    (lane, color)
                }
                None => {
                    let new_color = next_color;
                    next_color += 1;
                    let to = free_lane(&mut lanes, Some(lane));
                    lanes[to] = Some((parent.clone(), new_color));
                    (to, new_color)
                }
            };
            bottom.push(Segment {
                from: lane,
                to,
                color: segment_color,
            });
        }
        for (index, entry) in before.iter().enumerate() {
            if let Some((hash, lane_color)) = entry
                && *hash != commit.hash
            {
                bottom.push(Segment {
                    from: index,
                    to: index,
                    color: *lane_color,
                });
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        let width = before.len().max(lanes.len()).max(lane + 1);
        rows.push(GraphRow {
            lane,
            color,
            width,
            top,
            bottom,
            merge: commit.parents.len() > 1,
            head: commit.refs.iter().any(|r| r == "HEAD" || r.starts_with("HEAD -> ")),
        });
    }
    rows
}

/// The first empty lane other than `except`, or a new one at the end.
fn free_lane(lanes: &mut Vec<Option<(String, usize)>>, except: Option<usize>) -> usize {
    if let Some(index) = (0..lanes.len()).find(|i| lanes[*i].is_none() && Some(*i) != except) {
        return index;
    }
    lanes.push(None);
    lanes.len() - 1
}

/// The colour of lane colour `index`: the accent colour first, then system colours that keep their contrast in
/// light and dark mode (E66).
fn lane_color(index: usize) -> Retained<NSColor> {
    match index % 8 {
        0 => NSColor::controlAccentColor(),
        1 => NSColor::systemPurpleColor(),
        2 => NSColor::systemOrangeColor(),
        3 => NSColor::systemGreenColor(),
        4 => NSColor::systemPinkColor(),
        5 => NSColor::systemTealColor(),
        6 => NSColor::systemIndigoColor(),
        _ => NSColor::systemBrownColor(),
    }
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `GraphView` does not implement Drop.
    /// One row's part of the graph, as tall as the row so the lines meet those of the rows around it.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitGraphView"]
    #[ivars = RefCell<GraphRow>]
    pub struct GraphView;

    unsafe impl NSObjectProtocol for GraphView {}

    impl GraphView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            guarded((), || self.draw());
        }
    }
);

impl GraphView {
    pub fn new(mtm: MainThreadMarker, row: GraphRow) -> Retained<GraphView> {
        let this = Self::alloc(mtm).set_ivars(RefCell::new(row));
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let view: Retained<GraphView> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        view.setAccessibilityElement(false);
        view
    }

    /// The width the row's lanes take.
    pub fn width(row: &GraphRow) -> f64 {
        row.width.max(1) as f64 * LANE
    }

    fn draw(&self) {
        let row = self.ivars().borrow();
        let bounds = self.bounds();
        let height = bounds.size.height;
        let middle = height / 2.0;
        let rtl = self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft;
        let x = |lane: usize| {
            let along = lane as f64 * LANE + LANE / 2.0;
            if rtl { bounds.size.width - along } else { along }
        };
        let gap = |lane: usize| if lane == row.lane { DOT_RADIUS + 1.0 } else { 0.0 };
        for segment in &row.top {
            line(
                NSPoint::new(x(segment.from), 0.0),
                NSPoint::new(x(segment.to), middle - gap(segment.to)),
                segment.color,
            );
        }
        for segment in &row.bottom {
            line(
                NSPoint::new(x(segment.from), middle + gap(segment.from)),
                NSPoint::new(x(segment.to), height),
                segment.color,
            );
        }
        if row.lane == usize::MAX {
            return;
        }
        let color = lane_color(row.color);
        let center = NSPoint::new(x(row.lane), middle);
        let dot = |radius: f64| {
            NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(center.x - radius, center.y - radius),
                NSSize::new(radius * 2.0, radius * 2.0),
            ))
        };
        if row.head {
            let ring = dot(DOT_RADIUS);
            ring.setLineWidth(LINE_WIDTH + 0.5);
            color.setStroke();
            ring.stroke();
        } else if row.merge {
            let ring = dot(DOT_RADIUS);
            ring.setLineWidth(LINE_WIDTH);
            color.setStroke();
            ring.stroke();
            color.setFill();
            dot(DOT_RADIUS - 2.0).fill();
        } else {
            color.setFill();
            dot(DOT_RADIUS).fill();
        }
    }
}

/// How far a selection keeps from the list's edges, and how round its corners are, as the source list draws it.
const SELECTION_INSET: f64 = 10.0;
const SELECTION_RADIUS: f64 = 5.0;

define_class!(
    // SAFETY: NSTableRowView has no subclassing requirements and `GraphRowView` does not implement Drop.
    /// The row of a commit or of a commit's file, whose selection begins after the graph's lanes so it leaves the dot
    /// and the lines visible.
    #[unsafe(super(NSTableRowView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitGraphRowView"]
    pub struct GraphRowView;

    unsafe impl NSObjectProtocol for GraphRowView {}

    impl GraphRowView {
        /// The outline view sets its own style on every row view it adds; AppKit draws a source list's selection
        /// across the row by itself, so the row takes none and draws its own in `drawBackgroundInRect:`.
        #[unsafe(method(setSelectionHighlightStyle:))]
        fn set_selection_highlight_style(&self, _style: NSTableViewSelectionHighlightStyle) {
            // SAFETY: The superclass's own setter, with its signature.
            let _: () = unsafe { msg_send![super(self), setSelectionHighlightStyle: NSTableViewSelectionHighlightStyle::None] };
        }

        #[unsafe(method(setSelected:))]
        fn set_selected(&self, selected: bool) {
            // SAFETY: The superclass's own setter, with its signature.
            let _: () = unsafe { msg_send![super(self), setSelected: selected] };
            self.restyle();
        }

        #[unsafe(method(setEmphasized:))]
        fn set_emphasized(&self, emphasized: bool) {
            // SAFETY: The superclass's own setter, with its signature.
            let _: () = unsafe { msg_send![super(self), setEmphasized: emphasized] };
            self.restyle();
        }

        #[unsafe(method(drawBackgroundInRect:))]
        fn draw_background(&self, dirty: NSRect) {
            guarded((), || {
                // SAFETY: The superclass's own method, with its signature.
                let _: () = unsafe { msg_send![super(self), drawBackgroundInRect: dirty] };
                let Some(rect) = self.selection_rect().filter(|_| self.isSelected()) else {
                    return;
                };
                let color = if self.isEmphasized() {
                    NSColor::selectedContentBackgroundColor()
                } else {
                    NSColor::unemphasizedSelectedContentBackgroundColor()
                };
                color.setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, SELECTION_RADIUS, SELECTION_RADIUS).fill();
            });
        }
    }
);

impl GraphRowView {
    pub fn new(mtm: MainThreadMarker) -> Retained<GraphRowView> {
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![GraphRowView::alloc(mtm), initWithFrame: NSRect::ZERO] }
    }

    /// Redraws the selection and hands the cells the background they stand on, which AppKit does only for a row with
    /// a selection style of its own.
    fn restyle(&self) {
        self.setNeedsDisplay(true);
        let style = if self.isSelected() && self.isEmphasized() {
            NSBackgroundStyle::Emphasized
        } else {
            NSBackgroundStyle::Normal
        };
        for view in self.subviews().iter() {
            if let Some(cell) = view.downcast_ref::<NSTableCellView>() {
                cell.setBackgroundStyle(style);
            }
        }
    }

    /// The selection from the end of the row's lanes, or from the leading inset in a row without them, to the
    /// trailing inset.
    fn selection_rect(&self) -> Option<NSRect> {
        let bounds = self.bounds();
        let (start, end) = (
            bounds.origin.x + SELECTION_INSET,
            bounds.origin.x + bounds.size.width - SELECTION_INSET,
        );
        let rtl = self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft;
        // The dot's lane ends a little past the dot, so the selection may start just inside the lanes.
        let (left, right) = match graph_in(self).map(|g| self.convertRect_fromView(g.bounds(), Some(&g))) {
            Some(lanes) if rtl => (start, lanes.origin.x + 1.0),
            Some(lanes) => (lanes.origin.x + lanes.size.width - 1.0, end),
            None => (start, end),
        };
        (right > left).then(|| {
            NSRect::new(
                NSPoint::new(left, bounds.origin.y),
                NSSize::new(right - left, bounds.size.height),
            )
        })
    }
}

/// The graph somewhere inside `view`.
fn graph_in(view: &NSView) -> Option<Retained<GraphView>> {
    view.subviews()
        .iter()
        .find_map(|child| match child.clone().downcast::<GraphView>() {
            Ok(graph) => Some(graph),
            Err(_) => graph_in(&child),
        })
}

/// A line from `start` to `end`, straight down within a lane or bending over to another one.
fn line(start: NSPoint, end: NSPoint, color: usize) {
    let path = NSBezierPath::bezierPath();
    path.setLineWidth(LINE_WIDTH);
    path.moveToPoint(start);
    if (start.x - end.x).abs() < f64::EPSILON {
        path.lineToPoint(end);
    } else {
        let bend = (start.y + end.y) / 2.0;
        path.curveToPoint_controlPoint1_controlPoint2(end, NSPoint::new(start.x, bend), NSPoint::new(end.x, bend));
    }
    lane_color(color).setStroke();
    path.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str, parents: &[&str]) -> Commit {
        Commit {
            hash: hash.into(),
            parents: parents.iter().map(|p| (*p).to_string()).collect(),
            subject: String::new(),
            body: String::new(),
            author_name: String::new(),
            author_email: String::new(),
            author_time: 0,
            refs: if hash == "m" {
                vec!["HEAD -> main".into()]
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn a_straight_history_stays_in_one_lane() {
        let rows = layout(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1 && r.color == 0));
        assert!(rows[0].top.is_empty());
        assert_eq!(
            rows[1].top,
            [Segment {
                from: 0,
                to: 0,
                color: 0
            }]
        );
        assert!(rows[2].bottom.is_empty());
    }

    #[test]
    fn a_merge_opens_a_lane_for_its_second_parent_that_joins_where_both_meet() {
        // m merges x into b; x and b both come from a.
        let rows = layout(&[
            commit("m", &["b", "x"]),
            commit("x", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ]);
        assert!(rows[0].merge && rows[0].head);
        assert_eq!(
            rows[0].bottom,
            [
                Segment {
                    from: 0,
                    to: 0,
                    color: 0
                },
                Segment {
                    from: 0,
                    to: 1,
                    color: 1
                }
            ]
        );
        assert_eq!((rows[1].lane, rows[1].color), (1, 1));
        assert_eq!(rows[1].width, 2);
        assert_eq!((rows[2].lane, rows[2].color), (0, 0));
        assert!(rows[2].bottom.contains(&Segment {
            from: 1,
            to: 1,
            color: 1
        }));
        assert_eq!(rows[3].lane, 0);
        assert!(rows[3].top.contains(&Segment {
            from: 1,
            to: 0,
            color: 1
        }));
        assert_eq!(rows[3].width, 2);
    }

    #[test]
    fn a_second_tip_starts_its_own_lane_and_rows_beneath_a_commit_carry_its_lanes_on() {
        // u is ahead on the upstream, m on HEAD; both come from a.
        let rows = layout(&[commit("u", &["a"]), commit("m", &["a"]), commit("a", &[])]);
        assert_eq!((rows[0].lane, rows[1].lane), (0, 1));
        assert_ne!(rows[0].color, rows[1].color);
        let through = rows[1].through();
        assert_eq!(through.top, through.bottom);
        assert_eq!(through.top.iter().map(|s| s.to).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(rows[2].top.len(), 2);
    }
}
