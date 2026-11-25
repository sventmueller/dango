//! Keeps a selftest app on the display the runner made for it and on a pasteboard of its own, so that scenarios run
//! beside each other and beside the user's work without meeting (see "Selftest" in docs/development.md).

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, msg_send, sel};
use objc2_app_kit::{
    NSModalResponse, NSModalResponseCancel, NSPasteboard, NSSavePanel, NSScreen, NSWindow,
    NSWindowDidChangeScreenNotification, NSWindowDidMoveNotification,
};
use objc2_foundation::{NSArray, NSNotification, NSNotificationCenter, NSNumber, NSPoint, NSRect};

use crate::util::{guarded, ns};

/// The variable in which the runner names the display of the run by its number.
const DISPLAY: &str = "IDE_SELFTEST_DISPLAY";

/// The number of the run's display, if the runner made one.
fn display() -> Option<u32> {
    std::env::var(DISPLAY).ok()?.parse().ok()
}

/// Gives the run its pasteboard unless `system_pasteboard`, and with a display of its own makes that display the only
/// one AppKit and the app see.
pub(super) fn install(system_pasteboard: bool) {
    if !system_pasteboard && let Err(why) = own_pasteboard() {
        super::check("the run keeps its own pasteboard", false, why);
    }
    if display().is_some() {
        if let Err(why) = own_screen() {
            super::check("the run keeps to its own display", false, why);
        }
        bring_back_moved_windows();
    }
}

/// Moves a window back onto the run's display whenever it moved off it after it showed: AppKit places some panels,
/// the open panel among them, without asking `NSScreen`.
fn bring_back_moved_windows() {
    let block = RcBlock::new(|note: NonNull<NSNotification>| {
        guarded((), || {
            // SAFETY: The center passes a notification that lives for the call.
            let note = unsafe { note.as_ref() };
            if let Some(window) = note.object().as_deref().and_then(AnyObject::downcast_ref::<NSWindow>) {
                onto_own_screen(window, window.mtm());
            }
        });
    });
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: AppKit's notification names are constants set before its code runs.
    for name in unsafe { [NSWindowDidMoveNotification, NSWindowDidChangeScreenNotification] } {
        // SAFETY: AppKit posts these notifications on the main thread, which the window they name belongs to, and the
        // block keeps no reference to it.
        let observer = unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block) };
        std::mem::forget(observer);
    }
}

/// The name of the pasteboard of the run: one per display, since a display runs one scenario at a time.
fn pasteboard_name() -> String {
    let display = display().map_or_else(|| "main".to_string(), |number| number.to_string());
    format!("{}.selftest.{display}", ide_product::BUNDLE_ID)
}

fn own_pasteboard() -> Result<(), String> {
    let general: extern "C-unwind" fn(&AnyClass, Sel) -> *mut NSPasteboard = general_pasteboard;
    // SAFETY: The function takes the class and selector of a class method without arguments and returns an
    // autoreleased pasteboard, as `+generalPasteboard` does.
    let general =
        unsafe { std::mem::transmute::<extern "C-unwind" fn(&AnyClass, Sel) -> *mut NSPasteboard, Imp>(general) };
    replace(
        NSPasteboard::class().metaclass(),
        sel!(generalPasteboard),
        None,
        general,
    )?;
    NSPasteboard::generalPasteboard().clearContents();
    Ok(())
}

extern "C-unwind" fn general_pasteboard(_class: &AnyClass, _cmd: Sel) -> *mut NSPasteboard {
    guarded(std::ptr::null_mut(), || {
        Retained::autorelease_return(NSPasteboard::pasteboardWithName(&ns(&pasteboard_name())))
    })
}

fn own_screen() -> Result<(), String> {
    let screens: extern "C-unwind" fn(&AnyClass, Sel) -> *mut NSArray<NSScreen> = screens;
    let main: extern "C-unwind" fn(&AnyClass, Sel) -> *mut NSScreen = main_screen;
    // SAFETY: Both functions take the class and selector of a class method without arguments; `screens` returns an
    // autoreleased array of screens as `+screens` does, `main_screen` an autoreleased screen or nil as `+mainScreen`
    // and `+deepestScreen` do.
    let (screens, main) = unsafe {
        (
            std::mem::transmute::<extern "C-unwind" fn(&AnyClass, Sel) -> *mut NSArray<NSScreen>, Imp>(screens),
            std::mem::transmute::<extern "C-unwind" fn(&AnyClass, Sel) -> *mut NSScreen, Imp>(main),
        )
    };
    let meta = NSScreen::class().metaclass();
    replace(meta, sel!(screens), Some(sel!(ideSelftestScreens)), screens)?;
    replace(meta, sel!(mainScreen), None, main)?;
    replace(meta, sel!(deepestScreen), None, main)?;
    let run_modal: extern "C-unwind" fn(&NSSavePanel, Sel) -> NSModalResponse = run_modal;
    // SAFETY: The function takes the panel and selector of `-runModal`, which has no arguments, and returns the
    // response the kept implementation returns.
    let run_modal =
        unsafe { std::mem::transmute::<extern "C-unwind" fn(&NSSavePanel, Sel) -> NSModalResponse, Imp>(run_modal) };
    replace(
        NSSavePanel::class(),
        sel!(runModal),
        Some(sel!(ideSelftestRunModal)),
        run_modal,
    )
}

