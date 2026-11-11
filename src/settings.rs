//! The settings window (⌘,), with a tab per area in its toolbar.

mod agent;
mod agents;
mod ci;
#[cfg(feature = "dictation")]
mod dictation;
mod list_bar;
mod servers;

use std::cell::RefCell;

use ide_core::settings::{Locked, SearchEngine, Settings, keys};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSGridView, NSPopUpButton, NSStepper,
    NSTabViewController, NSTabViewControllerTabStyle, NSTabViewItem, NSTextField, NSView, NSViewController, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::{t, tf};

use crate::util::{guarded, label, ns, set_action, symbol};

/// Where the CI tab sits in the toolbar.
const CI_TAB: isize = 1;
/// Where the Agents tab sits in the toolbar.
const AGENTS_TAB: isize = 2;
/// Where the own agent's tab sits in the toolbar.
const AGENT_TAB: isize = 3;
/// Where the language servers tab sits in the toolbar.
#[cfg(feature = "selftest")]
const SERVERS_TAB: isize = 4;
/// Where the dictation tab sits in the toolbar.
#[cfg(all(feature = "selftest", feature = "dictation"))]
const DICTATION_TAB: isize = 5;

pub struct Ivars {
    settings: RefCell<Settings>,
    font_label: RefCell<Option<Retained<NSTextField>>>,
    ci: Retained<ci::CiSettings>,
    agents: Retained<agents::AgentsSettings>,
    agent: Retained<agent::AgentSettings>,
    servers: Retained<servers::ServersSettings>,
    #[cfg(feature = "dictation")]
    dictation: Retained<dictation::DictationSettings>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `SettingsController` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESettingsController"]
    #[ivars = Ivars]
    pub struct SettingsController;

    unsafe impl NSObjectProtocol for SettingsController {}

    impl SettingsController {
        #[unsafe(method(ideToggleReopen:))]
        fn toggle_reopen(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::REOPEN_PROJECTS, &on));
            });
        }

        #[unsafe(method(ideTogglePreviewTabs:))]
        fn toggle_preview_tabs(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::PREVIEW_TABS, &on));
            });
        }

        #[unsafe(method(ideToggleMarkdownPreview:))]
        fn toggle_markdown_preview(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::MARKDOWN_PREVIEW, &on));
            });
        }

        #[unsafe(method(ideToggleFormatOnSave:))]
        fn toggle_format_on_save(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::FORMAT_ON_SAVE, &on));
            });
        }

        #[unsafe(method(ideToggleOptionAsMeta:))]
        fn toggle_option_as_meta(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::OPTION_AS_META, &on));
                crate::terminal::set_option_as_meta(on);
            });
        }

        #[unsafe(method(ideFontSizeChanged:))]
        fn font_size_changed(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(stepper) = sender.and_then(|s| s.downcast_ref::<NSStepper>()) else { return };
                let size = stepper.doubleValue().round();
                self.store(|settings| settings.set(&keys::FONT_SIZE, &size));
                crate::terminal::set_font_size(size);
                if let Some(field) = self.ivars().font_label.borrow().as_ref() {
                    field.setStringValue(&ns(&font_size_text(size)));
                }
            });
        }

        #[unsafe(method(ideSearchEngineChanged:))]
        fn search_engine_changed(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(popup) = sender.and_then(|s| s.downcast_ref::<NSPopUpButton>()) else { return };
                let chosen = usize::try_from(popup.indexOfSelectedItem()).ok();
                if let Some(engine) = chosen.and_then(|i| SearchEngine::ALL.get(i)) {
                    self.store(|settings| settings.set(&keys::SEARCH_ENGINE, engine));
                }
            });
        }
    }
);

thread_local! {
    static WINDOW: RefCell<Option<(Retained<NSWindow>, Retained<SettingsController>)>> = const { RefCell::new(None) };
}

