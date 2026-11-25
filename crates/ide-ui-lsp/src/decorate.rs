//! What the server adds to the look of the text: semantic token colours over the lexer's, as TextKit 2 rendering
//! attributes that leave the text storage, its undo and the lexer's colours alone, set for the lines on screen; and
//! diagnostics as wavy lines in a view over the text, placed by TextKit 2's segment geometry, since TextKit 2 draws
//! no underline from a rendering attribute.

use std::collections::BTreeMap;
use std::ops::Range;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;

use ide_document::{Change, Unit};
use ide_lsp::SemanticTokens;
use ide_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, Bool};
use objc2::{AllocAnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAffineTransformNSAppKitAdditions, NSBezierPath, NSColor, NSForegroundColorAttributeName,
    NSTextContainer, NSTextContentStorage, NSTextElementProvider, NSTextLayoutManagerSegmentOptions,
    NSTextLayoutManagerSegmentType, NSTextRange, NSView, NSWindowOrderingMode,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect};
use splat_markdown::highlight::Token;

use super::ViewState;
use crate::editor::Editor;
use ide_ui_common::constant;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[expect(missing_docs, reason = "the names say it")]
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

    /// The symbol that tells the severity apart without its colour (E101).
    pub fn symbol(self) -> &'static str {
        match self {
            Severity::Error => "✕",
            Severity::Warning => "⚠",
            Severity::Info => "ℹ",
            Severity::Hint => "·",
        }
    }

    /// The severity as VoiceOver says it.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Error => splat_i18n::t("Fehler"),
            Severity::Warning => splat_i18n::t("Warnung"),
            Severity::Info => splat_i18n::t("Information"),
            Severity::Hint => splat_i18n::t("Hinweis"),
        }
    }

    /// The system colour its marks and symbols show in.
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
pub(crate) struct Mark {
    pub(crate) range: Range<usize>,
    pub(crate) severity: Severity,
    pub(crate) message: String,
    /// As the server sent it, for the code actions of its place.
    pub(crate) diagnostic: Diagnostic,
    /// The slot of the server that sent it, which replaces only its own (E140).
    pub(crate) source: u64,
}

/// A stretch the server gave a colour of its own.
#[derive(Debug, Clone)]
pub struct Painted {
    /// The stretch, in UTF-16 offsets of the document.
    pub range: Range<usize>,
    /// The palette colour it shows in.
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

/// What of the colours on screen edits and scrolling left stale since they were last painted.
#[derive(Debug, Default)]
pub(crate) struct Repaint {
    /// The text the edits touched, with the tokens they took from it. TextKit 2 moves the rendering attributes around
    /// it with the text, so the rest stays right.
    edited: Option<Range<usize>>,
    /// Other lines came on screen.
    pub(crate) screen: bool,
}

/// `a` and `b` together, with what lies between them.
fn union(a: Option<Range<usize>>, b: Range<usize>) -> Range<usize> {
    a.map_or_else(|| b.clone(), |a| a.start.min(b.start)..a.end.max(b.end))
}

/// Keeps diagnostics and colours on their text while the user types, until the server sends new ones.
pub(crate) fn shift(state: &mut ViewState, change: &Change) {
    let old = &change.utf16;
    let (removed, added) = (old.end - old.start, change.text_utf16);
    let behind = state.marks.partition_point(|m| m.range.start < old.end);
    for mark in &mut state.marks[..behind] {
        follow(&mut mark.range, change);
    }
    for mark in &mut state.marks[behind..] {
        // An empty mark where nothing was removed stays in front of what is typed there, as `follow` keeps it.
        if mark.range.end > old.start {
            mark.range = mark.range.start - removed + added..mark.range.end - removed + added;
        }
    }
    let moved = state.repaint.edited.take().map(|mut range| {
        follow(&mut range, change);
        range
    });
    let mut edited = union(moved, old.start..old.start + added);
    let tokens = &mut state.tokens;
    let first = tokens.partition_point(|p| p.range.end <= old.start);
    let behind = first + tokens[first..].partition_point(|p| p.range.start < old.end);
    for painted in tokens.drain(first..behind) {
        let mut range = painted.range;
        follow(&mut range, change);
        edited = union(Some(edited), range);
    }
    for painted in &mut tokens[first..] {
        painted.range = painted.range.start - removed + added..painted.range.end - removed + added;
    }
    state.repaint.edited = Some(edited);
}

/// The tokens of `tokens`, ordered by place and apart from each other, that reach into `range`.
fn overlapping<'a>(tokens: &'a [Painted], range: &Range<usize>) -> &'a [Painted] {
    let mut first = tokens.partition_point(|p| p.range.start < range.start);
    while first > 0 && tokens[first - 1].range.end > range.start {
        first -= 1;
    }
    let last = tokens.partition_point(|p| p.range.start < range.end).max(first);
    &tokens[first..last]
}

