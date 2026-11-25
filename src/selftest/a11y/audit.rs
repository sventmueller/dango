//! What VoiceOver gets from a window, read through the same accessibility attributes the accessibility server asks
//! for, and what it would miss there: a control without a name, an image without a description, a name that says
//! nothing or repeats beside it, a role that does not fit, a control it cannot press. Beside it, the view tree of a
//! right-to-left run: code that turns with the interface.

use std::collections::BTreeMap;

use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSImage, NSTextAlignment, NSTextField, NSTextView, NSUserInterfaceLayoutDirection, NSView, NSWindow,
    NSWindowStyleMask, NSWritingDirection,
};

use super::read::{self, Property};
use crate::util::ns;

/// How many elements a window may have before the walk stops, far more than any state of the app shows.
const LIMIT: usize = 6000;

/// Roles a person operates, which need a name.
const CONTROLS: &[&str] = &[
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXPopUpButton",
    "AXMenuButton",
    "AXTextField",
    "AXTextArea",
    "AXComboBox",
    "AXSlider",
    "AXIncrementor",
    "AXLink",
    "AXDisclosureTriangle",
    "AXColorWell",
];

/// Roles that group what a person moves through, which VoiceOver announces by their name.
const NAMED_GROUPS: &[&str] = &[
    "AXOutline",
    "AXTable",
    "AXList",
    "AXTabGroup",
    "AXRadioGroup",
    "AXBrowser",
];

/// Roles VoiceOver activates with its press action.
const PRESSABLE: &[&str] = &[
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXPopUpButton",
    "AXMenuButton",
    "AXLink",
    "AXDisclosureTriangle",
];

const TEXT_INPUTS: &[&str] = &["AXTextField", "AXTextArea", "AXComboBox"];

/// Subroles of parts AppKit draws, which VoiceOver announces by the subrole, as "close button" or "increment arrow".
const SYSTEM_PARTS: &[&str] = &[
    "AXCloseButton",
    "AXMinimizeButton",
    "AXZoomButton",
    "AXFullScreenButton",
    "AXIncrementArrow",
    "AXDecrementArrow",
    "AXIncrementPage",
    "AXDecrementPage",
];

/// Names that only repeat what VoiceOver says about the role anyway.
const GENERIC: &[&str] = &[
    "button",
    "taste",
    "schaltfläche",
    "image",
    "bild",
    "icon",
    "symbol",
    "view",
    "label",
    "text",
    "item",
    "group",
    "gruppe",
    "untitled",
];

/// One element as VoiceOver gets it.
pub(crate) struct Element {
    pub(crate) object: Retained<AnyObject>,
    /// The roles and names from the window down to the element.
    pub(crate) path: String,
    pub(crate) role: String,
    pub(crate) subrole: String,
    pub(crate) name: String,
    pub(crate) class: String,
    pub(crate) parent_role: String,
}

impl Element {
    /// The element as a finding names it.
    pub(crate) fn what(&self) -> String {
        let role = if self.subrole.is_empty() {
            self.role.clone()
        } else {
            format!("{}/{}", self.role, self.subrole)
        };
        if self.name.is_empty() {
            format!("{role} ({})", self.class)
        } else {
            format!("{role} {:?} ({})", short(&self.name), self.class)
        }
    }

    pub(crate) fn is_control(&self) -> bool {
        CONTROLS.contains(&self.role.as_str()) || NAMED_GROUPS.contains(&self.role.as_str())
    }

    pub(crate) fn enabled(&self) -> bool {
        read::enabled(&self.object)
    }

    /// Whether AppKit draws the element and VoiceOver names it by its subrole, as "close button".
    fn system_part(&self) -> bool {
        SYSTEM_PARTS.contains(&self.subrole.as_str())
            || (self.role == "AXDisclosureTriangle" && matches!(self.parent_role.as_str(), "AXCell" | "AXRow"))
    }

    /// Whether the element is text to read rather than to type into, which its content names.
    fn read_only_text(&self) -> bool {
        if let Some(text) = self.object.downcast_ref::<NSTextView>() {
            return !text.isEditable() && !text.string().is_empty();
        }
        if let Some(field) = self.object.downcast_ref::<NSTextField>() {
            return !field.isEditable() && !field.stringValue().is_empty();
        }
        false
    }
}

/// Something VoiceOver or a keyboard would miss.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    pub(crate) problem: String,
    pub(crate) what: String,
    pub(crate) path: String,
}

impl Finding {
    pub(crate) fn new(problem: impl Into<String>, what: impl Into<String>, path: impl Into<String>) -> Finding {
        Finding {
            problem: problem.into(),
            what: what.into(),
            path: path.into(),
        }
    }
}

