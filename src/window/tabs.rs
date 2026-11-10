//! The tab bar of the content area, drawn like Xcode's editor tabs, with its "+" menu, and the view of the selected
//! tab below it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityRadioButtonRole, NSAccessibilityTabButtonSubrole, NSAccessibilityTabGroupRole,
    NSBezierPath, NSBox, NSBoxType, NSButton, NSColor, NSEvent, NSFont, NSFontManager, NSFontTraitMask,
    NSImageSymbolConfiguration, NSImageView, NSLineBreakMode, NSMenu, NSMenuItem, NSStackView, NSStackViewDistribution,
    NSTextField, NSTrackingArea, NSTrackingAreaOptions, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSNumber, NSObjectProtocol, NSPoint, NSRect};
use splat_i18n::{t, tf};

use super::WindowController;
use super::panes::Side;
use crate::util::{constant, guarded, item, ns, pin_edges, separator, set_target, submenu, symbol, symbol_button};

const BAR_HEIGHT: f64 = 32.0;
const TAB_HEIGHT: f64 = 24.0;
const MIN_TAB_WIDTH: f64 = 90.0;
const TITLE_POINTS: f64 = 12.0;

/// What a tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Welcome,
    Editor,
    Terminal,
    Browser,
    Agent,
    Diff,
    Log,
    /// A coding agent's CLI in a terminal (I5).
    Cli,
    /// A plan an agent asks to approve, as a document to comment on.
    Plan,
    /// A document as it reads, beside its editor: Markdown rendered, or a PDF file.
    Preview,
}

impl TabKind {
    fn symbol(self) -> &'static str {
        match self {
            TabKind::Welcome => "house",
            TabKind::Editor => "doc.text",
            TabKind::Terminal => "terminal",
            TabKind::Browser => "globe",
            TabKind::Agent => "sparkle",
            TabKind::Diff => "plusminus",
            TabKind::Log => "doc.plaintext",
            TabKind::Cli => "apple.terminal",
            TabKind::Plan => "list.bullet.clipboard",
            TabKind::Preview => "eye",
        }
    }
}

pub struct ItemIvars {
    id: isize,
    target: Weak<WindowController>,
    title: Retained<NSTextField>,
    close: Retained<NSButton>,
    /// The dot that tells whether an agent works or waits, before the close button.
    status: Retained<NSImageView>,
    /// The symbol of the tab's kind, tinted in an agent's colour.
    icon: std::cell::RefCell<Option<Retained<NSImageView>>>,
    selected: Cell<bool>,
    hovered: Cell<bool>,
    dirty: Cell<bool>,
    /// Whether the tab is its panel's preview tab, which the next file opened by a single click replaces.
    preview: Cell<bool>,
    /// Whether the mouse moved since it went down on the tab, so letting go drops it.
    dragged: Cell<bool>,
    /// Whether the tab's bar belongs to the active one of several panels, where its selected tab shows the accent.
    active_bar: Cell<bool>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `TabItem` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETabItem"]
    #[ivars = ItemIvars]
    pub struct TabItem;

    unsafe impl NSObjectProtocol for TabItem {}

    impl TabItem {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: NSRect) {
            guarded((), || self.draw_background());
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || {
                self.ivars().dragged.set(false);
                self.clicked(event.clickCount());
            });
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            guarded((), || {
                self.ivars().dragged.set(true);
                let point = event.locationInWindow();
                self.with_target(|target, id| target.drag_tab(id, point));
            });
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            guarded((), || {
                if self.ivars().dragged.replace(false) {
                    let point = event.locationInWindow();
                    self.with_target(|target, id| target.drop_tab(id, point));
                }
            });
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, _event: &NSEvent) {
            guarded((), || {
                if let Some(target) = self.ivars().target.load() {
                    target.close_tab_by_id(self.ivars().id);
                }
            });
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
            guarded(None, || self.context_menu())
        }

        #[unsafe(method(ideKeepThisTab:))]
        fn keep_this(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.keep_tab(id)));
        }

        #[unsafe(method(ideCloseThisTab:))]
        fn close_this(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.close_tab_by_id(id)));
        }

        #[unsafe(method(ideCloseOtherTabs:))]
        fn close_others(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.close_other_tabs(id, false)));
        }

        #[unsafe(method(ideCloseTabsToTheRight:))]
        fn close_right(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.close_other_tabs(id, true)));
        }

        #[unsafe(method(ideSplitRight:))]
        fn split_right(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.split_tab(id, Side::Right)));
        }

        #[unsafe(method(ideSplitBelow:))]
        fn split_below(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_target(|target, id| target.split_tab(id, Side::Below)));
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_press(&self) -> bool {
            guarded(false, || {
                self.clicked(1);
                true
            })
        }
    }
);

