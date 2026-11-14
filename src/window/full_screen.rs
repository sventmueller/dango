//! The sidebar in full screen. AppKit then moves the toolbar into a window of its own and covers the sidebar's part of
//! it with a lighter background edged by a line, which outside full screen the sidebar shows through; clearing that
//! background lets the sidebar run up to the screen's top again.

use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplication, NSSplitView, NSSplitViewDidResizeSubviewsNotification, NSView, NSViewFrameDidChangeNotification,
    NSWindow, NSWindowDidMoveNotification, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSRect, NSRunLoop};

use crate::util::{constant, guarded};

/// How far two edges may lie apart and still count as one, for rounding in AppKit's layout.
const SLACK: f64 = 1.0;

/// Clears the toolbar's background above `sidebar` while `window` is in full screen, or brings it back with `clear`
/// false.
pub fn clear_above_sidebar(window: &NSWindow, sidebar: &NSView, clear: bool) {
    if clear && !window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return;
    }
    let column = window.convertRectToScreen(sidebar.convertRect_toView(sidebar.bounds(), None));
    if column.size.width < SLACK {
        return;
    }
    for (toolbar, frame) in toolbar_windows(window) {
        if let Some(background) = background_over(&frame, &toolbar, column) {
            background.setAlphaValue(if clear { 0.0 } else { 1.0 });
        }
    }
}

/// The window AppKit shows `window`'s toolbar in while it is in full screen, with its frame view.
fn toolbar_windows(window: &NSWindow) -> Vec<(Retained<NSWindow>, Retained<NSView>)> {
    let mtm = MainThreadMarker::from(window);
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .filter(|toolbar| is_toolbar_of(toolbar, window))
        .filter_map(|toolbar| {
            // SAFETY: Asking for the superview only reads; the frame view holds the content view while it shows.
            let frame = toolbar.contentView().and_then(|c| unsafe { c.superview() })?;
            Some((toolbar, frame))
        })
        .collect()
}

fn is_toolbar_of(toolbar: &NSWindow, window: &NSWindow) -> bool {
    // Key-value observing puts a prefix before the class's name.
    toolbar
        .class()
        .name()
        .to_bytes()
        .ends_with(b"NSToolbarFullScreenWindow")
        && same_columns(toolbar.frame(), window.frame())
}

/// How far the toolbar of `window` in full screen reaches down from the top of `sidebar`, none outside full screen:
/// down to where the safe area of `content`, the documents' column, begins, less a tab bar between them, and further
/// by as far as the toolbar slid down from the screen's top.
pub fn toolbar_reach(window: &NSWindow, sidebar: &NSView, content: &NSView) -> f64 {
    if !window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return 0.0;
    }
    let top = |view: &NSView, rect: NSRect| {
        let rect = view.convertRect_toView(rect, None);
        rect.origin.y + rect.size.height
    };
    let tab_bar = window
        .tabGroup()
        .filter(|group| group.isTabBarVisible())
        .and_then(|_| {
            let mut frames = toolbar_windows(window);
            // SAFETY: Asking for the superview only reads; the frame view holds the content view while it shows.
            if let Some(frame) = window.contentView().and_then(|c| unsafe { c.superview() }) {
                frames.push((objc2::Message::retain(window), frame));
            }
            frames.iter().find_map(|(_, frame)| tab_bar(frame))
        })
        .map_or(0.0, |bar| bar.bounds().size.height);
    let below = top(content, content.safeAreaLayoutGuide().frame()) + tab_bar;
    (top(sidebar, sidebar.bounds()) - below).max(0.0) + toolbar_slide(window)
}

/// How far the toolbar's window of `window` slid down from the window's top, as it does in full screen while the window
/// buttons or the menu bar show; the documents' column keeps its safe area meanwhile, and the toolbar covers its top.
pub fn toolbar_slide(window: &NSWindow) -> f64 {
    let frame = window.frame();
    toolbar_windows(window).first().map_or(0.0, |(toolbar, _)| {
        let toolbar = toolbar.frame();
        (frame.origin.y + frame.size.height - toolbar.origin.y - toolbar.size.height).max(0.0)
    })
}

