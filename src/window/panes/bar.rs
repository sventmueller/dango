//! The tab bar of a pane, drawn like Xcode's editor tabs (E145). It shows only while a pane of its window tab holds
//! more than one document; a tab selects its document, closes it, and drags it along the bar, into another pane or to
//! a pane's edge.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityRadioButtonRole, NSAccessibilityTabButtonSubrole, NSAccessibilityTabGroupRole,
    NSBezierPath, NSBox, NSBoxType, NSButton, NSColor, NSEvent, NSFont, NSFontManager, NSFontTraitMask, NSImageView,
    NSLayoutConstraint, NSLineBreakMode, NSMenu, NSStackView, NSStackViewDistribution, NSTextField, NSTrackingArea,
    NSTrackingAreaOptions, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSNumber, NSObjectProtocol, NSPoint, NSRect};
use splat_i18n::{t, tf};

use super::Side;
use crate::util::{constant, guarded, item, ns, separator, set_target, symbol, symbol_button};
use crate::window::tabs::Document;
use crate::window::{TabId, WindowController};

pub(super) const BAR_HEIGHT: f64 = 30.0;
const TAB_HEIGHT: f64 = 22.0;
const MIN_TAB_WIDTH: f64 = 90.0;
const MAX_TAB_WIDTH: f64 = 240.0;
const TITLE_POINTS: f64 = 12.0;

pub struct TabIvars {
    id: TabId,
    target: Weak<WindowController>,
    title: Retained<NSTextField>,
    icon: Retained<NSImageView>,
    close: Retained<NSButton>,
    /// The dot that tells whether an agent works or waits.
    status: Retained<NSImageView>,
    selected: Cell<bool>,
    hovered: Cell<bool>,
    dirty: Cell<bool>,
    preview: Cell<bool>,
    /// Whether the mouse moved since it went down on the tab, so letting go drops it.
    dragged: Cell<bool>,
    /// Whether the tab's pane is the active one, where its selected tab shows the accent.
    active_bar: Cell<bool>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `PaneTab` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEPaneTab"]
    #[ivars = TabIvars]
    pub struct PaneTab;

    unsafe impl NSObjectProtocol for PaneTab {}

    impl PaneTab {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: NSRect) {
            guarded((), || self.draw_background());
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || {
                self.ivars().dragged.set(false);
                let count = event.clickCount();
                self.with_target(|target, id| target.pane_tab_clicked(id, count));
            });
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            guarded((), || {
                self.ivars().dragged.set(true);
                let point = event.locationInWindow();
                self.with_target(|target, id| target.drag_pane_tab(id, point));
            });
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            guarded((), || {
                if self.ivars().dragged.replace(false) {
                    let point = event.locationInWindow();
                    self.with_target(|target, id| target.drop_pane_tab(id, point));
                }
            });
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, _event: &NSEvent) {
            guarded((), || self.with_target(|target, id| target.close_tab(id)));
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            guarded((), || self.set_hovered(true));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            guarded((), || self.set_hovered(false));
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            guarded((), || self.track());
            // SAFETY: The superclass's implementation, with this method's signature.
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
        }

        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, _event: &NSEvent) -> Option<Retained<NSMenu>> {
            guarded(None, || Some(self.context_menu()))
        }

        #[unsafe(method(ideKeepThisTab:))]
        fn keep_this(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.keep_tab(id)));
        }

        #[unsafe(method(ideCloseThisTab:))]
        fn close_this(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.close_tab(id)));
        }

        #[unsafe(method(ideCloseOtherTabs:))]
        fn close_others(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.close_other_pane_tabs(id)));
        }

        #[unsafe(method(ideSplitTabRight:))]
        fn split_right(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.split_off(id, Side::Right)));
        }

        #[unsafe(method(ideSplitTabBelow:))]
        fn split_below(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.split_off(id, Side::Below)));
        }

        #[unsafe(method(ideMoveTabToWindowTab:))]
        fn move_to_window_tab(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.move_to_window_tab(id)));
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_press(&self) -> bool {
            guarded(false, || {
                self.with_target(|target, id| target.pane_tab_clicked(id, 1));
                true
            })
        }
    }
);