/// The elements of `window` VoiceOver reaches, depth first; sheets and popovers are windows of their own.
pub(crate) fn elements(window: &NSWindow) -> Vec<Element> {
    let mut found = Vec::new();
    let root: &AnyObject = window.as_ref();
    let role = read::text_of(root, Property::Role);
    let path = format!("{role} {:?}", window.title().to_string());
    descend(root, &path, &role, 0, &mut found);
    found
}

/// What VoiceOver misses about `window` itself: it announces a window by its title, also where the title bar hides it.
pub(crate) fn window_findings(window: &NSWindow) -> Vec<Finding> {
    let root: &AnyObject = window.as_ref();
    let titled = window.styleMask().contains(NSWindowStyleMask::Titled);
    if titled && read::text_of(root, Property::Role) == "AXWindow" && window.title().to_string().trim().is_empty() {
        let class = window.class().name().to_string_lossy().into_owned();
        return vec![Finding::new("window without a title", class, "AXWindow \"\"")];
    }
    Vec::new()
}

fn descend(element: &AnyObject, path: &str, role: &str, depth: usize, found: &mut Vec<Element>) {
    if depth > 60 || found.len() > LIMIT {
        return;
    }
    for (index, child) in read::children(element).iter().enumerate() {
        let class = child.class().name().to_string_lossy();
        // An element of another process, such as the title bar's window tiling menu, which only it describes.
        if class == "NSAccessibilityRemoteUIElement" {
            continue;
        }
        let read = read(child, path, index, role);
        if matches!(read.role.as_str(), "AXSheet" | "AXPopover") {
            continue;
        }
        // The page's elements live in the web content process and say what the page says.
        let deeper = read.role != "AXWebArea" && !read.class.starts_with("WK");
        let (path, role) = (read.path.clone(), read.role.clone());
        found.push(read);
        if deeper {
            descend(child, &path, &role, depth + 1, found);
        }
    }
}

fn read(object: &AnyObject, parent: &str, index: usize, parent_role: &str) -> Element {
    let role = read::text_of(object, Property::Role);
    let subrole = read::text_of(object, Property::Subrole);
    let name = name_of(object, &role);
    let class = object.class().name().to_string_lossy().into_owned();
    let step = if name.is_empty() {
        format!("{role}#{index}")
    } else {
        format!("{role} {:?}", short(&name))
    };
    Element {
        object: object.retain(),
        path: format!("{parent} › {step}"),
        role,
        subrole,
        name,
        class,
        parent_role: parent_role.to_string(),
    }
}

/// What VoiceOver calls the element: its label, its title, the text of the element that labels it, a text field's
/// placeholder, static text's own text.
fn name_of(object: &AnyObject, role: &str) -> String {
    for which in [Property::Label, Property::Title] {
        let said = read::text_of(object, which);
        if !said.trim().is_empty() {
            return said;
        }
    }
    if let Some(label) = read::property(object, Property::TitleElement) {
        let said = read::text_of(&label, Property::Value);
        if !said.trim().is_empty() {
            return said;
        }
    }
    if TEXT_INPUTS.contains(&role) {
        return read::text_of(object, Property::Placeholder);
    }
    if role == "AXStaticText" {
        return read::text_of(object, Property::Value);
    }
    String::new()
}

fn short(name: &str) -> String {
    let line = name.lines().next().unwrap_or_default();
    if line.chars().count() > 48 {
        format!("{}…", line.chars().take(47).collect::<String>())
    } else {
        line.to_string()
    }
}

/// What VoiceOver would miss among `elements`.
pub(crate) fn findings(elements: &[Element]) -> Vec<Finding> {
    let mut found: Vec<Finding> = elements
        .iter()
        .flat_map(|element| {
            problems(element)
                .into_iter()
                .map(|p| Finding::new(p, element.what(), &element.path))
        })
        .collect();
    found.extend(repeated(elements));
    found
}

fn problems(element: &Element) -> Vec<String> {
    let role = element.role.as_str();
    let mut problems = Vec::new();
    let unnamed = element.name.trim().is_empty();
    if element.is_control() && unnamed && !element.system_part() && !element.read_only_text() {
        problems.push("has no name".to_string());
    }
    if role == "AXImage" && unnamed {
        problems.push("image without a description".to_string());
    }
    if !unnamed && (element.is_control() || role == "AXImage") && says_nothing(&element.name, role) {
        problems.push(format!("name says nothing: {:?}", element.name));
    }
    if role.is_empty() || role == "AXUnknown" {
        problems.push("has no role".to_string());
    }
    if PRESSABLE.contains(&role) && !read::can_press(&element.object) {
        problems.push("cannot be pressed".to_string());
    }
    if TEXT_INPUTS.contains(&role) && !element.read_only_text() && !read::focusable(&element.object) {
        problems.push("text input cannot take the focus".to_string());
    }
    if role == "AXRadioButton" {
        problems.extend(radio_problems(element));
    }
    problems
}

