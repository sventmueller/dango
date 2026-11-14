//! The editor tab: a TextKit 2 text view over one document of the model in `ide-document`.

mod blame;
mod blame_column;
mod changes;
mod compare;
mod conflict;
mod container;
mod context_menu;
mod editing;
mod fold;
mod highlight;
mod indent;
mod inset_fragment;
pub mod insets;
mod large;
mod line_diagnostics;
mod lsp_editor;
mod minimap;
mod multi;
mod on_disk;
mod overlay;
pub mod predict;
mod sticky;
pub use lsp_editor::lsp_editor;
pub use overlay::follow as follow_overlay;
pub(crate) use saving::{save, save_soon};
pub mod recovery;
mod ruler;
mod saving;
mod style;
mod view;
mod vim;
mod whitespace;

use std::path::{Path, PathBuf};

use ide_document::recovery::{Recovered, RecoveryId};
use ide_document::{Change, Document};
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle, NSColor,
    NSFontAttributeName, NSForegroundColorAttributeName, NSScrollView, NSUserInterfaceLayoutDirection,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use crate::util::{FlagGuard, constant, ns};
use crate::window::{TabKind, WindowController};
pub use view::EditorTextView;

/// The state of the editors beside the list of them.
#[derive(Default)]
pub struct EditorState {
    minimap: minimap::Shown,
    sticky: sticky::Lines,
    line_diagnostics: line_diagnostics::Shown,
    vim: vim::Enabled,
    comparisons: compare::Comparisons,
    recovery_copies: recovery::RecoveryCopies,
    #[cfg(feature = "selftest")]
    forced_contrast: insets::ForcedContrast,
}

/// The editors still open.
fn open_editors() -> Vec<Retained<EditorTextView>> {
    let cx = crate::context::app();
    let mut open = cx.editors().borrow_mut();
    open.retain(|view| view.load().is_some());
    open.iter().filter_map(Weak::load).collect()
}

/// The editors still open, in every window.
pub fn all_editors() -> Vec<Retained<EditorTextView>> {
    open_editors()
}

/// The open editor with `serial`, the number that other threads hold instead of the view.
pub fn find_editor(serial: u64) -> Option<Retained<EditorTextView>> {
    open_editors().into_iter().find(|v| v.ivars().serial == serial)
}