impl PaneTab {
    fn new(mtm: MainThreadMarker, target: &WindowController, id: TabId) -> Retained<PaneTab> {
        let title = NSTextField::labelWithString(&ns(""), mtm);
        title.setFont(Some(&NSFont::systemFontOfSize(TITLE_POINTS)));
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        title.setContentCompressionResistancePriority_forOrientation(
            200.0,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        let close = symbol_button(
            mtm,
            "xmark",
            t("Schließen"),
            9.0,
            Some(target),
            sel!(ideCloseTabButton:),
        );
        close.setTag(id.get());
        let status = NSImageView::new(mtm);
        status.setImage(symbol("circle.fill", "").as_deref());
        status.setSymbolConfiguration(Some(
            &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
                6.0,
                // SAFETY: AppKit sets its font weight constants when it loads.
                unsafe { objc2_app_kit::NSFontWeightRegular },
            ),
        ));
        status.setHidden(true);
        let icon = NSImageView::new(mtm);
        crate::util::hide_from_voiceover(&icon);
        crate::util::hide_from_voiceover(&status);
        let ivars = TabIvars {
            id,
            target: Weak::from(target),
            title,
            icon,
            close,
            status,
            selected: Cell::new(false),
            hovered: Cell::new(false),
            dirty: Cell::new(false),
            preview: Cell::new(false),
            dragged: Cell::new(false),
            active_bar: Cell::new(false),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.lay_out();
        this.setAccessibilityElement(true);
        this.setAccessibilityRole(Some(constant!(NSAccessibilityRadioButtonRole)));
        this.setAccessibilitySubrole(Some(constant!(NSAccessibilityTabButtonSubrole)));
        this.speak_selected(false);
        this
    }

    fn lay_out(&self) {
        let ivars = self.ivars();
        let stack = NSStackView::new(self.mtm());
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(ide_ui_common::style::space::SNUG);
        for view in [&*ivars.close as &NSView, &ivars.icon, &ivars.title, &ivars.status] {
            stack.addArrangedSubview(view);
        }
        self.addSubview(&stack);
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        let widest = self.widthAnchor().constraintLessThanOrEqualToConstant(MAX_TAB_WIDTH);
        widest.setPriority(750.0);
        for constraint in [
            stack.centerYAnchor().constraintEqualToAnchor(&self.centerYAnchor()),
            stack.centerXAnchor().constraintEqualToAnchor(&self.centerXAnchor()),
            stack
                .leadingAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&self.leadingAnchor(), 6.0),
            stack
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&self.trailingAnchor(), -6.0),
            self.heightAnchor().constraintEqualToConstant(TAB_HEIGHT),
            self.widthAnchor().constraintGreaterThanOrEqualToConstant(MIN_TAB_WIDTH),
            widest,
        ] {
            constraint.setActive(true);
        }
    }

    pub(super) fn id(&self) -> TabId {
        self.ivars().id
    }

    /// Shows what `document` is: its icon, title, unsaved changes, whether it is the preview, and its state.
    fn show(&self, document: &Document, selected: bool, active: bool) {
        let ivars = self.ivars();
        let title = document.title();
        if ivars.title.stringValue().to_string() != title {
            ivars.title.setStringValue(&ns(&title));
            let close = tf("{} schließen", &[&title]);
            ivars.close.setToolTip(Some(&ns(&close)));
            crate::util::set_label(&ivars.close, Some(&close));
        }
        ivars.icon.setImage(document.icon().as_deref());
        ivars.dirty.set(document.dirty());
        let preview = document.is_preview();
        if ivars.preview.replace(preview) != preview {
            let regular = NSFont::systemFontOfSize(TITLE_POINTS);
            let font = if preview {
                NSFontManager::sharedFontManager(self.mtm())
                    .convertFont_toHaveTrait(&regular, NSFontTraitMask::ItalicFontMask)
            } else {
                regular
            };
            ivars.title.setFont(Some(&font));
        }
        let status = document.status_colour();
        ivars.status.setHidden(status.is_none());
        if let Some(color) = &status {
            ivars.status.setContentTintColor(Some(color));
        }
        ivars.selected.set(selected);
        ivars.active_bar.set(active);
        self.speak_selected(selected);
        self.describe();
        self.refresh();
    }

    fn with_target(&self, f: impl FnOnce(&WindowController, TabId)) {
        if let Some(target) = self.ivars().target.load() {
            f(&target, self.ivars().id);
        }
    }

    fn set_hovered(&self, hovered: bool) {
        self.ivars().hovered.set(hovered);
        self.refresh();
    }

    /// VoiceOver reads a tab as on or off from its value, as it reads AppKit's own tabs.
    fn speak_selected(&self, selected: bool) {
        self.setAccessibilitySelected(selected);
        // SAFETY: A radio button's value is a number, the type AppKit expects of it.
        unsafe { self.setAccessibilityValue(Some(&NSNumber::new_bool(selected))) };
    }

    fn describe(&self) {
        let ivars = self.ivars();
        let title = ivars.title.stringValue().to_string();
        if ivars.preview.get() {
            self.setAccessibilityLabel(Some(&ns(&tf("{}, Vorschau", &[&title]))));
            self.setToolTip(Some(&ns(t(
                "Vorschau: Die nächste Datei, die du mit einem Klick öffnest, ersetzt diesen Tab. Ein Doppelklick behält ihn.",
            ))));
        } else {
            self.setAccessibilityLabel(Some(&ns(&title)));
            self.setToolTip(None);
        }
    }

    /// The close symbol shows on hover; a dot stands for unsaved changes otherwise; the title is fainter when the tab
    /// is in the background.
    fn refresh(&self) {
        let ivars = self.ivars();
        let (name, visible) = match (ivars.hovered.get(), ivars.dirty.get()) {
            (true, _) => ("xmark", true),
            (false, true) => ("circle.fill", true),
            (false, false) => ("xmark", false),
        };
        ivars.close.setImage(symbol(name, t("Schließen")).as_deref());
        ivars.close.setHidden(!visible);
        ivars.close.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let color = match (ivars.selected.get(), ivars.active_bar.get()) {
            (true, true) => NSColor::controlAccentColor(),
            (true, false) => NSColor::labelColor(),
            (false, _) => NSColor::secondaryLabelColor(),
        };
        ivars.title.setTextColor(Some(&color));
        self.setNeedsDisplay(true);
    }

    fn draw_background(&self) {
        let ivars = self.ivars();
        let fill = if ivars.selected.get() && ivars.active_bar.get() {
            NSColor::controlAccentColor().colorWithAlphaComponent(0.16)
        } else if ivars.selected.get() {
            NSColor::quaternaryLabelColor()
        } else if ivars.hovered.get() {
            NSColor::quinaryLabelColor()
        } else {
            return;
        };
        fill.setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), 6.0, 6.0).fill();
    }

    fn track(&self) {
        for area in self.trackingAreas().iter() {
            self.removeTrackingArea(&area);
        }
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY: The tracking area sends its events to this view, which answers them, and lives as long as the view.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                self.bounds(),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
    }

    fn context_menu(&self) -> Retained<NSMenu> {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let add = |title: &str, action, enabled: bool| {
            let entry = item(mtm, title, Some(action), "");
            set_target(&entry, self);
            entry.setEnabled(enabled);
            menu.addItem(&entry);
        };
        if self.ivars().preview.get() {
            add(t("Tab behalten"), sel!(ideKeepThisTab:), true);
            menu.addItem(&separator(mtm));
        }
        let several = self
            .ivars()
            .target
            .load()
            .is_some_and(|target| target.pane_holds_more(self.ivars().id));
        add(t("Tab schließen"), sel!(ideCloseThisTab:), true);
        add(t("Andere Tabs schließen"), sel!(ideCloseOtherTabs:), several);
        menu.addItem(&separator(mtm));
        add(t("Rechts teilen"), sel!(ideSplitTabRight:), several);
        add(t("Unten teilen"), sel!(ideSplitTabBelow:), several);
        add(t("In eigenen Tab verschieben"), sel!(ideMoveTabToWindowTab:), true);
        menu
    }
}

