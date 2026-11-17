//! The standard macOS About panel, with the project's address and linked license names, as in Splat.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSAboutPanelOptionCredits, NSAboutPanelOptionVersion, NSApplication, NSFont, NSFontAttributeName,
    NSLinkAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSTextAlignment,
};
use objc2_foundation::{NSDictionary, NSMutableAttributedString, NSRange, NSURL};
use splat_i18n::t;

use crate::util::{constant, ns};

const REPOSITORY: &str = "https://github.com/sventmueller/dango";

pub(crate) fn show(mtm: MainThreadMarker) {
    let keys = [
        constant!(NSAboutPanelOptionVersion),
        constant!(NSAboutPanelOptionCredits),
    ];
    // CFBundleShortVersionString already includes the build number; do not repeat it in parentheses.
    let values: [Retained<AnyObject>; 2] = [ns("").into(), credits().into()];
    let values: Vec<&AnyObject> = values.iter().map(|value| &**value).collect();
    let options = NSDictionary::from_slices(&keys, &values);
    // SAFETY: the options supply a string for the extra version and an attributed string for the credits.
    unsafe { NSApplication::sharedApplication(mtm).orderFrontStandardAboutPanelWithOptions(&options) };
}

/// The repository and license in the same small, centered type as Splat, each name linked to its page.
fn credits() -> Retained<NSMutableAttributedString> {
    let address = REPOSITORY.trim_start_matches("https://");
    let shown = format!("{address}\n{}", t("Lizenz: MIT oder Apache 2.0"));
    let paragraph = NSMutableParagraphStyle::new();
    paragraph.setAlignment(NSTextAlignment::Center);
    let attributes = NSDictionary::<_, AnyObject>::from_retained_objects(
        &[constant!(NSFontAttributeName), constant!(NSParagraphStyleAttributeName)],
        &[
            NSFont::systemFontOfSize(NSFont::smallSystemFontSize()).into(),
            paragraph.into(),
        ],
    );
    // SAFETY: the attributes map the font and paragraph-style keys to the matching AppKit objects.
    let text = unsafe {
        NSMutableAttributedString::initWithString_attributes(
            NSMutableAttributedString::alloc(),
            &ns(&shown),
            Some(&attributes),
        )
    };
    for (name, target) in [
        (address, REPOSITORY.to_string()),
        ("MIT", format!("{REPOSITORY}/blob/main/LICENSE-MIT")),
        ("Apache 2.0", format!("{REPOSITORY}/blob/main/LICENSE-APACHE")),
    ] {
        let (Some(start), Some(url)) = (shown.find(name), NSURL::URLWithString(&ns(&target))) else {
            continue;
        };
        let range = NSRange::new(shown[..start].encode_utf16().count(), name.encode_utf16().count());
        // SAFETY: the range covers the link's label in UTF-16, and the link attribute takes an NSURL.
        unsafe { text.addAttribute_value_range(constant!(NSLinkAttributeName), &url, range) };
    }
    text
}