impl SettingsController {
    fn new(mtm: MainThreadMarker) -> Retained<SettingsController> {
        let this = Self::alloc(mtm).set_ivars(Ivars {
            settings: RefCell::new(Settings::load_user()),
            font_label: RefCell::new(None),
            ci: ci::CiSettings::new(mtm),
            agents: agents::AgentsSettings::new(mtm),
            agent: agent::AgentSettings::new(mtm),
            servers: servers::ServersSettings::new(mtm),
            #[cfg(feature = "dictation")]
            dictation: dictation::DictationSettings::new(mtm),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Changes the user's settings as they are on disk now: a write sends every key, and others write some too.
    fn store(&self, change: impl FnOnce(&mut Settings) -> std::io::Result<()>) {
        let mut settings = self.ivars().settings.borrow_mut();
        *settings = Settings::load_user();
        if let Err(error) = change(&mut settings) {
            crate::diag::report("settings", error);
        }
    }
}

/// Brings the settings window to the front, building it the first time.
pub fn show(mtm: MainThreadMarker) {
    WINDOW.with(|cell| {
        let mut cell = cell.borrow_mut();
        let (window, controller) = cell.get_or_insert_with(|| build(mtm));
        controller.ivars().ci.reload();
        controller.ivars().agents.reload();
        controller.ivars().agent.reload();
        controller.ivars().servers.reload();
        #[cfg(feature = "dictation")]
        controller.ivars().dictation.reload();
        window.makeKeyAndOrderFront(None);
    });
}

/// Brings the settings window to the front on its CI tab.
pub fn show_ci(mtm: MainThreadMarker) {
    show_tab(mtm, CI_TAB);
}

/// Brings the settings window to the front on its own agent's tab.
pub fn show_agent(mtm: MainThreadMarker) {
    show_tab(mtm, AGENT_TAB);
}

/// Brings the settings window to the front on its language servers tab, for the self-test.
#[cfg(feature = "selftest")]
pub fn show_servers(mtm: MainThreadMarker) {
    show_tab(mtm, SERVERS_TAB);
}

/// Where the built-in languages' servers are, as a survey found them; an open settings window shows it.
fn servers_found(lines: Vec<(String, String)>) {
    if let Some(servers) = servers_tab() {
        servers.found(lines);
    }
}

/// The language servers tab of the settings window, once it was built.
pub fn servers_tab() -> Option<Retained<servers::ServersSettings>> {
    WINDOW.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().servers.clone())
    })
}

#[cfg(feature = "selftest")]
pub use servers::selftest as servers_selftest;

/// Brings the settings window to the front on its Agents tab.
pub fn show_agents(mtm: MainThreadMarker) {
    show_tab(mtm, AGENTS_TAB);
}

/// The agents changed, such as an installation that finished; an open settings window shows them again.
pub fn agents_changed() {
    if let Some(agents) = agents_tab_now() {
        agents.reload();
    }
}

/// The Agents tab of the settings window, once it was built.
fn agents_tab_now() -> Option<Retained<agents::AgentsSettings>> {
    WINDOW.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().agents.clone())
    })
}

/// What the Agents tab shows, line by line, for the self-test.
#[cfg(feature = "selftest")]
pub fn agents_lines() -> Vec<String> {
    agents_tab_now().map(|agents| agents.lines()).unwrap_or_default()
}

/// Opens the ACP registry's window as the Agents tab's button does; the window and the ids it lists, for the
/// self-test.
#[cfg(feature = "selftest")]
pub fn open_register() -> Option<(Retained<objc2_app_kit::NSPanel>, Vec<String>)> {
    agents::open_register(&*agents_tab_now()?);
    agents::register_window()
}

#[cfg(feature = "selftest")]
pub use agents::register_window;

fn show_tab(mtm: MainThreadMarker, tab: isize) {
    show(mtm);
    WINDOW.with(|cell| {
        let tabs = cell
            .borrow()
            .as_ref()
            .and_then(|(window, _)| window.contentViewController())
            .and_then(|c| c.downcast::<NSTabViewController>().ok());
        if let Some(tabs) = tabs {
            tabs.setSelectedTabViewItemIndex(tab);
        }
    });
}

/// Something else changed the CI settings; an open settings window shows them again.
pub fn ci_changed() {
    WINDOW.with(|cell| {
        if let Some((_, controller)) = cell.borrow().as_ref() {
            controller.ivars().ci.reload();
        }
    });
}

/// The settings window, if it was opened, for the self-test.
#[cfg(feature = "selftest")]
pub fn window() -> Option<Retained<NSWindow>> {
    WINDOW.with(|cell| cell.borrow().as_ref().map(|(window, _)| window.clone()))
}

