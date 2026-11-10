//! Editor and preview scroll together: the line at the top of one is at the top of the other, matched through
//! `splat_preview::map`, by paragraph.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSScrollView, NSViewBoundsDidChangeNotification};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRange};
use splat_markdown::text::Byte;
use splat_preview::map::{Shown, preview_offset};

use super::markdown::Preview;
use super::page::Views;
use crate::editor::EditorTextView;
use crate::util::{constant, guarded};

#[derive(Default)]
pub struct State {
    /// The source offset at the top of the preview, which a new text keeps in place when the editor is not on screen.
    top: Cell<Option<usize>>,
    /// Set while one side scrolls the other, so the other's answer does not scroll back.
    moving: Cell<bool>,
    observers: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
    /// The editor whose scrolling the preview follows.
    follows: RefCell<Option<objc2::rc::Weak<EditorTextView>>>,
}

/// Lets the preview and `editor` follow each other's scrolling.
pub fn attach(preview: &Rc<Preview>, editor: &EditorTextView) {
    let state = &preview.sync;
    if state
        .follows
        .borrow()
        .as_ref()
        .and_then(objc2::rc::Weak::load)
        .is_some_and(|followed| std::ptr::eq(&*followed, editor))
    {
        return;
    }
    stop(preview);
    *state.follows.borrow_mut() = Some(objc2::rc::Weak::from(editor));
    let key = preview.key;
    if let Some(scroll) = editor.enclosingScrollView() {
        observe(preview, &scroll, move || super::with(key, |p| editor_scrolled(p)));
    }
    observe(preview, &preview.views.scroll, move || {
        super::with(key, |p| preview_scrolled(p))
    });
}

fn observe(preview: &Preview, scroll: &NSScrollView, scrolled: impl Fn() + 'static) {
    let clip = scroll.contentView();
    clip.setPostsBoundsChangedNotifications(true);
    let block = RcBlock::new(move |_: NonNull<NSNotification>| guarded((), &scrolled));
    let name = constant!(NSViewBoundsDidChangeNotification);
    // SAFETY: The block only looks up the preview by its number and does nothing once it is gone; the observer is
    // removed when the preview closes.
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(name),
            Some(&clip),
            None,
            &block,
        )
    };
    preview.sync.observers.borrow_mut().push(observer);
}

/// Ends the following, as the preview closes or follows another editor.
pub fn stop(preview: &Preview) {
    let center = NSNotificationCenter::defaultCenter();
    for observer in preview.sync.observers.borrow_mut().drain(..) {
        // SAFETY: An observer this center handed out.
        unsafe { center.removeObserver(observer.as_ref()) };
    }
    *preview.sync.follows.borrow_mut() = None;
}

/// Runs `change` of the preview's text or place without the preview moving the editor.
pub fn while_moving(preview: &Preview, change: impl FnOnce()) {
    let was = preview.sync.moving.replace(true);
    change();
    preview.sync.moving.set(was);
}

/// After a new text: the preview stands where the editor stands, or where it stood itself.
pub fn keep_place(preview: &Preview) {
    match visible_editor(preview) {
        Some(_) => editor_scrolled(preview),
        None => {
            if let Some(top) = preview.sync.top.get() {
                scroll_preview_to(preview, top);
            }
        }
    }
}

fn visible_editor(preview: &Preview) -> Option<Retained<EditorTextView>> {
    preview
        .sync
        .follows
        .borrow()
        .as_ref()
        .and_then(objc2::rc::Weak::load)
        .filter(|editor| editor.window().is_some() && !editor.isHiddenOrHasHiddenAncestor())
}

fn editor_scrolled(preview: &Preview) {
    if preview.sync.moving.get() {
        return;
    }
    let Some(editor) = visible_editor(preview) else { return };
    let Some(byte) = crate::editor::top_byte(&editor) else {
        return;
    };
    scroll_preview_to(preview, byte);
}

fn scroll_preview_to(preview: &Preview, byte: usize) {
    let y = {
        let shown = preview.shown.borrow();
        let Some(laid) = shown.layout.as_ref() else { return };
        if laid.blocks.is_empty() || !shown.index.fits(laid) {
            return;
        }
        let at = preview_offset(laid, &shown.index, Byte::new(byte));
        y_of(&preview.views, at.get())
    };
    let Some(y) = y else { return };
    preview.sync.top.set(Some(byte));
    while_moving(preview, || {
        let text = &preview.views.text;
        // The top inset stays above the first line.
        let y = if y <= super::page::INSET.height + 1.0 { 0.0 } else { y };
        text.scrollPoint(NSPoint::new(0.0, y));
    });
}

fn preview_scrolled(preview: &Preview) {
    if preview.sync.moving.get() {
        return;
    }
    let Some(byte) = top_of_preview(preview) else { return };
    preview.sync.top.set(Some(byte));
    if let Some(editor) = visible_editor(preview) {
        while_moving(preview, || crate::editor::scroll_to_byte(&editor, byte));
    }
}

/// The source offset of the paragraph at the top of the preview.
fn top_of_preview(preview: &Preview) -> Option<usize> {
    let at = char_at_top(&preview.views)?;
    let shown = preview.shown.borrow();
    let laid = shown.layout.as_ref()?;
    if !shown.index.fits(laid) {
        return None;
    }
    let i = shown.index.para_at(Shown::new(at))?;
    let place = shown.index.paras.get(i)?;
    let block = laid.blocks.get(place.block)?;
    Some(
        block
            .paras
            .get(place.para)
            .map_or(block.src.start, |para| para.src.start)
            .get(),
    )
}

/// The top of the line holding character `at` of the preview, in its text view's coordinates.
fn y_of(views: &Views, at: usize) -> Option<f64> {
    let length = views.storage.length();
    if length == 0 {
        return None;
    }
    let at = at.min(length - 1);
    // SAFETY: A range inside the text, and no pointer for the actual range, which the method allows.
    let glyphs = unsafe {
        views
            .layout
            .glyphRangeForCharacterRange_actualCharacterRange(NSRange::new(at, 1), std::ptr::null_mut())
    };
    let rect = views
        .layout
        .boundingRectForGlyphRange_inTextContainer(glyphs, &views.container);
    Some(rect.origin.y + views.text.textContainerOrigin().y)
}

/// The character at the top of what the preview shows.
pub(super) fn char_at_top(views: &Views) -> Option<usize> {
    if views.storage.length() == 0 {
        return None;
    }
    let visible = views.text.visibleRect();
    let origin = views.text.textContainerOrigin();
    let point = NSPoint::new(1.0, (visible.origin.y - origin.y).max(0.0) + 1.0);
    let glyph = views.layout.glyphIndexForPoint_inTextContainer(point, &views.container);
    Some(views.layout.characterIndexForGlyphAtIndex(glyph))
}
