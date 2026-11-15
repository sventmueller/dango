//! The end of the `edit-prediction` scenario: its commands as the Editor menu and the status bar's button send them.
//! The menu's items are enabled with a proposal on view and the language's names the file's language with a tick;
//! taking a word, dropping and taking all; the language switched off from the menu and on again from the status bar;
//! the button leading to the settings without a provider, asking for consent for one outside the Mac, and offering to
//! install Copilot where its server is missing.

use std::sync::Arc;
use std::time::Duration;

use ide_predict::stub::Stub;
use ide_predict::{Prediction, Provider};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSApplication, NSButton, NSControlStateValueOn, NSMenuItem, NSMenuItemValidation,
};
use splat_i18n::t;

use super::{check, finish, give_up, helpers};
use crate::editor::predict::{self, model};
use crate::editor::{self, EditorTextView};

const SCENARIO: &str = "edit-prediction";
const PROJECT: &str = "predict-project";

/// The Editor menu's item of edit prediction with `tag`.
fn item(tag: isize) -> Option<Retained<NSMenuItem>> {
    let main = NSApplication::sharedApplication(MainThreadMarker::new()?).mainMenu()?;
    main.itemArray()
        .iter()
        .filter_map(|top| top.submenu())
        .find_map(|menu| {
            menu.itemArray()
                .iter()
                .find(|item| item.action() == Some(sel!(idePrediction:)) && item.tag() == tag)
        })
}

/// Sends the item with `tag` to `view`, as choosing it in the menu does.
fn choose(view: &EditorTextView, tag: isize) {
    let Some(item) = item(tag) else { return };
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: The item's own action, sent to the editor with the item as the sender, as AppKit sends it.
    unsafe { app.sendAction_to_from(sel!(idePrediction:), Some(view), Some(&item)) };
}

/// Presses the status bar's button of edit prediction, whose tag is 5.
fn press_status(view: &EditorTextView) {
    let button = NSButton::new(view.mtm());
    button.setTag(5);
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: The button's action, sent to the editor with a button of the same tag as the sender.
    unsafe { app.sendAction_to_from(sel!(idePrediction:), Some(view), Some(&button)) };
}

fn propose(view: &EditorTextView, stub: &Arc<dyn Provider>) -> bool {
    let caret = view.selectedRange().location;
    let byte = view.document().text().utf16_to_byte(caret).unwrap_or(0);
    let prediction = Prediction {
        range: byte..byte,
        text: "value_one + 2".into(),
        feedback: None,
    };
    predict::show_now(view, stub.clone(), prediction)
}

