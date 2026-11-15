//! The application object of a selftest run, which never makes the app the active one, so the user's typing never
//! reaches its windows while it runs (see "Selftest" in docs/development.md).

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSResponder, NSWindow};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSString};

use crate::util::guarded;

define_class!(
    // SAFETY: NSApplication may be subclassed; `SelftestApplication` adds no state and does not implement Drop.
    #[unsafe(super(NSApplication, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESelftestApplication"]
    struct SelftestApplication;

    impl SelftestApplication {
        // AppKit calls this when it begins an alert as a sheet, and it brings the app to the front even while another
        // app is in front.
        #[unsafe(method(activateIgnoringOtherApps:))]
        fn activate_ignoring_other_apps(&self, _flag: bool) {}

        #[unsafe(method(activate))]
        fn activate(&self) {}

        // The private method AppKit's activations end in, among them the one at launch from `_doOpenUntitled`.
        #[unsafe(method(_activateWithInfo:))]
        fn activate_with_info(&self, _info: Option<&AnyObject>) {}

        // Assistive apps and AppleScript's `set frontmost` end here, and AppKit answers through `NSRunningApplication`,
        // past `_activateWithInfo:`.
        #[unsafe(method(accessibilitySetFrontmostAttribute:))]
        fn set_frontmost(&self, _value: Option<&AnyObject>) {}

        // The accessibility audit runs with full keyboard access, a setting of the Mac's global domain that launch
        // arguments do not reach.
        #[unsafe(method(isFullKeyboardAccessEnabled))]
        fn full_keyboard_access(&self) -> bool {
            // SAFETY: NSApplication answers `isFullKeyboardAccessEnabled` with a BOOL.
            guarded(false, || audit() || unsafe { msg_send![super(self), isFullKeyboardAccessEnabled] })
        }
    }
);

/// Whether the run is the accessibility audit.
fn audit() -> bool {
    std::env::var("IDE_SELFTEST").is_ok_and(|scenario| scenario == "a11y" || scenario == "a11y-rtl")
}

/// Makes the shared application object one that never activates the app, when a scenario runs; called before anything
/// else asks for the shared application.
pub(crate) fn install(_mtm: MainThreadMarker) {
    if std::env::var_os("IDE_SELFTEST").is_none() {
        return;
    }
    // SAFETY: `+sharedApplication` creates the shared instance of the class it is sent to, here the subclass, once.
    let app: Retained<NSApplication> = unsafe { msg_send![SelftestApplication::class(), sharedApplication] };
    if !app.isKindOfClass(SelftestApplication::class()) {
        crate::diag::report(
            "selftest",
            "the shared application was made before the selftest's and may take the keyboard",
        );
    }
    pass_clicks_through();
}

/// Has every window let clicks through to what lies behind it from before it shows: the window server makes an app the
/// active one at a click into one of its windows, before the app hears of it.
fn pass_clicks_through() {
    let Some(name) = will_order_on_screen() else {
        super::check(
            "windows let clicks through",
            false,
            "AppKit has no NSWindowWillOrderOnScreenNotification",
        );
        return;
    };
    let block = RcBlock::new(|note: NonNull<NSNotification>| {
        guarded((), || {
            // SAFETY: The center passes a notification that lives for the call.
            let note = unsafe { note.as_ref() };
            if let Some(window) = note.object().as_deref().and_then(AnyObject::downcast_ref::<NSWindow>) {
                window.setIgnoresMouseEvents(true);
            }
        });
    });
    // SAFETY: AppKit posts the notification on the main thread, which the window it names belongs to, and the block
    // keeps no reference to it.
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
    };
    std::mem::forget(observer);
}

/// The name of the notification AppKit posts for every window, panels, sheets and popovers included, before it shows.
fn will_order_on_screen() -> Option<&'static NSString> {
    // SAFETY: dlsym reads the symbol table; a symbol of this name is AppKit's `NSString *const`.
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"NSWindowWillOrderOnScreenNotification".as_ptr()) };
    let name = NonNull::new(symbol.cast::<*const NSString>())?;
    // SAFETY: The constant is set before AppKit's code runs and never changes or goes away.
    unsafe { name.read().as_ref() }
}
