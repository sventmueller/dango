//! The editor tab: a TextKit 2 text view over one document of the model in `ide-document`.

mod changes;
mod compare;
mod conflict;
mod context_menu;
mod editing;
mod fold;
mod highlight;
mod indent;
mod multi;
mod overlay;
pub mod recovery;
mod ruler;
mod style;
mod view;
mod whitespace;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use ide_document::recovery::{Recovered, RecoveryId};
use ide_document::{Change, Document, SaveError};
use objc2::rc::{Retained, Weak};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle, NSColor,
    NSFontAttributeName, NSForegroundColorAttributeName, NSModalResponseOK, NSSavePanel, NSScrollView,
    NSUserInterfaceLayoutDirection,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use crate::util::{constant, ns};
use crate::window::{TabKind, WindowController};
pub use view::EditorTextView;

thread_local! {
    static OPEN: RefCell<Vec<Weak<EditorTextView>>> = const { RefCell::new(Vec::new()) };
}

/// The editors still open.
fn open_editors() -> Vec<Retained<EditorTextView>> {
    OPEN.with(|open| {
        let mut open = open.borrow_mut();
        open.retain(|view| view.load().is_some());
        open.iter().filter_map(Weak::load).collect()
    })
}

/// The editors still open, in every window.
pub fn all_editors() -> Vec<Retained<EditorTextView>> {
    open_editors()
}

/// The open editor with `serial`, the number that other threads hold instead of the view.
pub fn find_editor(serial: u64) -> Option<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .find(|v| v.ivars().lsp.borrow().serial == serial)
}