fn text(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

/// Goes on from `view`, the scenario's editor of Rust, with the stub forced.
pub(crate) fn run(view: Retained<EditorTextView>) {
    let stub: Arc<dyn Provider> = Arc::new(Stub::new(Vec::new(), Duration::from_secs(5)));
    predict::provider::force(Some(stub.clone()));
    let end = view.document().text().len_utf16();
    view.setSelectedRange(objc2_foundation::NSRange::new(end, 0));
    editor::type_text(&view, "\nlet more = ");
    let before = text(&view);
    let shown = propose(&view, &stub);
    let enabled: Vec<bool> = [1, 2, 3]
        .iter()
        .map(|tag| item(*tag).is_some_and(|item| view.validateMenuItem(&item)))
        .collect();
    let language = item(4).map(|item| {
        let on = view.validateMenuItem(&item);
        (item.title().to_string(), item.state() == NSControlStateValueOn, on)
    });
    check(
        "with a proposal on view the menu's commands are enabled, the language's named after Rust and ticked",
        shown
            && enabled == [true, true, true]
            && language
                .as_ref()
                .is_some_and(|(title, ticked, on)| title.contains("Rust") && *ticked && *on),
        format!("{enabled:?} {language:?}"),
    );
    choose(&view, 2);
    let word = text(&view);
    choose(&view, 3);
    let dropped = predict::shown_proposal(&view).is_none();
    propose(&view, &stub);
    choose(&view, 1);
    check(
        "the menu takes a word of the proposal, drops the rest, and takes a whole one",
        word.starts_with(&format!("{before}value_one")) && dropped && text(&view) == format!("{word}value_one + 2"),
        format!("{word:?} → {:?}", text(&view)),
    );
    choose(&view, 4);
    let off = predict::chosen().off_for.contains(&"rust".to_string());
    let status = predict::status_shown(&view);
    press_status(&view);
    let on = !predict::chosen().off_for.contains(&"rust".to_string());
    check(
        "the menu switches the language off, which the status bar says, and its button switches it on again",
        off && on && status.as_ref().is_some_and(|(title, _)| title.contains("Rust")),
        format!("{status:?}"),
    );
    helpers::wait_for(view_again, settings_button);
}

fn view_again() -> Option<Retained<EditorTextView>> {
    let root = helpers::root(PROJECT);
    let controller = crate::window::for_project(&root)?;
    editor::editor_for(&controller, &root.join("src/main.rs"))
}

/// Without a provider the button opens the settings; with one outside the Mac it asks for consent in a sheet.
fn settings_button(view: Option<Retained<EditorTextView>>) {
    let Some(view) = view else {
        return give_up(SCENARIO, "the editor is open");
    };
    predict::provider::force(None);
    predict::settings_changed();
    press_status(&view);
    let opened = crate::settings::window().is_some_and(|w| w.isVisible());
    if let Some(window) = crate::settings::window() {
        window.close();
    }
    check(
        "without a provider the status bar's button opens the settings",
        opened,
        "",
    );
    let mut settings = ide_core::settings::Settings::load_user();
    let remote = model::Local {
        url: "http://192.0.2.1:11434".into(),
        model: "qwen2.5-coder:1.5b".into(),
    };
    let stored = settings
        .set(&model::PROVIDER, &model::Choice::Ollama)
        .and_then(|()| settings.set(&model::OLLAMA, &remote));
    predict::settings_changed();
    press_status(&view);
    let window = view.window();
    let sheet = window.as_ref().and_then(|w| w.attachedSheet());
    if let (Some(window), Some(sheet)) = (&window, &sheet) {
        window.endSheet_returnCode(sheet, NSAlertFirstButtonReturn);
    }
    helpers::wait_until(
        || predict::chosen().consented(),
        move |consented| {
            predict::provider::revoke_all();
            let mut settings = ide_core::settings::Settings::load_user();
            let restored = settings.set(&model::PROVIDER, &model::Choice::Off);
            predict::settings_changed();
            check(
                "for a provider outside the Mac the button asks in a sheet, and Erlauben gives the consent",
                stored.is_ok() && restored.is_ok() && sheet.is_some() && consented,
                format!("sheet {}, consented {consented}", sheet.is_some()),
            );
            copilot_missing();
        },
    );
}

/// Copilot chosen and allowed where its server is not installed: it is looked for once, and the status bar offers to
/// install it.
fn copilot_missing() {
    let environment = crate::environment::get().map(<[_]>::to_vec).unwrap_or_default();
    if ide_predict::copilot::find(&ide_lsp::default_tools(), &environment).is_none() {
        return look_for_copilot();
    }
    check("skipped: Copilot's server is installed on this Mac", true, "");
    end();
}

fn look_for_copilot() {
    let mut settings = ide_core::settings::Settings::load_user();
    let chosen = settings.set(&model::PROVIDER, &model::Choice::Copilot);
    predict::settings_changed();
    predict::provider::consent();
    let missing = t("Copilot fehlt – installieren?");
    helpers::wait_until(
        move || {
            view_again()
                .and_then(|v| predict::status_shown(&v))
                .is_some_and(|(title, _)| title == missing)
        },
        move |ok| {
            let root = helpers::root(PROJECT);
            let state = predict::provider::copilot_state(&root);
            predict::provider::wake(&root);
            predict::provider::revoke_all();
            let mut settings = ide_core::settings::Settings::load_user();
            let restored = settings.set(&model::PROVIDER, &model::Choice::Off);
            predict::settings_changed();
            check(
                "Copilot without its server says so in the status bar, which offers to install it",
                chosen.is_ok() && restored.is_ok() && ok && state == predict::provider::CopilotState::Missing,
                format!("{state:?}"),
            );
            end();
        },
    );
}

fn end() {
    let saved = editor::save_all();
    check("every editor is saved before quitting", saved, "");
    finish(SCENARIO);
}
