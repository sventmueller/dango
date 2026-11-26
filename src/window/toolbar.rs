//! The toolbar of each window tab, ordered as Tapa's from the trailing edge (E201): a button per kind of content a new
//! tab holds (terminals, agents, browsers); left of them the splits (E145) and the input field's switch, which acts on
//! the terminal in front and is greyed without one; left of those Open Quickly and the activity, which is
//! `crate::activity`'s. The branch stays left, after back and forward and the title.

use ide_process::Kind;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSApplication, NSColor, NSEventType, NSMenu, NSMenuToolbarItem, NSToolbarFlexibleSpaceItemIdentifier,
    NSToolbarItem, NSToolbarItemIdentifier, NSToolbarSidebarTrackingSeparatorItemIdentifier,
    NSToolbarSpaceItemIdentifier, NSToolbarToggleSidebarItemIdentifier,
};
use objc2_foundation::{NSArray, NSPoint};
use splat_i18n::{t, tf};
use term_ui::{FieldLook, TerminalView};

use super::WindowController;
use super::agent_choices::AgentChoices;
use crate::util::{constant, ns};

const ACTIVITY: &str = "ide.activity";
const QUICK_OPEN: &str = "ide.quickOpen";
pub(crate) const SPLIT: &str = "ide.split";
pub(crate) const SPLIT_BELOW: &str = "ide.splitBelow";
pub(crate) const FIELD: &str = "ide.inputField";
pub(crate) const NEW_TERMINAL: &str = "ide.newTerminal";
pub(crate) const NEW_AGENT: &str = "ide.newAgent";
pub(crate) const NEW_BROWSER: &str = "ide.newBrowser";
/// The input field's switch: a box with a character, as Tapa shows it.
const FIELD_SYMBOL: &str = "character.textbox";
/// The point size of the switch's symbol while the field is on.
const ON_POINTS: f64 = 17.0;

pub(crate) fn default_identifiers() -> Retained<NSArray<NSToolbarItemIdentifier>> {
    let space = || constant!(NSToolbarSpaceItemIdentifier).to_owned();
    // A space between two groups of buttons keeps them apart, each in a capsule of its own.
    let ids = [
        constant!(NSToolbarToggleSidebarItemIdentifier).to_owned(),
        constant!(NSToolbarSidebarTrackingSeparatorItemIdentifier).to_owned(),
        ns(crate::navigation::TOOLBAR_ID),
        ns(ide_ui_git::TOOLBAR_ID),
        constant!(NSToolbarFlexibleSpaceItemIdentifier).to_owned(),
        ns(QUICK_OPEN),
        ns(ACTIVITY),
        space(),
        ns(SPLIT),
        ns(SPLIT_BELOW),
        ns(FIELD),
        space(),
        ns(NEW_TERMINAL),
        ns(NEW_AGENT),
        ns(NEW_BROWSER),
    ];
    NSArray::from_retained_slice(&ids)
}

pub(crate) fn make(controller: &WindowController, id: &str) -> Option<Retained<NSToolbarItem>> {
    let item = match id {
        ACTIVITY => crate::activity::toolbar_item(controller),
        QUICK_OPEN => quick_open(controller),
        SPLIT => button(
            controller,
            id,
            "rectangle.split.2x1",
            t("Rechts teilen"),
            sel!(ideSplitRight:),
        ),
        SPLIT_BELOW => button(
            controller,
            id,
            "rectangle.split.1x2",
            t("Unten teilen"),
            sel!(ideSplitBelow:),
        ),
        FIELD => button(
            controller,
            id,
            FIELD_SYMBOL,
            FieldLook::name(),
            sel!(ideToggleInputField:),
        ),
        NEW_TERMINAL => button(
            controller,
            id,
            "apple.terminal",
            t("Neues Terminal"),
            sel!(ideNewTerminal:),
        ),
        NEW_AGENT => agent_button(controller),
        NEW_BROWSER => button(controller, id, "globe", t("Neuer Browser-Tab"), sel!(ideNewBrowser:)),
        _ if id == crate::navigation::TOOLBAR_ID => crate::navigation::toolbar_item(controller),
        _ if id == ide_ui_git::TOOLBAR_ID => ide_ui_git::toolbar_item(controller),
        _ => return None,
    };
    Some(item)
}

/// The magnifying glass that opens Open Quickly, which otherwise only its shortcut reaches.
fn quick_open(controller: &WindowController) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(QUICK_OPEN));
    item.setImage(crate::util::symbol("magnifyingglass", t("Schnell öffnen")).as_deref());
    item.setLabel(&ns(t("Schnell öffnen")));
    item.setToolTip(Some(&ns(t("Schnell öffnen"))));
    // SAFETY: Without a target the item sends its action along the responder chain, where the window answers it.
    unsafe { item.setAction(Some(objc2::sel!(ideQuickOpen:))) };
    item
}

/// A button that sends `action` along the responder chain, where the window answers it.
fn button(controller: &WindowController, id: &str, symbol: &str, label: &str, action: Sel) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(id));
    item.setImage(crate::util::symbol(symbol, label).as_deref());
    item.setLabel(&ns(label));
    item.setToolTip(Some(&ns(label)));
    // SAFETY: Without a target the item sends its action along the responder chain, where the window answers it.
    unsafe { item.setAction(Some(action)) };
    item
}

