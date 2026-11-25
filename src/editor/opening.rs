//! Opening a file in an editor tab, and setting up the view: its font, its wrapping and what it follows.

use std::path::Path;

use ide_document::Document;
use ide_document::recovery::{Recovered, RecoveryId};
use ide_process::signpost;
use ide_ui_common::alert::warn;
use objc2::rc::{Retained, Weak};
use objc2::{AllocAnyThread, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSFontAttributeName, NSForegroundColorAttributeName, NSScrollView,
    NSUserInterfaceLayoutDirection, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use super::{
    EditorTextView, changes, container, display_name, highlight, indent, insets, overlay, reading, style, tab_title,
    update_dirty, vim, whitespace,
};
use crate::util::{FlagGuard, constant, ns};
use crate::window::{TabId, TabKind, WindowController};

/// Opens `path` in a new editor tab of `controller`, or in its preview tab; shows an alert when it cannot be read.
pub(crate) fn open_tab(controller: &WindowController, path: &Path, font_size: f64, preview: bool) {
    reading::open(controller, path, font_size, preview);
}

pub(super) fn opened(
    controller: &WindowController,
    path: &Path,
    document: std::io::Result<Document>,
    font_size: f64,
    preview: bool,
) {
    let _interval = signpost::begin(signpost::name!("open"));
    let mtm = controller.mtm();
    let document = match document {
        Ok(document) => document,
        Err(error) => {
            warn(
                mtm,
                &tf("„{}“ lässt sich nicht öffnen.", &[&display_name(path)]),
                &error.to_string(),
            );
            return;
        }
    };
    let key = path.to_string_lossy().into_owned();
    let view = add_tab(
        controller,
        document,
        Some(key),
        &tab_title(path),
        font_size,
        preview,
        RecoveryId::new(),
    );
    crate::markdown::opened(&view);
}

/// Opens the text a crash left unsaved in a new editor tab of `controller`, marked unsaved; its copy stays until the
/// text is saved or given up. A look at its file follows with `check_recovered`.
pub(crate) fn open_recovered(
    controller: &WindowController,
    recovered: Recovered,
    font_size: f64,
) -> Retained<EditorTextView> {
    let id = recovered.pending.id.clone();
    let path = recovered.pending.path.clone();
    let (key, title) = match &path {
        Some(path) => (Some(path.to_string_lossy().into_owned()), tab_title(path)),
        None => (None, t("Ohne Titel").to_string()),
    };
    let view = add_tab(controller, recovered.into_document(), key, &title, font_size, false, id);
    update_dirty(&view);
    view
}

/// Opens `path` in `controller`, or selects its tab, and marks `columns`, in UTF-16 code units, of line `line`,
/// counted from 1; `focus` selects them in the editor and gives it the keyboard focus.
pub(crate) fn reveal(
    controller: &WindowController,
    path: &Path,
    line: u64,
    columns: std::ops::Range<usize>,
    focus: bool,
) {
    controller.open_file(path);
    reading::with_opened(controller, path, move |view| {
        let line = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
        view.select_columns(line, columns, focus);
    });
}

/// Opens `path` in an editor tab of `controller`, or brings its tab forward, with the cursor on line `line` at column
/// `column`, both counted from 1 in characters as compilers print them; a column past the end means the line's end.
pub(crate) fn reveal_position(controller: &WindowController, path: &Path, line: usize, column: usize) {
    controller.open_file(path);
    reading::with_opened(controller, path, move |view| {
        place_cursor(view, line, column);
    });
}

pub(super) fn place_cursor(view: &EditorTextView, line: usize, column: usize) {
    let at = {
        let document = view.document();
        let text = document.text();
        let line = line.clamp(1, text.line_count().max(1)) - 1;
        let byte = text
            .offset(line, column.saturating_sub(1), ide_document::Unit::Utf32)
            .ok()
            .or_else(|| text.line_content(line).map(|content| content.end));
        byte.and_then(|byte| text.byte_to_utf16(byte).ok())
    };
    if let Some(at) = at {
        view.setSelectedRange(objc2_foundation::NSRange::new(at, 0));
        view.scrollRangeToVisible(objc2_foundation::NSRange::new(at, 0));
    }
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(view));
    }
}

