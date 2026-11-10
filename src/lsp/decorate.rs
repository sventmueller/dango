//! What the server adds to the look of the text: semantic token colours over the lexer's, as TextKit 2 rendering
//! attributes that leave the text storage, its undo and the lexer's colours alone, set for the lines on screen; and
//! diagnostics as wavy lines in a view over the text, placed by TextKit 2's segment geometry, since TextKit 2 draws
//! no underline from a rendering attribute.

use std::collections::BTreeMap;
use std::ops::Range;
use std::ptr::NonNull;

use block2::RcBlock;

use ide_document::{Change, Unit};
use ide_lsp::SemanticTokens;
use ide_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, Bool};
use objc2::{AllocAnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSForegroundColorAttributeName, NSTextContainer,
    NSTextContentStorage, NSTextElementProvider, NSTextLayoutManagerSegmentOptions, NSTextLayoutManagerSegmentType,
    NSTextRange, NSView, NSWindowOrderingMode,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect};
use splat_markdown::highlight::Token;

use super::ViewState;
use crate::editor::EditorTextView;
use crate::util::constant;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

impl Severity {
    fn of(diagnostic: &Diagnostic) -> Severity {
        match diagnostic.severity {
            Some(DiagnosticSeverity::WARNING) => Severity::Warning,
            Some(DiagnosticSeverity::INFORMATION) => Severity::Info,
            Some(DiagnosticSeverity::HINT) => Severity::Hint,
            _ => Severity::Error,
        }
    }

    pub fn color(self) -> Retained<NSColor> {
        match self {
            Severity::Error => NSColor::systemRedColor(),
            Severity::Warning => NSColor::systemOrangeColor(),
            Severity::Info => NSColor::systemBlueColor(),
            Severity::Hint => NSColor::tertiaryLabelColor(),
        }
    }
}

/// A diagnostic where it is in the document now, in UTF-16 code units.
#[derive(Debug, Clone)]
pub struct Mark {
    pub range: Range<usize>,
    pub severity: Severity,
    pub message: String,
}

/// A stretch the server gave a colour of its own.
#[derive(Debug, Clone)]
pub struct Painted {
    pub range: Range<usize>,
    pub token: Token,
}

/// The palette colour of a semantic token type; variables, parameters and what the lexer colours keep their look.
fn token_of(kind: &str) -> Option<Token> {
    Some(match kind {
        "type" | "class" | "struct" | "enum" | "interface" | "typeParameter" | "typeAlias" | "union" | "trait"
        | "builtinType" | "namespace" | "selfTypeKeyword" => Token::Url,
        "function" | "method" => Token::Math,
        "macro" | "decorator" | "attribute" | "derive" => Token::Html,
        "enumMember" | "constParameter" => Token::LinkText,
        _ => return None,
    })
}

/// Moves `range` with an edit; `false` when the edit touched it.
pub fn follow(range: &mut Range<usize>, change: &Change) -> bool {
    let old = &change.utf16;
    if range.end <= old.start {
        return true;
    }
    if range.start >= old.end {
        range.start = range.start - (old.end - old.start) + change.text_utf16;
        range.end = range.end - (old.end - old.start) + change.text_utf16;
        return true;
    }
    let new_end = old.start + change.text_utf16;
    range.start = range.start.min(old.start);
    range.end = if range.end <= old.end {
        new_end
    } else {
        range.end - (old.end - old.start) + change.text_utf16
    };
    false
}

/// Keeps diagnostics and colours on their text while the user types, until the server sends new ones.
pub fn shift(state: &mut ViewState, change: &Change) {
    for mark in &mut state.marks {
        follow(&mut mark.range, change);
    }
    state.tokens.retain_mut(|painted| follow(&mut painted.range, change));
}

pub fn set_diagnostics(view: &EditorTextView, diagnostics: &[Diagnostic]) {
    let mut marks: Vec<Mark> = {
        let document = view.document();
        diagnostics
            .iter()
            .map(|d| {
                let code = match &d.code {
                    Some(NumberOrString::String(code)) => format!(" [{code}]"),
                    Some(NumberOrString::Number(code)) => format!(" [{code}]"),
                    None => String::new(),
                };
                Mark {
                    range: super::text::range(document.text(), d.range),
                    severity: Severity::of(d),
                    message: format!("{}{code}", d.message),
                }
            })
            .collect()
    };
    marks.sort_by_key(|m| (m.range.start, std::cmp::Reverse(m.severity)));
    view.ivars().lsp.borrow_mut().marks = marks;
    paint(view);
    crate::editor::redraw_gutter(view);
    super::status::refresh(view);
}

