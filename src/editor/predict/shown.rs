//! How a proposal shows in the editor: ghost text in a gap after the caret (E81), the lines below the caret's line or
//! a change as before and after in a box under the line, on the system's popover material, the replaced range marked
//! in the text; the hint "Tab: Zeile 42" at the end of the caret's line for a change elsewhere; and the switch in the
//! status bar. Nothing here moves: with or without "Bewegung reduzieren", ghost text appears without an animation.

use std::cell::Cell;
use std::ops::Range;
use std::path::Path;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSBackgroundColorAttributeName, NSBox, NSBoxType, NSButton,
    NSCellImagePosition, NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSModalResponse,
    NSPasteboard, NSPasteboardTypeString, NSStackView, NSStackViewGravity, NSStrikethroughStyleAttributeName,
    NSTextField, NSTitlePosition, NSUnderlineStyleAttributeName, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindow,
};
use objc2_foundation::{NSMutableAttributedString, NSNumber, NSPoint, NSRange, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use super::model::{Kind, Proposal};
use crate::editor::EditorTextView;
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::util::{constant, guarded, main_soon, ns, set_action, symbol};

/// Room between the box's edge and its text.
const PADDING: f64 = 6.0;
/// The most lines the box shows; the rest is cut off with a line of its own.
const MOST_LINES: usize = 14;

/// The views a proposal and the status bar use.
#[derive(Default)]
pub struct Parts {
    boxed: Option<Retained<NSBox>>,
    field: Option<Retained<NSTextField>>,
    /// The range of the text marked as changing, in UTF-16 code units.
    marked: Option<Range<usize>>,
    button: Option<Retained<NSButton>>,
}

/// Shows `proposal` in `view`.
pub fn present(view: &EditorTextView, proposal: &Proposal) {
    let at = super::utf16(view, proposal.range.start);
    match &proposal.kind {
        Kind::Insert { first, rest } => {
            let inline = !first.is_empty() && insets::gap_possible(view, at);
            if inline {
                // At the line's end the ghost text needs no gap, which spares the text an attribute change per keystroke.
                let place = if at_line_end(view, at) {
                    Place::LineEnd
                } else {
                    Place::Inline
                };
                let mut ghost = Inset::new(at, first.clone(), place);
                ghost.ghost = true;
                ghost.tooltip = Some(t("Vorschlag: Tab übernimmt, Esc verwirft").to_string());
                insets::set(view, Layer::Prediction, vec![ghost]);
            }
            let below = if inline || first.is_empty() {
                rest.clone()
            } else {
                Some(proposal.text.clone())
            };
            if let Some(below) = below.filter(|lines| !lines.is_empty()) {
                show_box(view, at, &ghost_lines(view, &below));
            }
        }
        Kind::Change => {
            let end = super::utf16(view, proposal.range.end);
            mark(view, at..end);
            show_box(view, end, &before_after(view, &proposal.old, &proposal.text));
        }
        Kind::Elsewhere { line } => {
            let caret = view.selectedRange().location;
            let mut hint = Inset::new(caret, tf("Tab: Zeile {}", &[&(line + 1).to_string()]), Place::LineEnd);
            hint.tooltip = Some(t("Tab springt zum Vorschlag, ein zweites Tab übernimmt ihn").to_string());
            insets::set(view, Layer::Prediction, vec![hint]);
        }
    }
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
pub fn clear(view: &EditorTextView) {
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
    let Some(text_range) = crate::lsp::text_range(&content, &range) else {
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
    if let Some(text_range) = crate::lsp::text_range(&content, &range) {
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
    let Some(line) = crate::lsp::rect_of(view, at..at) else {
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
pub fn status_item(mtm: MainThreadMarker, view: &EditorTextView, stack: &NSStackView, tag: isize) {
    // SAFETY: A nil target until `set_action` below, which checks that the view answers the action.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), None, None, mtm) };
    button.setBordered(false);
    button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    button.setImage(symbol("text.cursor", t("Vorschläge beim Tippen")).as_deref());
    button.setImagePosition(NSCellImagePosition::ImageLeading);
    button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    button.setTag(tag);
    set_action(&button, view, sel!(idePrediction:));
    stack.addView_inGravity(&button, NSStackViewGravity::Trailing);
    view.ivars().predict.borrow_mut().parts.button = Some(button);
}

/// Shows `title` in the status bar's switch, with `tip` as its tooltip; greyed out when it can do nothing.
pub fn set_status(view: &EditorTextView, title: &str, tip: &str, enabled: bool) {
    let Ok(state) = view.ivars().predict.try_borrow() else {
        return;
    };
    if let Some(button) = &state.parts.button {
        button.setTitle(&ns(title));
        button.setToolTip(Some(&ns(tip)));
        button.setEnabled(enabled);
    }
}

/// Shows `alert` as a sheet on `window`; `then` learns whether its first button ended it.
fn sheet(alert: &NSAlert, window: &NSWindow, then: impl FnOnce(bool) + 'static) {
    let then = Cell::new(Some(then));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            if let Some(then) = then.take() {
                then(response == NSAlertFirstButtonReturn);
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// Asks once whether `provider`, which is not on this Mac, may get the text proposals are made from, and says what
/// goes there (E101); `then` runs when the user allows it.
pub fn ask_consent(window: &NSWindow, provider: &str, then: impl FnOnce() + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(&tf("Code an {} senden?", &[provider])));
    alert.setInformativeText(&ns(&tf(
        "Für Vorschläge beim Tippen gehen an {}: die Datei, in der du tippst, mit Pfad und Sprache. Dateien auf der Ausschlussliste und Ordner, denen du nicht vertraust, bleiben außen vor. Die Erlaubnis lässt sich in den Einstellungen unter „Vorhersage“ zurücknehmen.",
        &[provider],
    )));
    alert.addButtonWithTitle(&ns(t("Erlauben")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    sheet(&alert, window, move |agreed| {
        if agreed {
            then();
        }
    });
}

/// Signs in to Copilot of `root` through GitHub's device flow, with the code in a sheet on the settings window.
pub fn sign_in_sheet(root: &Path) {
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
                    let alert = NSAlert::new(window.mtm());
                    alert.setMessageText(&ns(t("Anmeldung bei Copilot fehlgeschlagen")));
                    alert.setInformativeText(&ns(&format!("{error:?}")));
                    sheet(&alert, &window, |_| {});
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
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(t("Bei GitHub anmelden")));
    alert.setInformativeText(&ns(&tf(
        "Gib auf {} den Code {} ein. „Code kopieren und öffnen“ legt ihn in die Zwischenablage und öffnet die Seite im Browser.",
        &[&code.verification_uri, &code.user_code],
    )));
    alert.addButtonWithTitle(&ns(t("Code kopieren und öffnen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let user_code = code.user_code.clone();
    sheet(&alert, window, move |agreed| {
        if !agreed {
            return;
        }
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        // SAFETY: A string under the pasteboard's string type.
        unsafe { pasteboard.setString_forType(&NSString::from_str(&user_code), NSPasteboardTypeString) };
        copilot.finish_sign_in(ide_predict::Cancel::new());
    });
}

/// The text of the box under the line, if one shows, for the self-test.
#[cfg(feature = "selftest")]
pub fn boxed_text(view: &EditorTextView) -> Option<String> {
    let state = view.ivars().predict.borrow();
    state.parts.field.as_ref().map(|field| field.stringValue().to_string())
}

/// The range of the text marked as changing, for the self-test.
#[cfg(feature = "selftest")]
pub fn marked(view: &EditorTextView) -> Option<Range<usize>> {
    view.ivars().predict.borrow().parts.marked.clone()
}

/// The status bar's switch: its title and whether it is enabled, for the self-test.
#[cfg(feature = "selftest")]
pub fn status_shown(view: &EditorTextView) -> Option<(String, bool)> {
    let state = view.ivars().predict.borrow();
    state
        .parts
        .button
        .as_ref()
        .map(|button| (button.title().to_string(), button.isEnabled()))
}
