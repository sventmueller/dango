//! The editor tab: a TextKit 2 text view over one document of the model in `ide-document`.

mod blame;
mod blame_column;
mod changes;
mod closing;
mod compare;
mod conflict;
mod container;
mod context_menu;
mod editing;
mod file;
mod fold;
mod highlight;
mod indent;
mod inset_fragment;
pub(crate) mod insets;
mod line_diagnostics;
mod long_lines;
mod lsp_editor;
mod minimap;
mod multi;
mod on_disk;
mod opening;
mod overlay;
mod place;
pub(crate) mod predict;
mod reading;
mod sticky;
pub(crate) use lsp_editor::lsp_editor;
pub(crate) use overlay::follow as follow_overlay;
#[cfg(feature = "selftest")]
pub(crate) use saving::{run_until, save};
pub(crate) use saving::{save_soon, save_then};
pub(crate) mod recovery;
mod resync;
mod ruler;
mod saving;
mod style;
mod view;
mod viewport;
mod vim;
mod whitespace;

use std::path::{Path, PathBuf};

use ide_document::Change;
use ide_document::recovery::RecoveryId;
use ide_ui_common::alert::Alert;
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::tf;

use crate::util::ns;
use crate::window::{TabId, WindowController};
pub(crate) use closing::{
    Again, Answer, confirm_quit, discard_changes, keep_copies, may_close, save_below, save_each, store_copies,
    tab_document, unsaved_below, unsaved_in,
};
#[cfg(feature = "selftest")]
pub(crate) use closing::{dirty_count, save_all, save_below_now};
use opening::opened;
#[cfg(feature = "selftest")]
pub(crate) use opening::wraps;
pub(crate) use opening::{
    attach_restored, new_untitled, open_recovered, open_tab, reveal, reveal_position, set_font_size, toggle_wrap,
};
pub(crate) use reading::with_opened;
pub(crate) use view::EditorTextView;

/// The state of the editors beside the list of them.
#[derive(Default)]
pub(crate) struct EditorState {
    minimap: minimap::Shown,
    sticky: sticky::Lines,
    line_diagnostics: line_diagnostics::Shown,
    vim: vim::Enabled,
    comparisons: compare::Comparisons,
    recovery_copies: recovery::RecoveryCopies,
    reads: reading::Reads,
    #[cfg(feature = "selftest")]
    forced_contrast: insets::ForcedContrast,
}

/// The editors still open.
fn open_editors() -> Vec<Retained<EditorTextView>> {
    crate::context::app().editors().live()
}

/// The editors still open, in every window.
pub(crate) fn all_editors() -> Vec<Retained<EditorTextView>> {
    open_editors()
}

/// The open editor with `serial`, the number that other threads hold instead of the view.
pub(crate) fn find_editor(serial: ide_ui_lsp::EditorSerial) -> Option<Retained<EditorTextView>> {
    open_editors().into_iter().find(|v| v.ivars().serial == serial)
}

/// The editor of `path` in `controller`'s window.
pub(crate) fn editor_for(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    editors_in(controller)
        .into_iter()
        .find(|v| v.document().path() == Some(path))
}

/// The UTF-16 range of the lines on screen, with a margin.
pub(crate) fn visible_utf16(view: &EditorTextView) -> std::ops::Range<usize> {
    utf16_in(view, view.visibleRect())
}

/// The UTF-16 range of the lines laid out in `rect` of the view, with a margin of lines on both sides.
pub(crate) fn utf16_in(view: &EditorTextView, rect: NSRect) -> std::ops::Range<usize> {
    let lines = highlight::lines_in(view, rect);
    let length = view.document().text().len_utf16();
    let start = view.line_start(lines.start).unwrap_or(0);
    let end = view.line_start(lines.end).unwrap_or(length);
    start..end.max(start)
}

/// The tab that shows `view`.
pub(crate) fn tab_of(view: &EditorTextView) -> Option<TabId> {
    view.ivars().tab.get()
}

/// The rectangle of line `line` across the whole width of `editor`.
pub(crate) fn line_rect(editor: &EditorTextView, line: usize) -> Option<NSRect> {
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

pub(crate) use blame::{at_line_end as blame_at_line_end, set_line_end as set_blame_at_line_end, with_view_items};
pub(crate) use container::redraw_gutter;
pub(crate) use file::update_dirty;
pub(crate) use fold::hidden_by_fold;
pub(crate) use ide_ui_common::util::undo_step;
pub(crate) use line_diagnostics::refresh as diagnostics_changed;
pub(crate) use minimap::redraw as redraw_minimap;
pub(crate) use place::{byte_y, byte_y_near, laid_out_line_rect, top_byte};
pub(crate) use style::{code_font, palette_color};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

/// Takes the language server's folding ranges, as first line and last hidden line, or `None` for the indentation's.
pub(crate) fn set_server_folds(view: &EditorTextView, ranges: Option<Vec<(usize, usize)>>) {
    fold::set_server_ranges(view, ranges);
}

/// The file of `view` got a new name, as a server's rename of a file did: the tab follows.
pub(crate) fn path_changed(view: &EditorTextView) {
    if let (Some(path), Some(controller), Some(tab)) = (
        view.document().path().map(Path::to_path_buf),
        view.controller(),
        view.ivars().tab.get(),
    ) {
        controller.set_tab_title(tab, &tab_title(&path));
    }
    ide_ui_lsp::opened(view);
    changes::reload(view, true);
}

/// Something changed in a `.git` folder below `root`: the change marks of its editors follow the commit.
pub(crate) fn git_changed(root: &Path) {
    changes::git_changed(root);
}

/// The texts of the editors with unsaved changes to files below `root`, which a project search reads in place of
/// the files.
pub(crate) fn unsaved_buffers(root: &Path) -> Vec<(PathBuf, String)> {
    unsaved_texts(root)
        .into_iter()
        .map(|(path, text)| (path, text.to_string()))
        .collect()
}

/// Shares unsaved text with a worker without materializing the whole buffer on the main thread.
pub(crate) fn unsaved_texts(root: &Path) -> Vec<(PathBuf, ide_document::Text)> {
    open_editors()
        .iter()
        .filter_map(|view| {
            let document = view.document();
            let path = document.path().filter(|p| document.is_dirty() && p.starts_with(root))?;
            Some((path.to_path_buf(), document.text().clone()))
        })
        .collect()
}

/// Settings changed outside the editor's own switches, such as by the sync from another Mac: the editors read the
/// changed ones again and show them.
pub(crate) fn settings_changed(names: &[String]) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let cx = crate::context::AppContext::get(mtm);
    settings_change::apply(&cx, names);
}

