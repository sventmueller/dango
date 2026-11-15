//! The settings window (⌘,), with a tab per area in its toolbar.

mod agent;
mod agents;
mod ci;
mod commit_message;
#[cfg(feature = "dictation")]
pub(crate) mod dictation;
mod editor;
mod form;
mod list_bar;
mod predict;
mod servers;
mod stored_rules;
mod sync;

use std::cell::RefCell;

use ide_core::settings::{Locked, SearchEngine, Settings, keys};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSFontAttributeName, NSPopUpButton, NSStepper,
    NSStringDrawing, NSTabViewController, NSTabViewControllerTabStyle, NSTabViewItem, NSTextField, NSView,
    NSViewController, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDictionary, NSObjectProtocol, NSString};
use splat_i18n::{t, tf};

use self::form::Row;
use crate::util::{activate, guarded, label, ns, set_action, symbol};

pub(crate) use stored_rules::remember as remember_claude_rules;

/// Where the CI tab sits in the toolbar.
const CI_TAB: isize = 1;
/// Where the Agents tab sits in the toolbar.
const AGENTS_TAB: isize = 2;
/// Where the own agent's tab sits in the toolbar.
const AGENT_TAB: isize = 3;
/// Where the dictation tab sits in the toolbar.
#[cfg(feature = "dictation")]
const DICTATION_TAB: isize = 5;

pub(crate) struct Ivars {
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
    pub(crate) struct SettingsController;

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

        #[unsafe(method(ideToggleShowIgnoredFiles:))]
        fn toggle_show_ignored_files(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::SHOW_IGNORED_FILES, &on));
                crate::window::show_ignored_files(on);
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

        #[unsafe(method(ideToggleEslintFixOnSave:))]
        fn toggle_eslint_fix(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::ESLINT_FIX_ON_SAVE, &on));
            });
        }

        #[unsafe(method(ideToggleEmmet:))]
        fn toggle_emmet(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                self.store(|settings| settings.set(&keys::EMMET, &on));
                ide_ui_lsp::servers_changed();
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

/// The settings window and the parts of it on view.
#[derive(Default)]
pub(crate) struct SettingsState {
    settings_window: SettingsWindow,
    agent_registry: agents::AgentRegistryWindow,
    commit_message_settings: commit_message::CommitMessageShown,
    editor_settings: editor::EditorSettingsShown,
    prediction: predict::PredictionShown,
    /// The object the sync's switch in the General tab sends its actions to.
    sync_target: RefCell<Option<Retained<objc2::runtime::NSObject>>>,
}

/// The settings window while it exists, with its controller.
#[derive(Default)]
pub(crate) struct SettingsWindow {
    window: RefCell<Option<(Retained<NSWindow>, Retained<SettingsController>)>>,
}

fn with_window<R>(f: impl FnOnce(&RefCell<Option<(Retained<NSWindow>, Retained<SettingsController>)>>) -> R) -> R {
    f(&crate::context::app().settings().settings_window.window)
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
pub(crate) fn show(mtm: MainThreadMarker) {
    with_window(|cell| {
        let mut cell = cell.borrow_mut();
        let (window, controller) = cell.get_or_insert_with(|| build(mtm));
        controller.ivars().ci.reload();
        controller.ivars().agents.reload();
        controller.ivars().agent.reload();
        controller.ivars().servers.reload();
        #[cfg(feature = "dictation")]
        controller.ivars().dictation.reload();
        editor::reload();
        window.makeKeyAndOrderFront(None);
    });
    fit(AGENTS_TAB);
    #[cfg(feature = "dictation")]
    fit(DICTATION_TAB);
}

/// Where the tab of edit prediction sits in the toolbar: the last.
const PREDICTION_TAB: isize = if cfg!(feature = "dictation") { 6 } else { 5 };

/// Brings the settings window to the front on the tab of edit prediction.
pub(crate) fn show_prediction(mtm: MainThreadMarker) {
    show_tab(mtm, PREDICTION_TAB);
}

pub(crate) use predict::prediction_changed;
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

/// Brings the settings window to the front on its CI tab.
pub(crate) fn show_ci(mtm: MainThreadMarker) {
    show_tab(mtm, CI_TAB);
}

/// Brings the settings window to the front on its own agent's tab.
pub(crate) fn show_agent(mtm: MainThreadMarker) {
    show_tab(mtm, AGENT_TAB);
}

/// Where the built-in languages' servers are, as a survey found them; an open settings window shows it.
fn servers_found(lines: Vec<(String, String)>) {
    if let Some(servers) = servers_tab() {
        servers.found(lines);
    }
}

/// The language servers tab of the settings window, once it was built.
pub(crate) fn servers_tab() -> Option<Retained<servers::ServersSettings>> {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().servers.clone())
    })
}

