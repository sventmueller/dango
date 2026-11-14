//! Signature help (E84): the signature of the call around the cursor in a small window above it, on the system's
//! material and never taking the keyboard, with the active parameter set apart and read out by VoiceOver. The server's
//! trigger characters and "Editor ▸ Parameter zeigen" open it; ↑ and ↓ page through overloads while no completion list
//! shows; Esc closes it, and so does leaving the call, when the server no longer names a signature.

use std::cell::{Cell, RefCell};
use std::ops::Range;

use ide_lsp::lsp_types::{
    Documentation, ParameterLabel, SignatureHelp, SignatureHelpContext, SignatureHelpTriggerKind, SignatureInformation,
};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSBackingStoreType, NSColor, NSFont, NSFontAttributeName, NSFontWeightSemibold,
    NSForegroundColorAttributeName, NSLineBreakMode, NSPanel, NSTextField, NSTextInputClient, NSTextView,
    NSUnderlineStyleAttributeName, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
    NSVisualEffectView, NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{NSMutableAttributedString, NSNumber, NSPoint, NSRange, NSRect, NSSize};

use super::{server_of, text};
use crate::editor::Editor;
use crate::host as editor;
use ide_ui_common::constant;
use ide_ui_common::util::{main_soon, ns};

const PADDING: f64 = 6.0;
const MAX_WIDTH: f64 = 640.0;

/// The window and what it shows; one for the app, as only one editor has the keyboard.
struct Help {
    panel: Retained<NSPanel>,
    field: Retained<NSTextField>,
    view: Cell<Option<crate::EditorSerial>>,
    shown: RefCell<Option<SignatureHelp>>,
    /// The signature shown, which ↑ and ↓ change.
    active: Cell<usize>,
    /// Counts questions, so that an answer to an older one is dropped.
    asked: Cell<u64>,
    /// The revision and cursor the last question was about, so a cursor that rests is not asked about twice.
    about: Cell<(u64, usize)>,
    /// The parameter VoiceOver read last.
    announced: RefCell<String>,
}

/// The signature help on view.
#[derive(Default)]
pub struct ShownSignature {
    help: RefCell<Option<std::rc::Rc<Help>>>,
}

fn with_help<R>(f: impl FnOnce(&RefCell<Option<std::rc::Rc<Help>>>) -> R) -> R {
    f(&crate::host::state().signature_help.help)
}

fn help(mtm: MainThreadMarker) -> std::rc::Rc<Help> {
    with_help(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| std::rc::Rc::new(build(mtm)))
            .clone()
    })
}

fn build(mtm: MainThreadMarker) -> Help {
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 24.0)),
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    // SAFETY: Switching it off keeps AppKit from releasing the panel behind Rust's back; `Help` owns it.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setHasShadow(true);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setBecomesKeyOnlyIfNeeded(true);
    let material = NSVisualEffectView::new(mtm);
    material.setMaterial(NSVisualEffectMaterial::ToolTip);
    material.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    material.setState(NSVisualEffectState::Active);
    material.setWantsLayer(true);
    if let Some(layer) = material.layer() {
        layer.setCornerRadius(6.0);
        layer.setMasksToBounds(true);
    }
    let field = NSTextField::labelWithString(&ns(""), mtm);
    field.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    field.setPreferredMaxLayoutWidth(MAX_WIDTH);
    material.addSubview(&field);
    panel.setContentView(Some(&material));
    Help {
        panel,
        field,
        view: Cell::new(None),
        shown: RefCell::new(None),
        active: Cell::new(0),
        asked: Cell::new(0),
        about: Cell::new((u64::MAX, usize::MAX)),
        announced: RefCell::new(String::new()),
    }
}

/// The window, if it shows for `view`.
fn open_for(view: &dyn Editor) -> Option<std::rc::Rc<Help>> {
    let help = with_help(|cell| cell.borrow().clone())?;
    let ours = help.view.get() == Some(view.serial());
    (ours && help.panel.isVisible()).then_some(help)
}

