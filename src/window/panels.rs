//! The panel beside the activity bar: one page per panel, of which one shows.

use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSColor, NSFont, NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation, NSLineBreakMode, NSStackView,
    NSTextAlignment, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use splat_i18n::{t, tf};

use super::{Panel, WindowController};
use crate::util::{borderless_button, label, ns, symbol};

/// How far the panel's title keeps from the sidebar's leading edge.
const HEADER_INSET: f64 = 8.0;
/// How far the title's vertical centre sits below the panel's top.
const HEADER_MIDDLE: f64 = 17.0;
/// How far the folder tree reaches past the panel's leading edge, which its source list style indents more than the
/// other panels' content.
const TREE_OUTDENT: f64 = 5.0;
/// How far a title's buttons sit below its centre: symbols are taller than the title's capitals, and this lets
/// both start at the same height.
const ACCESSORY_DROP: f64 = 2.0;

/// Where the parts of a panel's title row sit, in the panel's coordinates.
#[cfg(feature = "selftest")]
pub struct TitleRow {
    pub title_middle: f64,
    pub buttons_middle: f64,
    pub page_top: f64,
    pub buttons_start: f64,
    pub buttons_end: f64,
    pub text_end: f64,
    pub width: f64,
}

pub struct Panels {
    mtm: MainThreadMarker,
    shown: std::cell::Cell<Panel>,
    view: Retained<NSView>,
    header: Retained<NSTextField>,
    accessory: Retained<NSStackView>,
    folder: RefCell<Option<String>>,
    pages: RefCell<Vec<(Panel, Retained<NSView>)>>,
    accessories: RefCell<Vec<(Panel, Retained<NSView>)>>,
}

