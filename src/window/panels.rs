//! The panel beside the activity bar: one page per panel, of which one shows.

use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSColor, NSFont, NSImageView, NSLayoutAttribute, NSLineBreakMode, NSStackView, NSTextAlignment, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::{t, tf};

use super::{Panel, WindowController};
use crate::util::{activate, auto_layout, borderless_button, label, ns, pin_inset, pin_sides, symbol};
use ide_ui_common::ui_size;

/// How far the panel's title keeps from the sidebar's leading edge.
const HEADER_INSET: f64 = 8.0;
/// How far the title's vertical centre sits below the panel's top.
const HEADER_MIDDLE: f64 = 17.0;
/// How far the folder tree reaches past the panel's leading edge, which its source list style indents more than the
/// other panels' content.
const TREE_OUTDENT: f64 = 4.0;
/// How far a title's buttons sit below its centre: symbols are taller than the title's capitals, and this lets
/// both start at the same height.
const ACCESSORY_DROP: f64 = 2.0;
/// How far a page keeps below the panel's title.
const PAGE_GAP: f64 = 8.0;
/// The size of the symbol of a place without content.
const EMPTY_SYMBOL: f64 = 28.0;
pub(crate) use ide_ui_common::panel::ACCESSORY_INSET;

pub(crate) struct Panels {
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
    pub(crate) fn new(mtm: MainThreadMarker, _target: &WindowController, project: Option<&Path>) -> Panels {
        let view = NSView::new(mtm);
        let header = ide_ui_common::panel::heading(mtm, "");
        header.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        let accessory = NSStackView::new(mtm);
        accessory.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        view.addSubview(&header);
        view.addSubview(&accessory);
        auto_layout(&[&header, &accessory]);
        let middle = header
            .centerYAnchor()
            .constraintEqualToAnchor_constant(&view.topAnchor(), HEADER_MIDDLE);
        let drop = accessory
            .centerYAnchor()
            .constraintEqualToAnchor_constant(&header.centerYAnchor(), ACCESSORY_DROP);
        ui_size::follow_length(&middle, HEADER_MIDDLE);
        ui_size::follow_length(&drop, ACCESSORY_DROP);
        activate([
            header
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&view.leadingAnchor(), HEADER_INSET),
            middle,
            accessory
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&view.trailingAnchor(), -ACCESSORY_INSET),
            drop,
            header
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&accessory.leadingAnchor(), -4.0),
        ]);
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

    pub(crate) fn view(&self) -> &NSView {
        &self.view
    }

    /// Rebuilds the pages for `project`.
    pub(crate) fn set_project(&self, project: Option<&Path>) {
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
            page.setTranslatesAutoresizingMaskIntoConstraints(false);
            page.setHidden(true);
            pages.push((panel, page));
        }
        *self.pages.borrow_mut() = pages;
        self.show(self.shown.get());
    }

    /// Puts `page` into the panel, below the title, unless it is there.
    fn attach(&self, page: &NSView) {
        if placed(page) {
            return;
        }
        self.view.addSubview(page);
        pin_sides(page, &self.view, 0.0);
        let below_title = page
            .topAnchor()
            .constraintEqualToAnchor_constant(&self.header.bottomAnchor(), PAGE_GAP);
        ui_size::follow_length(&below_title, PAGE_GAP);
        activate([
            below_title,
            page.bottomAnchor().constraintEqualToAnchor(&self.view.bottomAnchor()),
        ]);
    }

    /// Takes the pages of the panels that do not show out of the panel, so a move of the sidebar into another window
    /// tab moves only the page that shows; each comes in when its panel shows.
    pub(crate) fn leave_out_hidden(&self) {
        let shown = self.shown.get();
        for (panel, page) in self.pages.borrow().iter() {
            if *panel != shown && placed(page) {
                page.removeFromSuperview();
            }
        }
    }

    /// Shows `view`, the folder tree, on the files page, and `buttons` at the end of its title row.
    pub(crate) fn set_files(&self, view: &NSView, buttons: &NSView) {
        self.place(Panel::Files, view, -TREE_OUTDENT);
        self.set_accessory(Panel::Files, buttons);
    }

    /// Shows `view` at the end of the title row of `panel`.
    pub(crate) fn set_accessory(&self, panel: Panel, view: &NSView) {
        let mut accessories = self.accessories.borrow_mut();
        accessories.retain(|(owner, _)| *owner != panel);
        accessories.push((panel, objc2::Message::retain(view)));
        drop(accessories);
        self.show(self.shown.get());
    }

    /// Shows `view` on the page of `panel`, in place of what it showed.
    pub(crate) fn set_page(&self, panel: Panel, view: &NSView) {
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
        let insets = NSEdgeInsets {
            top: 0.0,
            left: leading,
            bottom: 0.0,
            right: 0.0,
        };
        pin_inset(view, page, insets);
    }

    pub(crate) fn show(&self, panel: Panel) {
        self.shown.set(panel);
        let folder = self.folder.borrow().is_some();
        self.header.setHidden(!folder);
        self.accessory.setHidden(!folder);
        self.header.setStringValue(&ns(&self.heading(panel)));
        for view in self.accessory.arrangedSubviews().iter() {
            self.accessory.removeView(&view);
        }
        if let Some((_, view)) = self.accessories.borrow().iter().find(|(owner, _)| *owner == panel) {
            self.accessory.addArrangedSubview(view);
        }
        for (candidate, page) in self.pages.borrow().iter() {
            if *candidate == panel {
                self.attach(page);
            }
            page.setHidden(*candidate != panel);
        }
    }

    /// The page of the panel that shows.
    pub(crate) fn shown_page(&self) -> Option<Retained<NSView>> {
        let shown = self.shown.get();
        self.pages
            .borrow()
            .iter()
            .find(|(panel, _)| *panel == shown)
            .map(|(_, page)| page.clone())
    }

    /// The title of `panel`; the files panel names the folder it shows.
    pub(crate) fn heading(&self, panel: Panel) -> String {
        match (panel, self.folder.borrow().as_deref()) {
            (Panel::Files, Some(folder)) => tf("Dateien: {}", &[folder]),
            _ => panel.title().to_owned(),
        }
    }

    fn page(&self, panel: Panel, project: Option<&Path>) -> Retained<NSView> {
        let mtm = self.mtm;
        let state = match (panel, project) {
            // Every panel needs a folder; without one they all show the same.
            (_, None) => empty_state(
                mtm,
                "folder",
                t("Kein Ordner geöffnet"),
                t("Öffne einen Ordner, um mit ihm zu arbeiten."),
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
            (panel, _) => empty_state(mtm, panel.symbol(), panel.title(), t("In Vorbereitung"), None),
        };
        let page = NSView::new(mtm);
        page.addSubview(&state);
        state.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
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
        ]);
        page
    }
}