/// The characters after which the server of `view` offers signatures, and those after which it updates them.
fn triggers(view: &dyn Editor) -> (Vec<String>, Vec<String>) {
    let provider = server_of(view)
        .and_then(|(server, _)| server.capabilities())
        .and_then(|c| c.signature_help_provider);
    provider.map_or_else(Default::default, |p| {
        (
            p.trigger_characters.unwrap_or_default(),
            p.retrigger_characters.unwrap_or_default(),
        )
    })
}

/// Asks for the signatures at the cursor, as the menu item does.
pub fn show(view: &dyn Editor) {
    let context = SignatureHelpContext {
        trigger_kind: SignatureHelpTriggerKind::INVOKED,
        trigger_character: None,
        is_retrigger: open_for(view).is_some(),
        active_signature_help: None,
    };
    request(view, context);
}

/// Text was typed into `view`: a trigger character opens the window, and while it shows, every keystroke updates it.
pub fn typed(view: &dyn Editor, typed: &str) {
    let open = open_for(view);
    if open.is_none() && server_of(view).is_none() {
        return;
    }
    let (triggers, retriggers) = triggers(view);
    let last = typed.chars().last().map(String::from).unwrap_or_default();
    let trigger = triggers.contains(&last) || (open.is_some() && retriggers.contains(&last));
    let about = (view.document().revision(), view.selectedRange().location);
    if !trigger && open.as_ref().is_none_or(|help| help.about.get() == about) {
        return;
    }
    let context = SignatureHelpContext {
        trigger_kind: if trigger {
            SignatureHelpTriggerKind::TRIGGER_CHARACTER
        } else {
            SignatureHelpTriggerKind::CONTENT_CHANGE
        },
        trigger_character: trigger.then_some(last),
        is_retrigger: open.is_some(),
        active_signature_help: open.and_then(|help| current(&help)),
    };
    request(view, context);
}

/// The cursor moved or the text changed while the window shows: it follows, or closes once the cursor left the call.
pub fn moved(view: &dyn Editor) {
    let Some(help) = open_for(view) else { return };
    let about = (view.document().revision(), view.selectedRange().location);
    if help.about.get() == about {
        return;
    }
    let context = SignatureHelpContext {
        trigger_kind: SignatureHelpTriggerKind::CONTENT_CHANGE,
        trigger_character: None,
        is_retrigger: true,
        active_signature_help: current(&help),
    };
    request(view, context);
}

/// What the window shows, with the overload the user paged to as the active one.
fn current(help: &Help) -> Option<SignatureHelp> {
    let mut shown = help.shown.borrow().clone()?;
    shown.active_signature = u32::try_from(help.active.get()).ok();
    Some(shown)
}

fn request(view: &dyn Editor, context: SignatureHelpContext) {
    let Some((server, uri)) = server_of(view) else { return };
    let cursor = view.selectedRange().location;
    let revision = view.document().revision();
    let Some(position) = text::position(view.document().text(), cursor) else {
        return;
    };
    let shared = help(view.mtm());
    let asked = shared.asked.get() + 1;
    shared.asked.set(asked);
    shared.about.set((revision, cursor));
    let serial = view.lsp().borrow().serial;
    let _ = server.signature_help(&uri, position, Some(context), move |answer| {
        main_soon(move || {
            let (Some(view), Some(mtm)) = (editor::find_editor(serial), MainThreadMarker::new()) else {
                return;
            };
            let help = help(mtm);
            if help.asked.get() != asked || view.document().revision() != revision {
                return;
            }
            match answer {
                Ok(Some(found)) if !found.signatures.is_empty() => present(&help, &*view, found),
                Ok(_) => close(&help),
                Err(error) => ide_ui_common::diag::report("signature help", error),
            }
        });
    });
}

/// A command of the keyboard reached `view`; whether the window took it: ↑ and ↓ between overloads, Esc to close.
pub fn command(view: &dyn Editor, command: Sel) -> bool {
    let Some(help) = open_for(view) else {
        return false;
    };
    if command == sel!(cancelOperation:) {
        close(&help);
        return true;
    }
    let count = help.shown.borrow().as_ref().map_or(0, |h| h.signatures.len());
    if count < 2 {
        return false;
    }
    let by = match command {
        c if c == sel!(moveUp:) => count - 1,
        c if c == sel!(moveDown:) => 1,
        _ => return false,
    };
    help.active.set((help.active.get() + by) % count);
    render(&help, view);
    true
}