/// Whether `item` of one of `controller`'s toolbars can act now. The input field's switch shows the state of the
/// field of the terminal in front of its window tab, and is greyed without one or while the field cannot show.
pub(crate) fn validate(controller: &WindowController, item: &NSToolbarItem) -> bool {
    if *item.itemIdentifier() != *ns(FIELD) {
        return true;
    }
    let look = terminal_of(controller, item).map(|view| view.field_look());
    show_look(item, look);
    matches!(look, Some(FieldLook::On | FieldLook::Off))
}

/// Shows on the switch in the toolbar of `controller`'s window tab in front what the field of its terminal does now,
/// which changes without an event that would have AppKit ask.
pub(crate) fn show_field(controller: &WindowController) {
    if let Some(toolbar) = controller.current_tab().and_then(|tab| tab.window.toolbar()) {
        toolbar.validateVisibleItems();
    }
}

/// Turns the input field of the terminal in front of the window tab whose toolbar holds `item` on or off.
pub(crate) fn toggle_field(controller: &WindowController, item: &NSToolbarItem) {
    if let Some(view) = terminal_of(controller, item) {
        crate::terminal::toggle_input_field(controller, &view);
    }
}

/// The terminal in front of the window tab of `controller` whose toolbar holds `item`.
fn terminal_of(controller: &WindowController, item: &NSToolbarItem) -> Option<Retained<TerminalView>> {
    let toolbar = item.toolbar()?;
    let tab = controller
        .open_tabs()
        .into_iter()
        .find(|tab| tab.window.toolbar().is_some_and(|own| std::ptr::eq(&*own, &*toolbar)))?;
    crate::terminal::of_document(controller, tab.focused()?.id)
}

fn show_look(item: &NSToolbarItem, look: Option<FieldLook>) {
    let on = look == Some(FieldLook::On);
    // A selectable item would take a capsule of its own, apart from the splits it belongs with, so the accent colour
    // goes into the image.
    if item.image().is_none_or(|image| image.isTemplate() == on) {
        let image = if on {
            // A tinted image is drawn as it is, so it needs the size the toolbar gives its symbols.
            term_ui::kit::symbol_at(FIELD_SYMBOL, FieldLook::name(), ON_POINTS)
                .map(|symbol| term_ui::kit::tinted(&symbol, &NSColor::controlAccentColor()))
        } else {
            crate::util::symbol(FIELD_SYMBOL, FieldLook::name())
        };
        item.setImage(image.as_deref());
    }
    let tip = look.map_or_else(FieldLook::name, FieldLook::tool_tip);
    if item.toolTip().is_none_or(|shown| shown.to_string() != tip) {
        item.setToolTip(Some(&ns(tip)));
    }
}

/// The agent a click on the agent button opens without its menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnlyAgent {
    Claude,
    Preset(usize),
    Own,
}

/// The agent a click opens as in Tapa: the only one the menu offers that can start.
fn only_agent(choices: &AgentChoices) -> Option<OnlyAgent> {
    let presets = choices
        .installed_presets
        .iter()
        .filter(|(index, _)| choices.presets.get(*index).is_some_and(|(_, usable)| *usable))
        .map(|(index, _)| OnlyAgent::Preset(*index));
    let mut offered = choices
        .claude
        .then_some(OnlyAgent::Claude)
        .into_iter()
        .chain(presets)
        .chain(choices.own.then_some(OnlyAgent::Own));
    match (offered.next(), offered.next()) {
        (Some(agent), None) => Some(agent),
        _ => None,
    }
}

/// New AI Thread: the agents of the "+" menu (E65).
fn agent_button(controller: &WindowController) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    let item = NSMenuToolbarItem::initWithItemIdentifier(NSMenuToolbarItem::alloc(mtm), &ns(NEW_AGENT));
    let label = t("Neuer AI-Thread");
    item.setImage(crate::util::symbol("sparkles", label).as_deref());
    item.setLabel(&ns(label));
    item.setToolTip(Some(&ns(label)));
    item.setShowsIndicator(false);
    // Agents that cannot start grey their own entries; the button stays, also before the agents were looked for.
    item.setAutovalidates(false);
    item.setEnabled(true);
    let target: &AnyObject = controller;
    // Without an action AppKit draws the button as a pull-down of its own, outside the capsule it shares with new
    // terminal and new browser; with one, a click sends it and a long press shows the menu.
    // SAFETY: The window controller answers the button's action and outlives the toolbars of its window tabs.
    unsafe {
        item.setTarget(Some(target));
        item.setAction(Some(sel!(ideNewAgent:)));
    }
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    // AppKit never opens an empty menu, so it would never ask the delegate to fill it.
    super::tabs::add_agents(mtm, &menu, &super::agent_choices::now(controller.context()));
    super::new_tab_button::aim(&menu, controller);
    menu.setDelegate(Some(ProtocolObject::from_ref(controller)));
    item.setMenu(&menu);
    Retained::into_super(item)
}

