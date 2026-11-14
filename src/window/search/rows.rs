//! The rows of the search results: a file with its folder and match count, and a matching line with its number and
//! the matches marked. While the replace field shows, a row offers its replace button when the pointer is on it, and a
//! line shows each match struck through with its replacement after it.

use std::cell::RefCell;
use std::path::Path;

use ide_project::{LineResult, SearchOptions};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBackgroundColorAttributeName, NSButton, NSColor, NSEvent, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSLayoutConstraintOrientation, NSLineBreakMode, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSStrikethroughStyleAttributeName, NSTableCellView, NSTextAlignment, NSTextField,
    NSTrackingArea, NSTrackingAreaOptions, NSUserInterfaceLayoutDirection, NSView, NSWritingDirection,
};
use objc2_foundation::{NSMutableAttributedString, NSNumber, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize};
use splat_i18n::tf;

use super::SearchPanel;
use crate::util::{constant, guarded, label, ns};

const FONT_SIZE: f64 = 11.0;

/// What the rows need while the replace field shows: the panel their buttons talk to, the replacement and, for a
/// regular expression, the options that fill in its groups.
pub(super) struct Replacing<'a> {
    pub(super) panel: &'a SearchPanel,
    pub(super) with: &'a str,
    pub(super) options: &'a SearchOptions,
}

#[derive(Default)]
pub struct CellIvars {
    button: RefCell<Option<Retained<NSButton>>>,
    /// What the button covers while it shows.
    covered: RefCell<Option<Retained<NSView>>>,
}

define_class!(
    // SAFETY: NSTableCellView has no subclassing requirements and `ResultCell` does not implement Drop.
    /// A row of the results that shows its replace button while the pointer is on it.
    #[unsafe(super(NSTableCellView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESearchResultCell"]
    #[ivars = CellIvars]
    pub struct ResultCell;

    unsafe impl NSObjectProtocol for ResultCell {}

    impl ResultCell {
        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
            guarded((), || self.track());
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            guarded((), || self.hover(true));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            guarded((), || self.hover(false));
        }
    }
);

impl ResultCell {
    fn new(mtm: MainThreadMarker) -> Retained<ResultCell> {
        let this = ResultCell::alloc(mtm).set_ivars(CellIvars::default());
        // SAFETY: NSTableCellView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 22.0))] }
    }

    fn track(&self) {
        if self.ivars().button.borrow().is_none() || !self.trackingAreas().is_empty() {
            return;
        }
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY: The cell owns the area and outlives it; AppKit holds the owner weakly.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
    }

    fn hover(&self, on: bool) {
        if let Some(button) = self.ivars().button.borrow().as_ref() {
            button.setHidden(!on);
        }
        if let Some(covered) = self.ivars().covered.borrow().as_ref() {
            covered.setHidden(on);
        }
    }

    /// Puts a replace button labelled `name` at the trailing edge, over `covered`, for the pointer to bring out.
    fn offer(&self, replacing: &Replacing, name: &str, covered: Option<&NSView>) {
        let mtm = self.mtm();
        let action: Sel = objc2::sel!(ideSearchReplaceRow:);
        let button = crate::util::symbol_button(
            mtm,
            "arrow.2.squarepath",
            name,
            ide_ui_common::style::symbol::INLINE,
            Some(replacing.panel),
            action,
        );
        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        button.setHidden(true);
        self.addSubview(&button);
        button.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            button
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&self.trailingAnchor(), -4.0),
            button.centerYAnchor().constraintEqualToAnchor(&self.centerYAnchor()),
        ] {
            constraint.setActive(true);
        }
        *self.ivars().button.borrow_mut() = Some(button);
        *self.ivars().covered.borrow_mut() = covered.map(objc2::Message::retain);
    }
}

/// The replace button of a row the self-test asks for.
#[cfg(feature = "selftest")]
pub(super) fn button_of(view: &NSView) -> Option<Retained<NSButton>> {
    view.downcast_ref::<ResultCell>()?.ivars().button.borrow().clone()
}