/// Closes the window if it shows for `view`.
pub fn dismiss_for(view: &dyn Editor) {
    if let Some(help) = open_for(view) {
        close(&help);
    }
}

fn close(help: &Help) {
    if let Some(parent) = help.panel.parentWindow() {
        parent.removeChildWindow(&help.panel);
    }
    help.panel.orderOut(None);
    help.asked.set(help.asked.get() + 1);
    help.shown.borrow_mut().take();
    help.announced.borrow_mut().clear();
}

fn present(help: &Help, view: &dyn Editor, found: SignatureHelp) {
    let same_view = help.view.get() == Some(view.serial());
    let count = found.signatures.len();
    let active = match found.active_signature {
        Some(active) => usize::try_from(active).unwrap_or(0),
        None if same_view => help.active.get(),
        None => 0,
    };
    help.active.set(active.min(count - 1));
    help.view.set(Some(view.serial()));
    *help.shown.borrow_mut() = Some(found);
    render(help, view);
}

/// The index of the parameter the cursor is in, of `signature` within `found`.
fn active_parameter(found: &SignatureHelp, signature: &SignatureInformation) -> Option<usize> {
    let index = signature.active_parameter.or(found.active_parameter)?;
    usize::try_from(index).ok()
}

/// Where parameter `index` of `signature` is in its label, in UTF-16 code units.
fn parameter_range(signature: &SignatureInformation, index: usize) -> Option<Range<usize>> {
    let parameter = signature.parameters.as_ref()?.get(index)?;
    match &parameter.label {
        ParameterLabel::LabelOffsets([start, end]) => Some(usize::try_from(*start).ok()?..usize::try_from(*end).ok()?),
        ParameterLabel::Simple(name) => {
            let open = signature.label.find('(').unwrap_or(0);
            let byte = signature.label[open..].find(name.as_str())? + open;
            let start = signature.label[..byte].encode_utf16().count();
            Some(start..start + name.encode_utf16().count())
        }
    }
}

fn documentation_text(documentation: &Documentation) -> &str {
    match documentation {
        Documentation::String(text) => text,
        Documentation::MarkupContent(markup) => &markup.value,
    }
}

/// The first paragraph of the documentation of the active parameter, else of the signature.
fn note(signature: &SignatureInformation, parameter: Option<usize>) -> Option<String> {
    let of_parameter = parameter
        .and_then(|i| signature.parameters.as_ref()?.get(i)?.documentation.as_ref())
        .map(documentation_text);
    let text = of_parameter.or_else(|| signature.documentation.as_ref().map(documentation_text))?;
    let paragraph: Vec<&str> = text
        .trim()
        .lines()
        .take_while(|line| !line.trim().is_empty() && !line.starts_with("```"))
        .collect();
    let joined = paragraph.join(" ");
    (!joined.is_empty()).then_some(joined)
}