/// Opens an empty, untitled editor tab.
pub(crate) fn new_untitled(controller: &WindowController, font_size: f64) {
    add_tab(
        controller,
        Document::untitled(""),
        None,
        t("Ohne Titel"),
        font_size,
        false,
        RecoveryId::new(),
    );
}

/// Adds an editor tab, which takes the focus unless it is a preview the user is only looking at.
pub(super) fn add_tab(
    controller: &WindowController,
    document: Document,
    key: Option<String>,
    title: &str,
    font_size: f64,
    preview: bool,
    recovery: RecoveryId,
) -> Retained<EditorTextView> {
    let (view, container) = build_editor(controller, document, font_size, recovery);
    let id = if preview {
        controller.add_preview_tab(TabKind::Editor, key, title, container)
    } else {
        controller.add_tab(TabKind::Editor, key, title, container)
    };
    finish_editor(controller, &view, id);
    if !preview && let Some(window) = view.window() {
        window.makeFirstResponder(Some(&view));
    }
    view
}

/// Fills the existing tab without changing its identity or its position in the pane.
pub(crate) fn attach_restored(
    controller: &WindowController,
    id: TabId,
    host: &NSView,
    document: Document,
    font_size: f64,
    recovery: RecoveryId,
) -> Retained<EditorTextView> {
    let (view, container) = build_editor(controller, document, font_size, recovery);
    host.addSubview(&container);
    crate::util::pin_edges(&container, host);
    finish_editor(controller, &view, id);
    view
}

pub(super) fn build_editor(
    controller: &WindowController,
    document: Document,
    font_size: f64,
    recovery: RecoveryId,
) -> (Retained<EditorTextView>, Retained<NSView>) {
    let mtm = controller.mtm();
    let view = EditorTextView::create(mtm, document, controller, recovery);
    configure(&view, font_size);
    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0)),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    scroll.setDocumentView(Some(&view));
    scroll.setHasHorizontalScroller(!wraps(&view));
    follow_width(&scroll, &view);
    let (container, gutter) = container::editor_container(mtm, &scroll, &view);
    ide_ui_lsp::attach(&*view);
    whitespace::attach(&view, &scroll);
    vim::attach(&view);
    overlay::attach(&view);
    insets::attach(&view);
    crate::debug::attach_rotor(&view, controller.context().debug_rotors());
    *view.ivars().around.gutter.borrow_mut() = Some(objc2::rc::Weak::from(&*gutter));
    (view, container)
}

pub(super) fn finish_editor(controller: &WindowController, view: &EditorTextView, id: TabId) {
    view.ivars().tab.set(Some(id));
    let editors = controller.context().editors();
    let serial = editors.next_id();
    editors.insert(serial, Weak::from(view));
    {
        let _interval = signpost::begin(signpost::name!("open recolor"));
        highlight::recolor_all(view);
    }
    // The server hears of the file, with the text as it is then, once the tab's first picture is on screen.
    let (controller, editor) = (Weak::from(controller), Weak::from(view));
    crate::util::after_frame(move || {
        let (Some(controller), Some(view)) = (controller.load(), editor.load()) else {
            return;
        };
        if controller.context().editors().get(serial).is_some() {
            let _interval = signpost::begin(signpost::name!("open lsp"));
            ide_ui_lsp::opened(&*view);
        }
    });
    changes::reload(view, true);
}

