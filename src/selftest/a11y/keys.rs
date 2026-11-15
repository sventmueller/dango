//! The keyboard's side of the audit: the views Tab reaches in a window, the controls it never reaches, and key
//! presses handed on the way the application hands them on.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, Message, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSButton, NSCell, NSControl, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem,
    NSTableView, NSTextField, NSTextView, NSView, NSWindow,
};
use objc2_foundation::NSPoint;

use super::audit::{Element, Finding};
use crate::util::ns;

/// How many steps Tab may take before the loop counts as endless.
const STEPS: usize = 300;
/// How many views of the key view loop the check follows, where AppKit links every view of the window.
const LINKS: usize = 5000;

/// The views Tab moves the focus to in `window`, from its focus onwards, until it comes round; whether it did. The
/// focus is put back afterwards.
pub(crate) fn tab_loop(window: &NSWindow) -> (Vec<Retained<NSView>>, bool) {
    let start = focused(window);
    let mut reached: Vec<Retained<NSView>> = Vec::new();
    let mut round = false;
    let mut at = start.clone();
    for _ in 0..STEPS {
        // Tab in a text field ends its editing and moves on from the field, not from the field editor.
        let next = match &at {
            Some(view) => step(window, view),
            None => {
                window.selectNextKeyView(None);
                focused(window)
            }
        };
        // A popover's loop goes on into the window it points at, which has a walk of its own.
        let Some(view) = next.filter(|view| in_window(view, window)) else {
            break;
        };
        at = Some(view.clone());
        if reached.iter().any(|seen| std::ptr::eq(&**seen, &*view)) {
            round = true;
            break;
        }
        reached.push(view);
    }
    if let Some(start) = start {
        window.makeFirstResponder(Some(&start));
    }
    (reached, round)
}

/// Tab from `view` in `window`: the view it reaches, which then has the focus, unless it is a web view.
fn step(window: &NSWindow, view: &NSView) -> Option<Retained<NSView>> {
    // SAFETY: AppKit answers with a view of the window's key view loop or nil; the walk holds it retained.
    let next = unsafe { view.nextValidKeyView() };
    // WebKit answers the focus by focusing the page's first field and then asks for the focus once more, which would
    // arrive after the walk put the focus back.
    if let Some(web) = next.filter(|next| next.downcast_ref::<objc2_web_kit::WKWebView>().is_some()) {
        return Some(web);
    }
    window.selectKeyViewFollowingView(view);
    focused(window)
}

/// The view with the focus; for the field editor, the text field it edits.
pub(crate) fn focused(window: &NSWindow) -> Option<Retained<NSView>> {
    let view = window.firstResponder()?.downcast::<NSView>().ok()?;
    if let Some(text) = view.downcast_ref::<NSTextView>()
        && text.isFieldEditor()
    {
        // SAFETY: A text view answers `delegate` with an object or nil; a field editor's is the field it edits.
        let delegate: Option<Retained<AnyObject>> = unsafe { msg_send![text, delegate] };
        if let Some(field) = delegate.and_then(|d| d.downcast::<NSView>().ok()) {
            return Some(field);
        }
    }
    Some(view)
}

/// Why a control needs no place in the Tab loop, if it does not.
fn exempt(view: &NSView, window: &NSWindow) -> Option<&'static str> {
    let alert = view
        .downcast_ref::<NSControl>()
        .and_then(|control| control.cell())
        .is_some_and(|cell| cell.class().name().to_string_lossy() == "_NSAlertButtonCell");
    if alert {
        return Some("buttons of AppKit's alerts, pressed with Return, Esc and ⌘ with their first letter");
    }
    let content = window.contentView()?;
    if !view.isDescendantOf(&content) {
        return Some("title bar and toolbar, reached with ⌃F5");
    }
    if in_tab(view) {
        return Some("tabs, switched with ⇧⌘[ and ⇧⌘] and closed with ⌘W");
    }
    None
}

/// Whether `view` is a tab of a tab bar or inside one, as its close button is.
fn in_tab(view: &NSView) -> bool {
    let mut next = Some(view.retain());
    while let Some(view) = next {
        if view.class().name().to_string_lossy() == "IDETabItem" {
            return true;
        }
        // SAFETY: A view's superview outlives the walk, which holds each one retained.
        next = unsafe { view.superview() };
    }
    false
}

