//! Documents as they read, beside their editor (E1, Phase 9): Markdown as native text with the formulas, diagrams and
//! pictures of splat's render path (docs/splat-render.md), PDF files through PDFKit, pictures, media and office
//! documents through Quick Look, HTML files in the browser tab.

mod markdown;
mod page;
mod pdf;
mod quick_look;
mod sync;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::sel;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_foundation::NSURL;

use crate::editor::EditorTextView;
use crate::util::{main_after, ns};
use crate::window::{TabId, TabKind, WindowController};
use markdown::Preview;

/// The open Markdown previews of all windows.
pub struct Previews {
    open: RefCell<Vec<Rc<Preview>>>,
    next_key: Cell<u64>,
    looking: Cell<bool>,
}

impl Default for Previews {
    fn default() -> Self {
        Previews {
            open: RefCell::default(),
            next_key: Cell::new(1),
            looking: Cell::default(),
        }
    }
}

/// How often the picture files of the open previews are compared with what they show, as in splat.
const LOOK_AT_IMAGES: Duration = Duration::from_secs(2);

/// Tells the render path where its page lies and where failures go, before any window opens.
pub fn start(mtm: MainThreadMarker) {
    splat_render::start(splat_render::Host::of_main_bundle(|what, detail| {
        crate::diag::report(what, detail);
    }));
    splat_render::images::watch_memory(mtm);
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["md", "markdown", "mdown", "mkd", "mkdn"].contains(&e.to_ascii_lowercase().as_str()))
}

pub fn is_html(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["html", "htm", "xhtml"].contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether `path` opens in a tab that shows it rather than in an editor.
pub fn is_document(path: &Path) -> bool {
    pdf::is_pdf(path) || quick_look::is_quick_look(path)
}

/// Opens `path` in a tab that shows it, or in the preview tab, or selects the tab that shows it; `false` when Quick
/// Look is missing and an editor should take the file.
pub fn open_document(controller: &WindowController, path: &Path, preview: bool) -> bool {
    let key = path.to_string_lossy().into_owned();
    if select_shown(controller, &key, preview) {
        return true;
    }
    let name = crate::editor::display_name(path);
    let view = if pdf::is_pdf(path) {
        let Some(view) = pdf::view(controller.mtm(), path, &name) else {
            crate::editor::show_alert(
                controller.mtm(),
                &splat_i18n::tf("„{}“ lässt sich nicht öffnen.", &[&name]),
                splat_i18n::t("Die Datei ist kein lesbares PDF."),
            );
            return true;
        };
        view
    } else {
        let Some(view) = quick_look::view(controller.mtm(), path, &name) else {
            return false;
        };
        view
    };
    if preview {
        controller.add_preview_tab(TabKind::Preview, Some(key), &name, view);
    } else {
        controller.add_tab(TabKind::Preview, Some(key), &name, view);
    }
    true
}

/// Opens the drawing of the SVG file `path` in a tab of its own, apart from an editor of its source, or selects it.
pub fn open_drawing(controller: &WindowController, path: &Path) {
    let key = format!("preview:{}", path.display());
    if select_shown(controller, &key, false) {
        return;
    }
    let name = crate::editor::display_name(path);
    if let Some(view) = quick_look::view(controller.mtm(), path, &name) {
        let title = splat_i18n::tf("Vorschau: {}", &[&name]);
        controller.add_tab(TabKind::Preview, Some(key), &title, view);
    }
}

/// Brings the tab that shows `key` to the front, and keeps it unless `preview`; whether there is one.
fn select_shown(controller: &WindowController, key: &str, preview: bool) -> bool {
    let Some(id) = controller.tab_with_key(key) else {
        return false;
    };
    if !preview {
        controller.keep_tab(id);
    }
    controller.select_tab_by_id(id);
    true
}

/// Whether the menu offers a preview of the document `view` shows.
pub fn validate(view: &EditorTextView, action: Option<Sel>) -> Option<bool> {
    (action == Some(sel!(ideShowPreview:))).then(|| {
        view.document()
            .path()
            .is_some_and(|path| is_markdown(path) || is_html(path))
    })
}

/// Opens the preview of the document `view` shows beside it in its tab, or closes the Markdown preview it has:
/// Markdown rendered, an HTML file in a browser. The keyboard focus stays in the editor.
pub fn toggle_beside(view: &EditorTextView) {
    if !close_beside(view, false) {
        open_beside(view, false);
    }
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(view));
    }
}