pub(super) fn configure(view: &EditorTextView, font_size: f64) {
    view.ivars().font_size.set(font_size);
    view.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    view.setRichText(false);
    view.setImportsGraphics(false);
    view.setAllowsUndo(true);
    view.setAutomaticQuoteSubstitutionEnabled(false);
    view.setAutomaticDashSubstitutionEnabled(false);
    view.setAutomaticTextReplacementEnabled(false);
    view.setAutomaticSpellingCorrectionEnabled(false);
    view.setContinuousSpellCheckingEnabled(false);
    view.setSmartInsertDeleteEnabled(false);
    view.setAutomaticLinkDetectionEnabled(false);
    view.setGrammarCheckingEnabled(false);
    view.setAutomaticDataDetectionEnabled(false);
    view.setAutomaticTextCompletionEnabled(false);
    view.setInlinePredictionType(objc2_app_kit::NSTextInputTraitType::No);
    view.setMathExpressionCompletionType(objc2_app_kit::NSTextInputTraitType::No);
    // With any checking type left on, NSTextCheckingController reads and annotates the text around every edit.
    view.setEnabledTextCheckingTypes(0);
    view.setUsesFindBar(true);
    view.setIncrementalSearchingEnabled(true);
    view.setVerticallyResizable(true);
    view.setHorizontallyResizable(true);
    view.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    view.setTextContainerInset(NSSize::new(6.0, 8.0));
    view.setBackgroundColor(&NSColor::textBackgroundColor());
    show_font(view, font_size);
    let (text, read_only, name) = {
        let document = view.document();
        let name = document.path().map(display_name).unwrap_or_default();
        (document.text().to_string(), document.read_only().is_some(), name)
    };
    {
        let _interval = signpost::begin(signpost::name!("open setString"));
        let _applying = FlagGuard::raise(&view.ivars().applying);
        let string = owned_ns(text).unwrap_or_else(|| ns(&view.document().text().to_string()));
        view.setString(&string);
    }
    indent::configure(view);
    // Markdown is prose, whose paragraphs are single long lines; VS Code wraps it by default as well.
    if view
        .document()
        .language()
        .is_some_and(ide_document::syntax::Language::is_markdown)
    {
        set_wrap(view, true);
    }
    view.setEditable(!read_only);
    view.setAccessibilityLabel(Some(&ns(&tf("Editor: {}", &[&name]))));
    view.follow_storage();
}

/// `text` as an `NSString` that takes over its bytes instead of copying them; `None` if Foundation refused them.
fn owned_ns(text: String) -> Option<Retained<NSString>> {
    if text.is_empty() {
        return Some(ns(""));
    }
    let bytes = std::ptr::NonNull::new(text.as_ptr().cast_mut().cast::<std::ffi::c_void>())?;
    let length = text.len();
    let owner = std::cell::Cell::new(Some(text));
    let release = block2::RcBlock::new(move |_: std::ptr::NonNull<std::ffi::c_void>, _: usize| {
        crate::util::guarded((), || drop(owner.take()));
    });
    // SAFETY: `bytes` points to `length` bytes of UTF-8 that `owner` keeps alive and unchanged until the string calls
    // `release`, which drops them.
    unsafe {
        NSString::initWithBytesNoCopy_length_encoding_deallocator(
            NSString::alloc(),
            bytes,
            length,
            objc2_foundation::NSUTF8StringEncoding,
            Some(&release),
        )
    }
}

/// Sets the code font of `font_size` on `view`, for its text and for what is typed.
pub(super) fn show_font(view: &EditorTextView, font_size: f64) {
    let font = style::code_font(font_size);
    view.setFont(Some(&font));
    let attributes = NSDictionary::<NSString, objc2::runtime::AnyObject>::from_retained_objects(
        &[
            constant!(NSFontAttributeName),
            constant!(NSForegroundColorAttributeName),
        ],
        &[font.into(), NSColor::textColor().into()],
    );
    // SAFETY: The dictionary holds a font and a colour under their keys, as typing attributes expect.
    unsafe { view.setTypingAttributes(&attributes) };
}

