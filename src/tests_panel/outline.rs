//! What the Tests panel's outline asks its data source, answered from an index built once per tree: each row's
//! children, and one object per node that stays the same while the panel lives.

use std::collections::HashMap;

use ide_test::board::{Node, Row};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::NSNumber;

#[derive(Default)]
pub(super) struct Outline {
    top: Vec<usize>,
    children: Vec<Vec<usize>>,
    /// The serial of each row's node.
    serial_of: Vec<usize>,
    /// The row of each serial shown now.
    row_of: HashMap<usize, usize>,
    serials: HashMap<Node, usize>,
    /// The object standing for each serial in the outline.
    items: Vec<Retained<NSNumber>>,
}

impl Outline {
    /// Builds the index of `rows`, packages before their files and tests.
    pub(super) fn index(&mut self, rows: &[Row]) {
        self.top.clear();
        self.children = vec![Vec::new(); rows.len()];
        self.serial_of.clear();
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
            let next = self.items.len();
            let serial = *self.serials.entry(row.node.clone()).or_insert(next);
            if serial == next {
                self.items.push(NSNumber::new_usize(serial));
            }
            self.serial_of.push(serial);
            self.row_of.insert(serial, at);
        }
    }

    /// The rows one level under `row`, or the packages for none.
    pub(super) fn children(&self, row: Option<usize>) -> &[usize] {
        match row {
            None => &self.top,
            Some(row) => self.children.get(row).map_or(&[], Vec::as_slice),
        }
    }

    /// The rows one level under the row `item` stands for, or the packages for none; `None` where `item` stands for
    /// no row.
    pub(super) fn children_of(&self, item: Option<&AnyObject>) -> Option<&[usize]> {
        match item {
            None => Some(&self.top),
            Some(item) => Some(self.children(Some(self.row_of(item)?))),
        }
    }

    /// The row `item` stands for.
    pub(super) fn row_of(&self, item: &AnyObject) -> Option<usize> {
        let serial = item.downcast_ref::<NSNumber>()?.unsignedIntegerValue();
        self.row_of.get(&serial).copied()
    }

    /// The object that stands for `row`.
    pub(super) fn item(&self, row: usize) -> Option<Retained<NSNumber>> {
        self.items.get(*self.serial_of.get(row)?).cloned()
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
        let file = outline.item(1).unwrap();
        let file: &AnyObject = &file;
        assert_eq!(outline.children_of(Some(file)), Some(&[2, 3][..]));
        assert_eq!(outline.children_of(None), Some(&[0, 4][..]));
        let item = outline.item(3).unwrap();
        outline.index(&rows[2..]);
        assert_eq!(outline.item(1).unwrap(), item, "a node keeps its object");
        assert_eq!(outline.row_of(&item), Some(1));
        assert!(same_shape(&rows, &rows) && !same_shape(&rows, &rows[1..]));
    }
}
