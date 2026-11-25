use ide_core::layout::{Editor, Kind, Layout, Tab, Untitled, Welcome, WindowTab};
use ide_dap::kept::{ConsoleKind, ConsoleLine};
use serde_json::Value;

use crate::browser::KeptPage;
use crate::debug::KeptConsole;
use crate::site_preview::KeptSite;
use crate::terminal::{KeptCli, KeptTerminal};
use ide_ui_agent::KeptSession;

/// A layout file of version 4 as the app writes it, with a document of every kind and the debugger's part.
const STORED: &str = include_str!("../../../crates/ide-core/src/layout/stored-v4.json");

/// The names of every kind of document the app keeps in a layout.
const NAMES: &[&str] = &[
    Welcome::NAME,
    Editor::NAME,
    Untitled::NAME,
    KeptTerminal::NAME,
    KeptCli::NAME,
    KeptPage::NAME,
    KeptSite::NAME,
    KeptSession::NAME,
    KeptConsole::NAME,
];

/// `tab` read as the kind `K` and kept again, if it is of that kind.
fn again<K: Kind>(tab: &Tab) -> Option<Tab> {
    Tab::new(&tab.read::<K>()?)
}

/// `tab` read by the feature that knows its kind and kept again by it; none for a kind no feature knows.
fn through_its_kind(tab: &Tab) -> Option<Tab> {
    again::<Welcome>(tab)
        .or_else(|| again::<Editor>(tab))
        .or_else(|| again::<Untitled>(tab))
        .or_else(|| again::<KeptTerminal>(tab))
        .or_else(|| again::<KeptCli>(tab))
        .or_else(|| again::<KeptPage>(tab))
        .or_else(|| again::<KeptSite>(tab))
        .or_else(|| again::<KeptSession>(tab))
        .or_else(|| again::<KeptConsole>(tab))
}

fn documents(layout: &Layout) -> Vec<&Tab> {
    layout.tabs.iter().flat_map(WindowTab::documents).collect()
}

#[test]
fn every_kind_of_document_has_a_name_of_its_own() {
    let mut names = NAMES.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), NAMES.len(), "{NAMES:?}");
}

#[test]
fn every_kind_of_a_stored_layout_reads_and_is_written_as_it_was() {
    let file: Value = serde_json::from_str(STORED).unwrap();
    let stored: Layout = serde_json::from_value(file["tabs"].clone()).unwrap();
    let mut kinds: Vec<&str> = documents(&stored).into_iter().map(Tab::kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let mut names = NAMES.to_vec();
    names.sort_unstable();
    assert_eq!(kinds, names, "the fixture holds a document of every kind");
    let mut written = stored.clone();
    for window_tab in &mut written.tabs {
        for pane in panes_mut(&mut window_tab.root) {
            for document in &mut pane.documents {
                *document = through_its_kind(document).unwrap_or_else(|| panic!("{document:?} reads"));
            }
        }
    }
    assert_eq!(written, stored);
    let mut file_again = file.clone();
    file_again["tabs"] = serde_json::to_value(&written).unwrap();
    let mut text = serde_json::to_string_pretty(&file_again).unwrap();
    text.push('\n');
    let mut expected = serde_json::to_string_pretty(&file).unwrap();
    expected.push('\n');
    assert_eq!(text, expected);
    let order_kept = serde_json::to_string(&serde_json::from_str::<Value>(r#"{"b":0,"a":0}"#).unwrap()).unwrap()
        == r#"{"b":0,"a":0}"#;
    if order_kept {
        assert_eq!(text, STORED, "the layout comes out byte for byte as it went in");
    }
}

#[test]
fn the_features_read_what_they_kept() {
    let file: Value = serde_json::from_str(STORED).unwrap();
    let stored: Layout = serde_json::from_value(file["tabs"].clone()).unwrap();
    let documents = documents(&stored);
    assert_eq!(
        documents[0].read::<Editor>(),
        Some(Editor {
            path: "/p/src/main.rs".into()
        })
    );
    assert_eq!(
        documents[4].read::<KeptSite>(),
        Some(KeptSite {
            path: "/p/site/index.html".into()
        })
    );
    assert_eq!(
        documents[6].read::<KeptSession>(),
        Some(KeptSession { session: "abc".into() })
    );
    let console = documents[7].read::<KeptConsole>().expect("a console");
    assert_eq!(console.title, "shop");
    let kinds: Vec<ConsoleKind> = console.history.iter().map(|line| line.kind).collect();
    assert_eq!(
        kinds,
        [
            ConsoleKind::Input,
            ConsoleKind::Result,
            ConsoleKind::Output,
            ConsoleKind::Error,
            ConsoleKind::Note
        ]
    );
    assert_eq!(
        documents[8].read::<KeptCli>(),
        Some(KeptCli {
            name: "claude".into(),
            session: Some("s1".into())
        })
    );
    assert_eq!(
        documents[9].read::<KeptCli>(),
        Some(KeptCli {
            name: "codex".into(),
            session: None
        })
    );
    assert_eq!(documents[9].read::<KeptTerminal>(), None);
}

#[test]
fn a_console_without_its_history_reads_as_an_empty_one() {
    let tab: Tab = serde_json::from_str(r#"{"kind": "debug", "title": "shop"}"#).unwrap();
    assert_eq!(
        tab.read::<KeptConsole>(),
        Some(KeptConsole {
            title: "shop".into(),
            history: Vec::<ConsoleLine>::new()
        })
    );
}

fn panes_mut(node: &mut ide_core::layout::Node) -> Vec<&mut ide_core::layout::Pane> {
    match node {
        ide_core::layout::Node::Pane(pane) => vec![pane],
        ide_core::layout::Node::Split { children, .. } => children.iter_mut().flat_map(panes_mut).collect(),
    }
}