/// The view that stands for `object` in the key view loop: the object itself, or the control of a cell.
fn view_of(object: &AnyObject) -> Option<Retained<NSView>> {
    if let Some(view) = object.downcast_ref::<NSView>() {
        return Some(view.retain());
    }
    let cell = object.downcast_ref::<NSCell>()?;
    // SAFETY: The cell's control is alive while its window shows it, as during the walk.
    unsafe { cell.controlView() }
}

/// The table that `view` is a row or a cell of, which Tab reaches instead of what is inside.
fn table_of(view: &NSView) -> Option<Retained<NSTableView>> {
    // SAFETY: A view's superview outlives the walk, which holds each one retained.
    let mut next = unsafe { view.superview() };
    while let Some(view) = next {
        if let Ok(table) = view.clone().downcast::<NSTableView>() {
            return Some(table);
        }
        // SAFETY: As above.
        next = unsafe { view.superview() };
    }
    None
}

/// The views the key view loop of `window` links after `start`, as AppKit linked them, whether or not each would take
/// the focus now.
fn chain(start: &NSView) -> Vec<Retained<NSView>> {
    let mut linked: Vec<Retained<NSView>> = vec![start.retain()];
    while linked.len() < LINKS {
        // SAFETY: The next key view is weak in AppKit; the walk holds each one retained while it runs.
        let Some(next) = linked.last().and_then(|view| unsafe { view.nextKeyView() }) else {
            break;
        };
        if linked.iter().any(|seen| std::ptr::eq(&**seen, &*next)) {
            break;
        }
        linked.push(next);
    }
    linked
}

/// Whether `view` is a control Tab reaches only with full keyboard access, as buttons and pop-up menus, and does not
/// refuse it.
fn needs_full_keyboard_access(view: &NSView) -> bool {
    let typed = view.downcast_ref::<NSTextField>().is_some() || view.downcast_ref::<NSTableView>().is_some();
    view.downcast_ref::<NSControl>()
        .is_some_and(|control| !control.refusesFirstResponder())
        && !typed
}

/// Whether the key view loop links `view`, as AppKit does with each view it lets Tab reach: it is among `loop_views`,
/// or following the links from it comes back to it.
pub(crate) fn linked(view: &NSView, loop_views: &[Retained<NSView>]) -> bool {
    loop_views.iter().any(|seen| std::ptr::eq(&**seen, view))
        || chain(view).iter().skip(1).any(|next| std::ptr::eq(&**next, view))
}

/// The longest run of views the key view loop of `window` links, from where Tab started or the window's first views.
pub(crate) fn key_view_loop(window: &NSWindow, reached: &[Retained<NSView>]) -> Vec<Retained<NSView>> {
    [
        reached.first().cloned(),
        window.initialFirstResponder(),
        window.contentView(),
    ]
    .into_iter()
    .flatten()
    .filter(|start| in_window(start, window))
    .map(|start| chain(&start))
    .max_by_key(Vec::len)
    .unwrap_or_default()
}

fn in_window(view: &NSView, window: &NSWindow) -> bool {
    view.window().is_some_and(|own| std::ptr::eq(&*own, window))
}

