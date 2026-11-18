//! Window tabs (E60): every document gets a native macOS tab, an `NSWindow` of its own in the folder's tab group with
//! its own toolbar and split view; the folder's one sidebar moves into the tab in front. A window tab shows its
//! documents in panes (E145); while no pane holds more than one, the documents after the first have a small badge at
//! their top that names them and closes them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAttributedStringAttachmentConveniences, NSTextAttachment};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSButton, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSImage, NSImageAlignment,
    NSImageScaling, NSImageSymbolConfiguration, NSImageView, NSLineBreakMode, NSMenu, NSMenuItem, NSResponder,
    NSSplitViewController, NSSplitViewItem, NSStackView, NSTextField, NSTitlebarSeparatorStyle, NSToolbar,
    NSToolbarDisplayMode, NSUserInterfaceLayoutOrientation, NSView, NSViewController, NSWindow, NSWindowStyleMask,
    NSWindowToolbarStyle,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSNotificationCenter, NSObjectProtocol, NSPoint,
    NSRect, NSSize, NSString,
};
use splat_i18n::{t, tf};

use super::WindowController;
use super::agent_choices::AgentChoices;
use super::panes::{PaneId, Panes, Place};
use crate::util::{activate, fixed_size, item, ns, pin_edges, pin_sides, separator, submenu, symbol, symbol_button};

const WINDOW_SIZE: NSSize = NSSize::new(1240.0, 780.0);
const MIN_WINDOW_SIZE: NSSize = NSSize::new(720.0, 440.0);
/// How far the badge of the second of two documents keeps from its top and trailing edge.
const BADGE_INSET: f64 = 8.0;
/// The height of the icon before a tab's title.
const ICON_HEIGHT: f64 = 14.0;

pub(crate) use ide_ui_common::window::{TabId, TabKind};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// A document of a window tab: its view and what its tab and badge say about it.
pub(crate) struct Document {
    pub(crate) id: TabId,
    pub(crate) kind: TabKind,
    /// What the document shows, such as a file's path, so it opens once.
    pub(crate) key: Option<String>,
    pub(crate) view: Retained<NSView>,
    /// `view` with the badge over it, which sits in a pane.
    pub(super) host: Retained<NSView>,
    badge: Retained<NSBox>,
    badge_title: Retained<NSTextField>,
    badge_icon: Retained<NSImageView>,
    close: Retained<NSButton>,
    title: RefCell<String>,
    preview: Cell<bool>,
    dirty: Cell<bool>,
    status: RefCell<Option<Retained<NSColor>>>,
    tint: RefCell<Option<Retained<NSColor>>>,
    /// The colours of the symbol's left, top, right and bottom point instead of its tint, as Google's agents show.
    four_colours: RefCell<Option<[Retained<NSColor>; 4]>>,
    /// The view in the document that had the keyboard focus last, which gets it back when the document is activated.
    focus: RefCell<Option<Weak<NSView>>>,
}

