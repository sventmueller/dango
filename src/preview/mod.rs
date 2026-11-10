//! Documents as they read, beside their editor (E1, Phase 9): Markdown as native text with the formulas, diagrams and
//! pictures of splat's render path (docs/splat-render.md), PDF files through PDFKit, HTML files in the browser tab.

mod markdown;
mod page;
mod pdf;
mod sync;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::sel;
use objc2_foundation::NSURL;

use crate::editor::EditorTextView;
use crate::util::{main_after, ns};
use crate::window::{Side, TabKind, WindowController};
use markdown::Preview;
pub use pdf::{is_pdf, open as open_pdf};

thread_local! {
    /// The open Markdown previews of all windows.
    static OPEN: RefCell<Vec<Rc<Preview>>> = const { RefCell::new(Vec::new()) };
    static NEXT_KEY: Cell<u64> = const { Cell::new(1) };
    static LOOKING: Cell<bool> = const { Cell::new(false) };
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

/// Whether the menu offers a preview of the document `view` shows.
pub fn validate(view: &EditorTextView, action: Option<Sel>) -> Option<bool> {
    (action == Some(sel!(ideShowPreview:))).then(|| {
        view.document()
            .path()
            .is_some_and(|path| is_markdown(path) || is_html(path))
    })
}

/// Opens the preview of the document `view` shows in a panel to its right, or closes the Markdown preview it has:
/// Markdown rendered, an HTML file in a browser tab. The keyboard focus stays in the editor.
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
    controller.close_tab(preview.tab());
    true
}

fn shown_for(view: &EditorTextView, controller: &WindowController) -> Option<Rc<Preview>> {
    find(|p| p.follows(view) && p.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

fn open_markdown(
    controller: &WindowController,
    view: &EditorTextView,
    path: &Path,
    editor_tab: isize,
    automatic: bool,
) {
    let key = NEXT_KEY.with(|n| n.replace(n.get() + 1));
    let preview = Preview::new(controller, view, path, key, automatic);
    let title = splat_i18n::tf("Vorschau: {}", &[&crate::editor::display_name(path)]);
    let page: Retained<objc2_app_kit::NSView> = Retained::into_super(preview.views.page.clone());
    let id = controller.add_tab(TabKind::Preview, None, &title, page);
    preview.set_tab(id);
    OPEN.with(|open| open.borrow_mut().push(preview.clone()));
    beside(controller, id, editor_tab);
    sync::attach(&preview, view);
    preview.lay_out();
    look_at_images_later();
}

/// Opens an HTML file in a browser tab beside the editor; the browser shows only files of the window's folder.
fn open_html(controller: &WindowController, path: &Path, editor_tab: isize) {
    let Some(url) = NSURL::fileURLWithPath(&ns(&path.to_string_lossy())).absoluteString() else {
        return;
    };
    let before = controller.tab_ids();
    crate::browser::open_tab(controller, Some(&url.to_string()));
    if let Some(id) = controller.tab_ids().into_iter().find(|id| !before.contains(id)) {
        beside(controller, id, editor_tab);
    }
}

/// Moves the new tab `id` into a panel right of the editor's, whose tab stays in front.
fn beside(controller: &WindowController, id: isize, editor_tab: isize) {
    controller.split_tab(id, Side::Right);
    controller.select_tab_by_id(editor_tab);
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
pub fn closing(controller: &WindowController, id: isize) {
    let closed: Vec<Rc<Preview>> = OPEN.with(|open| {
        let mut open = open.borrow_mut();
        let (closed, kept) = open
            .drain(..)
            .partition(|p| p.tab() == id && p.controller().is_some_and(|c| std::ptr::eq(&*c, controller)));
        *open = kept;
        closed
    });
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

/// Runs `f` with the open preview `key`, if there is one.
fn with(key: u64, f: impl FnOnce(&Rc<Preview>)) {
    if let Some(preview) = find(|p| p.key == key) {
        f(&preview);
    }
}

fn find(matches: impl Fn(&Preview) -> bool) -> Option<Rc<Preview>> {
    OPEN.with(|open| open.borrow().iter().find(|p| matches(p)).cloned())
}

/// The open previews; those whose window closed leave on the way.
fn all() -> Vec<Rc<Preview>> {
    let gone: Vec<Rc<Preview>> = OPEN.with(|open| {
        let mut open = open.borrow_mut();
        let (gone, kept) = open.drain(..).partition(|p| p.is_closed());
        *open = kept;
        gone
    });
    for preview in gone {
        preview.close();
    }
    OPEN.with(|open| open.borrow().clone())
}

/// Compares the picture files of the open previews with what they show every two seconds, while any is open.
fn look_at_images_later() {
    if LOOKING.replace(true) {
        return;
    }
    main_after(LOOK_AT_IMAGES, || {
        LOOKING.set(false);
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

    /// The visible top of the preview of `path`, in its view's coordinates.
    pub fn scrolled(path: &Path) -> Option<f64> {
        Some(find(|p| p.path() == path)?.views.text.visibleRect().origin.y)
    }

    pub fn page_count(view: &objc2_app_kit::NSView) -> usize {
        super::pdf::page_count(view)
    }
}