/// The controls among `elements` of `window` that Tab never reaches, and how many were left out with the reason. With
/// full keyboard access off on this Mac, a button counts as reached where the key view loop links it.
pub(crate) fn unreached(window: &NSWindow, elements: &[Element]) -> (Vec<Finding>, Vec<(&'static str, usize)>) {
    let (reached, round) = tab_loop(window);
    let loop_views = key_view_loop(window, &reached);
    let has = |views: &[Retained<NSView>], view: &NSView| views.iter().any(|seen| std::ptr::eq(&**seen, view));
    let mut found = Vec::new();
    let mut left_out: Vec<(&'static str, usize)> = Vec::new();
    if !round && !reached.is_empty() {
        let last = reached.last().map(|v| v.class().name().to_string_lossy().into_owned());
        found.push(Finding::new(
            "Tab stops instead of coming round",
            last.unwrap_or_default(),
            format!("{:?}", window.title().to_string()),
        ));
    }
    for element in elements.iter().filter(|e| e.is_control() && e.enabled()) {
        let Some(view) = view_of(&element.object) else {
            continue;
        };
        if view.isHiddenOrHasHiddenAncestor() || view.visibleRect().size.width < 1.0 || view.window().is_none() {
            continue;
        }
        if let Some(reason) = exempt(&view, window) {
            match left_out.iter_mut().find(|(r, _)| *r == reason) {
                Some((_, count)) => *count += 1,
                None => left_out.push((reason, 1)),
            }
            continue;
        }
        let through_table = table_of(&view).is_some_and(|table| has(&reached, &table));
        let with_access = needs_full_keyboard_access(&view) && linked(&view, &loop_views);
        if !has(&reached, &view) && !through_table && !with_access {
            found.push(Finding::new("Tab never reaches it", element.what(), &element.path));
        }
    }
    (found, left_out)
}

/// A key press with `characters`, the key `code` and `modifiers` for `window`.
pub(crate) fn key_event(
    window: &NSWindow,
    characters: &str,
    code: u16,
    modifiers: NSEventModifierFlags,
) -> Retained<NSEvent> {
    let plain = if modifiers.contains(NSEventModifierFlags::Shift) {
        characters.to_lowercase()
    } else {
        characters.to_string()
    };
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        modifiers,
        0.0,
        window.windowNumber(),
        None,
        &ns(characters),
        &ns(&plain),
        false,
        code,
    )
    .expect("AppKit makes a key event from valid characters")
}

/// Where a key press went.
pub(crate) enum Went {
    /// A view of the window took it as a key equivalent.
    Equivalent,
    /// The menu bar's item with that shortcut sent its action.
    Menu(String),
    /// The menu bar has an item with that shortcut, but nothing enabled it.
    Disabled(String),
    /// The view with the focus got it.
    Focus,
}

impl std::fmt::Display for Went {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Went::Equivalent => f.write_str("a view's key equivalent"),
            Went::Menu(title) => write!(f, "menu item {title:?}"),
            Went::Disabled(title) => write!(f, "menu item {title:?}, disabled"),
            Went::Focus => f.write_str("the focus"),
        }
    }
}

/// Presses a key in `window` as the application hands it on: with ⌘ or ⌃ first as a key equivalent to the window's
/// views, then to the menu bar's item with that shortcut; else to the view with the focus, whose responder chain ends
/// at the window. The keys go to the window itself, since a selftest app is never the active one and has no key
/// window.
pub(crate) fn press(window: &NSWindow, characters: &str, code: u16, modifiers: NSEventModifierFlags) -> Went {
    let event = key_event(window, characters, code, modifiers);
    if modifiers.intersects(NSEventModifierFlags::Command | NSEventModifierFlags::Control) {
        if window.performKeyEquivalent(&event) {
            return Went::Equivalent;
        }
        let app = NSApplication::sharedApplication(MainThreadMarker::from(window));
        if let Some(item) = app.mainMenu().and_then(|menu| item_for(&menu, &event)) {
            let title = item.title().to_string();
            return if perform(window, &item) {
                Went::Menu(title)
            } else {
                Went::Disabled(title)
            };
        }
    }
    // An alert begun while the app is not active takes Return off its default button and only names it here.
    if characters == "\r"
        && modifiers.is_empty()
        && window.isSheet()
        && let Some(default) = window.defaultButtonCell()
    {
        // SAFETY: A click takes any sender; none is given.
        unsafe { default.performClick(None) };
        return Went::Equivalent;
    }
    // With full keyboard access, Space presses the button with the focus.
    if characters == " "
        && let Some(button) = window.firstResponder().and_then(|r| r.downcast::<NSButton>().ok())
    {
        // SAFETY: A click takes any sender; none is given.
        unsafe { button.performClick(None) };
        return Went::Focus;
    }
    if characters == "\t" && !types_tabs(window) {
        let view = focused(window);
        match (view, modifiers.contains(NSEventModifierFlags::Shift)) {
            (Some(view), false) => window.selectKeyViewFollowingView(&view),
            (Some(view), true) => window.selectKeyViewPrecedingView(&view),
            (None, _) => window.selectNextKeyView(None),
        }
        return Went::Focus;
    }
    match window.firstResponder() {
        Some(responder) => responder.keyDown(&event),
        None => window.keyDown(&event),
    }
    Went::Focus
}

/// Whether the view with the focus of `window` takes Tab as input, as an editable text view and a terminal do; any
/// other view leaves it to the window, which moves the focus on.
fn types_tabs(window: &NSWindow) -> bool {
    let Some(responder) = window.firstResponder() else {
        return false;
    };
    let text = responder
        .downcast_ref::<NSTextView>()
        .is_some_and(|text| text.isEditable() && !text.isFieldEditor());
    text || responder.class().name().to_string_lossy() == "IDETerminalView"
}