impl Document {
    pub(crate) fn new(
        controller: &WindowController,
        id: TabId,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Rc<Document> {
        let mtm = controller.mtm();
        let host = NSView::new(mtm);
        let close = symbol_button(
            mtm,
            "xmark",
            "",
            ide_ui_common::style::symbol::TAB_CLOSE,
            Some(controller),
            sel!(ideCloseTabButton:),
        );
        close.setTag(id.get());
        close.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let (badge, badge_title, badge_icon) = badge(mtm, &close);
        host.addSubview(&view);
        host.addSubview(&badge);
        pin_edges(&view, &host);
        badge.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            badge
                .topAnchor()
                .constraintEqualToAnchor_constant(&host.topAnchor(), BADGE_INSET),
            badge
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&host.trailingAnchor(), -BADGE_INSET),
            badge
                .leadingAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&host.leadingAnchor(), BADGE_INSET),
        ]);
        badge.setHidden(true);
        Rc::new(Document {
            id,
            kind,
            key,
            view,
            host,
            badge,
            badge_title,
            badge_icon,
            close,
            title: RefCell::new(title.to_string()),
            preview: Cell::new(false),
            dirty: Cell::new(false),
            status: RefCell::new(None),
            tint: RefCell::new(None),
            four_colours: RefCell::new(None),
            focus: RefCell::new(None),
        })
    }

    pub(crate) fn title(&self) -> String {
        self.title.borrow().clone()
    }

    pub(crate) fn is_preview(&self) -> bool {
        self.preview.get()
    }

    pub(crate) fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_string();
        self.name_badge();
    }

    pub(crate) fn set_preview(&self, preview: bool) {
        self.preview.set(preview);
    }

    pub(crate) fn set_dirty(&self, dirty: bool) {
        self.dirty.set(dirty);
    }

    pub(super) fn dirty(&self) -> bool {
        self.dirty.get()
    }

    pub(super) fn status_colour(&self) -> Option<Retained<NSColor>> {
        self.status.borrow().clone()
    }

    pub(crate) fn set_status(&self, color: Option<&NSColor>) {
        *self.status.borrow_mut() = color.map(objc2::Message::retain);
    }

    pub(crate) fn set_tint(&self, color: &NSColor) {
        *self.tint.borrow_mut() = Some(objc2::Message::retain(color));
        self.name_badge();
    }

    pub(crate) fn set_four_colours(&self, colors: [Retained<NSColor>; 4]) {
        *self.four_colours.borrow_mut() = Some(colors);
        self.name_badge();
    }

    /// The small picture before the title of its tab: a file's icon, else a symbol of what it shows in the colour of
    /// its agent or worktree.
    pub(super) fn icon(&self) -> Option<Retained<NSImage>> {
        let tint = self.tint.borrow().clone();
        let path = self.key.as_deref().map(std::path::Path::new);
        let name = match self.kind {
            TabKind::Editor => {
                if let Some(image) = path.and_then(crate::file_icon::image) {
                    return Some(image);
                }
                "doc.text"
            }
            TabKind::Preview if let Some(image) = path.and_then(crate::file_icon::image) => return Some(image),
            kind => kind.symbol(),
        };
        let four = self.four_colours.borrow().clone();
        let color = match (tint, self.kind) {
            (Some(tint), _) => tint,
            (None, TabKind::Agent | TabKind::Plan) => ide_ui_agent::Agent::Claude.colour(),
            (None, _) => NSColor::secondaryLabelColor(),
        };
        let configuration = NSImageSymbolConfiguration::configurationWithPointSize_weight(
            11.0,
            crate::util::constant!(objc2_app_kit::NSFontWeightRegular),
        )
        .configurationByApplyingConfiguration(&NSImageSymbolConfiguration::configurationWithHierarchicalColor(&color));
        let image = if ide_ui_common::util::is_own_symbol(name) {
            ide_ui_common::util::own_symbol_in(name, "", 11.0, &color)?
        } else {
            symbol(name, "")?.imageWithSymbolConfiguration(&configuration)?
        };
        Some(match four {
            Some(colors) => crate::util::four_colours(&image, &colors),
            None => image,
        })
    }

    /// Whether `view` is the document's or inside it.
    fn holds(&self, view: &NSView) -> bool {
        view.isDescendantOf(&self.host)
    }

    /// Shows the badge, which names the document and closes it, while it is in front in a pane after the first and no
    /// pane shows a tab bar.
    pub(super) fn set_badge(&self, shown: bool, focused: bool) {
        self.badge.setHidden(!shown);
        let color = if focused {
            NSColor::labelColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        self.badge_title.setTextColor(Some(&color));
        if shown {
            self.name_badge();
        }
    }

    /// Puts the document's name and icon on its badge: a Markdown preview is "Vorschau" with an eye, as it sits
    /// beside the document it shows.
    fn name_badge(&self) {
        let (text, close, image) = if self.kind == TabKind::Preview && self.key.is_none() {
            (
                t("Vorschau").to_string(),
                t("Vorschau schließen").to_string(),
                symbol("eye", ""),
            )
        } else {
            let title = self.title();
            let close = tf("{} schließen", &[&title]);
            (title, close, self.icon())
        };
        self.badge_title.setStringValue(&ns(&text));
        self.badge_icon.setImage(image.as_deref());
        self.close.setToolTip(Some(&ns(&close)));
        crate::util::set_label(&self.close, Some(&close));
    }

    /// The view to give the keyboard focus: the one that had it last, else the first in the document that takes it.
    pub(super) fn focus_target(&self) -> Option<Retained<NSView>> {
        let remembered = self
            .focus
            .borrow()
            .as_ref()
            .and_then(Weak::load)
            .filter(|view| view.isDescendantOf(&self.view));
        remembered.or_else(|| first_responder_in(&self.view))
    }
}

