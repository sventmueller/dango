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

use ide_product::REPOSITORY;

use crate::util::{constant, ns};

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The page the link at the UTF-16 index `at` of `text` leads to.
    fn link_at(text: &NSMutableAttributedString, at: usize) -> Option<String> {
        // SAFETY: A null effective range is allowed, and `at` lies inside the text.
        let value =
            unsafe { text.attribute_atIndex_effectiveRange(constant!(NSLinkAttributeName), at, std::ptr::null_mut()) }?;
        let url = value.downcast_ref::<NSURL>()?;
        url.absoluteString().map(|s| s.to_string())
    }

    #[test]
    fn the_credits_name_the_repository_and_both_licenses_each_linked_to_its_page() {
        let text = credits();
        let shown = text.string().to_string();
        assert_eq!(
            shown,
            format!("github.com/sventmueller/dango\n{}", t("Lizenz: MIT oder Apache 2.0"))
        );
        let at = |name: &str| shown[..shown.find(name).unwrap()].encode_utf16().count();
        assert_eq!(link_at(&text, 0).as_deref(), Some(REPOSITORY));
        assert_eq!(
            link_at(&text, at("MIT") + 2).as_deref(),
            Some("https://github.com/sventmueller/dango/blob/main/LICENSE-MIT")
        );
        assert_eq!(
            link_at(&text, at("Apache 2.0") + "Apache 2.0".len() - 1).as_deref(),
            Some("https://github.com/sventmueller/dango/blob/main/LICENSE-APACHE")
        );
        assert_eq!(link_at(&text, at("\n")), None);
    }
}
