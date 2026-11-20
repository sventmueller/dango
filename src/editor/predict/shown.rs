//! How a proposal shows in the editor: ghost text in a gap after the caret (E81), the lines below the caret's line or
//! a change as before and after in a box under the line, on the system's popover material, the replaced range marked
//! in the text; the hint "Tab übernimmt" after ghost text at a line's end or a box, or "Tab: Zeile 42" after the
//! caret's line for a change elsewhere; and the switch in the status bar. Nothing here moves: with or without
//! "Bewegung reduzieren", ghost text appears without an animation.

use std::ops::Range;
use std::path::Path;

use ide_ui_common::alert::{Alert, Role, first};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSBackgroundColorAttributeName, NSBox, NSBoxType, NSButton, NSCellImagePosition, NSColor, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSPasteboard, NSPasteboardTypeString, NSStackView,
    NSStackViewGravity, NSStrikethroughStyleAttributeName, NSTextField, NSTitlePosition, NSUnderlineStyleAttributeName,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindow,
};
use objc2_foundation::{NSMutableAttributedString, NSNumber, NSPoint, NSRange, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use super::model::{Kind, Proposal};
use crate::editor::EditorTextView;
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::util::{constant, main_soon, ns, set_action, symbol};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

/// Room between the box's edge and its text.
const PADDING: f64 = 6.0;
/// The most lines the box shows; the rest is cut off with a line of its own.
const MOST_LINES: usize = 14;

/// The views a proposal and the status bar use.
#[derive(Default)]
pub(crate) struct Parts {
    boxed: Option<Retained<NSBox>>,
    field: Option<Retained<NSTextField>>,
    /// The range of the text marked as changing, in UTF-16 code units.
    marked: Option<Range<usize>>,
    button: Option<Retained<NSButton>>,
}

/// Shows `proposal` in `view`.
pub(crate) fn present(view: &EditorTextView, proposal: &Proposal) {
    let at = super::utf16(view, proposal.range.start);
    match &proposal.kind {
        Kind::Insert { first, rest } => {
            let inline = !first.is_empty() && insets::gap_possible(view, at);
            let line_end = at_line_end(view, at);
            let mut shown = Vec::new();
            if inline {
                // At the line's end the ghost text needs no gap, which spares the text an attribute change per keystroke.
                let place = if line_end { Place::LineEnd } else { Place::Inline };
                let mut ghost = Inset::new(at, first.clone(), place);
                ghost.ghost = true;
                ghost.tooltip = Some(t("Vorschlag: Tab übernimmt, Esc verwirft").to_string());
                shown.push(ghost);
            }
            let below = if inline || first.is_empty() {
                rest.clone()
            } else {
                Some(proposal.text.clone())
            };
            let below = below.filter(|lines| !lines.is_empty());
            if let Some(below) = &below {
                show_box(view, at, &ghost_lines(view, below));
            }
            // Beside ghost text within the line the hint would make every keystroke lay out the rest of the line (E25).
            let hinted = if inline { line_end } else { below.is_some() };
            if hinted {
                shown.push(accept_hint(at));
            }
            if !shown.is_empty() {
                insets::set(view, Layer::Prediction, shown);
            }
        }
        Kind::Change => {
            let end = super::utf16(view, proposal.range.end);
            mark(view, at..end);
            show_box(view, end, &before_after(view, &proposal.old, &proposal.text));
            insets::set(view, Layer::Prediction, vec![accept_hint(end)]);
        }
        Kind::Elsewhere { line } => {
            let caret = view.selectedRange().location;
            let mut hint = Inset::new(caret, tf("Tab: Zeile {}", &[&(line + 1).to_string()]), Place::LineEnd);
            hint.tooltip = Some(t("Tab springt zum Vorschlag, ein zweites Tab übernimmt ihn").to_string());
            insets::set(view, Layer::Prediction, vec![hint]);
        }
    }
}

/// The hint after the end of the line holding UTF-16 offset `at` that Tab takes the proposal shown there.
fn accept_hint(at: usize) -> Inset {
    let mut hint = Inset::new(at, t("Tab übernimmt"), Place::LineEnd);
    hint.tooltip = Some(t("Vorschlag: Tab übernimmt, Esc verwirft").to_string());
    hint
}

/// Whether UTF-16 offset `at` of `view` is where its line's text ends.
fn at_line_end(view: &EditorTextView, at: usize) -> bool {
    let document = view.document();
    let text = document.text();
    let Ok(byte) = text.utf16_to_byte(at) else {
        return false;
    };
    text.line_content(text.line_of(byte))
        .is_some_and(|content| content.end == byte)
}

/// Takes everything a proposal showed out of `view`.
pub(crate) fn clear(view: &EditorTextView) {
    insets::clear(view, Layer::Prediction);
    let (boxed, marked) = {
        let mut state = view.ivars().predict.borrow_mut();
        state.parts.field = None;
        (state.parts.boxed.take(), state.parts.marked.take())
    };
    if let Some(boxed) = boxed {
        boxed.removeFromSuperview();
    }
    if let Some(range) = marked {
        unmark(view, range);
    }
}

fn mark(view: &EditorTextView, range: Range<usize>) {
    if range.is_empty() {
        return;
    }
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return;
    };
    let Some(text_range) = ide_ui_lsp::text_range(&content, &range) else {
        return;
    };
    let color: Retained<AnyObject> = NSColor::systemOrangeColor().colorWithAlphaComponent(0.3).into();
    let underline: Retained<AnyObject> = NSNumber::new_i64(1).into();
    let (background, style) = (
        constant!(NSBackgroundColorAttributeName),
        constant!(NSUnderlineStyleAttributeName),
    );
    // SAFETY: A colour under the background colour's key and a number under the underline style's, as they expect.
    unsafe {
        layout.addRenderingAttribute_value_forTextRange(background, Some(&color), &text_range);
        layout.addRenderingAttribute_value_forTextRange(style, Some(&underline), &text_range);
    }
    view.ivars().predict.borrow_mut().parts.marked = Some(range);
}

