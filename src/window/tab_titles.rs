//! The titles of a window's native tabs, which tell apart tabs whose documents share a name, such as an HTML file's
//! editor and its page in a browser, in the tab bar and the overview of all tabs.

use std::path::Path;

use ide_ui_common::window::TabKind;
use splat_i18n::t;

/// What a tab is named after: its document in front.
pub(super) struct Named<'a> {
    pub(super) title: &'a str,
    pub(super) kind: TabKind,
    /// What the document shows, such as a file's path or a page's address.
    pub(super) key: Option<&'a str>,
}

/// The title of each of `tabs`: a name that only one carries stays, else an editor keeps it while another kind adds
/// what it is, and tabs of the same kind add the folder of their file.
pub(super) fn distinct(tabs: &[Named]) -> Vec<String> {
    tabs.iter()
        .map(|tab| {
            let same: Vec<&Named> = tabs.iter().filter(|other| other.title == tab.title).collect();
            let suffix = if same.len() < 2 {
                None
            } else if same.iter().filter(|other| other.kind == tab.kind).count() < 2 {
                (tab.kind != TabKind::Editor)
                    .then(|| kind_name(tab.kind))
                    .flatten()
                    .map(str::to_string)
            } else {
                tab.key.and_then(folder)
            };
            match suffix {
                Some(suffix) => format!("{} – {suffix}", tab.title),
                None => tab.title.to_string(),
            }
        })
        .collect()
}

fn kind_name(kind: TabKind) -> Option<&'static str> {
    match kind {
        TabKind::Browser => Some(t("Browser")),
        TabKind::Preview => Some(t("Vorschau")),
        TabKind::Terminal => Some(t("Terminal")),
        TabKind::Agent => Some(t("Agent")),
        TabKind::Plan => Some(t("Plan")),
        TabKind::Diff => Some(t("Änderungen")),
        _ => None,
    }
}

/// The name of the folder that holds the file `key`, unless `key` is an address.
fn folder(key: &str) -> Option<String> {
    if key.contains("://") {
        return None;
    }
    Path::new(key)
        .parent()?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named<'a>(title: &'a str, kind: TabKind, key: Option<&'a str>) -> Named<'a> {
        Named { title, kind, key }
    }

    #[test]
    fn names_only_one_tab_carries_stay() {
        let tabs = [
            named("a.rs", TabKind::Editor, Some("/p/a.rs")),
            named("Terminal", TabKind::Terminal, None),
        ];
        assert_eq!(distinct(&tabs), ["a.rs", "Terminal"]);
    }

    #[test]
    fn a_page_named_like_its_file_says_it_is_a_browser() {
        let tabs = [
            named("dango_qa.html", TabKind::Editor, Some("/p/dango_qa.html")),
            named("dango_qa.html", TabKind::Browser, Some("file:///p/dango_qa.html")),
        ];
        let page = format!("dango_qa.html – {}", t("Browser"));
        assert_eq!(distinct(&tabs), ["dango_qa.html", page.as_str()]);
    }

    #[test]
    fn editors_of_files_with_one_name_add_their_folder() {
        let tabs = [
            named("mod.rs", TabKind::Editor, Some("/p/src/window/mod.rs")),
            named("mod.rs", TabKind::Editor, Some("/p/src/editor/mod.rs")),
            named("lib.rs", TabKind::Editor, Some("/p/src/lib.rs")),
        ];
        assert_eq!(distinct(&tabs), ["mod.rs – window", "mod.rs – editor", "lib.rs"]);
    }

    #[test]
    fn pages_of_one_title_keep_it_without_a_folder() {
        let tabs = [
            named("Docs", TabKind::Browser, Some("https://a.example/docs")),
            named("Docs", TabKind::Browser, Some("https://b.example/docs")),
        ];
        assert_eq!(distinct(&tabs), ["Docs", "Docs"]);
    }

    #[test]
    fn a_kind_without_a_name_keeps_the_title() {
        let tabs = [
            named("Willkommen", TabKind::Editor, Some("/p/Willkommen")),
            named("Willkommen", TabKind::Welcome, None),
        ];
        assert_eq!(distinct(&tabs), ["Willkommen", "Willkommen"]);
    }
}
