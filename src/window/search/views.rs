//! The views of the search panel: the option toggles for the panel's title row, the field, the replace field behind a
//! disclosure, the file filters, the status line and the list of matches.

use ide_ui_common::style::space;
use ide_ui_common::ui_size;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSButton, NSButtonType, NSCellImagePosition, NSColor, NSControlSize,
    NSControlStateValueOff, NSControlStateValueOn, NSImageSymbolConfiguration, NSLayoutAttribute,
    NSLayoutConstraintOrientation, NSScrollView, NSStackView, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::t;

use super::{SearchOutlineView, SearchPanel, Toggle};
use crate::util::{activate, ns, pin_edges, pin_sides, set_action, symbol};
#[cfg(feature = "selftest")]
mod selftest_probe;

const MARGIN: f64 = 8.0;
const SMALL_FONT: f64 = 11.0;
/// The width of the replace chevron, which the replace field keeps free below it.
const CHEVRON_WIDTH: f64 = 12.0;
/// The size of both chevrons, the same as the folder tree's.
const CHEVRON_POINTS: f64 = 10.0;
/// Where every field begins: after a chevron and the stack's spacing.
const FIELD_INDENT: f64 = CHEVRON_WIDTH + space::XXS;
/// The width of "Alle ersetzen"; the fields without it end where the replace field does.
const ALL_WIDTH: f64 = 18.0;
pub(super) struct Views {
    pub(super) page: Retained<NSView>,
    /// The option toggles for the panel's title row.
    pub(super) header: Retained<NSStackView>,
    pub(super) field: Retained<NSTextField>,
    toggles: Vec<(Toggle, Retained<NSButton>)>,
    pub(super) replace_toggle: Retained<NSButton>,
    replace_row: Retained<NSStackView>,
    pub(super) replace: Retained<NSTextField>,
    pub(super) replace_all: Retained<NSButton>,
    disclosure: Retained<NSButton>,
    pub(super) globs: Retained<NSView>,
    pub(super) include: Retained<NSTextField>,
    pub(super) exclude: Retained<NSTextField>,
    pub(super) status: Retained<NSTextField>,
    results: Retained<NSScrollView>,
    pub(super) outline: Retained<SearchOutlineView>,
    /// The fields and the matches, which give way to a hierarchy.
    text: [Retained<NSView>; 2],
}

impl Views {
    pub(super) fn new(mtm: MainThreadMarker, panel: &SearchPanel) -> Views {
        let page = NSView::new(mtm);
        let field = search_field(mtm, panel);
        let toggles: Vec<(Toggle, Retained<NSButton>)> =
            Toggle::ALL.iter().map(|t| (*t, toggle(mtm, panel, *t))).collect();
        let header = crate::window::panels::header_row(mtm);
        for (_, button) in &toggles {
            header.addArrangedSubview(button);
        }
        let replace_toggle = replace_toggle(mtm, panel);
        let field_row = NSStackView::new(mtm);
        field_row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        ui_size::follow_spacing(&field_row, space::XXS);
        field_row.addArrangedSubview(&replace_toggle);
        field_row.addArrangedSubview(&field);
        field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        let (replace_row, replace, replace_all) = replace_row(mtm, panel);
        let (disclosure_row, disclosure) = disclosure(mtm, panel);
        let include = glob_field(
            mtm,
            panel,
            t("Einschließen, z. B. *.rs, src"),
            t("Dateien einschließen"),
        );
        let exclude = glob_field(
            mtm,
            panel,
            t("Ausschließen, z. B. tests, *.md"),
            t("Dateien ausschließen"),
        );
        let globs = glob_fields(mtm, &include, &exclude);
        let status = NSTextField::wrappingLabelWithString(&ns(""), mtm);
        ui_size::set_system_font(&status, SMALL_FONT);
        status.setTextColor(Some(&NSColor::secondaryLabelColor()));
        status.setHidden(true);
        let bar = top_bar(mtm);
        for view in [&*field_row as &NSView, &replace_row, &disclosure_row, &globs, &status] {
            bar.addArrangedSubview(view);
        }
        for (view, inset) in [
            (&*field_row as &NSView, space::XXS + ALL_WIDTH),
            (&replace_row, 0.0),
            (&globs, space::XXS + ALL_WIDTH),
            (&status, 0.0),
        ] {
            let trailing = view.trailingAnchor().constraintEqualToAnchor(&bar.trailingAnchor());
            ui_size::keep(&*trailing, ui_size::Aspect::Length, move |trailing| {
                trailing.setConstant(-(MARGIN + ui_size::scaled(inset)));
            });
            trailing.setActive(true);
        }
        let (results, outline) = results(mtm, panel);
        let body = NSView::new(mtm);
        body.addSubview(&results);
        pin_edges(&results, &body);
        results.setHidden(true);
        page.addSubview(&bar);
        page.addSubview(&body);
        layout(&page, &bar, &body);
        let text = [bar.into_super(), body];
        Views {
            page,
            header,
            field,
            toggles,
            replace_toggle,
            replace_row,
            replace,
            replace_all,
            disclosure,
            globs,
            include,
            exclude,
            status,
            results,
            outline,
            text,
        }
    }

