//! The `settings-general` scenario: a damaged settings file is named on the General tab and left alone; each switch,
//! the font size's stepper and the search engine's menu on that tab store what they show at once; the Agents tab
//! lists the agents again when they change while it is open, in a window as large as the list; a change of the CI
//! settings from elsewhere reaches the open window; the own agent keeps and forgets its rules, endpoints and keys and
//! starts on the service that has a key.

use std::time::Duration;

use ide_core::settings::{Key, SearchEngine, Settings, keys};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, MainThreadOnly, Message, sel};
use objc2_app_kit::{
    NSApplication, NSButton, NSControl, NSControlStateValueOn, NSPopUpButton, NSStepper, NSTabViewController,
    NSTextField, NSView,
};

use super::{check, finish};
use crate::util::main_after;

const NAME: &str = "settings-general";

const DAMAGED: &str = "{ \"reopenProjects\": tru";

fn settings_file() -> std::path::PathBuf {
    ide_core::paths::data_dir().join("settings.json")
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let file = settings_file();
    let before = std::fs::read(&file).ok();
    let _ = std::fs::create_dir_all(ide_core::paths::data_dir());
    let _ = std::fs::write(&file, DAMAGED);
    crate::settings::show(mtm);
    damaged(before);
    main_after(Duration::from_millis(400), general);
}

/// The General tab of a damaged settings file says so, and its switches leave the file as it is.
fn damaged(before: Option<Vec<u8>>) {
    let file = settings_file();
    let mut labels = Vec::new();
    if let Some(view) = general_view() {
        collect_labels(&view, &mut labels);
    }
    let warning = splat_i18n::tf(
        "{}: {}",
        &[
            &file.display().to_string(),
            splat_i18n::t("Die Datei ist beschädigt und bleibt unverändert, bis du sie reparierst."),
        ],
    );
    if let Some(button) = general_view()
        .and_then(|view| control_for(&view, sel!(ideToggleReopen:)))
        .and_then(|c| c.downcast::<NSButton>().ok())
    {
        button.setState(if button.state() == NSControlStateValueOn {
            0
        } else {
            NSControlStateValueOn
        });
        act(&button);
    }
    let kept = std::fs::read_to_string(&file).unwrap_or_default();
    check(
        "a damaged settings file is named on the General tab, and a switch leaves it as it is",
        labels.contains(&warning) && kept == DAMAGED,
        format!("{labels:?}, file {kept:?}"),
    );
    match before {
        Some(bytes) => {
            let _ = std::fs::write(&file, bytes);
        }
        None => {
            let _ = std::fs::remove_file(&file);
        }
    }
}

/// The General tab's view.
fn general_view() -> Option<Retained<NSView>> {
    let window = crate::settings::window()?;
    let tabs = window.contentViewController()?.downcast::<NSTabViewController>().ok()?;
    let mtm = window.mtm();
    tabs.tabViewItems()
        .iter()
        .next()?
        .viewController(mtm)
        .map(|page| page.view())
}

fn controls(view: &NSView, found: &mut Vec<Retained<NSControl>>) {
    if let Some(control) = view.downcast_ref::<NSControl>() {
        found.push(control.retain());
    }
    for child in view.subviews().iter() {
        controls(&child, found);
    }
}

fn control_for(view: &NSView, action: Sel) -> Option<Retained<NSControl>> {
    let mut found = Vec::new();
    controls(view, &mut found);
    found.into_iter().find(|control| control.action() == Some(action))
}

/// Sends `control`'s action as a click on it does.
fn act(control: &NSControl) {
    let app = NSApplication::sharedApplication(control.mtm());
    let target = control.target();
    if let Some(action) = control.action() {
        // SAFETY: The control's own action and target, with the control as the sender.
        unsafe { app.sendAction_to_from(action, target.as_deref(), Some(control)) };
    }
}