/// A radio button says whether it is on, and stands in a group of them; a tab is a radio button in a tab group with
/// the tab button subrole, as AppKit's own tabs are.
fn radio_problems(element: &Element) -> Vec<String> {
    let mut problems = Vec::new();
    let value = read::text_of(&element.object, Property::Value);
    if value != "0" && value != "1" {
        problems.push(format!("radio button does not say whether it is on: value {value:?}"));
    }
    let parent = element.parent_role.as_str();
    if parent != "AXTabGroup" && parent != "AXRadioGroup" {
        problems.push(format!("radio button outside a tab or radio group, in {parent}"));
    }
    if parent == "AXTabGroup" && element.subrole != "AXTabButton" {
        problems.push("tab without the tab button subrole".to_string());
    }
    problems
}

fn says_nothing(name: &str, role: &str) -> bool {
    let name = name.trim();
    let lower = name.to_lowercase();
    let role_word = role.trim_start_matches("AX").to_lowercase();
    let image_name = name.starts_with("NS") && name.chars().nth(2).is_some_and(|c| c.is_ascii_uppercase());
    let one_sign = name.chars().count() == 1 && !name.chars().all(char::is_alphanumeric);
    GENERIC.contains(&lower.as_str()) || lower == role_word || symbol_name(name) || image_name || one_sign
}

/// Whether `name` is the name of an SF Symbol, such as "doc.on.doc", which tells a listener nothing.
fn symbol_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
        && NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns(name), None).is_some()
}

/// Controls beside each other with the same role and name, which VoiceOver cannot tell apart.
fn repeated(elements: &[Element]) -> Vec<Finding> {
    let mut groups: BTreeMap<(String, String, String), Vec<&Element>> = BTreeMap::new();
    for element in elements
        .iter()
        .filter(|e| CONTROLS.contains(&e.role.as_str()) && !e.name.trim().is_empty())
    {
        let parent = element.path.rsplit_once(" › ").map_or("", |(parent, _)| parent);
        groups
            .entry((parent.to_string(), element.role.clone(), element.name.clone()))
            .or_default()
            .push(element);
    }
    groups
        .into_values()
        .filter(|same| same.len() > 1)
        .map(|same| {
            Finding::new(
                format!("{} controls beside each other have the same name", same.len()),
                same[0].what(),
                &same[0].path,
            )
        })
        .collect()
}

/// What turns wrongly when the interface reads right to left: code and terminal views stay left to right (E3).
pub(crate) fn direction_findings(window: &NSWindow) -> Vec<Finding> {
    let mut found = Vec::new();
    if let Some(content) = window.contentView() {
        let path = format!("{:?}", window.title().to_string());
        directions(&content, &path, &mut found);
    }
    found
}

fn directions(view: &NSView, path: &str, found: &mut Vec<Finding>) {
    let class = view.class().name().to_string_lossy().into_owned();
    let path = format!("{path} › {class}");
    let left_to_right = view.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::LeftToRight;
    if view.isHiddenOrHasHiddenAncestor() {
        return;
    }
    if class == "IDETerminalView" && !left_to_right {
        found.push(Finding::new("terminal turns right to left", &class, &path));
    }
    if let Some(text) = view.downcast_ref::<NSTextView>()
        && (class == "IDEEditorTextView" || text.font().is_some_and(|f| f.isFixedPitch()))
        && !text.isFieldEditor()
    {
        if !left_to_right {
            found.push(Finding::new("code view lays out right to left", &class, &path));
        }
        let own_scroll = text.enclosingScrollView().filter(|scroll| {
            scroll
                .documentView()
                .is_some_and(|document| std::ptr::eq(&*document, view))
        });
        if own_scroll.is_some_and(|s| s.userInterfaceLayoutDirection() != NSUserInterfaceLayoutDirection::LeftToRight) {
            found.push(Finding::new("code view's scroller moves to the left", &class, &path));
        }
        if text.baseWritingDirection() != NSWritingDirection::LeftToRight && text.alignment() != NSTextAlignment::Left {
            found.push(Finding::new("code runs right to left", &class, &path));
        }
    }
    for subview in view.subviews().iter() {
        directions(&subview, &path, found);
    }
}