/// Brings the settings window to the front on its Agents tab.
pub(crate) fn show_agents(mtm: MainThreadMarker) {
    show_tab(mtm, AGENTS_TAB);
}

/// The agents changed, such as an installation that finished; an open settings window shows them again.
pub(crate) fn agents_changed() {
    if let Some(mtm) = MainThreadMarker::new() {
        crate::window::refresh_agent_choices(&crate::context::AppContext::get(mtm));
    }
    if let Some(agents) = agents_tab_now() {
        agents.reload();
        fit(AGENTS_TAB);
    }
}

/// Sizes the tab at `tab` to what it shows now, and the window with it while that tab is open; does nothing while
/// the window is being built or shown, which measures every tab anyway.
fn fit(tab: isize) {
    with_window(|cell| {
        let Ok(cell) = cell.try_borrow() else { return };
        let Some((window, _)) = cell.as_ref() else { return };
        let Some(tabs) = window
            .contentViewController()
            .and_then(|c| c.downcast::<NSTabViewController>().ok())
        else {
            return;
        };
        let page = usize::try_from(tab)
            .ok()
            .and_then(|n| tabs.tabViewItems().iter().nth(n))
            .and_then(|item| item.viewController(window.mtm()));
        let Some(page) = page else { return };
        let size = page.view().fittingSize();
        page.setPreferredContentSize(size);
        let Some(content) = window.contentView() else { return };
        let now = content.frame().size;
        if tabs.selectedTabViewItemIndex() != tab || (now.width == size.width && now.height == size.height) {
            return;
        }
        let mut frame = window.frame();
        frame.size.width += size.width - now.width;
        frame.size.height += size.height - now.height;
        frame.origin.y -= size.height - now.height;
        window.setFrame_display_animate(frame, true, window.isVisible());
    });
}

/// The Agents tab of the settings window, once it was built.
fn agents_tab_now() -> Option<Retained<agents::AgentsSettings>> {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().agents.clone())
    })
}

fn show_tab(mtm: MainThreadMarker, tab: isize) {
    show(mtm);
    with_window(|cell| {
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
pub(crate) fn ci_changed() {
    with_window(|cell| {
        if let Some((_, controller)) = cell.borrow().as_ref() {
            controller.ivars().ci.reload();
        }
    });
}

/// The settings window, if it was opened.
pub(crate) fn window() -> Option<Retained<NSWindow>> {
    with_window(|cell| cell.borrow().as_ref().map(|(window, _)| window.clone()))
}

/// The Agent tab of the settings window, once it was built.
fn agent_tab_now() -> Option<Retained<agent::AgentSettings>> {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().agent.clone())
    })
}

fn build(mtm: MainThreadMarker) -> (Retained<NSWindow>, Retained<SettingsController>) {
    let controller = SettingsController::new(mtm);
    let tabs = NSTabViewController::new(mtm);
    tabs.setTabStyle(NSTabViewControllerTabStyle::Toolbar);
    let mut pages = vec![
        Page::of(general_view(mtm, &controller), t("Allgemein"), "gearshape"),
        Page::of(controller.ivars().ci.view(), t("CI"), "checkmark.seal"),
        Page::of(controller.ivars().agents.view(), t("Agenten"), "person.2"),
        Page {
            sized: true,
            ..Page::of(controller.ivars().agent.view(), t("Eigener Agent"), "sparkles")
        },
        Page::of(controller.ivars().servers.view(), t("Sprachserver"), "curlybraces"),
    ];
    #[cfg(feature = "dictation")]
    pages.push(Page::of(controller.ivars().dictation.view(), t("Diktat"), "mic"));
    pages.push(Page::of(predict::view(mtm), t("Vorhersage"), "text.cursor"));
    let width = toolbar_width(&pages.iter().map(|page| page.title).collect::<Vec<_>>());
    for page in &pages {
        add_tab(&tabs, page, width);
    }
    let window = NSWindow::windowWithContentViewController(&tabs);
    window.setStyleMask(NSWindowStyleMask::Titled | NSWindowStyleMask::Closable);
    // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back; the thread-local owns it.
    unsafe { window.setReleasedWhenClosed(false) };
    window.center();
    (window, controller)
}

