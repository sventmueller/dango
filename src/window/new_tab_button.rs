//! The tab bar's "+": a press, short or held, opens the menu of what a tab can show below it, whose entry opens in a
//! new tab straight away (E79); ⌘T opens the same menu. The "+" is AppKit's own and would open an empty window tab,
//! so a monitor of the app's mouse presses takes the ones on it.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSEventMask, NSMenu, NSView, NSWindow};
use objc2_foundation::{NSObjectProtocol, NSPoint};

use super::{WindowController, tabs};
use crate::util::guarded;

thread_local! {
    static MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

/// The menu of the "+" in the tab bar shown last, and what the self-test reads of it.
#[derive(Default)]
pub(crate) struct NewTabMenu {
    /// The menu shown last, kept until the next: AppKit sends the chosen entry's action after the menu closed.
    shown: RefCell<Option<Retained<NSMenu>>>,
    /// Whether the menu is only recorded instead of shown, so a self-test neither waits on it nor takes the focus.
    record_only: Cell<bool>,
    /// Whether the menu shown last stood below the "+", for the self-test.
    below_plus: Cell<Option<bool>>,
}

/// Starts watching the presses on the "+" of the app's tab bars; once.
pub(crate) fn install() {
    if MONITOR.with(|monitor| monitor.borrow().is_some()) {
        return;
    }
    let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        guarded(event.as_ptr(), || {
            // SAFETY: AppKit passes a live event for the call.
            let taken = pressed(unsafe { event.as_ref() });
            if taken { std::ptr::null_mut() } else { event.as_ptr() }
        })
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
    let Some(controller) = controller_of(&crate::context::AppContext::get(mtm), &window) else {
        return false;
    };
    let Some(button) = plus_button(&window) else {
        return false;
    };
    let inside = button.convertPoint_fromView(point, None);
    if !objc2_foundation::NSPointInRect(inside, button.bounds()) {
        return false;
    }
    show_menu(&controller, &button, true);
    true
}

/// Opens the menu of what a tab can show below the "+" of the active pane of a split window (E145), else below the
/// "+" of `controller`'s window in front, or at the top of its documents when the tab bar has no "+".
pub(super) fn open(controller: &WindowController) {
    let Some(window) = controller.window() else { return };
    let pane_plus = controller.current_tab().and_then(|tab| tab.panes.active_plus());
    if let Some(button) = pane_plus {
        show_menu(controller, &button, true);
    } else if let Some(button) = plus_button(&window) {
        show_menu(controller, &button, true);
    } else if let Some(content) = window.contentView() {
        show_menu(controller, &content, false);
    }
}

/// Opens the menu below `button`, a pane's "+" (E145), whose entries open in the active pane.
pub(super) fn open_below(controller: &WindowController, button: &NSView) {
    show_menu(controller, button, true);
}

/// Opens the menu below `view`; a press still held drags through it, a click leaves it open.
fn show_menu(controller: &WindowController, view: &NSView, below_plus: bool) {
    let menu = menu_for(controller);
    let menu_state = controller.context().new_tab_menu();
    *menu_state.shown.borrow_mut() = Some(menu.clone());
    menu_state.below_plus.set(Some(below_plus));
    if menu_state.record_only.get() {
        return;
    }
    let size = view.bounds().size;
    let below = match (below_plus, view.isFlipped()) {
        (true, _) => NSPoint::new(0.0, size.height + 4.0),
        (false, true) => NSPoint::new(size.width / 2.0, 0.0),
        (false, false) => NSPoint::new(size.width / 2.0, size.height),
    };
    menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(view));
}

/// The menu the "+" of `controller`'s window shows; its entries open in a new tab of that window, whichever is key.
pub(super) fn menu_for(controller: &WindowController) -> Retained<NSMenu> {
    let choices = super::agent_choices::now(controller.context());
    let menu = tabs::new_tab_menu(controller.mtm(), &choices);
    tabs::fill_new_tab_menu(controller, &menu);
    aim(&menu, controller);
    super::agent_choices::refresh(controller.context());
    menu
}

fn aim(menu: &NSMenu, controller: &WindowController) {
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

/// The controller of the open window that has `window` among its tabs.
fn controller_of(cx: &crate::context::AppContext, window: &NSWindow) -> Option<Retained<WindowController>> {
    cx.windows()
        .find(|controller| controller.tab_of_window(window).is_some())
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

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