/// Shows `diagnostics` of the server of slot `source` in `view`, in place of what that server sent before; the other
/// servers' stay.
pub(crate) fn set_diagnostics_from(view: &dyn Editor, source: u64, diagnostics: &[Diagnostic]) {
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
                    diagnostic: d.clone(),
                    source,
                }
            })
            .collect()
    };
    let old = std::mem::take(&mut view.lsp().borrow_mut().marks);
    marks = replaced(old, source, marks);
    marks.sort_by_key(|m| (m.range.start, std::cmp::Reverse(m.severity)));
    {
        let mut state = view.lsp().borrow_mut();
        state.marks = marks;
        state.line_marks = None;
    }
    paint(view);
    view.redraw_gutter();
    view.diagnostics_changed();
    super::status::refresh(view);
}

pub(crate) fn set_tokens(view: &dyn Editor, tokens: &SemanticTokens) {
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
    view.lsp().borrow_mut().tokens = painted;
    paint(view);
    view.redraw_minimap();
}

/// The marks of `old` from other servers than `source` with `new`, the marks `source` sends now (E140).
fn replaced(old: Vec<Mark>, source: u64, mut new: Vec<Mark>) -> Vec<Mark> {
    new.extend(old.into_iter().filter(|m| m.source != source));
    new
}

/// The diagnostics of at least `Info` by the line they start on: the start of the line's most serious one and every
/// message of the line with its severity, the most serious first.
pub fn line_messages(view: &dyn Editor) -> BTreeMap<usize, (usize, Vec<(Severity, String)>)> {
    let state = view.lsp().borrow();
    let document = view.document();
    let text = document.text();
    let mut lines: BTreeMap<usize, (usize, Vec<(Severity, String)>)> = BTreeMap::new();
    for mark in state.marks.iter().filter(|m| m.severity >= Severity::Info) {
        let Ok(byte) = text.utf16_to_byte(mark.range.start.min(text.len_utf16())) else {
            continue;
        };
        let entry = lines
            .entry(text.line_of(byte))
            .or_insert_with(|| (mark.range.start, Vec::new()));
        if entry.1.first().is_none_or(|(top, _)| mark.severity > *top) {
            entry.0 = mark.range.start;
        }
        entry.1.push((mark.severity, mark.message.clone()));
    }
    for (_, messages) in lines.values_mut() {
        messages.sort_by_key(|(severity, _)| std::cmp::Reverse(*severity));
    }
    lines
}

/// The diagnostics of at least `Info` by the line they start on, with the most serious one's severity and every
/// message.
pub(crate) type LineMarks = BTreeMap<usize, (Severity, String)>;

/// The diagnostics by the line they start on, the most serious first, for the line number column and the minimap;
/// found once per revision of the document and set of diagnostics.
pub fn line_marks(view: &dyn Editor) -> Rc<LineMarks> {
    let revision = view.document().revision();
    if let Some((seen, lines)) = &view.lsp().borrow().line_marks
        && *seen == revision
    {
        return lines.clone();
    }
    let lines = Rc::new(find_line_marks(view));
    view.lsp().borrow_mut().line_marks = Some((revision, lines.clone()));
    lines
}