/// Opens the preview of the document `view` shows beside it unless one is open; `automatic` for one that opened with
/// the document and closes with its editor.
pub fn open_beside(view: &EditorTextView, automatic: bool) {
    let Some(controller) = view.controller() else { return };
    let Some(path) = view.document().path().map(Path::to_path_buf) else {
        return;
    };
    let editor_tab = crate::editor::tab_of(view);
    if is_html(&path) {
        open_html(&controller, &path, editor_tab);
    } else if is_markdown(&path) && shown_for(view, &controller).is_none() {
        open_markdown(&controller, view, &path, editor_tab, automatic);
    }
}

/// Closes the Markdown preview that follows `view`, or with `automatic` only one that opened by itself; whether one
/// closed.
pub fn close_beside(view: &EditorTextView, automatic: bool) -> bool {
    let Some(controller) = view.controller() else {
        return false;
    };
    let Some(preview) = shown_for(view, &controller).filter(|p| !automatic || p.automatic()) else {
        return false;
    };
    if let Some(tab) = preview.tab() {
        controller.close_tab(tab);
    }
    true
}

fn shown_for(view: &EditorTextView, controller: &WindowController) -> Option<Rc<Preview>> {
    find(|p| p.follows(view) && p.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

fn open_markdown(
    controller: &WindowController,
    view: &EditorTextView,
    path: &Path,
    editor_tab: Option<TabId>,
    automatic: bool,
) {
    let previews = &controller.context().previews();
    let key = previews.next_key.replace(previews.next_key.get() + 1);
    let preview = Preview::new(controller, view, path, key, automatic);
    let title = splat_i18n::tf("Vorschau: {}", &[&crate::editor::display_name(path)]);
    let page: Retained<objc2_app_kit::NSView> = Retained::into_super(preview.views.page.clone());
    let mut id = None;
    beside(controller, editor_tab, || {
        id = Some(controller.add_tab(TabKind::Preview, None, &title, page))
    });
    if let Some(id) = id {
        preview.set_tab(id);
    }
    previews.open.borrow_mut().push(preview.clone());
    sync::attach(&preview, view);
    preview.lay_out();
    look_at_images_later();
}

/// Opens an HTML file in a browser beside the editor; the browser shows only files of the window's folder.
fn open_html(controller: &WindowController, path: &Path, editor_tab: Option<TabId>) {
    let Some(url) = NSURL::fileURLWithPath(&ns(&path.to_string_lossy())).absoluteString() else {
        return;
    };
    beside(controller, editor_tab, || {
        crate::browser::open_tab(controller, Some(&url.to_string()))
    });
}

/// Runs `open` beside the editor's tab, or where documents open by default for an editor not in a tab yet.
fn beside(controller: &WindowController, editor_tab: Option<TabId>, open: impl FnOnce()) {
    match editor_tab {
        Some(tab) => controller.open_beside(tab, open),
        None => open(),
    }
}

/// The text of `view` changed: the previews that follow it lay it out again.
pub fn edited(view: &EditorTextView) {
    for preview in all() {
        if preview.follows(view) {
            preview.follow(view);
            sync::attach(&preview, view);
            preview.text_changed();
        }
    }
}

/// The tab `id` of `controller` is about to close.
pub fn closing(controller: &WindowController, id: TabId) {
    let closed: Vec<Rc<Preview>> = {
        let mut open = controller.context().previews().open.borrow_mut();
        let (closed, kept) = open
            .drain(..)
            .partition(|p| p.tab() == Some(id) && p.controller().is_some_and(|c| std::ptr::eq(&*c, controller)));
        *open = kept;
        closed
    };
    for preview in closed {
        preview.close();
    }
}

/// The user asked for the pictures from the web of the preview `key`.
fn allow_web_pictures(key: u64) {
    with(key, |preview| preview.allow_web_pictures());
}

fn appearance_changed(key: u64) {
    with(key, |preview| preview.lay_out());
}

fn width_changed(key: u64) {
    with(key, |preview| preview.render_soon());
}

fn drag_ended(key: u64) {
    with(key, |preview| preview.views.page.drag_ended());
}

/// Runs `f` with the open preview `key`, if there is one.
fn with(key: u64, f: impl FnOnce(&Rc<Preview>)) {
    if let Some(preview) = find(|p| p.key == key) {
        f(&preview);
    }
}

fn find(matches: impl Fn(&Preview) -> bool) -> Option<Rc<Preview>> {
    crate::context::app()
        .previews()
        .open
        .borrow()
        .iter()
        .find(|p| matches(p))
        .cloned()
}

/// The open previews; those whose window closed leave on the way.
fn all() -> Vec<Rc<Preview>> {
    let cx = crate::context::app();
    let gone: Vec<Rc<Preview>> = {
        let mut open = cx.previews().open.borrow_mut();
        let (gone, kept) = open.drain(..).partition(|p| p.is_closed());
        *open = kept;
        gone
    };
    for preview in gone {
        preview.close();
    }
    cx.previews().open.borrow().clone()
}

/// Compares the picture files of the open previews with what they show every two seconds, while any is open.
fn look_at_images_later() {
    if crate::context::app().previews().looking.replace(true) {
        return;
    }
    main_after(LOOK_AT_IMAGES, || {
        crate::context::app().previews().looking.set(false);
        let open = all();
        if open.is_empty() {
            return;
        }
        splat_render::images::look_at_images(open.iter().map(|p| (p.viewer(), p.images())));
        look_at_images_later();
    });
}

/// What the self-test reads of the previews.
#[cfg(feature = "selftest")]
pub mod seen {
    use super::*;

    /// The text the Markdown preview of `path` shows, its attachments as U+FFFC, and how many attachments it has.
    pub fn text(path: &Path) -> Option<(String, usize)> {
        let preview = find(|p| p.path() == path)?;
        let storage = &preview.views.storage;
        let text = storage.string().to_string();
        let mut attachments = 0;
        let key = crate::util::constant!(objc2_app_kit::NSAttachmentAttributeName);
        for (at, _) in text.char_indices().filter(|(_, c)| *c == '\u{FFFC}') {
            let utf16 = text[..at].encode_utf16().count();
            // SAFETY: An index inside the storage and AppKit's attachment key.
            let value = unsafe { storage.attribute_atIndex_effectiveRange(key, utf16, std::ptr::null_mut()) };
            attachments += usize::from(value.is_some());
        }
        Some((text, attachments))
    }

    /// Whether the preview of `path` offers to load the pictures from the web.
    pub fn offers_web_pictures(path: &Path) -> Option<bool> {
        Some(find(|p| p.path() == path)?.views.offers_web_pictures())
    }

    /// Presses the button of the offer.
    pub fn load_web_pictures(path: &Path) {
        if let Some(preview) = find(|p| p.path() == path) {
            allow_web_pictures(preview.key);
        }
    }

    /// The addresses from the web the preview of `path` reports to the render path as shown.
    pub fn web_pictures_shown(path: &Path) -> Option<Vec<String>> {
        let preview = find(|p| p.path() == path)?;
        Some(splat_render::Viewer::web_images_shown(&*preview.viewer()))
    }

    /// The line of text at the top of what the preview of `path` shows.
    pub fn top_line(path: &Path) -> Option<String> {
        let preview = find(|p| p.path() == path)?;
        let at = sync::char_at_top(&preview.views)?;
        let units: Vec<u16> = preview.views.storage.string().to_string().encode_utf16().collect();
        let start = units[..at.min(units.len())]
            .iter()
            .rposition(|u| *u == u16::from(b'\n'))
            .map_or(0, |n| n + 1);
        let end = units[start..]
            .iter()
            .position(|u| *u == u16::from(b'\n'))
            .map_or(units.len(), |n| start + n);
        Some(String::from_utf16_lossy(&units[start..end]))
    }

    /// Scrolls the preview of `path` so that `y` of its view is at the top.
    pub fn scroll_to(path: &Path, y: f64) {
        if let Some(preview) = find(|p| p.path() == path) {
            preview.views.text.scrollPoint(objc2_foundation::NSPoint::new(0.0, y));
        }
    }

    /// Lets the preview of `path` and its editor follow each other's scrolling now instead of on the run loop's next
    /// round.
    pub fn settle(path: &Path) {
        if let Some(preview) = find(|p| p.path() == path) {
            sync::follow_due(&preview);
        }
    }

    /// The visible top of the preview of `path`, in its view's coordinates.
    pub fn scrolled(path: &Path) -> Option<f64> {
        Some(find(|p| p.path() == path)?.views.text.visibleRect().origin.y)
    }

    pub fn page_count(view: &objc2_app_kit::NSView) -> usize {
        super::pdf::page_count(view)
    }

    pub fn is_quick_look(view: &objc2_app_kit::NSView) -> bool {
        super::quick_look::shows(view)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn markdown_and_html_are_known_by_extension_in_any_case() {
        assert!(is_markdown(Path::new("README.MD")));
        assert!(is_markdown(Path::new("notes.mkdn")));
        assert!(!is_markdown(Path::new("md")));
        assert!(is_html(Path::new("page.XHTML")));
        assert!(!is_html(Path::new("page.html.txt")));
    }

    #[test]
    fn documents_open_in_a_viewer_but_source_files_in_an_editor() {
        for shown in ["a.pdf", "b.PNG", "c.docx", "d.mp4"] {
            assert!(is_document(Path::new(shown)), "{shown}");
        }
        // Quick Look would take `.ts` as video and `.key` as Keynote.
        for edited in ["main.ts", "server.key", "a.md", "Makefile"] {
            assert!(!is_document(Path::new(edited)), "{edited}");
        }
    }
}
