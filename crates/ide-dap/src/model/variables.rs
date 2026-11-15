//! What the inspector shows of the selected frame: its scopes with their variables, whose children the adapter sends
//! only when a row opens, and the watch expressions with their values.

/// A scope, a variable or the value of a watch expression, with its children once the adapter sent them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Node {
    pub name: String,
    /// Its value as the adapter shows it.
    pub value: String,
    /// Its type, if the adapter says.
    pub kind: Option<String>,
    /// The adapter's handle for its children; 0 for none.
    pub(crate) reference: i64,
    /// Its children, `None` until they were asked for and came.
    pub(crate) children: Option<Vec<Node>>,
}

impl Node {
    /// A node without type or children yet.
    pub fn new(name: impl Into<String>, value: impl Into<String>, reference: i64) -> Node {
        Node {
            name: name.into(),
            value: value.into(),
            kind: None,
            reference,
            children: None,
        }
    }

    /// Whether the adapter can send children for it.
    pub(crate) fn has_children(&self) -> bool {
        self.reference > 0
    }
}

/// A watch expression and what it came to in the selected frame.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Watch {
    pub expression: String,
    /// Its value, or the adapter's error, once evaluated; `None` while nothing is paused.
    pub value: Option<Result<Node, String>>,
}

/// Where a row of the inspector is: a scope with the names below it, or a watch expression by its place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Path {
    /// A scope's variable, by the names from the scope down.
    Scope(Vec<String>),
    /// A watch expression by its place, and the names below it.
    Watch(usize, Vec<String>),
}

impl Path {
    /// The path of the child `name` of this row.
    pub(crate) fn child(&self, name: &str) -> Path {
        let mut path = self.clone();
        match &mut path {
            Path::Scope(names) | Path::Watch(_, names) => names.push(name.to_string()),
        }
        path
    }
}

/// The scopes and watches of the selected frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Variables {
    /// Counts the frames shown, so an answer about an earlier one is dropped.
    pub(crate) generation: u64,
    /// The session and frame they belong to.
    pub frame: Option<(u64, i64)>,
    /// The frame's scopes, `None` until the adapter sent them.
    pub scopes: Option<Vec<Node>>,
    /// The watch expressions.
    pub watches: Vec<Watch>,
}

impl Variables {
    /// An inspector with `watches` and nothing shown.
    pub(crate) fn new(watches: Vec<String>) -> Variables {
        Variables {
            watches: watches
                .into_iter()
                .map(|expression| Watch {
                    expression,
                    value: None,
                })
                .collect(),
            ..Variables::default()
        }
    }

    /// Starts over for `frame`, or for none when nothing is paused; the new generation.
    pub(crate) fn show(&mut self, frame: Option<(u64, i64)>) -> u64 {
        self.generation += 1;
        self.frame = frame;
        self.scopes = None;
        for watch in &mut self.watches {
            watch.value = None;
        }
        self.generation
    }

    /// The watch expressions, as kept.
    pub(crate) fn expressions(&self) -> Vec<String> {
        self.watches.iter().map(|w| w.expression.clone()).collect()
    }

    /// Adds a watch expression; its place, or `None` for an empty or repeated one.
    pub(crate) fn add_watch(&mut self, expression: &str) -> Option<usize> {
        let expression = expression.trim();
        if expression.is_empty() || self.watches.iter().any(|w| w.expression == expression) {
            return None;
        }
        self.watches.push(Watch {
            expression: expression.to_string(),
            value: None,
        });
        Some(self.watches.len() - 1)
    }

    /// Removes the watch at `index`; whether there was one.
    pub(crate) fn remove_watch(&mut self, index: usize) -> bool {
        if index < self.watches.len() {
            self.watches.remove(index);
            true
        } else {
            false
        }
    }

    /// The node at `path`.
    pub fn node(&self, path: &Path) -> Option<&Node> {
        match path {
            Path::Scope(names) => {
                let (first, rest) = names.split_first()?;
                let scope = self.scopes.as_ref()?.iter().find(|n| &n.name == first)?;
                descend(scope, rest)
            }
            Path::Watch(index, names) => {
                let node = self.watches.get(*index)?.value.as_ref()?.as_ref().ok()?;
                descend(node, names)
            }
        }
    }

    fn node_mut(&mut self, path: &Path) -> Option<&mut Node> {
        match path {
            Path::Scope(names) => {
                let (first, rest) = names.split_first()?;
                let scope = self.scopes.as_mut()?.iter_mut().find(|n| &n.name == first)?;
                descend_mut(scope, rest)
            }
            Path::Watch(index, names) => {
                let node = self.watches.get_mut(*index)?.value.as_mut()?.as_mut().ok()?;
                descend_mut(node, names)
            }
        }
    }

