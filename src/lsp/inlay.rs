//! Inlay hints (E83): what the language server knows that the text does not say, such as inferred types and parameter
//! names, shown through the editor's insets (E81) for the lines on screen. Asked for again after a pause in typing,
//! after scrolling and when the server asks for a refresh. On by default, off for all editors or for one language.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use ide_core::settings::{Settings, keys};
use ide_lsp::lsp_types::{
    InlayHint, InlayHintLabel, InlayHintLabelPartTooltip, InlayHintTooltip, MarkupContent, Range as LspRange,
};
use objc2::{DefinedClass, sel};
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn, NSMenuItem};
use splat_i18n::tf;

use super::{CONTENT_MODIFIED, language_of, server_of, text};
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::editor::{self, EditorTextView};
use crate::util::main_soon;

/// How long scrolling pauses before the hints of the lines that came into view are asked for.
const SCROLL_DELAY: f64 = 0.15;

thread_local! {
    /// Whether hints are on, and the languages that leave them out, as the user last chose.
    static CHOSEN: RefCell<Option<(bool, Vec<String>)>> = const { RefCell::new(None) };
    /// The last question of each editor by serial, so that the answer to an older one is dropped.
    static ASKED: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(0) };
    /// The document revision the hints of each editor by serial belong to. While it holds, an answer for the lines on
    /// screen keeps the hints elsewhere, so scrolling back neither waits for them nor moves the text again.
    static SHOWN: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
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
        request(&view);
    }
}

/// Turns inlay hints on or off for every editor and remembers the choice.
pub fn toggle() {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::INLAY_HINTS, &!enabled()) {
        crate::diag::report("saving the inlay hints setting", error);
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
        crate::diag::report("saving the inlay hints setting", error);
    }
    settings_changed();
}

/// The name of the language whose server serves `view`.
pub fn language(view: &EditorTextView) -> Option<String> {
    let path = view.document().path()?.to_path_buf();
    language_of(&path).map(|(language, _)| language.name().to_string())
}

fn shows(view: &EditorTextView) -> bool {
    enabled() && language(view).is_some_and(|name| enabled_for(&name))
}

/// The menu items of inlay hints: ticked when on, the one for the editor's language named after it and greyed out
/// without a language; `None` for every other item.
pub fn validate(view: &EditorTextView, item: &NSMenuItem) -> Option<bool> {
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
        item.setTitle(&crate::util::ns(&tf(
            "Inlay-Hinweise für {}",
            &[name.as_deref().unwrap_or("…")],
        )));
        tick(name.as_deref().is_some_and(enabled_for));
        return Some(name.is_some() && enabled());
    }
    None
}

