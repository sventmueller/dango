//! Editor reactions to settings, matched against their declarations at the serialization boundary.

use super::*;
use ide_core::settings::keys;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Minimap,
    StickyLines,
    LineDiagnostics,
    Whitespace,
    Blame,
    Vim,
}

impl Change {
    fn named(name: &str) -> Option<Self> {
        [
            (keys::MINIMAP.declared().name, Self::Minimap),
            (keys::STICKY_SCROLL_LINES.declared().name, Self::StickyLines),
            (keys::DIAGNOSTICS_AT_LINE_END.declared().name, Self::LineDiagnostics),
            (keys::SHOW_WHITESPACE.declared().name, Self::Whitespace),
            (keys::BLAME_AT_LINE_END.declared().name, Self::Blame),
            (keys::VIM_MODE.declared().name, Self::Vim),
        ]
        .into_iter()
        .find_map(|(key, change)| (key == name).then_some(change))
    }
}

pub(super) fn apply(cx: &crate::context::AppContext, names: &[String]) {
    for change in names.iter().filter_map(|name| Change::named(name)) {
        match change {
            Change::Minimap => cx.editor().minimap.forget(),
            Change::StickyLines => cx.editor().sticky.forget(),
            Change::LineDiagnostics => cx.editor().line_diagnostics.forget(),
            Change::Whitespace => whitespace::forget(),
            Change::Blame => blame::forget(),
            Change::Vim => {
                cx.editor().vim.forget();
                for view in cx.editors().live() {
                    vim::attach(&view);
                }
            }
        }
    }
    for view in cx.editors().live() {
        if let Some(map) = view.ivars().around.minimap.borrow().as_ref().and_then(Weak::load) {
            map.fit();
        }
        sticky::refresh(&view);
        line_diagnostics::refresh(&view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_names_keep_their_editor_reactions() {
        for (name, expected) in [
            ("minimap", Change::Minimap),
            ("stickyScrollLines", Change::StickyLines),
            ("diagnosticsAtLineEnd", Change::LineDiagnostics),
            ("showWhitespace", Change::Whitespace),
            ("blameAtLineEnd", Change::Blame),
            ("vimMode", Change::Vim),
        ] {
            assert_eq!(Change::named(name), Some(expected));
        }
        assert_eq!(Change::named("fontSize"), None);
        assert_eq!(Change::named(""), None);
    }
}
