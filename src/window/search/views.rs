//! The views of the search panel: the option toggles for the panel's title row, the field, the replace field behind a
//! disclosure, the file filters, the status line and the list of matches.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSButton, NSButtonType, NSCellImagePosition, NSColor, NSControlSize,
    NSControlStateValueOff, NSControlStateValueOn, NSFont, NSLayoutAttribute, NSLayoutConstraintOrientation,
    NSScrollView, NSStackView, NSTableColumn, NSTableColumnResizingOptions, NSTableViewColumnAutoresizingStyle,
    NSTableViewStyle, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::{SearchOutlineView, SearchPanel, Toggle};
use crate::util::{ns, pin_edges, set_action, symbol, symbol_button};

const MARGIN: f64 = 8.0;
const SMALL_FONT: f64 = 11.0;
/// The width of the replace chevron, which the replace field keeps free below it.
const CHEVRON_WIDTH: f64 = 12.0;
/// The size of both chevrons, the same as the folder tree's.
const CHEVRON_POINTS: f64 = 10.0;
/// Where every field begins: after a chevron and the stack's spacing.
const FIELD_INDENT: f64 = CHEVRON_WIDTH + 2.0;
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
        field_row.setSpacing(2.0);
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
        status.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
        status.setTextColor(Some(&NSColor::secondaryLabelColor()));
        status.setHidden(true);
        let bar = NSStackView::new(mtm);
        bar.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        bar.setAlignment(NSLayoutAttribute::Leading);
        bar.setSpacing(6.0);
        bar.setEdgeInsets(NSEdgeInsets {
            top: 0.0,
            left: MARGIN,
            bottom: 4.0,
            right: MARGIN,
        });
        for view in [&*field_row as &NSView, &replace_row, &disclosure_row, &globs, &status] {
            bar.addArrangedSubview(view);
        }
        for (view, inset) in [
            (&*field_row as &NSView, MARGIN + 2.0 + ALL_WIDTH),
            (&replace_row, MARGIN),
            (&globs, MARGIN + 2.0 + ALL_WIDTH),
            (&status, MARGIN),
        ] {
            view.trailingAnchor()
                .constraintEqualToAnchor_constant(&bar.trailingAnchor(), -inset)
                .setActive(true);
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

    #[cfg(feature = "selftest")]
    pub(super) fn set_toggled(&self, toggle: Toggle, on: bool) {
        if let Some((_, button)) = self.toggles.iter().find(|(t, _)| *t == toggle) {
            button.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        self.paint_toggles();
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

    #[cfg(feature = "selftest")]
    pub(super) fn results_shown(&self) -> bool {
        !self.results.isHidden()
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

fn layout(page: &NSView, bar: &NSStackView, body: &NSView) {
    for view in [bar as &NSView, body] {
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
        view.leadingAnchor()
            .constraintEqualToAnchor(&page.leadingAnchor())
            .setActive(true);
        view.trailingAnchor()
            .constraintEqualToAnchor(&page.trailingAnchor())
            .setActive(true);
    }
    for constraint in [
        bar.topAnchor().constraintEqualToAnchor(&page.topAnchor()),
        body.topAnchor()
            .constraintEqualToAnchor_constant(&bar.bottomAnchor(), 2.0),
        body.bottomAnchor().constraintEqualToAnchor(&page.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
}

/// A plain field like the replace field below it; typing, Return and Esc reach the panel through the delegate.
fn search_field(mtm: MainThreadMarker, panel: &SearchPanel) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    let title = t("Im Projekt suchen");
    field.setPlaceholderString(Some(&ns(title)));
    crate::util::set_label(&field, Some(title));
    field.setControlSize(NSControlSize::Small);
    field.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
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
    button
        .widthAnchor()
        .constraintEqualToConstant(CHEVRON_WIDTH)
        .setActive(true);
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
    field.setControlSize(NSControlSize::Small);
    field.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    // SAFETY: The field holds its delegate weakly; the window controller keeps the panel alive while the field shows.
    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(panel))) };
    field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    let all = symbol_button(
        mtm,
        "arrow.2.squarepath",
        t("Alle ersetzen"),
        12.0,
        Some(panel),
        sel!(ideSearchReplaceAll:),
    );
    all.setEnabled(false);
    all.widthAnchor().constraintEqualToConstant(ALL_WIDTH).setActive(true);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(2.0);
    // As wide as the chevron above, so the two fields line up.
    let indent = NSView::new(mtm);
    indent
        .widthAnchor()
        .constraintEqualToConstant(CHEVRON_WIDTH)
        .setActive(true);
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
    chevron.setImagePosition(NSCellImagePosition::ImageOnly);
    chevron.setButtonType(NSButtonType::PushOnPushOff);
    chevron.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    crate::util::set_label(&chevron, Some(title));
    chevron.setAccessibilityExpanded(false);
    chevron.setTranslatesAutoresizingMaskIntoConstraints(false);
    chevron
        .widthAnchor()
        .constraintEqualToConstant(CHEVRON_WIDTH)
        .setActive(true);
    // SAFETY: A nil target sends the action along the responder chain until `set_action` names the panel.
    let label =
        unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, Some(sel!(ideSearchDisclose:)), mtm) };
    set_action(&label, panel, sel!(ideSearchDisclose:));
    label.setBordered(false);
    label.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    label.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    label.setAccessibilityElement(false);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(2.0);
    row.addArrangedSubview(&chevron);
    row.addArrangedSubview(&label);
    (row, chevron)
}