/// The editor of `path` in `controller`'s window.
pub fn editor_for(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .find(|v| v.document().path() == Some(path) && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

/// The UTF-16 range of the lines on screen, with a margin.
pub fn visible_utf16(view: &EditorTextView) -> std::ops::Range<usize> {
    let lines = highlight::visible_lines(view);
    let length = view.document().text().len_utf16();
    let start = view.line_start(lines.start).unwrap_or(0);
    let end = view.line_start(lines.end).unwrap_or(length);
    start..end.max(start)
}

/// The byte offset where the first line on screen starts.
pub fn top_byte(view: &EditorTextView) -> Option<usize> {
    let line = highlight::line_at(view, NSPoint::new(0.0, view.visibleRect().origin.y))?;
    view.document().text().offset(line, 0, ide_document::Unit::Utf16).ok()
}

/// Scrolls `view` so that the line holding byte `byte` is the first on screen.
pub fn scroll_to_byte(view: &EditorTextView, byte: usize) {
    let Ok(at) = view.document().text().byte_to_utf16(byte) else {
        return;
    };
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return;
    };
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let Some(location) = content.locationFromLocation_withOffset(&start, isize::try_from(at).unwrap_or(isize::MAX))
    else {
        return;
    };
    layout.ensureLayoutForRange(&objc2_app_kit::NSTextRange::initWithLocation(
        objc2_app_kit::NSTextRange::alloc(),
        &location,
    ));
    let Some(fragment) = layout.textLayoutFragmentForLocation(&location) else {
        return;
    };
    let y = fragment.layoutFragmentFrame().origin.y + view.textContainerOrigin().y;
    view.scrollPoint(NSPoint::new(view.visibleRect().origin.x, y));
}

/// The tab that shows `view`.
pub fn tab_of(view: &EditorTextView) -> isize {
    view.ivars().tab.get()
}

/// Redraws the line number column of `view`, after its diagnostics changed.
pub fn redraw_gutter(view: &EditorTextView) {
    if let Some(gutter) = view.ivars().gutter.borrow().as_ref().and_then(Weak::load) {
        gutter.setNeedsDisplay(true);
    }
}

pub use fold::hidden_by_fold;
pub use style::{code_font, palette_color};

/// Takes the language server's folding ranges, as first line and last hidden line, or `None` for the indentation's.
pub fn set_server_folds(view: &EditorTextView, ranges: Option<Vec<(usize, usize)>>) {
    fold::set_server_ranges(view, ranges);
}

/// Runs `edit`, whose changes of `view` become one undoable step of their own, also right after typing in the same
/// turn of the run loop; inside a group a caller opened, they join it.
pub fn undo_step(view: &objc2_app_kit::NSTextView, edit: impl FnOnce()) {
    view.breakUndoCoalescing();
    let undo = view.undoManager();
    if let Some(undo) = &undo
        && undo.groupsByEvent()
        && undo.groupingLevel() == 1
    {
        // AppKit closes the turn's group only at its end and until then refuses registrations without an open group,
        // so a fresh one replaces it, which the end of the turn or the next undo closes.
        undo.endUndoGrouping();
        undo.beginUndoGrouping();
    }
    let own = undo
        .as_ref()
        .is_some_and(|u| u.groupsByEvent() && u.groupingLevel() == 0);
    if let Some(undo) = &undo {
        if own {
            undo.setGroupsByEvent(false);
        }
        undo.beginUndoGrouping();
    }
    edit();
    if let Some(undo) = &undo {
        undo.endUndoGrouping();
        if own {
            undo.setGroupsByEvent(true);
        }
    }
    view.breakUndoCoalescing();
}

/// The file of `view` got a new name, as a server's rename of a file did: the tab follows.
pub fn path_changed(view: &EditorTextView) {
    if let (Some(path), Some(controller)) = (view.document().path().map(Path::to_path_buf), view.controller()) {
        controller.set_tab_title(view.ivars().tab.get(), &tab_title(&path));
    }
    crate::lsp::opened(view);
    changes::reload(view, true);
}

/// Something changed in a `.git` folder below `root`: the change marks of its editors follow the commit.
pub fn git_changed(root: &Path) {
    changes::git_changed(root);
}

/// The texts of the editors with unsaved changes to files below `root`, which a project search reads in place of
/// the files.
pub fn unsaved_buffers(root: &Path) -> Vec<(PathBuf, String)> {
    open_editors()
        .iter()
        .filter_map(|view| {
            let document = view.document();
            let path = document.path().filter(|p| document.is_dirty() && p.starts_with(root))?;
            Some((path.to_path_buf(), document.text().to_string()))
        })
        .collect()
}

/// Adds the controls of the editor's whitespace rules, and of dictation, to its status bar's `stack`.
pub fn status_items(mtm: MainThreadMarker, view: &EditorTextView, stack: &objc2_app_kit::NSStackView) {
    indent::status_items(mtm, view, stack);
    #[cfg(feature = "dictation")]
    crate::dictation::editor_items(mtm, view, stack);
}

/// The tab size and whether `view` indents with spaces, for a language server's formatting.
pub fn formatting_options(view: &EditorTextView) -> (u32, bool) {
    indent::formatting(view)
}

/// The editor in tab `id` of `controller` is about to close.
pub fn closing(controller: &WindowController, id: isize) {
    let view = open_editors()
        .into_iter()
        .find(|v| v.ivars().tab.get() == id && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)));
    if let Some(view) = view {
        crate::lsp::closed(&view);
        crate::markdown::closing(&view);
        OPEN.with(|open| {
            open.borrow_mut()
                .retain(|w| w.load().is_some_and(|v| !std::ptr::eq(&*v, &*view)));
        });
    }
}

/// Shows a warning with `message` and `detail` and waits for OK.
pub fn show_alert(mtm: MainThreadMarker, message: &str, detail: &str) {
    alert(mtm, message, detail);
}

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The title of a file's tab: its name, and for a file in a thread's worktree (I1) which worktree, so the user sees
/// which copy they edit.
pub fn tab_title(path: &Path) -> String {
    let name = display_name(path);
    match crate::worktrees::tree_of(path) {
        Some(tree) => {
            let short = tree.branch.strip_prefix("thread/").unwrap_or(&tree.branch).to_string();
            tf("{} · Worktree {}", &[&name, &short])
        }
        None => name,
    }
}

