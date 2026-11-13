//! Text attributes. Nothing outside this file names a key together with a value, so AppKit never gets a value of a
//! type its key does not expect.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, ClassType};
use objc2_app_kit::{
    NSAttachmentAttributeName, NSBackgroundColorAttributeName, NSBaselineOffsetAttributeName, NSColor, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSLinkAttributeName, NSParagraphStyle,
    NSParagraphStyleAttributeName, NSStrikethroughStyleAttributeName, NSTextAttachment, NSToolTipAttributeName,
    NSUnderlineStyleAttributeName,
};
use objc2_foundation::{
    NSAttributedString, NSAttributedStringKey, NSDictionary, NSMutableAttributedString, NSNumber, NSRange, NSURL,
};

use ide_ui_common::constant;
use ide_ui_common::util::ns;

pub struct Attribute {
    key: &'static NSAttributedStringKey,
    value: Retained<AnyObject>,
}

impl Attribute {
    fn new<T: ClassType + 'static>(key: &'static NSAttributedStringKey, value: &T) -> Attribute {
        Attribute {
            key,
            value: value.retain().into(),
        }
    }

    pub fn font(font: &NSFont) -> Attribute {
        Attribute::new(constant!(NSFontAttributeName), font)
    }

    pub fn foreground(color: &NSColor) -> Attribute {
        Attribute::new(constant!(NSForegroundColorAttributeName), color)
    }

    pub fn background(color: &NSColor) -> Attribute {
        Attribute::new(constant!(NSBackgroundColorAttributeName), color)
    }

    pub fn paragraph(style: &NSParagraphStyle) -> Attribute {
        Attribute::new(constant!(NSParagraphStyleAttributeName), style)
    }

    pub fn strikethrough() -> Attribute {
        Attribute::new(constant!(NSStrikethroughStyleAttributeName), &*NSNumber::new_isize(1))
    }

    pub fn underline() -> Attribute {
        Attribute::new(constant!(NSUnderlineStyleAttributeName), &*NSNumber::new_isize(1))
    }

    pub fn baseline_offset(offset: f64) -> Attribute {
        Attribute::new(constant!(NSBaselineOffsetAttributeName), &*NSNumber::new_f64(offset))
    }

    pub fn tool_tip(text: &str) -> Attribute {
        Attribute::new(constant!(NSToolTipAttributeName), &*ns(text))
    }

    pub fn link(url: &NSURL) -> Attribute {
        Attribute::new(constant!(NSLinkAttributeName), url)
    }

    pub fn attachment(attachment: &NSTextAttachment) -> Attribute {
        Attribute::new(constant!(NSAttachmentAttributeName), attachment)
    }
}

pub struct Attributes(Retained<NSDictionary<NSAttributedStringKey, AnyObject>>);

impl Attributes {
    pub fn new(attributes: &[Attribute]) -> Attributes {
        let keys: Vec<&NSAttributedStringKey> = attributes.iter().map(|a| a.key).collect();
        let values: Vec<&AnyObject> = attributes.iter().map(|a| &*a.value).collect();
        Attributes(NSDictionary::from_slices(&keys, &values))
    }
}

pub fn attributed(text: &str, attributes: &[Attribute]) -> Retained<NSAttributedString> {
    let attributes = Attributes::new(attributes);
    // SAFETY: `Attribute` gives every key a value of the type it expects.
    unsafe {
        NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &ns(text), Some(&attributes.0))
    }
}

pub fn attributed_mut(text: &str, attributes: &[Attribute]) -> Retained<NSMutableAttributedString> {
    let attributes = Attributes::new(attributes);
    // SAFETY: `Attribute` gives every key a value of the type it expects.
    unsafe {
        NSMutableAttributedString::initWithString_attributes(
            NSMutableAttributedString::alloc(),
            &ns(text),
            Some(&attributes.0),
        )
    }
}

pub fn add_attribute(text: &NSMutableAttributedString, attribute: &Attribute, range: NSRange) {
    // SAFETY: `Attribute` gives every key a value of the type it expects.
    unsafe { text.addAttribute_value_range(attribute.key, &attribute.value, range) };
}

/// The range of UTF-16 units that the bytes `start..end` of `text` cover, from `base` on.
pub fn utf16_range(text: &str, start: usize, end: usize, base: usize) -> NSRange {
    let from = text.get(..start).map_or(0, |s| s.encode_utf16().count());
    let length = text.get(start..end).map_or(0, |s| s.encode_utf16().count());
    NSRange::new(base + from, length)
}
