use serde_json::json;

use super::*;

/// A layout file of version 4 as the app writes it, with a document of every kind the app keeps and the debugger's
/// part beside the tabs.
const STORED: &str = include_str!("stored-v4.json");

/// A document of a kind that a feature, not the layout, knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Page {
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

impl Kind for Page {
    const NAME: &'static str = "page";
}

/// A kind that would hide its name behind a field of its own.
#[derive(Serialize, Deserialize)]
struct Shadowing {
    kind: String,
}

impl Kind for Shadowing {
    const NAME: &'static str = "shadowing";
}

fn page(url: &str) -> Tab {
    Tab::new(&Page {
        url: url.into(),
        title: None,
    })
    .unwrap()
}

fn editor(path: &str) -> Tab {
    Tab::new(&Editor { path: path.into() }).unwrap()
}

fn welcome() -> Tab {
    Tab::new(&Welcome {}).unwrap()
}

fn stored(value: Value) -> Tab {
    serde_json::from_value(value).unwrap()
}

/// `value` as the layout file holds it: through a JSON value, written as the store writes it.
fn as_written(value: &impl Serialize) -> String {
    let mut text = serde_json::to_string_pretty(&serde_json::to_value(value).unwrap()).unwrap();
    text.push('\n');
    text
}

/// Whether JSON objects keep the order of their fields, as they do in the app, where a dependency turns on
/// serde_json's `preserve_order`; without it every object comes out sorted.
fn order_kept() -> bool {
    let text = r#"{"b":0,"a":0}"#;
    serde_json::to_string(&serde_json::from_str::<Value>(text).unwrap()).unwrap() == text
}

