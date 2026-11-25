//! The project both apps open and the settings that make them comparable: no language servers, predictions, update
//! checks or telemetry, no blinking cursor where the app lets one turn it off, one font size and one tab shortcut.

use std::path::Path;

use serde_json::{Value, json};

use crate::files;

/// The Rust file of the project and the line typed into, one of the long lines in its middle.
pub(crate) const RUST_FILE: &str = "code.rs";
pub(crate) const RUST_LINE: usize = 5001;
/// The Markdown file, typed into at its first line.
pub(crate) const MARKDOWN_FILE: &str = "notes.md";
pub(crate) const MARKDOWN_LINE: usize = 1;
const LINES: usize = 10_000;

/// The editor's text size in points: the default `fontSize` of this app (`ide_core::settings::keys::FONT_SIZE`),
/// which its settings therefore leave alone.
pub(crate) const FONT_SIZE: f64 = 13.0;

/// The shortcuts of the next and the previous tab, the same in both apps: arrows, since the defaults' brackets are
/// keys of their own only on some keyboard layouts.
pub(crate) const NEXT_TAB_OURS: &str = "ctrl+cmd+right";
pub(crate) const PREVIOUS_TAB_OURS: &str = "ctrl+cmd+left";

/// `count` lines of Rust in five shapes over and over, as the document crate's budget tests use.
pub(crate) fn rust_lines(count: usize) -> String {
    let shapes = [
        "    let value_{n} = compute(\"label {n}\", {n}) + 0x{x}; // note {n}",
        "    if value_{n} > {n} { return Err(Error::new(\"too big\")); }",
        "    /* block comment {n} */ items.push(Item { id: {n}, name: String::from(\"x\") });",
        "fn helper_{n}(input: &str) -> Result<usize, Error> {",
        "}",
    ];
    (0..count)
        .map(|n| {
            shapes[n % shapes.len()]
                .replace("{x}", &format!("{n:x}"))
                .replace("{n}", &n.to_string())
                + "\n"
        })
        .collect()
}

/// `count` lines of Markdown with every kind of block, a section of 18 lines over and over.
pub(crate) fn markdown_lines(count: usize) -> String {
    let section = [
        "## Section {n}",
        "",
        "Text with **bold**, *emphasis*, `code {n}` and a [link {n}](https://example.org/{n}), long enough to wrap.",
        "More of the same paragraph with _more_ words and ~~struck~~ ones, and a formula $x_{n}^2$ in it.",
        "",
        "- item {n} with `code`",
        "- [ ] task {n}",
        "",
        "> A quote {n} with a <b>tag</b>",
        "",
        "| col | value |",
        "|-----|-------|",
        "| a{n} | {n} |",
        "",
        "```rust",
        "fn f_{n}() -> u32 { let s = \"{n}\"; {n} }",
        "```",
        "",
    ];
    (0..count)
        .map(|n| section[n % section.len()].replace("{n}", &(n / section.len()).to_string()) + "\n")
        .collect()
}

/// Writes the project into `dir`, replacing what an earlier run or an app left there.
pub(crate) fn write_project(dir: &Path) -> Result<(), String> {
    files::create_dir_all(dir)?;
    files::write(dir.join(RUST_FILE), rust_lines(LINES))?;
    files::write(dir.join(MARKDOWN_FILE), markdown_lines(LINES))
}

/// Zed's `settings.json`.
pub(crate) fn zed_settings() -> Value {
    json!({
        "cursor_blink": false,
        "enable_language_server": false,
        "edit_predictions": { "provider": "none" },
        "show_edit_predictions": false,
        "disable_ai": true,
        "telemetry": { "diagnostics": false, "metrics": false },
        "auto_update": false,
        "buffer_font_size": FONT_SIZE,
        "preview_tabs": { "enabled": false },
        "restore_on_startup": "last_session",
        "confirm_quit": false,
        "minimap": { "show": "never" }
    })
}

/// Zed's `keymap.json`, with the tab shortcuts of [`NEXT_TAB_OURS`] in Zed's spelling.
pub(crate) fn zed_keymap() -> Value {
    json!([{
        "context": "Pane",
        "bindings": {
            "ctrl-cmd-right": "pane::ActivateNextItem",
            "ctrl-cmd-left": "pane::ActivatePreviousItem"
        }
    }])
}

