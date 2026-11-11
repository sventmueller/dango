//! Window tabs (E60): every document gets a native macOS tab, an `NSWindow` of its own in the folder's tab group with
//! its own toolbar and split view; the folder's one sidebar moves into the tab in front. A window tab shows one
//! document, or two side by side, the second then with a small badge at its top that names it and closes it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAttributedStringAttachmentConveniences, NSLayoutConstraint, NSTextAttachment};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSButton, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSImage, NSImageAlignment,
    NSImageScaling, NSImageSymbolConfiguration, NSImageView, NSLineBreakMode, NSMenu, NSMenuItem, NSResponder,
    NSSplitViewController, NSSplitViewDividerStyle, NSSplitViewItem, NSStackView, NSTextField, NSToolbar,
    NSToolbarDisplayMode, NSUserInterfaceLayoutOrientation, NSView, NSViewController, NSWindow, NSWindowStyleMask,
    NSWindowToolbarStyle,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
};
use splat_i18n::{t, tf};

use super::WindowController;
use super::document_area::DocumentArea;
use crate::util::{item, ns, pin_edges, separator, submenu, symbol, symbol_button};

const WINDOW_SIZE: NSSize = NSSize::new(1240.0, 780.0);
const MIN_WINDOW_SIZE: NSSize = NSSize::new(720.0, 440.0);
/// How far the badge of the second of two documents keeps from its top and trailing edge.
const BADGE_INSET: f64 = 8.0;
/// The height of the icon before a tab's title.
const ICON_HEIGHT: f64 = 14.0;
/// The priority that holds the share: above the split view's holding priority, below a drag of its divider.
const SHARE_PRIORITY: f32 = 260.0;
/// The narrowest a document gets beside another.
const MIN_SIDE: f64 = 220.0;

/// What a document shows.
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
    /// The page of a new tab, which offers what it can show (E17) and gives way to it.
    NewTab,
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
            TabKind::NewTab => "plus",
        }
    }
}

/// A document of a window tab: its view and what its tab and badge say about it.
pub struct Document {
    pub id: isize,
    pub kind: TabKind,
    /// What the document shows, such as a file's path, so it opens once.
    pub key: Option<String>,
    pub view: Retained<NSView>,
    /// `view` with the badge over it, which sit side by side with another document's.
    host: Retained<NSView>,
    badge: Retained<NSBox>,
    badge_title: Retained<NSTextField>,
    badge_icon: Retained<NSImageView>,
    close: Retained<NSButton>,
    title: RefCell<String>,
    preview: Cell<bool>,
    dirty: Cell<bool>,
    status: RefCell<Option<Retained<NSColor>>>,
    tint: RefCell<Option<Retained<NSColor>>>,
    /// The view in the document that had the keyboard focus last, which gets it back when the document is activated.
    focus: RefCell<Option<Weak<NSView>>>,
}