/// The badge of a document beside another: its icon, its name and `close`, on a rounded plate.
fn badge(mtm: MainThreadMarker, close: &NSButton) -> (Retained<NSBox>, Retained<NSTextField>, Retained<NSImageView>) {
    let title = NSTextField::labelWithString(&ns(""), mtm);
    title.setFont(Some(&NSFont::systemFontOfSize(ide_ui_common::style::SMALL)));
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    title.setContentCompressionResistancePriority_forOrientation(
        200.0,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    let icon = NSImageView::new(mtm);
    icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    crate::util::hide_from_voiceover(&icon);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(ide_ui_common::style::space::SNUG);
    row.addArrangedSubview(&icon);
    row.addArrangedSubview(&title);
    row.addArrangedSubview(close);
    row.setEdgeInsets(objc2_foundation::NSEdgeInsets {
        top: 3.0,
        left: 8.0,
        bottom: 3.0,
        right: 6.0,
    });
    let plate = NSBox::new(mtm);
    plate.setBoxType(objc2_app_kit::NSBoxType::Custom);
    plate.setTitlePosition(objc2_app_kit::NSTitlePosition::NoTitle);
    plate.setCornerRadius(7.0);
    plate.setBorderColor(&NSColor::separatorColor());
    plate.setFillColor(&NSColor::windowBackgroundColor());
    plate.setContentViewMargins(NSSize::new(0.0, 0.0));
    if let Some(content) = plate.contentView() {
        content.addSubview(&row);
        pin_edges(&row, &content);
    }
    (plate, title, icon)
}

/// A window tab: its window and the panes of the documents it shows.
pub(crate) struct TabWindow {
    pub(crate) window: Retained<NSWindow>,
    split: Retained<NSSplitViewController>,
    sidebar: Retained<NSSplitViewItem>,
    /// The inspector at the trailing edge (E94, E101), and where the folder's inspector goes while the tab is in front.
    pub(super) inspector: Retained<NSSplitViewItem>,
    pub(super) inspector_slot: Retained<NSView>,
    /// Where the folder's sidebar goes while the tab is in front.
    slot: Retained<NSView>,
    /// What the sidebar looked like when it left, for a window that stays on screen beside the one in front.
    picture: Retained<NSImageView>,
    /// The documents' area and the notes over it.
    content: Retained<NSView>,
    pub(super) panes: Panes,
    /// What the tab's controller is, for the panes' bars.
    controller: Weak<WindowController>,
    /// The dot of the tab's state beside its title.
    status: Retained<NSImageView>,
    /// The watch that keeps the toolbar clear above the sidebar in full screen.
    sidebar_watch: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    /// The watches that keep the sidebar below the toolbar as it slides and the documents' column moves with it.
    toolbar_watch: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl Drop for TabWindow {
    fn drop(&mut self) {
        let center = NSNotificationCenter::defaultCenter();
        for token in std::iter::once(&self.sidebar_watch).chain(&self.toolbar_watch) {
            // SAFETY: The token came from this center's block-based `addObserverForName`.
            unsafe { center.removeObserver(token.as_ref()) };
        }
    }
}

impl TabWindow {
    pub(crate) fn new(controller: &WindowController, identifier: &str, frame: Option<NSRect>) -> Rc<TabWindow> {
        let mtm = controller.mtm();
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
        let rect = frame.unwrap_or(NSRect::new(NSPoint::new(0.0, 0.0), WINDOW_SIZE));
        // SAFETY: releasedWhenClosed is switched off right below, so Rust's reference stays the owner.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setMinSize(MIN_WINDOW_SIZE);
        // Documents and their views come and go after the window shows, past the key view loop AppKit makes once.
        window.setAutorecalculatesKeyViewLoop(true);
        window.setToolbarStyle(NSWindowToolbarStyle::Unified);
        window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
        window.setTabbingIdentifier(&ns(identifier));
        window.setDelegate(Some(ProtocolObject::from_ref(controller)));

        let split = NSSplitViewController::new(mtm);
        let slot = NSView::new(mtm);
        let picture = NSImageView::new(mtm);
        picture.setImageScaling(NSImageScaling::ScaleNone);
        picture.setImageAlignment(NSImageAlignment::AlignTopLeft);
        picture.setHidden(true);
        crate::util::hide_from_voiceover(&picture);
        slot.addSubview(&picture);
        pin_edges(&picture, &slot);
        let sidebar_controller = NSViewController::new(mtm);
        sidebar_controller.setView(&slot);
        let sidebar = NSSplitViewItem::sidebarWithViewController(&sidebar_controller);
        // Above the content's, so resizing the window resizes the content and the sidebar keeps its width.
        sidebar.setHoldingPriority(260.0);

        let content = NSView::new(mtm);
        let area = NSView::new(mtm);
        content.addSubview(&area);
        pin_sides(&area, &content, 0.0);
        // Below the title bar and the tab bar, which the content reaches under, so the panes' bars and badges show.
        activate([
            area.topAnchor()
                .constraintEqualToAnchor(&content.safeAreaLayoutGuide().topAnchor()),
            area.bottomAnchor().constraintEqualToAnchor(&content.bottomAnchor()),
        ]);
        let panes = Panes::new(mtm, area);
        let content_controller = NSViewController::new(mtm);
        content_controller.setView(&content);
        let content_item = NSSplitViewItem::splitViewItemWithViewController(&content_controller);
        content_item.setMinimumThickness(360.0);
        split.addSplitViewItem(&sidebar);
        split.addSplitViewItem(&content_item);
        let (inspector, inspector_slot) = super::inspector::split_item(mtm);
        split.addSplitViewItem(&inspector);
        window.setContentViewController(Some(&split));

        let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &ns("IDEProjectToolbar"));
        toolbar.setDelegate(Some(ProtocolObject::from_ref(controller)));
        toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
        window.setToolbar(Some(&toolbar));

        let status = status_dot(mtm);
        let sidebar_watch = super::full_screen::follow_sidebar(&window, &slot, &split.splitView());
        let top = content_top(&content);
        let controller_ref = Weak::from(controller);
        let follow = move || {
            if let Some(controller) = controller_ref.load() {
                controller.follow_toolbar();
            }
        };
        let mut toolbar_watch = super::full_screen::follow_frames(&[&content, &top], follow.clone());
        toolbar_watch.push(super::full_screen::follow_toolbar_window(&window, follow));
        Rc::new(TabWindow {
            window,
            split,
            sidebar,
            inspector,
            inspector_slot,
            slot,
            picture,
            content,
            panes,
            controller: Weak::from(controller),
            status,
            sidebar_watch,
            toolbar_watch,
        })
    }

    /// The documents, pane by pane, left to right and top to bottom.
    pub(crate) fn documents(&self) -> Vec<Rc<Document>> {
        self.panes.documents()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub(crate) fn holds(&self, id: TabId) -> bool {
        self.panes.of_document(id).is_some()
    }

    pub(crate) fn document(&self, id: TabId) -> Option<Rc<Document>> {
        self.panes.document(id)
    }

    /// The document commands act on: the one in front in the active pane.
    pub(crate) fn focused(&self) -> Option<Rc<Document>> {
        self.panes.focused()
    }

    /// The area under the documents, where notes show.
    pub(crate) fn content(&self) -> &NSView {
        &self.content
    }

    /// Puts `document` where `place` says and brings it to the front of its pane.
    pub(crate) fn insert(&self, document: Rc<Document>, place: Place) {
        self.panes.insert(document, place);
        self.refresh();
    }

    /// Takes the document `id` out of the window tab; a pane left empty goes with it.
    pub(crate) fn remove(&self, id: TabId) -> Option<Rc<Document>> {
        let document = self.panes.take(id);
        self.refresh();
        document
    }

    /// Follows the keyboard focus: the pane that has it becomes the active one, and the document remembers where it
    /// was.
    pub(crate) fn follow_focus(&self) {
        let responder = self.window.firstResponder();
        let Some(view) = responder.and_then(|r| Retained::downcast::<NSView>(r).ok()) else {
            return;
        };
        let Some(pane) = self.panes.pane_holding(&view) else {
            return;
        };
        if let Some(doc) = self.panes.front(pane).filter(|doc| doc.holds(&view)) {
            *doc.focus.borrow_mut() = Some(Weak::from(&*view));
        }
        if self.panes.active() != pane {
            self.panes.set_active(pane);
            self.refresh();
        }
    }

    /// Makes the document `id` the one commands act on, in front of its pane, without moving the keyboard focus.
    pub(crate) fn mark_focused(&self, id: TabId) {
        if self.panes.mark_focused(id) {
            self.refresh();
        }
    }

    /// Gives the keyboard focus to the document `id`, bringing it to the front of its pane.
    pub(crate) fn activate(&self, id: TabId) {
        let Some(pane) = self.panes.of_document(id) else { return };
        self.panes.select(id);
        self.activate_pane(pane);
    }

    /// Makes `pane` the active one and gives the keyboard focus to its document in front.
    pub(super) fn activate_pane(&self, pane: PaneId) {
        self.panes.set_active(pane);
        self.refresh();
        match self.panes.focus_target(pane) {
            Some(view) => self.window.makeFirstResponder(Some(&view)),
            None => self.window.makeFirstResponder(None),
        };
    }

    /// Gives the keyboard focus to the next pane that shows, after the last the first.
    pub(crate) fn focus_next_pane(&self) {
        let next = self.panes.step_pane(1);
        self.refresh();
        if let Some(next) = next {
            self.activate_pane(next);
        }
    }

    /// Says on the tab what the focused document is and whether a document has unsaved changes, and shows the panes'
    /// bars or the badges.
    pub(crate) fn refresh(&self) {
        if let Some(controller) = self.controller.load() {
            self.panes.settle(&controller);
        }
        let documents = self.documents();
        let Some(doc) = self.focused().or_else(|| documents.first().cloned()) else {
            return;
        };
        let title = doc.title();
        let tab = self.window.tab();
        let font = NSFont::systemFontOfSize(NSFont::systemFontSize());
        let font = if doc.preview.get() {
            NSFontManager::sharedFontManager(self.window.mtm())
                .convertFont_toHaveTrait(&font, NSFontTraitMask::ItalicFontMask)
        } else {
            font
        };
        tab.setAttributedTitle(Some(&titled(doc.icon().as_deref(), &title, &font)));
        tab.setTitle(Some(&ns(&title)));
        let names: Vec<String> = documents.iter().map(|doc| doc.title()).collect();
        let tip = if doc.preview.get() {
            t("Vorschau: Die nächste Datei, die du mit einem Klick öffnest, ersetzt diesen Tab. Ein Doppelklick behält ihn.")
                .to_string()
        } else {
            names.join(" | ")
        };
        tab.setToolTip(Some(&ns(&tip)));
        self.window
            .setDocumentEdited(documents.iter().any(|doc| doc.dirty.get()));
        let status = documents.iter().find_map(|doc| doc.status.borrow().clone());
        if let Some(color) = &status {
            self.status.setContentTintColor(Some(color));
            objc2_app_kit::NSAccessibility::setAccessibilityLabel(&*self.status, Some(&ns(status_words(color))));
        }
        let current = tab.accessoryView(self.window.mtm());
        if status.is_some() && current.is_none() {
            tab.setAccessoryView(Some(&self.status));
        } else if status.is_none() && current.is_some() {
            tab.setAccessoryView(None);
        }
    }

    /// Puts the folder's sidebar `view` into the tab, in place of its picture.
    pub(crate) fn adopt_sidebar(&self, view: &NSView) {
        self.picture.setHidden(true);
        self.picture.setImage(None);
        self.slot.addSubview(view);
        pin_edges(view, &self.slot);
    }

    /// Shows what `view`, the sidebar about to leave, looks like now, for a window that stays on screen.
    pub(crate) fn keep_picture_of(&self, view: &NSView) {
        let bounds = view.bounds();
        let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) else {
            return;
        };
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let image = NSImage::initWithSize(NSImage::alloc(), bounds.size);
        image.addRepresentation(&rep);
        self.picture.setImage(Some(&image));
        self.picture.setHidden(false);
    }

    /// Lets go of the sidebar's picture, for a window that left the screen with it.
    pub(crate) fn forget_picture(&self) {
        self.picture.setHidden(true);
        self.picture.setImage(None);
    }

    /// Whether the tab shows a picture of the sidebar.
    pub(crate) fn has_picture(&self) -> bool {
        self.picture.image().is_some()
    }

    /// Clears the toolbar's background above the sidebar in full screen, or brings it back with `clear` false.
    pub(crate) fn clear_above_sidebar(&self, clear: bool) {
        super::full_screen::clear_above_sidebar(&self.window, &self.slot, clear);
    }

    /// How far the toolbar reaches down over the sidebar in full screen, where the menu bar can push it down.
    pub(crate) fn toolbar_reach(&self) -> f64 {
        super::full_screen::toolbar_reach(&self.window, &self.slot, &self.content)
    }

    /// The width of the sidebar now.
    pub(crate) fn sidebar_width(&self) -> f64 {
        self.sidebar.viewController(self.window.mtm()).view().frame().size.width
    }

    pub(crate) fn sidebar_collapsed(&self) -> bool {
        self.sidebar.isCollapsed()
    }

    /// Holds the sidebar at `width`, which a window tab that has not shown yet takes only this way.
    pub(crate) fn pin_sidebar(&self, width: f64, collapsed: bool) {
        self.sidebar.setMinimumThickness(width);
        self.sidebar.setMaximumThickness(width);
        if self.sidebar.isCollapsed() != collapsed {
            self.sidebar.setCollapsed(collapsed);
        }
        // Setting the divider's position unfolds a folded sidebar.
        if !collapsed {
            self.split.splitView().setPosition_ofDividerAtIndex(width, 0);
        }
    }

    /// Lets the user drag the sidebar between `min` and `max` again.
    pub(crate) fn release_sidebar(&self, min: f64, max: f64) {
        if self.sidebar.minimumThickness() != min {
            self.sidebar.setMinimumThickness(min);
        }
        if self.sidebar.maximumThickness() != max {
            self.sidebar.setMaximumThickness(max);
        }
    }
}

/// An empty line at the top of `content`'s safe area, whose frame changes when the toolbar above it moves.
fn content_top(content: &NSView) -> Retained<NSView> {
    let line = NSView::new(content.mtm());
    line.setHidden(true);
    content.addSubview(&line);
    line.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        line.topAnchor()
            .constraintEqualToAnchor(&content.safeAreaLayoutGuide().topAnchor()),
        line.leadingAnchor().constraintEqualToAnchor(&content.leadingAnchor()),
    ]);
    fixed_size(&line, 1.0, 0.0);
    line
}

