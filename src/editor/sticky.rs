//! Sticky Scroll (E108): the header lines of the blocks the top visible line is in, held at the top of the editor over
//! its line numbers and text, at most `stickyScrollLines` of them. The headers come from the language server's
//! document symbols, else from the folds by indentation; Markdown and plain text have none.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use ide_core::settings::{Settings, keys};
use objc2::rc::{Retained, Weak};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSEvent, NSForegroundColorAttributeName, NSGraphicsContext, NSLayoutConstraint,
    NSMenuItem, NSScrollView, NSView, NSViewBoundsDidChangeNotification,
};
use objc2_core_foundation::{CFAttributedString, CFString, CGAffineTransform};
use objc2_core_graphics::CGContext;
use objc2_core_text::{CTLine, kCTForegroundColorAttributeName};
use objc2_foundation::{NSMutableAttributedString, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString};

use super::view::EditorTextView;
use crate::util::{constant, guarded};

/// The headers' number when the menu item turns Sticky Scroll on.
const DEFAULT_LINES: usize = 5;
const MOST_LINES: usize = 10;
/// NSTextContainer's default `lineFragmentPadding`, which the editor keeps.
const LINE_PADDING: f64 = 5.0;

/// How many header lines editors hold, as the user last chose; read from the settings once.
#[derive(Default)]
pub struct Lines(Cell<Option<usize>>);

impl Lines {
    /// Reads the setting again on next use.
    pub fn forget(&self) {
        self.0.set(None);
    }
}

fn context() -> Option<std::rc::Rc<crate::context::AppContext>> {
    MainThreadMarker::new().map(crate::context::AppContext::get)
}

/// How many header lines editors hold at most, 0 while Sticky Scroll is off.
pub fn lines() -> usize {
    let Some(cx) = context() else { return 0 };
    let cell = &cx.editor().sticky.0;
    cell.get().unwrap_or_else(|| {
        let value = Settings::load_user().get(&keys::STICKY_SCROLL_LINES).min(MOST_LINES);
        cell.set(Some(value));
        value
    })
}

/// Turns Sticky Scroll off, or on with five lines, in every editor and remembers the choice.
pub fn toggle() {
    let value = if lines() > 0 { 0 } else { DEFAULT_LINES };
    if let Some(cx) = context() {
        cx.editor().sticky.0.set(Some(value));
    }
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::STICKY_SCROLL_LINES, &value) {
        crate::diag::report("saving the sticky scroll setting", error);
    }
    for view in super::open_editors() {
        refresh(&view);
    }
}

/// Whether `item` is the menu item of Sticky Scroll, which then gets its check mark.
pub fn validate(item: &NSMenuItem) -> Option<bool> {
    if item.action() != Some(objc2::sel!(ideToggleStickyScroll:)) {
        return None;
    }
    item.setState(if lines() > 0 {
        objc2_app_kit::NSControlStateValueOn
    } else {
        objc2_app_kit::NSControlStateValueOff
    });
    Some(true)
}

fn of(view: &EditorTextView) -> Option<Retained<Sticky>> {
    view.ivars().sticky.borrow().as_ref().and_then(Weak::load)
}

/// Finds the blocks and headers of `view` again and draws them.
pub fn refresh(view: &EditorTextView) {
    if let Some(sticky) = of(view) {
        sticky.update(true);
    }
}

/// The height the headers of `view` take over its text.
pub fn height(view: &EditorTextView) -> f64 {
    of(view).map_or(0.0, |sticky| sticky.frame().size.height)
}

/// The blocks of `view` as their header line and last line, ordered by header line: the server's symbols, else the
/// folds by indentation.
fn scopes(view: &EditorTextView) -> super::fold::Lines {
    if let Some(mut symbols) = ide_ui_lsp::symbol_scopes(view) {
        symbols.sort_unstable();
        return symbols.into();
    }
    let language = view.document().language();
    match language {
        Some(language) if !language.is_markdown() => super::fold::indent_ranges(view),
        _ => Rc::new([]),
    }
}

/// The header lines over `top`, the first line under them: the blocks that start above it and end on or after it,
/// outermost first, at most `most`, one per line; `scopes` are ordered by header line.
pub fn headers_over(scopes: &[(usize, usize)], top: usize, most: usize) -> Vec<usize> {
    let above = scopes.partition_point(|(head, _)| *head < top);
    let mut lines: Vec<usize> = Vec::new();
    for (head, _) in scopes[..above].iter().filter(|(_, end)| *end >= top) {
        if lines.len() == most {
            break;
        }
        if lines.last() != Some(head) {
            lines.push(*head);
        }
    }
    lines
}

