//! The sheet an export shows on its window while it runs: what it does, a spinning indicator and "Abbrechen"; when it
//! fails, the reason and "OK".

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSProgressIndicator, NSProgressIndicatorStyle, NSStackView, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use ide_ui_common::util::{guarded, ns};

pub struct SheetIvars {
    /// The export the sheet belongs to.
    number: u64,
    /// The export failed: the button only closes the sheet.
    failed: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `SheetTarget` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEExportSheetTarget"]
    #[ivars = SheetIvars]
    struct SheetTarget;

    unsafe impl NSObjectProtocol for SheetTarget {}

    impl SheetTarget {
        #[unsafe(method(ideExportSheetButton:))]
        fn button(&self, _sender: Option<&AnyObject>) {
            let (number, failed) = (self.ivars().number, self.ivars().failed.get());
            guarded((), || super::sheet_button(number, failed));
        }
    }
);

/// A sheet on the window of an export.
pub struct Sheet {
    parent: Retained<NSWindow>,
    panel: Retained<NSWindow>,
    label: Retained<NSTextField>,
    spinner: Retained<NSProgressIndicator>,
    button: Retained<NSButton>,
    /// The button's target, which the button holds only weakly.
    target: Retained<SheetTarget>,
}

impl Sheet {
    /// Shows a sheet on `parent` saying `what` runs, for the export `number`.
    pub fn show(parent: &NSWindow, what: &str, number: u64) -> Sheet {
        let mtm = parent.mtm();
        let target = SheetTarget::alloc(mtm).set_ivars(SheetIvars {
            number,
            failed: Cell::new(false),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let target: Retained<SheetTarget> = unsafe { msg_send![super(target), init] };
        let label = NSTextField::wrappingLabelWithString(&ns(what), mtm);
        label.setPreferredMaxLayoutWidth(320.0);
        let spinner = NSProgressIndicator::new(mtm);
        spinner.setStyle(NSProgressIndicatorStyle::Bar);
        spinner.setIndeterminate(true);
        // SAFETY: Starting the animation of an indicator made on the main thread, with no sender.
        unsafe { spinner.startAnimation(None) };
        // SAFETY: The target answers the action and lives as long as the sheet.
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &ns(t("Abbrechen")),
                Some(&target),
                Some(sel!(ideExportSheetButton:)),
                mtm,
            )
        };
        let views: [&NSView; 3] = [&label, &spinner, &button];
        let stack = NSStackView::stackViewWithViews(&NSArray::from_slice(&views), mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setSpacing(14.0);
        stack.setEdgeInsets(NSEdgeInsets {
            top: 20.0,
            left: 20.0,
            bottom: 20.0,
            right: 20.0,
        });
        spinner.widthAnchor().constraintEqualToConstant(320.0).setActive(true);
        let panel = panel(mtm);
        panel.setContentView(Some(&stack));
        parent.beginSheet_completionHandler(&panel, None);
        Sheet {
            parent: parent.retain(),
            panel,
            label,
            spinner,
            button,
            target,
        }
    }

    pub fn close(&self) {
        self.parent.endSheet(&self.panel);
        self.panel.orderOut(None);
    }

    /// Says why the export failed; the sheet stays until the user closes it with OK.
    pub fn failed(&self, reason: &str) {
        self.target.ivars().failed.set(true);
        self.label.setStringValue(&ns(reason));
        // SAFETY: Stopping the animation the sheet started, with no sender.
        unsafe { self.spinner.stopAnimation(None) };
        self.spinner.setHidden(true);
        self.button.setTitle(&ns(t("OK")));
    }

    /// What the sheet says now.
    #[cfg(feature = "selftest")]
    pub fn text(&self) -> String {
        self.label.stringValue().to_string()
    }
}

fn panel(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 120.0));
    // SAFETY: NSWindow's designated initializer on a newly allocated window.
    let panel = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel
}