/// Asks the server of `view` for the hints of the lines on screen, or takes them away where they are off.
pub fn request(view: &EditorTextView) {
    if !shows(view) {
        insets::clear(view, Layer::Inlay);
        return;
    }
    let Some((server, uri)) = server_of(view) else {
        return;
    };
    let visible = editor::visible_utf16(view);
    let range = {
        let document = view.document();
        let text = document.text();
        match (text::position(text, visible.start), text::position(text, visible.end)) {
            (Some(start), Some(end)) => LspRange::new(start, end),
            _ => return,
        }
    };
    let serial = view.ivars().lsp.borrow().serial;
    let revision = view.document().revision();
    let asked = NEXT.with(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    ASKED.with(|all| all.borrow_mut().insert(serial, asked));
    let sent = server.inlay_hints(&uri, range, move |answer| {
        main_soon(move || arrived(serial, revision, asked, visible, answer));
    });
    if let Err(ide_lsp::Error::Unsupported(_)) = sent {
        insets::clear(view, Layer::Inlay);
    }
}

/// The view of `view` scrolled: the hints follow once it rests.
pub fn scrolled(view: &EditorTextView) {
    if shows(view) {
        view.later(sel!(ideInlayHints), SCROLL_DELAY);
    }
}

/// The server's hints of `view` may have changed everywhere: the next answer replaces them all.
pub fn forget(view: &EditorTextView) {
    let serial = view.ivars().lsp.borrow().serial;
    SHOWN.with(|shown| shown.borrow_mut().remove(&serial));
}

fn arrived(
    serial: u64,
    revision: u64,
    asked: u64,
    lines: std::ops::Range<usize>,
    answer: Result<Vec<InlayHint>, ide_lsp::Error>,
) {
    if ASKED.with(|all| all.borrow().get(&serial).copied()) != Some(asked) {
        return;
    }
    let Some(view) = editor::find_editor(serial) else {
        ASKED.with(|all| all.borrow_mut().remove(&serial));
        return;
    };
    match answer {
        Ok(hints) if view.document().revision() == revision && shows(&view) => {
            let mut insets: Vec<Inset> = {
                let document = view.document();
                hints.into_iter().map(|hint| inset(document.text(), hint)).collect()
            };
            let same = SHOWN.with(|shown| shown.borrow_mut().insert(serial, revision)) == Some(revision);
            if same {
                let elsewhere = insets::of(&view, Layer::Inlay)
                    .into_iter()
                    .filter(|kept| !lines.contains(&kept.at));
                insets.extend(elsewhere);
            }
            insets::set(&view, Layer::Inlay, insets);
        }
        Ok(_) => view.later(sel!(ideInlayHints), SCROLL_DELAY),
        Err(ide_lsp::Error::Server { code, .. }) if code == CONTENT_MODIFIED => {
            view.later(sel!(ideInlayHints), SCROLL_DELAY);
        }
        Err(ide_lsp::Error::Unsupported(_)) => insets::clear(&view, Layer::Inlay),
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

/// What one hint in `text` shows, says on hover and does on a click.
fn inset(text: &ide_document::Text, hint: InlayHint) -> Inset {
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
    let mut inset = Inset::new(at, label.trim().to_string(), Place::Inline);
    inset.tooltip = match hint.tooltip {
        Some(InlayHintTooltip::String(text)) => Some(text),
        Some(InlayHintTooltip::MarkupContent(markup)) => Some(plain(&markup)),
        None => part_tooltip,
    }
    .filter(|text| !text.trim().is_empty());
    inset.pad_left = hint.padding_left.unwrap_or(false) || label.starts_with(' ');
    inset.pad_right = hint.padding_right.unwrap_or(false) || label.ends_with(' ');
    if let Some(location) = location
        && let Some(path) = ide_lsp::uri::to_path(&location.uri)
    {
        let range = location.range;
        inset.action = Some(Rc::new(move |view: &EditorTextView| {
            if let Some(controller) = view.controller() {
                super::navigate::open_at(&controller, &path, range);
            }
        }));
    }
    inset
}

/// The hints `view` shows, as their offsets and texts, for the self-test.
#[cfg(feature = "selftest")]
pub fn shown(view: &EditorTextView) -> Vec<(usize, String, bool)> {
    insets::of(view, Layer::Inlay)
        .into_iter()
        .map(|inset| (inset.at, inset.text, inset.action.is_some()))
        .collect()
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
        let shown = inset(&text, plain);
        assert_eq!(shown.at, 21);
        assert_eq!(shown.text, ": i32");
        assert_eq!(shown.place, Place::Inline);
        assert_eq!(shown.tooltip.as_deref(), Some("the type"));
        assert!(shown.pad_left && !shown.pad_right);
        assert!(shown.action.is_none(), "nothing to jump to");
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
        let shown = inset(&text, hint(InlayHintLabel::LabelParts(parts)));
        assert_eq!(shown.text, ": Vec<u8>");
        assert_eq!(
            shown.tooltip.as_deref(),
            Some("struct Vec<T>"),
            "without the code fences"
        );
        assert!(shown.pad_right, "a label ending in a space keeps it as room");
        assert!(shown.action.is_some());
    }
}