/// Puts an open or save panel in the middle of the run's display before it shows: macOS places these panels by the
/// display with the menu bar, past `NSScreen`.
extern "C-unwind" fn run_modal(panel: &NSSavePanel, _cmd: Sel) -> NSModalResponse {
    guarded(NSModalResponseCancel, || {
        if let Some(screen) = NSScreen::mainScreen(panel.mtm()) {
            let (visible, size) = (screen.visibleFrame(), panel.frame().size);
            let origin = NSPoint::new(
                visible.origin.x + (visible.size.width - size.width) / 2.0,
                visible.origin.y + (visible.size.height - size.height) / 2.0,
            );
            panel.setFrame_display(NSRect::new(origin, size), false);
        }
        // SAFETY: The kept selector carries `-runModal`'s own implementation.
        unsafe { msg_send![panel, ideSelftestRunModal] }
    })
}

/// Gives the method `name` of `target`, a class or for class methods its metaclass, the implementation `imp`, keeping
/// the one it had under `kept` if given.
fn replace(target: &AnyClass, name: Sel, kept: Option<Sel>, imp: Imp) -> Result<(), String> {
    let method = target
        .instance_method(name)
        .ok_or_else(|| format!("{:?} has no {name}", target.name()))?;
    if let Some(kept) = kept {
        // SAFETY: The method's type encoding lives as long as the method; the kept selector gets the method's own
        // implementation with that encoding, on the same class.
        let added = unsafe {
            objc2::ffi::class_addMethod(
                std::ptr::from_ref(target).cast_mut(),
                kept,
                method.implementation(),
                objc2::ffi::method_getTypeEncoding(method),
            )
        };
        if !added.as_bool() {
            return Err(format!("{:?} already has {kept}", target.name()));
        }
    }
    // SAFETY: The caller passes an implementation with the method's signature that returns what the method returns.
    unsafe { method.set_implementation(imp) };
    Ok(())
}

extern "C-unwind" fn screens(class: &AnyClass, _cmd: Sel) -> *mut NSArray<NSScreen> {
    guarded(std::ptr::null_mut(), || {
        // SAFETY: The kept selector carries `+screens`' own implementation, which returns an array of screens.
        let all: Retained<NSArray<NSScreen>> = unsafe { msg_send![class, ideSelftestScreens] };
        Retained::autorelease_return(own_screens(all))
    })
}

/// The screens of the run's display out of `all`, or all of them while the display is not there.
fn own_screens(all: Retained<NSArray<NSScreen>>) -> Retained<NSArray<NSScreen>> {
    let Some(display) = display() else { return all };
    let own: Vec<Retained<NSScreen>> = all.iter().filter(|screen| number(screen) == Some(display)).collect();
    if own.is_empty() {
        all
    } else {
        NSArray::from_retained_slice(&own)
    }
}

fn number(screen: &NSScreen) -> Option<u32> {
    let value = screen.deviceDescription().objectForKey(&ns("NSScreenNumber"))?;
    value
        .downcast::<NSNumber>()
        .ok()
        .map(|number| number.unsignedIntValue())
}

extern "C-unwind" fn main_screen(class: &AnyClass, _cmd: Sel) -> *mut NSScreen {
    guarded(std::ptr::null_mut(), || {
        // SAFETY: `+screens` returns an array of screens, here the run's own.
        let screens: Retained<NSArray<NSScreen>> = unsafe { msg_send![class, screens] };
        screens
            .firstObject()
            .map_or(std::ptr::null_mut(), Retained::autorelease_return)
    })
}

/// Moves `window`, before it shows, onto the run's display if it lies on none of the display: AppKit places some
/// windows, alerts among them, by the frame of the display with the menu bar.
pub(super) fn onto_own_screen(window: &NSWindow, mtm: MainThreadMarker) {
    if display().is_none() || window.parentWindow().is_some() || window.sheetParent().is_some() {
        return;
    }
    let Some(screen) = NSScreen::mainScreen(mtm) else {
        return;
    };
    let (frame, visible) = (window.frame(), screen.visibleFrame());
    if objc2_foundation::NSIntersectsRect(frame, screen.frame()) {
        return;
    }
    let along = |origin: f64, size: f64, start: f64, room: f64| (start + origin).min(start + room - size).max(start);
    let origin = NSPoint::new(
        along(frame.origin.x, frame.size.width, visible.origin.x, visible.size.width),
        along(frame.origin.y, frame.size.height, visible.origin.y, visible.size.height),
    );
    window.setFrame_display(NSRect::new(origin, frame.size), false);
}