fn general() {
    let Some(view) = general_view() else {
        check("the settings window has its General tab", false, "");
        return finish(NAME);
    };
    let switches: [(Sel, Key<bool>, &str); 6] = [
        (sel!(ideToggleReopen:), keys::REOPEN_PROJECTS, "reopen"),
        (sel!(ideTogglePreviewTabs:), keys::PREVIEW_TABS, "preview tabs"),
        (
            sel!(ideToggleMarkdownPreview:),
            keys::MARKDOWN_PREVIEW,
            "Markdown preview",
        ),
        (
            sel!(ideToggleShowIgnoredFiles:),
            keys::SHOW_IGNORED_FILES,
            "ignored files",
        ),
        (sel!(ideToggleFormatOnSave:), keys::FORMAT_ON_SAVE, "format on save"),
        (sel!(ideToggleOptionAsMeta:), keys::OPTION_AS_META, "Option as Meta"),
    ];
    let mut wrong = Vec::new();
    for (action, key, name) in switches {
        let Some(button) = control_for(&view, action).and_then(|c| c.downcast::<NSButton>().ok()) else {
            wrong.push(format!("{name}: no switch"));
            continue;
        };
        for _ in 0..2 {
            button.setState(if button.state() == NSControlStateValueOn {
                0
            } else {
                NSControlStateValueOn
            });
            act(&button);
            let shown = button.state() == NSControlStateValueOn;
            let stored = Settings::load_user().get(&key);
            if stored != shown {
                wrong.push(format!("{name}: shows {shown}, stored {stored}"));
            }
        }
    }
    check(
        "each switch of the General tab stores what it shows, on and off",
        wrong.is_empty(),
        wrong.join("; "),
    );
    font_and_engine(&view);
    main_after(Duration::from_millis(300), agents);
}

fn font_and_engine(view: &NSView) {
    let stepper = control_for(view, sel!(ideFontSizeChanged:)).and_then(|c| c.downcast::<NSStepper>().ok());
    let size = stepper.as_ref().map_or(0.0, |stepper| {
        stepper.setDoubleValue(17.4);
        act(stepper);
        Settings::load_user().get(&keys::FONT_SIZE)
    });
    let mut labels = Vec::new();
    collect_labels(view, &mut labels);
    check(
        "the font size's stepper stores a whole size and its label shows it",
        (size - 17.0).abs() < f64::EPSILON && labels.iter().any(|l| *l == splat_i18n::tf("{} pt", &["17"])),
        format!("{size}, {labels:?}"),
    );
    let popup = control_for(view, sel!(ideSearchEngineChanged:)).and_then(|c| c.downcast::<NSPopUpButton>().ok());
    let chosen = popup.as_ref().map(|popup| {
        popup.selectItemAtIndex(1);
        act(popup);
        Settings::load_user().get(&keys::SEARCH_ENGINE)
    });
    check(
        "the search engine's menu stores the engine picked",
        chosen == Some(SearchEngine::ALL[1]),
        format!("{chosen:?}"),
    );
}

fn collect_labels(view: &NSView, out: &mut Vec<String>) {
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        out.push(field.stringValue().to_string());
    }
    for child in view.subviews().iter() {
        collect_labels(&child, out);
    }
}

/// Whether the settings window's content is as large as the page in front asks.
fn fits() -> (bool, String) {
    let Some(window) = crate::settings::window() else {
        return (false, "no window".into());
    };
    let page = window
        .contentViewController()
        .and_then(|c| c.downcast::<NSTabViewController>().ok())
        .and_then(|tabs| {
            let index = usize::try_from(tabs.selectedTabViewItemIndex()).ok()?;
            tabs.tabViewItems().iter().nth(index)?.viewController(window.mtm())
        });
    let (Some(page), Some(content)) = (page, window.contentView()) else {
        return (false, "no page".into());
    };
    let (want, have) = (page.view().fittingSize(), content.frame().size);
    let same = (want.width - have.width).abs() < 1.0 && (want.height - have.height).abs() < 1.0;
    (same, format!("page {want:?}, window {have:?}"))
}

