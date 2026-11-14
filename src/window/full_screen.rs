//! The sidebar in full screen. AppKit then moves the toolbar into a window of its own and covers the sidebar's part of
//! it with a lighter background edged by a line, which outside full screen the sidebar shows through; clearing that
//! background lets the sidebar run up to the screen's top again.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplication, NSSplitView, NSSplitViewDidResizeSubviewsNotification, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSRect, NSRunLoop};

use crate::util::{constant, guarded};

/// How far two edges may lie apart and still count as one, for rounding in AppKit's layout.
const SLACK: f64 = 1.0;

/// Clears the toolbar's background above `sidebar` while `window` is in full screen, or brings it back with `clear`
/// false; answers how far the toolbar then reaches down over the sidebar's top, none outside full screen.
pub fn clear_above_sidebar(window: &NSWindow, sidebar: &NSView, clear: bool) -> f64 {
    if clear && !window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return 0.0;
    }
    let column = window.convertRectToScreen(sidebar.convertRect_toView(sidebar.bounds(), None));
    if column.size.width < SLACK {
        return 0.0;
    }
    let mut reach = 0.0;
    let mtm = MainThreadMarker::from(window);
    for toolbar in NSApplication::sharedApplication(mtm).windows().iter() {
        // Key-value observing puts a prefix before the class's name.
        if !toolbar
            .class()
            .name()
            .to_bytes()
            .ends_with(b"NSToolbarFullScreenWindow")
            || !same_columns(toolbar.frame(), window.frame())
        {
            continue;
        }
        // SAFETY: Asking for the superview only reads; the frame view holds the content view while the window shows.
        let Some(frame) = toolbar.contentView().and_then(|c| unsafe { c.superview() }) else {
            continue;
        };
        if let Some(background) = background_over(&frame, &toolbar, column) {
            background.setAlphaValue(if clear { 0.0 } else { 1.0 });
            let below = toolbar.convertRectToScreen(background.convertRect_toView(background.bounds(), None));
            reach = (column.origin.y + column.size.height - below.origin.y).max(0.0);
        }
    }
    if clear { reach } else { 0.0 }
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