/// Fills the window with the signature shown and puts it above the cursor.
fn render(help: &Help, view: &dyn Editor) {
    let Some(found) = help.shown.borrow().clone() else {
        return;
    };
    let count = found.signatures.len();
    let Some(signature) = found.signatures.get(help.active.get()) else {
        return;
    };
    let parameter = active_parameter(&found, signature);
    let highlighted = parameter.and_then(|i| parameter_range(signature, i));
    let styled = styled(
        signature,
        highlighted.clone(),
        count,
        help.active.get(),
        note(signature, parameter),
    );
    help.field.setAttributedStringValue(&styled);
    let fitted = help.field.cell().map_or(NSSize::new(MAX_WIDTH, 20.0), |cell| {
        cell.cellSizeForBounds(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(MAX_WIDTH, 1.0e6)))
    });
    let size = NSSize::new(fitted.width.ceil().min(MAX_WIDTH), fitted.height.ceil());
    help.field.setFrame(NSRect::new(NSPoint::new(PADDING, PADDING), size));
    place(
        help,
        view,
        NSSize::new(size.width + 2.0 * PADDING, size.height + 2.0 * PADDING),
    );
    let units: Vec<u16> = signature.label.encode_utf16().collect();
    let spoken = highlighted
        .filter(|r| r.end <= units.len())
        .map(|r| String::from_utf16_lossy(&units[r]))
        .unwrap_or_default();
    let mut announced = help.announced.borrow_mut();
    if announced.is_empty() {
        let first = if spoken.is_empty() {
            signature.label.clone()
        } else {
            format!("{}, {spoken}", signature.label)
        };
        ide_ui_common::panel::announce(view.mtm(), &first);
    } else if *announced != spoken && !spoken.is_empty() {
        ide_ui_common::panel::announce(view.mtm(), &spoken);
    }
    *announced = if spoken.is_empty() {
        signature.label.clone()
    } else {
        spoken
    };
    help.field.setAccessibilityLabel(Some(&ns(&announced)));
}

/// The text of the window: which overload of how many, the signature with the parameter at `highlighted` in bold,
/// and below it what the documentation says.
fn styled(
    signature: &SignatureInformation,
    highlighted: Option<Range<usize>>,
    count: usize,
    active: usize,
    note: Option<String>,
) -> Retained<NSMutableAttributedString> {
    let code = crate::host::code_font(12.0);
    let (font_key, color_key) = (
        constant!(NSFontAttributeName),
        constant!(NSForegroundColorAttributeName),
    );
    let string = NSMutableAttributedString::new();
    let append = |text: &str, font: &NSFont, color: &NSColor| {
        let start = string.length();
        string.replaceCharactersInRange_withString(NSRange::new(start, 0), &ns(text));
        let range = NSRange::new(start, string.length() - start);
        // SAFETY: A font and a colour under the keys AppKit reads them from, over a range of the string.
        unsafe {
            string.addAttribute_value_range(font_key, font, range);
            string.addAttribute_value_range(color_key, color, range);
        }
        start
    };
    if count > 1 {
        append(
            &format!("{}/{count}  ", active + 1),
            &code,
            &NSColor::secondaryLabelColor(),
        );
    }
    let start = append(&signature.label, &code, &NSColor::labelColor());
    if let Some(range) = highlighted.filter(|r| start + r.end <= string.length()) {
        let at = NSRange::new(start + range.start, range.end - range.start);
        let bold =
            NSFont::monospacedSystemFontOfSize_weight(ide_ui_common::style::LABEL, constant!(NSFontWeightSemibold));
        let underline = constant!(NSUnderlineStyleAttributeName);
        // SAFETY: The font, colour and underline keys get a font, a colour and an NSNumber, over a range inside the string.
        unsafe {
            string.addAttribute_value_range(font_key, &bold, at);
            string.addAttribute_value_range(color_key, &NSColor::controlAccentColor(), at);
            string.addAttribute_value_range(underline, &NSNumber::new_i64(1), at);
        }
    }
    if let Some(note) = note {
        append("\n", &code, &NSColor::labelColor());
        append(
            &note,
            &NSFont::systemFontOfSize(ide_ui_common::style::SMALL),
            &NSColor::secondaryLabelColor(),
        );
    }
    string
}

/// Puts the window of `size` just above the cursor, or below it where the screen ends.
fn place(help: &Help, view: &dyn Editor, size: NSSize) {
    let Some(window) = view.window() else { return };
    let cursor = view.selectedRange().location;
    let text_view: &NSTextView = view;
    // SAFETY: A range within the text; no actual range is asked for.
    let caret =
        unsafe { text_view.firstRectForCharacterRange_actualRange(NSRange::new(cursor, 0), std::ptr::null_mut()) };
    let mut origin = NSPoint::new(caret.origin.x - PADDING, caret.origin.y + caret.size.height + 2.0);
    if let Some(screen) = window.screen() {
        let visible = screen.visibleFrame();
        if origin.y + size.height > visible.origin.y + visible.size.height {
            origin.y = caret.origin.y - size.height - 2.0;
        }
        origin.x = origin
            .x
            .min(visible.origin.x + visible.size.width - size.width)
            .max(visible.origin.x);
    }
    help.panel.setFrame_display(NSRect::new(origin, size), true);
    if help.panel.parentWindow().is_none() {
        // SAFETY: The window keeps a weak reference to the panel, which `Help` owns and `close` detaches.
        unsafe { window.addChildWindow_ordered(&help.panel, NSWindowOrderingMode::Above) };
    }
    help.panel.orderFront(None);
}