/// This app's `settings.json`: no preview beside Markdown, which Zed does not show either, and the folder back at the
/// next launch, as Zed's `restore_on_startup` does.
pub(crate) fn our_settings() -> Value {
    let mut settings = json!({
        "markdownPreviewBeside": false,
        "reopenProjects": true,
        "inlayHints": false
    });
    // An experiment turns parts of the editor off through `COMPARE_DANGO_SETTINGS`, a JSON object laid over these.
    if let Some(Value::Object(extra)) = std::env::var("COMPARE_DANGO_SETTINGS")
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        && let Value::Object(base) = &mut settings
    {
        base.extend(extra);
    }
    settings
}

/// This app's `keybindings.json`.
pub(crate) fn our_keybindings() -> Value {
    json!({ "tab.next": NEXT_TAB_OURS, "tab.previous": PREVIOUS_TAB_OURS })
}

/// This app's `trust.json` (`ide_project::trust`), trusting `project`: an untrusted folder asks with a sheet first.
/// Trust lets the project run what it configures; the fixture configures nothing.
pub(crate) fn our_trust(project: &Path) -> Value {
    json!({ "version": 1, "roots": [project] })
}

/// Writes `value` as pretty JSON to `path`, creating its folder.
pub(crate) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        files::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| format!("{}: {e}", path.display()))?;
    files::write(path, text + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_have_ten_thousand_lines_and_the_typed_lines_are_long() {
        let rust = rust_lines(LINES);
        assert_eq!(rust.lines().count(), LINES);
        let typed = rust.lines().nth(RUST_LINE - 1).unwrap();
        assert!(
            typed.starts_with("    let value_5000 = compute(\"label 5000\""),
            "{typed}"
        );
        let markdown = markdown_lines(LINES);
        assert_eq!(markdown.lines().count(), LINES);
        assert_eq!(markdown.lines().nth(MARKDOWN_LINE - 1), Some("## Section 0"));
    }

    #[test]
    fn the_rust_file_is_the_one_the_document_tests_use() {
        assert_eq!(
            rust_lines(5),
            "    let value_0 = compute(\"label 0\", 0) + 0x0; // note 0\n    if value_1 > 1 { return \
             Err(Error::new(\"too big\")); }\n    /* block comment 2 */ items.push(Item { id: 2, name: \
             String::from(\"x\") });\nfn helper_3(input: &str) -> Result<usize, Error> {\n}\n"
        );
        assert!(rust_lines(30).contains("0x19; // note 25"));
    }

    #[test]
    fn both_apps_get_the_same_font_size_and_the_same_tab_keys() {
        assert_eq!(zed_settings()["buffer_font_size"], json!(FONT_SIZE));
        assert_eq!(zed_settings()["cursor_blink"], json!(false));
        assert_eq!(zed_settings()["enable_language_server"], json!(false));
        let bindings = &zed_keymap()[0]["bindings"];
        assert_eq!(bindings["ctrl-cmd-right"], "pane::ActivateNextItem");
        assert_eq!(our_keybindings()["tab.next"], NEXT_TAB_OURS);
        assert_eq!(
            NEXT_TAB_OURS.replace('+', "-"),
            "ctrl-cmd-right",
            "the same keys in both spellings"
        );
    }

    #[test]
    fn the_trust_file_names_the_project() {
        assert_eq!(
            our_trust(Path::new("/tmp/p")),
            json!({ "version": 1, "roots": ["/tmp/p"] })
        );
    }

    #[test]
    fn the_project_and_settings_are_written() {
        let dir = std::env::temp_dir().join(format!("xtask-compare-fixtures-{}", std::process::id()));
        write_project(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(MARKDOWN_FILE))
                .unwrap()
                .lines()
                .count(),
            LINES
        );
        write_json(&dir.join("deep/settings.json"), &zed_settings()).unwrap();
        let back: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("deep/settings.json")).unwrap()).unwrap();
        assert_eq!(back, zed_settings());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