/// The CI tab of the open settings window, for the self-test.
#[cfg(feature = "selftest")]
pub fn ci_tab() -> Option<Retained<ci::CiSettings>> {
    WINDOW.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().ci.clone())
    })
}

#[cfg(feature = "selftest")]
pub use ci::{List as CiList, selftest as ci_selftest};

/// The Agent tab of the settings window, once it was built.
fn agent_tab_now() -> Option<Retained<agent::AgentSettings>> {
    WINDOW.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().agent.clone())
    })
}

/// The Agent tab of the open settings window, for the self-test.
#[cfg(feature = "selftest")]
pub fn agent_tab() -> Option<Retained<agent::AgentSettings>> {
    WINDOW.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().agent.clone())
    })
}

#[cfg(feature = "selftest")]
pub use agent::{AgentSettings, List as AgentList, selftest as agent_selftest};

fn build(mtm: MainThreadMarker) -> (Retained<NSWindow>, Retained<SettingsController>) {
    let controller = SettingsController::new(mtm);
    let tabs = NSTabViewController::new(mtm);
    tabs.setTabStyle(NSTabViewControllerTabStyle::Toolbar);
    let general = NSViewController::new(mtm);
    let view = general_view(mtm, &controller);
    general.setPreferredContentSize(view.fittingSize());
    general.setView(&view);
    general.setTitle(Some(&ns(t("Allgemein"))));
    let item = NSTabViewItem::tabViewItemWithViewController(&general);
    item.setLabel(&ns(t("Allgemein")));
    item.setImage(symbol("gearshape", t("Allgemein")).as_deref());
    tabs.addTabViewItem(&item);
    let ci = NSViewController::new(mtm);
    let view = controller.ivars().ci.view();
    ci.setPreferredContentSize(view.fittingSize());
    ci.setView(&view);
    ci.setTitle(Some(&ns(t("CI"))));
    let item = NSTabViewItem::tabViewItemWithViewController(&ci);
    item.setLabel(&ns(t("CI")));
    item.setImage(symbol("checkmark.seal", t("CI")).as_deref());
    tabs.addTabViewItem(&item);
    let agents = NSViewController::new(mtm);
    let view = controller.ivars().agents.view();
    agents.setPreferredContentSize(view.frame().size);
    agents.setView(&view);
    agents.setTitle(Some(&ns(t("Agenten"))));
    let item = NSTabViewItem::tabViewItemWithViewController(&agents);
    item.setLabel(&ns(t("Agenten")));
    item.setImage(symbol("person.2", t("Agenten")).as_deref());
    tabs.addTabViewItem(&item);
    let agent = NSViewController::new(mtm);
    let view = controller.ivars().agent.view();
    agent.setPreferredContentSize(view.frame().size);
    agent.setView(&view);
    agent.setTitle(Some(&ns(t("Eigener Agent"))));
    let item = NSTabViewItem::tabViewItemWithViewController(&agent);
    item.setLabel(&ns(t("Eigener Agent")));
    item.setImage(symbol("sparkles", t("Eigener Agent")).as_deref());
    tabs.addTabViewItem(&item);
    add_tab(
        &tabs,
        &controller.ivars().servers.view(),
        t("Sprachserver"),
        "curlybraces",
    );
    #[cfg(feature = "dictation")]
    add_tab(&tabs, &controller.ivars().dictation.view(), t("Diktat"), "mic");
    let window = NSWindow::windowWithContentViewController(&tabs);
    window.setStyleMask(NSWindowStyleMask::Titled | NSWindowStyleMask::Closable);
    // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back; the thread-local owns it.
    unsafe { window.setReleasedWhenClosed(false) };
    window.center();
    (window, controller)
}

/// A tab of `view`, named `title` with the symbol `image`.
fn add_tab(tabs: &NSTabViewController, view: &NSView, title: &str, image: &str) {
    let page = NSViewController::new(tabs.mtm());
    page.setPreferredContentSize(view.fittingSize());
    page.setView(view);
    page.setTitle(Some(&ns(title)));
    let item = NSTabViewItem::tabViewItemWithViewController(&page);
    item.setLabel(&ns(title));
    item.setImage(symbol(image, title).as_deref());
    tabs.addTabViewItem(&item);
}