/// A tab's title after its icon, which is centred on the capitals of the text and left out for VoiceOver, which reads
/// the title.
fn titled(icon: Option<&NSImage>, title: &str, font: &NSFont) -> Retained<NSAttributedString> {
    let text = NSMutableAttributedString::new();
    if let Some(icon) = icon {
        let size = icon.size();
        let height = ICON_HEIGHT.min(size.height.max(1.0));
        let width = size.width * height / size.height.max(1.0);
        let attachment = NSTextAttachment::new();
        attachment.setImage(Some(icon));
        attachment.setBounds(NSRect::new(
            NSPoint::new(0.0, (font.capHeight() - height) / 2.0),
            NSSize::new(width, height),
        ));
        text.appendAttributedString(&NSAttributedString::attributedStringWithAttachment(&attachment));
        text.appendAttributedString(&attributed("\u{2002}", font));
    }
    text.appendAttributedString(&attributed(title, font));
    text.into_super()
}

fn attributed(text: &str, font: &NSFont) -> Retained<NSAttributedString> {
    // SAFETY: NSFontAttributeName is a constant AppKit sets when it loads.
    let key = unsafe { objc2_app_kit::NSFontAttributeName };
    let attributes =
        NSDictionary::<NSString, objc2::runtime::AnyObject>::from_slices(&[key], &[font as &objc2::runtime::AnyObject]);
    // SAFETY: The attributes map an attribute name to a font, which the font attribute takes.
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &ns(text), Some(&attributes)) }
}

