//! What the self-test reads from the panes of a window tab, kept out of their production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Panes {
    /// The tree in a line: a pane as its documents' titles in brackets, the one in front starred, the active pane
    /// with `!`; a split as `H(…)` side by side or `V(…)` one above the other.
    pub(crate) fn shape(&self) -> String {
        fn of(panes: &Panes, node: &Node, active: PaneId) -> String {
            match node {
                Node::Pane(id) => {
                    let front = panes.front(*id).map(|doc| doc.id);
                    let documents = panes.documents_in(*id);
                    let titles: Vec<String> = documents
                        .iter()
                        .map(|doc| {
                            let star = if Some(doc.id) == front && documents.len() > 1 {
                                "*"
                            } else {
                                ""
                            };
                            format!("{}{star}", doc.title())
                        })
                        .collect();
                    let mark = if *id == active { "!" } else { "" };
                    format!("[{}]{mark}", titles.join(", "))
                }
                Node::Split(split) => {
                    let kind = if split.side_by_side() { "H" } else { "V" };
                    let children: Vec<String> = split.children().iter().map(|child| of(panes, child, active)).collect();
                    format!("{kind}({})", children.join(" "))
                }
            }
        }
        let root = self.read(|tree| tree.root().clone());
        of(self, &root, self.active())
    }

    /// Whether each pane shows its tab bar, and whether it shows at all, in reading order.
    pub(crate) fn bars_and_shown(&self) -> Vec<(bool, bool)> {
        self.all()
            .into_iter()
            .filter_map(|id| self.view(id))
            .map(|pane| (!pane.bar.view.isHidden(), !pane.host.isHidden()))
            .collect()
    }

    /// The point, in window coordinates, at `x` and `y` of the width and height of the documents of the pane
    /// `number`, measured from its bottom left.
    pub(crate) fn point_in(&self, number: usize, x: f64, y: f64) -> Option<objc2_foundation::NSPoint> {
        let pane = self.view(self.by_number(number)?)?;
        let size = pane.content_size();
        let local = objc2_foundation::NSPoint::new(size.width * x, size.height * y);
        Some(pane.content.convertPoint_toView(local, None))
    }

    /// The root split while it holds two parts side by side.
    fn halves(&self) -> Option<SplitId> {
        self.read(|tree| match tree.root() {
            Node::Split(split) if split.side_by_side() && split.children().len() == 2 => Some(split.id()),
            _ => None,
        })
    }

    /// The share of the first of two parts side by side, while the window tab is split so.
    pub(crate) fn first_share(&self) -> Option<f64> {
        let id = self.halves()?;
        self.read(|tree| tree.split(id)?.shares().first().copied())
    }

    /// The split view of the root while the window tab is split.
    pub(crate) fn root_area(&self) -> Option<Retained<crate::window::document_area::DocumentArea>> {
        let id = self.read(|tree| match tree.root() {
            Node::Split(split) => Some(split.id()),
            Node::Pane(_) => None,
        })?;
        self.splits.borrow().get(&id).map(|split| split.area.clone())
    }

    /// Gives the first of two parts side by side `share` of the width.
    pub(crate) fn set_first_share(&self, share: f64) {
        let Some(id) = self.halves() else { return };
        let share = share.clamp(0.1, 0.9);
        self.tree.borrow_mut().set_shares(id, &[share, 1.0 - share]);
        let split = self.splits.borrow().get(&id).cloned();
        let shares = self.read(|tree| tree.split(id).map(|split| split.shares().to_vec()));
        if let (Some(split), Some(shares)) = (split, shares) {
            split.hold(&shares);
        }
    }
}