fn find_line_marks(view: &dyn Editor) -> LineMarks {
    let state = view.lsp().borrow();
    let document = view.document();
    let text = document.text();
    let mut lines = LineMarks::new();
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
pub fn paint(view: &dyn Editor) {
    let visible = view.visible_utf16();
    view.lsp().borrow_mut().repaint = Repaint::default();
    paint_range(view, &visible);
    finish_paint(view);
}

/// Paints what edits or scrolling left stale since the last painting: the lines on screen after scrolling, only the
/// edited text on screen after typing.
pub fn paint_due(view: &dyn Editor) {
    let due = std::mem::take(&mut view.lsp().borrow_mut().repaint);
    let visible = view.visible_utf16();
    if due.screen {
        paint_range(view, &visible);
    } else if let Some(edited) = due.edited {
        let range = edited.start.max(visible.start)..edited.end.min(visible.end);
        if !range.is_empty() {
            paint_range(view, &range);
        }
    }
    finish_paint(view);
}

/// Sets the colours of `range` from the editor's own colours and the view's tokens over them.
fn paint_range(view: &dyn Editor, range: &Range<usize>) {
    view.paint_under_server(range);
    paint_tokens(view, range);
}

/// Paints the view's tokens over `range` again, after the editor painted its own colours there.
pub fn paint_tokens(view: &dyn Editor, range: &Range<usize>) {
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return;
    };
    let Ok(state) = view.lsp().try_borrow() else {
        return;
    };
    let foreground = constant!(NSForegroundColorAttributeName);
    for painted in overlapping(&state.tokens, range) {
        if let Some(range) = text_range(&content, &painted.range) {
            let color: Retained<AnyObject> = crate::host::palette_color(painted.token).into();
            // SAFETY: The value of the foreground colour attribute is a colour.
            unsafe { layout.addRenderingAttribute_value_forTextRange(foreground, Some(&color), &range) };
        }
    }
}

fn finish_paint(view: &dyn Editor) {
    view.paint_found();
    if let Some(overlay) = view.lsp().borrow().squiggles.as_ref().and_then(Weak::load) {
        overlay.setNeedsDisplay(true);
    }
}

/// Moves the diagnostics' overlay of `view` over the part on screen.
pub fn follow_squiggles(view: &dyn Editor) {
    let Some(overlay) = view.lsp().borrow().squiggles.as_ref().and_then(Weak::load) else {
        return;
    };
    let seen = view.visibleRect();
    if overlay.frame() != seen {
        overlay.setFrame(seen);
        overlay.setNeedsDisplay(true);
    }
}