    /// The paths of the rows below `path`; `None` for the scopes at the top.
    pub fn children(&self, path: Option<&Path>) -> Vec<Path> {
        match path {
            None => self
                .scopes
                .iter()
                .flatten()
                .map(|scope| Path::Scope(vec![scope.name.clone()]))
                .collect(),
            Some(path) => self
                .node(path)
                .and_then(|node| node.children.as_ref())
                .map(|children| children.iter().map(|child| path.child(&child.name)).collect())
                .unwrap_or_default(),
        }
    }

    /// The reference to ask the adapter for when the row at `path` opens, if its children are not there yet.
    pub(crate) fn wanted(&self, path: &Path) -> Option<i64> {
        let node = self.node(path)?;
        (node.has_children() && node.children.is_none()).then_some(node.reference)
    }

    /// Takes the scopes of `generation`'s frame; whether they were still wanted.
    pub(crate) fn set_scopes(&mut self, generation: u64, scopes: Vec<Node>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.scopes = Some(scopes);
        true
    }

    /// Takes the children of the row at `path`; whether they were still wanted.
    pub fn set_children(&mut self, generation: u64, path: &Path, children: Vec<Node>) -> bool {
        if generation != self.generation {
            return false;
        }
        match self.node_mut(path) {
            Some(node) => {
                node.children = Some(children);
                true
            }
            None => false,
        }
    }

    /// Takes the value of the watch expression `expression`; whether it was still wanted.
    pub fn set_watch(&mut self, generation: u64, expression: &str, value: Result<Node, String>) -> bool {
        if generation != self.generation {
            return false;
        }
        match self.watches.iter_mut().find(|w| w.expression == expression) {
            Some(watch) => {
                watch.value = Some(value);
                true
            }
            None => false,
        }
    }

    /// The variables of the innermost scope with their values, for the values at the line ends in the editor.
    pub fn locals(&self) -> Vec<(String, String)> {
        self.scopes
            .iter()
            .flatten()
            .find_map(|scope| scope.children.as_ref())
            .map(|children| children.iter().map(|n| (n.name.clone(), n.value.clone())).collect())
            .unwrap_or_default()
    }
}

/// A part of the inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// The selected frame's scopes.
    Variables,
    /// The watch expressions.
    Watches,
}

/// A row of the inspector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Row {
    /// A part's title row.
    Part(Part),
    /// The line a part shows while it has nothing.
    Empty(Part),
    /// A variable or watch.
    Node(Path),
}

impl Variables {
    /// The rows below `row`, the two parts for `None`.
    pub fn rows(&self, row: Option<&Row>) -> Vec<Row> {
        let rows: Vec<Row> = match row {
            None => return vec![Row::Part(Part::Variables), Row::Part(Part::Watches)],
            Some(Row::Part(Part::Variables)) => self.children(None).into_iter().map(Row::Node).collect(),
            Some(Row::Part(Part::Watches)) => (0..self.watches.len())
                .map(|index| Row::Node(Path::Watch(index, Vec::new())))
                .collect(),
            Some(Row::Node(path)) => self.children(Some(path)).into_iter().map(Row::Node).collect(),
            Some(Row::Empty(_)) => Vec::new(),
        };
        match row {
            Some(Row::Part(part)) if rows.is_empty() => vec![Row::Empty(*part)],
            _ => rows,
        }
    }

    /// Whether the row can open: a part, or a node with children.
    pub fn opens(&self, row: &Row) -> bool {
        match row {
            Row::Part(_) => true,
            Row::Node(path) => self.node(path).is_some_and(Node::has_children),
            Row::Empty(_) => false,
        }
    }
}

fn descend<'a>(node: &'a Node, names: &[String]) -> Option<&'a Node> {
    let Some((first, rest)) = names.split_first() else {
        return Some(node);
    };
    let child = node.children.as_ref()?.iter().find(|n| &n.name == first)?;
    descend(child, rest)
}