fn unmark(view: &EditorTextView, range: Range<usize>) {
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return;
    };
    let length = view.document().text().len_utf16();
    let range = range.start.min(length)..range.end.min(length);
    if let Some(text_range) = ide_ui_lsp::text_range(&content, &range) {
        layout.removeRenderingAttribute_forTextRange(constant!(NSBackgroundColorAttributeName), &text_range);
        layout.removeRenderingAttribute_forTextRange(constant!(NSUnderlineStyleAttributeName), &text_range);
    }
}

/// `text` cut to the lines the box shows.
fn shortened(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= MOST_LINES {
        return text.trim_end_matches('\n').to_string();
    }
    let mut kept = lines[..MOST_LINES].join("\n");
    kept.push('\n');
    kept.push_str(&tf("… {} weitere Zeilen", &[&(lines.len() - MOST_LINES).to_string()]));
    kept
}

fn append(string: &NSMutableAttributedString, text: &str, font: &NSFont, color: &NSColor, struck: bool) {
    let start = string.length();
    string.replaceCharactersInRange_withString(NSRange::new(start, 0), &ns(text));
    let range = NSRange::new(start, string.length() - start);
    let (font_key, color_key, strike_key) = (
        constant!(NSFontAttributeName),
        constant!(NSForegroundColorAttributeName),
        constant!(NSStrikethroughStyleAttributeName),
    );
    // SAFETY: A font, a colour and a number under the keys that take them.
    unsafe {
        string.addAttribute_value_range(font_key, font, range);
        string.addAttribute_value_range(color_key, color, range);
        // New text takes the attributes of the text before it, the strikethrough too.
        string.addAttribute_value_range(strike_key, &NSNumber::new_i64(i64::from(struck)), range);
    }
}

/// The lines below the caret's line, styled as the ghost text in the line.
fn ghost_lines(view: &EditorTextView, text: &str) -> Retained<NSMutableAttributedString> {
    let string = NSMutableAttributedString::new();
    let font = crate::editor::style::styled_code_font(view.ivars().font_size.get(), false, true);
    append(&string, &shortened(text), &font, &NSColor::secondaryLabelColor(), false);
    string
}

/// The text a change replaces, struck through after a minus, and what it puts there after a plus.
fn before_after(view: &EditorTextView, old: &str, new: &str) -> Retained<NSMutableAttributedString> {
    let string = NSMutableAttributedString::new();
    let font = crate::editor::code_font(view.ivars().font_size.get());
    let mut first = true;
    for (sign, text, color, struck) in [
        ("−", old, NSColor::systemRedColor(), true),
        ("+", new, NSColor::systemGreenColor(), false),
    ] {
        if text.is_empty() {
            continue;
        }
        for line in shortened(text).lines() {
            if !first {
                append(&string, "\n", &font, &color, false);
            }
            first = false;
            append(&string, &format!("{sign} "), &font, &color, false);
            append(&string, line, &font, &color, struck);
        }
    }
    string
}

/// Puts the box with `text` under the line that holds UTF-16 offset `at`, at the left edge of the text.
fn show_box(view: &EditorTextView, at: usize, text: &NSMutableAttributedString) {
    let Some(line) = ide_ui_lsp::rect_of(view, at..at) else {
        return;
    };
    let mtm = view.mtm();
    let field = NSTextField::labelWithAttributedString(text, mtm);
    field.setSelectable(false);
    field.setAccessibilityLabel(Some(&ns(&tf("Vorschlag: {}", &[&text.string().to_string()]))));
    let size = field.fittingSize();
    let material = NSVisualEffectView::new(mtm);
    material.setMaterial(NSVisualEffectMaterial::Popover);
    material.setBlendingMode(NSVisualEffectBlendingMode::WithinWindow);
    material.setState(NSVisualEffectState::Active);
    // The material lets the code below shine through, which would read as part of the proposal; the box's own ground
    // keeps it out.
    let boxed = NSBox::new(mtm);
    boxed.setBoxType(NSBoxType::Custom);
    boxed.setTitlePosition(NSTitlePosition::NoTitle);
    boxed.setFillColor(&NSColor::textBackgroundColor());
    boxed.setBorderColor(&NSColor::separatorColor());
    boxed.setBorderWidth(1.0);
    boxed.setCornerRadius(6.0);
    boxed.setContentViewMargins(NSSize::new(0.0, 0.0));
    boxed.setContentView(Some(&material));
    let width = (size.width + 2.0 * PADDING).min(view.visibleRect().size.width.max(200.0));
    let left = view.textContainerOrigin().x;
    boxed.setFrame(NSRect::new(
        NSPoint::new(left, line.origin.y + line.size.height + 1.0),
        NSSize::new(width, size.height + 2.0 * PADDING),
    ));
    field.setFrame(NSRect::new(NSPoint::new(PADDING, PADDING), size));
    material.addSubview(&field);
    view.addSubview(&boxed);
    let mut state = view.ivars().predict.borrow_mut();
    state.parts.boxed = Some(boxed);
    state.parts.field = Some(field);
}

