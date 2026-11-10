//! The tab bar's "+" as Safari's back button works: a click opens the page of a new tab (E17), holding it opens the
//! menu of what a tab can show, whose entry opens in a new tab straight away. The "+" is AppKit's own and offers no
//! hold, so a monitor of the app's mouse presses takes the ones on it.

use std::cell::RefCell;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventType, NSView, NSWindow};
use objc2_foundation::{NSDate, NSObjectProtocol, NSPoint};

use super::{WINDOWS, WindowController, tabs};
use crate::util::{constant, guarded};

/// How long a press on the "+" lasts before its menu opens; a release before is a click.
const HOLD: f64 = 0.35;
/// How far the pointer may move during a click before it counts as dragging into the menu.
const SLOP: f64 = 4.0;

thread_local! {
    static MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
    /// The menu shown last, kept until the next: AppKit sends the chosen entry's action after the menu closed.
    static SHOWN: RefCell<Option<Retained<objc2_app_kit::NSMenu>>> = const { RefCell::new(None) };
}

/// Starts watching the presses on the "+" of the app's tab bars; once.
pub fn install() {
    if MONITOR.with(|monitor| monitor.borrow().is_some()) {
        return;
    }
    let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes a live event for the call.
        let taken = guarded(false, || pressed(unsafe { event.as_ref() }));
        if taken { std::ptr::null_mut() } else { event.as_ptr() }
    });
    // SAFETY: The block returns the event it was given, or none for a press it handled itself.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseDown, &block) };
    MONITOR.with(|slot| *slot.borrow_mut() = monitor);
}

/// Handles a press on the "+" of one of the app's tab bars; whether it was one.
fn pressed(event: &NSEvent) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(window) = event.window(mtm) else { return false };
    let point = event.locationInWindow();
    if point.y < window.contentLayoutRect().size.height {
        return false;
    }
    let Some(controller) = controller_of(&window) else {
        return false;
    };
    let Some(button) = plus_button(&window) else {
        return false;
    };
    let inside = button.convertPoint_fromView(point, None);
    if !objc2_foundation::NSPointInRect(inside, button.bounds()) {
        return false;
    }
    if released_within(mtm, point) {
        controller.open_new_tab_page();
    } else {
        show_menu(&controller, &button);
    }
    true
}

/// Whether the button comes up within `HOLD` of the press at `start`, without the pointer moving off.
pub(super) fn released_within(mtm: MainThreadMarker, start: NSPoint) -> bool {
    let app = NSApplication::sharedApplication(mtm);
    let until = NSDate::dateWithTimeIntervalSinceNow(HOLD);
    loop {
        let next = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            NSEventMask::LeftMouseUp | NSEventMask::LeftMouseDragged,
            Some(&until),
            constant!(objc2_app_kit::NSEventTrackingRunLoopMode),
            true,
        );
        let Some(event) = next else { return false };
        if event.r#type() == NSEventType::LeftMouseUp {
            return true;
        }
        let at = event.locationInWindow();
        if (at.x - start.x).abs() > SLOP || (at.y - start.y).abs() > SLOP {
            return false;
        }
    }
}

/// Opens the menu of what a tab can show below the "+", which the held press then drags through.
fn show_menu(controller: &WindowController, button: &NSView) {
    let menu = menu_for(controller);
    SHOWN.with(|shown| *shown.borrow_mut() = Some(menu.clone()));
    let below = NSPoint::new(0.0, button.bounds().size.height + 4.0);
    menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(button));
}

/// The menu the held "+" of `controller`'s window shows; its entries open in a new tab of that window, whichever is
/// key.
pub(super) fn menu_for(controller: &WindowController) -> Retained<objc2_app_kit::NSMenu> {
    let menu = tabs::new_tab_menu(controller.mtm());
    tabs::fill_new_tab_menu(controller, &menu);
    aim(&menu, controller);
    menu
}

fn aim(menu: &objc2_app_kit::NSMenu, controller: &WindowController) {
    for item in menu.itemArray().iter() {
        if let Some(submenu) = item.submenu() {
            aim(&submenu, controller);
        } else if item
            .action()
            .is_some_and(|action| controller.respondsToSelector(action))
        {
            crate::util::set_target(&item, controller);
        }
    }
}

fn controller_of(window: &NSWindow) -> Option<Retained<WindowController>> {
    WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .find(|controller| controller.tab_of_window(window).is_some())
            .cloned()
    })
}

/// The "+" at the end of `window`'s tab bar, AppKit's own button.
pub(super) fn plus_button(window: &NSWindow) -> Option<Retained<NSView>> {
    fn find(view: &NSView) -> Option<Retained<NSView>> {
        // Key-value observing puts a prefix before the class's name.
        if view.class().name().to_bytes().ends_with(b"NSTabBarNewTabButton") {
            return Some(objc2::Message::retain(view));
        }
        view.subviews().iter().find_map(|child| find(&child))
    }
    // SAFETY: Asking for the superview only reads; the frame view holds the content view while the window shows.
    let frame = window
        .contentView()
        .and_then(|content| unsafe { content.superview() })?;
    find(&frame)
}
