//! The view of one pane of a window tab: its documents behind one another, the one in front showing, its tab bar
//! above them, and what an empty pane offers to open (E145).

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSAccessibility;
use objc2_app_kit::{
    NSBox, NSButton, NSColor, NSEvent, NSFont, NSStackView, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSPoint;
use splat_i18n::t;

use super::Side;
use super::bar::{PaneBar, drop_highlight};
use crate::util::{activate, center_in, guarded, ns, pin_edges, pin_sides};
use crate::window::WindowController;
use crate::window::tabs::Document;

define_class!(
    // SAFETY: NSView has no subclassing requirements and `EmptyPane` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEEmptyPane"]
    pub(crate) struct EmptyPane;

    unsafe impl NSObjectProtocol for EmptyPane {}

    impl EmptyPane {
        /// The empty pane takes the keyboard focus, so it becomes the active pane the next document opens in.
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            guarded((), || {
                if let Some(window) = self.window() {
                    window.makeFirstResponder(Some(self));
                }
            });
        }
    }
);

impl EmptyPane {
    fn new(mtm: MainThreadMarker) -> Retained<EmptyPane> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        let note = NSTextField::labelWithString(&ns(t("Leerer Bereich")), mtm);
        note.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSize())));
        note.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let button = |title: &str, action| {
            // SAFETY: A nil target sends the action along the responder chain to the window, which answers it.
            unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, Some(action), mtm) }
        };
        let open = button(t("Schnell öffnen …"), sel!(ideQuickOpen:));
        let new = button(t("Neuer Tab …"), sel!(ideNewTab:));
        let column = NSStackView::new(mtm);
        column.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        column.setSpacing(ide_ui_common::style::space::L);
        for view in [&*note as &NSView, &open, &new] {
            column.addArrangedSubview(view);
        }
        this.addSubview(&column);
        center_in(&column, &this);
        this
    }
}

/// The view of one pane: the bar over its documents, the documents behind one another with the one in front showing,
/// and what an empty pane offers.
pub(crate) struct Pane {
    /// The bar over the documents.
    pub(in crate::window) host: Retained<NSView>,
    pub(super) bar: PaneBar,
    pub(super) content: Retained<NSView>,
    empty: Retained<EmptyPane>,
    /// The documents the pane shows now, as the tree last said.
    hosted: RefCell<Vec<Rc<Document>>>,
    /// The accent over the part of the pane a dragged tab would drop into.
    drop: Retained<NSBox>,
}

impl Pane {
    pub(super) fn new(mtm: MainThreadMarker, number: usize) -> Rc<Pane> {
        let host = NSView::new(mtm);
        // A group of its own, so VoiceOver keeps a pane's controls apart from the sidebar's and other panes'.
        host.setAccessibilityElement(true);
        host.setAccessibilityRole(Some(crate::util::constant!(objc2_app_kit::NSAccessibilityGroupRole)));
        let bar = PaneBar::new(mtm, number);
        let content = NSView::new(mtm);
        let empty = EmptyPane::new(mtm);
        content.addSubview(&empty);
        pin_edges(&empty, &content);
        let drop = drop_highlight(mtm);
        drop.setTranslatesAutoresizingMaskIntoConstraints(true);
        for view in [&*bar.view as &NSView, &content, &drop] {
            host.addSubview(view);
        }
        pin_sides(&bar.view, &host, 0.0);
        pin_sides(&content, &host, 0.0);
        activate([
            bar.view.topAnchor().constraintEqualToAnchor(&host.topAnchor()),
            content.topAnchor().constraintEqualToAnchor(&bar.view.bottomAnchor()),
            content.bottomAnchor().constraintEqualToAnchor(&host.bottomAnchor()),
        ]);
        Rc::new(Pane {
            host,
            bar,
            content,
            empty,
            hosted: RefCell::new(Vec::new()),
            drop,
        })
    }

