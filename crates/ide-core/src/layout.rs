//! The panels of a project's window and the tabs they show (I10), kept per project in the app's data folder rather
//! than in the project, since a layout is the user's and not the repository's.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;
use crate::store::{Migration, Store};

/// The migrations of a layout file.
const MIGRATIONS: &[Migration] = &[];

/// The field that holds the tree.
const FIELD: &str = "panels";

/// A tab that can be opened again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Tab {
    /// The welcome tab.
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
    /// A browser tab at a page.
    Browser {
        /// The page.
        url: String,
    },
    /// An AI tab, opened again by resuming its session.
    Agent {
        /// The agent's id for the session.
        session: String,
    },
}

/// A panel, or panels beside or above each other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Node {
    /// A panel with its tabs, left to right, and which of them shows.
    Panel {
        /// The tabs.
        tabs: Vec<Tab>,
        /// The index of the tab in front.
        selected: Option<usize>,
    },
    /// Panels split side by side or one above the other.
    Split {
        /// Whether the children sit side by side, else one above the other.
        side_by_side: bool,
        /// Each child's share of the room, adding up to about 1.
        shares: Vec<f64>,
        /// The parts, left to right or top to bottom.
        children: Vec<Node>,
    },
}

impl Node {
    /// Whether the tree holds `tab`.
    pub fn has(&self, tab: &Tab) -> bool {
        match self {
            Node::Panel { tabs, .. } => tabs.contains(tab),
            Node::Split { children, .. } => children.iter().any(|child| child.has(tab)),
        }
    }

    /// How many tabs the tree holds.
    pub fn tab_count(&self) -> usize {
        match self {
            Node::Panel { tabs, .. } => tabs.len(),
            Node::Split { children, .. } => children.iter().map(Node::tab_count).sum(),
        }
    }
}

/// The file that keeps the layout of the project at `root`, named by [`paths::project_key`].
pub fn file_of(root: &Path) -> PathBuf {
    paths::data_dir()
        .join("layouts")
        .join(format!("{}.json", paths::project_key(root)))
}

/// The layout saved for the project at `root`, if there is a readable one.
pub fn load(root: &Path) -> Option<Node> {
    load_from(file_of(root))
}

/// Saves `layout` for the project at `root`.
pub fn save(root: &Path, layout: &Node) -> io::Result<()> {
    save_to(file_of(root), layout)
}

fn load_from(file: PathBuf) -> Option<Node> {
    let store = Store::load(file, MIGRATIONS);
    serde_json::from_value(store.get(FIELD)?.clone()).ok()
}

fn save_to(file: PathBuf, layout: &Node) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let value = serde_json::to_value(layout).map_err(io::Error::other)?;
    Store::load(file, MIGRATIONS).set(FIELD, value, MIGRATIONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Node {
        Node::Split {
            side_by_side: true,
            shares: vec![0.6, 0.4],
            children: vec![
                Node::Panel {
                    tabs: vec![Tab::Editor {
                        path: PathBuf::from("/p/src/main.rs"),
                    }],
                    selected: Some(0),
                },
                Node::Split {
                    side_by_side: false,
                    shares: vec![0.5, 0.5],
                    children: vec![
                        Node::Panel {
                            tabs: vec![Tab::Terminal {
                                folder: PathBuf::from("/p"),
                            }],
                            selected: Some(0),
                        },
                        Node::Panel {
                            tabs: vec![
                                Tab::Browser {
                                    url: "http://localhost:5173".into(),
                                },
                                Tab::Agent { session: "abc".into() },
                                Tab::Welcome,
                                Tab::Untitled {
                                    recovery: "a1-2-3".into(),
                                },
                            ],
                            selected: Some(1),
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn a_layout_comes_back_as_it_was_saved() {
        let file = crate::store::tests::scratch("layout").join("layouts/shop.json");
        assert_eq!(load_from(file.clone()), None);
        save_to(file.clone(), &sample()).unwrap();
        assert_eq!(load_from(file.clone()), Some(sample()));
        assert_eq!(sample().tab_count(), 6);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(r#""kind": "terminal""#), "{text}");
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(load_from(file), None, "a damaged file restores nothing");
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
