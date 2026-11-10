//! Window tabs (E60): every document gets a native macOS tab, an `NSWindow` of its own in the folder's tab group with
//! its own toolbar and split view; the folder's one sidebar moves into the tab in front. A window tab shows one
//! document, or two side by side, each then under a header that names it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSBackingStoreType, NSButton, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSImage,
    NSImageAlignment, NSImageScaling, NSImageSymbolConfiguration, NSImageView, NSLayoutConstraint, NSLineBreakMode,
    NSMenu, NSMenuItem, NSResponder, NSSplitView, NSSplitViewController, NSSplitViewDividerStyle, NSSplitViewItem,
    NSStackView, NSTextField, NSToolbar, NSToolbarDisplayMode, NSUserInterfaceLayoutOrientation, NSView,
    NSViewController, NSWindow, NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_app_kit::{NSAttributedStringAttachmentConveniences, NSTextAttachment};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
};
use splat_i18n::{t, tf};

use super::WindowController;
use crate::util::{item, ns, pin_edges, separator, submenu, symbol, symbol_button};

const WINDOW_SIZE: NSSize = NSSize::new(1240.0, 780.0);
const MIN_WINDOW_SIZE: NSSize = NSSize::new(720.0, 440.0);
/// The height of a document's header while its window tab shows two documents.
const HEADER_HEIGHT: f64 = 26.0;
/// The height of the icon before a tab's title.
const ICON_HEIGHT: f64 = 14.0;
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