pub(crate) struct SquiggleIvars {
    /// The editor it covers, by its serial.
    editor: crate::EditorSerial,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Squiggles` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESquiggles"]
    #[ivars = SquiggleIvars]
    pub(crate) struct Squiggles;

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
            ide_ui_common::util::guarded((), || self.draw(dirty));
        }
    }
);

impl Squiggles {
    /// The overlay of `editor`, covering the part on screen, which keeps its layer small in a long text.
    pub(crate) fn attach(editor: &dyn Editor) {
        let this = Squiggles::alloc(crate::editor::marker(editor)).set_ivars(SquiggleIvars {
            editor: editor.serial(),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Squiggles> = unsafe { msg_send![super(this), initWithFrame: editor.visibleRect()] };
        this.setAccessibilityElement(false);
        editor.addSubview_positioned_relativeTo(&this, NSWindowOrderingMode::Above, None);
        editor.lsp().borrow_mut().squiggles = Some(Weak::from(&*this));
    }

    fn draw(&self, dirty: NSRect) {
        let Some(editor) = crate::host::find_editor(self.ivars().editor) else {
            return;
        };
        let (Some(layout), Some(content)) = (editor.textLayoutManager(), editor.textContentStorage()) else {
            return;
        };
        // What follows draws in the editor's coordinates, of which the overlay shows the part on screen.
        let shift = self.frame().origin;
        let transform = objc2_foundation::NSAffineTransform::transform();
        transform.translateXBy_yBy(-shift.x, -shift.y);
        transform.concat();
        let dirty = NSRect::new(
            NSPoint::new(dirty.origin.x + shift.x, dirty.origin.y + shift.y),
            dirty.size,
        );
        let origin = editor.textContainerOrigin();
        let visible = editor.visible_utf16();
        let length = editor.document().text().len_utf16();
        let state = editor.lsp().borrow();
        let reaching = state.marks.partition_point(|m| m.range.start <= visible.end);
        let marks: Vec<(Range<usize>, Severity)> = state.marks[..reaching]
            .iter()
            .filter(|m| m.severity >= Severity::Info)
            .map(|m| (shown_range(&m.range, length), m.severity))
            .filter(|(range, _)| range.end > visible.start && range.start < visible.end)
            .filter(|(range, _)| !editor.hidden_by_fold(range.start))
            .collect();
        drop(state);
        for (range, severity) in marks {
            let Some(text_range) = text_range(&content, &range) else {
                continue;
            };
            severity.color().setStroke();
            let block = RcBlock::new(
                |_: *mut NSTextRange, frame: NSRect, _: f64, _: NonNull<NSTextContainer>| -> Bool {
                    ide_ui_common::util::guarded(Bool::NO, || {
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
                    })
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

    fn mark(source: u64, message: &str) -> Mark {
        Mark {
            range: 0..1,
            severity: Severity::Error,
            message: message.into(),
            diagnostic: Diagnostic::default(),
            source,
        }
    }

    #[test]
    fn each_server_replaces_only_its_own_diagnostics() {
        let both = replaced(vec![mark(1, "main old")], 2, vec![mark(2, "eslint")]);
        let after = replaced(both, 1, vec![mark(1, "main new")]);
        let mut messages: Vec<_> = after.iter().map(|m| m.message.as_str()).collect();
        messages.sort_unstable();
        assert_eq!(messages, ["eslint", "main new"]);
        let cleared = replaced(after, 2, Vec::new());
        assert_eq!(cleared.len(), 1);
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

    fn painted(range: Range<usize>) -> Painted {
        Painted {
            range,
            token: Token::Url,
        }
    }

    #[test]
    fn the_tokens_of_a_stretch_are_found_by_place() {
        let tokens: Vec<Painted> = [0..2, 4..6, 8..12, 14..15].into_iter().map(painted).collect();
        let found = |range: Range<usize>| -> Vec<Range<usize>> {
            overlapping(&tokens, &range).iter().map(|p| p.range.clone()).collect()
        };
        assert_eq!(found(5..9), [4..6, 8..12]);
        assert_eq!(found(6..8), Vec::<Range<usize>>::new());
        assert_eq!(found(10..20), [8..12, 14..15]);
        assert_eq!(found(0..1), [painted(0..2).range]);
        assert_eq!(found(15..30), Vec::<Range<usize>>::new());
    }

    #[test]
    fn an_edit_marks_its_text_and_the_tokens_it_took_for_painting_again() {
        let mut state = ViewState {
            tokens: [0..2, 4..6, 8..10].into_iter().map(painted).collect(),
            ..ViewState::default()
        };
        shift(&mut state, &change("0123456789", 5..5, "ab"));
        let kept: Vec<Range<usize>> = state.tokens.iter().map(|p| p.range.clone()).collect();
        assert_eq!(kept, [0..2, 10..12]);
        assert_eq!(
            state.repaint.edited,
            Some(4..8),
            "the inserted text and the token it split"
        );
        shift(&mut state, &change("01234ab56789", 0..1, ""));
        assert_eq!(
            state.repaint.edited,
            Some(0..7),
            "the earlier stretch moves with the text"
        );
    }

    #[test]
    fn shifting_by_place_moves_marks_and_tokens_as_following_each_one_does() {
        let text = "0123456789012345678901234567890123456789";
        let tokens = [0..2, 3..3, 4..6, 8..12, 14..15, 20..26, 30..31];
        let marks = [0..4, 2..3, 5..5, 5..9, 10..30, 12..13, 20..20, 33..39];
        for start in 0..text.len() {
            for end in start..(start + 6).min(text.len()) {
                for new in ["", "x", "abc"] {
                    let edit = change(text, start..end, new);
                    let mut state = ViewState {
                        tokens: tokens.iter().cloned().map(painted).collect(),
                        marks: marks
                            .iter()
                            .map(|range| Mark {
                                range: range.clone(),
                                ..mark(1, "")
                            })
                            .collect(),
                        ..ViewState::default()
                    };
                    shift(&mut state, &edit);
                    let mut expected_tokens = tokens.to_vec();
                    expected_tokens.retain_mut(|range| follow(range, &edit));
                    let mut expected_marks = marks.to_vec();
                    for range in &mut expected_marks {
                        follow(range, &edit);
                    }
                    let got_tokens: Vec<_> = state.tokens.iter().map(|p| p.range.clone()).collect();
                    let got_marks: Vec<_> = state.marks.iter().map(|m| m.range.clone()).collect();
                    assert_eq!(got_tokens, expected_tokens, "tokens after {start}..{end} → {new:?}");
                    assert_eq!(got_marks, expected_marks, "marks after {start}..{end} → {new:?}");
                }
            }
        }
    }

    #[test]
    fn token_types_map_to_the_palette_where_the_lexer_cannot_tell() {
        assert_eq!(token_of("struct"), Some(Token::Url));
        assert_eq!(token_of("function"), Some(Token::Math));
        assert_eq!(token_of("variable"), None);
        assert_eq!(token_of("keyword"), None);
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
