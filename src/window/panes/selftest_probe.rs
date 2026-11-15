//! What the self-test reads from the panes of a window tab, kept out of their production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Panes {
    /// The tree in a line: a pane as its documents' titles in brackets, the one in front starred, the active pane
    /// with `!`; a split as `H(…)` side by side or `V(…)` one above the other.
    pub fn shape(&self) -> String {
        fn of(node: &Node, active: &Rc<Pane>) -> String {
            match node {
                Node::Pane(pane) => {
                    let front = pane.front().map(|doc| doc.id);
                    let titles: Vec<String> = pane
                        .documents()
                        .iter()
                        .map(|doc| {
                            let star = if Some(doc.id) == front && pane.count() > 1 {
                                "*"
                            } else {
                                ""
                            };
                            format!("{}{star}", doc.title())
                        })
                        .collect();
                    let mark = if Rc::ptr_eq(pane, active) { "!" } else { "" };
                    format!("[{}]{mark}", titles.join(", "))
                }
                Node::Split(split) => {
                    let kind = if split.side_by_side() { "H" } else { "V" };
                    let children: Vec<String> = split.children.borrow().iter().map(|child| of(child, active)).collect();
                    format!("{kind}({})", children.join(" "))
                }
            }
        }
        of(&self.root.borrow(), &self.active())
    }

    /// Whether each pane shows its tab bar, and whether it shows at all, in reading order.
    pub fn bars_and_shown(&self) -> Vec<(bool, bool)> {
        self.all()
            .iter()
            .map(|pane| (!pane.bar.view.isHidden(), !pane.host.isHidden()))
            .collect()
    }

    /// The point, in window coordinates, at `x` and `y` of the width and height of the documents of the pane
    /// `number`, measured from its bottom left.
    pub fn point_in(&self, number: usize, x: f64, y: f64) -> Option<objc2_foundation::NSPoint> {
        let pane = self.by_number(number)?;
        let size = pane.content_size();
        let local = objc2_foundation::NSPoint::new(size.width * x, size.height * y);
        Some(pane.content.convertPoint_toView(local, None))
    }

    /// The share of the first of two parts side by side, while the window tab is split so.
    pub fn first_share(&self) -> Option<f64> {
        match &*self.root.borrow() {
            Node::Split(split) if split.side_by_side() && split.children.borrow().len() == 2 => {
                split.shares().first().copied()
            }
            _ => None,
        }
    }

    /// The split view of the root while the window tab is split.
    pub fn root_area(&self) -> Option<Retained<crate::window::document_area::DocumentArea>> {
        match &*self.root.borrow() {
            Node::Split(split) => Some(split.area.clone()),
            Node::Pane(_) => None,
        }
    }

    /// Gives the first of two parts side by side `share` of the width.
    pub fn set_first_share(&self, share: f64) {
        if let Node::Split(split) = &*self.root.borrow()
            && split.children.borrow().len() == 2
        {
            let share = share.clamp(0.1, 0.9);
            split.set_shares(&[share, 1.0 - share]);
            split.hold();
        }
    }
}