/// A document of a window tab: its view and what its tab and header say about it.
pub struct Document {
    pub id: isize,
    pub kind: TabKind,
    /// What the document shows, such as a file's path, so it opens once.
    pub key: Option<String>,
    pub view: Retained<NSView>,
    /// The header above `view` and `view`, which sit side by side with another document's.
    host: Retained<NSView>,
    header_height: Retained<NSLayoutConstraint>,
    header: Retained<NSView>,
    header_title: Retained<NSTextField>,
    header_icon: Option<Retained<NSImageView>>,
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
        let header = NSView::new(mtm);
        let header_title = NSTextField::labelWithString(&ns(title), mtm);
        header_title.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        header_title.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        header_title.setContentCompressionResistancePriority_forOrientation(
            200.0,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        let close = symbol_button(
            mtm,
            "xmark",
            &tf("{} schließen", &[title]),
            9.0,
            Some(controller),
            sel!(ideCloseTabButton:),
        );
        close.setTag(id);
        close.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let header_icon = symbol(kind.symbol(), "").map(|image| NSImageView::imageViewWithImage(&image, mtm));
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(6.0);
        if let Some(icon) = &header_icon {
            icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
            row.addArrangedSubview(icon);
        }
        row.addArrangedSubview(&header_title);
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Horizontal);
        row.addArrangedSubview(&spacer);
        row.addArrangedSubview(&close);
        header.addSubview(&row);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        host.addSubview(&header);
        host.addSubview(&view);
        for item in [&*header as &NSView, &view] {
            item.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        let header_height = header.heightAnchor().constraintEqualToConstant(0.0);
        for constraint in [
            row.leadingAnchor()
                .constraintEqualToAnchor_constant(&header.leadingAnchor(), 10.0),
            row.trailingAnchor()
                .constraintEqualToAnchor_constant(&header.trailingAnchor(), -8.0),
            row.centerYAnchor().constraintEqualToAnchor(&header.centerYAnchor()),
            header.topAnchor().constraintEqualToAnchor(&host.topAnchor()),
            header.leadingAnchor().constraintEqualToAnchor(&host.leadingAnchor()),
            header.trailingAnchor().constraintEqualToAnchor(&host.trailingAnchor()),
            header_height.clone(),
            view.topAnchor().constraintEqualToAnchor(&header.bottomAnchor()),
            loose(view.topAnchor().constraintEqualToAnchor(&host.topAnchor())),
            view.leadingAnchor().constraintEqualToAnchor(&host.leadingAnchor()),
            view.trailingAnchor().constraintEqualToAnchor(&host.trailingAnchor()),
            view.bottomAnchor().constraintEqualToAnchor(&host.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        header.setHidden(true);
        Rc::new(Document {
            id,
            kind,
            key,
            view,
            host,
            header_height,
            header,
            header_title,
            header_icon,
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
        self.header_title.setStringValue(&ns(title));
        let close = tf("{} schließen", &[title]);
        self.close.setToolTip(Some(&ns(&close)));
        crate::util::set_label(&self.close, Some(&close));
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
        if let Some(icon) = &self.header_icon {
            icon.setContentTintColor(Some(color));
        }
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
        let agent = matches!(self.kind, TabKind::Agent | TabKind::Plan)
            || (self.kind == TabKind::Cli && self.title().starts_with("claude"));
        let color = match tint {
            Some(tint) => tint,
            None if agent => crate::agent::style::terracotta(),
            None => NSColor::secondaryLabelColor(),
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

    /// Shows the header, which names the document and closes it, while it shares its window tab; above the document
    /// when `above`, else in the docked bar.
    fn set_header(&self, shown: bool, above: bool, focused: bool) {
        self.header.setHidden(!shown);
        self.header_height
            .setConstant(if shown || !above { HEADER_HEIGHT } else { 0.0 });
        let color = if focused {
            NSColor::labelColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        self.header_title.setTextColor(Some(&color));
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

/// Holds the view's top at the host's while the header is in the bar above the documents instead.
fn loose(constraint: Retained<NSLayoutConstraint>) -> Retained<NSLayoutConstraint> {
    constraint.setPriority(250.0);
    constraint
}

/// Whether macOS docks bars above a split view item's content (macOS 26 and later), where the bars of the title, the
/// tabs and the toolbar fade what lies right below them, so a header there has to be such a bar.
fn accessories_available() -> bool {
    objc2::runtime::AnyClass::get(c"NSSplitViewItemAccessoryViewController").is_some()
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
    /// The bar docked above the documents that holds their headers while two share the tab, where macOS has it.
    bar: Option<(
        Retained<objc2_app_kit::NSSplitViewItemAccessoryViewController>,
        Retained<NSView>,
    )>,
    area: Retained<NSSplitView>,
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
        let area = NSSplitView::new(mtm);
        area.setVertical(true);
        area.setDividerStyle(NSSplitViewDividerStyle::Thin);
        content.addSubview(&area);
        area.setTranslatesAutoresizingMaskIntoConstraints(false);
        // Below the title bar and the tab bar, which the content reaches under, so the headers of two documents show.
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
        let bar = accessories_available().then(|| {
            let view = NSView::new(mtm);
            view.heightAnchor()
                .constraintEqualToConstant(HEADER_HEIGHT)
                .setActive(true);
            // AppKit exposes a docked bar to VoiceOver whatever it holds; a group passes its headers on.
            view.setAccessibilityRole(Some(crate::util::constant!(objc2_app_kit::NSAccessibilityGroupRole)));
            let accessory = objc2_app_kit::NSSplitViewItemAccessoryViewController::new(mtm);
            accessory.setView(&view);
            accessory.setHidden(true);
            content_item.addTopAlignedAccessoryViewController(&accessory);
            (accessory, view)
        });
        split.addSplitViewItem(&sidebar);
        split.addSplitViewItem(&content_item);
        window.setContentViewController(Some(&split));

        let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &ns("IDEProjectToolbar"));
        toolbar.setDelegate(Some(ProtocolObject::from_ref(controller)));
        toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
        window.setToolbar(Some(&toolbar));

        let status = status_dot(mtm);
        Rc::new(TabWindow {
            window,
            split,
            sidebar,
            slot,
            picture,
            content,
            bar,
            area,
            documents: RefCell::new(Vec::new()),
            focused: Cell::new(0),
            status,
        })
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
        let share = self.share();
        match self.documents.borrow().get(index) {
            Some(after) => self.area.addSubview_positioned_relativeTo(
                &document.host,
                objc2_app_kit::NSWindowOrderingMode::Below,
                Some(&after.host),
            ),
            None => self.area.addSubview(&document.host),
        }
        if let Some((_, bar)) = &self.bar {
            document.header.removeFromSuperview();
            bar.addSubview(&document.header);
            for constraint in [
                document.header.topAnchor().constraintEqualToAnchor(&bar.topAnchor()),
                document
                    .header
                    .bottomAnchor()
                    .constraintEqualToAnchor(&bar.bottomAnchor()),
                document
                    .header
                    .leadingAnchor()
                    .constraintEqualToAnchor(&document.host.leadingAnchor()),
                document
                    .header
                    .trailingAnchor()
                    .constraintEqualToAnchor(&document.host.trailingAnchor()),
            ] {
                constraint.setActive(true);
            }
        }
        self.documents.borrow_mut().insert(index, document);
        self.focused.set(index);
        self.area.adjustSubviews();
        if count == 1 {
            self.set_share(share.unwrap_or(0.5));
        }
        self.refresh();
    }

    /// Takes the document `id` out of the window tab.
    pub fn remove(&self, id: isize) -> Option<Rc<Document>> {
        let index = self.documents.borrow().iter().position(|doc| doc.id == id)?;
        let document = self.documents.borrow_mut().remove(index);
        if self.bar.is_some() {
            document.header.removeFromSuperview();
        }
        document.host.removeFromSuperview();
        let focused = self.focused.get();
        if focused >= index && focused > 0 {
            self.focused.set(focused - 1);
        }
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

    pub fn set_share(&self, share: f64) {
        if self.documents.borrow().len() < 2 {
            return;
        }
        let total = self.area.bounds().size.width;
        if total > 1.0 {
            let position = (total * share).clamp(MIN_SIDE, (total - MIN_SIDE).max(MIN_SIDE));
            self.area.setPosition_ofDividerAtIndex(position, 0);
        }
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
    /// headers while two documents share the tab.
    pub fn refresh(&self) {
        let documents = self.documents.borrow();
        let focused = self.focused.get().min(documents.len().saturating_sub(1));
        let two = documents.len() > 1;
        let docked = self.bar.is_some();
        for (index, doc) in documents.iter().enumerate() {
            doc.set_header(two, !docked, index == focused);
        }
        if let Some((accessory, _)) = &self.bar {
            accessory.setHidden(!two);
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
        self.split.splitView().setPosition_ofDividerAtIndex(width, 0);
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