/// Shows the open `view` at `font_size` points, as a change in the settings asks: its text, the styles of its syntax
/// and Markdown, and the line numbers beside it.
pub(crate) fn set_font_size(view: &EditorTextView, font_size: f64) {
    if view.ivars().font_size.replace(font_size).to_bits() == font_size.to_bits() {
        return;
    }
    show_font(view, font_size);
    highlight::recolor_all(view);
    if let Some(gutter) = view.ivars().around.gutter.borrow().as_ref().and_then(Weak::load) {
        gutter.fit();
    }
    container::redraw_gutter(view);
}

pub(crate) fn toggle_wrap(view: &EditorTextView) {
    // SAFETY: A text view keeps its text container alive as long as it lives.
    let Some(container) = (unsafe { view.textContainer() }) else {
        return;
    };
    set_wrap(view, !container.widthTracksTextView());
}

/// Wraps long lines at the view's edge, or lets them run on to the right.
pub(super) fn set_wrap(view: &EditorTextView, wrap: bool) {
    // SAFETY: A text view keeps its text container alive as long as it lives.
    let Some(container) = (unsafe { view.textContainer() }) else {
        return;
    };
    container.setWidthTracksTextView(wrap);
    if !wrap {
        container.setContainerSize(NSSize::new(1.0e7, 1.0e7));
    }
    view.setHorizontallyResizable(!wrap);
    if let Some(scroll) = view.enclosingScrollView() {
        scroll.setHasHorizontalScroller(!wrap);
    }
    fit_width(view);
}

/// Keeps a wrapping view as wide as its scroll view shows, whenever that changes size.
pub(super) fn follow_width(scroll: &NSScrollView, view: &EditorTextView) {
    let clip = scroll.contentView();
    clip.setPostsFrameChangedNotifications(true);
    let name = constant!(objc2_app_kit::NSViewFrameDidChangeNotification);
    let editor = Weak::from(view);
    let watch = crate::util::observe(view.mtm(), name, Some(&clip), move |_| {
        if let Some(editor) = editor.load() {
            fit_width_soon(&editor);
        }
    });
    view.ivars().watches.borrow_mut().push(watch);
    fit_width(view);
}

/// Lets a wrapping view take its scroll view's width in its next layout pass, before TextKit lays out the lines on
/// screen there, so that it wraps once per frame however often a drag changed the size before (SN-S3).
fn fit_width_soon(view: &EditorTextView) {
    if !wraps(view) || view.ivars().width_due.replace(true) {
        return;
    }
    view.setNeedsLayout(true);
    // A view outside a window gets no layout pass.
    view.later(objc2::sel!(ideFitWidthDue), 0.0);
}

/// `fit_width` if `fit_width_soon` asked for it since.
pub(super) fn fit_width_due(view: &EditorTextView) {
    if view.ivars().width_due.replace(false) {
        fit_width(view);
    }
}

/// The window's live resize ended: the view takes its final width at once, and what was left for the end of the drag,
/// such as laying out the whole text for a jump (`place::lay_out_whole`), follows when asked for.
pub(super) fn resize_ended(view: &EditorTextView) {
    view.ivars().width_due.set(false);
    fit_width(view);
}

/// Gives a wrapping view the width its scroll view shows, at its left edge, so lines wrap where the view ends; a text
/// view only follows its own frame, which nothing else narrows.
pub(super) fn fit_width(view: &EditorTextView) {
    if !wraps(view) {
        return;
    }
    let Some(scroll) = view.enclosingScrollView() else {
        return;
    };
    let width = scroll.contentSize().width;
    let frame = view.frame();
    if (frame.size.width - width).abs() > 0.5 {
        view.setFrameSize(NSSize::new(width, frame.size.height));
    }
    let clip = scroll.contentView();
    let origin = clip.bounds().origin;
    if origin.x != 0.0 {
        clip.scrollToPoint(NSPoint::new(0.0, origin.y));
        scroll.reflectScrolledClipView(&clip);
    }
}

/// Whether long lines wrap at the view's edge.
pub(crate) fn wraps(view: &EditorTextView) -> bool {
    // SAFETY: A text view keeps its text container alive as long as it lives.
    unsafe { view.textContainer() }.is_some_and(|c| c.widthTracksTextView())
}