    /// Shows the text search, or hides it and its toggles for a hierarchy.
    pub(super) fn set_text_shown(&self, shown: bool) {
        for view in &self.text {
            view.setHidden(!shown);
        }
        self.header.setHidden(!shown);
    }

    pub(super) fn replace_shown(&self) -> bool {
        !self.replace_row.isHidden()
    }

    /// Shows or hides the replace field and the replace buttons of the matches.
    pub(super) fn set_replace_shown(&self, shown: bool) {
        self.replace_row.setHidden(!shown);
        self.replace_toggle
            .setImage(symbol(if shown { "chevron.down" } else { "chevron.right" }, "").as_deref());
        self.replace_toggle.setState(if shown {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.replace_toggle.setAccessibilityExpanded(shown);
    }

    pub(super) fn toggled(&self, toggle: Toggle) -> bool {
        self.toggles
            .iter()
            .find(|(t, _)| *t == toggle)
            .is_some_and(|(_, button)| button.state() == NSControlStateValueOn)
    }

    /// Draws each toggle in the accent colour while it is on, as a borderless button shows no state.
    pub(super) fn paint_toggles(&self) {
        for (_, button) in &self.toggles {
            button.setContentTintColor(Some(&toggle_color(button.state() == NSControlStateValueOn)));
        }
    }

    pub(super) fn set_globs_shown(&self, shown: bool) {
        self.globs.setHidden(!shown);
        self.disclosure
            .setImage(symbol(if shown { "chevron.down" } else { "chevron.right" }, "").as_deref());
        self.disclosure.setState(if shown {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.disclosure.setAccessibilityExpanded(shown);
    }

    /// Shows the matches and the status line while the field holds a query.
    pub(super) fn show_results(&self, searching: bool) {
        self.results.setHidden(!searching);
        self.status.setHidden(!searching);
    }

    pub(super) fn set_status(&self, text: &str, problem: bool) {
        self.status.setStringValue(&ns(text));
        let color = if problem {
            NSColor::systemRedColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        self.status.setTextColor(Some(&color));
    }
}

/// A vertical stack for the fields or the switches above a list, `MARGIN` from the sidebar's edges.
pub(super) fn top_bar(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let bar = NSStackView::new(mtm);
    bar.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    bar.setAlignment(NSLayoutAttribute::Leading);
    ui_size::follow_spacing(&bar, space::S);
    ui_size::keep(&*bar, ui_size::Aspect::Insets, |bar| {
        bar.setEdgeInsets(NSEdgeInsets {
            top: 0.0,
            left: MARGIN,
            bottom: ui_size::scaled(space::XS),
            right: MARGIN,
        });
    });
    bar
}

/// Puts `bar` at the top of `page` and `body` below it, both across the page's width.
pub(super) fn layout(page: &NSView, bar: &NSView, body: &NSView) {
    for view in [bar, body] {
        pin_sides(view, page, 0.0);
    }
    let gap = body.topAnchor().constraintEqualToAnchor(&bar.bottomAnchor());
    ui_size::follow_length(&gap, space::XXS);
    activate([
        bar.topAnchor().constraintEqualToAnchor(&page.topAnchor()),
        gap,
        body.bottomAnchor().constraintEqualToAnchor(&page.bottomAnchor()),
    ]);
}

/// Draws `field` in the small size at the interface size.
fn small_field(field: &NSTextField) {
    ui_size::follow_control_size(field, NSControlSize::Small);
    ui_size::set_system_font(field, SMALL_FONT);
}

/// Keeps `view` `base` wide at the interface size.
fn follow_width(view: &NSView, base: f64) {
    let width = view.widthAnchor().constraintEqualToConstant(base);
    ui_size::follow_length(&width, base);
    width.setActive(true);
}

/// A plain field like the replace field below it; typing, Return and Esc reach the panel through the delegate.
fn search_field(mtm: MainThreadMarker, panel: &SearchPanel) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    let title = t("Im Projekt suchen");
    field.setPlaceholderString(Some(&ns(title)));
    crate::util::set_label(&field, Some(title));
    small_field(&field);
    // SAFETY: The field holds its delegate weakly; the window controller keeps the panel alive while the field shows.
    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(panel))) };
    field
}

/// A button in the panel's title row that switches one search option, labelled for VoiceOver with what it switches.
fn toggle(mtm: MainThreadMarker, panel: &SearchPanel, toggle: Toggle) -> Retained<NSButton> {
    let (name, description) = match toggle {
        Toggle::Case => ("textformat", t("Groß-/Kleinschreibung beachten")),
        Toggle::Word => ("character.textbox", t("Ganzes Wort")),
        Toggle::Regex => ("asterisk", t("Regulärer Ausdruck")),
    };
    let button =
        crate::window::panels::header_button(mtm, name, description, Some(panel), sel!(ideSearchOptionChanged:));
    button.setButtonType(NSButtonType::PushOnPushOff);
    button.setState(NSControlStateValueOff);
    button.setContentTintColor(Some(&toggle_color(false)));
    button
}

fn toggle_color(on: bool) -> Retained<NSColor> {
    if on {
        NSColor::controlAccentColor()
    } else {
        NSColor::secondaryLabelColor()
    }
}

/// The chevron before the search field that shows the replace field, as in VS Code.
fn replace_toggle(mtm: MainThreadMarker, panel: &SearchPanel) -> Retained<NSButton> {
    let name = t("Ersetzen ein- oder ausblenden");
    let image = symbol("chevron.right", name).unwrap_or_default();
    // SAFETY: A nil target sends the action along the responder chain until `set_action` names the panel.
    let button =
        unsafe { NSButton::buttonWithImage_target_action(&image, None, Some(sel!(ideSearchToggleReplace:)), mtm) };
    set_action(&button, panel, sel!(ideSearchToggleReplace:));
    button.setBordered(false);
    size_chevron(&button);
    // At its leading edge, where the chevron of the file filters below sits; the title would show beside it.
    button.setTitle(&ns(""));
    button.setImagePosition(NSCellImagePosition::ImageLeading);
    button.setButtonType(NSButtonType::PushOnPushOff);
    button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    button.setToolTip(Some(&ns(name)));
    crate::util::set_label(&button, Some(name));
    button.setAccessibilityExpanded(false);
    button.setTranslatesAutoresizingMaskIntoConstraints(false);
    follow_width(&button, CHEVRON_WIDTH);
    button
}

/// The replace field and its button that replaces every match, hidden until the chevron opens them.
fn replace_row(
    mtm: MainThreadMarker,
    panel: &SearchPanel,
) -> (Retained<NSStackView>, Retained<NSTextField>, Retained<NSButton>) {
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    let title = t("Ersetzen durch");
    field.setPlaceholderString(Some(&ns(title)));
    crate::util::set_label(&field, Some(title));
    small_field(&field);
    // SAFETY: The field holds its delegate weakly; the window controller keeps the panel alive while the field shows.
    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(panel))) };
    field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    let all = ui_size::symbol_button(
        mtm,
        "arrow.2.squarepath",
        t("Alle ersetzen"),
        12.0,
        Some(panel),
        sel!(ideSearchReplaceAll:),
    );
    all.setEnabled(false);
    follow_width(&all, ALL_WIDTH);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    ui_size::follow_spacing(&row, space::XXS);
    // As wide as the chevron above, so the two fields line up.
    let indent = NSView::new(mtm);
    follow_width(&indent, CHEVRON_WIDTH);
    row.addArrangedSubview(&indent);
    row.addArrangedSubview(&field);
    row.addArrangedSubview(&all);
    row.setHidden(true);
    (row, field, all)
}

