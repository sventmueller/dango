//! The window tabs of a project's window, their panes and the documents they show (E60, E145), kept per project in the app's data folder
//! rather than in the project, since a layout is the user's and not the repository's.

use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::paths;
use crate::store::{Migration, Store};

/// The migrations of a layout file.
pub(crate) const MIGRATIONS: &[Migration] = &[panels_to_tabs, console_tabs, split_to_panes];

/// The field that holds the window tabs.
const FIELD: &str = "tabs";

/// A document that can be opened again: the name of its kind, which the feature that opens it gives, and what that
/// feature keeps of it, stored as one JSON object that names the kind in its field `kind`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    kind: String,
    fields: Map<String, Value>,
}

/// A kind of document as the feature that opens it keeps it in the layout: a struct with named fields, `{}` for none.
pub trait Kind: Serialize + DeserializeOwned {
    /// The name the field `kind` holds, which no other kind has and which stays once a layout was written with it.
    const NAME: &'static str;
}

/// The field of a stored document that names its kind.
const KIND: &str = "kind";

impl Tab {
    /// `document` as the layout keeps it; none for one that is no JSON object, such as one with a path that is no
    /// UTF-8, or that has a field `kind` of its own.
    pub fn new<K: Kind>(document: &K) -> Option<Tab> {
        match serde_json::to_value(document).ok()? {
            Value::Object(fields) if !fields.contains_key(KIND) => Some(Tab {
                kind: K::NAME.to_string(),
                fields,
            }),
            _ => None,
        }
    }

    /// The name of its kind.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Whether it is of the kind `K`.
    pub fn is<K: Kind>(&self) -> bool {
        self.kind == K::NAME
    }

    /// The document as its kind keeps it; none for a document of another kind or with fields `K` does not read.
    pub fn read<K: Kind>(&self) -> Option<K> {
        if !self.is::<K>() {
            return None;
        }
        serde_json::from_value(Value::Object(self.fields.clone())).ok()
    }
}

impl Serialize for Tab {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.fields.len() + 1))?;
        map.serialize_entry(KIND, &self.kind)?;
        for (name, value) in &self.fields {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Tab {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Tab, D::Error> {
        let mut kind = None;
        let fields = Map::<String, Value>::deserialize(deserializer)?
            .into_iter()
            .filter_map(|(name, value)| {
                if name == KIND {
                    kind = Some(value);
                    None
                } else {
                    Some((name, value))
                }
            })
            .collect();
        match kind {
            Some(Value::String(kind)) => Ok(Tab { kind, fields }),
            _ => Err(serde::de::Error::missing_field(KIND)),
        }
    }
}

/// The welcome page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {}

impl Kind for Welcome {
    const NAME: &'static str = "welcome";
}

/// A file in an editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Editor {
    /// The file.
    pub path: PathBuf,
}

impl Kind for Editor {
    const NAME: &'static str = "editor";
}

/// An editor without a file, which comes back only from the copy a crash left of its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Untitled {
    /// The name of the copy.
    pub recovery: String,
}

impl Kind for Untitled {
    const NAME: &'static str = "untitled";
}

/// Documents behind one another in a part of a window tab, one of them in front (E145).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    /// The documents, in the order of the pane's tab bar.
    pub documents: Vec<Tab>,
    /// The index of the document in front.
    #[serde(default)]
    pub selected: usize,
}

/// How a window tab is divided: a pane, or parts side by side or one above the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Node {
    /// One pane.
    Pane(Pane),
    /// Parts next to one another.
    Split {
        /// Left to right when true, top to bottom when false.
        side_by_side: bool,
        /// The parts.
        children: Vec<Node>,
        /// Each part's share of the width or height, adding up to 1.
        shares: Vec<f64>,
    },
}

impl Node {
    /// A pane holding the one document `tab`.
    pub(crate) fn single(tab: Tab) -> Node {
        Node::Pane(Pane {
            documents: vec![tab],
            selected: 0,
        })
    }