fn sample() -> Layout {
    Layout {
        tabs: vec![
            WindowTab {
                root: Node::Split {
                    side_by_side: true,
                    children: vec![
                        Node::Pane(Pane {
                            documents: vec![editor("/p/src/main.rs"), welcome()],
                            selected: 1,
                        }),
                        Node::Split {
                            side_by_side: false,
                            children: vec![
                                Node::single(page("http://localhost:5173")),
                                Node::single(stored(json!({"kind": "terminal", "folder": "/q"}))),
                            ],
                            shares: vec![0.3, 0.7],
                        },
                    ],
                    shares: vec![0.6, 0.4],
                },
            },
            WindowTab::single(stored(json!({"kind": "terminal", "folder": "/p"}))),
            WindowTab::single(stored(json!({"kind": "agent", "session": "abc"}))),
            WindowTab::single(welcome()),
            WindowTab::single(
                Tab::new(&Untitled {
                    recovery: "a1-2-3".into(),
                })
                .unwrap(),
            ),
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
    assert!(sample().has(&welcome()));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains(r#""kind": "terminal""#), "{text}");
    std::fs::write(&file, "{ not json").unwrap();
    assert_eq!(load_from(file), None, "a damaged file restores nothing");
}

#[test]
fn a_stored_layout_of_version_four_is_written_again_byte_for_byte() {
    let scratch_dir = crate::store::tests::scratch("layout-bytes");
    let file = scratch_dir.join("layouts/shop.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, STORED).unwrap();
    let layout = load_from(file.clone()).expect("the fixture reads");
    assert_eq!(layout.document_count(), 10);
    let kinds: Vec<&str> = layout
        .tabs
        .iter()
        .flat_map(WindowTab::documents)
        .map(Tab::kind)
        .collect();
    assert_eq!(
        kinds,
        [
            "editor",
            "untitled",
            "welcome",
            "browser",
            "sitepreview",
            "terminal",
            "agent",
            "debug",
            "cli",
            "cli"
        ]
    );
    save_to(file.clone(), &layout).unwrap();
    let written = std::fs::read_to_string(&file).unwrap();
    let fixture: Value = serde_json::from_str(STORED).unwrap();
    assert_eq!(written, as_written(&fixture));
    if order_kept() {
        assert_eq!(written, STORED, "the app writes the file it read");
    }
}

#[test]
fn a_document_of_a_kind_the_layout_does_not_know_keeps_its_fields_in_their_order() {
    let text = r#"{"kind":"debug","title":"shop","history":[{"kind":"output","text":"hi\n"}],"extra":1}"#;
    let tab: Tab = serde_json::from_str(text).unwrap();
    assert_eq!(tab.kind(), "debug");
    let read: Value = serde_json::from_str(text).unwrap();
    assert_eq!(as_written(&tab), as_written(&read));
    if order_kept() {
        assert_eq!(serde_json::to_string(&tab).unwrap(), text);
    }
}

#[test]
fn a_document_reads_only_as_its_own_kind() {
    let tab = page("http://localhost:3000");
    assert_eq!(
        serde_json::to_string(&tab).unwrap(),
        r#"{"kind":"page","url":"http://localhost:3000"}"#
    );
    assert_eq!(
        tab.read::<Page>(),
        Some(Page {
            url: "http://localhost:3000".into(),
            title: None
        })
    );
    assert!(tab.is::<Page>() && !tab.is::<Editor>());
    assert_eq!(tab.read::<Editor>(), None);
    let without_url = stored(json!({"kind": "page"}));
    assert_eq!(without_url.kind(), "page");
    assert_eq!(
        without_url.read::<Page>(),
        None,
        "fields the kind does not read open nothing"
    );
    assert_eq!(serde_json::to_string(&welcome()).unwrap(), r#"{"kind":"welcome"}"#);
    assert_eq!(welcome().read::<Welcome>(), Some(Welcome {}));
}

#[test]
fn a_document_needs_a_kind_and_has_no_field_of_that_name() {
    assert!(serde_json::from_str::<Tab>(r#"{"url": "x"}"#).is_err());
    assert!(serde_json::from_str::<Tab>(r#"{"kind": 3}"#).is_err());
    assert!(serde_json::from_str::<Tab>(r#"["kind", "page"]"#).is_err());
    assert_eq!(Tab::new(&Shadowing { kind: "page".into() }), None);
}

#[test]
fn a_feature_keeps_its_part_beside_the_tabs() {
    let scratch_dir = crate::store::tests::scratch("layout-part");
    let file = scratch_dir.join("layouts/shop.json");
    assert_eq!(load_part(file.clone(), "debugging"), None);
    let part = json!({"breakpoints": [{"path": "/p/a.rs", "line": 3}]});
    save_part(file.clone(), "debugging", part.clone()).unwrap();
    save_to(file.clone(), &sample()).unwrap();
    assert_eq!(load_part(file.clone(), "debugging"), Some(part.clone()));
    save_part(file.clone(), "debugging", json!({})).unwrap();
    assert_eq!(load_from(file.clone()), Some(sample()), "the tabs stay as they were");
    assert!(save_part(file.clone(), "tabs", json!([])).is_err());
    assert!(save_part(file.clone(), "version", json!(1)).is_err());
    assert_eq!(load_from(file), Some(sample()));
}

#[test]
fn a_newer_layout_is_never_overwritten_by_this_version() {
    let scratch_dir = crate::store::tests::scratch("layout-newer");
    let file = scratch_dir.join("layouts/shop.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let newer = r#"{"version": 99, "debugging": {"breakpoints": []}}"#;
    std::fs::write(&file, newer).unwrap();
    assert!(save_part(file.clone(), "debugging", json!({})).is_err());
    assert!(save_to(file.clone(), &sample()).is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), newer);
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
                WindowTab::single(editor("/p/a.rs")),
                WindowTab::single(welcome()),
                WindowTab::single(stored(json!({"kind": "terminal", "folder": "/p"}))),
            ],
            selected: Some(1),
        }
    );
}

#[test]
fn a_layout_of_version_two_keeps_its_tabs_and_has_no_part() {
    let scratch_dir = crate::store::tests::scratch("layout-two");
    let file = scratch_dir.join("layouts/shop.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        r#"{"version": 2, "tabs": {"tabs": [{"documents": [{"kind": "welcome"}], "share": 0.5}], "selected": 0}}"#,
    )
    .unwrap();
    assert_eq!(load_part(file.clone(), "debugging"), None);
    assert_eq!(
        load_from(file),
        Some(Layout {
            tabs: vec![WindowTab::single(welcome())],
            selected: Some(0),
        })
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
    assert_eq!(
        layout,
        Layout {
            tabs: vec![
                WindowTab {
                    root: Node::Split {
                        side_by_side: true,
                        children: vec![Node::single(editor("/p/a.rs")), Node::single(welcome())],
                        shares: vec![0.7, 1.0 - 0.7],
                    },
                },
                WindowTab::single(stored(json!({"kind": "terminal", "folder": "/p"}))),
            ],
            selected: Some(1),
        }
    );
}

#[test]
fn normalising_drops_empty_panes_and_merges_splits_of_one_direction() {
    let pane = |name: &str| Node::single(page(name));
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
        documents: vec![welcome()],
        selected: 4,
    });
    assert_eq!(node.normalised(), Some(Node::single(welcome())));
}

#[test]
fn each_project_has_a_file_of_its_own_that_names_it() {
    let a = file_of(Path::new("/work/shop"));
    let b = file_of(Path::new("/other/shop"));
    assert_ne!(a, b);
    assert!(a.file_name().unwrap().to_string_lossy().starts_with("shop-"));
    assert_eq!(a, file_of(Path::new("/work/shop")));
}