/// The row that opens the file filters: a chevron as wide as the replace chevron, so the title stays put whichever
/// way the chevron points, and the title, which opens them as well.
fn disclosure(mtm: MainThreadMarker, panel: &SearchPanel) -> (Retained<NSStackView>, Retained<NSButton>) {
    let title = t("Dateien ein- und ausschließen");
    let image = symbol("chevron.right", "").unwrap_or_default();
    // SAFETY: A nil target sends the action along the responder chain until `set_action` names the panel.
    let chevron = unsafe { NSButton::buttonWithImage_target_action(&image, None, Some(sel!(ideSearchDisclose:)), mtm) };
    set_action(&chevron, panel, sel!(ideSearchDisclose:));
    chevron.setBordered(false);
    size_chevron(&chevron);
    // At its leading edge, as the replace chevron above has it, so the two line up; the title would show beside it.
    chevron.setTitle(&ns(""));
    chevron.setImagePosition(NSCellImagePosition::ImageLeading);
    chevron.setButtonType(NSButtonType::PushOnPushOff);
    chevron.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    crate::util::set_label(&chevron, Some(title));
    chevron.setAccessibilityExpanded(false);
    chevron.setTranslatesAutoresizingMaskIntoConstraints(false);
    follow_width(&chevron, CHEVRON_WIDTH);
    // SAFETY: A nil target sends the action along the responder chain until `set_action` names the panel.
    let label =
        unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, Some(sel!(ideSearchDisclose:)), mtm) };
    set_action(&label, panel, sel!(ideSearchDisclose:));
    label.setBordered(false);
    ui_size::set_system_font(&label, SMALL_FONT);
    label.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    label.setAccessibilityElement(false);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    ui_size::follow_spacing(&row, space::XXS);
    row.addArrangedSubview(&chevron);
    row.addArrangedSubview(&label);
    (row, chevron)
}