impl TabItem {
    fn new(
        mtm: MainThreadMarker,
        target: &WindowController,
        id: isize,
        kind: TabKind,
        title: &str,
    ) -> Retained<TabItem> {
        let label = NSTextField::labelWithString(&ns(title), mtm);
        label.setFont(Some(&NSFont::systemFontOfSize(TITLE_POINTS)));
        label.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        let close = symbol_button(
            mtm,
            "xmark",
            &tf("{} schließen", &[title]),
            9.0,
            Some(target),
            sel!(ideCloseTabButton:),
        );
        close.setTag(id);
        let status = NSImageView::new(mtm);
        status.setImage(symbol("circle.fill", "").as_deref());
        status.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
            6.0,
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightRegular },
        )));
        status.setHidden(true);
        let ivars = ItemIvars {
            id,
            target: Weak::from(target),
            title: label,
            close,
            status,
            icon: std::cell::RefCell::new(None),
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
        let icon = symbol(kind.symbol(), "").map(|image| NSImageView::imageViewWithImage(&image, mtm));
        let ivars = this.ivars();
        let mut views: Vec<&NSView> = vec![&ivars.close];
        if let Some(icon) = &icon {
            let tint = if kind == TabKind::Agent {
                crate::agent::style::terracotta()
            } else {
                NSColor::secondaryLabelColor()
            };
            icon.setContentTintColor(Some(&tint));
            views.push(icon);
        }
        ivars.icon.replace(icon.clone());
        views.push(&ivars.title);
        views.push(&ivars.status);
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(5.0);
        for view in views {
            stack.addArrangedSubview(view);
        }
        this.addSubview(&stack);
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            stack.centerYAnchor().constraintEqualToAnchor(&this.centerYAnchor()),
            stack.centerXAnchor().constraintEqualToAnchor(&this.centerXAnchor()),
            stack
                .leadingAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&this.leadingAnchor(), 6.0),
            stack
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&this.trailingAnchor(), -6.0),
            this.heightAnchor().constraintEqualToConstant(TAB_HEIGHT),
            this.widthAnchor().constraintGreaterThanOrEqualToConstant(MIN_TAB_WIDTH),
        ] {
            constraint.setActive(true);
        }
        ivars.title.setContentCompressionResistancePriority_forOrientation(
            200.0,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        this.setAccessibilityElement(true);
        this.setAccessibilityRole(Some(constant!(NSAccessibilityRadioButtonRole)));
        this.setAccessibilitySubrole(Some(constant!(NSAccessibilityTabButtonSubrole)));
        this.setAccessibilityLabel(Some(&ns(title)));
        this.speak_selected(false);
        this.refresh();
        this
    }

    /// A click selects the tab; a double click also keeps a preview tab.
    fn clicked(&self, count: isize) {
        self.with_target(|target, id| {
            if count >= 2 {
                target.keep_tab(id);
            }
            target.click_tab(id);
        });
    }

    fn with_target(&self, f: impl FnOnce(&WindowController, isize)) {
        if let Some(target) = self.ivars().target.load() {
            f(&target, self.ivars().id);
        }
    }

    fn set_hovered(&self, hovered: bool) {
        self.ivars().hovered.set(hovered);
        self.refresh();
    }

    fn set_selected(&self, selected: bool) {
        self.ivars().selected.set(selected);
        self.speak_selected(selected);
        self.refresh();
    }

    /// VoiceOver reads a tab as on or off from its value, as it reads AppKit's own tabs.
    fn speak_selected(&self, selected: bool) {
        self.setAccessibilitySelected(selected);
        // SAFETY: A radio button's value is a number, the type AppKit expects of it.
        unsafe { self.setAccessibilityValue(Some(&NSNumber::new_bool(selected))) };
    }

    fn set_dirty(&self, dirty: bool) {
        self.ivars().dirty.set(dirty);
        self.refresh();
    }

    fn set_status(&self, color: Option<&NSColor>) {
        let status = &self.ivars().status;
        status.setHidden(color.is_none());
        if let Some(color) = color {
            status.setContentTintColor(Some(color));
        }
    }

    fn set_tint(&self, color: &NSColor) {
        if let Some(icon) = self.ivars().icon.borrow().as_ref() {
            icon.setContentTintColor(Some(color));
        }
    }

    fn set_title(&self, title: &str) {
        let ivars = self.ivars();
        ivars.title.setStringValue(&ns(title));
        let close = tf("{} schließen", &[title]);
        ivars.close.setToolTip(Some(&ns(&close)));
        crate::util::set_label(&ivars.close, Some(&close));
        self.describe();
    }

    /// Shows the title in italics while the tab is the preview tab, which VoiceOver and the tooltip name.
    fn set_preview(&self, preview: bool) {
        let ivars = self.ivars();
        ivars.preview.set(preview);
        let regular = NSFont::systemFontOfSize(TITLE_POINTS);
        let font = if preview {
            NSFontManager::sharedFontManager(self.mtm())
                .convertFont_toHaveTrait(&regular, NSFontTraitMask::ItalicFontMask)
        } else {
            regular
        };
        ivars.title.setFont(Some(&font));
        self.describe();
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
        let hovered = ivars.hovered.get();
        let (name, visible) = match (hovered, ivars.dirty.get()) {
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

    fn set_active_bar(&self, active: bool) {
        if self.ivars().active_bar.replace(active) != active {
            self.refresh();
        }
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

    fn context_menu(&self) -> Option<Retained<NSMenu>> {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        if self.ivars().preview.get() {
            let keep = item(mtm, t("Tab behalten"), Some(sel!(ideKeepThisTab:)), "");
            set_target(&keep, self);
            menu.addItem(&keep);
            menu.addItem(&separator(mtm));
        }
        for (title, action) in [
            (t("Tab schließen"), sel!(ideCloseThisTab:)),
            (t("Andere Tabs schließen"), sel!(ideCloseOtherTabs:)),
            (t("Tabs rechts schließen"), sel!(ideCloseTabsToTheRight:)),
        ] {
            let entry = item(mtm, title, Some(action), "");
            set_target(&entry, self);
            menu.addItem(&entry);
        }
        menu.addItem(&separator(mtm));
        let movable = self
            .ivars()
            .target
            .load()
            .is_some_and(|target| target.can_split(self.ivars().id));
        for (title, action) in [
            (t("In eigenem Panel rechts öffnen"), sel!(ideSplitRight:)),
            (t("In eigenem Panel unten öffnen"), sel!(ideSplitBelow:)),
        ] {
            let entry = item(mtm, title, Some(action), "");
            set_target(&entry, self);
            entry.setEnabled(movable);
            menu.addItem(&entry);
        }
        let path = self
            .ivars()
            .target
            .load()
            .and_then(|target| crate::editor::tab_document(&target, self.ivars().id)?.0);
        if let Some(entry) = path.and_then(|path| crate::markdown::open_in_splat_item(mtm, &path)) {
            menu.addItem(&separator(mtm));
            menu.addItem(&entry);
        }
        Some(menu)
    }
}

/// A tab of a bar, which moves to another bar whole, its view still running.
pub struct Tab {
    id: isize,
    kind: TabKind,
    /// What the tab shows, such as a file's path, so it opens once.
    key: Option<String>,
    item: Retained<TabItem>,
    view: Retained<NSView>,
}

pub struct TabBar {
    mtm: MainThreadMarker,
    target: Weak<WindowController>,
    bar: Retained<NSView>,
    strip: Retained<NSStackView>,
    plus: Retained<NSButton>,
    content: Retained<NSView>,
    menu: Retained<NSMenu>,
    tabs: RefCell<Vec<Tab>>,
    selected: Cell<Option<isize>>,
    /// The numbers of the tabs of the window, shared by its bars, so a tab keeps its number in another panel.
    next: Rc<Cell<isize>>,
    /// The accent the bar shows while a dragged tab would drop into it.
    drop: Retained<NSBox>,
    /// Whether the bar belongs to the active one of several panels.
    active: Cell<bool>,
}

impl TabBar {
    /// The bar of the panel numbered `panel`, whose "+" says so in its tag; `next` numbers its tabs.
    pub fn new(mtm: MainThreadMarker, target: &WindowController, panel: usize, next: Rc<Cell<isize>>) -> TabBar {
        let bar = NSView::new(mtm);
        bar.setAccessibilityElement(true);
        bar.setAccessibilityRole(Some(constant!(NSAccessibilityTabGroupRole)));
        bar.setAccessibilityLabel(Some(&ns(t("Tabs"))));
        let strip = NSStackView::new(mtm);
        strip.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        strip.setDistribution(NSStackViewDistribution::FillEqually);
        strip.setSpacing(4.0);
        let plus = symbol_button(mtm, "plus", t("Neuer Tab"), 13.0, Some(target), sel!(ideNewTab:));
        plus.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        plus.setTag(isize::try_from(panel).unwrap_or(0));
        // A stack view, so the "+" follows the tabs in the reading direction, as the tabs in the strip do.
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setDistribution(NSStackViewDistribution::Fill);
        row.setAlignment(objc2_app_kit::NSLayoutAttribute::CenterY);
        row.setSpacing(6.0);
        row.addArrangedSubview(&strip);
        row.addArrangedSubview(&plus);
        strip.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Horizontal);
        let drop = drop_highlight(mtm);
        bar.addSubview(&drop);
        bar.addSubview(&row);
        for view in [&*row as &NSView, &drop] {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        for constraint in [
            drop.leadingAnchor()
                .constraintEqualToAnchor_constant(&bar.leadingAnchor(), 4.0),
            drop.trailingAnchor()
                .constraintEqualToAnchor_constant(&bar.trailingAnchor(), -4.0),
            drop.topAnchor().constraintEqualToAnchor_constant(&bar.topAnchor(), 2.0),
            drop.bottomAnchor()
                .constraintEqualToAnchor_constant(&bar.bottomAnchor(), -2.0),
            bar.heightAnchor().constraintEqualToConstant(BAR_HEIGHT),
            row.leadingAnchor()
                .constraintEqualToAnchor_constant(&bar.leadingAnchor(), 8.0),
            row.centerYAnchor().constraintEqualToAnchor(&bar.centerYAnchor()),
            row.trailingAnchor()
                .constraintEqualToAnchor_constant(&bar.trailingAnchor(), -10.0),
        ] {
            constraint.setActive(true);
        }
        TabBar {
            mtm,
            target: Weak::from(target),
            bar,
            strip,
            plus,
            content: NSView::new(mtm),
            menu: new_tab_menu(mtm),
            tabs: RefCell::new(vec![]),
            selected: Cell::new(None),
            next,
            drop,
            active: Cell::new(false),
        }
    }

    /// Names the bar for VoiceOver, such as "Panel 2 von 3".
    pub fn set_label(&self, label: &str) {
        self.bar.setAccessibilityLabel(Some(&ns(label)));
    }

    /// Marks the bar as the active one of several panels: its selected tab shows the accent colour.
    pub fn set_active(&self, active: bool) {
        self.active.set(active);
        for tab in self.tabs.borrow().iter() {
            tab.item.set_active_bar(active);
        }
    }

    /// Shows or hides the accent that tells a dragged tab would drop into this bar.
    pub fn highlight(&self, on: bool) {
        self.drop.setHidden(!on);
    }

    /// Whether `point`, in window coordinates, lies on the bar.
    pub fn holds(&self, point: NSPoint) -> bool {
        let local = self.bar.convertPoint_fromView(point, None);
        self.bar.window().is_some() && objc2_foundation::NSPointInRect(local, self.bar.bounds())
    }

    /// Where in the strip a tab dropped at `point`, in window coordinates, goes.
    pub fn index_at(&self, point: NSPoint) -> usize {
        let x = self.strip.convertPoint_fromView(point, None).x;
        self.tabs
            .borrow()
            .iter()
            .filter(|tab| {
                let frame = tab.item.frame();
                x > frame.origin.x + frame.size.width / 2.0
            })
            .count()
    }

    /// Takes the tab `id` out of the bar, its view still where it was until another bar adopts it, and selects a
    /// neighbour.
    pub fn take(&self, id: isize) -> Option<Tab> {
        let (tab, next) = {
            let mut tabs = self.tabs.borrow_mut();
            let position = tabs.iter().position(|tab| tab.id == id)?;
            let tab = tabs.remove(position);
            let next = tabs.get(position).or_else(|| tabs.last()).map(|tab| tab.id);
            (tab, next)
        };
        self.strip.removeArrangedSubview(&tab.item);
        tab.item.removeFromSuperview();
        tab.item.set_hovered(false);
        if self.selected.get() == Some(id) {
            self.selected.set(None);
            if let Some(next) = next {
                self.select(next);
            }
        }
        Some(tab)
    }

    /// Takes in `tab` from another bar at `position`, or at the end, and selects it; a moved preview tab is kept.
    pub fn adopt(&self, tab: Tab, position: Option<usize>) {
        let id = tab.id;
        tab.item.set_preview(false);
        tab.item.set_active_bar(self.active.get());
        let position = position.unwrap_or(usize::MAX).min(self.tabs.borrow().len());
        self.strip
            .insertArrangedSubview_atIndex(&tab.item, isize::try_from(position).unwrap_or(0));
        self.content.addSubview(&tab.view);
        pin_edges(&tab.view, &self.content);
        self.tabs.borrow_mut().insert(position, tab);
        self.select(id);
    }

    /// The tabs, left to right: their number, kind and key.
    pub fn entries(&self) -> Vec<(isize, TabKind, Option<String>)> {
        self.tabs
            .borrow()
            .iter()
            .map(|tab| (tab.id, tab.kind, tab.key.clone()))
            .collect()
    }

    /// The view of the selected tab.
    pub fn selected_view(&self) -> Option<Retained<NSView>> {
        let selected = self.selected.get()?;
        self.tabs
            .borrow()
            .iter()
            .find(|tab| tab.id == selected)
            .map(|tab| tab.view.clone())
    }

    pub fn view(&self) -> &NSView {
        &self.bar
    }

    pub fn content(&self) -> &NSView {
        &self.content
    }

    pub fn count(&self) -> usize {
        self.tabs.borrow().len()
    }

    pub fn selected_id(&self) -> Option<isize> {
        self.selected.get()
    }

    pub fn find(&self, kind: TabKind) -> Option<isize> {
        self.tabs.borrow().iter().find(|tab| tab.kind == kind).map(|tab| tab.id)
    }

    pub fn find_key(&self, key: &str) -> Option<isize> {
        self.tabs
            .borrow()
            .iter()
            .find(|tab| tab.key.as_deref() == Some(key))
            .map(|tab| tab.id)
    }

    /// The ids of all tabs, left to right.
    pub fn ids(&self) -> Vec<isize> {
        self.tabs.borrow().iter().map(|tab| tab.id).collect()
    }

    /// The ids and kinds of all tabs, left to right.
    pub fn kinds(&self) -> Vec<(isize, TabKind)> {
        self.tabs.borrow().iter().map(|tab| (tab.id, tab.kind)).collect()
    }

    /// Adds a tab titled `title` that shows `view`, and selects it.
    pub fn add(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        self.insert(kind, key, title, view, None)
    }

    /// Adds the preview tab, in place of the tab `replace` when given, and selects it.
    pub fn add_preview(
        &self,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
        replace: Option<isize>,
    ) -> isize {
        let position = replace.and_then(|id| self.remove(id));
        let id = self.insert(kind, key, title, view, position);
        if let Some(tab) = self.tabs.borrow().iter().find(|tab| tab.id == id) {
            tab.item.set_preview(true);
        }
        id
    }

    fn insert(
        &self,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
        position: Option<usize>,
    ) -> isize {
        let id = self.next.get();
        self.next.set(id + 1);
        let Some(target) = self.target.load() else { return id };
        let item = TabItem::new(self.mtm, &target, id, kind, title);
        item.set_active_bar(self.active.get());
        let position = position.unwrap_or_else(|| self.tabs.borrow().len());
        self.strip
            .insertArrangedSubview_atIndex(&item, isize::try_from(position).unwrap_or(0));
        self.content.addSubview(&view);
        pin_edges(&view, &self.content);
        view.setHidden(true);
        let tab = Tab {
            id,
            kind,
            key,
            item,
            view,
        };
        let mut tabs = self.tabs.borrow_mut();
        let position = position.min(tabs.len());
        tabs.insert(position, tab);
        drop(tabs);
        self.select(id);
        id
    }

    /// The window's preview tab, if it has one.
    pub fn preview_id(&self) -> Option<isize> {
        self.tabs
            .borrow()
            .iter()
            .find(|tab| tab.item.ivars().preview.get())
            .map(|tab| tab.id)
    }

    /// Makes the tab `id` an ordinary one, which no other file replaces.
    pub fn keep(&self, id: isize) {
        if let Some(tab) = self
            .tabs
            .borrow()
            .iter()
            .find(|tab| tab.id == id && tab.item.ivars().preview.get())
        {
            tab.item.set_preview(false);
        }
    }

    /// Moves the tab `id` into the gap the pointer at `point`, in window coordinates, falls into.
    pub fn drag_to(&self, id: isize, point: NSPoint) {
        self.drag(id, self.strip.convertPoint_fromView(point, None).x);
    }

    /// Moves the tab `id` into the gap the pointer at `x`, in the strip's coordinates, falls into; a dragged preview
    /// tab is kept.
    pub fn drag(&self, id: isize, x: f64) {
        self.keep(id);
        let mut tabs = self.tabs.borrow_mut();
        let Some(from) = tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let to = tabs
            .iter()
            .filter(|tab| tab.id != id)
            .filter(|tab| {
                let frame = tab.item.frame();
                x > frame.origin.x + frame.size.width / 2.0
            })
            .count();
        if to == from {
            return;
        }
        let tab = tabs.remove(from);
        self.strip.removeArrangedSubview(&tab.item);
        self.strip
            .insertArrangedSubview_atIndex(&tab.item, isize::try_from(to).unwrap_or(0));
        tabs.insert(to, tab);
        crate::session::changed();
    }

    /// Shows a dot of `color` on the tab `id`, or none.
    pub fn set_status(&self, id: isize, color: Option<&NSColor>) {
        if let Some(tab) = self.tabs.borrow().iter().find(|tab| tab.id == id) {
            tab.item.set_status(color);
        }
    }

    /// Tints the symbol of the tab `id`, such as in its agent's colour.
    pub fn set_tint(&self, id: isize, color: &NSColor) {
        if let Some(tab) = self.tabs.borrow().iter().find(|tab| tab.id == id) {
            tab.item.set_tint(color);
        }
    }

    /// Gives the tab `id` a new title, such as the terminal's.
    pub fn set_title(&self, id: isize, title: &str) {
        if let Some(tab) = self.tabs.borrow().iter().find(|tab| tab.id == id) {
            tab.item.set_title(title);
        }
    }

    /// Shows a dot instead of the close symbol while the tab has unsaved changes; the first change keeps a preview
    /// tab.
    pub fn set_dirty(&self, id: isize, dirty: bool) {
        if dirty {
            self.keep(id);
        }
        if let Some(tab) = self.tabs.borrow().iter().find(|tab| tab.id == id) {
            tab.item.set_dirty(dirty);
        }
    }

    pub fn select(&self, id: isize) {
        let tabs = self.tabs.borrow();
        if !tabs.iter().any(|tab| tab.id == id) {
            return;
        }
        for tab in tabs.iter() {
            let selected = tab.id == id;
            tab.view.setHidden(!selected);
            tab.item.set_selected(selected);
        }
        self.selected.set(Some(id));
        crate::session::changed();
    }

    /// Selects the tab `step` places from the selected one, wrapping around.
    pub fn select_relative(&self, step: isize) {
        let next = {
            let tabs = self.tabs.borrow();
            let Some(position) = tabs.iter().position(|tab| Some(tab.id) == self.selected.get()) else {
                return;
            };
            let count = isize::try_from(tabs.len()).unwrap_or(1);
            let index = (isize::try_from(position).unwrap_or(0) + step).rem_euclid(count);
            usize::try_from(index).ok().and_then(|i| tabs.get(i)).map(|tab| tab.id)
        };
        if let Some(id) = next {
            self.select(id);
        }
    }

    /// Takes the tab `id` out of the bar; where it was.
    fn remove(&self, id: isize) -> Option<usize> {
        let mut tabs = self.tabs.borrow_mut();
        let position = tabs.iter().position(|tab| tab.id == id)?;
        let tab = tabs.remove(position);
        tab.item.removeFromSuperview();
        tab.view.removeFromSuperview();
        Some(position)
    }

    pub fn close(&self, id: isize) {
        let next = {
            let Some(position) = self.remove(id) else { return };
            let tabs = self.tabs.borrow();
            tabs.get(position).or_else(|| tabs.last()).map(|tab| tab.id)
        };
        if self.selected.get() == Some(id) {
            self.selected.set(None);
            if let Some(next) = next {
                self.select(next);
            }
        }
        crate::session::changed();
    }

    /// Opens the "+" menu below its button.
    pub fn show_new_tab_menu(&self) {
        let clis = self
            .menu
            .itemWithTitle(&ns(t("CLI-Thread")))
            .and_then(|item| item.submenu());
        if let (Some(target), Some(clis)) = (self.target.load(), clis) {
            crate::terminal::fill_cli_menu(&target, &clis);
        }
        let own = self
            .menu
            .itemWithTitle(&ns(t("AI-Thread")))
            .and_then(|item| item.submenu())
            .and_then(|agents| agents.itemWithTitle(&ns(t("Eigener Agent"))))
            .and_then(|item| item.submenu());
        if let Some(own) = own {
            crate::agent::fill_own_menu(self.mtm, &own);
        }
        let below = NSPoint::new(0.0, self.plus.frame().size.height + 4.0);
        self.menu
            .popUpMenuPositioningItem_atLocation_inView(None, below, Some(&self.plus));
    }

    /// The menu of new tabs, which ⌘T and the "+" button open.
    #[cfg(feature = "selftest")]
    pub fn new_tab_menu(&self) -> Retained<NSMenu> {
        self.menu.clone()
    }

    /// Whether the "+" comes after the tabs in the reading direction: right of them, or left right to left.
    #[cfg(feature = "selftest")]
    pub fn plus_follows_tabs(&self) -> bool {
        self.bar.layoutSubtreeIfNeeded();
        let (strip, plus) = (self.strip.frame(), self.plus.frame());
        if self.bar.userInterfaceLayoutDirection() == objc2_app_kit::NSUserInterfaceLayoutDirection::RightToLeft {
            plus.origin.x + plus.size.width <= strip.origin.x + 1.0
        } else {
            plus.origin.x >= strip.origin.x + strip.size.width - 1.0
        }
    }

    /// The title of each tab, left to right, whether it is the preview tab, and what VoiceOver calls it.
    #[cfg(feature = "selftest")]
    pub fn previews(&self) -> Vec<(String, bool, String)> {
        self.tabs
            .borrow()
            .iter()
            .map(|tab| {
                let ivars = tab.item.ivars();
                let spoken = tab.item.accessibilityLabel().map(|l| l.to_string()).unwrap_or_default();
                (ivars.title.stringValue().to_string(), ivars.preview.get(), spoken)
            })
            .collect()
    }

    /// Clicks the tab `id` `count` times, as the mouse does.
    #[cfg(feature = "selftest")]
    pub fn click(&self, id: isize, count: isize) {
        let item = self
            .tabs
            .borrow()
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.item.clone());
        if let Some(item) = item {
            item.clicked(count);
        }
    }

    /// Whether the bar shows itself as the active panel's.
    #[cfg(feature = "selftest")]
    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// What VoiceOver calls the bar.
    #[cfg(feature = "selftest")]
    pub fn label(&self) -> String {
        self.bar.accessibilityLabel().map(|l| l.to_string()).unwrap_or_default()
    }

    #[cfg(feature = "selftest")]
    pub fn titles(&self) -> Vec<String> {
        self.tabs
            .borrow()
            .iter()
            .map(|tab| tab.item.ivars().title.stringValue().to_string())
            .collect()
    }

    /// Whether each tab, left to right, shows the mark of unsaved changes.
    #[cfg(feature = "selftest")]
    pub fn dirty_marks(&self) -> Vec<bool> {
        self.tabs
            .borrow()
            .iter()
            .map(|tab| tab.item.ivars().dirty.get())
            .collect()
    }

    /// The titles in the submenu of "+" that `path` names by the titles of its parents.
    #[cfg(feature = "selftest")]
    pub fn new_tab_submenu_titles(&self, path: &[&str]) -> Vec<String> {
        let mut menu = self.menu.clone();
        for title in path {
            let next = menu
                .itemArray()
                .iter()
                .find(|item| item.title().to_string() == *title)
                .and_then(|item| item.submenu());
            let Some(next) = next else { return Vec::new() };
            menu = next;
        }
        menu.itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| item.title().to_string())
            .collect()
    }

    #[cfg(feature = "selftest")]
    pub fn new_tab_titles(&self) -> Vec<String> {
        self.menu
            .itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| item.title().to_string())
            .collect()
    }
}