/// The editor of `path` in `controller`'s window.
pub fn editor_for(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .find(|v| v.document().path() == Some(path) && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

/// The UTF-16 range of the lines on screen, with a margin.
pub fn visible_utf16(view: &EditorTextView) -> std::ops::Range<usize> {
    utf16_in(view, view.visibleRect())
}

/// The UTF-16 range of the lines laid out in `rect` of the view, with a margin of lines on both sides.
pub fn utf16_in(view: &EditorTextView, rect: NSRect) -> std::ops::Range<usize> {
    let lines = highlight::lines_in(view, rect);
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

/// The top of the line holding byte `byte`, in the view's coordinates.
pub fn byte_y(view: &EditorTextView, byte: usize) -> Option<f64> {
    let at = view.document().text().byte_to_utf16(byte).ok()?;
    let layout = view.textLayoutManager()?;
    let content = view.textContentStorage()?;
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let location = content.locationFromLocation_withOffset(&start, isize::try_from(at).unwrap_or(isize::MAX))?;
    lay_out_whole(view, &layout, &content);
    ensure_paragraph(&layout, &location);
    let fragment = layout.textLayoutFragmentForLocation(&location)?;
    Some(fragment.layoutFragmentFrame().origin.y + view.textContainerOrigin().y)
}

/// Lays out the whole text of `view`, once per revision and size of the view. TextKit 2 places a fragment by estimated
/// heights until all text is laid out, and a jump would land beside it.
fn lay_out_whole(
    view: &EditorTextView,
    layout: &objc2_app_kit::NSTextLayoutManager,
    content: &objc2_app_kit::NSTextContentStorage,
) {
    let revision = view.document().revision();
    if view.ivars().laid_out.get() == Some((revision, view.frame().size)) {
        return;
    }
    layout.ensureLayoutForRange(&objc2_app_kit::NSTextElementProvider::documentRange(content));
    view.ivars().laid_out.set(Some((revision, view.frame().size)));
}

/// The tab that shows `view`.
pub fn tab_of(view: &EditorTextView) -> isize {
    view.ivars().tab.get()
}

/// Redraws the line number column of `view`, after its diagnostics changed, once for any number of asks in a turn of
/// the run loop.
pub fn redraw_gutter(view: &EditorTextView) {
    if view.ivars().gutter_due.replace(true) {
        return;
    }
    let serial = view.ivars().serial;
    crate::util::main_soon(move || {
        if let Some(view) = find_editor(serial) {
            redraw_gutter_now(&view);
        }
    });
}

fn redraw_gutter_now(view: &EditorTextView) {
    view.ivars().gutter_due.set(false);
    if let Some(gutter) = view.ivars().gutter.borrow().as_ref().and_then(Weak::load) {
        gutter.setNeedsDisplay(true);
    }
    minimap::redraw(view);
    sticky::refresh(view);
}

/// Lays out the paragraph at `location`. A paragraph TextKit has not laid out yet answers with a fragment of its
/// estimate, without line fragments, which a later edit trips over as one of no text
/// ("NSTextLayoutManagerBreakOnNilContentManager").
fn ensure_paragraph(
    layout: &objc2_app_kit::NSTextLayoutManager,
    location: &objc2::runtime::ProtocolObject<dyn objc2_app_kit::NSTextLocation>,
) {
    if let Some(range) = objc2_app_kit::NSTextRange::initWithLocation_endLocation(
        <objc2_app_kit::NSTextRange as objc2::AllocAnyThread>::alloc(),
        location,
        Some(location),
    ) {
        layout.ensureLayoutForRange(&range);
    }
}

/// `line_rect` of a line that may lie outside what TextKit laid out, such as the paused line of a debugger, laying
/// its paragraph out first.
pub fn laid_out_line_rect(editor: &EditorTextView, line: usize) -> Option<NSRect> {
    let at = editor.line_start(line)?;
    let (layout, content) = (editor.textLayoutManager()?, editor.textContentStorage()?);
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let location = content.locationFromLocation_withOffset(&start, isize::try_from(at).ok()?)?;
    let fragment = layout.textLayoutFragmentForLocation(&location)?;
    if insets::line_fragments(&fragment).is_none() {
        ensure_paragraph(&layout, &location);
    }
    line_rect(editor, line)
}

/// The rectangle of line `line` across the whole width of `editor`.
pub fn line_rect(editor: &EditorTextView, line: usize) -> Option<NSRect> {
    let at = editor.line_start(line)?;
    let layout = editor.textLayoutManager()?;
    let content = editor.textContentStorage()?;
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let location = content.locationFromLocation_withOffset(&start, isize::try_from(at).ok()?)?;
    let fragment = layout.textLayoutFragmentForLocation(&location)?;
    let frame = fragment.layoutFragmentFrame();
    let origin = editor.textContainerOrigin();
    Some(NSRect::new(
        NSPoint::new(0.0, frame.origin.y + origin.y),
        NSSize::new(editor.bounds().size.width, frame.size.height),
    ))
}

pub use blame::{at_line_end as blame_at_line_end, set_line_end as set_blame_at_line_end, with_view_items};
pub use fold::hidden_by_fold;
pub use ide_ui_common::util::undo_step;
pub use line_diagnostics::refresh as diagnostics_changed;
pub use minimap::redraw as redraw_minimap;
pub use style::{code_font, palette_color};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// Takes the language server's folding ranges, as first line and last hidden line, or `None` for the indentation's.
pub fn set_server_folds(view: &EditorTextView, ranges: Option<Vec<(usize, usize)>>) {
    fold::set_server_ranges(view, ranges);
}

/// The file of `view` got a new name, as a server's rename of a file did: the tab follows.
pub fn path_changed(view: &EditorTextView) {
    if let (Some(path), Some(controller)) = (view.document().path().map(Path::to_path_buf), view.controller()) {
        controller.set_tab_title(view.ivars().tab.get(), &tab_title(&path));
    }
    ide_ui_lsp::opened(view);
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

/// Settings changed outside the editor's own switches, such as by the sync from another Mac: the editors read the
/// changed ones again and show them.
pub fn settings_changed(names: &[String]) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let cx = crate::context::AppContext::get(mtm);
    for name in names {
        match name.as_str() {
            "minimap" => cx.editor().minimap.forget(),
            "stickyScrollLines" => cx.editor().sticky.forget(),
            "diagnosticsAtLineEnd" => cx.editor().line_diagnostics.forget(),
            "showWhitespace" => whitespace::forget(),
            "blameAtLineEnd" => blame::forget(),
            "vimMode" => {
                cx.editor().vim.forget();
                for view in open_editors() {
                    vim::attach(&view);
                }
            }
            _ => {}
        }
    }
    for view in open_editors() {
        if let Some(map) = view.ivars().minimap.borrow().as_ref().and_then(Weak::load) {
            map.fit();
        }
        sticky::refresh(&view);
        line_diagnostics::refresh(&view);
    }
}

/// Puts the unsaved texts a restored checkpoint held into their editors, opening in `controller` the files not open,
/// each as one undoable step that leaves the editor unsaved.
pub fn take_back_buffers(controller: &WindowController, buffers: Vec<(PathBuf, Vec<u8>)>) {
    for (path, bytes) in buffers {
        let Ok(text) = String::from_utf8(bytes) else { continue };
        let view = match open_editors()
            .into_iter()
            .find(|v| v.document().path() == Some(path.as_path()))
        {
            Some(view) => view,
            None => {
                controller.open_file(&path);
                let Some(view) = editor_for(controller, &path) else {
                    continue;
                };
                view
            }
        };
        take_disk(&view);
        let changes = view.document_mut().replace_all(&text);
        match changes {
            Ok(changes) => view.apply_model_changes(&changes),
            Err(error) => crate::diag::report("taking back an unsaved text", error),
        }
        update_dirty(&view);
    }
}

/// Adds the controls of the editor's whitespace rules, and of dictation, to its status bar's `stack`.
pub fn status_items(mtm: MainThreadMarker, view: &EditorTextView, stack: &objc2_app_kit::NSStackView) {
    vim::status_items(mtm, view, stack);
    indent::status_items(mtm, view, stack);
    predict::status_items(mtm, view, stack);
    #[cfg(feature = "dictation")]
    ide_ui_dictation::editor_items(mtm, std::rc::Rc::new(EditorField(Weak::from(view))), view, stack);
}

/// An editor as a field that dictation goes into.
#[cfg(feature = "dictation")]
struct EditorField(Weak<EditorTextView>);

#[cfg(feature = "dictation")]
impl ide_ui_dictation::DictationField for EditorField {
    fn notice(&self, text: &str) -> bool {
        let controller = self.0.load().and_then(|editor| editor.controller());
        controller.inspect(|controller| controller.show_notice(text)).is_some()
    }
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
        ide_ui_lsp::closed(&*view);
        crate::markdown::closing(&view);
        crate::context::app()
            .editors()
            .borrow_mut()
            .retain(|w| w.load().is_some_and(|v| !std::ptr::eq(&*v, &*view)));
    }
}

/// Shows a warning with `message` and `detail` and waits for OK.
pub fn show_alert(mtm: MainThreadMarker, message: &str, detail: &str) {
    alert(mtm, message, detail);
}

/// Shows `text`, such as a test run's output, in a scrolling field of an alert titled `message`.
pub fn show_text(mtm: MainThreadMarker, message: &str, text: &str) {
    use objc2_app_kit::{NSAlert, NSScrollView, NSTextView};
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(message));
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(560.0, 320.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    view.setEditable(false);
    view.setFont(Some(&code_font(11.0)));
    view.setString(&ns(text));
    scroll.setDocumentView(Some(&view));
    alert.setAccessoryView(Some(&scroll));
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return;
    }
    alert.runModal();
}

/// The editor of `controller`'s window that has the keyboard focus, else the one in view.
pub fn focused_editor(controller: &WindowController) -> Option<Retained<EditorTextView>> {
    let mine: Vec<Retained<EditorTextView>> = open_editors()
        .into_iter()
        .filter(|e| e.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
        .collect();
    let responder = controller.window().and_then(|w| w.firstResponder());
    mine.iter()
        .find(|e| {
            responder
                .as_ref()
                .is_some_and(|r| std::ptr::eq(Retained::as_ptr(r).cast::<u8>(), Retained::as_ptr(e).cast::<u8>()))
        })
        .or_else(|| mine.iter().find(|e| !e.isHiddenOrHasHiddenAncestor()))
        .cloned()
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
    if large::is_large(path) && large::open(controller, path, font_size, preview) {
        return;
    }
    opened(controller, path, Document::open(path), font_size, preview);
}

fn opened(
    controller: &WindowController,
    path: &Path,
    document: std::io::Result<Document>,
    font_size: f64,
    preview: bool,
) {
    let mtm = controller.mtm();
    let document = match document {
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

/// Opens `path` in `controller`, or selects its tab, and marks `columns`, in UTF-16 code units, of line `line`,
/// counted from 1; `focus` selects them in the editor and gives it the keyboard focus.
pub fn reveal(controller: &WindowController, path: &Path, line: u64, columns: std::ops::Range<usize>, focus: bool) {
    controller.open_file(path);
    large::with_opened(controller, path, move |view| {
        let line = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
        view.select_columns(line, columns, focus);
    });
}

/// Opens `path` in an editor tab of `controller`, or brings its tab forward, with the cursor on line `line` at column
/// `column`, both counted from 1 in characters as compilers print them; a column past the end means the line's end.
pub fn reveal_position(controller: &WindowController, path: &Path, line: usize, column: usize) {
    controller.open_file(path);
    large::with_opened(controller, path, move |view| {
        place_cursor(view, line, column);
    });
}

fn place_cursor(view: &EditorTextView, line: usize, column: usize) {
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
    let (container, gutter) = container::editor_container(mtm, &scroll, &view);
    ide_ui_lsp::attach(&*view);
    follow_scrolling(&scroll, &view);
    whitespace::attach(&view, &scroll);
    vim::attach(&view);
    overlay::attach(&view);
    insets::attach(&view);
    crate::debug::attach_rotor(&view, controller.context().debug_rotors());
    *view.ivars().gutter.borrow_mut() = Some(objc2::rc::Weak::from(&*gutter));
    let id = if preview {
        controller.add_preview_tab(TabKind::Editor, key, title, container)
    } else {
        controller.add_tab(TabKind::Editor, key, title, container)
    };
    view.ivars().tab.set(id);
    crate::context::app().editors().borrow_mut().push(Weak::from(&*view));
    highlight::recolor_all(&view);
    ide_ui_lsp::opened(&*view);
    changes::reload(&view, true);
    if !preview && let Some(window) = view.window() {
        window.makeFirstResponder(Some(&view));
    }
    view
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
    {
        let _applying = FlagGuard::raise(&view.ivars().applying);
        view.setString(&ns(&text));
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
    ide_ui_lsp::changed(view, change);
    predict::changed(view);
    crate::preview::edited(view);
    crate::debug::edited(view, change);
    if let Some(path) = view.document().path() {
        crate::window::buffer_edited(path);
    }
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
    let clip = scroll.contentView();
    clip.setPostsFrameChangedNotifications(true);
    let name = constant!(objc2_app_kit::NSViewFrameDidChangeNotification);
    let editor = Weak::from(view);
    let watch = crate::util::observe(view.mtm(), name, Some(&clip), move |_| {
        if let Some(editor) = editor.load() {
            fit_width(&editor);
        }
    });
    view.ivars().watches.borrow_mut().push(watch);
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
        on_disk::look(&view, on_disk::Look::Changed);
    }
}

/// Looks at the files of every open editor below `root` and takes what changed on disk, after a git operation of the
/// app's own: the watcher reports only folders the file tree has read.
pub fn check_disk_below(root: &Path) {
    for view in open_editors() {
        if view.document().path().is_some_and(|path| path.starts_with(root)) {
            on_disk::look(&view, on_disk::Look::Changed);
        }
    }
}

/// Takes what changed in the file of `view` before going on, as a restored checkpoint must before it puts its text in.
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
        if ids.contains(&view.ivars().recovery) {
            on_disk::look(&view, on_disk::Look::Recovered);
        }
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

/// The editor of the document `tab` in `controller`'s window.
pub fn editor_of_tab(controller: &WindowController, tab: isize) -> Option<Retained<EditorTextView>> {
    editors_in(controller)
        .into_iter()
        .find(|view| view.ivars().tab.get() == tab)
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

/// The files below `root` whose open editors have unsaved changes, those of gone files aside.
pub fn unsaved_below(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = unsaved()
        .iter()
        .filter_map(|view| {
            view.document()
                .path()
                .filter(|p| p.starts_with(root))
                .map(Path::to_path_buf)
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Saves the editors of files below `root` with unsaved changes; whether none is left unsaved.
pub fn save_below(root: &Path) -> bool {
    let below = |view: &Retained<EditorTextView>| view.document().path().is_some_and(|p| p.starts_with(root));
    for view in unsaved().iter().filter(|view| below(view)) {
        save(view, false);
    }
    !unsaved().iter().any(below)
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

/// Tells the language servers when `scroll` shows other lines of `view`.
fn follow_scrolling(scroll: &NSScrollView, view: &Retained<EditorTextView>) {
    let clip = scroll.contentView();
    clip.setPostsBoundsChangedNotifications(true);
    let editor = Weak::from(&**view);
    let name = constant!(objc2_app_kit::NSViewBoundsDidChangeNotification);
    let watch = crate::util::observe(view.mtm(), name, Some(&clip), move |_| {
        if let Some(editor) = editor.load() {
            ide_ui_lsp::scrolled(&*editor);
        }
    });
    view.ivars().watches.borrow_mut().push(watch);
}

/// Whether long lines wrap at the view's edge.
pub fn wraps(view: &EditorTextView) -> bool {
    // SAFETY: A text view keeps its text container alive as long as it lives.
    unsafe { view.textContainer() }.is_some_and(|c| c.widthTracksTextView())
}