/// A tab of the settings window before it is added.
struct Page {
    view: Retained<NSView>,
    title: &'static str,
    image: &'static str,
    /// Whether the view has the size of its frame rather than one its constraints give.
    sized: bool,
}

impl Page {
    fn of(view: Retained<NSView>, title: &'static str, image: &'static str) -> Page {
        Page {
            view,
            title,
            image,
            sized: false,
        }
    }
}

/// How much wider than its label a toolbar item of the settings style is, and how narrow it gets at least; read from
/// macOS 26's layout with a point to spare.
const ITEM_PADDING: f64 = 14.0;
const ITEM_MIN: f64 = 56.0;
/// The toolbar's margin at each end, which macOS keeps at about 7 points, with room to spare.
const TOOLBAR_MARGIN: f64 = 12.0;

/// The content width at which the toolbar shows an item for each of `titles` without moving any into its overflow
/// menu.
fn toolbar_width(titles: &[&str]) -> f64 {
    let font = ide_ui_common::style::small_system();
    let attributes = NSDictionary::<NSString, AnyObject>::from_retained_objects(
        &[crate::util::constant!(NSFontAttributeName)],
        &[font.into()],
    );
    let items: f64 = titles
        .iter()
        // SAFETY: The attributes hold a font, as the method expects.
        .map(|title| unsafe { ns(title).sizeWithAttributes(Some(&attributes)) }.width)
        .map(|width| (width + ITEM_PADDING).max(ITEM_MIN))
        .sum();
    items + 2.0 * TOOLBAR_MARGIN
}

/// Adds `page` as a tab whose content is at least `width` wide; a narrower view keeps its size, centred.
fn add_tab(tabs: &NSTabViewController, page: &Page, width: f64) {
    let mtm = tabs.mtm();
    let view = &page.view;
    let size = view.frame().size;
    let content = NSView::new(mtm);
    content.addSubview(view);
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    let mut constraints = vec![
        view.topAnchor().constraintEqualToAnchor(&content.topAnchor()),
        view.bottomAnchor().constraintEqualToAnchor(&content.bottomAnchor()),
        view.centerXAnchor().constraintEqualToAnchor(&content.centerXAnchor()),
        view.leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor(&content.leadingAnchor()),
        content.widthAnchor().constraintGreaterThanOrEqualToConstant(width),
    ];
    if page.sized {
        constraints.push(view.widthAnchor().constraintEqualToConstant(size.width));
        constraints.push(view.heightAnchor().constraintEqualToConstant(size.height));
    }
    activate(constraints);
    let controller = NSViewController::new(mtm);
    controller.setPreferredContentSize(content.fittingSize());
    controller.setView(&content);
    controller.setTitle(Some(&ns(page.title)));
    let item = NSTabViewItem::tabViewItemWithViewController(&controller);
    item.setLabel(&ns(page.title));
    item.setImage(symbol(page.image, page.title).as_deref());
    tabs.addTabViewItem(&item);
}

/// The dictation tab of the open settings window, for the self-test.
#[cfg(all(feature = "selftest", feature = "dictation"))]
pub(crate) fn dictation_keys() -> Vec<String> {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| dictation::key_lines(&controller.ivars().dictation))
            .unwrap_or_default()
    })
}

/// Whether the open dictation tab offers Lowspeak's choices and shows a missing model's download, for the self-test.
#[cfg(all(feature = "selftest", feature = "dictation"))]
pub(crate) fn dictation_rows() -> (bool, bool) {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| dictation::rows_shown(&controller.ivars().dictation))
            .unwrap_or_default()
    })
}