fn agents() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show_agents(mtm);
    crate::settings::agents_changed();
    main_after(Duration::from_millis(600), || {
        let (same, detail) = fits();
        check(
            "the agents changing while their tab is open list them again, in a window as large as the list",
            same && !crate::settings::agents_lines().is_empty(),
            detail,
        );
        let entry = "git.settings-general.example=gitlab";
        let mut settings = Settings::load_user();
        let mut hosts = settings.get(&crate::ci_panel::CI_HOSTS);
        hosts.push(entry.to_string());
        let stored = settings.set(&crate::ci_panel::CI_HOSTS, &hosts).is_ok();
        crate::settings::ci_changed();
        let rows = crate::settings::ci_tab()
            .map(|owner| crate::settings::ci_selftest::rows(&owner, crate::settings::CiList::Hosts))
            .unwrap_or_default();
        check(
            "a change of the CI settings from elsewhere shows in the open window's CI tab",
            stored
                && rows.0.iter().any(|row| row[0] == "git.settings-general.example")
                && usize::try_from(rows.1).is_ok_and(|shown| shown == rows.0.len()),
            format!("{rows:?}"),
        );
        own_agent();
        finish(NAME);
    });
}

/// The own agent's rules and endpoints as its settings tab and its threads keep and forget them.
fn own_agent() {
    use crate::own_agent;
    use ide_agent::{Endpoint, ProviderKind, Rule};

    let root = ide_core::paths::home().join("rules-of-settings");
    let fetch = Rule::Fetch { host: "docs.rs".into() };
    own_agent::add_rule(&root, Rule::Edits);
    own_agent::add_rule(&root, fetch.clone());
    own_agent::add_rule(&root, Rule::Edits);
    let kept = own_agent::rules(&root);
    let project = root.display().to_string();
    own_agent::remove_rule(&project, 0);
    let left = own_agent::rules(&root);
    own_agent::remove_rule(&project, 0);
    let stored = Settings::load_user().get(&own_agent::RULES);
    check(
        "the own agent keeps a project's rule once and forgets the project with its last rule",
        kept == [Rule::Edits, fetch.clone()] && left == [fetch] && !stored.contains_key(&project),
        format!("{kept:?} → {left:?} → {stored:?}"),
    );
    let id = own_agent::new_endpoint_id();
    let endpoint = Endpoint::new(&id, ProviderKind::OpenAiCompatible, "Local", "http://127.0.0.1:9");
    own_agent::save_endpoint(endpoint.clone());
    let keyed = own_agent::set_key(&endpoint, "sk-settings-general").is_ok() && own_agent::key(&endpoint).is_some();
    own_agent::remove_endpoint(&id);
    let account = ide_agent::keys::account(&endpoint);
    let key_left = own_agent::secrets().get(&account).ok().flatten();
    check(
        "removing an added endpoint forgets it and its key",
        keyed && own_agent::endpoint(&id).is_none() && key_left.is_none(),
        format!("keyed {keyed}, key left {}", key_left.is_some()),
    );
    let Some(service) = own_agent::endpoint(ProviderKind::Anthropic.id()) else {
        return check("the own agent offers Anthropic's service", false, "");
    };
    let before = own_agent::default_choice();
    let _ = own_agent::set_key(&service, "sk-ant-settings-general");
    let keyed = own_agent::default_choice();
    let _ = own_agent::set_key(&service, "   ");
    let blank = own_agent::default_choice();
    let wanted = ProviderKind::Anthropic
        .default_model()
        .map(|model| own_agent::ModelChoice {
            endpoint: service.id.clone(),
            model: model.to_string(),
        });
    check(
        "without a chosen model a thread starts on the default model of the service with a key; a blank key forgets it",
        before.is_none()
            && keyed == wanted
            && wanted.is_some()
            && blank.is_none()
            && own_agent::key(&service).is_none(),
        format!("{before:?} → {keyed:?} → {blank:?}"),
    );
}