/// Draws the chevron of `button` at the size of every chevron of the sidebar, whatever its font.
fn size_chevron(button: &NSButton) {
    ui_size::keep(button, ui_size::Aspect::Symbol, |button| {
        button.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
            ui_size::scaled(CHEVRON_POINTS),
            crate::util::constant!(objc2_app_kit::NSFontWeightSemibold),
        )));
    });
}

/// The include and exclude fields, beginning where the fields above do; hidden until the disclosure opens them.
fn glob_fields(mtm: MainThreadMarker, include: &NSTextField, exclude: &NSTextField) -> Retained<NSView> {
    let globs = NSView::new(mtm);
    for field in [include, exclude] {
        globs.addSubview(field);
        field.setTranslatesAutoresizingMaskIntoConstraints(false);
        let leading = field.leadingAnchor().constraintEqualToAnchor(&globs.leadingAnchor());
        ui_size::follow_length(&leading, FIELD_INDENT);
        activate([
            leading,
            field.trailingAnchor().constraintEqualToAnchor(&globs.trailingAnchor()),
        ]);
    }
    let gap = exclude.topAnchor().constraintEqualToAnchor(&include.bottomAnchor());
    ui_size::follow_length(&gap, space::XS);
    activate([
        include.topAnchor().constraintEqualToAnchor(&globs.topAnchor()),
        gap,
        exclude.bottomAnchor().constraintEqualToAnchor(&globs.bottomAnchor()),
    ]);
    globs.setHidden(true);
    globs
}

fn glob_field(mtm: MainThreadMarker, panel: &SearchPanel, placeholder: &str, name: &str) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setPlaceholderString(Some(&ns(placeholder)));
    crate::util::set_label(&field, Some(name));
    small_field(&field);
    // SAFETY: The field holds its delegate weakly; the window controller keeps the panel alive while the field shows.
    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(panel))) };
    field
}

fn results(mtm: MainThreadMarker, panel: &SearchPanel) -> (Retained<NSScrollView>, Retained<SearchOutlineView>) {
    let outline = SearchOutlineView::new(mtm, ide_ui_common::panel::LIST_FRAME);
    let scroll = ide_ui_common::panel::source_list_outline(mtm, &outline, "match", t("Suchergebnisse"));
    ui_size::follow_rows(&outline, ide_ui_common::style::ROW_HEIGHT);
    // SAFETY: The outline view holds its data source and delegate weakly; the window controller keeps the panel alive.
    unsafe {
        outline.setDataSource(Some(ProtocolObject::from_ref(panel)));
        outline.setDelegate(Some(ProtocolObject::from_ref(panel)));
    }
    set_action(&outline, panel, sel!(ideSearchRowClicked:));
    let menu = objc2_app_kit::NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(panel)));
    // SAFETY: The view keeps the menu; its delegate, the panel, lives as long as the window controller keeps the view.
    unsafe { outline.setMenu(Some(&menu)) };
    // SAFETY: The panel answers the double action with a method that takes the sender; `set_action` checked the target.
    unsafe { outline.setDoubleAction(Some(sel!(ideSearchRowDoubleClicked:))) };
    outline.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    (scroll, outline)
}
