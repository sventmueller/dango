//! Inlay hints (E83): what the language server knows that the text does not say, such as inferred types and parameter
//! names, shown through the editor's insets (E81) for the lines on screen. Asked for again after a pause in typing,
//! after scrolling and when the server asks for a refresh. On by default, off for all editors or for one language.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use ide_core::settings::{Settings, keys};
use ide_lsp::lsp_types::{
    InlayHint, InlayHintLabel, InlayHintLabelPartTooltip, InlayHintTooltip, MarkupContent, Range as LspRange,
};
use objc2::sel;
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn, NSMenuItem};
use splat_i18n::tf;

use super::{CONTENT_MODIFIED, language_of, server_of, text};
use crate::lsp::editor::Editor;
use crate::lsp::editor::Hint;
use crate::lsp::host as editor;
use ide_ui_common::util::main_soon;

/// How long scrolling pauses before the hints of the lines that came into view are asked for.
const SCROLL_DELAY: f64 = 0.15;

thread_local! {
    /// Whether hints are on, and the languages that leave them out, as the user last chose.
    static CHOSEN: RefCell<Option<(bool, Vec<String>)>> = const { RefCell::new(None) };
}

/// Where the inlay hint requests of the editors stand.
#[derive(Default)]
pub struct InlayRequests {
    /// The last question of each editor by serial, so that the answer to an older one is dropped.
    asked: RefCell<HashMap<u64, u64>>,
    next: Cell<u64>,
    /// The document revision the hints of each editor by serial belong to. While it holds, an answer for the lines on
    /// screen keeps the hints elsewhere, so scrolling back neither waits for them nor moves the text again.
    shown: RefCell<HashMap<u64, u64>>,
}

fn with_asked<R>(f: impl FnOnce(&RefCell<HashMap<u64, u64>>) -> R) -> R {
    f(&crate::lsp::host::state().inlay_requests.asked)
}

fn with_next<R>(f: impl FnOnce(&Cell<u64>) -> R) -> R {
    f(&crate::lsp::host::state().inlay_requests.next)
}

fn with_shown<R>(f: impl FnOnce(&RefCell<HashMap<u64, u64>>) -> R) -> R {
    f(&crate::lsp::host::state().inlay_requests.shown)
}

fn chosen() -> (bool, Vec<String>) {
    CHOSEN.with(|chosen| {
        chosen
            .borrow_mut()
            .get_or_insert_with(|| {
                let settings = Settings::load_user();
                (settings.get(&keys::INLAY_HINTS), settings.get(&keys::INLAY_HINTS_OFF))
            })
            .clone()
    })
}

/// Whether editors show inlay hints at all.
pub fn enabled() -> bool {
    chosen().0
}

/// Whether editors of the language named `language` show inlay hints, when they are on.
pub fn enabled_for(language: &str) -> bool {
    !chosen().1.iter().any(|off| off == language)
}

/// The settings changed elsewhere, as in the settings window: every editor follows them.
pub fn settings_changed() {
    CHOSEN.with(|chosen| chosen.borrow_mut().take());
    for view in editor::all_editors() {
        request(&*view);
    }
}

/// Turns inlay hints on or off for every editor and remembers the choice.
pub fn toggle() {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::INLAY_HINTS, &!enabled()) {
        ide_ui_common::diag::report("saving the inlay hints setting", error);
    }
    settings_changed();
}

/// Turns inlay hints on or off for the language named `language` and remembers the choice.
pub fn toggle_language(language: &str) {
    let mut off = chosen().1;
    if off.iter().any(|l| l == language) {
        off.retain(|l| l != language);
    } else {
        off.push(language.to_string());
        off.sort();
    }
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::INLAY_HINTS_OFF, &off) {
        ide_ui_common::diag::report("saving the inlay hints setting", error);
    }
    settings_changed();
}

/// The name of the language whose server serves `view`.
pub fn language(view: &dyn Editor) -> Option<String> {
    let path = view.document().path()?.to_path_buf();
    language_of(&path).map(|(language, _)| language.name().to_string())
}

fn shows(view: &dyn Editor) -> bool {
    enabled() && language(view).is_some_and(|name| enabled_for(&name))
}

/// The menu items of inlay hints: ticked when on, the one for the editor's language named after it and greyed out
/// without a language; `None` for every other item.
pub fn validate(view: &dyn Editor, item: &NSMenuItem) -> Option<bool> {
    let action = item.action()?;
    let tick = |on: bool| {
        item.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        })
    };
    if action == sel!(ideToggleInlayHints:) {
        tick(enabled());
        return Some(true);
    }
    if action == sel!(ideToggleInlayHintsLanguage:) {
        let name = language(view);
        item.setTitle(&ide_ui_common::util::ns(&tf(
            "Inlay-Hinweise für {}",
            &[name.as_deref().unwrap_or("…")],
        )));
        tick(name.as_deref().is_some_and(enabled_for));
        return Some(name.is_some() && enabled());
    }
    None
}