/// Opens `path` in a new editor tab of `controller`, or in its preview tab; shows an alert when it cannot be read.
pub fn open_tab(controller: &WindowController, path: &Path, font_size: f64, preview: bool) {
    let mtm = controller.mtm();
    let document = match Document::open(path) {
        Ok(document) => document,
        Err(error) => {
            alert(
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

/// Opens `path` in a new editor tab of `controller` as a launch puts back the tabs of the last run: a file that cannot
/// be read is left out and only logged; whether it opened.
pub fn open_quietly(controller: &WindowController, path: &Path, font_size: f64) -> bool {
    match Document::open(path) {
        Ok(document) => {
            let key = path.to_string_lossy().into_owned();
            add_tab(
                controller,
                document,
                Some(key),
                &tab_title(path),
                font_size,
                false,
                RecoveryId::new(),
            );
            true
        }
        Err(error) => {
            crate::diag::report("reopening a file", error);
            false
        }
    }
}

/// Opens the text a crash left unsaved in a new editor tab of `controller`, marked unsaved; its copy stays until the
/// text is saved or given up. A look at its file follows with `check_recovered`.
pub fn open_recovered(controller: &WindowController, recovered: Recovered, font_size: f64) -> Retained<EditorTextView> {
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

/// Opens `path` in `controller`, or selects its tab, and selects `columns`, in UTF-16 code units, of line `line`,
/// counted from 1.
pub fn reveal(controller: &WindowController, path: &Path, line: u64, columns: std::ops::Range<usize>) {
    controller.open_file(path);
    let view = open_editors()
        .into_iter()
        .find(|v| v.document().path() == Some(path) && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)));
    if let Some(view) = view {
        let line = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
        view.select_columns(line, columns);
    }
}

/// Opens `path` in an editor tab of `controller`, or brings its tab forward, with the cursor on line `line` at column
/// `column`, both counted from 1 in characters as compilers print them; a column past the end means the line's end.
pub fn reveal_position(controller: &WindowController, path: &Path, line: usize, column: usize) {
    controller.open_file(path);
    let Some(view) = open_editors()
        .into_iter()
        .find(|v| v.document().path() == Some(path) && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
    else {
        return;
    };
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
        window.makeFirstResponder(Some(&view));
    }
}

/// Opens an empty, untitled editor tab.
pub fn new_untitled(controller: &WindowController, font_size: f64) {
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
fn add_tab(
    controller: &WindowController,
    document: Document,
    key: Option<String>,
    title: &str,
    font_size: f64,
    preview: bool,
    recovery: RecoveryId,
) -> Retained<EditorTextView> {
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
    let (container, gutter) = editor_container(mtm, &scroll, &view);
    crate::lsp::follow_scrolling(&scroll, &view);
    whitespace::attach(&view, &scroll);
    overlay::attach(&view);
    *view.ivars().gutter.borrow_mut() = Some(objc2::rc::Weak::from(&*gutter));
    let id = if preview {
        controller.add_preview_tab(TabKind::Editor, key, title, container)
    } else {
        controller.add_tab(TabKind::Editor, key, title, container)
    };
    view.ivars().tab.set(id);
    OPEN.with(|open| open.borrow_mut().push(Weak::from(&*view)));
    highlight::recolor_all(&view);
    crate::lsp::opened(&view);
    changes::reload(&view, true);
    if !preview && let Some(window) = view.window() {
        window.makeFirstResponder(Some(&view));
    }
    view
}

/// The scroll view with the line numbers beside it and the status bar of the language server below.
fn editor_container(
    mtm: MainThreadMarker,
    scroll: &objc2_app_kit::NSScrollView,
    view: &EditorTextView,
) -> (Retained<objc2_app_kit::NSView>, Retained<ruler::LineNumbers>) {
    let gutter = ruler::LineNumbers::new(mtm, scroll, view);
    let status = crate::lsp::status::bar(mtm, view);
    let bar = conflict::Bar::new(mtm);
    let container = objc2_app_kit::NSView::new(mtm);
    container.addSubview(&gutter);
    container.addSubview(scroll);
    container.addSubview(&status);
    container.addSubview(bar.view());
    bar.view().setTranslatesAutoresizingMaskIntoConstraints(false);
    gutter.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
    status.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        bar.view().topAnchor().constraintEqualToAnchor(&container.topAnchor()),
        bar.view()
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor()),
        bar.view()
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        // Code and its line numbers stay left to right in a right-to-left interface (E3); the bars around follow it.
        gutter.leftAnchor().constraintEqualToAnchor(&container.leftAnchor()),
        gutter.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        gutter.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        gutter.widthAnchor().constraintEqualToConstant(ruler::WIDTH),
        scroll.leftAnchor().constraintEqualToAnchor(&gutter.rightAnchor()),
        scroll.rightAnchor().constraintEqualToAnchor(&container.rightAnchor()),
        scroll.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        scroll.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        status
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor()),
        status
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        status.bottomAnchor().constraintEqualToAnchor(&container.bottomAnchor()),
        status
            .heightAnchor()
            .constraintEqualToConstant(crate::lsp::status::HEIGHT),
    ] {
        constraint.setActive(true);
    }
    *view.ivars().bar.borrow_mut() = Some(bar);
    (container, gutter)
}

fn configure(view: &EditorTextView, font_size: f64) {
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
    view.setUsesFindBar(true);
    view.setIncrementalSearchingEnabled(true);
    view.setVerticallyResizable(true);
    view.setHorizontallyResizable(true);
    view.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    view.setTextContainerInset(NSSize::new(6.0, 8.0));
    view.setBackgroundColor(&NSColor::textBackgroundColor());
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
    let (text, read_only, name) = {
        let document = view.document();
        let name = document.path().map(display_name).unwrap_or_default();
        (document.text().to_string(), document.read_only().is_some(), name)
    };
    view.ivars().applying.set(true);
    view.setString(&ns(&text));
    view.ivars().applying.set(false);
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

/// Shows the dirty state of the view's document on its tab when it changed.
pub(crate) fn update_dirty(view: &EditorTextView) {
    let dirty = view.document().is_dirty();
    if dirty == view.ivars().shown_dirty.get() {
        return;
    }
    view.ivars().shown_dirty.set(dirty);
    if let Some(controller) = view.controller() {
        controller.set_tab_dirty(view.ivars().tab.get(), dirty);
    }
}

/// What other parts of the app follow: the language server and a project search that shows unsaved text.
fn notify_change(view: &EditorTextView, change: &Change) {
    crate::lsp::changed(view, change);
    crate::preview::edited(view);
    if let Some(path) = view.document().path() {
        crate::window::buffer_edited(path);
    }
}

/// Writes the document to its file, or asks for one; `save_as` always asks.
pub(crate) fn save(view: &EditorTextView, save_as: bool) {
    let target = if save_as || view.document().path().is_none() {
        let Some(path) = ask_path(view) else { return };
        Some(path)
    } else {
        None
    };
    indent::before_save(view, target.as_deref());
    let result = match &target {
        Some(path) => view.document_mut().save_as(path.clone()),
        None => view.document_mut().save(),
    };
    match result {
        Ok(()) => saved(view, target.as_deref()),
        Err(SaveError::ChangedOnDisk) => {
            let mtm = view.mtm();
            let name = view.document().path().map(display_name).unwrap_or_default();
            if confirm(
                mtm,
                &tf("„{}“ wurde inzwischen außerhalb geändert.", &[&name]),
                t("Sichern überschreibt diese Änderungen."),
                t("Überschreiben"),
            ) {
                let job = view.document().begin_save(true);
                match job.and_then(|job| job.run()) {
                    Ok(done) => {
                        view.document_mut().end_save(done);
                        saved(view, None);
                    }
                    Err(error) => alert(mtm, t("Sichern fehlgeschlagen"), &format!("{error:?}")),
                }
            }
        }
        Err(error) => alert(view.mtm(), t("Sichern fehlgeschlagen"), &format!("{error:?}")),
    }
}

fn saved(view: &EditorTextView, new_path: Option<&Path>) {
    update_dirty(view);
    recovery::discard(view);
    conflict::saved(view);
    if new_path.is_some() {
        crate::session::changed();
    }
    if let (Some(path), Some(controller)) = (new_path, view.controller()) {
        controller.set_tab_title(view.ivars().tab.get(), &tab_title(path));
        let language = ide_document::syntax::Language::detect(Some(path), "");
        view.document_mut().set_language(language);
        highlight::recolor_all(view);
        crate::lsp::opened(view);
        indent::configure(view);
        changes::reload(view, true);
    }
    crate::lsp::saved(view);
    if let (Some(path), Some(controller)) = (view.document().path().map(Path::to_path_buf), view.controller()) {
        crate::worktrees::file_saved(&controller, &path);
    }
}

fn ask_path(view: &EditorTextView) -> Option<PathBuf> {
    let panel = NSSavePanel::savePanel(view.mtm());
    if let Some(root) = view.controller().and_then(|c| c.project_root()) {
        panel.setDirectoryURL(Some(&objc2_foundation::NSURL::fileURLWithPath(&ns(
            &root.to_string_lossy()
        ))));
    }
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    panel
        .URL()
        .and_then(|url| url.path())
        .map(|p| PathBuf::from(p.to_string()))
}

pub(crate) fn toggle_wrap(view: &EditorTextView) {
    // SAFETY: A text view keeps its text container alive as long as it lives.
    let Some(container) = (unsafe { view.textContainer() }) else {
        return;
    };
    set_wrap(view, !container.widthTracksTextView());
}

/// Wraps long lines at the view's edge, or lets them run on to the right.
fn set_wrap(view: &EditorTextView, wrap: bool) {
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
fn follow_width(scroll: &NSScrollView, view: &EditorTextView) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::NSViewFrameDidChangeNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter};

    let clip = scroll.contentView();
    clip.setPostsFrameChangedNotifications(true);
    let name = constant!(NSViewFrameDidChangeNotification);
    let editor = Weak::from(view);
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        if let Some(editor) = editor.load() {
            fit_width(&editor);
        }
    });
    // SAFETY: The block only resizes the editor; the weak reference makes it do nothing once the editor is gone.
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(name),
            Some(&clip),
            None,
            &block,
        )
    };
    std::mem::forget(observer);
    fit_width(view);
}