/// The accent over the part of a pane, or the bar, a dragged tab would drop into.
pub(super) fn drop_highlight(mtm: MainThreadMarker) -> Retained<NSBox> {
    let drop = NSBox::new(mtm);
    drop.setBoxType(NSBoxType::Custom);
    drop.setBorderWidth(0.0);
    drop.setCornerRadius(6.0);
    drop.setFillColor(&NSColor::controlAccentColor().colorWithAlphaComponent(0.18));
    drop.setTitlePosition(objc2_app_kit::NSTitlePosition::NoTitle);
    drop.setHidden(true);
    drop
}

pub(super) struct PaneBar {
    pub(super) view: Retained<NSView>,
    strip: Retained<NSStackView>,
    items: RefCell<Vec<Retained<PaneTab>>>,
    drop: Retained<NSBox>,
    height: Retained<NSLayoutConstraint>,
}

impl PaneBar {
    pub(super) fn new(mtm: MainThreadMarker) -> PaneBar {
        let view = NSView::new(mtm);
        view.setAccessibilityElement(true);
        view.setAccessibilityRole(Some(constant!(NSAccessibilityTabGroupRole)));
        view.setAccessibilityLabel(Some(&ns(t("Tabs"))));
        let strip = NSStackView::new(mtm);
        strip.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        strip.setDistribution(NSStackViewDistribution::FillProportionally);
        strip.setSpacing(ide_ui_common::style::space::XS);
        let drop = drop_highlight(mtm);
        view.addSubview(&drop);
        view.addSubview(&strip);
        for part in [&*strip as &NSView, &drop] {
            part.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        let height = view.heightAnchor().constraintEqualToConstant(0.0);
        let trailing = strip
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&view.trailingAnchor(), -8.0);
        for constraint in [
            drop.leadingAnchor()
                .constraintEqualToAnchor_constant(&view.leadingAnchor(), 4.0),
            drop.trailingAnchor()
                .constraintEqualToAnchor_constant(&view.trailingAnchor(), -4.0),
            drop.topAnchor()
                .constraintEqualToAnchor_constant(&view.topAnchor(), 2.0),
            drop.bottomAnchor()
                .constraintEqualToAnchor_constant(&view.bottomAnchor(), -2.0),
            strip
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&view.leadingAnchor(), 6.0),
            strip.centerYAnchor().constraintEqualToAnchor(&view.centerYAnchor()),
            trailing,
            height.clone(),
        ] {
            constraint.setActive(true);
        }
        view.setHidden(true);
        PaneBar {
            view,
            strip,
            items: RefCell::new(Vec::new()),
            drop,
            height,
        }
    }

    /// Shows the bar, or folds it away to no height.
    pub(super) fn set_shown(&self, shown: bool) {
        if self.view.isHidden() == shown {
            self.view.setHidden(!shown);
            self.height.setConstant(if shown { BAR_HEIGHT } else { 0.0 });
        }
    }

    /// Names the bar for VoiceOver, such as "Bereich 2 von 3".
    pub(super) fn set_label(&self, label: &str) {
        self.view.setAccessibilityLabel(Some(&ns(label)));
    }

    /// Gives the bar a tab for each of `documents`, in their order, `selected` the one in front; `active` for the
    /// active pane of several.
    pub(super) fn show(&self, target: &WindowController, documents: &[Rc<Document>], selected: usize, active: bool) {
        let mut items = self.items.borrow_mut();
        let same = items.len() == documents.len() && items.iter().zip(documents).all(|(item, doc)| item.id() == doc.id);
        if !same {
            for item in items.iter() {
                self.strip.removeArrangedSubview(item);
            }
            let mut kept: Vec<Retained<PaneTab>> = Vec::with_capacity(documents.len());
            for doc in documents {
                let found = items.iter().position(|item| item.id() == doc.id);
                let item = match found {
                    Some(index) => items.remove(index),
                    None => PaneTab::new(target.mtm(), target, doc.id),
                };
                kept.push(item);
            }
            for gone in items.drain(..) {
                gone.removeFromSuperview();
            }
            for item in &kept {
                self.strip.addArrangedSubview(item);
            }
            *items = kept;
        }
        for (index, (item, doc)) in items.iter().zip(documents).enumerate() {
            item.show(doc, index == selected, active);
        }
    }

    /// Shows or hides the accent that tells a dragged tab would drop into this bar.
    pub(super) fn highlight(&self, on: bool) {
        self.drop.setHidden(!on);
    }

    /// Whether `point`, in window coordinates, lies on the bar while it shows.
    pub(super) fn holds(&self, point: NSPoint) -> bool {
        let local = self.view.convertPoint_fromView(point, None);
        !self.view.isHidden() && objc2_foundation::NSPointInRect(local, self.view.bounds())
    }

    /// Where among the tabs one dropped at `point`, in window coordinates, goes, leaving out the tab `id`.
    pub(super) fn index_at(&self, point: NSPoint, id: TabId) -> usize {
        let x = self.strip.convertPoint_fromView(point, None).x;
        self.items
            .borrow()
            .iter()
            .filter(|item| item.id() != id)
            .filter(|item| {
                let frame = item.frame();
                x > frame.origin.x + frame.size.width / 2.0
            })
            .count()
    }
}