pub fn set_tokens(view: &EditorTextView, tokens: &SemanticTokens) {
    let painted: Vec<Painted> = {
        let document = view.document();
        let text = document.text();
        let mut line_start = (u32::MAX, 0usize);
        tokens
            .tokens
            .iter()
            .filter_map(|token| {
                let kind = token_of(tokens.kind(token)?)?;
                if line_start.0 != token.line {
                    let line = usize::try_from(token.line).ok()?;
                    let byte = text.offset(line, 0, Unit::Utf16).ok()?;
                    line_start = (token.line, text.byte_to_utf16(byte).ok()?);
                }
                let start = line_start.1 + usize::try_from(token.start).ok()?;
                let end = start + usize::try_from(token.length).ok()?;
                Some(Painted {
                    range: start..end.min(text.len_utf16()),
                    token: kind,
                })
            })
            .collect()
    };
    view.ivars().lsp.borrow_mut().tokens = painted;
    paint(view);
}

/// The diagnostics by the line they start on, the most serious first, for the line number column.
pub fn line_marks(view: &EditorTextView) -> BTreeMap<usize, (Severity, String)> {
    let state = view.ivars().lsp.borrow();
    let document = view.document();
    let text = document.text();
    let mut lines: BTreeMap<usize, (Severity, String)> = BTreeMap::new();
    for mark in state.marks.iter().filter(|m| m.severity >= Severity::Info) {
        let Ok(byte) = text.utf16_to_byte(mark.range.start.min(text.len_utf16())) else {
            continue;
        };
        let entry = lines
            .entry(text.line_of(byte))
            .or_insert_with(|| (mark.severity, String::new()));
        entry.0 = entry.0.max(mark.severity);
        if !entry.1.is_empty() {
            entry.1.push('\n');
        }
        entry.1.push_str(&mark.message);
    }
    lines
}

/// The TextKit 2 range of the UTF-16 offsets `range`.
pub fn text_range(content: &NSTextContentStorage, range: &Range<usize>) -> Option<Retained<NSTextRange>> {
    let start = content.documentRange().location();
    let from = content.locationFromLocation_withOffset(&start, isize::try_from(range.start).ok()?)?;
    let to = content.locationFromLocation_withOffset(&start, isize::try_from(range.end).ok()?)?;
    NSTextRange::initWithLocation_endLocation(NSTextRange::alloc(), &from, Some(&to))
}

/// Sets the colours of the lines on screen from the view's tokens and redraws its diagnostics.
pub fn paint(view: &EditorTextView) {
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return;
    };
    let visible = crate::editor::visible_utf16(view);
    let Some(whole) = text_range(&content, &visible) else {
        return;
    };
    let foreground = constant!(NSForegroundColorAttributeName);
    layout.removeRenderingAttribute_forTextRange(foreground, &whole);
    let state = view.ivars().lsp.borrow();
    let inside = |range: &Range<usize>| range.end > visible.start && range.start < visible.end;
    for painted in state.tokens.iter().filter(|p| inside(&p.range)) {
        if let Some(range) = text_range(&content, &painted.range) {
            let color: Retained<AnyObject> = crate::editor::palette_color(painted.token).into();
            // SAFETY: The value of the foreground colour attribute is a colour.
            unsafe { layout.addRenderingAttribute_value_forTextRange(foreground, Some(&color), &range) };
        }
    }
    drop(state);
    if let Some(overlay) = view.ivars().lsp.borrow().squiggles.as_ref().and_then(Weak::load) {
        overlay.setNeedsDisplay(true);
    }
}

