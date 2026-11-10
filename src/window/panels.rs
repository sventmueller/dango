//! The panel beside the activity bar: one page per panel, of which one shows.

use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSColor, NSFont, NSImageView, NSLayoutAttribute, NSStackView, NSTextAlignment, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use splat_i18n::t;

use super::{Panel, WindowController};
use crate::util::{borderless_button, label, ns, symbol};

/// How far the panel's title keeps from the activity bar's divider.
const HEADER_INSET: f64 = 8.0;

pub struct Panels {
    mtm: MainThreadMarker,
    shown: std::cell::Cell<Panel>,
    view: Retained<NSView>,
    header: Retained<NSTextField>,
    pages: RefCell<Vec<(Panel, Retained<NSView>)>>,
}

impl Panels {
    pub fn new(mtm: MainThreadMarker, _target: &WindowController, project: Option<&Path>) -> Panels {
        let view = NSView::new(mtm);
        let header = label(mtm, "");
        header.setFont(Some(&NSFont::boldSystemFontOfSize(11.0)));
        header.setTextColor(Some(&NSColor::secondaryLabelColor()));
        view.addSubview(&header);
        header.setTranslatesAutoresizingMaskIntoConstraints(false);
        header
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&view.leadingAnchor(), HEADER_INSET)
            .setActive(true);
        header
            .topAnchor()
            .constraintEqualToAnchor_constant(&view.topAnchor(), 10.0)
            .setActive(true);
        let panels = Panels {
            mtm,
            view,
            header,
            pages: RefCell::new(vec![]),
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

    /// Shows `view`, the folder tree, on the files page.
    pub fn set_files(&self, view: &NSView) {
        self.set_page(Panel::Files, view);
    }

    /// Shows `view` on the page of `panel`, in place of what it showed.
    pub fn set_page(&self, panel: Panel, view: &NSView) {
        let pages = self.pages.borrow();
        let Some((_, page)) = pages.iter().find(|(candidate, _)| *candidate == panel) else {
            return;
        };
        for subview in page.subviews().iter() {
            subview.removeFromSuperview();
        }
        page.addSubview(view);
        crate::util::pin_edges(view, page);
    }

    pub fn show(&self, panel: Panel) {
        self.shown.set(panel);
        self.header.setStringValue(&ns(panel.title()));
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

    #[cfg(feature = "selftest")]
    pub fn title(&self) -> String {
        self.header.stringValue().to_string()
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