/// Fills `menu`, an agent button's, as it opens with the agents as read last, and reads them again for the next
/// opening, as the "+" menu does.
pub(crate) fn agents_menu_opens(controller: &WindowController, menu: &NSMenu) {
    let mtm = controller.mtm();
    let choices = super::agent_choices::now(controller.context());
    let fresh = NSMenu::new(mtm);
    super::tabs::add_agents(mtm, &fresh, &choices);
    super::tabs::fill_new_tab_menu(controller, &fresh);
    // AppKit asks again while an item is being chosen; rebuilding then would drop the chosen item's action.
    if shape(menu) != shape(&fresh) {
        menu.removeAllItems();
        for entry in fresh.itemArray().iter() {
            fresh.removeItem(&entry);
            menu.addItem(&entry);
        }
        super::new_tab_button::aim(menu, controller);
    }
    super::agent_choices::refresh(controller.context());
}

/// The titles of `menu`'s entries and their submenus', and whether each is enabled.
fn shape(menu: &NSMenu) -> Vec<(String, bool)> {
    let mut shape = Vec::new();
    for entry in menu.itemArray().iter() {
        shape.push((entry.title().to_string(), entry.isEnabled()));
        if let Some(submenu) = entry.submenu() {
            shape.extend(self::shape(&submenu));
        }
    }
    shape
}

/// A click on `button`, the agent button: opens the only agent on offer in a new tab, else shows the button's menu.
pub(crate) fn agent_clicked(controller: &WindowController, button: &NSMenuToolbarItem) {
    let Some(agent) = only_agent(&super::agent_choices::now(controller.context())) else {
        return pop_up(controller, &button.menu());
    };
    controller.at_end(|| {
        let tab = match agent {
            OnlyAgent::Claude => ide_ui_agent::open_tab(controller, None),
            OnlyAgent::Preset(index) => ide_ui_agent::open_preset(controller, index),
            OnlyAgent::Own => ide_ui_agent::open_own(controller, None),
        };
        if let Some(tab) = tab {
            tab.focus_composer();
        }
    });
}

/// Shows `menu` below the toolbar where it was clicked, else below the middle of the toolbar of the window in front.
fn pop_up(controller: &WindowController, menu: &NSMenu) {
    let mtm = controller.mtm();
    let event = NSApplication::sharedApplication(mtm).currentEvent();
    let mouse = event.filter(|event| matches!(event.r#type(), NSEventType::LeftMouseDown | NSEventType::LeftMouseUp));
    let Some(window) = mouse
        .as_ref()
        .and_then(|event| event.window(mtm))
        .or_else(|| controller.window())
    else {
        return;
    };
    let below = window.contentLayoutRect();
    let x = mouse.as_ref().map_or(below.origin.x + below.size.width / 2.0, |event| {
        event.locationInWindow().x
    });
    let at = window.convertPointToScreen(NSPoint::new(x, below.origin.y + below.size.height));
    menu.popUpMenuPositioningItem_atLocation_inView(None, at, None);
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

/// What a process is there for, as the activity view names it.
pub(crate) fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Terminal => t("Terminal"),
        Kind::LanguageServer => t("Sprachserver"),
        Kind::Agent => t("Agent"),
        Kind::DevServer => t("Dev-Server"),
        Kind::Git => t("Git"),
        Kind::DebugAdapter => t("Debug-Adapter"),
        Kind::Tool => t("Werkzeug"),
        Kind::Test => t("Test"),
        Kind::AgentCommand => t("Agent-Befehl"),
        Kind::Gate => t("Prüfstation"),
    }
}

/// A running time like `12 s`, `3 min` or `2 h`.
pub(crate) fn duration(seconds: u64) -> String {
    match seconds {
        0..60 => tf("{} s", &[&seconds.to_string()]),
        60..3600 => tf("{} min", &[&(seconds / 60).to_string()]),
        _ => tf("{} h", &[&(seconds / 3600).to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentChoices, OnlyAgent, duration, only_agent};

    #[test]
    fn running_times_read_in_the_largest_whole_unit() {
        assert_eq!(duration(5), "5 s");
        assert_eq!(duration(125), "2 min");
        assert_eq!(duration(7300), "2 h");
    }

    #[test]
    fn a_click_on_the_agent_button_opens_the_only_agent_that_can_start() {
        let claude = AgentChoices {
            claude: true,
            ..AgentChoices::default()
        };
        assert_eq!(only_agent(&claude), Some(OnlyAgent::Claude));
        let greyed_preset = AgentChoices {
            presets: vec![("Codex".into(), false)],
            installed_presets: vec![(0, "Codex".into())],
            ..claude.clone()
        };
        assert_eq!(
            only_agent(&greyed_preset),
            Some(OnlyAgent::Claude),
            "a greyed entry is no choice"
        );
        let two = AgentChoices { own: true, ..claude };
        assert_eq!(only_agent(&two), None, "two agents leave the choice to the menu");
        assert_eq!(
            only_agent(&AgentChoices::default()),
            None,
            "without an agent the menu says so"
        );
    }
}