/// The dictation tab of the open settings window, for the self-test.
#[cfg(all(feature = "selftest", feature = "dictation"))]
pub fn dictation_keys() -> Vec<String> {
    WINDOW.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| dictation::key_lines(&controller.ivars().dictation))
            .unwrap_or_default()
    })
}

/// Brings the settings window to the front on its dictation tab, for the self-test.
#[cfg(all(feature = "selftest", feature = "dictation"))]
pub fn show_dictation(mtm: MainThreadMarker) {
    show(mtm);
    WINDOW.with(|cell| {
        let tabs = cell
            .borrow()
            .as_ref()
            .and_then(|(window, _)| window.contentViewController())
            .and_then(|c| c.downcast::<NSTabViewController>().ok());
        if let Some(tabs) = tabs {
            tabs.setSelectedTabViewItemIndex(DICTATION_TAB);
        }
    });
}

fn markdown_row(mtm: MainThreadMarker, controller: &SettingsController, settings: &Settings) -> Vec<Retained<NSView>> {
    vec![
        label(mtm, t("Markdown:")).into_super().into_super(),
        checkbox(
            controller,
            t("Markdown-Vorschau automatisch daneben öffnen"),
            sel!(ideToggleMarkdownPreview:),
            settings.get(&keys::MARKDOWN_PREVIEW),
        )
        .into_super()
        .into_super(),
    ]
}

fn general_view(mtm: MainThreadMarker, controller: &SettingsController) -> Retained<NSView> {
    let settings = controller.ivars().settings.borrow();
    let reopen = checkbox(
        controller,
        t("Beim Start die zuletzt offenen Ordner öffnen"),
        sel!(ideToggleReopen:),
        settings.get(&keys::REOPEN_PROJECTS),
    );
    let previews = checkbox(
        controller,
        t("Ein Klick auf eine Datei öffnet sie in einem Vorschau-Tab"),
        sel!(ideTogglePreviewTabs:),
        settings.get(&keys::PREVIEW_TABS),
    );

    let format_on_save = checkbox(
        controller,
        t("Beim Sichern formatieren"),
        sel!(ideToggleFormatOnSave:),
        settings.get(&keys::FORMAT_ON_SAVE),
    );
    format_on_save.setToolTip(Some(&ns(t(
        "Lässt den Sprachserver eine Datei formatieren, bevor ⌘S sie sichert.",
    ))));

    let size_title = label(mtm, t("Schriftgröße:"));
    let size_row = font_size_row(mtm, controller, settings.get(&keys::FONT_SIZE), &size_title);
    let engine_title = label(mtm, t("Suchmaschine:"));
    let engine = search_engine_popup(mtm, controller, settings.get(&keys::SEARCH_ENGINE), &engine_title);

    let mut rows: Vec<Vec<Retained<NSView>>> = vec![
        vec![
            label(mtm, t("Start:")).into_super().into_super(),
            reopen.into_super().into_super(),
        ],
        vec![
            label(mtm, t("Tabs:")).into_super().into_super(),
            previews.into_super().into_super(),
        ],
        vec![size_title.into_super().into_super(), size_row],
        vec![
            label(mtm, t("Code:")).into_super().into_super(),
            format_on_save.into_super().into_super(),
        ],
        markdown_row(mtm, controller, &settings),
        vec![
            label(mtm, t("Terminal:")).into_super().into_super(),
            checkbox(
                controller,
                t("Wahltaste als Meta-Taste"),
                sel!(ideToggleOptionAsMeta:),
                settings.get(&keys::OPTION_AS_META),
            )
            .into_super()
            .into_super(),
        ],
        vec![engine_title.into_super().into_super(), engine],
    ];
    for (path, locked) in settings.problems() {
        let reason = match locked {
            Locked::Damaged => t("Die Datei ist beschädigt und bleibt unverändert, bis du sie reparierst."),
            Locked::Newer => t("Die Datei stammt von einer neueren Version und bleibt unverändert."),
        };
        let warning =
            NSTextField::wrappingLabelWithString(&ns(&tf("{}: {}", &[&path.display().to_string(), reason])), mtm);
        warning.setTextColor(Some(&NSColor::systemRedColor()));
        warning.setPreferredMaxLayoutWidth(360.0);
        rows.push(vec![
            label(mtm, "").into_super().into_super(),
            warning.into_super().into_super(),
        ]);
    }
    let rows: Vec<Retained<objc2_foundation::NSArray<NSView>>> = rows
        .iter()
        .map(|row| objc2_foundation::NSArray::from_retained_slice(row))
        .collect();
    let grid = NSGridView::gridViewWithViews(&objc2_foundation::NSArray::from_retained_slice(&rows), mtm);
    grid.setRowSpacing(12.0);
    grid.setColumnSpacing(8.0);
    if let Some(column) = grid.columnAtIndex(0).into() {
        column.setXPlacement(objc2_app_kit::NSGridCellPlacement::Trailing);
    }
    let view = NSView::new(mtm);
    view.addSubview(&grid);
    grid.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        grid.leadingAnchor()
            .constraintEqualToAnchor_constant(&view.leadingAnchor(), 30.0),
        grid.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&view.trailingAnchor(), -30.0),
        grid.topAnchor()
            .constraintEqualToAnchor_constant(&view.topAnchor(), 24.0),
        grid.bottomAnchor()
            .constraintEqualToAnchor_constant(&view.bottomAnchor(), -24.0),
        view.widthAnchor().constraintGreaterThanOrEqualToConstant(460.0),
    ] {
        constraint.setActive(true);
    }
    view
}