impl Document {
    pub fn new(
        controller: &WindowController,
        id: isize,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Rc<Document> {
        let mtm = controller.mtm();
        let host = NSView::new(mtm);
        let close = symbol_button(mtm, "xmark", "", 9.0, Some(controller), sel!(ideCloseTabButton:));
        close.setTag(id);
        close.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let (badge, badge_title, badge_icon) = badge(mtm, &close);
        host.addSubview(&view);
        host.addSubview(&badge);
        pin_edges(&view, &host);
        badge.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            badge
                .topAnchor()
                .constraintEqualToAnchor_constant(&host.topAnchor(), BADGE_INSET),
            badge
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&host.trailingAnchor(), -BADGE_INSET),
            badge
                .leadingAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&host.leadingAnchor(), BADGE_INSET),
        ] {
            constraint.setActive(true);
        }
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
            focus: RefCell::new(None),
        })
    }

    pub fn title(&self) -> String {
        self.title.borrow().clone()
    }

    pub fn is_preview(&self) -> bool {
        self.preview.get()
    }

    #[cfg(feature = "selftest")]
    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_string();
        self.name_badge();
    }

    pub fn set_preview(&self, preview: bool) {
        self.preview.set(preview);
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.dirty.set(dirty);
    }

    pub fn set_status(&self, color: Option<&NSColor>) {
        *self.status.borrow_mut() = color.map(objc2::Message::retain);
    }

    pub fn set_tint(&self, color: &NSColor) {
        *self.tint.borrow_mut() = Some(objc2::Message::retain(color));
        self.name_badge();
    }

    /// The small picture before the title of its tab: a file's icon, else a symbol of what it shows in the colour of
    /// its agent or worktree.
    fn icon(&self) -> Option<Retained<NSImage>> {
        let tint = self.tint.borrow().clone();
        let path = self.key.as_deref().map(std::path::Path::new);
        let name = match self.kind {
            TabKind::Editor => {
                if let Some(image) = path.and_then(crate::file_icon::image) {
                    return Some(image);
                }
                "doc.text"
            }
            TabKind::Preview if path.is_some_and(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))) => {
                "doc.richtext"
            }
            kind => kind.symbol(),
        };
        let color = match (tint, self.kind) {
            (Some(tint), _) => tint,
            (None, TabKind::Cli) => {
                crate::agent::cli_colour(self.title().split_whitespace().next().unwrap_or_default())
            }
            (None, TabKind::Agent | TabKind::Plan) => crate::agent::Agent::Claude.colour(),
            (None, _) => NSColor::secondaryLabelColor(),
        };
        let configuration = NSImageSymbolConfiguration::configurationWithPointSize_weight(
            11.0,
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightRegular },
        )
        .configurationByApplyingConfiguration(&NSImageSymbolConfiguration::configurationWithHierarchicalColor(&color));
        symbol(name, "")?.imageWithSymbolConfiguration(&configuration)
    }

    /// Whether `view` is the document's or inside it.
    fn holds(&self, view: &NSView) -> bool {
        view.isDescendantOf(&self.host)
    }

    /// Shows the badge, which names the document and closes it, while it is the second of two in its window tab.
    fn set_badge(&self, shown: bool, focused: bool) {
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

    /// Puts the document's name and icon on its badge: a preview is "Vorschau" with an eye, as it sits beside the
    /// document it shows.
    fn name_badge(&self) {
        let (text, close, image) = if self.kind == TabKind::Preview {
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
    fn focus_target(&self) -> Option<Retained<NSView>> {
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
    title.setFont(Some(&NSFont::systemFontOfSize(11.0)));
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
    row.setSpacing(5.0);
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

/// A window tab: its window and the documents it shows, left to right.
pub struct TabWindow {
    pub window: Retained<NSWindow>,
    split: Retained<NSSplitViewController>,
    sidebar: Retained<NSSplitViewItem>,
    /// Where the folder's sidebar goes while the tab is in front.
    slot: Retained<NSView>,
    /// What the sidebar looked like when it left, for a window that stays on screen beside the one in front.
    picture: Retained<NSImageView>,
    /// The documents' area and the notes over it.
    content: Retained<NSView>,
    /// The left document's share of the width while there are two, which a resize keeps and only a drag changes.
    ratio: Cell<f64>,
    /// The constraints that hold the share and the documents' least width.
    holding: RefCell<Vec<Retained<NSLayoutConstraint>>>,
    area: Retained<DocumentArea>,
    documents: RefCell<Vec<Rc<Document>>>,
    focused: Cell<usize>,
    /// The dot of the tab's state beside its title.
    status: Retained<NSImageView>,
}

impl TabWindow {
    pub fn new(controller: &WindowController, identifier: &str, frame: Option<NSRect>) -> Rc<TabWindow> {
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
        let area = DocumentArea::new(mtm);
        area.setVertical(true);
        area.setDividerStyle(NSSplitViewDividerStyle::Thin);
        content.addSubview(&area);
        area.setTranslatesAutoresizingMaskIntoConstraints(false);
        // Below the title bar and the tab bar, which the content reaches under, so the badge of a second document shows.
        for constraint in [
            area.topAnchor()
                .constraintEqualToAnchor(&content.safeAreaLayoutGuide().topAnchor()),
            area.leadingAnchor().constraintEqualToAnchor(&content.leadingAnchor()),
            area.trailingAnchor().constraintEqualToAnchor(&content.trailingAnchor()),
            area.bottomAnchor().constraintEqualToAnchor(&content.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        let content_controller = NSViewController::new(mtm);
        content_controller.setView(&content);
        let content_item = NSSplitViewItem::splitViewItemWithViewController(&content_controller);
        content_item.setMinimumThickness(360.0);
        split.addSplitViewItem(&sidebar);
        split.addSplitViewItem(&content_item);
        window.setContentViewController(Some(&split));

        let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &ns("IDEProjectToolbar"));
        toolbar.setDelegate(Some(ProtocolObject::from_ref(controller)));
        toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
        window.setToolbar(Some(&toolbar));

        let status = status_dot(mtm);
        let tab = Rc::new(TabWindow {
            window,
            split,
            sidebar,
            slot,
            picture,
            content,
            ratio: Cell::new(0.5),
            holding: RefCell::new(Vec::new()),
            area,
            documents: RefCell::new(Vec::new()),
            focused: Cell::new(0),
            status,
        });
        TabWindow::follow_drags(&tab);
        tab
    }

    pub fn documents(&self) -> Vec<Rc<Document>> {
        self.documents.borrow().clone()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.borrow().is_empty()
    }

    pub fn holds(&self, id: isize) -> bool {
        self.documents.borrow().iter().any(|doc| doc.id == id)
    }

    pub fn document(&self, id: isize) -> Option<Rc<Document>> {
        self.documents.borrow().iter().find(|doc| doc.id == id).cloned()
    }

    /// The document commands act on: the one with the keyboard focus, else the one that had it last.
    pub fn focused(&self) -> Option<Rc<Document>> {
        let documents = self.documents.borrow();
        documents.get(self.focused.get()).or_else(|| documents.first()).cloned()
    }

    /// The area under the documents, where notes show.
    pub fn content(&self) -> &NSView {
        &self.content
    }

    /// Puts `document` at `index` of the documents, or last; two at most share a window tab.
    pub fn insert(&self, document: Rc<Document>, index: Option<usize>) {
        let count = self.documents.borrow().len();
        let index = index.unwrap_or(count).min(count);
        match self.documents.borrow().get(index) {
            Some(after) => self.area.addSubview_positioned_relativeTo(
                &document.host,
                objc2_app_kit::NSWindowOrderingMode::Below,
                Some(&after.host),
            ),
            None => self.area.addSubview(&document.host),
        }
        self.documents.borrow_mut().insert(index, document);
        self.focused.set(index);
        self.area.adjustSubviews();
        if count == 1 {
            self.set_share(0.5);
        }
        self.refresh();
    }

    /// Takes the document `id` out of the window tab.
    pub fn remove(&self, id: isize) -> Option<Rc<Document>> {
        let index = self.documents.borrow().iter().position(|doc| doc.id == id)?;
        let document = self.documents.borrow_mut().remove(index);
        document.host.removeFromSuperview();
        let focused = self.focused.get();
        if focused >= index && focused > 0 {
            self.focused.set(focused - 1);
        }
        self.hold_share();
        self.area.adjustSubviews();
        self.refresh();
        Some(document)
    }

    /// The left document's share of the width while there are two.
    pub fn share(&self) -> Option<f64> {
        let documents = self.documents.borrow();
        let (Some(left), Some(_)) = (documents.first(), documents.get(1)) else {
            return None;
        };
        let total = self.area.bounds().size.width;
        (total > 1.0).then(|| (left.host.frame().size.width / total).clamp(0.1, 0.9))
    }

    /// Gives the left of two documents `share` of the width, which it keeps as the tab changes size.
    pub fn set_share(&self, share: f64) {
        self.ratio.set(share.clamp(0.1, 0.9));
        self.hold_share();
    }

    /// Holds the left document at the kept share of the width, above the split view's own sizing, which shrinks one
    /// side only, and below a drag of the divider.
    fn hold_share(&self) {
        for constraint in self.holding.borrow_mut().drain(..) {
            constraint.setActive(false);
        }
        let documents = self.documents.borrow();
        let (Some(left), Some(right)) = (documents.first(), documents.get(1)) else {
            return;
        };
        let share = left
            .host
            .widthAnchor()
            .constraintEqualToAnchor_multiplier(&self.area.widthAnchor(), self.ratio.get());
        share.setPriority(SHARE_PRIORITY);
        let mut holding = vec![share];
        for side in [left, right] {
            let least = side.host.widthAnchor().constraintGreaterThanOrEqualToConstant(MIN_SIDE);
            least.setPriority(SHARE_PRIORITY + 10.0);
            holding.push(least);
        }
        for constraint in &holding {
            constraint.setActive(true);
        }
        *self.holding.borrow_mut() = holding;
    }

    /// Takes the share the user drags the divider to.
    fn follow_drags(tab: &Rc<TabWindow>) {
        let dragged = Rc::downgrade(tab);
        tab.area.on_drag(move || {
            if let Some(tab) = dragged.upgrade()
                && let Some(share) = tab.share()
            {
                tab.set_share(share);
            }
        });
    }

    /// Makes the document at `index` the one commands act on, as the keyboard focus moves into it.
    fn set_focused(&self, index: usize) {
        if self.focused.replace(index) != index {
            self.refresh();
        }
    }

    /// Follows the keyboard focus: the document that has it becomes the focused one and remembers where it was.
    pub fn follow_focus(&self) {
        let responder = self.window.firstResponder();
        let Some(view) = responder.and_then(|r| Retained::downcast::<NSView>(r).ok()) else {
            return;
        };
        let found = self.documents.borrow().iter().position(|doc| doc.holds(&view));
        if let Some(index) = found {
            if let Some(doc) = self.documents.borrow().get(index) {
                *doc.focus.borrow_mut() = Some(Weak::from(&*view));
            }
            self.set_focused(index);
        }
    }

    /// Makes the document `id` the one commands act on, without moving the keyboard focus.
    pub fn mark_focused(&self, id: isize) {
        let found = self.documents.borrow().iter().position(|doc| doc.id == id);
        if let Some(index) = found {
            self.set_focused(index);
        }
    }

    /// Gives the keyboard focus to the document `id`.
    pub fn activate(&self, id: isize) {
        let found = self.documents.borrow().iter().position(|doc| doc.id == id);
        let Some(index) = found else { return };
        self.set_focused(index);
        let target = self.documents.borrow().get(index).and_then(|doc| doc.focus_target());
        match target {
            Some(view) => self.window.makeFirstResponder(Some(&view)),
            None => self.window.makeFirstResponder(None),
        };
    }

    /// Gives the keyboard focus to the other document of two.
    pub fn focus_other(&self) {
        let documents = self.documents();
        if documents.len() < 2 {
            return;
        }
        let next = (self.focused.get() + 1) % documents.len();
        self.activate(documents[next].id);
    }

    /// Says on the tab what the focused document is and whether a document has unsaved changes, and shows the
    /// badge of the second document while two share the tab.
    pub fn refresh(&self) {
        let documents = self.documents.borrow();
        let focused = self.focused.get().min(documents.len().saturating_sub(1));
        let two = documents.len() > 1;
        for (index, doc) in documents.iter().enumerate() {
            doc.set_badge(two && index > 0, index == focused);
        }
        let Some(doc) = documents.get(focused) else { return };
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
        }
        let current = tab.accessoryView(self.window.mtm());
        if status.is_some() && current.is_none() {
            tab.setAccessoryView(Some(&self.status));
        } else if status.is_none() && current.is_some() {
            tab.setAccessoryView(None);
        }
    }

    /// Puts the folder's sidebar `view` into the tab, in place of its picture.
    pub fn adopt_sidebar(&self, view: &NSView) {
        self.picture.setHidden(true);
        self.picture.setImage(None);
        self.slot.addSubview(view);
        pin_edges(view, &self.slot);
    }

    /// Shows what `view`, the sidebar about to leave, looks like now, for a window that stays on screen.
    pub fn keep_picture_of(&self, view: &NSView) {
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

    /// What the documents' badges say, left to right: the name and the close button's label of those that show.
    #[cfg(feature = "selftest")]
    pub fn badges(&self) -> Vec<Option<(String, String)>> {
        use objc2_app_kit::NSAccessibility;
        self.documents
            .borrow()
            .iter()
            .map(|doc| {
                (!doc.badge.isHidden()).then(|| {
                    let label = doc
                        .close
                        .accessibilityLabel()
                        .map(|l| l.to_string())
                        .unwrap_or_default();
                    (doc.badge_title.stringValue().to_string(), label)
                })
            })
            .collect()
    }

    /// The close button on the badge of the document `id`.
    #[cfg(feature = "selftest")]
    pub fn badge_close(&self, id: isize) -> Option<Retained<NSButton>> {
        self.document(id).map(|doc| doc.close.clone())
    }

    /// Drags the divider to `share` with the mouse: the drag and the release wait in the event queue, which the split
    /// view's tracking of the press reads.
    #[cfg(feature = "selftest")]
    pub fn drag_share(&self, share: f64) {
        use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};

        let Some(left) = self.documents.borrow().first().map(|doc| doc.host.frame().size.width) else {
            return;
        };
        let bounds = self.area.bounds();
        let target = bounds.size.width * share;
        let y = bounds.size.height / 2.0;
        let event = |kind: NSEventType, x: f64| {
            NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                kind,
                self.area.convertPoint_toView(NSPoint::new(x, y), None),
                NSEventModifierFlags::empty(),
                0.0,
                self.window.windowNumber(),
                None,
                0,
                1,
                1.0,
            )
        };
        let app = NSApplication::sharedApplication(self.window.mtm());
        for (kind, x) in [
            (NSEventType::LeftMouseDragged, (left + target) / 2.0),
            (NSEventType::LeftMouseDragged, target),
            (NSEventType::LeftMouseUp, target),
        ] {
            if let Some(event) = event(kind, x) {
                app.postEvent_atStart(&event, false);
            }
        }
        if let Some(down) = event(NSEventType::LeftMouseDown, left + 0.5) {
            // SAFETY: The split view takes the press, which lives for the call.
            let () = unsafe { objc2::msg_send![&*self.area, mouseDown: &*down] };
        }
    }

    /// The narrowest and widest the user may drag the sidebar to now.
    #[cfg(feature = "selftest")]
    pub fn sidebar_range(&self) -> (f64, f64) {
        (self.sidebar.minimumThickness(), self.sidebar.maximumThickness())
    }

    /// Folds the sidebar away or back, as the menu's command does.
    #[cfg(feature = "selftest")]
    pub fn toggle_sidebar(&self) {
        // SAFETY: The action takes no sender.
        unsafe { self.split.toggleSidebar(None) };
    }

    /// Whether the tab shows the picture of the sidebar.
    #[cfg(feature = "selftest")]
    pub fn shows_picture(&self) -> bool {
        !self.picture.isHidden()
    }

    /// The width of the sidebar now.
    pub fn sidebar_width(&self) -> f64 {
        self.sidebar.viewController(self.window.mtm()).view().frame().size.width
    }

    pub fn sidebar_collapsed(&self) -> bool {
        self.sidebar.isCollapsed()
    }

    /// Holds the sidebar at `width`, which a window tab that has not shown yet takes only this way.
    pub fn pin_sidebar(&self, width: f64, collapsed: bool) {
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
    pub fn release_sidebar(&self, min: f64, max: f64) {
        self.sidebar.setMinimumThickness(min);
        self.sidebar.setMaximumThickness(max);
    }
}

/// A tab's title after its icon, which sits on the text's baseline like a letter and is left out for VoiceOver, which
/// reads the title.
fn titled(icon: Option<&NSImage>, title: &str, font: &NSFont) -> Retained<NSAttributedString> {
    let text = NSMutableAttributedString::new();
    if let Some(icon) = icon {
        let size = icon.size();
        let height = ICON_HEIGHT.min(size.height.max(1.0));
        let width = size.width * height / size.height.max(1.0);
        let attachment = NSTextAttachment::new();
        attachment.setImage(Some(icon));
        attachment.setBounds(NSRect::new(
            NSPoint::new(0.0, font.descender()),
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
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightRegular },
    )));
    crate::util::hide_from_voiceover(&status);
    status
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

/// The agents a thread in a worktree of its own can run (I1), each tagged with its place in
/// `crate::worktrees::agents`; the window enables them as it does the agents above.
/// The installed agents' entries for a thread in a worktree of its own, if any agent is installed.
fn worktree_menu(mtm: MainThreadMarker) -> Option<Retained<NSMenuItem>> {
    let agents: Vec<Retained<NSMenuItem>> = crate::worktrees::agents()
        .into_iter()
        .enumerate()
        .filter(|(_, (agent, _))| crate::agent::installed(agent))
        .map(|(n, (agent, _))| {
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

/// What a new tab can show (E17): the page of a new tab lists it, and ⌘T and the tab bar's "+" open that page.
pub fn new_tab_menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let entry = |title: &str, symbol_name: &str, action: Option<Sel>| {
        let entry = item(mtm, title, action, "");
        entry.setImage(symbol(symbol_name, title).as_deref());
        entry.setEnabled(action.is_some());
        entry
    };
    // Only installed agents are offered; the own agent's endpoints and models are filled in each time the menu opens.
    let claude = crate::agent::installed(&crate::agent::Agent::Claude)
        .then(|| entry("Claude", "sparkle", Some(sel!(ideNewClaudeThread:))));
    let presets = crate::agent::installed_presets().into_iter().map(|(index, name)| {
        let agent = entry(&name, "sparkles", Some(sel!(ideNewAcpThread:)));
        agent.setTag(isize::try_from(index).unwrap_or(-1));
        agent
    });
    let own = crate::agent::installed(&crate::agent::Agent::Own).then(|| {
        let (own, _) = submenu(mtm, t("Eigener Agent"), Vec::new());
        own.setImage(symbol("brain", t("Eigener Agent")).as_deref());
        own
    });
    let mut agents: Vec<Retained<NSMenuItem>> = claude.into_iter().chain(presets).chain(own).collect();
    if let Some(worktree) = worktree_menu(mtm) {
        agents.push(separator(mtm));
        agents.push(worktree);
    }
    if !agents.is_empty() {
        agents.push(separator(mtm));
    }
    agents.push(entry(
        t("Agenten verwalten …"),
        "gearshape",
        Some(sel!(ideOpenAgentsSettings:)),
    ));
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

/// Fills the parts of `menu` that change: the CLIs on the login PATH and the own agent's endpoints.
pub fn fill_new_tab_menu(controller: &WindowController, menu: &NSMenu) {
    let clis = menu.itemWithTitle(&ns(t("CLI-Thread"))).and_then(|item| item.submenu());
    if let Some(clis) = clis {
        crate::terminal::fill_cli_menu(controller, &clis);
    }
    let own = menu
        .itemWithTitle(&ns(t("AI-Thread")))
        .and_then(|item| item.submenu())
        .and_then(|agents| agents.itemWithTitle(&ns(t("Eigener Agent"))))
        .and_then(|item| item.submenu());
    if let Some(own) = own {
        crate::agent::fill_own_menu(controller.mtm(), &own);
    }
}

/// The documents' default size, for a window that has no saved frame.
pub fn default_size() -> NSSize {
    WINDOW_SIZE
}
