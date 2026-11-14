use super::*;

fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

fn names(tool: Tool, root: &Path) -> Vec<(String, String, u32)> {
    let mut out: Vec<(String, String, u32)> = tool
        .from_source(root)
        .into_iter()
        .map(|t| (t.id.package, t.id.name, t.location.map_or(0, |l| l.line)))
        .collect();
    out.sort();
    out
}

const LIB: &str = r##"pub mod paths;
#[cfg(feature = "engine")]
mod engine;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds() {
        let text = "
}
";
        let raw = r#"{"type":"x"}"#;
        let quote = '"';
    }

    /* a comment with "a quote
    } */
    mod nested {
        #[tokio::test(flavor = "multi_thread")]
        async fn inner() {}
    }

    #[test]
    #[ignore]
    fn slow() {}

    #[cfg(feature = "engine")]
    #[test]
    fn with_the_engine() {}
}
"##;

#[test]
fn a_workspace_gives_its_default_members_tests_named_as_each_tool_names_them() {
    let root = ide_core::testing::scratch("test-source");
    write(
        &root,
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\", \"xtask\"]\ndefault-members = [\n    \".\",\n    \"crates/*\",\n]\n\n[package]\nname = \"app\"\n\n[[bin]]\nname = \"ide\"\npath = \"src/main.rs\"\n",
            ),
            ("src/main.rs", "mod window;\nfn main() {}\n"),
            (
                "src/window/mod.rs",
                "#[cfg(test)]\n#[path = \"window_tests.rs\"]\nmod tests;\n",
            ),
            ("src/window/window_tests.rs", "#[test]\nfn opens() {}\n"),
            ("src/unused.rs", "#[test]\nfn never_built() {}\n"),
            (
                "crates/my-core/Cargo.toml",
                "[package]\nname = \"my-core\" # the core\n",
            ),
            ("crates/my-core/src/lib.rs", LIB),
            ("crates/my-core/src/paths.rs", "#[test]\nfn joins() {}\n"),
            ("crates/my-core/src/engine.rs", "#[test]\nfn needs_the_feature() {}\n"),
            (
                "crates/my-core/tests/end-to-end.rs",
                "mod common;\n\n#[test]\nfn works() {}\n",
            ),
            ("crates/my-core/tests/common/mod.rs", "#[test]\nfn shared() {}\n"),
            ("xtask/Cargo.toml", "[package]\nname = \"xtask\"\n"),
            ("xtask/src/main.rs", "#[test]\nfn bundles() {}\n"),
        ],
    );
    let line = |name: &str| u32::try_from(LIB.lines().position(|l| l.contains(name)).unwrap() + 1).unwrap();
    assert_eq!(
        names(Tool::Cargo, &root),
        [
            ("end_to_end".into(), "common::shared".into(), 2),
            ("end_to_end".into(), "works".into(), 4),
            ("ide".into(), "window::tests::opens".into(), 2),
            ("my_core".into(), "paths::joins".into(), 2),
            ("my_core".into(), "tests::adds".into(), line("fn adds")),
            ("my_core".into(), "tests::nested::inner".into(), line("fn inner")),
            ("my_core".into(), "tests::slow".into(), line("fn slow")),
        ]
    );
    let nextest: Vec<String> = names(Tool::Nextest, &root)
        .into_iter()
        .map(|(p, n, _)| format!("{p} {n}"))
        .collect();
    assert_eq!(
        nextest,
        [
            "app::bin/ide window::tests::opens",
            "my-core paths::joins",
            "my-core tests::adds",
            "my-core tests::nested::inner",
            "my-core tests::slow",
            "my-core::end-to-end common::shared",
            "my-core::end-to-end works",
        ]
    );
    assert!(Tool::Go.from_source(&root).is_empty());
}

#[test]
fn a_root_package_without_default_members_tests_only_itself() {
    let root = ide_core::testing::scratch("test-source-root");
    write(
        &root,
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\"]\n\n[package]\nname = \"app\"\n",
            ),
            ("src/lib.rs", "#[test]\nfn here() {}\n"),
            ("src/bin/tool.rs", "#[test]\nfn there() {}\n"),
            ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("crates/a/src/lib.rs", "#[test]\nfn elsewhere() {}\n"),
        ],
    );
    let found: Vec<(String, String)> = names(Tool::Cargo, &root).into_iter().map(|(p, n, _)| (p, n)).collect();
    assert_eq!(found, [("app".into(), "here".into()), ("tool".into(), "there".into())]);
}

#[test]
fn strings_and_comments_hide_what_looks_like_code_in_them() {
    let names: Vec<String> = in_file(LIB).iter().map(|(path, _)| path.join("::")).collect();
    assert_eq!(names, ["tests::adds", "tests::nested::inner", "tests::slow"]);
    assert_eq!(after(r##"let a = r#"say "hi""#;"##, Lexed::Code), Lexed::Code);
    assert_eq!(after(r##"let a = br#"open"##, Lexed::Code), Lexed::Raw(1));
    assert_eq!(after(r"let c = '\''; let s = '\\';", Lexed::Code), Lexed::Code);
    assert_eq!(after("fn a<'a>(x: &'a str) {} // \"", Lexed::Code), Lexed::Code);
    assert_eq!(after("/* /* */", Lexed::Code), Lexed::Comment(1));
    assert_eq!(module_item("pub(crate) mod tests {"), Some(("tests".into(), true)));
    assert_eq!(module_item("mod tests;"), Some(("tests".into(), false)));
    assert_eq!(module_item("mod empty {}"), None);
    assert!(leaves_out("#[cfg(feature = \"engine\")]") && leaves_out("#[cfg(not(test))]"));
    assert!(!leaves_out("#[cfg(any(test, feature = \"testing\"))]") && !leaves_out("#[cfg(test)]"));
    assert!(matches("ide-*", "ide-test") && !matches("ide-*", "lowspeak") && matches("*", "x"));
}