/// Gives a wrapping view the width its scroll view shows, at its left edge, so lines wrap where the view ends; a text
/// view only follows its own frame, which nothing else narrows.
fn fit_width(view: &EditorTextView) {
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

/// Looks at the files of open editors in `dirs` and takes what changed on disk.
pub fn files_changed(dirs: &[PathBuf]) {
    for view in open_editors() {
        let path = view.document().path().map(Path::to_path_buf);
        let Some(path) = path else { continue };
        if !path.parent().is_some_and(|parent| dirs.iter().any(|dir| dir == parent)) {
            continue;
        }
        take_disk(&view);
    }
}

fn take_disk(view: &EditorTextView) {
    let Some(probe) = view.document().disk_probe() else {
        return;
    };
    let Ok(check) = probe.check() else { return };
    conflict::follow(view, check);
}

/// Looks at the file of each editor whose text came back after a crash, `ids` naming their copies: a file unchanged
/// since, or holding the same text, leaves the text as it came back; a file changed meanwhile gets the bar, never
/// merged without a word.
pub fn check_recovered(ids: &[RecoveryId]) {
    for view in open_editors() {
        if !ids.contains(&view.ivars().recovery) {
            continue;
        }
        let Some(probe) = view.document().disk_probe() else {
            continue;
        };
        match probe.check() {
            Ok(check) => conflict::follow(&view, check),
            Err(error) => crate::diag::report("looking at a recovered file", error),
        }
        recovery::store(&view);
    }
}

fn editor_in(controller: &WindowController, id: isize) -> Option<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .find(|v| v.ivars().tab.get() == id && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

/// The file and the copy against a crash of the editor in tab `id` of `controller`, if that tab is an editor.
pub fn tab_document(controller: &WindowController, id: isize) -> Option<(Option<PathBuf>, RecoveryId)> {
    let view = editor_in(controller, id)?;
    let path = view.document().path().map(Path::to_path_buf);
    Some((path, view.ivars().recovery.clone()))
}

/// Writes the copy of every editor of `controller` that holds unsaved text, as after a close the user cancelled.
pub fn store_copies(controller: &WindowController) {
    for view in open_editors() {
        if view.controller().is_some_and(|c| std::ptr::eq(&*c, controller)) {
            recovery::store(&view);
        }
    }
}

/// Whether the editor in tab `id` of `controller` may close: saved, or the user said so.
pub fn may_close(controller: &WindowController, id: isize) -> bool {
    let Some(view) = editor_in(controller, id) else {
        return true;
    };
    if conflict::is_gone(&view) {
        return conflict::may_close(&view);
    }
    if !view.document().is_dirty() {
        recovery::discard(&view);
        return true;
    }
    let name = view
        .document()
        .path()
        .map_or_else(|| t("Ohne Titel").to_string(), display_name);
    let alert = NSAlert::new(controller.mtm());
    alert.setMessageText(&ns(&tf("Änderungen an „{}“ sichern?", &[&name])));
    alert.setInformativeText(&ns(t("Ohne Sichern gehen deine Änderungen verloren.")));
    alert.addButtonWithTitle(&ns(t("Sichern")));
    alert.addButtonWithTitle(&ns(t("Nicht sichern")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let response = alert.runModal();
    if response == NSAlertFirstButtonReturn {
        save(&view, false);
        !view.document().is_dirty()
    } else if response == NSAlertSecondButtonReturn {
        recovery::discard(&view);
        true
    } else {
        false
    }
}

/// Whether the app may quit: the user saved or gave up the text of each editor whose file is gone, one question each,
/// then saved or dropped the other unsaved changes. When the user cancels, every editor keeps its copy.
pub fn confirm_quit(mtm: MainThreadMarker) -> bool {
    let agreed = open_editors()
        .iter()
        .filter(|view| conflict::is_gone(view))
        .all(|view| conflict::may_close(view))
        && unsaved_may_go(mtm);
    if !agreed {
        for view in open_editors() {
            recovery::store(&view);
        }
    }
    agreed
}

fn unsaved_may_go(mtm: MainThreadMarker) -> bool {
    let count = dirty_count();
    if count == 0 {
        return true;
    }
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&tf(
        "{} Dateien haben ungesicherte Änderungen.",
        &[&count.to_string()],
    )));
    alert.setInformativeText(&ns(t("Ohne Sichern gehen deine Änderungen verloren.")));
    alert.addButtonWithTitle(&ns(t("Alle sichern")));
    alert.addButtonWithTitle(&ns(t("Nicht sichern")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let response = alert.runModal();
    if response == NSAlertFirstButtonReturn {
        save_all()
    } else if response == NSAlertSecondButtonReturn {
        for view in open_editors() {
            recovery::discard(&view);
        }
        true
    } else {
        false
    }
}

/// The editors of `controller`'s window.
fn editors_in(controller: &WindowController) -> Vec<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .filter(|view| view.controller().is_some_and(|own| std::ptr::eq(&*own, controller)))
        .collect()
}

/// The names of the documents in `controller`'s window whose text closing would lose: unsaved changes, and files gone
/// from disk (E43).
pub fn unsaved_in(controller: &WindowController) -> Vec<String> {
    editors_in(controller)
        .into_iter()
        .filter(|view| view.document().is_dirty() || conflict::is_gone(view))
        .map(|view| {
            view.document()
                .path()
                .map_or_else(|| t("Ohne Titel").to_string(), display_name)
        })
        .collect()
}

/// Gives up the changes in the editors of `controller`'s window and their crash copies, as "Nicht sichern" does.
pub fn discard_changes(controller: &WindowController) {
    for view in editors_in(controller) {
        recovery::discard(&view);
    }
}

/// How many open editors have unsaved changes, those of gone files aside, which get a question of their own.
pub fn dirty_count() -> usize {
    unsaved().len()
}

/// Saves every editor with unsaved changes, those of gone files aside; whether all of them were saved.
pub fn save_all() -> bool {
    for view in unsaved() {
        save(&view, false);
    }
    dirty_count() == 0
}

fn unsaved() -> Vec<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .filter(|view| view.document().is_dirty() && !conflict::is_gone(view))
        .collect()
}

fn confirm(mtm: MainThreadMarker, message: &str, detail: &str, action: &str) -> bool {
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&ns(message));
    alert.setInformativeText(&ns(detail));
    alert.addButtonWithTitle(&ns(action));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    alert.runModal() == NSAlertFirstButtonReturn
}

fn alert(mtm: MainThreadMarker, message: &str, detail: &str) {
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&ns(message));
    alert.setInformativeText(&ns(detail));
    alert.runModal();
}

