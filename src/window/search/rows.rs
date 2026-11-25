//! The rows of the search results: a file with its folder and match count, and a matching line with its number and
//! the matches marked. While the replace field shows, a row offers its replace button when the pointer is on it, and a
//! line shows each match struck through with its replacement after it. A row takes its sizes at the interface size as
//! it is made, and the lists make their rows anew when that size changes.

use std::cell::RefCell;
use std::path::Path;

use ide_project::LineResult;
use ide_project::replace::Replacer;
use ide_ui_common::ui_size::scaled;
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
use crate::util::{activate, constant, fixed_size, guarded, label, ns};

const FONT_SIZE: f64 = 11.0;
/// The side of a file's icon.
const ICON_SIDE: f64 = 16.0;

/// What the replace field holds while it shows, read once for every row the list shows next.
pub(super) struct Replace {
    pub(super) with: String,
    pub(super) regex: bool,
    /// The matcher that fills in the groups of a regular expression; `None` for a literal pattern or an invalid one.
    pub(super) groups: Option<Replacer>,
}

/// What the rows need while the replace field shows: the panel their buttons talk to and the replacement.
pub(super) struct Replacing<'a> {
    pub(super) panel: &'a SearchPanel,
    pub(super) replace: &'a Replace,
}

#[derive(Default)]
pub(crate) struct CellIvars {
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
    pub(crate) struct ResultCell;

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
            scaled(ide_ui_common::style::symbol::INLINE),
            Some(replacing.panel),
            action,
        );
        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        button.setHidden(true);
        self.addSubview(&button);
        button.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            button
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&self.trailingAnchor(), -4.0),
            button.centerYAnchor().constraintEqualToAnchor(&self.centerYAnchor()),
        ]);
        *self.ivars().button.borrow_mut() = Some(button);
        *self.ivars().covered.borrow_mut() = covered.map(objc2::Message::retain);
    }
}

/// Hides the replace button of a row that comes back into the list, where the pointer may have left it.
pub(super) fn rest(view: &NSView) {
    if let Some(cell) = view.downcast_ref::<ResultCell>() {
        cell.hover(false);
    }
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
    let image = crate::file_icon::image_view(mtm, Path::new(""), true);
    crate::file_icon::show_scaled(&image, path, false, scaled(1.0));
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let folder = Path::new(relative)
        .parent()
        .map_or_else(String::new, |p| p.to_string_lossy().into_owned());
    let title = label(mtm, &name);
    title.setFont(Some(&NSFont::systemFontOfSize_weight(
        scaled(ide_ui_common::style::LABEL),
        constant!(objc2_app_kit::NSFontWeightSemibold),
    )));
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    let place = label(mtm, &folder);
    place.setFont(Some(&NSFont::systemFontOfSize(scaled(FONT_SIZE))));
    place.setTextColor(Some(&NSColor::secondaryLabelColor()));
    place.setLineBreakMode(NSLineBreakMode::ByTruncatingHead);
    let number = label(mtm, &matches.to_string());
    number.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
        scaled(FONT_SIZE),
        constant!(objc2_app_kit::NSFontWeightRegular),
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
    fixed_size(&image, scaled(ICON_SIDE), scaled(ICON_SIDE));
    activate([
        image.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        image.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        title
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&image.trailingAnchor(), scaled(5.0)),
        title.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        place
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&title.trailingAnchor(), scaled(6.0)),
        place
            .firstBaselineAnchor()
            .constraintEqualToAnchor(&title.firstBaselineAnchor()),
        number
            .leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&place.trailingAnchor(), scaled(6.0)),
        number
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -6.0),
        number
            .firstBaselineAnchor()
            .constraintEqualToAnchor(&title.firstBaselineAnchor()),
    ]);
    cell.setToolTip(Some(&ns(relative)));
    if let Some(replacing) = replacing {
        let covered: &NSView = &number;
        cell.offer(replacing, splat_i18n::t("Alle in dieser Datei ersetzen"), Some(covered));
    }
    cell.into_super().into_super()
}

fn number_font() -> Retained<NSFont> {
    NSFont::monospacedDigitSystemFontOfSize_weight(scaled(FONT_SIZE), constant!(objc2_app_kit::NSFontWeightRegular))
}

/// How wide the number of a line row is for numbers of `digits` digits at the interface size as it is now.
pub(super) fn number_width(mtm: MainThreadMarker, digits: usize) -> f64 {
    let sample = label(mtm, &"0".repeat(digits));
    sample.setFont(Some(&number_font()));
    sample.intrinsicContentSize().width
}

/// A matching line: its number, `number_width` wide, and the line with the matches marked, left to right in every
/// interface language, as code is. VoiceOver reads "line n:" with the text.
pub(super) fn line_row(
    mtm: MainThreadMarker,
    line: &LineResult,
    number_width: f64,
    replacing: Option<&Replacing>,
) -> Retained<NSView> {
    let cell = ResultCell::new(mtm);
    cell.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    let font = number_font();
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
    activate([
        number.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        number.widthAnchor().constraintEqualToConstant(number_width),
        number.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        text.leadingAnchor()
            .constraintEqualToAnchor_constant(&number.trailingAnchor(), scaled(6.0)),
        text.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        text.firstBaselineAnchor()
            .constraintEqualToAnchor(&number.firstBaselineAnchor()),
    ]);
    if let Some(replacing) = replacing {
        cell.offer(replacing, splat_i18n::t("Ersetzen"), None);
    }
    cell.into_super().into_super()
}

/// What each match of `line` is replaced with, where the preview shows all of them; else none.
fn replacements_of(line: &LineResult, replacing: &Replacing) -> Vec<String> {
    let replace = replacing.replace;
    if !replace.regex {
        return vec![replace.with.clone(); line.columns.len()];
    }
    let found = replace
        .groups
        .as_ref()
        .and_then(|groups| groups.replacements(&line.preview.text, &replace.with).ok())
        .unwrap_or_default();
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
    let font =
        NSFont::monospacedSystemFontOfSize_weight(scaled(FONT_SIZE), constant!(objc2_app_kit::NSFontWeightRegular));
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

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