    /// The panes, left to right and top to bottom.
    pub(crate) fn panes(&self) -> Vec<&Pane> {
        fn walk<'a>(node: &'a Node, into: &mut Vec<&'a Pane>) {
            match node {
                Node::Pane(pane) => into.push(pane),
                Node::Split { children, .. } => children.iter().for_each(|child| walk(child, into)),
            }
        }
        let mut panes = Vec::new();
        walk(self, &mut panes);
        panes
    }

    /// The node without empty panes, without splits of fewer than two parts, with a split inside a split of the same
    /// direction merged into it, and with shares that add up to 1; `None` when no document is left.
    pub(crate) fn normalised(self) -> Option<Node> {
        match self {
            Node::Pane(mut pane) => {
                if pane.documents.is_empty() {
                    return None;
                }
                pane.selected = pane.selected.min(pane.documents.len() - 1);
                Some(Node::Pane(pane))
            }
            Node::Split {
                side_by_side,
                children,
                shares,
            } => normalised_split(side_by_side, children, shares),
        }
    }
}

fn normalised_split(side_by_side: bool, children: Vec<Node>, shares: Vec<f64>) -> Option<Node> {
    let even = 1.0 / children.len().max(1) as f64;
    let usable = shares.len() == children.len() && shares.iter().all(|share| share.is_finite() && *share > 0.0);
    let mut parts: Vec<(Node, f64)> = Vec::new();
    for (index, child) in children.into_iter().enumerate() {
        let share = if usable { shares[index] } else { even };
        match child.normalised() {
            Some(Node::Split {
                side_by_side: inner,
                children,
                shares,
            }) if inner == side_by_side => {
                parts.extend(
                    children
                        .into_iter()
                        .zip(shares)
                        .map(|(node, part)| (node, part * share)),
                );
            }
            Some(node) => parts.push((node, share)),
            None => {}
        }
    }
    if parts.len() < 2 {
        return parts.pop().map(|(node, _)| node);
    }
    let total: f64 = parts.iter().map(|(_, share)| share).sum();
    let (children, shares) = parts.into_iter().map(|(node, share)| (node, share / total)).unzip();
    Some(Node::Split {
        side_by_side,
        children,
        shares,
    })
}

/// A window tab and how its documents are divided into panes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowTab {
    /// The panes.
    pub root: Node,
}

impl WindowTab {
    /// A window tab with the one document `tab`.
    pub(crate) fn single(tab: Tab) -> WindowTab {
        WindowTab {
            root: Node::single(tab),
        }
    }

    /// The documents, pane by pane in reading order.
    pub fn documents(&self) -> Vec<&Tab> {
        self.root.panes().into_iter().flat_map(|pane| &pane.documents).collect()
    }
}

/// The window tabs of a project's window, in the order of its tab bar, and the one in front.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Layout {
    pub tabs: Vec<WindowTab>,
    /// The index of the window tab in front.
    pub selected: Option<usize>,
}

impl Layout {
    /// Whether a window tab shows `tab`.
    pub fn has(&self, tab: &Tab) -> bool {
        self.tabs.iter().any(|window_tab| window_tab.documents().contains(&tab))
    }

    /// How many documents the window tabs show.
    pub fn document_count(&self) -> usize {
        self.tabs.iter().map(|window_tab| window_tab.documents().len()).sum()
    }
}

/// The file that keeps the layout of the project at `root`, named by [`paths::project_key`].
pub fn file_of(root: &Path) -> PathBuf {
    paths::data_dir()
        .join("layouts")
        .join(format!("{}.json", paths::project_key(root)))
}

/// The layout saved for the project at `root`, if there is a readable one.
pub fn load(root: &Path) -> Option<Layout> {
    load_from(file_of(root))
}

