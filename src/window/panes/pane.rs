//! One pane of a window tab: its documents behind one another, the one in front showing, its tab bar above them, and
//! what an empty pane offers to open (E145).

use std::cell::{Cell, RefCell};
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
use crate::util::{guarded, ns, pin_edges};
use crate::window::tabs::Document;
use crate::window::{TabId, WindowController};

define_class!(
    // SAFETY: NSView has no subclassing requirements and `EmptyPane` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEEmptyPane"]
    pub struct EmptyPane;

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
        column.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            column.centerXAnchor().constraintEqualToAnchor(&this.centerXAnchor()),
            column.centerYAnchor().constraintEqualToAnchor(&this.centerYAnchor()),
        ] {
            constraint.setActive(true);
        }
        this
    }
}

pub struct Pane {
    /// The pane's number in its window tab, which stays while the pane lives.
    pub(in crate::window) number: usize,
    /// The bar over the documents.
    pub(in crate::window) host: Retained<NSView>,
    pub(super) bar: PaneBar,
    pub(super) content: Retained<NSView>,
    empty: Retained<EmptyPane>,
    documents: RefCell<Vec<Rc<Document>>>,
    selected: Cell<usize>,
    /// The document whose preview the pane shows beside it, which it follows and closes with (E59).
    pub(in crate::window) companion_of: Cell<Option<TabId>>,
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
        for view in [&*bar.view as &NSView, &content] {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        for constraint in [
            bar.view.topAnchor().constraintEqualToAnchor(&host.topAnchor()),
            bar.view.leadingAnchor().constraintEqualToAnchor(&host.leadingAnchor()),
            bar.view
                .trailingAnchor()
                .constraintEqualToAnchor(&host.trailingAnchor()),
            content.topAnchor().constraintEqualToAnchor(&bar.view.bottomAnchor()),
            content.leadingAnchor().constraintEqualToAnchor(&host.leadingAnchor()),
            content.trailingAnchor().constraintEqualToAnchor(&host.trailingAnchor()),
            content.bottomAnchor().constraintEqualToAnchor(&host.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        Rc::new(Pane {
            number,
            host,
            bar,
            content,
            empty,
            documents: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            companion_of: Cell::new(None),
            drop,
        })
    }

    pub(in crate::window) fn documents(&self) -> Vec<Rc<Document>> {
        self.documents.borrow().clone()
    }

    pub(in crate::window) fn count(&self) -> usize {
        self.documents.borrow().len()
    }

    pub(in crate::window) fn holds(&self, id: TabId) -> bool {
        self.documents.borrow().iter().any(|doc| doc.id == id)
    }

    /// Whether `view` is in the pane.
    pub(in crate::window) fn holds_view(&self, view: &NSView) -> bool {
        view.isDescendantOf(&self.host)
    }

    /// The document in front.
    pub(in crate::window) fn front(&self) -> Option<Rc<Document>> {
        self.documents.borrow().get(self.selected.get()).cloned()
    }

    /// Puts `document` at `index`, or after the one in front, and brings it to the front.
    pub(in crate::window) fn insert(&self, document: Rc<Document>, index: Option<usize>) {
        let count = self.count();
        let index = index
            .unwrap_or(if count == 0 { 0 } else { self.selected.get() + 1 })
            .min(count);
        self.content.addSubview(&document.host);
        pin_edges(&document.host, &self.content);
        self.documents.borrow_mut().insert(index, document);
        self.select_index(index);
    }

    /// Takes the document `id` out; the one after it comes to the front, else the one before.
    pub(super) fn take(&self, id: TabId) -> Option<Rc<Document>> {
        let index = self.documents.borrow().iter().position(|doc| doc.id == id)?;
        let document = self.documents.borrow_mut().remove(index);
        document.host.removeFromSuperview();
        document.host.setHidden(false);
        let selected = self.selected.get();
        let next = if selected > index || (selected == index && index >= self.count()) {
            selected.saturating_sub(1)
        } else {
            selected
        };
        self.select_index(next);
        Some(document)
    }

    /// Brings the document `id` to the front.
    pub(in crate::window) fn select(&self, id: TabId) {
        let index = self.documents.borrow().iter().position(|doc| doc.id == id);
        if let Some(index) = index {
            self.select_index(index);
        }
    }

    /// Brings the document `step` places from the one in front to the front, wrapping around.
    pub(in crate::window) fn select_relative(&self, step: isize) {
        let count = isize::try_from(self.count()).unwrap_or(0);
        if count < 2 {
            return;
        }
        let index = (isize::try_from(self.selected.get()).unwrap_or(0) + step).rem_euclid(count);
        self.select_index(usize::try_from(index).unwrap_or(0));
    }

    fn select_index(&self, index: usize) {
        let documents = self.documents.borrow();
        let index = index.min(documents.len().saturating_sub(1));
        self.selected.set(index);
        // A document in the background keeps its view in the window, where agents and terminals look for it.
        for (at, doc) in documents.iter().enumerate() {
            let hidden = at != index;
            if doc.host.isHidden() != hidden {
                doc.host.setHidden(hidden);
            }
        }
        self.empty.setHidden(!documents.is_empty());
    }

    /// The view to give the keyboard focus to, in the document in front or the empty pane.
    pub(in crate::window) fn focus_target(&self) -> Option<Retained<NSView>> {
        match self.front() {
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

    /// Shows the bar over the documents or not, and the badges of the documents while there is none: `badged` for a
    /// pane after the first, `active` for the active pane.
    pub(super) fn show(&self, target: &WindowController, bars: bool, badged: bool, active: bool) {
        self.bar.set_shown(bars);
        let documents = self.documents.borrow();
        let name = match documents.get(self.selected.get()) {
            Some(front) => front.title(),
            None => t("Leerer Bereich").to_string(),
        };
        self.host.setAccessibilityLabel(Some(&ns(&name)));
        if bars {
            self.bar.show(target, &documents, self.selected.get(), active);
        }
        for (index, doc) in documents.iter().enumerate() {
            doc.set_badge(!bars && badged && index == self.selected.get(), active);
        }
    }
}