/// Brings the settings window to the front on its dictation tab, for the self-test.
#[cfg(all(feature = "selftest", feature = "dictation"))]
pub(crate) fn show_dictation(mtm: MainThreadMarker) {
    show(mtm);
    with_window(|cell| {
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

fn general_view(mtm: MainThreadMarker, controller: &SettingsController) -> Retained<NSView> {
    let settings = controller.ivars().settings.borrow();
    let toggle = |title: &str, action: objc2::runtime::Sel, on: bool| checkbox(controller, title, action, on);
    let reopen = toggle(
        t("Beim Start die zuletzt offenen Ordner öffnen"),
        sel!(ideToggleReopen:),
        settings.get(&keys::REOPEN_PROJECTS),
    );
    let previews = toggle(
        t("Ein Klick auf eine Datei öffnet sie in einem Vorschau-Tab"),
        sel!(ideTogglePreviewTabs:),
        settings.get(&keys::PREVIEW_TABS),
    );
    let files = toggle(
        t("Ignorierte und versteckte Dateien anzeigen"),
        sel!(ideToggleShowIgnoredFiles:),
        settings.get(&keys::SHOW_IGNORED_FILES),
    );
    files.setToolTip(Some(&ns(t(
        "Zeigt im Dateibaum auch, was .gitignore ausschließt oder macOS verbirgt, abgeblendet.",
    ))));
    let format_on_save = toggle(
        t("Beim Sichern formatieren"),
        sel!(ideToggleFormatOnSave:),
        settings.get(&keys::FORMAT_ON_SAVE),
    );
    format_on_save.setToolTip(Some(&ns(t(
        "Lässt den Sprachserver eine Datei formatieren, bevor ⌘S sie sichert.",
    ))));
    let eslint = toggle(
        t("ESLint-Probleme beim Sichern beheben"),
        sel!(ideToggleEslintFixOnSave:),
        settings.get(&keys::ESLINT_FIX_ON_SAVE),
    );
    let emmet = toggle(
        t("Emmet-Kürzel in der Vervollständigung"),
        sel!(ideToggleEmmet:),
        settings.get(&keys::EMMET),
    );
    let markdown = toggle(
        t("Markdown-Vorschau automatisch daneben öffnen"),
        sel!(ideToggleMarkdownPreview:),
        settings.get(&keys::MARKDOWN_PREVIEW),
    );
    let meta = toggle(
        t("Wahltaste als Meta-Taste"),
        sel!(ideToggleOptionAsMeta:),
        settings.get(&keys::OPTION_AS_META),
    );
    let engine = search_engine_popup(mtm, controller, settings.get(&keys::SEARCH_ENGINE));
    let mut rows = vec![
        Row::new(mtm, t("Start:"), &reopen),
        Row::new(mtm, t("Tabs:"), &previews),
        Row::new(mtm, t("Dateien:"), &files),
        font_size_row(mtm, controller, settings.get(&keys::FONT_SIZE)),
        Row::new(mtm, t("Code:"), &format_on_save),
        Row::under(&eslint),
        Row::under(&emmet),
        Row::new(mtm, t("Markdown:"), &markdown),
        Row::new(mtm, t("Terminal:"), &meta),
        Row::new(mtm, t("Suchmaschine:"), &engine),
    ];
    rows.extend(editor::rows(mtm));
    rows.extend(sync::rows(
        mtm,
        &crate::context::AppContext::get(mtm).settings().sync_target,
    ));
    for (path, locked) in settings.problems() {
        let reason = match locked {
            Locked::Damaged => t("Die Datei ist beschädigt und bleibt unverändert, bis du sie reparierst."),
            Locked::Newer => t("Die Datei stammt von einer neueren Version und bleibt unverändert."),
            Locked::Unreadable => t("Die Datei lässt sich nicht lesen und bleibt unverändert."),
        };
        let warning = form::value(mtm, &tf("{}: {}", &[&path.display().to_string(), reason]));
        warning.setTextColor(Some(&NSColor::systemRedColor()));
        rows.push(Row::under(&warning));
    }
    form::page(mtm, &form::grid(mtm, &rows))
}

/// The font size and the stepper that changes it.
fn font_size_row(mtm: MainThreadMarker, controller: &SettingsController, size: f64) -> Row {
    let size_label = label(mtm, &font_size_text(size));
    let stepper = NSStepper::new(mtm);
    stepper.setMinValue(9.0);
    stepper.setMaxValue(32.0);
    stepper.setIncrement(1.0);
    stepper.setDoubleValue(size);
    set_action(&stepper, controller, sel!(ideFontSizeChanged:));
    let line = form::line(mtm, &[&size_label, &stepper]);
    let row = Row::group(mtm, t("Schriftgröße:"), &line, &size_label);
    if let Some(caption) = row.caption() {
        crate::util::set_title_element(&stepper, caption);
    }
    *controller.ivars().font_label.borrow_mut() = Some(size_label);
    row
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

/// Where the browser's address field searches, which the user picks by name.
fn search_engine_popup(
    mtm: MainThreadMarker,
    controller: &SettingsController,
    current: SearchEngine,
) -> Retained<NSView> {
    let popup = NSPopUpButton::new(mtm);
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