/// The dot of a document's state, such as an agent's turn, beside its tab's title.
fn status_dot(mtm: MainThreadMarker) -> Retained<NSImageView> {
    let status = NSImageView::new(mtm);
    status.setImage(symbol("circle.fill", "").as_deref());
    status.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
        7.0,
        crate::util::constant!(objc2_app_kit::NSFontWeightRegular),
    )));
    status
}

/// What a tab's dot of `color` means, for VoiceOver: red waits for the user, as an agent's question or an order does;
/// any other colour works.
pub(super) fn status_words(color: &NSColor) -> &'static str {
    if *color == *NSColor::systemRedColor() {
        t("Wartet auf dich")
    } else {
        t("Arbeitet")
    }
}

/// The view at or under `view` to give the keyboard focus: a text view if there is one, as an editor or a composer
/// has, else the first that takes the focus.
pub(super) fn first_responder_in(view: &NSView) -> Option<Retained<NSView>> {
    fn find(view: &NSView, want: &dyn Fn(&NSView) -> bool) -> Option<Retained<NSView>> {
        if want(view) {
            return Some(objc2::Message::retain(view));
        }
        view.subviews().iter().find_map(|sub| find(&sub, want))
    }
    let takes = |view: &NSView| {
        let responder: &NSResponder = view;
        responder.acceptsFirstResponder()
    };
    let text = |view: &NSView| {
        let object: &objc2::runtime::AnyObject = view;
        object.downcast_ref::<objc2_app_kit::NSTextView>().is_some()
    };
    find(view, &|view| text(view) && takes(view)).or_else(|| find(view, &|view| takes(view)))
}