pub struct StickyIvars {
    editor: Weak<EditorTextView>,
    scroll: Weak<NSScrollView>,
    height: RefCell<Option<Retained<NSLayoutConstraint>>>,
    /// The header lines shown, outermost first.
    shown: RefCell<Vec<usize>>,
    /// The height of one line, as the editor laid out its first line.
    row: Cell<f64>,
    scrolled: OnceCell<crate::util::Observation>,
    /// The blocks found last, with the document's revision then, which scrolling uses again.
    scopes: RefCell<Option<(u64, super::fold::Lines)>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Sticky` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEStickyScroll"]
    #[ivars = StickyIvars]
    pub struct Sticky;

    unsafe impl NSObjectProtocol for Sticky {}

    impl Sticky {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: NSRect) {
            guarded((), || self.draw(rect));
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || self.clicked(event));
        }
    }
);

impl Sticky {
    /// The headers of `editor`, over `scroll` and the columns before it, updated as it scrolls.
    pub fn new(mtm: MainThreadMarker, scroll: &NSScrollView, editor: &EditorTextView) -> Retained<Sticky> {
        let this = Self::alloc(mtm).set_ivars(StickyIvars {
            editor: Weak::from(editor),
            scroll: Weak::from(scroll),
            height: RefCell::new(None),
            shown: RefCell::new(Vec::new()),
            row: Cell::new(0.0),
            scrolled: OnceCell::new(),
            scopes: RefCell::new(None),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.setClipsToBounds(true);
        // VoiceOver reads the same lines in the text.
        this.setAccessibilityElement(false);
        let clip = scroll.contentView();
        clip.setPostsBoundsChangedNotifications(true);
        let sticky = Weak::from(&*this);
        let watch = crate::util::observe(
            mtm,
            constant!(NSViewBoundsDidChangeNotification),
            Some(&clip),
            move |_| {
                if let Some(sticky) = sticky.load() {
                    sticky.update(false);
                }
            },
        );
        let _ = this.ivars().scrolled.set(watch);
        *editor.ivars().sticky.borrow_mut() = Some(Weak::from(&*this));
        this
    }

    /// Holds the headers at `constraint`'s height, zero while there are none.
    pub fn set_height_constraint(&self, constraint: Retained<NSLayoutConstraint>) {
        *self.ivars().height.borrow_mut() = Some(constraint);
    }

    /// The blocks of `editor`, found again when `fresh` or after an edit, else those found last.
    fn scopes(&self, editor: &EditorTextView, fresh: bool) -> super::fold::Lines {
        let revision = editor.document().revision();
        if !fresh
            && let Some((seen, scopes)) = self.ivars().scopes.borrow().as_ref()
            && *seen == revision
        {
            return scopes.clone();
        }
        let scopes = scopes(editor);
        *self.ivars().scopes.borrow_mut() = Some((revision, scopes.clone()));
        scopes
    }

    /// Finds the headers for what the editor shows now, sizes the view to them and draws them; the blocks are found
    /// again when `fresh`.
    fn update(&self, fresh: bool) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let most = lines();
        let row = row_height(&editor);
        self.ivars().row.set(row);
        let shown = if most == 0 || row <= 0.0 {
            Vec::new()
        } else {
            let scopes = self.scopes(&editor, fresh);
            let top = editor.visibleRect().origin.y;
            // The headers cover lines themselves, so the line under them decides, found again as they grow.
            let mut headers = Vec::new();
            for _ in 0..=most {
                let under = top + headers.len() as f64 * row;
                let line = super::highlight::line_at(&editor, NSPoint::new(0.0, under)).unwrap_or(0);
                let next = headers_over(&scopes, line, most);
                if next.len() == headers.len() {
                    headers = next;
                    break;
                }
                headers = next;
            }
            headers
        };
        let height = shown.len() as f64 * row;
        if let Some(constraint) = self.ivars().height.borrow().as_ref()
            && (constraint.constant() - height).abs() > f64::EPSILON
        {
            constraint.setConstant(height);
        }
        *self.ivars().shown.borrow_mut() = shown;
        self.setNeedsDisplay(true);
    }

    fn draw(&self, rect: NSRect) {
        NSColor::textBackgroundColor().setFill();
        objc2_app_kit::NSRectFill(rect);
        let (Some(editor), Some(scroll)) = (self.ivars().editor.load(), self.ivars().scroll.load()) else {
            return;
        };
        let row = self.ivars().row.get();
        let size = self.bounds().size;
        let shown = self.ivars().shown.borrow().clone();
        let text_x = scroll.frame().origin.x + editor.textContainerOrigin().x - scroll.contentView().bounds().origin.x
            + LINE_PADDING;
        let number_right = scroll.frame().origin.x - 8.0;
        let Some(context) = NSGraphicsContext::currentContext() else {
            return;
        };
        let ctx = context.CGContext();
        for (index, line) in shown.iter().enumerate() {
            let top = index as f64 * row;
            if let Some(header) = header_line(&editor, *line) {
                draw_line(&ctx, &header, text_x, top + row * 0.78);
            }
            let number = line_of_plain(&format!("{}", line + 1), &editor, &NSColor::secondaryLabelColor());
            let width = line_width(&number);
            draw_line(&ctx, &number, number_right - width, top + row * 0.78);
        }
        NSColor::separatorColor().setFill();
        objc2_app_kit::NSRectFill(NSRect::new(
            NSPoint::new(0.0, size.height - 1.0),
            NSSize::new(size.width, 1.0),
        ));
    }

    /// A click on a header scrolls the editor to its line, which then stands right under the headers left.
    fn clicked(&self, event: &NSEvent) {
        let y = self.convertPoint_fromView(event.locationInWindow(), None).y;
        self.click_row(y);
    }

    fn click_row(&self, y: f64) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let row = self.ivars().row.get();
        if row <= 0.0 {
            return;
        }
        let index = (y / row).floor();
        let shown = self.ivars().shown.borrow().clone();
        let Some(&line) = shown
            .iter()
            .enumerate()
            .find(|(i, _)| *i as f64 == index)
            .map(|(_, l)| l)
        else {
            return;
        };
        if let Some(start) = editor.line_start(line) {
            editor.setSelectedRange(NSRange::new(start, 0));
            scroll_line_to_top(&editor, line, index * row);
        }
    }
}

/// The height of one line of `editor`, read from the line at the top of what it shows, which TextKit has laid out;
/// a line elsewhere may have only its estimate, which a later edit trips over.
fn row_height(editor: &EditorTextView) -> f64 {
    let Some(layout) = editor.textLayoutManager() else {
        return 0.0;
    };
    let top = NSPoint::new(0.0, editor.visibleRect().origin.y - editor.textContainerOrigin().y);
    layout
        .textLayoutFragmentForPosition(top)
        .and_then(|fragment| super::insets::line_fragments(&fragment))
        .and_then(|lines| lines.firstObject())
        .map_or(0.0, |line| line.typographicBounds().size.height)
}

/// Scrolls `editor` so that `line` starts `above` points under the top of what it shows.
fn scroll_line_to_top(editor: &EditorTextView, line: usize, above: f64) {
    let Some(rect) = super::laid_out_line_rect(editor, line) else {
        return;
    };
    let Some(scroll) = editor.enclosingScrollView() else {
        return;
    };
    let clip = scroll.contentView();
    let most = (editor.frame().size.height - clip.bounds().size.height).max(0.0);
    let y = (rect.origin.y - above).clamp(0.0, most);
    clip.scrollToPoint(NSPoint::new(clip.bounds().origin.x, y));
    scroll.reflectScrolledClipView(&clip);
}

/// Moves `editor` so that `range`, already scrolled into view, does not end up under the headers.
pub fn keep_clear(editor: &EditorTextView, range: NSRange) {
    let covered = height(editor);
    if covered <= 0.0 {
        return;
    }
    let Some(line) = editor_line_of(editor, range.location) else {
        return;
    };
    let Some(rect) = super::laid_out_line_rect(editor, line) else {
        return;
    };
    let visible = editor.visibleRect();
    if rect.origin.y < visible.origin.y + covered && rect.origin.y >= visible.origin.y - rect.size.height {
        scroll_line_to_top(editor, line, covered);
    }
}

fn editor_line_of(editor: &EditorTextView, utf16: usize) -> Option<usize> {
    let document = editor.document();
    let text = document.text();
    let byte = text.utf16_to_byte(utf16.min(text.len_utf16())).ok()?;
    Some(text.line_of(byte))
}

/// Line `line` of `editor` with its font and syntax colours, as a Core Text line.
fn header_line(editor: &EditorTextView, line: usize) -> Option<objc2_core_foundation::CFRetained<CTLine>> {
    let start = editor.line_start(line)?;
    let end = editor
        .line_start(line + 1)
        .map_or_else(|| editor.document().text().len_utf16(), |next| next.saturating_sub(1));
    let storage = editor.storage()?;
    let range = NSRange::new(
        start,
        end.saturating_sub(start).min(storage.length().saturating_sub(start)),
    );
    let piece = storage.attributedSubstringFromRange(range);
    let copy = NSMutableAttributedString::initWithAttributedString(NSMutableAttributedString::alloc(), &piece);
    with_core_text_colors(&copy);
    Some(core_text_line(&copy))
}

/// `text` in the editor's font and `color`, as a Core Text line.
fn line_of_plain(text: &str, editor: &EditorTextView, color: &NSColor) -> objc2_core_foundation::CFRetained<CTLine> {
    let string = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &crate::util::ns(text));
    let whole = NSRange::new(0, string.length());
    let font = super::code_font(editor.ivars().font_size.get() * 0.85);
    // SAFETY: AppKit's font and colour keys with a font and a colour.
    unsafe {
        string.addAttribute_value_range(constant!(objc2_app_kit::NSFontAttributeName), &font, whole);
        string.addAttribute_value_range(constant!(NSForegroundColorAttributeName), color, whole);
    }
    with_core_text_colors(&string);
    core_text_line(&string)
}

/// Gives every run of `string` its AppKit colour as Core Text's, which ignores AppKit's key.
fn with_core_text_colors(string: &NSMutableAttributedString) {
    let length = string.length();
    // SAFETY: CFString and NSString are toll-free bridged.
    let key = unsafe { &*std::ptr::from_ref::<CFString>(kCTForegroundColorAttributeName).cast::<NSString>() };
    let mut at = 0;
    while at < length {
        let mut run = NSRange::new(at, 0);
        // SAFETY: `at` lies inside the string and `run` is a valid out-pointer.
        let color =
            unsafe { string.attribute_atIndex_effectiveRange(constant!(NSForegroundColorAttributeName), at, &mut run) };
        let color = color
            .and_then(|c| c.downcast::<NSColor>().ok())
            .unwrap_or_else(NSColor::textColor);
        let cg = color.CGColor();
        // SAFETY: Core Text's colour key takes a CGColor, which is an object.
        unsafe {
            string.addAttribute_value_range(key, &*std::ptr::from_ref(&*cg).cast::<objc2::runtime::AnyObject>(), run)
        };
        at = (run.location + run.length).max(at + 1);
    }
}

fn core_text_line(string: &NSMutableAttributedString) -> objc2_core_foundation::CFRetained<CTLine> {
    // SAFETY: NSAttributedString and CFAttributedString are toll-free bridged.
    let attributed = unsafe { &*std::ptr::from_ref(string).cast::<CFAttributedString>() };
    // SAFETY: The attributed string is valid for the call.
    unsafe { CTLine::with_attributed_string(attributed) }
}

fn line_width(line: &CTLine) -> f64 {
    // SAFETY: Null pointers skip the metrics not asked for.
    unsafe { line.typographic_bounds(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) }
}

/// Draws `line` with its baseline at `baseline` in the flipped view's coordinates.
fn draw_line(ctx: &CGContext, line: &CTLine, x: f64, baseline: f64) {
    let c = Some(ctx);
    CGContext::save_g_state(c);
    CGContext::translate_ctm(c, x, baseline);
    CGContext::scale_ctm(c, 1.0, -1.0);
    CGContext::set_text_matrix(
        c,
        CGAffineTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        },
    );
    // SAFETY: The context is the one being drawn into.
    unsafe { line.draw(ctx) };
    CGContext::restore_g_state(c);
}

#[cfg(test)]
mod tests {
    use super::headers_over;

    #[test]
    fn the_headers_are_the_blocks_around_the_line_under_them_outermost_first() {
        let scopes = [(0, 20), (2, 10), (4, 8), (12, 18)];
        assert_eq!(headers_over(&scopes, 5, 5), vec![0, 2, 4]);
        assert_eq!(headers_over(&scopes, 5, 2), vec![0, 2]);
        assert_eq!(headers_over(&scopes, 9, 5), vec![0, 2]);
        assert_eq!(headers_over(&scopes, 0, 5), Vec::<usize>::new());
        assert_eq!(headers_over(&[(3, 9), (3, 6)], 4, 5), vec![3]);
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