/// A file row: its icon, name, folder and how many matches it has. VoiceOver reads the name with the count and folder.
pub(super) fn file_row(
    mtm: MainThreadMarker,
    path: &Path,
    relative: &str,
    matches: usize,
    replacing: Option<&Replacing>,
) -> Retained<NSView> {
    let cell = ResultCell::new(mtm);
    let image = crate::file_icon::image_view(mtm, path, false);
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let folder = Path::new(relative)
        .parent()
        .map_or_else(String::new, |p| p.to_string_lossy().into_owned());
    let title = label(mtm, &name);
    // SAFETY: AppKit sets its font weight constants when it loads.
    title.setFont(Some(&NSFont::systemFontOfSize_weight(
        ide_ui_common::style::LABEL,
        unsafe { objc2_app_kit::NSFontWeightSemibold },
    )));
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    let place = label(mtm, &folder);
    place.setFont(Some(&NSFont::systemFontOfSize(FONT_SIZE)));
    place.setTextColor(Some(&NSColor::secondaryLabelColor()));
    place.setLineBreakMode(NSLineBreakMode::ByTruncatingHead);
    let number = label(mtm, &matches.to_string());
    // SAFETY: AppKit sets its font weight constants when it loads.
    number.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
        FONT_SIZE,
        unsafe { objc2_app_kit::NSFontWeightRegular },
    )));
    number.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let spoken = match (matches, folder.is_empty()) {
        (1, true) => tf("{}, 1 Treffer", &[&name]),
        (1, false) => tf("{}, 1 Treffer, in {}", &[&name, &folder]),
        (_, true) => tf("{}, {} Treffer", &[&name, &matches.to_string()]),
        (_, false) => tf("{}, {} Treffer, in {}", &[&name, &matches.to_string(), &folder]),
    };
    crate::util::set_label(&title, Some(&spoken));
    crate::util::hide_from_voiceover(&place);
    crate::util::hide_from_voiceover(&number);
    for view in [&*image as &NSView, &title, &place, &number] {
        cell.addSubview(view);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    // SAFETY: The views are subviews of the cell, which keeps them.
    unsafe {
        cell.setImageView(Some(&image));
        cell.setTextField(Some(&title));
    }
    place.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    title.setContentCompressionResistancePriority_forOrientation(700.0, NSLayoutConstraintOrientation::Horizontal);
    for constraint in [
        image.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        image.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        image.widthAnchor().constraintEqualToConstant(16.0),
        image.heightAnchor().constraintEqualToConstant(16.0),
        title
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&image.trailingAnchor(), 5.0),
        title.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        place
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&title.trailingAnchor(), 6.0),
        place
            .firstBaselineAnchor()
            .constraintEqualToAnchor(&title.firstBaselineAnchor()),
        number
            .leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&place.trailingAnchor(), 6.0),
        number
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -6.0),
        number
            .firstBaselineAnchor()
            .constraintEqualToAnchor(&title.firstBaselineAnchor()),
    ] {
        constraint.setActive(true);
    }
    cell.setToolTip(Some(&ns(relative)));
    if let Some(replacing) = replacing {
        let covered: &NSView = &number;
        cell.offer(replacing, splat_i18n::t("Alle in dieser Datei ersetzen"), Some(covered));
    }
    cell.into_super().into_super()
}