pub struct SquiggleIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Squiggles` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESquiggles"]
    #[ivars = SquiggleIvars]
    pub struct Squiggles;

    unsafe impl NSObjectProtocol for Squiggles {}

    impl Squiggles {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> Option<Retained<NSView>> {
            None
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            crate::util::guarded((), || self.draw(dirty));
        }
    }
);

impl Squiggles {
    /// The overlay of `editor`, covering it and following its size.
    pub fn attach(editor: &EditorTextView) {
        let this = Squiggles::alloc(editor.mtm()).set_ivars(SquiggleIvars {
            editor: Weak::from(editor),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Squiggles> = unsafe { msg_send![super(this), initWithFrame: editor.bounds()] };
        this.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        this.setAccessibilityElement(false);
        editor.addSubview_positioned_relativeTo(&this, NSWindowOrderingMode::Above, None);
        editor.ivars().lsp.borrow_mut().squiggles = Some(Weak::from(&*this));
    }

    fn draw(&self, dirty: NSRect) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let (Some(layout), Some(content)) = (editor.textLayoutManager(), editor.textContentStorage()) else {
            return;
        };
        let origin = editor.textContainerOrigin();
        let visible = crate::editor::visible_utf16(&editor);
        let length = editor.document().text().len_utf16();
        let marks: Vec<(Range<usize>, Severity)> = editor
            .ivars()
            .lsp
            .borrow()
            .marks
            .iter()
            .filter(|m| m.severity >= Severity::Info)
            .map(|m| (shown_range(&m.range, length), m.severity))
            .filter(|(range, _)| range.end > visible.start && range.start < visible.end)
            .filter(|(range, _)| !crate::editor::hidden_by_fold(&editor, range.start))
            .collect();
        for (range, severity) in marks {
            let Some(text_range) = text_range(&content, &range) else {
                continue;
            };
            severity.color().setStroke();
            let block = RcBlock::new(
                |_: *mut NSTextRange, frame: NSRect, _: f64, _: NonNull<NSTextContainer>| -> Bool {
                    let frame = NSRect::new(
                        NSPoint::new(frame.origin.x + origin.x, frame.origin.y + origin.y),
                        frame.size,
                    );
                    if frame.origin.y <= dirty.origin.y + dirty.size.height
                        && frame.origin.y + frame.size.height >= dirty.origin.y
                    {
                        wave(&frame);
                    }
                    Bool::YES
                },
            );
            layout.enumerateTextSegmentsInRange_type_options_usingBlock(
                &text_range,
                NSTextLayoutManagerSegmentType::Standard,
                NSTextLayoutManagerSegmentOptions::RangeNotRequired,
                &block,
            );
        }
    }
}

/// A diagnostic's range as it is drawn: an empty one covers the character after it, or before it at the end.
fn shown_range(range: &Range<usize>, length: usize) -> Range<usize> {
    if !range.is_empty() {
        return range.clone();
    }
    if range.end < length {
        range.start..range.end + 1
    } else {
        range.start.saturating_sub(1)..range.end
    }
}

/// A wavy line along the bottom of `frame`.
fn wave(frame: &NSRect) {
    let path = NSBezierPath::bezierPath();
    let bottom = frame.origin.y + frame.size.height - 1.0;
    let end = frame.origin.x + frame.size.width.max(4.0);
    let mut x = frame.origin.x;
    path.moveToPoint(NSPoint::new(x, bottom));
    let mut up = true;
    while x < end {
        x += 2.0;
        path.lineToPoint(NSPoint::new(x, if up { bottom - 2.0 } else { bottom }));
        up = !up;
    }
    path.setLineWidth(1.1);
    path.stroke();
}

#[cfg(test)]
mod tests {
    use ide_document::{Document, Edit};

    use super::*;

    fn change(text: &str, range: Range<usize>, new: &str) -> Change {
        let mut document = Document::untitled(text);
        document
            .apply(Edit {
                range_utf16: range,
                text: new.into(),
            })
            .unwrap()
    }

    #[test]
    fn ranges_follow_edits_before_and_after_them_and_note_edits_inside() {
        let text = "0123456789";
        let mut after = 5..7;
        assert!(follow(&mut after, &change(text, 1..2, "abc")));
        assert_eq!(after, 7..9);
        let mut before = 1..3;
        assert!(
            follow(&mut before, &change(text, 3..3, "x")),
            "typing right after a range leaves it"
        );
        assert_eq!(before, 1..3);
        let mut inside = 2..6;
        assert!(!follow(&mut inside, &change(text, 3..4, "")));
        assert_eq!(inside, 2..5);
        let mut covered = 4..5;
        assert!(!follow(&mut covered, &change(text, 2..8, "zz")));
        assert_eq!(covered, 2..4);
    }

    #[test]
    fn token_types_map_to_the_palette_where_the_lexer_cannot_tell() {
        assert_eq!(token_of("struct"), Some(Token::Url));
        assert_eq!(token_of("function"), Some(Token::Math));
        assert_eq!(token_of("variable"), None);
        assert_eq!(token_of("keyword"), None);
    }
}