impl Panels {
    pub fn new(mtm: MainThreadMarker, _target: &WindowController, project: Option<&Path>) -> Panels {
        let view = NSView::new(mtm);
        let header = label(mtm, "");
        header.setFont(Some(&NSFont::boldSystemFontOfSize(11.0)));
        header.setTextColor(Some(&NSColor::secondaryLabelColor()));
        header.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        header.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
        let accessory = NSStackView::new(mtm);
        accessory.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        view.addSubview(&header);
        view.addSubview(&accessory);
        header.setTranslatesAutoresizingMaskIntoConstraints(false);
        accessory.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            header
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&view.leadingAnchor(), HEADER_INSET),
            header
                .centerYAnchor()
                .constraintEqualToAnchor_constant(&view.topAnchor(), HEADER_MIDDLE),
            accessory
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&view.trailingAnchor(), -6.0),
            accessory
                .centerYAnchor()
                .constraintEqualToAnchor_constant(&header.centerYAnchor(), ACCESSORY_DROP),
            header
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&accessory.leadingAnchor(), -4.0),
        ] {
            constraint.setActive(true);
        }
        let panels = Panels {
            mtm,
            view,
            header,
            accessory,
            folder: RefCell::new(None),
            pages: RefCell::new(vec![]),
            accessories: RefCell::new(vec![]),
            shown: std::cell::Cell::new(Panel::Files),
        };
        panels.set_project(project);
        panels
    }

    pub fn view(&self) -> &NSView {
        &self.view
    }

    /// Rebuilds the pages for `project`.
    pub fn set_project(&self, project: Option<&Path>) {
        *self.folder.borrow_mut() = project
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned());
        self.accessories.borrow_mut().clear();
        for (_, page) in self.pages.borrow().iter() {
            page.removeFromSuperview();
        }
        let mut pages = vec![];
        for panel in Panel::ALL {
            let page = self.page(panel, project);
            self.view.addSubview(&page);
            page.setTranslatesAutoresizingMaskIntoConstraints(false);
            for constraint in [
                page.leadingAnchor().constraintEqualToAnchor(&self.view.leadingAnchor()),
                page.trailingAnchor()
                    .constraintEqualToAnchor(&self.view.trailingAnchor()),
                page.topAnchor()
                    .constraintEqualToAnchor_constant(&self.header.bottomAnchor(), 8.0),
                page.bottomAnchor().constraintEqualToAnchor(&self.view.bottomAnchor()),
            ] {
                constraint.setActive(true);
            }
            page.setHidden(true);
            pages.push((panel, page));
        }
        *self.pages.borrow_mut() = pages;
        self.show(self.shown.get());
    }

    /// Shows `view`, the folder tree, on the files page, and `buttons` at the end of its title row.
    pub fn set_files(&self, view: &NSView, buttons: &NSView) {
        self.place(Panel::Files, view, -TREE_OUTDENT);
        let mut accessories = self.accessories.borrow_mut();
        accessories.retain(|(panel, _)| *panel != Panel::Files);
        accessories.push((Panel::Files, objc2::Message::retain(buttons)));
        drop(accessories);
        self.show(self.shown.get());
    }

    /// Shows `view` on the page of `panel`, in place of what it showed.
    pub fn set_page(&self, panel: Panel, view: &NSView) {
        self.place(panel, view, 0.0);
    }

    /// Fills the page of `panel` with `view`, its leading edge `leading` from the page's.
    fn place(&self, panel: Panel, view: &NSView, leading: f64) {
        let pages = self.pages.borrow();
        let Some((_, page)) = pages.iter().find(|(candidate, _)| *candidate == panel) else {
            return;
        };
        for subview in page.subviews().iter() {
            subview.removeFromSuperview();
        }
        page.addSubview(view);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            view.leadingAnchor()
                .constraintEqualToAnchor_constant(&page.leadingAnchor(), leading),
            view.trailingAnchor().constraintEqualToAnchor(&page.trailingAnchor()),
            view.topAnchor().constraintEqualToAnchor(&page.topAnchor()),
            view.bottomAnchor().constraintEqualToAnchor(&page.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
    }

    pub fn show(&self, panel: Panel) {
        self.shown.set(panel);
        self.header.setStringValue(&ns(&self.heading(panel)));
        for view in self.accessory.arrangedSubviews().iter() {
            self.accessory.removeView(&view);
        }
        if let Some((_, view)) = self.accessories.borrow().iter().find(|(owner, _)| *owner == panel) {
            self.accessory.addArrangedSubview(view);
        }
        for (candidate, page) in self.pages.borrow().iter() {
            page.setHidden(*candidate != panel);
        }
    }

    /// The page of the panel that shows.
    pub fn shown_page(&self) -> Option<Retained<NSView>> {
        let shown = self.shown.get();
        self.pages
            .borrow()
            .iter()
            .find(|(panel, _)| *panel == shown)
            .map(|(_, page)| page.clone())
    }

    /// The title of `panel`; the files panel names the folder it shows.
    pub fn heading(&self, panel: Panel) -> String {
        match (panel, self.folder.borrow().as_deref()) {
            (Panel::Files, Some(folder)) => tf("Dateien: {}", &[folder]),
            _ => panel.title().to_owned(),
        }
    }

    #[cfg(feature = "selftest")]
    pub fn title(&self) -> String {
        self.header.stringValue().to_string()
    }

    /// The geometry of the title row, in the panel's coordinates.
    #[cfg(feature = "selftest")]
    pub fn title_row(&self) -> TitleRow {
        use objc2_foundation::NSRect;
        self.view.layoutSubtreeIfNeeded();
        let in_panel = |view: &NSView| -> NSRect { view.convertRect_toView(view.bounds(), Some(&self.view)) };
        let middle = |rect: NSRect| rect.origin.y + rect.size.height / 2.0;
        let title = in_panel(&self.header);
        let buttons = self.accessory.arrangedSubviews();
        let first = buttons.firstObject().map(|view| in_panel(&view));
        let last = buttons.lastObject().map(|view| in_panel(&view));
        let page = self.shown_page().map_or(f64::NAN, |page| {
            let rect = in_panel(&page);
            if self.view.isFlipped() {
                rect.origin.y
            } else {
                rect.origin.y + rect.size.height
            }
        });
        TitleRow {
            title_middle: middle(title),
            buttons_middle: first.map_or(f64::NAN, middle),
            page_top: page,
            buttons_start: first.map_or(f64::NAN, |rect| rect.origin.x),
            buttons_end: last.map_or(f64::NAN, |rect| rect.origin.x + rect.size.width),
            text_end: title.origin.x + self.header.intrinsicContentSize().width,
            width: self.view.bounds().size.width,
        }
    }

    fn page(&self, panel: Panel, project: Option<&Path>) -> Retained<NSView> {
        let mtm = self.mtm;
        let state = match (panel, project) {
            (Panel::Files, None) => empty_state(
                mtm,
                "folder",
                t("Kein Ordner geöffnet"),
                t("Öffne einen Ordner, um seine Dateien hier zu sehen."),
                Some((t("Ordner öffnen …"), sel!(ideOpenFolder:))),
            ),
            (Panel::Files, Some(folder)) => empty_state(
                mtm,
                "folder",
                &folder
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                t("In Vorbereitung"),
                None,
            ),
            (Panel::Search, None) => empty_state(
                mtm,
                panel.symbol(),
                panel.title(),
                t("Öffne einen Ordner, um darin zu suchen."),
                Some((t("Ordner öffnen …"), sel!(ideOpenFolder:))),
            ),
            (panel, _) => empty_state(mtm, panel.symbol(), panel.title(), t("In Vorbereitung"), None),
        };
        let page = NSView::new(mtm);
        page.addSubview(&state);
        state.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            state.centerXAnchor().constraintEqualToAnchor(&page.centerXAnchor()),
            state
                .centerYAnchor()
                .constraintEqualToAnchor_constant(&page.centerYAnchor(), -40.0),
            state
                .leadingAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&page.leadingAnchor(), 16.0),
            state
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&page.trailingAnchor(), -16.0),
        ] {
            constraint.setActive(true);
        }
        page
    }
}

/// A symbol, a title, a line of explanation and maybe a button, centred, as macOS shows a place without content.
pub fn empty_state(
    mtm: MainThreadMarker,
    symbol_name: &str,
    title: &str,
    detail: &str,
    button: Option<(&str, Sel)>,
) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::CenterX);
    stack.setSpacing(8.0);
    if let Some(image) = symbol(symbol_name, title) {
        let view = NSImageView::imageViewWithImage(&image, mtm);
        view.setSymbolConfiguration(Some(
            &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
                28.0,
                // SAFETY: AppKit sets its font weight constants when it loads.
                unsafe { objc2_app_kit::NSFontWeightLight },
            ),
        ));
        view.setContentTintColor(Some(&NSColor::tertiaryLabelColor()));
        stack.addArrangedSubview(&view);
    }
    let heading = label(mtm, title);
    heading.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
    heading.setAlignment(NSTextAlignment::Center);
    stack.addArrangedSubview(&heading);
    let explanation = NSTextField::wrappingLabelWithString(&ns(detail), mtm);
    explanation.setTextColor(Some(&NSColor::secondaryLabelColor()));
    explanation.setAlignment(NSTextAlignment::Center);
    explanation.setPreferredMaxLayoutWidth(200.0);
    stack.addArrangedSubview(&explanation);
    if let Some((title, action)) = button {
        let button = borderless_button(mtm, title, None, action);
        button.setContentTintColor(Some(&NSColor::controlAccentColor()));
        stack.addArrangedSubview(&button);
    }
    stack
}