/// A matching line: its number and the line with the matches marked, left to right in every interface language, as
/// code is. The numbers of a file are as wide as `widest`, its largest. VoiceOver reads "line n:" with the text.
pub(super) fn line_row(
    mtm: MainThreadMarker,
    line: &LineResult,
    widest: u64,
    replacing: Option<&Replacing>,
) -> Retained<NSView> {
    let cell = ResultCell::new(mtm);
    cell.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    // SAFETY: AppKit sets its font weight constants when it loads.
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(FONT_SIZE, unsafe { objc2_app_kit::NSFontWeightRegular });
    let sample = label(mtm, &"0".repeat(widest.max(line.line).to_string().len()));
    sample.setFont(Some(&font));
    let number_width = sample.intrinsicContentSize().width;
    let number = label(mtm, &line.line.to_string());
    number.setFont(Some(&font));
    number.setTextColor(Some(&NSColor::secondaryLabelColor()));
    number.setAlignment(NSTextAlignment::Right);
    crate::util::hide_from_voiceover(&number);
    let text = NSTextField::labelWithAttributedString(&marked(line, replacing), mtm);
    text.setBaseWritingDirection(NSWritingDirection::LeftToRight);
    text.setAlignment(NSTextAlignment::Left);
    text.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    text.setAccessibilityLabel(Some(&ns(&tf(
        "Zeile {}: {}",
        &[&line.line.to_string(), &line.preview.text],
    ))));
    // Not the cell's text field, which a source list restyles, dropping the code font and the marks.
    for view in [&*number as &NSView, &text] {
        cell.addSubview(view);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    text.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    for constraint in [
        number.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        number.widthAnchor().constraintEqualToConstant(number_width),
        number.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        text.leadingAnchor()
            .constraintEqualToAnchor_constant(&number.trailingAnchor(), 6.0),
        text.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        text.firstBaselineAnchor()
            .constraintEqualToAnchor(&number.firstBaselineAnchor()),
    ] {
        constraint.setActive(true);
    }
    if let Some(replacing) = replacing {
        cell.offer(replacing, splat_i18n::t("Ersetzen"), None);
    }
    cell.into_super().into_super()
}

/// What each match of `line` is replaced with, where the preview shows all of them; else none.
fn replacements_of(line: &LineResult, replacing: &Replacing) -> Vec<String> {
    if !replacing.options.regex {
        return vec![replacing.with.to_string(); line.columns.len()];
    }
    let found = ide_project::replacements(&line.preview.text, replacing.options, replacing.with).unwrap_or_default();
    if found.len() == line.columns.len() {
        found.into_iter().map(|r| r.text).collect()
    } else {
        Vec::new()
    }
}

/// The shown part of the line in the code font, with an ellipsis where it is cut and the matches marked as find
/// results are; while replacing, each match struck through with its replacement after it.
fn marked(line: &LineResult, replacing: Option<&Replacing>) -> Retained<NSMutableAttributedString> {
    let preview = &line.preview;
    let lead = if preview.cut_before { "…" } else { "" };
    let tail = if preview.cut_after { "…" } else { "" };
    let shown = format!("{lead}{}{tail}", preview.text);
    let text = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns(&shown));
    let whole = NSRange::new(0, shown.encode_utf16().count());
    let style = NSMutableParagraphStyle::new();
    style.setBaseWritingDirection(NSWritingDirection::LeftToRight);
    style.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    // SAFETY: AppKit sets its font weight constants when it loads.
    let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, unsafe { objc2_app_kit::NSFontWeightRegular });
    let (font_key, style_key) = (constant!(NSFontAttributeName), constant!(NSParagraphStyleAttributeName));
    let (color_key, marker_key) = (
        constant!(NSForegroundColorAttributeName),
        constant!(NSBackgroundColorAttributeName),
    );
    // SAFETY: Each value has the type its AppKit attribute key expects: a font, a paragraph style, a colour.
    unsafe {
        text.addAttribute_value_range(font_key, &font, whole);
        text.addAttribute_value_range(style_key, &style, whole);
        text.addAttribute_value_range(color_key, &NSColor::labelColor(), whole);
    }
    let offset = lead.encode_utf16().count();
    let length = preview.text.encode_utf16().count();
    let with = replacing.map(|r| replacements_of(line, r)).unwrap_or_default();
    // From the end, so the replacements put in leave the positions before them as they are.
    for (n, column) in line.columns.iter().enumerate().rev() {
        let start = column.start.saturating_sub(preview.start).min(length);
        let end = column.end.saturating_sub(preview.start).min(length);
        if end <= start {
            continue;
        }
        let range = NSRange::new(offset + start, end - start);
        if replacing.is_some() {
            strike(&text, range, with.get(n).map(String::as_str), &font, &style);
            continue;
        }
        // SAFETY: Both values are colours, as the background and foreground colour keys expect.
        unsafe {
            text.addAttribute_value_range(marker_key, &NSColor::findHighlightColor(), range);
            text.addAttribute_value_range(color_key, &NSColor::blackColor(), range);
        }
    }
    text
}

/// Strikes the match at `range` through in red and puts `with` after it in green, as a diff shows a change.
fn strike(
    text: &NSMutableAttributedString,
    range: NSRange,
    with: Option<&str>,
    font: &NSFont,
    style: &NSMutableParagraphStyle,
) {
    let (color_key, marker_key) = (
        constant!(NSForegroundColorAttributeName),
        constant!(NSBackgroundColorAttributeName),
    );
    let (strike_key, font_key, style_key) = (
        constant!(NSStrikethroughStyleAttributeName),
        constant!(NSFontAttributeName),
        constant!(NSParagraphStyleAttributeName),
    );
    let removed = NSColor::systemRedColor().colorWithAlphaComponent(0.25);
    // SAFETY: The values have the types their AppKit keys expect: colours and a number for the strike style.
    unsafe {
        text.addAttribute_value_range(marker_key, &removed, range);
        text.addAttribute_value_range(color_key, &NSColor::labelColor(), range);
        text.addAttribute_value_range(strike_key, &NSNumber::new_i32(1), range);
    }
    let Some(with) = with.filter(|w| !w.is_empty()) else {
        return;
    };
    let added = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns(with));
    let whole = NSRange::new(0, with.encode_utf16().count());
    // SAFETY: Each value has the type its AppKit attribute key expects: a font, a paragraph style, colours.
    unsafe {
        added.addAttribute_value_range(font_key, font, whole);
        added.addAttribute_value_range(style_key, style, whole);
        added.addAttribute_value_range(color_key, &NSColor::labelColor(), whole);
        added.addAttribute_value_range(
            marker_key,
            &NSColor::systemGreenColor().colorWithAlphaComponent(0.3),
            whole,
        );
    }
    text.insertAttributedString_atIndex(&added, range.location + range.length);
}

/// What VoiceOver reads for a row made here: the one text of the row that has a label of its own.
#[cfg(feature = "selftest")]
pub(super) fn spoken(view: &NSView) -> Option<String> {
    view.subviews().iter().find_map(|subview| {
        let field = subview.downcast_ref::<NSTextField>()?;
        field.accessibilityLabel().map(|label| label.to_string())
    })
}
