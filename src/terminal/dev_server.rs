//! The bar above a terminal that names a development server its program started, with a button that opens the address
//! in the window's browser tab; Tapa's notices above the view carry no button.

use std::cell::RefCell;

use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBox, NSBoxType, NSButton, NSColor, NSControlSize, NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation,
    NSLineBreakMode, NSStackView, NSTextField, NSTitlePosition, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::util::{guarded, label, ns, pin_edges, set_action, symbol, symbol_button};
use crate::window::WindowController;

/// The size of the close button's symbol, in points.
const CLOSE_SYMBOL: f64 = 10.0;

/// What a banner keeps.
pub(crate) struct Ivars {
    /// The window whose browser tab the button opens the address in.
    controller: Weak<WindowController>,
    message: Retained<NSTextField>,
    /// The address shown now.
    url: RefCell<Option<String>>,
    /// Every address the banner showed, which it shows only once each.
    shown: RefCell<Vec<String>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `DevServerBanner` does not implement Drop.
    /// The dev server's bar above a terminal, hidden until a program announces one.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDevServerBanner"]
    #[ivars = Ivars]
    pub(crate) struct DevServerBanner;

    impl DevServerBanner {
        #[unsafe(method(ideOpenDevServer:))]
        fn open_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.open());
        }

        #[unsafe(method(ideDismissDevServer:))]
        fn dismiss_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.setHidden(true));
        }
    }
);

impl DevServerBanner {
    /// A hidden banner whose button opens the address in `controller`'s browser tab.
    pub(crate) fn new(mtm: MainThreadMarker, controller: &WindowController) -> Retained<DevServerBanner> {
        let message = label(mtm, "");
        message.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        message.setContentCompressionResistancePriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        let ivars = Ivars {
            controller: Weak::from(controller),
            message,
            url: RefCell::new(None),
            shown: RefCell::new(Vec::new()),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.assemble(mtm);
        this.setHidden(true);
        this
    }

    fn assemble(&self, mtm: MainThreadMarker) {
        let tint = NSColor::controlAccentColor().colorWithAlphaComponent(0.14);
        let background = NSBox::new(mtm);
        background.setBoxType(NSBoxType::Custom);
        background.setTitlePosition(NSTitlePosition::NoTitle);
        background.setBorderWidth(0.0);
        background.setCornerRadius(0.0);
        background.setContentViewMargins(NSSize::new(0.0, 0.0));
        background.setFillColor(&tint);
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setAlignment(NSLayoutAttribute::CenterY);
        row.setSpacing(ide_ui_common::style::space::M);
        row.setEdgeInsets(NSEdgeInsets {
            top: 5.0,
            left: 10.0,
            bottom: 5.0,
            right: 8.0,
        });
        if let Some(image) = symbol("network", "") {
            let icon = NSImageView::imageViewWithImage(&image, mtm);
            icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
            row.addArrangedSubview(&icon);
        }
        row.addArrangedSubview(&self.ivars().message);
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        row.addArrangedSubview(&spacer);
        let target: &AnyObject = self;
        // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
        let button =
            unsafe { NSButton::buttonWithTitle_target_action(&ns(t("Im Browser-Tab öffnen")), None, None, mtm) };
        set_action(&button, target, sel!(ideOpenDevServer:));
        button.setControlSize(NSControlSize::Small);
        row.addArrangedSubview(&button);
        let dismiss = symbol_button(
            mtm,
            "xmark",
            t("Schließen"),
            CLOSE_SYMBOL,
            Some(target),
            sel!(ideDismissDevServer:),
        );
        dismiss.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        row.addArrangedSubview(&dismiss);
        self.addSubview(&background);
        pin_edges(&background, self);
        self.addSubview(&row);
        pin_edges(&row, self);
    }

    /// Shows that a development server listens at `url`, unless the banner showed that address before.
    pub(crate) fn show(&self, url: &str) {
        let mut shown = self.ivars().shown.borrow_mut();
        if shown.iter().any(|known| known == url) {
            return;
        }
        shown.push(url.to_string());
        drop(shown);
        self.ivars()
            .message
            .setStringValue(&ns(&tf("Dev-Server läuft: {}", &[url])));
        *self.ivars().url.borrow_mut() = Some(url.to_string());
        self.setHidden(false);
    }

    fn open(&self) {
        self.setHidden(true);
        let url = self.ivars().url.borrow().clone();
        if let (Some(url), Some(controller)) = (url, self.ivars().controller.load()) {
            crate::browser::open_tab(&controller, Some(&url));
        }
    }
}

/// The banner above `root`, what a terminal's pane shows, in a column with it.
pub(crate) fn above(mtm: MainThreadMarker, banner: &DevServerBanner, root: &NSView) -> Retained<NSView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setDistribution(objc2_app_kit::NSStackViewDistribution::Fill);
    stack.setSpacing(ide_ui_common::style::space::NONE);
    for view in [&**banner, root] {
        stack.addArrangedSubview(view);
        crate::util::pin_sides(view, &stack, 0.0);
    }
    root.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
    root.setContentCompressionResistancePriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
    stack.into_super()
}

#[cfg(feature = "selftest")]
mod selftest_probe;