/// Adds the switch to the status bar's `stack`, sending `idePrediction:` with `tag`.
pub(crate) fn status_item(mtm: MainThreadMarker, view: &EditorTextView, stack: &NSStackView, tag: isize) {
    // SAFETY: A nil target until `set_action` below, which checks that the view answers the action.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), None, None, mtm) };
    button.setBordered(false);
    button.setFont(Some(&NSFont::systemFontOfSize(ide_ui_common::style::SMALL)));
    button.setImage(symbol("text.cursor", t("Vorschläge beim Tippen")).as_deref());
    button.setImagePosition(NSCellImagePosition::ImageLeading);
    button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    button.setTag(tag);
    set_action(&button, view, sel!(idePrediction:));
    stack.addView_inGravity(&button, NSStackViewGravity::Trailing);
    view.ivars().predict.borrow_mut().parts.button = Some(button);
}

/// Shows `title` in the status bar's switch, with `tip` as its tooltip; greyed out when it can do nothing.
pub(crate) fn set_status(view: &EditorTextView, title: &str, tip: &str, enabled: bool) {
    let Ok(state) = view.ivars().predict.try_borrow() else {
        return;
    };
    if let Some(button) = &state.parts.button {
        button.setTitle(&ns(title));
        button.setToolTip(Some(&ns(tip)));
        button.setEnabled(enabled);
    }
}

/// Asks once whether `provider`, which is not on this Mac, may get the text proposals are made from, and says what
/// goes there (E101); `then` runs when the user allows it.
pub(crate) fn ask_consent(window: &NSWindow, provider: &str, then: impl FnOnce() + 'static) {
    Alert::new(window.mtm(), &tf("Code an {} senden?", &[provider]))
        .informative(&tf(
            "Für Vorschläge beim Tippen gehen an {}: die Datei, in der du tippst, mit Pfad und Sprache. Dateien auf der Ausschlussliste und Ordner, denen du nicht vertraust, bleiben außen vor. Die Erlaubnis lässt sich in den Einstellungen unter „Vorhersage“ zurücknehmen.",
            &[provider],
        ))
        .button(t("Erlauben"), Role::Plain)
        .button(t("Abbrechen"), Role::Plain)
        .sheet(window, move |response| {
            if first(response) {
                then();
            }
        });
}

/// Signs in to Copilot of `root` through GitHub's device flow, with the code in a sheet on the settings window.
pub(crate) fn sign_in_sheet(root: &Path) {
    let Some(copilot) = super::provider::copilot(root) else {
        return;
    };
    let finisher = copilot.clone();
    copilot.sign_in(move |answer| {
        main_soon(move || {
            let Some(window) = crate::settings::window() else {
                return;
            };
            match answer {
                Ok(ide_predict::copilot::SignIn::Device(code)) => device_code(&window, &code, finisher),
                Ok(ide_predict::copilot::SignIn::Already { .. }) => {}
                Err(error) => {
                    Alert::new(window.mtm(), t("Anmeldung bei Copilot fehlgeschlagen"))
                        .informative(&ide_core::explain::Explain::explain(&error))
                        .sheet(&window, |_| {});
                }
            }
        });
    });
}

fn device_code(
    window: &NSWindow,
    code: &ide_predict::copilot::DeviceCode,
    copilot: std::sync::Arc<ide_predict::copilot::Copilot>,
) {
    let user_code = code.user_code.clone();
    Alert::new(window.mtm(), t("Bei GitHub anmelden"))
        .informative(&tf(
            "Gib auf {} den Code {} ein. „Code kopieren und öffnen“ legt ihn in die Zwischenablage und öffnet die Seite im Browser.",
            &[&code.verification_uri, &code.user_code],
        ))
        .button(t("Code kopieren und öffnen"), Role::Plain)
        .button(t("Abbrechen"), Role::Plain)
        .sheet(window, move |response| {
            if !first(response) {
                return;
            }
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();
            // SAFETY: A string under the pasteboard's string type.
            unsafe { pasteboard.setString_forType(&NSString::from_str(&user_code), NSPasteboardTypeString) };
            copilot.finish_sign_in(ide_predict::Cancel::new());
        });
}