fn descend_mut<'a>(node: &'a mut Node, names: &[String]) -> Option<&'a mut Node> {
    let Some((first, rest)) = names.split_first() else {
        return Some(node);
    };
    let child = node.children.as_mut()?.iter_mut().find(|n| &n.name == first)?;
    descend_mut(child, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paused() -> (Variables, u64) {
        let mut variables = Variables::new(vec!["n * 2".into()]);
        let generation = variables.show(Some((1, 1000)));
        variables.set_scopes(
            generation,
            vec![Node::new("Locals", "", 7), Node::new("Globals", "", 8)],
        );
        (variables, generation)
    }

    #[test]
    fn children_come_only_when_a_row_opens() {
        let (mut variables, generation) = paused();
        let locals = Path::Scope(vec!["Locals".into()]);
        assert_eq!(variables.children(None).len(), 2);
        assert_eq!(variables.wanted(&locals), Some(7));
        assert!(variables.children(Some(&locals)).is_empty());
        let mut point = Node::new("point", "Point { x: 1, y: 2 }", 9);
        point.kind = Some("Point".into());
        variables.set_children(generation, &locals, vec![Node::new("n", "3", 0), point]);
        assert_eq!(variables.wanted(&locals), None);
        let point = locals.child("point");
        assert_eq!(variables.children(Some(&locals)), [locals.child("n"), point.clone()]);
        assert_eq!(variables.wanted(&point), Some(9));
        assert_eq!(
            variables.wanted(&locals.child("n")),
            None,
            "a plain value has no children"
        );
        variables.set_children(generation, &point, vec![Node::new("x", "1", 0), Node::new("y", "2", 0)]);
        assert_eq!(variables.node(&point.child("y")).map(|n| n.value.as_str()), Some("2"));
        assert_eq!(
            variables.locals(),
            [
                ("n".into(), "3".into()),
                ("point".into(), "Point { x: 1, y: 2 }".into())
            ]
        );
    }

    #[test]
    fn an_answer_about_an_earlier_frame_is_dropped() {
        let (mut variables, old) = paused();
        let new = variables.show(Some((1, 1001)));
        assert!(!variables.set_scopes(old, vec![Node::new("Stale", "", 1)]));
        assert!(!variables.set_watch(old, "n * 2", Ok(Node::new("", "6", 0))));
        assert!(variables.scopes.is_none());
        assert!(variables.set_watch(new, "n * 2", Ok(Node::new("", "8", 0))));
        assert_eq!(
            variables.watches[0].value,
            Some(Ok(Node::new("", "8", 0))),
            "the watch keeps its expression and gets the new value"
        );
    }

    #[test]
    fn watches_are_added_once_and_removed_by_place() {
        let mut variables = Variables::new(vec![]);
        assert_eq!(variables.add_watch(" items.len() "), Some(0));
        assert_eq!(variables.add_watch("items.len()"), None);
        assert_eq!(variables.add_watch("  "), None);
        assert_eq!(variables.add_watch("n"), Some(1));
        assert!(variables.remove_watch(0));
        assert!(!variables.remove_watch(5));
        assert_eq!(variables.expressions(), ["n"]);
    }

    #[test]
    fn a_watch_with_children_and_an_error() {
        let mut variables = Variables::new(vec!["v".into(), "nope".into()]);
        let generation = variables.show(Some((1, 5)));
        variables.set_watch(generation, "v", Ok(Node::new("", "Vec(2)", 4)));
        variables.set_watch(generation, "nope", Err("no such name".into()));
        let v = Path::Watch(0, vec![]);
        assert_eq!(variables.wanted(&v), Some(4));
        variables.set_children(generation, &v, vec![Node::new("[0]", "1", 0)]);
        assert_eq!(variables.children(Some(&v)), [v.child("[0]")]);
        assert_eq!(variables.node(&Path::Watch(1, vec![])), None);
    }

    #[test]
    fn rows_list_scopes_and_watches_and_say_when_there_are_none() {
        let mut variables = Variables::new(vec![]);
        assert_eq!(
            variables.rows(Some(&Row::Part(Part::Variables))),
            [Row::Empty(Part::Variables)]
        );
        assert_eq!(
            variables.rows(Some(&Row::Part(Part::Watches))),
            [Row::Empty(Part::Watches)]
        );
        let (paused, _) = paused();
        variables = paused;
        assert_eq!(
            variables.rows(Some(&Row::Part(Part::Variables))),
            [
                Row::Node(Path::Scope(vec!["Locals".into()])),
                Row::Node(Path::Scope(vec!["Globals".into()]))
            ]
        );
        assert_eq!(
            variables.rows(Some(&Row::Part(Part::Watches))),
            [Row::Node(Path::Watch(0, vec![]))]
        );
        assert!(variables.opens(&Row::Node(Path::Scope(vec!["Locals".into()]))));
        assert!(
            !variables.opens(&Row::Node(Path::Watch(0, vec![]))),
            "not evaluated yet"
        );
    }
}