/// The accent behind a bar or over a panel that a dragged tab would drop into.
pub fn drop_highlight(mtm: MainThreadMarker) -> Retained<NSBox> {
    let drop = NSBox::new(mtm);
    drop.setBoxType(NSBoxType::Custom);
    drop.setBorderWidth(0.0);
    drop.setCornerRadius(6.0);
    drop.setFillColor(&NSColor::controlAccentColor().colorWithAlphaComponent(0.18));
    drop.setTitlePosition(objc2_app_kit::NSTitlePosition::NoTitle);
    drop.setHidden(true);
    drop
}

/// The agents a thread in a worktree of its own can run (I1), each tagged with its place in
/// `crate::worktrees::agents`; the window enables them as it does the agents above.
fn worktree_menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let agents = crate::worktrees::agents()
        .into_iter()
        .enumerate()
        .map(|(n, (agent, _))| {
            let entry = item(mtm, &tf("{} …", &[agent.name()]), Some(sel!(ideNewWorktreeThread:)), "");
            entry.setTag(isize::try_from(n).unwrap_or(-1));
            entry
        })
        .collect();
    let (parent, menu) = submenu(mtm, t("In eigenem Worktree"), agents);
    menu.setAutoenablesItems(true);
    parent.setImage(symbol("arrow.triangle.branch", t("In eigenem Worktree")).as_deref());
    parent
}

