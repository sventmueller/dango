//! What the Tests panel's outline asks its data source, answered from an index built once per tree: each row's
//! children, and the row of each node.

use std::collections::HashMap;

use ide_test::board::{Node, Row};

#[derive(Default)]
pub(super) struct Outline {
    top: Vec<usize>,
    children: Vec<Vec<usize>>,
    /// The node of each row.
    nodes: Vec<Node>,
    /// The row of each node shown now.
    row_of: HashMap<Node, usize>,
}

impl Outline {
    /// Builds the index of `rows`, packages before their files and tests.
    pub(super) fn index(&mut self, rows: &[Row]) {
        self.top.clear();
        self.children = vec![Vec::new(); rows.len()];
        self.nodes.clear();
        self.row_of.clear();
        let mut open: Vec<usize> = Vec::new();
        for (at, row) in rows.iter().enumerate() {
            while open.last().is_some_and(|&p| rows[p].level >= row.level) {
                open.pop();
            }
            match open.last() {
                None if row.level == 0 => self.top.push(at),
                Some(&parent) if rows[parent].level + 1 == row.level => self.children[parent].push(at),
                _ => {}
            }
            open.push(at);
            self.nodes.push(row.node.clone());
            self.row_of.insert(row.node.clone(), at);
        }
    }

    /// The rows one level under `row`, or the packages for none.
    pub(super) fn children(&self, row: Option<usize>) -> &[usize] {
        match row {
            None => &self.top,
            Some(row) => self.children.get(row).map_or(&[], Vec::as_slice),
        }
    }

    /// The rows one level under the row of `node`, or the packages for none; `None` where `node` has no row.
    pub(super) fn children_of(&self, node: Option<&Node>) -> Option<&[usize]> {
        match node {
            None => Some(&self.top),
            Some(node) => Some(self.children(Some(self.row_of(node)?))),
        }
    }

    /// The row of `node`.
    pub(super) fn row_of(&self, node: &Node) -> Option<usize> {
        self.row_of.get(node).copied()
    }

    /// The node of `row`.
    pub(super) fn node(&self, row: usize) -> Option<&Node> {
        self.nodes.get(row)
    }
}

/// Whether `old` and `new` have the same nodes at the same levels, so the outline keeps its rows.
pub(super) fn same_shape(old: &[Row], new: &[Row]) -> bool {
    old.len() == new.len() && old.iter().zip(new).all(|(a, b)| a.level == b.level && a.node == b.node)
}

#[cfg(test)]
mod tests {
    use ide_test::{State, Tool};

    use super::*;

    fn row(level: usize, node: Node) -> Row {
        Row {
            level,
            label: String::new(),
            state: State::NotRun,
            node,
        }
    }

    #[test]
    fn each_row_has_the_rows_one_level_below_until_its_level_comes_again() {
        let rows = [
            row(0, Node::Package(Tool::Go, "a".into())),
            row(1, Node::File(Tool::Go, "a".into(), "a_test.go".into())),
            row(2, Node::Test(0)),
            row(2, Node::Test(1)),
            row(0, Node::Package(Tool::Go, "b".into())),
            row(1, Node::Test(2)),
        ];
        let mut outline = Outline::default();
        outline.index(&rows);
        assert_eq!(outline.children(None), [0, 4]);
        assert_eq!(outline.children(Some(0)), [1]);
        assert_eq!(outline.children(Some(1)), [2, 3]);
        assert_eq!(outline.children(Some(4)), [5]);
        assert!(outline.children(Some(5)).is_empty());
        let file = outline.node(1).unwrap().clone();
        assert_eq!(outline.children_of(Some(&file)), Some(&[2, 3][..]));
        assert_eq!(outline.children_of(None), Some(&[0, 4][..]));
        assert_eq!(outline.children_of(Some(&Node::Test(9))), None);
        let node = outline.node(3).unwrap().clone();
        outline.index(&rows[2..]);
        assert_eq!(outline.row_of(&node), Some(1), "a node is found in its new row");
        assert!(same_shape(&rows, &rows) && !same_shape(&rows, &rows[1..]));
    }
}