mod settings_change;

/// Puts the unsaved texts a restored checkpoint held into their editors, opening in `controller` the files not open,
/// each as one undoable step that leaves the editor unsaved.
pub(crate) fn take_back_buffers(controller: &WindowController, buffers: Vec<(PathBuf, Vec<u8>)>) {
    for (path, bytes) in buffers {
        let Ok(text) = String::from_utf8(bytes) else { continue };
        let apply = move |view: &EditorTextView| {
            reading::take_disk(view, move |view| {
                let changes = view.document_mut().replace_all(&text);
                match changes {
                    Ok(changes) => view.apply_model_changes(&changes),
                    Err(error) => crate::diag::report("taking back an unsaved text", error),
                }
                update_dirty(view);
            });
        };
        if let Some(view) = open_editors()
            .into_iter()
            .find(|v| v.document().path() == Some(path.as_path()))
        {
            apply(&view);
        } else {
            controller.open_file(&path);
            with_opened(controller, &path, apply);
        }
    }
}

/// Adds the controls of the editor's whitespace rules, and of dictation, to its status bar's `stack`.
pub(crate) fn status_items(mtm: MainThreadMarker, view: &EditorTextView, stack: &objc2_app_kit::NSStackView) {
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
pub(crate) fn formatting_options(view: &EditorTextView) -> ide_lsp::lsp_types::FormattingOptions {
    let (tab_size, insert_spaces) = indent::formatting(view);
    ide_lsp::lsp_types::FormattingOptions {
        tab_size,
        insert_spaces,
        ..Default::default()
    }
}

/// The editor in tab `id` of `controller` is about to close.
pub(crate) fn closing(controller: &WindowController, id: TabId) {
    if let Some(view) = editor_of_tab(controller, id) {
        ide_ui_lsp::closed(&*view);
        crate::markdown::closing(&view);
        controller
            .context()
            .editors()
            .retain(|w| w.load().is_some_and(|v| !std::ptr::eq(&*v, &*view)));
    }
}

/// Shows `text`, such as a test run's output, in a scrolling field of an alert titled `message`.
pub(crate) fn show_text(mtm: MainThreadMarker, message: &str, text: &str) {
    use objc2_app_kit::{NSScrollView, NSTextView};
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(560.0, 320.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    view.setEditable(false);
    view.setFont(Some(&code_font(11.0)));
    view.setString(&ns(text));
    scroll.setDocumentView(Some(&view));
    let alert = Alert::new(mtm, message).accessory(&scroll);
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return;
    }
    alert.run_modal();
}

/// The editor of `controller`'s window that has the keyboard focus, else the one in view.
pub(crate) fn focused_editor(controller: &WindowController) -> Option<Retained<EditorTextView>> {
    let mine = editors_in(controller);
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

pub(crate) fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The title of a file's tab: its name, and for a file in a thread's worktree (I1) which worktree, so the user sees
/// which copy they edit.
pub(crate) fn tab_title(path: &Path) -> String {
    let name = display_name(path);
    match crate::worktrees::tree_of(path) {
        Some(tree) => {
            let short = tree.branch.strip_prefix("thread/").unwrap_or(&tree.branch).to_string();
            tf("{} · Worktree {}", &[&name, &short])
        }
        None => name,
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

/// Looks at the files of open editors in `dirs` and takes what changed on disk.
pub(crate) fn files_changed(dirs: &[PathBuf]) {
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
pub(crate) fn check_disk_below(root: &Path) {
    for view in open_editors() {
        if view.document().path().is_some_and(|path| path.starts_with(root)) {
            on_disk::look(&view, on_disk::Look::Changed);
        }
    }
}

/// Looks at the file of each editor whose text came back after a crash, `ids` naming their copies: a file unchanged
/// since, or holding the same text, leaves the text as it came back; a file changed meanwhile gets the bar, never
/// merged without a word.
pub(crate) fn check_recovered(ids: &[RecoveryId]) {
    for view in open_editors() {
        if ids.contains(&view.ivars().file.recovery) {
            on_disk::look(&view, on_disk::Look::Recovered);
        }
    }
}

/// The editor of the document `tab` in `controller`'s window.
pub(crate) fn editor_of_tab(controller: &WindowController, tab: TabId) -> Option<Retained<EditorTextView>> {
    editors_in(controller)
        .into_iter()
        .find(|view| view.ivars().tab.get() == Some(tab))
}

/// The editors of `controller`'s window.
fn editors_in(controller: &WindowController) -> Vec<Retained<EditorTextView>> {
    open_editors()
        .into_iter()
        .filter(|view| view.controller().is_some_and(|own| std::ptr::eq(&*own, controller)))
        .collect()
}