/// Whether the menu command `action` can run in `view`, for "Parameter zeigen"; `None` for every other one.
pub fn validate(view: &dyn Editor, action: Sel) -> Option<bool> {
    (action == sel!(ideSignatureHelp:)).then(|| {
        server_of(view).is_some_and(|(server, _)| {
            server
                .capabilities()
                .is_some_and(|c| c.signature_help_provider.is_some())
        })
    })
}

/// What the self-test reads from the window.
#[cfg(feature = "selftest")]
pub mod selftest {
    use crate::editor::Editor;

    /// The text the window shows for `view`, and the part set apart as the active parameter; `None` when closed.
    pub fn shown(view: &dyn Editor) -> Option<(String, String)> {
        let help = super::open_for(view)?;
        let text = help.field.stringValue().to_string();
        Some((text, help.announced.borrow().clone()))
    }

    /// The window, for a picture.
    pub fn panel() -> Option<objc2::rc::Retained<objc2_app_kit::NSPanel>> {
        super::with_help(|cell| cell.borrow().as_ref().map(|h| h.panel.clone()))
    }
}

#[cfg(test)]
mod tests {
    use ide_lsp::lsp_types::ParameterInformation;

    use super::*;

    fn signature(label: &str, parameters: Vec<ParameterLabel>) -> SignatureInformation {
        SignatureInformation {
            label: label.into(),
            documentation: None,
            parameters: Some(
                parameters
                    .into_iter()
                    .map(|label| ParameterInformation {
                        label,
                        documentation: None,
                    })
                    .collect(),
            ),
            active_parameter: None,
        }
    }

    #[test]
    fn a_parameter_is_found_by_its_offsets_or_by_its_name_after_the_parenthesis() {
        let by_name = signature(
            "fn n(n: u32, ä: u32) -> u32",
            vec![
                ParameterLabel::Simple("n: u32".into()),
                ParameterLabel::Simple("ä: u32".into()),
            ],
        );
        assert_eq!(parameter_range(&by_name, 0), Some(5..11));
        assert_eq!(parameter_range(&by_name, 1), Some(13..19));
        assert_eq!(parameter_range(&by_name, 2), None);
        let by_offsets = signature(
            "Repeat(s string, count int) string",
            vec![ParameterLabel::LabelOffsets([17, 26])],
        );
        assert_eq!(parameter_range(&by_offsets, 0), Some(17..26));
    }

    #[test]
    fn the_signature_says_which_parameter_unless_the_answer_does() {
        let mut one = signature("f(a, b)", vec![]);
        let found = SignatureHelp {
            signatures: vec![one.clone()],
            active_signature: None,
            active_parameter: Some(1),
        };
        assert_eq!(active_parameter(&found, &one), Some(1));
        one.active_parameter = Some(0);
        assert_eq!(active_parameter(&found, &one), Some(0));
    }

    #[test]
    fn the_note_is_the_first_paragraph_of_the_parameters_documentation_else_the_signatures() {
        let mut documented = signature("f(a)", vec![ParameterLabel::Simple("a".into())]);
        documented.documentation = Some(Documentation::String("Does f.\nReally.\n\nMore.".into()));
        assert_eq!(note(&documented, Some(0)).as_deref(), Some("Does f. Really."));
        if let Some(parameters) = documented.parameters.as_mut() {
            parameters[0].documentation = Some(Documentation::String("The a.".into()));
        }
        assert_eq!(note(&documented, Some(0)).as_deref(), Some("The a."));
        assert_eq!(note(&signature("g()", vec![]), None), None);
    }
}