/// The editors of `controller` with their tab ids, for the self-test.
#[cfg(feature = "selftest")]
pub fn editors_of(controller: &WindowController) -> Vec<(isize, Retained<EditorTextView>)> {
    open_editors()
        .into_iter()
        .filter(|v| v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
        .map(|v| (v.ivars().tab.get(), v))
        .collect()
}

/// What the bar above `view` says and offers while it shows, for the self-test.
#[cfg(feature = "selftest")]
pub fn conflict_bar(view: &EditorTextView) -> Option<(String, Vec<String>)> {
    conflict::inspect(view)
}

/// The message, informative text and buttons with their keys of the question before the text of a gone file's editor
/// goes, for the self-test.
#[cfg(feature = "selftest")]
pub fn discard_question(view: &EditorTextView) -> (String, String, Vec<(String, String)>) {
    conflict::inspect_question(view)
}

/// Clicks the button titled `title` of the bar above `view`, for the self-test; whether there was one.
#[cfg(feature = "selftest")]
pub fn press_conflict_button(view: &EditorTextView, title: &str) -> bool {
    conflict::press(view, title)
}

/// The text of the tab with the differences for `path`, for the self-test.
#[cfg(feature = "selftest")]
pub fn compared(path: &Path) -> Option<String> {
    compare::shown(path)
}

/// The copy against a crash of `view`, for the self-test.
#[cfg(feature = "selftest")]
pub fn recovery_of(view: &EditorTextView) -> RecoveryId {
    view.ivars().recovery.clone()
}

/// The text view of the front editor of `controller`, for the self-test.
#[cfg(feature = "selftest")]
pub fn editor_of(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    editor_for(controller, path)
}

/// What the self-test does with folds and carets, through the code the commands run.
#[cfg(feature = "selftest")]
pub mod selftest {
    pub use super::fold::{
        click as click_fold, fold_all, fold_at_cursor, folded, from_server, gutter, laid_out_lines, placeholders,
        unfold_all,
    };
    pub use super::multi::{add_next_occurrence, add_vertical, carets, column, copied, paste_text, set as set_carets};
}

/// Types `text` as the keyboard would, through the view's input path.
#[cfg(feature = "selftest")]
pub fn type_text(view: &EditorTextView, text: &str) {
    use objc2_app_kit::NSTextInputClient;
    // SAFETY: The inserted object is an NSString, one of the two types this method accepts.
    unsafe {
        view.insertText_replacementRange(
            &ns(text),
            objc2_foundation::NSRange::new(objc2_foundation::NSNotFound.cast_unsigned(), 0),
        );
    }
}

/// Presses Return in `view`.
#[cfg(feature = "selftest")]
pub fn type_newline(view: &EditorTextView) {
    editing::newline(view);
}

/// Saves `view` as ⌘S does.
#[cfg(feature = "selftest")]
pub fn save_view(view: &EditorTextView) {
    save(view, false);
}

#[cfg(feature = "selftest")]
pub use changes::{Mark as ChangeMark, Seen as ChangesSeen, popover_parts as change_popover, seen as changes_seen};
#[cfg(feature = "selftest")]
pub use indent::shown as indent_shown;
#[cfg(feature = "selftest")]
pub use whitespace::visible as whitespace_visible;

/// Clicks the change mark of `line`, counted from 0, in `view`'s line number column, as the mouse would.
#[cfg(feature = "selftest")]
pub fn click_change(view: &EditorTextView, line: usize) -> bool {
    let Some(gutter) = view.ivars().gutter.borrow().as_ref().and_then(Weak::load) else {
        return false;
    };
    let Some(start) = view.line_start(line) else {
        return false;
    };
    let Some(row) = crate::lsp::rect_of(view, start..start + 1) else {
        return false;
    };
    let row = gutter.convertRect_fromView(row, Some(view));
    let width = gutter.bounds().size.width;
    let at = NSRect::new(
        NSPoint::new(width - 8.0, row.origin.y),
        NSSize::new(8.0, row.size.height),
    );
    changes::clicked(view, line, &gutter, at)
}

/// Whether the first `fn` of `text` is shown in a keyword colour.
#[cfg(feature = "selftest")]
pub fn keyword_coloured(view: &EditorTextView, text: &str) -> bool {
    let Some(at) = text.find("fn ") else { return false };
    let Some(storage) = view.storage() else { return false };
    let at = text[..at].encode_utf16().count();
    let key = constant!(NSForegroundColorAttributeName);
    // SAFETY: The index lies inside the text; no effective range is asked for.
    let color = unsafe { storage.attribute_atIndex_effectiveRange(key, at, std::ptr::null_mut()) };
    let expected: Retained<objc2::runtime::AnyObject> =
        style::token_color(splat_markdown::code::CodeToken::Keyword).into();
    color.is_some_and(|c| std::ptr::eq(&*c, &*expected))
}

/// The colour at UTF-16 offset `at` in sRGB as the view's appearance shows it, and whether the font there is bold.
#[cfg(feature = "selftest")]
pub fn styled_at(view: &EditorTextView, at: usize) -> Option<([f64; 3], bool)> {
    use objc2_app_kit::{NSAppearanceCustomization, NSColorSpace, NSFont, NSFontDescriptorSymbolicTraits};
    let storage = view.storage()?;
    let (color_key, font_key) = (
        constant!(NSForegroundColorAttributeName),
        constant!(NSFontAttributeName),
    );
    // SAFETY: The index lies inside the text; no effective range is asked for.
    let color = unsafe { storage.attribute_atIndex_effectiveRange(color_key, at, std::ptr::null_mut()) }?;
    // SAFETY: As above.
    let font = unsafe { storage.attribute_atIndex_effectiveRange(font_key, at, std::ptr::null_mut()) }?;
    let color = color.downcast::<NSColor>().ok()?;
    let rgb = std::cell::Cell::new(None);
    view.effectiveAppearance()
        .performAsCurrentDrawingAppearance(&block2::RcBlock::new(|| {
            rgb.set(
                color
                    .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
                    .map(|c| [c.redComponent(), c.greenComponent(), c.blueComponent()]),
            );
        }));
    let bold = font
        .downcast::<NSFont>()
        .ok()?
        .fontDescriptor()
        .symbolicTraits()
        .contains(NSFontDescriptorSymbolicTraits::TraitBold);
    Some((rgb.get()?, bold))
}

/// Whether long lines wrap at the view's edge.
pub fn wraps(view: &EditorTextView) -> bool {
    // SAFETY: A text view keeps its text container alive as long as it lives.
    unsafe { view.textContainer() }.is_some_and(|c| c.widthTracksTextView())
}
