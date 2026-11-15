//! The frame every overlay of an AI tab shares (E146): a dimmed layer over the tab and a card in its middle with a
//! title, a close button, a scrolling list and a row of buttons. The layer keeps clicks beside the card from reaching
//! the tab, Esc closes it from anywhere in it, and VoiceOver reads the card as a modal group.

use std::cell::RefCell;
use std::rc::Weak as RcWeak;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSBox, NSColor, NSEvent, NSLayoutConstraint, NSScrollView, NSStackView, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect};
use splat_i18n::t;

use crate::tab::AgentTab;
use crate::ui;
use ide_ui_common::util::{activate, guarded, pin_edges};

/// The card's width, and the share of the tab's height it may take.
const WIDTH: f64 = 560.0;
const MAX_SHARE: f64 = 0.8;
/// The key code of Esc.
const ESCAPE: u16 = 53;

pub(crate) struct ShadeIvars {
    tab: RcWeak<AgentTab>,
    card: RefCell<Option<Retained<NSBox>>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Shade` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentDialogShade"]
    #[ivars = ShadeIvars]
    pub(crate) struct Shade;

    unsafe impl NSObjectProtocol for Shade {}

    impl Shade {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            guarded((), || {
                NSColor::blackColor().colorWithAlphaComponent(0.18).setFill();
                objc2_app_kit::NSBezierPath::fillRect(self.bounds());
            });
        }

        /// Clicks on the card reach its controls; clicks beside it stop here. The layer fills its superview from
        /// its origin, so `point`, in the superview's coordinates, is the same in the layer's.
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
            let card = self.ivars().card.borrow().clone();
            if let Some(card) = card {
                let local = card.convertPoint_fromView(point, Some(self));
                if objc2_foundation::NSPointInRect(local, card.bounds()) {
                    // SAFETY: The overridden method with arguments of its types.
                    return unsafe { msg_send![super(self), hitTest: point] };
                }
            }
            Some(crate::ui::view(self))
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        /// A click beside the card closes the overlay, as clicking beside a popover does.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            guarded((), || self.close());
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if event.keyCode() == ESCAPE {
                guarded((), || self.close());
            } else {
                // SAFETY: The overridden method with arguments of its types.
                let () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }

        /// Esc in a text field of the card reaches the layer as this.
        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.close());
        }

        #[unsafe(method(ideDialogClose:))]
        fn close_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.close());
        }

        #[unsafe(method(ideDialogAction:))]
        fn action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = crate::target::tag(sender);
                if let Some(tab) = self.ivars().tab.upgrade() {
                    super::run_action(&tab, tag);
                }
            });
        }
    }
);

impl Shade {
    fn new(mtm: MainThreadMarker, tab: RcWeak<AgentTab>) -> Retained<Shade> {
        let this = Self::alloc(mtm).set_ivars(ShadeIvars {
            tab,
            card: RefCell::new(None),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    fn close(&self) {
        if let Some(tab) = self.ivars().tab.upgrade() {
            tab.close_dialog();
        }
    }
}

/// The views of an open overlay.
pub(crate) struct Frame {
    pub(crate) shade: Retained<Shade>,
    title: String,
    /// The rows in the scrolling list.
    pub(crate) list: Retained<NSStackView>,
    /// The line above the list that says what keeps it waiting or what went wrong.
    notice: Retained<NSTextField>,
    /// The row of buttons below the list.
    footer: Retained<NSStackView>,
}

impl Frame {
    /// A frame titled `title` with the symbol `symbol` over `tab`, not shown yet.
    pub(crate) fn new(mtm: MainThreadMarker, tab: RcWeak<AgentTab>, title: &str, symbol: &str) -> Frame {
        let shade = Shade::new(mtm, tab);
        let heading = ui::label(mtm, title, &crate::style::bold(13.0), &NSColor::labelColor());
        let close = ui::symbol_button(mtm, "xmark", t("Schließen"), 12.0, &shade, sel!(ideDialogClose:), 0);
        close.setTitle(&ide_ui_common::util::ns(""));
        ide_ui_common::util::set_label(&close, Some(t("Schließen")));
        let head = ui::row(
            mtm,
            8.0,
            &[
                &ui::icon(mtm, symbol, title, 14.0, &NSColor::secondaryLabelColor()),
                &heading,
                &ui::spacer(mtm),
                &close,
            ],
        );
        let notice = ui::wrapping(
            mtm,
            "",
            &crate::style::small(),
            &NSColor::secondaryLabelColor(),
            WIDTH - 40.0,
        );
        notice.setHidden(true);
        let list = ui::column(mtm, 6.0, &[]);
        let scroll = scrolling(mtm, &list);
        let footer = ui::row(mtm, 8.0, &[&ui::spacer(mtm)]);
        let column = ui::column(mtm, 10.0, &[&head, &ui::separator(mtm), &notice, &scroll, &footer]);
        let card = ui::framed(
            mtm,
            &column,
            &NSColor::separatorColor(),
            Some(&NSColor::windowBackgroundColor()),
            12.0,
            ui::insets(14.0, 16.0, 14.0, 16.0),
        );
        card.setAccessibilityElement(true);
        card.setAccessibilityRole(Some(ide_ui_common::constant!(objc2_app_kit::NSAccessibilityGroupRole)));
        card.setAccessibilityLabel(Some(&ide_ui_common::util::ns(title)));
        card.setAccessibilityModal(true);
        shade.addSubview(&card);
        *shade.ivars().card.borrow_mut() = Some(card.clone());
        let fits = card
            .heightAnchor()
            .constraintLessThanOrEqualToAnchor_multiplier(&shade.heightAnchor(), MAX_SHARE);
        let wide = card.widthAnchor().constraintEqualToConstant(WIDTH);
        wide.setPriority(700.0);
        let tall = scroll.heightAnchor().constraintEqualToAnchor(&list.heightAnchor());
        tall.setPriority(600.0);
        let constraints: [Retained<NSLayoutConstraint>; 6] = [
            card.centerXAnchor().constraintEqualToAnchor(&shade.centerXAnchor()),
            card.centerYAnchor().constraintEqualToAnchor(&shade.centerYAnchor()),
            card.widthAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&shade.widthAnchor(), -32.0),
            wide,
            fits,
            tall,
        ];
        activate(constraints);
        Frame {
            shade,
            title: title.to_string(),
            list,
            notice,
            footer,
        }
    }

    /// Lays the frame over `root` and moves the keyboard focus into it.
    pub(crate) fn show(&self, root: &NSView) {
        root.addSubview(&self.shade);
        pin_edges(&self.shade, root);
        if let Some(window) = root.window() {
            window.makeFirstResponder(Some(&self.shade));
        }
        ide_ui_common::panel::announce(self.shade.mtm(), &self.title);
    }

    /// Takes the frame away.
    pub(crate) fn hide(&self) {
        self.shade.removeFromSuperview();
    }

    /// Shows `text` above the list, or nothing.
    pub(crate) fn set_notice(&self, text: Option<&str>) {
        self.notice
            .setStringValue(&ide_ui_common::util::ns(text.unwrap_or_default()));
        self.notice.setHidden(text.is_none());
    }

    /// Puts `rows` in the list in place of the ones before.
    pub(crate) fn set_rows(&self, rows: &[Retained<NSView>]) {
        for old in self.list.arrangedSubviews().iter() {
            self.list.removeArrangedSubview(&old);
            old.removeFromSuperview();
        }
        for row in rows {
            ui::add_full_width(&self.list, row);
        }
    }

    /// Puts `buttons` in the footer, right-aligned, in place of the ones before.
    pub(crate) fn set_buttons(&self, buttons: &[Retained<NSView>]) {
        for old in self.footer.arrangedSubviews().iter().skip(1) {
            self.footer.removeArrangedSubview(&old);
            old.removeFromSuperview();
        }
        for button in buttons {
            self.footer.addArrangedSubview(button);
        }
    }

    /// The target of the frame's buttons.
    pub(crate) fn target(&self) -> &AnyObject {
        &self.shade
    }
}

/// `list` in a scroll view that grows with it up to the card's room, with the list's top at the top.
fn scrolling(mtm: MainThreadMarker, list: &NSStackView) -> Retained<NSScrollView> {
    let scroll = NSScrollView::new(mtm);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
    let document = crate::agent_panel::flipped(mtm);
    document.setTranslatesAutoresizingMaskIntoConstraints(false);
    document.addSubview(list);
    scroll.setDocumentView(Some(&document));
    pin_edges(list, &document);
    activate([document
        .widthAnchor()
        .constraintEqualToAnchor(&scroll.contentView().widthAnchor())]);
    scroll
}