/// The font size and the stepper that changes it, which VoiceOver names by `title`.
fn font_size_row(
    mtm: MainThreadMarker,
    controller: &SettingsController,
    size: f64,
    title: &NSTextField,
) -> Retained<NSView> {
    let size_label = label(mtm, &font_size_text(size));
    let stepper = NSStepper::new(mtm);
    crate::util::set_title_element(&stepper, title);
    stepper.setMinValue(9.0);
    stepper.setMaxValue(32.0);
    stepper.setIncrement(1.0);
    stepper.setDoubleValue(size);
    set_action(&stepper, controller, sel!(ideFontSizeChanged:));
    let size_row = NSView::new(mtm);
    size_row.addSubview(&size_label);
    size_row.addSubview(&stepper);
    size_label.setTranslatesAutoresizingMaskIntoConstraints(false);
    stepper.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        size_label
            .leadingAnchor()
            .constraintEqualToAnchor(&size_row.leadingAnchor()),
        size_label
            .centerYAnchor()
            .constraintEqualToAnchor(&size_row.centerYAnchor()),
        stepper
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&size_label.trailingAnchor(), 8.0),
        stepper
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor(&size_row.trailingAnchor()),
        stepper.topAnchor().constraintEqualToAnchor(&size_row.topAnchor()),
        stepper.bottomAnchor().constraintEqualToAnchor(&size_row.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    *controller.ivars().font_label.borrow_mut() = Some(size_label);
    size_row
}

/// A checkbox titled `title` that sends `action` to `controller`, ticked when `on`.
fn checkbox(controller: &SettingsController, title: &str, action: objc2::runtime::Sel, on: bool) -> Retained<NSButton> {
    // SAFETY: The checkbox sends its action to the controller, which answers it, set right below.
    let checkbox = unsafe { NSButton::checkboxWithTitle_target_action(&ns(title), None, None, controller.mtm()) };
    set_action(&checkbox, controller, action);
    checkbox.setState(if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    checkbox
}

/// Where the browser's address field searches, which the user picks by name; VoiceOver names it by `title`.
fn search_engine_popup(
    mtm: MainThreadMarker,
    controller: &SettingsController,
    current: SearchEngine,
    title: &NSTextField,
) -> Retained<NSView> {
    let popup = NSPopUpButton::new(mtm);
    crate::util::set_title_element(&popup, title);
    for engine in SearchEngine::ALL {
        popup.addItemWithTitle(&ns(engine.name()));
    }
    let index = SearchEngine::ALL.iter().position(|e| *e == current).unwrap_or(0);
    popup.selectItemAtIndex(isize::try_from(index).unwrap_or(0));
    set_action(&popup, controller, sel!(ideSearchEngineChanged:));
    popup.into_super().into_super().into_super()
}

fn font_size_text(size: f64) -> String {
    tf("{} pt", &[&format!("{size:.0}")])
}
