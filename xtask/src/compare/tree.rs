//! The measured app's accessibility tree: a row of its file tree to click, and dialogs that must not be there.

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CGPoint, CGRect, CGSize, Type};

use crate::compare::apps::{ax_attribute, main_window};

/// More elements than a window of an editor has; the search stops there instead of walking a huge tree.
const MAX_ELEMENTS: usize = 20_000;

fn text(element: &AXUIElement, name: &str) -> Option<String> {
    Some(ax_attribute(element, name)?.downcast::<CFString>().ok()?.to_string())
}

fn children(element: &AXUIElement) -> Vec<CFRetained<AXUIElement>> {
    let Some(list) = ax_attribute(element, "AXChildren").and_then(|v| v.downcast::<CFArray>().ok()) else {
        return Vec::new();
    };
    // SAFETY: AXChildren is an array of AXUIElements.
    let list = unsafe { CFRetained::cast_unchecked::<CFArray<AXUIElement>>(list) };
    list.iter().collect()
}

fn geometry<T: Default>(element: &AXUIElement, name: &str, kind: AXValueType) -> Option<T> {
    let value = ax_attribute(element, name)?.downcast::<AXValue>().ok()?;
    let mut out = T::default();
    // SAFETY: `out` is the CGPoint or CGSize that `kind` names.
    unsafe { value.value(kind, std::ptr::NonNull::from(&mut out).cast()) }.then_some(out)
}

/// Where `element` is on screen, in points from the top left of the main display.
fn frame(element: &AXUIElement) -> Option<CGRect> {
    let origin: CGPoint = geometry(element, "AXPosition", AXValueType::CGPoint)?;
    let size: CGSize = geometry(element, "AXSize", AXValueType::CGSize)?;
    (size.width > 0.0 && size.height > 0.0).then_some(CGRect::new(origin, size))
}

/// The middle of the first element of the main window of `pid` that shows `name` and lies within `inside`, as the
/// row of a file tree does, with its role and frame for the log.
pub(crate) fn find_named(pid: libc::pid_t, name: &str, inside: CGRect) -> Option<(CGPoint, String)> {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    // The window itself is left out, as its title may name the file in front.
    let window = main_window(&app)?;
    let mut stack = children(&window);
    let mut visited = 0;
    while let Some(element) = stack.pop() {
        visited += 1;
        if visited > MAX_ELEMENTS {
            return None;
        }
        let shows = ["AXValue", "AXTitle", "AXDescription"]
            .into_iter()
            .any(|attribute| text(&element, attribute).is_some_and(|t| t == name));
        if shows && let Some(at) = frame(&element) {
            let middle = CGPoint::new(at.origin.x + at.size.width / 2.0, at.origin.y + at.size.height / 2.0);
            if contains(inside, middle) {
                let role = text(&element, "AXRole").unwrap_or_default();
                return Some((middle, format!("{role} {name:?} at {at:?}")));
            }
        }
        stack.extend(children(&element));
    }
    None
}

/// The titles of the buttons and the texts within `element`, for the log.
fn describe(element: &AXUIElement) -> String {
    let mut parts = Vec::new();
    let mut stack = vec![element.retain()];
    let mut visited = 0;
    while let Some(next) = stack.pop() {
        visited += 1;
        if visited > 500 {
            break;
        }
        match text(&next, "AXRole").as_deref() {
            Some("AXButton") => parts.extend(text(&next, "AXTitle").map(|t| format!("[{t}]"))),
            Some("AXStaticText") => parts.extend(text(&next, "AXValue")),
            _ => {}
        }
        stack.extend(children(&next));
    }
    parts.join(" ")
}

/// The frame of the first element of `role` in the main window of `pid` whose middle lies within `inside`.
pub(crate) fn frame_of_role(pid: libc::pid_t, role: &str, inside: CGRect) -> Option<CGRect> {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    let window = main_window(&app)?;
    let mut stack = children(&window);
    let mut visited = 0;
    while let Some(element) = stack.pop() {
        visited += 1;
        if visited > MAX_ELEMENTS {
            return None;
        }
        if text(&element, "AXRole").as_deref() == Some(role)
            && let Some(at) = frame(&element)
            && contains(
                inside,
                CGPoint::new(at.origin.x + at.size.width / 2.0, at.origin.y + at.size.height / 2.0),
            )
        {
            return Some(at);
        }
        stack.extend(children(&element));
    }
    None
}

fn contains(rect: CGRect, point: CGPoint) -> bool {
    point.x > rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y > rect.origin.y
        && point.y < rect.origin.y + rect.size.height
}

/// An open menu of `pid`, such as a context menu, described by its items.
pub(crate) fn open_menu(pid: libc::pid_t) -> Option<String> {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    let menu = children(&app)
        .into_iter()
        .find(|c| text(c, "AXRole").as_deref() == Some("AXMenu"))?;
    let items: Vec<String> = children(&menu)
        .iter()
        .filter_map(|item| text(item, "AXTitle"))
        .take(5)
        .collect();
    Some(items.join(", "))
}

/// A dialog, sheet or panel of `pid` besides its document windows, described for the log.
pub(crate) fn dialog(pid: libc::pid_t) -> Option<String> {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    let windows = ax_attribute(&app, "AXWindows")?.downcast::<CFArray>().ok()?;
    // SAFETY: AXWindows is an array of AXUIElements.
    let windows = unsafe { CFRetained::cast_unchecked::<CFArray<AXUIElement>>(windows) };
    for window in windows.iter() {
        let subrole = text(&window, "AXSubrole").unwrap_or_default();
        if matches!(subrole.as_str(), "AXDialog" | "AXSystemDialog" | "AXFloatingWindow") {
            let title = text(&window, "AXTitle").unwrap_or_default();
            return Some(format!("{subrole} {title:?} ({})", describe(&window)));
        }
        if let Some(sheet) = children(&window)
            .iter()
            .find(|c| text(c, "AXRole").as_deref() == Some("AXSheet"))
        {
            let title = text(sheet, "AXTitle").unwrap_or_default();
            return Some(format!("sheet {title:?} ({})", describe(sheet)));
        }
    }
    None
}