/// Asks the server of `view` for the hints of the lines on screen, or takes them away where they are off.
pub fn request(view: &dyn Editor) {
    if !shows(view) {
        view.clear_inlay_hints();
        return;
    }
    let Some((server, uri)) = server_of(view) else {
        return;
    };
    let visible = view.visible_utf16();
    let range = {
        let document = view.document();
        let text = document.text();
        match (text::position(text, visible.start), text::position(text, visible.end)) {
            (Some(start), Some(end)) => LspRange::new(start, end),
            _ => return,
        }
    };
    let serial = view.lsp().borrow().serial;
    let revision = view.document().revision();
    let asked = with_next(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    with_asked(|all| all.borrow_mut().insert(serial, asked));
    let sent = server.inlay_hints(&uri, range, move |answer| {
        main_soon(move || arrived(serial, revision, asked, visible, answer));
    });
    if let Err(ide_lsp::Error::Unsupported(_)) = sent {
        view.clear_inlay_hints();
    }
}

/// The view of `view` scrolled: the hints follow once it rests.
pub fn scrolled(view: &dyn Editor) {
    if shows(view) {
        view.later(sel!(ideInlayHints), SCROLL_DELAY);
    }
}

/// The server's hints of `view` may have changed everywhere: the next answer replaces them all.
pub fn forget(view: &dyn Editor) {
    let serial = view.lsp().borrow().serial;
    with_shown(|shown| shown.borrow_mut().remove(&serial));
}

fn arrived(
    serial: u64,
    revision: u64,
    asked: u64,
    lines: std::ops::Range<usize>,
    answer: Result<Vec<InlayHint>, ide_lsp::Error>,
) {
    if with_asked(|all| all.borrow().get(&serial).copied()) != Some(asked) {
        return;
    }
    let Some(view) = editor::find_editor(serial) else {
        with_asked(|all| all.borrow_mut().remove(&serial));
        return;
    };
    match answer {
        Ok(hints) if view.document().revision() == revision && shows(&*view) => {
            let hints: Vec<Hint> = {
                let document = view.document();
                hints.into_iter().map(|hint| hint_of(document.text(), hint)).collect()
            };
            let same = with_shown(|shown| shown.borrow_mut().insert(serial, revision)) == Some(revision);
            view.set_inlay_hints(hints, same.then_some(lines));
        }
        Ok(_) => view.later(sel!(ideInlayHints), SCROLL_DELAY),
        Err(ide_lsp::Error::Server { code, .. }) if code == CONTENT_MODIFIED => {
            view.later(sel!(ideInlayHints), SCROLL_DELAY);
        }
        Err(ide_lsp::Error::Unsupported(_)) => view.clear_inlay_hints(),
        Err(_) => {}
    }
}

/// The text of a tooltip, without the fences of a code block.
fn plain(markup: &MarkupContent) -> String {
    markup
        .value
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What one hint in `text` shows, says on hover and where a click on it goes.
fn hint_of(text: &ide_document::Text, hint: InlayHint) -> Hint {
    let at = text::offset(text, hint.position);
    let (label, part_tooltip, location) = match hint.label {
        InlayHintLabel::String(label) => (label, None, None),
        InlayHintLabel::LabelParts(parts) => {
            let label: String = parts.iter().map(|part| part.value.as_str()).collect();
            let tooltip = parts.iter().find_map(|part| match &part.tooltip {
                Some(InlayHintLabelPartTooltip::String(text)) => Some(text.clone()),
                Some(InlayHintLabelPartTooltip::MarkupContent(markup)) => Some(plain(markup)),
                None => None,
            });
            let location = parts.into_iter().find_map(|part| part.location);
            (label, tooltip, location)
        }
    };
    let tooltip = match hint.tooltip {
        Some(InlayHintTooltip::String(text)) => Some(text),
        Some(InlayHintTooltip::MarkupContent(markup)) => Some(plain(&markup)),
        None => part_tooltip,
    }
    .filter(|text| !text.trim().is_empty());
    Hint {
        at,
        text: label.trim().to_string(),
        tooltip,
        pad_left: hint.padding_left.unwrap_or(false) || label.starts_with(' '),
        pad_right: hint.padding_right.unwrap_or(false) || label.ends_with(' '),
        target: location.and_then(|location| Some((ide_lsp::uri::to_path(&location.uri)?, location.range))),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ide_lsp::lsp_types::{InlayHintLabelPart, Location, MarkupKind, Position, Uri};

    use super::*;

    fn hint(label: InlayHintLabel) -> InlayHint {
        InlayHint {
            position: Position::new(1, 9),
            label,
            kind: None,
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        }
    }

    #[test]
    fn a_hint_sits_at_its_position_with_its_label_and_tooltip() {
        let text = ide_document::Text::new("fn main() {\n    let total = 1;\n}\n");
        let mut plain = hint(InlayHintLabel::String(": i32".into()));
        plain.tooltip = Some(InlayHintTooltip::String("the type".into()));
        plain.padding_left = Some(true);
        let shown = hint_of(&text, plain);
        assert_eq!(shown.at, 21);
        assert_eq!(shown.text, ": i32");
        assert_eq!(shown.tooltip.as_deref(), Some("the type"));
        assert!(shown.pad_left && !shown.pad_right);
        assert!(shown.target.is_none(), "nothing to jump to");
    }

    #[test]
    fn label_parts_join_and_the_first_located_part_gives_the_jump() {
        let text = ide_document::Text::new("a\nlet v = x;\n");
        let location = Location::new(
            Uri::from_str("file:///tmp/lib.rs").unwrap(),
            ide_lsp::lsp_types::Range::default(),
        );
        let parts = vec![
            InlayHintLabelPart {
                value: ": ".into(),
                ..InlayHintLabelPart::default()
            },
            InlayHintLabelPart {
                value: "Vec".into(),
                tooltip: Some(InlayHintLabelPartTooltip::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "```rust\nstruct Vec<T>\n```".into(),
                })),
                location: Some(location),
                command: None,
            },
            InlayHintLabelPart {
                value: "<u8> ".into(),
                ..InlayHintLabelPart::default()
            },
        ];
        let shown = hint_of(&text, hint(InlayHintLabel::LabelParts(parts)));
        assert_eq!(shown.text, ": Vec<u8>");
        assert_eq!(
            shown.tooltip.as_deref(),
            Some("struct Vec<T>"),
            "without the code fences"
        );
        assert!(shown.pad_right, "a label ending in a space keeps it as room");
        assert!(shown.target.is_some());
    }
}