fn tab_bar(view: &NSView) -> Option<Retained<NSView>> {
    if view.class().name().to_bytes().ends_with(b"NSTabBar") && !view.isHiddenOrHasHiddenAncestor() {
        return Some(objc2::Message::retain(view));
    }
    view.subviews().iter().find_map(|child| tab_bar(&child))
}

/// Clears the toolbar above `sidebar` again whenever `split` resizes it in full screen, where AppKit lays the toolbar's
/// parts out anew; the token ends the watch when handed to `NSNotificationCenter::removeObserver`.
pub fn follow_sidebar(
    window: &NSWindow,
    sidebar: &NSView,
    split: &NSSplitView,
) -> Retained<ProtocolObject<dyn NSObjectProtocol>> {
    let window = Weak::from(window);
    let sidebar = Weak::from(sidebar);
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        let (window, sidebar) = (window.clone(), sidebar.clone());
        let later = RcBlock::new(move || {
            guarded((), || {
                if let (Some(window), Some(sidebar)) = (window.load(), sidebar.load()) {
                    clear_above_sidebar(&window, &sidebar, true);
                }
            });
        });
        // After the toolbar followed the new widths, which AppKit lays out once this notification is through.
        // SAFETY: Split views post on the main thread, whose run loop then calls the block on the same thread.
        unsafe { NSRunLoop::currentRunLoop().performBlock(&later) };
    });
    let center = NSNotificationCenter::defaultCenter();
    let name = constant!(NSSplitViewDidResizeSubviewsNotification);
    // SAFETY: The block holds the window and the sidebar weakly and runs on the main thread, where split views post.
    unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), Some(split), None, &block) }
}

/// Calls `moved` whenever AppKit moves the toolbar's window of `window` in full screen; the token ends the watch when
/// handed to `NSNotificationCenter::removeObserver`.
pub fn follow_toolbar_window(
    window: &NSWindow,
    moved: impl Fn() + 'static,
) -> Retained<ProtocolObject<dyn NSObjectProtocol>> {
    let window = Weak::from(window);
    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        guarded((), || {
            // SAFETY: The center hands the block a notification that lives for the call.
            let object = unsafe { notification.as_ref() }.object();
            let toolbar = object.as_deref().and_then(|object| object.downcast_ref::<NSWindow>());
            if let (Some(toolbar), Some(window)) = (toolbar, window.load())
                && is_toolbar_of(toolbar, &window)
            {
                moved();
            }
        });
    });
    let center = NSNotificationCenter::defaultCenter();
    let name = constant!(NSWindowDidMoveNotification);
    // SAFETY: The block holds the window weakly and runs on the main thread, where windows post.
    unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block) }
}

/// Calls `moved` once AppKit laid out anew after `views` changed their frames; the tokens end the watch when handed to
/// `NSNotificationCenter::removeObserver`.
pub fn follow_frames(
    views: &[&NSView],
    moved: impl Fn() + 'static,
) -> Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>> {
    let moved = Rc::new(moved);
    let center = NSNotificationCenter::defaultCenter();
    views
        .iter()
        .map(|view| {
            view.setPostsFrameChangedNotifications(true);
            let moved = moved.clone();
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                let moved = moved.clone();
                let later = RcBlock::new(move || guarded((), || moved()));
                // SAFETY: Views post on the main thread, whose run loop then calls the block on the same thread.
                unsafe { NSRunLoop::currentRunLoop().performBlock(&later) };
            });
            let name = constant!(NSViewFrameDidChangeNotification);
            // SAFETY: The block runs on the main thread, where views post, and holds nothing the view owns.
            unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), Some(*view), None, &block) }
        })
        .collect()
}

/// The toolbar's background whose columns on screen are those of `column`.
fn background_over(view: &NSView, toolbar: &NSWindow, column: NSRect) -> Option<Retained<NSView>> {
    if view.class().name().to_bytes().ends_with(b"NSTitlebarBackgroundView") {
        let on_screen = toolbar.convertRectToScreen(view.convertRect_toView(view.bounds(), None));
        return same_columns(on_screen, column).then(|| objc2::Message::retain(view));
    }
    view.subviews()
        .iter()
        .find_map(|child| background_over(&child, toolbar, column))
}

fn same_columns(a: NSRect, b: NSRect) -> bool {
    (a.origin.x - b.origin.x).abs() < SLACK && (a.size.width - b.size.width).abs() < SLACK
}