/// The kinds of tab "+" offers; those not built yet are shown, disabled.
fn new_tab_menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let entry = |title: &str, symbol_name: &str, action: Option<Sel>| {
        let entry = item(mtm, title, action, "");
        entry.setImage(symbol(symbol_name, title).as_deref());
        entry.setEnabled(action.is_some());
        entry
    };
    // The own agent's endpoints and models are filled in each time the menu opens.
    let (own, _) = submenu(mtm, t("Eigener Agent"), Vec::new());
    own.setImage(symbol("brain", t("Eigener Agent")).as_deref());
    let agents = std::iter::once(entry("Claude", "sparkle", Some(sel!(ideNewClaudeThread:))))
        .chain(std::iter::once(own))
        .chain(
            crate::agent::preset_menu()
                .into_iter()
                .enumerate()
                .map(|(n, (name, _))| {
                    let agent = entry(&name, "sparkles", Some(sel!(ideNewAcpThread:)));
                    agent.setTag(isize::try_from(n).unwrap_or(-1));
                    agent
                }),
        )
        .chain([
            separator(mtm),
            worktree_menu(mtm),
            separator(mtm),
            entry(
                t("Agenten verwalten …"),
                "gearshape",
                Some(sel!(ideOpenAgentsSettings:)),
            ),
        ])
        .collect();
    let (agent_item, agent_menu) = submenu(mtm, t("AI-Thread"), agents);
    // The agents are enabled as they are there or can be installed, which the window checks each time.
    agent_menu.setAutoenablesItems(true);
    agent_item.setImage(symbol("sparkles", t("AI-Thread")).as_deref());
    // The terminal module fills it each time the menu opens, from the login PATH and the folder's trust.
    let (cli_item, _) = submenu(mtm, t("CLI-Thread"), Vec::new());
    cli_item.setImage(symbol("apple.terminal", t("CLI-Thread")).as_deref());
    for entry in [
        entry(t("Neue Datei"), "doc.badge.plus", Some(sel!(ideNewFile:))),
        entry(t("Terminal"), "terminal", Some(sel!(ideNewTerminal:))),
        entry(t("Browser"), "globe", Some(sel!(ideNewBrowser:))),
        agent_item,
        cli_item,
        separator(mtm),
        entry(t("Willkommen"), "house", Some(sel!(ideOpenWelcome:))),
    ] {
        menu.addItem(&entry);
    }
    menu
}