/// Saves `layout` for the project at `root`.
pub fn save(root: &Path, layout: &Layout) -> io::Result<()> {
    save_to(file_of(root), layout)
}

fn load_from(file: PathBuf) -> Option<Layout> {
    let store = Store::load(file, MIGRATIONS);
    serde_json::from_value(store.get(FIELD)?.clone()).ok()
}

fn save_to(file: PathBuf, layout: &Layout) -> io::Result<()> {
    let value = serde_json::to_value(layout).map_err(io::Error::other)?;
    write(file, FIELD, value)
}

/// What a feature keeps under `name` in the layout file `file`, beside the window tabs, such as the debugger its
/// breakpoints; [`file_of`] names a project's file.
pub fn load_part(file: PathBuf, name: &str) -> Option<Value> {
    Store::load(file, MIGRATIONS).get(name).cloned()
}

/// Keeps `value` under `name` in the layout file `file`, beside the window tabs and what other features keep there.
pub fn save_part(file: PathBuf, name: &str, value: Value) -> io::Result<()> {
    if name == FIELD || name == crate::store::VERSION_FIELD {
        return Err(io::Error::other(format!("{name} is the layout's own field")));
    }
    write(file, name, value)
}

fn write(file: PathBuf, name: &str, value: Value) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    Store::load(file, MIGRATIONS).set(name, value, MIGRATIONS)
}

/// The panels of version 1, before every document got a window tab of its own.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Panels {
    Panel { tabs: Vec<Tab>, selected: Option<usize> },
    Split { children: Vec<Panels> },
}

impl Panels {
    /// The tabs of the panels, left to right and top to bottom, and the index of the first panel's tab in front.
    fn flatten(self, into: &mut Layout) {
        match self {
            Panels::Panel { tabs, selected } => {
                if into.selected.is_none() {
                    into.selected = selected.map(|index| into.tabs.len() + index);
                }
                into.tabs.extend(tabs.into_iter().map(WindowTab::single));
            }
            Panels::Split { children } => {
                for child in children {
                    child.flatten(into);
                }
            }
        }
    }
}

/// Version 1 kept panels with tab bars of their own; each of their tabs becomes a window tab, in reading order.
fn panels_to_tabs(values: &mut Map<String, Value>) {
    let Some(panels) = values.remove("panels") else { return };
    let Ok(panels) = serde_json::from_value::<Panels>(panels) else {
        return;
    };
    let mut layout = Layout::default();
    panels.flatten(&mut layout);
    if let Ok(tabs) = serde_json::to_value(&layout) {
        values.insert(FIELD.to_string(), tabs);
    }
}

/// Version 3 added the debugger's part and its console tabs, which version 2 would not read; a missing part reads as
/// nothing kept, so only the number changes.
fn console_tabs(_: &mut Map<String, Value>) {}

/// Version 3 kept one document or two side by side per window tab; they become a pane, or two panes side by side.
fn split_to_panes(values: &mut Map<String, Value>) {
    let Some(tabs) = values
        .get_mut(FIELD)
        .and_then(|layout| layout.get_mut(FIELD))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for window_tab in tabs.iter_mut() {
        let Some(object) = window_tab.as_object_mut() else {
            continue;
        };
        let Some(Value::Array(documents)) = object.remove("documents") else {
            continue;
        };
        let share = object.remove("share").and_then(|share| share.as_f64()).unwrap_or(0.5);
        let pane = |document: Value| serde_json::json!({"pane": {"documents": [document], "selected": 0}});
        let root = match <[Value; 2]>::try_from(documents) {
            Ok([left, right]) => serde_json::json!({"split": {
                "side_by_side": true,
                "children": [pane(left), pane(right)],
                "shares": [share, 1.0 - share],
            }}),
            Err(documents) => serde_json::json!({"pane": {"documents": documents, "selected": 0}}),
        };
        object.insert("root".to_string(), root);
    }
}

#[cfg(test)]
mod tests;
