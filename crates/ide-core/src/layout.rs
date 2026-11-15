//! The window tabs of a project's window, their panes and the documents they show (E60, E145), kept per project in the app's data folder
//! rather than in the project, since a layout is the user's and not the repository's.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::paths;
use crate::store::{Migration, Store};

/// The migrations of a layout file.
pub(crate) const MIGRATIONS: &[Migration] = &[panels_to_tabs, crate::debugging::add_debugging, split_to_panes];

/// The field that holds the window tabs.
const FIELD: &str = "tabs";

/// A document that can be opened again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Tab {
    /// The welcome page.
    Welcome,
    /// A file in an editor.
    Editor {
        /// The file.
        path: PathBuf,
    },
    /// An editor without a file, which comes back only from the copy a crash left of its text.
    Untitled {
        /// The name of the copy.
        recovery: String,
    },
    /// A terminal, opened again as a new shell in the folder it was in.
    Terminal {
        /// The shell's working folder.
        folder: PathBuf,
    },
    /// A browser at a page.
    Browser {
        /// The page.
        url: String,
    },
    /// The live preview of a static page (E131), which opens again with a server on a new port.
    SitePreview {
        /// The page's file.
        path: PathBuf,
    },
    /// An AI thread, opened again by resuming its session.
    Agent {
        /// The agent's id for the session.
        session: String,
    },
    /// A debug console, which comes back as the finished history of its session, only to read; no session starts.
    Debug {
        /// The name of the session it belonged to.
        title: String,
        /// What it showed, oldest first.
        #[serde(default)]
        history: Vec<crate::debugging::ConsoleLine>,
    },
    /// A CLI thread of an older version, which the app no longer has; it opens again as a terminal.
    Cli {
        /// The CLI's name, such as `claude`.
        name: String,
        /// The CLI's id for the session, where the app could tell it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
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
    pub fn single(tab: Tab) -> Node {
        Node::Pane(Pane {
            documents: vec![tab],
            selected: 0,
        })
    }

    /// The panes, left to right and top to bottom.
    pub fn panes(&self) -> Vec<&Pane> {
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
    pub fn normalised(self) -> Option<Node> {
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
    pub fn single(tab: Tab) -> WindowTab {
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
pub struct Layout {
    /// The window tabs.
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
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let value = serde_json::to_value(layout).map_err(io::Error::other)?;
    Store::load(file, MIGRATIONS).set(FIELD, value, MIGRATIONS)
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
pub(crate) mod tests {
    use super::*;

    pub(crate) fn save_layout(file: PathBuf, layout: &Layout) {
        save_to(file, layout).unwrap();
    }

    pub(crate) fn load_layout(file: PathBuf) -> Option<Layout> {
        load_from(file)
    }

    fn sample() -> Layout {
        Layout {
            tabs: vec![
                WindowTab {
                    root: Node::Split {
                        side_by_side: true,
                        children: vec![
                            Node::Pane(Pane {
                                documents: vec![
                                    Tab::Editor {
                                        path: PathBuf::from("/p/src/main.rs"),
                                    },
                                    Tab::Welcome,
                                ],
                                selected: 1,
                            }),
                            Node::Split {
                                side_by_side: false,
                                children: vec![
                                    Node::single(Tab::Browser {
                                        url: "http://localhost:5173".into(),
                                    }),
                                    Node::single(Tab::Terminal {
                                        folder: PathBuf::from("/q"),
                                    }),
                                ],
                                shares: vec![0.3, 0.7],
                            },
                        ],
                        shares: vec![0.6, 0.4],
                    },
                },
                WindowTab::single(Tab::Terminal {
                    folder: PathBuf::from("/p"),
                }),
                WindowTab::single(Tab::Agent { session: "abc".into() }),
                WindowTab::single(Tab::Welcome),
                WindowTab::single(Tab::Untitled {
                    recovery: "a1-2-3".into(),
                }),
            ],
            selected: Some(1),
        }
    }

    #[test]
    fn a_layout_comes_back_as_it_was_saved() {
        let scratch_dir = crate::store::tests::scratch("layout");
        let file = scratch_dir.join("layouts/shop.json");
        assert_eq!(load_from(file.clone()), None);
        save_to(file.clone(), &sample()).unwrap();
        assert_eq!(load_from(file.clone()), Some(sample()));
        assert_eq!(sample().document_count(), 8);
        assert!(sample().has(&Tab::Welcome));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(r#""kind": "terminal""#), "{text}");
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(load_from(file), None, "a damaged file restores nothing");
    }

    #[test]
    fn the_panels_of_version_one_become_window_tabs_in_reading_order() {
        let scratch_dir = crate::store::tests::scratch("layout-panels");
        let file = scratch_dir.join("layouts/shop.json");
        let old = r#"{
          "version": 1,
          "panels": {"split": {"side_by_side": true, "shares": [0.6, 0.4], "children": [
            {"panel": {"tabs": [{"kind": "editor", "path": "/p/a.rs"}, {"kind": "welcome"}], "selected": 1}},
            {"panel": {"tabs": [{"kind": "terminal", "folder": "/p"}], "selected": 0}}
          ]}}
        }"#;
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, old).unwrap();
        let layout = load_from(file).unwrap();
        assert_eq!(
            layout,
            Layout {
                tabs: vec![
                    WindowTab::single(Tab::Editor {
                        path: PathBuf::from("/p/a.rs")
                    }),
                    WindowTab::single(Tab::Welcome),
                    WindowTab::single(Tab::Terminal {
                        folder: PathBuf::from("/p")
                    }),
                ],
                selected: Some(1),
            }
        );
    }

    #[test]
    fn two_documents_side_by_side_of_version_three_become_two_panes() {
        let scratch_dir = crate::store::tests::scratch("layout-split");
        let file = scratch_dir.join("layouts/shop.json");
        let old = r#"{"version": 3, "tabs": {"tabs": [
            {"documents": [{"kind": "editor", "path": "/p/a.rs"}, {"kind": "welcome"}], "share": 0.7},
            {"documents": [{"kind": "terminal", "folder": "/p"}]}
        ], "selected": 1}}"#;
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, old).unwrap();
        let layout = load_from(file).unwrap();
        let editor = Tab::Editor {
            path: PathBuf::from("/p/a.rs"),
        };
        assert_eq!(
            layout,
            Layout {
                tabs: vec![
                    WindowTab {
                        root: Node::Split {
                            side_by_side: true,
                            children: vec![Node::single(editor), Node::single(Tab::Welcome)],
                            shares: vec![0.7, 1.0 - 0.7],
                        },
                    },
                    WindowTab::single(Tab::Terminal {
                        folder: PathBuf::from("/p")
                    }),
                ],
                selected: Some(1),
            }
        );
    }

    #[test]
    fn normalising_drops_empty_panes_and_merges_splits_of_one_direction() {
        let pane = |name: &str| Node::single(Tab::Browser { url: name.into() });
        let empty = Node::Pane(Pane {
            documents: Vec::new(),
            selected: 0,
        });
        let nested = Node::Split {
            side_by_side: true,
            children: vec![
                pane("a"),
                Node::Split {
                    side_by_side: true,
                    children: vec![pane("b"), pane("c")],
                    shares: vec![0.5, 0.5],
                },
                Node::Split {
                    side_by_side: false,
                    children: vec![empty.clone(), pane("d")],
                    shares: vec![0.5, 0.5],
                },
            ],
            shares: vec![0.4, 0.4, 0.2],
        };
        let Some(Node::Split {
            side_by_side: true,
            children,
            shares,
        }) = nested.normalised()
        else {
            panic!("a split side by side stays");
        };
        assert_eq!(children, [pane("a"), pane("b"), pane("c"), pane("d")]);
        let rounded: Vec<f64> = shares.iter().map(|share| (share * 100.0).round() / 100.0).collect();
        assert_eq!(rounded, [0.4, 0.2, 0.2, 0.2]);
        assert_eq!(empty.normalised(), None);
        let lone = Node::Split {
            side_by_side: false,
            children: vec![pane("a")],
            shares: vec![1.0],
        };
        assert_eq!(lone.normalised(), Some(pane("a")));
        let bad = Node::Split {
            side_by_side: false,
            children: vec![pane("a"), pane("b")],
            shares: vec![f64::NAN],
        };
        let Some(Node::Split { shares, .. }) = bad.normalised() else {
            panic!("two panes stay a split");
        };
        assert_eq!(shares, [0.5, 0.5], "shares that do not fit become even");
    }

    #[test]
    fn a_pane_in_front_stays_inside_its_documents() {
        let node = Node::Pane(Pane {
            documents: vec![Tab::Welcome],
            selected: 4,
        });
        assert_eq!(node.normalised(), Some(Node::single(Tab::Welcome)));
    }

    #[test]
    fn a_cli_thread_of_an_older_version_still_reads() {
        let known = Tab::Cli {
            name: "claude".into(),
            session: Some("s1".into()),
        };
        let unknown = Tab::Cli {
            name: "codex".into(),
            session: None,
        };
        let written = serde_json::to_string(&[&known, &unknown]).unwrap();
        assert_eq!(
            written,
            r#"[{"kind":"cli","name":"claude","session":"s1"},{"kind":"cli","name":"codex"}]"#
        );
        let read: Vec<Tab> = serde_json::from_str(&written).unwrap();
        assert_eq!(read, [known, unknown]);
    }

    #[test]
    fn each_project_has_a_file_of_its_own_that_names_it() {
        let a = file_of(Path::new("/work/shop"));
        let b = file_of(Path::new("/other/shop"));
        assert_ne!(a, b);
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("shop-"));
        assert_eq!(a, file_of(Path::new("/work/shop")));
    }
}