/// Draws the chevron of `button` at the size of every chevron of the sidebar, whatever its font.
fn size_chevron(button: &NSButton) {
    button.setSymbolConfiguration(Some(
        &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
            CHEVRON_POINTS,
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightSemibold },
        ),
    ));
}

/// The include and exclude fields, beginning where the fields above do; hidden until the disclosure opens them.
fn glob_fields(mtm: MainThreadMarker, include: &NSTextField, exclude: &NSTextField) -> Retained<NSView> {
    let globs = NSView::new(mtm);
    for field in [include, exclude] {
        globs.addSubview(field);
        field.setTranslatesAutoresizingMaskIntoConstraints(false);
        field
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&globs.leadingAnchor(), FIELD_INDENT)
            .setActive(true);
        field
            .trailingAnchor()
            .constraintEqualToAnchor(&globs.trailingAnchor())
            .setActive(true);
    }
    for constraint in [
        include.topAnchor().constraintEqualToAnchor(&globs.topAnchor()),
        exclude
            .topAnchor()
            .constraintEqualToAnchor_constant(&include.bottomAnchor(), 4.0),
        exclude.bottomAnchor().constraintEqualToAnchor(&globs.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    globs.setHidden(true);
    globs
}

fn glob_field(mtm: MainThreadMarker, panel: &SearchPanel, placeholder: &str, name: &str) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setPlaceholderString(Some(&ns(placeholder)));
    crate::util::set_label(&field, Some(name));
    field.setControlSize(NSControlSize::Small);
    field.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    // SAFETY: The field holds its delegate weakly; the window controller keeps the panel alive while the field shows.
    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(panel))) };
    field
}

fn results(mtm: MainThreadMarker, panel: &SearchPanel) -> (Retained<NSScrollView>, Retained<SearchOutlineView>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 400.0));
    let outline = SearchOutlineView::new(mtm, frame);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("match"));
    column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
    outline.addTableColumn(&column);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { outline.setOutlineTableColumn(Some(&column)) };
    outline.setHeaderView(None);
    outline.setStyle(NSTableViewStyle::SourceList);
    outline.setIndentationPerLevel(12.0);
    outline.setRowHeight(22.0);
    outline.setAutoresizesOutlineColumn(false);
    outline.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
    outline.setAccessibilityLabel(Some(&ns(t("Suchergebnisse"))));
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
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(&outline));
    outline.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    (scroll, outline)
}
