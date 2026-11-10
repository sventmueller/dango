//! Reading an element's accessibility properties in-process. A view describes itself through the NSAccessibility
//! methods (`accessibilityRole`, the values `setAccessibilityLabel:` stores), AppKit's cells and system parts through
//! the attributes of the older protocol; asked through the older `accessibilityAttributeValue:`, a view that only set
//! its properties answers `AXUnknown` and no name. So each property is read from the method first and from the
//! attribute where the method says nothing, as AppKit's bridge to the accessibility server does.

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Method, Sel};
use objc2::{ClassType, msg_send, sel};
use objc2_app_kit::{
    NSAccessibilityChildrenAttribute, NSAccessibilityDescriptionAttribute, NSAccessibilityEnabledAttribute,
    NSAccessibilityFocusedAttribute, NSAccessibilityPlaceholderValueAttribute, NSAccessibilityRoleAttribute,
    NSAccessibilitySubroleAttribute, NSAccessibilityTitleAttribute, NSAccessibilityTitleUIElementAttribute,
    NSAccessibilityValueAttribute, NSView,
};
use objc2_foundation::{NSArray, NSAttributedString, NSNumber, NSString};

use crate::util::constant;

/// A property VoiceOver asks an element for.
#[derive(Clone, Copy)]
pub enum Property {
    Role,
    Subrole,
    /// What `setAccessibilityLabel:` sets, the attribute `AXDescription`.
    Label,
    Title,
    TitleElement,
    Value,
    Placeholder,
    Children,
}

impl Property {
    fn getter(self) -> Sel {
        match self {
            Property::Role => sel!(accessibilityRole),
            Property::Subrole => sel!(accessibilitySubrole),
            Property::Label => sel!(accessibilityLabel),
            Property::Title => sel!(accessibilityTitle),
            Property::TitleElement => sel!(accessibilityTitleUIElement),
            Property::Value => sel!(accessibilityValue),
            Property::Placeholder => sel!(accessibilityPlaceholderValue),
            Property::Children => sel!(accessibilityChildren),
        }
    }

    fn attribute(self) -> &'static NSString {
        match self {
            Property::Role => constant!(NSAccessibilityRoleAttribute),
            Property::Subrole => constant!(NSAccessibilitySubroleAttribute),
            Property::Label => constant!(NSAccessibilityDescriptionAttribute),
            Property::Title => constant!(NSAccessibilityTitleAttribute),
            Property::TitleElement => constant!(NSAccessibilityTitleUIElementAttribute),
            Property::Value => constant!(NSAccessibilityValueAttribute),
            Property::Placeholder => constant!(NSAccessibilityPlaceholderValueAttribute),
            Property::Children => constant!(NSAccessibilityChildrenAttribute),
        }
    }
}

/// The property of `object`, from its NSAccessibility method where that says something, else from the attribute.
pub fn property(object: &AnyObject, property: Property) -> Option<Retained<AnyObject>> {
    // The older attribute lists the children with the ignored views in between left out, as VoiceOver gets them.
    if matches!(property, Property::Children) {
        return legacy(object, property).or_else(|| modern(object, property));
    }
    let modern = modern(object, property);
    if modern.as_ref().is_some_and(|value| says_something(value)) {
        return modern;
    }
    legacy(object, property).or(modern)
}

fn modern(object: &AnyObject, property: Property) -> Option<Retained<AnyObject>> {
    if !object.class().responds_to(property.getter()) {
        return None;
    }
    // SAFETY: Each of the getters takes no argument and returns an object or nil; the object answers it.
    unsafe { msg_send![object, performSelector: property.getter()] }
}

fn legacy(object: &AnyObject, property: Property) -> Option<Retained<AnyObject>> {
    // SAFETY: AppKit's accessibility category of NSObject answers `accessibilityAttributeValue:` for any attribute
    // name, with an object or nil.
    unsafe { msg_send![object, accessibilityAttributeValue: property.attribute()] }
}

fn says_something(value: &AnyObject) -> bool {
    if let Some(array) = value.downcast_ref::<NSArray>() {
        return array.count() > 0;
    }
    if value.downcast_ref::<NSString>().is_none() && value.downcast_ref::<NSAttributedString>().is_none() {
        return true;
    }
    let said = text(value);
    !said.trim().is_empty() && said != "AXUnknown"
}

/// A property's text: a string as it is, a number as its digits, anything else as nothing.
pub fn text(value: &AnyObject) -> String {
    if let Some(string) = value.downcast_ref::<NSString>() {
        string.to_string()
    } else if let Some(string) = value.downcast_ref::<NSAttributedString>() {
        string.string().to_string()
    } else if let Some(number) = value.downcast_ref::<NSNumber>() {
        number.stringValue().to_string()
    } else {
        String::new()
    }
}

/// The property's text, or nothing if the element has none.
pub fn text_of(object: &AnyObject, which: Property) -> String {
    property(object, which).map(|value| text(&value)).unwrap_or_default()
}

/// The children of `object` VoiceOver walks to.
pub fn children(object: &AnyObject) -> Vec<Retained<AnyObject>> {
    property(object, Property::Children)
        .and_then(|value| value.downcast::<NSArray>().ok())
        .map(|array| array.iter().collect())
        .unwrap_or_default()
}

/// Whether VoiceOver can press `object`: a cell lists the press action, a view implements the method itself instead
/// of inheriting NSView's.
pub fn can_press(object: &AnyObject) -> bool {
    let press = sel!(accessibilityPerformPress);
    let base: &AnyClass = NSView::class();
    let own = object.class().instance_method(press).map(std::ptr::from_ref::<Method>);
    if own.is_some() && own != base.instance_method(press).map(std::ptr::from_ref::<Method>) {
        return true;
    }
    // SAFETY: The accessibility category answers with the element's action names, or nil.
    let names: Option<Retained<NSArray<NSString>>> = unsafe { msg_send![object, accessibilityActionNames] };
    names.is_some_and(|names| names.iter().any(|name| name.to_string() == "AXPress"))
}

/// Whether the element can be used now; VoiceOver reads a disabled control, but nobody operates it.
pub fn enabled(object: &AnyObject) -> bool {
    let modern = sel!(isAccessibilityEnabled);
    // SAFETY: `isAccessibilityEnabled` takes no argument and returns a BOOL.
    let off_modern = object.class().responds_to(modern) && !unsafe { msg_send![object, isAccessibilityEnabled] };
    let attribute: &NSString = constant!(NSAccessibilityEnabledAttribute);
    // SAFETY: As in `legacy`.
    let value: Option<Retained<AnyObject>> = unsafe { msg_send![object, accessibilityAttributeValue: attribute] };
    let off_legacy = value
        .and_then(|value| value.downcast::<NSNumber>().ok())
        .is_some_and(|value| !value.boolValue());
    !off_modern && !off_legacy
}

/// Whether the keyboard focus can go to `object`, which VoiceOver moves there for typing.
pub fn focusable(object: &AnyObject) -> bool {
    if let Some(view) = object.downcast_ref::<NSView>() {
        return view.acceptsFirstResponder();
    }
    let attribute: &NSString = constant!(NSAccessibilityFocusedAttribute);
    // SAFETY: The accessibility category answers for any attribute name whether a client may set it.
    unsafe { msg_send![object, accessibilityIsAttributeSettable: attribute] }
}