/// The shortcut of the menu bar's item for `action`, as AppKit shows it: its characters and modifiers.
pub(crate) fn shortcut_of(window: &NSWindow, action: Sel) -> Option<(String, NSEventModifierFlags)> {
    fn find(menu: &NSMenu, action: Sel) -> Option<Retained<NSMenuItem>> {
        menu.itemArray().iter().find_map(|item| match item.submenu() {
            Some(submenu) => find(&submenu, action),
            None => (item.action() == Some(action)).then(|| item.clone()),
        })
    }
    let app = NSApplication::sharedApplication(MainThreadMarker::from(window));
    let menu = app.mainMenu()?;
    let item = find(&menu, action)?;
    let key = item.keyEquivalent().to_string();
    let mut modifiers = item.keyEquivalentModifierMask();
    if key.chars().any(char::is_uppercase) {
        modifiers |= NSEventModifierFlags::Shift;
    }
    (!key.is_empty()).then_some((key, modifiers))
}

/// Types `text` into the view with the focus of `window`, a key press for each character.
pub(crate) fn type_keys(window: &NSWindow, text: &str) {
    for c in text.chars() {
        let _ = press(window, &c.to_string(), 0, NSEventModifierFlags::empty());
    }
}

/// Sends the action of `item` as a menu does for `window` as the key window: to the item's target, else to the first
/// along the window's responder chain, its delegate, the application and its delegate that answers it, if that one
/// enables the item; whether it went out.
pub(crate) fn perform(window: &NSWindow, item: &NSMenuItem) -> bool {
    let Some(action) = item.action() else { return false };
    let Some(target) = item.target().or_else(|| chain_target(window, action)) else {
        return false;
    };
    if !validated(&target, item) {
        return false;
    }
    let app = NSApplication::sharedApplication(MainThreadMarker::from(window));
    let sender: &AnyObject = item.as_ref();
    // SAFETY: The target answers the action, which takes the sender, the item, as AppKit requires of menu items.
    unsafe { app.sendAction_to_from(action, Some(&target), Some(sender)) }
}

/// The first object that answers `action` for `window` as the key window.
fn chain_target(window: &NSWindow, action: Sel) -> Option<Retained<AnyObject>> {
    let mut next = window.firstResponder();
    while let Some(responder) = next {
        if responder.class().responds_to(action) {
            let object: &AnyObject = responder.as_ref();
            return Some(object.retain());
        }
        // SAFETY: A responder's next one is a responder or nil.
        next = unsafe { responder.nextResponder() };
    }
    let mtm = MainThreadMarker::from(window);
    let app = NSApplication::sharedApplication(mtm);
    let application: &AnyObject = app.as_ref();
    let others: [Option<Retained<AnyObject>>; 3] = [
        window.delegate().map(Retained::<AnyObject>::from),
        Some(application.retain()),
        app.delegate().map(Retained::<AnyObject>::from),
    ];
    others
        .into_iter()
        .flatten()
        .find(|other| other.class().responds_to(action))
}

/// Whether `target` enables `item`, as AppKit asks it before sending the item's action.
fn validated(target: &AnyObject, item: &NSMenuItem) -> bool {
    if !target.class().responds_to(sel!(validateMenuItem:)) {
        return true;
    }
    // SAFETY: An object that answers `validateMenuItem:` takes a menu item and returns a BOOL.
    unsafe { msg_send![target, validateMenuItem: item] }
}

/// The item of `menu` or its submenus whose shortcut `event` is.
fn item_for(menu: &NSMenu, event: &NSEvent) -> Option<Retained<NSMenuItem>> {
    let flags = NSEventModifierFlags::Command
        | NSEventModifierFlags::Shift
        | NSEventModifierFlags::Option
        | NSEventModifierFlags::Control;
    let wanted = event.modifierFlags() & flags;
    let key = event
        .charactersIgnoringModifiers()
        .map(|c| c.to_string())
        .unwrap_or_default();
    menu.itemArray().iter().find_map(|item| {
        if let Some(submenu) = item.submenu() {
            return item_for(&submenu, event);
        }
        let equivalent = item.keyEquivalent().to_string();
        let mut mask = item.keyEquivalentModifierMask() & flags;
        if equivalent.chars().any(char::is_uppercase) {
            mask |= NSEventModifierFlags::Shift;
        }
        (!equivalent.is_empty()
            && equivalent.to_lowercase() == key.to_lowercase()
            && mask == wanted
            && !item.isHidden())
        .then(|| item.clone())
    })
}