pub(crate) use ide_ui_common::panel::{header_button, header_row};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

/// A symbol, a title, a line of explanation and maybe a button, centred, as macOS shows a place without content.
pub(crate) fn empty_state(
    mtm: MainThreadMarker,
    symbol_name: &str,
    title: &str,
    detail: &str,
    button: Option<(&str, Sel)>,
) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::CenterX);
    ui_size::follow_spacing(&stack, ide_ui_common::style::space::M);
    let own = ide_ui_common::util::is_own_symbol(symbol_name);
    let image = if own {
        ide_ui_common::util::symbol_at(symbol_name, title, EMPTY_SYMBOL)
    } else {
        symbol(symbol_name, title)
    };
    if let Some(image) = image {
        let view = NSImageView::imageViewWithImage(&image, mtm);
        let (name, description) = (symbol_name.to_owned(), title.to_owned());
        ui_size::keep(&*view, ui_size::Aspect::Symbol, move |view| {
            let points = ui_size::scaled(EMPTY_SYMBOL);
            if own {
                view.setImage(ide_ui_common::util::symbol_at(&name, &description, points).as_deref());
            }
            view.setSymbolConfiguration(Some(
                &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
                    points,
                    crate::util::constant!(objc2_app_kit::NSFontWeightLight),
                ),
            ));
        });
        view.setContentTintColor(Some(&NSColor::tertiaryLabelColor()));
        stack.addArrangedSubview(&view);
    }
    let heading = label(mtm, title);
    ui_size::set_font(&heading, &NSFont::boldSystemFontOfSize(ide_ui_common::style::HEADING));
    heading.setAlignment(NSTextAlignment::Center);
    stack.addArrangedSubview(&heading);
    let explanation = NSTextField::wrappingLabelWithString(&ns(detail), mtm);
    ui_size::set_system_font(&explanation, ide_ui_common::style::BODY);
    explanation.setTextColor(Some(&NSColor::secondaryLabelColor()));
    explanation.setAlignment(NSTextAlignment::Center);
    explanation.setPreferredMaxLayoutWidth(200.0);
    stack.addArrangedSubview(&explanation);
    if let Some((title, action)) = button {
        let button = borderless_button(mtm, title, None, action);
        ui_size::set_system_font(&button, ide_ui_common::style::BODY);
        button.setContentTintColor(Some(&NSColor::controlAccentColor()));
        stack.addArrangedSubview(&button);
    }
    stack
}

/// Whether `page` is in a view.
fn placed(page: &NSView) -> bool {
    // SAFETY: Asking a view for its superview on the main thread, where views live.
    unsafe { page.superview() }.is_some()
}