/// The installed agents' entries for a thread in a worktree of its own (I1), if any agent is installed, each tagged with
/// its place in `crate::worktrees::agents`; the window enables them as they can start.
fn worktree_menu(mtm: MainThreadMarker, choices: &AgentChoices) -> Option<Retained<NSMenuItem>> {
    let agents: Vec<Retained<NSMenuItem>> = choices
        .worktree
        .iter()
        .enumerate()
        .filter(|(_, (_, _, installed))| *installed)
        .map(|(n, (agent, _, _))| {
            let entry = item(mtm, &tf("{} …", &[agent.name()]), Some(sel!(ideNewWorktreeThread:)), "");
            entry.setTag(isize::try_from(n).unwrap_or(-1));
            entry
        })
        .collect();
    if agents.is_empty() {
        return None;
    }
    let (parent, menu) = submenu(mtm, t("In eigenem Worktree"), agents);
    menu.setAutoenablesItems(true);
    parent.setImage(symbol("arrow.triangle.branch", t("In eigenem Worktree")).as_deref());
    Some(parent)
}

/// What a new tab can show (E17), the menu that ⌘T and the tab bar's "+" open (E79). The installed agents stand in it
/// directly, a thread in a worktree of its own below them, as `choices` knows them.
pub(crate) fn new_tab_menu(mtm: MainThreadMarker, choices: &AgentChoices) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let entry = |title: &str, symbol_name: &str, action: Option<Sel>| {
        let entry = item(mtm, title, action, "");
        entry.setImage(symbol(symbol_name, title).as_deref());
        entry.setEnabled(action.is_some());
        entry
    };
    for entry in [
        entry(t("Neue Datei"), "doc.badge.plus", Some(sel!(ideNewFile:))),
        entry(t("Terminal"), "terminal", Some(sel!(ideNewTerminal:))),
        entry(t("Browser"), "globe", Some(sel!(ideNewBrowser:))),
        separator(mtm),
    ] {
        menu.addItem(&entry);
    }
    // Only installed agents are offered; the own agent's endpoints and models are filled in each time the menu opens.
    if choices.claude {
        menu.addItem(&entry("Claude", "sparkle", Some(sel!(ideNewClaudeThread:))));
    }
    for (index, name) in &choices.installed_presets {
        let agent = entry(name, "sparkles", Some(sel!(ideNewAcpThread:)));
        agent.setTag(isize::try_from(*index).unwrap_or(-1));
        agent.setEnabled(choices.presets.get(*index).is_some_and(|(_, usable)| *usable));
        menu.addItem(&agent);
    }
    if choices.own {
        let (own, _) = submenu(mtm, t("Eigener Agent"), Vec::new());
        own.setImage(symbol("brain", t("Eigener Agent")).as_deref());
        menu.addItem(&own);
    }
    if let Some(worktree) = worktree_menu(mtm, choices) {
        menu.addItem(&worktree);
    }
    for entry in [
        entry(
            t("Agenten verwalten …"),
            "gearshape",
            Some(sel!(ideOpenAgentsSettings:)),
        ),
        separator(mtm),
        entry(t("Willkommen"), "house", Some(sel!(ideOpenWelcome:))),
    ] {
        menu.addItem(&entry);
    }
    menu
}

/// Fills the parts of `menu` that change: the own agent's endpoints.
pub(crate) fn fill_new_tab_menu(controller: &WindowController, menu: &NSMenu) {
    let own = menu
        .itemWithTitle(&ns(t("Eigener Agent")))
        .and_then(|item| item.submenu());
    if let Some(own) = own {
        ide_ui_agent::fill_own_menu(controller.mtm(), &own);
    }
}

/// The documents' default size, for a window that has no saved frame.
pub(crate) fn default_size() -> NSSize {
    WINDOW_SIZE
}