    /// Whether `view` is in the pane.
    pub(in crate::window) fn holds_view(&self, view: &NSView) -> bool {
        view.isDescendantOf(&self.host)
    }

    /// Shows `documents` behind one another, the one at `selected` in front: those that left the pane go out of it and
    /// show again for wherever they go, new ones come in. A document in the background keeps its view in the window,
    /// where agents and terminals look for it.
    pub(super) fn mirror(&self, documents: Vec<Rc<Document>>, selected: usize) {
        let before = self.hosted.replace(documents.clone());
        for gone in before
            .iter()
            .filter(|old| !documents.iter().any(|doc| doc.id == old.id))
        {
            if super::is_in(&gone.host, &self.content) {
                gone.host.removeFromSuperview();
                gone.host.setHidden(false);
            }
        }
        for doc in &documents {
            if !super::is_in(&doc.host, &self.content) {
                self.content.addSubview(&doc.host);
                pin_edges(&doc.host, &self.content);
            }
        }
        let front = selected.min(documents.len().saturating_sub(1));
        for (at, doc) in documents.iter().enumerate() {
            let hidden = at != front;
            if doc.host.isHidden() != hidden {
                doc.host.setHidden(hidden);
            }
        }
        self.empty.setHidden(!documents.is_empty());
    }

    /// The view to give the keyboard focus to, in `front`, the document in front, or the empty pane.
    pub(super) fn focus_target(&self, front: Option<&Document>) -> Option<Retained<NSView>> {
        match front {
            Some(doc) => doc.focus_target(),
            None => Some(Retained::into_super(self.empty.clone())),
        }
    }

    /// `point`, in window coordinates, in the pane's documents, if it lies there.
    pub(super) fn content_point(&self, point: NSPoint) -> Option<NSPoint> {
        if self.host.isHidden() {
            return None;
        }
        let local = self.content.convertPoint_fromView(point, None);
        objc2_foundation::NSPointInRect(local, self.content.bounds()).then_some(local)
    }

    pub(super) fn content_size(&self) -> objc2_foundation::NSSize {
        self.content.bounds().size
    }

    /// Shows the accent over the right or bottom half, or over the whole content, or hides it.
    pub(super) fn show_drop(&self, part: Option<Option<Side>>) {
        let Some(side) = part else {
            self.drop.setHidden(true);
            return;
        };
        let content = self.content.frame();
        let (origin, size) = (content.origin, content.size);
        let half = |x: f64, y: f64, w: f64, h: f64| {
            objc2_foundation::NSRect::new(NSPoint::new(x, y), objc2_foundation::NSSize::new(w, h))
        };
        let frame = match side {
            Some(Side::Right) => half(origin.x + size.width / 2.0, origin.y, size.width / 2.0, size.height),
            Some(Side::Below) => half(origin.x, origin.y, size.width, size.height / 2.0),
            None => content,
        };
        let inset = 6.0;
        self.drop.setFrame(half(
            frame.origin.x + inset,
            frame.origin.y + inset,
            (frame.size.width - 2.0 * inset).max(0.0),
            (frame.size.height - 2.0 * inset).max(0.0),
        ));
        self.drop.setHidden(false);
    }

    /// Shows the bar over `documents` or not, `selected` the one in front, and the badges of the documents while
    /// there is none: `badged` for a pane after the first, `active` for the active pane.
    pub(super) fn show(
        &self,
        target: &WindowController,
        documents: &[Rc<Document>],
        selected: usize,
        bars: bool,
        badged: bool,
        active: bool,
    ) {
        self.bar.set_shown(bars);
        let name = match documents.get(selected) {
            Some(front) => front.title(),
            None => t("Leerer Bereich").to_string(),
        };
        self.host.setAccessibilityLabel(Some(&ns(&name)));
        if bars {
            self.bar.show(target, documents, selected, active);
        }
        for (index, doc) in documents.iter().enumerate() {
            doc.set_badge(!bars && badged && index == selected, active);
        }
    }
}
