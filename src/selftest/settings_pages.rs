//! The `settings-pages` scenario: the settings tabs driven through their own controls and sheets, against a stub
//! provider on the loopback interface. The Agent tab adds an endpoint, keeps its key, refuses and changes its address,
//! puts a moved service back and forgets an endpoint left without address; it picks the endpoint, model and effort of
//! new threads, keeps and removes a price, removes a rule and a folder and asks for folders with "Erlauben"; its part
//! for commit messages picks a provider, a model and takes permissions back. The Agents tab rolls an installed agent
//! back, asks before updating it, removes a rule Claude stored for a folder without window and leads to the Agent tab.
//! The General tab switches inlay hints, a language's hints and blame; the tab of edit prediction lists the local
//! server's models, keeps its address, model and exclusions, warns about a model of no known family, asks before
//! Copilot gets code and takes that back. The CI and language server tabs ask in a sheet for what "+" adds.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_agent::stub::{Answer, Stub};
use ide_agent::{Endpoint, ProviderKind, Rule};
use ide_core::settings::Settings;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, MainThreadOnly, Message, sel};
use objc2_app_kit::{
    NSAccessibility, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSComboBox, NSControl,
    NSControlStateValueOff, NSControlStateValueOn, NSModalResponse, NSModalResponseCancel, NSOpenPanel, NSPopUpButton,
    NSTabViewController, NSTableView, NSTextField, NSView, NSWindow,
};
use serde_json::json;

use super::helpers::{self, answer_alert, answer_alert_saying, in_any_mode_after, wait_until};
use super::{check, finish};
use crate::editor::predict::model::{self as predict, Choice, Local};
use crate::settings::{AgentList as List, agent_selftest as agent};
use crate::util::ns;
use ide_ui_agent::own_agent;

const NAME: &str = "settings-pages";
const MODEL: &str = "stub-coder";
const OLLAMA_MODEL: &str = "qwen2.5-coder:1.5b";
/// The agent preset whose installation the scenario fakes, the first of the Agents tab.
const AGENT: &str = "codex-acp";
const RULE_ELSEWHERE: &str = "Bash(make:*)";

/// What the steps share: the project, the stub's address and the endpoint with a key made before the window.
#[derive(Clone)]
struct Fixture {
    root: PathBuf,
    url: String,
    elsewhere: PathBuf,
}

fn stub() -> Option<Stub> {
    Stub::start(|seen| {
        if seen.path.starts_with("/api/tags") {
            return Answer::json(
                200,
                &json!({"models": [{"name": OLLAMA_MODEL}, {"name": "starcoder2:3b"}]}),
            );
        }
        Answer::json(
            200,
            &json!({"data": [{"id": MODEL, "display_name": "Stub Coder"}], "has_more": false}),
        )
    })
    .ok()
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(stub) = stub() else {
        check("the stub provider starts", false, "");
        return finish(NAME);
    };
    let url = stub.url();
    // The stub serves until the app quits.
    std::mem::forget(stub);
    let home = ide_core::paths::home();
    let root = home.join("settings-project");
    let elsewhere = home.join("rules-elsewhere");
    let _ = std::fs::create_dir_all(root.join("src"));
    let _ = std::fs::write(root.join("src/main.rs"), "fn main() {}\n");
    let _ = std::fs::create_dir_all(elsewhere.join(".claude"));
    let _ = std::fs::write(
        elsewhere.join(".claude/settings.local.json"),
        json!({"permissions": {"allow": [RULE_ELSEWHERE]}}).to_string(),
    );
    let root = root.canonicalize().unwrap_or(root);
    let elsewhere = elsewhere.canonicalize().unwrap_or(elsewhere);
    crate::project::trust(&root);
    prepare(&root, &elsewhere, &url);
    crate::app::open_project(mtm, root.clone());
    let fixture = Fixture { root, url, elsewhere };
    helpers::wait_for_window(&fixture.root.clone(), move |_| {
        if let Some(mtm) = MainThreadMarker::new() {
            crate::settings::show_agent(mtm);
        }
        wait_until(|| agent_tab().is_some(), move |_| add_endpoint(fixture));
    });
}

/// What the settings hold before the window is first built: an endpoint with a key and the consent to send it
/// changes, a rule of the own agent, the local servers of edit prediction at the stub, an installed agent with a
/// version before, and a folder whose rules Claude stored.
fn prepare(root: &Path, elsewhere: &Path, url: &str) {
    let werkbank = Endpoint::new(
        &own_agent::new_endpoint_id(),
        ProviderKind::OpenAiCompatible,
        "Werkbank",
        &format!("{url}/v1"),
    );
    own_agent::save_endpoint(werkbank.clone());
    let keyed = own_agent::set_key(&werkbank, "sk-werkbank");
    check(
        "an endpoint keeps its key before the settings open",
        keyed.is_ok(),
        format!("{keyed:?}"),
    );
    own_agent::add_rule(root, Rule::Edits);
    let mut settings = Settings::load_user();
    let local = |model: &str| Local {
        url: url.to_string(),
        model: model.to_string(),
    };
    let stored = [
        settings.set(&ide_ui_git::message::source::CONSENT, &vec![werkbank.id.clone()]),
        settings.set(&predict::OLLAMA, &local("")),
        settings.set(&predict::LM_STUDIO, &local("weird-model")),
    ];
    check(
        "the settings take what the scenario starts from",
        stored.iter().all(Result::is_ok),
        format!("{stored:?}"),
    );
    crate::editor::predict::provider::settings_changed();
    let agents = ide_acp::Installs::default_root().join(AGENT);
    for version in ["2.4.0", "2.5.0"] {
        let bin = agents.join(version).join("bin");
        let _ = std::fs::create_dir_all(&bin);
        let _ = std::fs::write(bin.join(AGENT), "#!/bin/sh\n");
    }
    let installed = |version: &str| json!({"version": version, "program": format!("bin/{AGENT}"), "args": [], "env": [], "verified": false});
    let _ = std::fs::write(
        agents.join("state.json"),
        json!({"current": installed("2.5.0"), "previous": installed("2.4.0")}).to_string(),
    );
    crate::settings::remember_claude_rules(elsewhere);
}

fn window() -> Option<Retained<NSWindow>> {
    crate::settings::window()
}

fn tabs() -> Option<Retained<NSTabViewController>> {
    window()?
        .contentViewController()?
        .downcast::<NSTabViewController>()
        .ok()
}

fn select_tab(index: isize) {
    if let Some(tabs) = tabs() {
        tabs.setSelectedTabViewItemIndex(index);
    }
}

fn collect<T: Message + objc2::DowncastTarget>(view: &NSView, found: &mut Vec<Retained<T>>) {
    if let Some(item) = view.downcast_ref::<T>() {
        found.push(item.retain());
    }
    for child in view.subviews().iter() {
        collect(&child, found);
    }
}

pub(super) fn all_in<T: Message + objc2::DowncastTarget>(window: &NSWindow) -> Vec<Retained<T>> {
    let mut found = Vec::new();
    if let Some(content) = window.contentView() {
        collect(&content, &mut found);
    }
    found
}

/// The control of the settings window's open tab that sends `action`, with `tag` if given.
pub(super) fn control(action: Sel, tag: Option<isize>) -> Option<Retained<NSControl>> {
    all_in::<NSControl>(&*window()?)
        .into_iter()
        .find(|control| control.action() == Some(action) && tag.is_none_or(|tag| control.tag() == tag))
}

/// Sends `control`'s action as a click on it does; whether it could be clicked.
pub(super) fn act(control: &NSControl) -> bool {
    if !control.isEnabled() || control.isHiddenOrHasHiddenAncestor() {
        return false;
    }
    let app = NSApplication::sharedApplication(control.mtm());
    let target = control.target();
    let Some(action) = control.action() else { return false };
    // SAFETY: The control's own action and target, with the control as the sender.
    unsafe { app.sendAction_to_from(action, target.as_deref(), Some(control)) }
}

/// Clicks the control that sends `action`; whether there was one to click.
fn press(action: Sel) -> bool {
    control(action, None).is_some_and(|c| act(&c))
}

/// Picks `index` of the menu that sends `action` and sends it, as choosing the item does.
fn choose(action: Sel, index: isize) -> bool {
    let Some(menu) = control(action, None).and_then(|c| c.downcast::<NSPopUpButton>().ok()) else {
        return false;
    };
    menu.selectItemAtIndex(index);
    act(&menu)
}

/// Types `text` into the field that sends `action` and ends the editing with Return.
fn enter(action: Sel, text: &str) -> bool {
    let Some(field) = control(action, None).and_then(|c| c.downcast::<NSTextField>().ok()) else {
        return false;
    };
    field.setStringValue(&ns(text));
    act(&field)
}

fn sheet() -> Option<Retained<NSWindow>> {
    window()?.attachedSheet()
}

/// Calls `then` once no sheet is in front of the settings window.
fn sheet_gone(then: impl FnOnce() + 'static) {
    wait_until(|| sheet().is_none(), move |_| then());
}

/// The texts the sheet in front shows, labels and fields alike.
fn sheet_texts(sheet: &NSWindow) -> Vec<String> {
    all_in::<NSTextField>(sheet)
        .iter()
        .map(|field| field.stringValue().to_string())
        .collect()
}

/// Clicks `action`, waits for its sheet, types `values` into its fields in order, picks `kind` in its menu and ends it
/// with `response`; `then` gets what the sheet showed before, or nothing when no sheet came.
fn through_sheet(
    action: Sel,
    values: Vec<String>,
    kind: Option<isize>,
    response: NSModalResponse,
    then: impl FnOnce(Option<Vec<String>>) + 'static,
) {
    if !press(action) {
        return then(None);
    }
    wait_until(
        || sheet().is_some(),
        move |came| {
            let Some(sheet) = sheet().filter(|_| came) else {
                return then(None);
            };
            let shown = sheet_texts(&sheet);
            let fields: Vec<Retained<NSTextField>> = all_in::<NSTextField>(&sheet)
                .into_iter()
                .filter(|field| field.isEditable())
                .collect();
            for (field, value) in fields.iter().zip(&values) {
                field.setStringValue(&ns(value));
            }
            if let (Some(index), Some(menu)) = (kind, all_in::<NSPopUpButton>(&sheet).first()) {
                menu.selectItemAtIndex(index);
            }
            if let Some(window) = window() {
                window.endSheet_returnCode(&sheet, response);
            }
            sheet_gone(move || then(Some(shown)));
        },
    );
}

fn agent_tab() -> Option<Retained<crate::settings::AgentSettings>> {
    crate::settings::agent_tab()
}

fn rows(list: List) -> Vec<Vec<String>> {
    agent_tab().map(|tab| agent::rows(&tab, list)).unwrap_or_default()
}

fn status() -> String {
    agent_tab().map(|tab| agent::shown(&tab).1).unwrap_or_default()
}

fn row_named(name: &str) -> Option<usize> {
    rows(List::Endpoints)
        .iter()
        .position(|row| row.first().map(String::as_str) == Some(name))
}

fn select(list: List, row: usize) {
    if let Some(tab) = agent_tab() {
        agent::select(&tab, list, row);
    }
}

fn add_endpoint(f: Fixture) {
    let address = format!("{}/v0", f.url);
    through_sheet(
        sel!(ideAddAgentEndpoint:),
        vec!["Neu".into(), address.clone()],
        Some(0),
        NSAlertFirstButtonReturn,
        move |shown| {
            let added = row_named("Neu").map(|row| rows(List::Endpoints)[row].clone());
            check(
                "\"+\" asks for an endpoint in a sheet and keeps what it says",
                shown.is_some() && added.as_ref().is_some_and(|row| row[1] == address && row[2] == "—"),
                format!("{added:?}"),
            );
            let before = rows(List::Endpoints).len();
            through_sheet(
                sel!(ideAddAgentEndpoint:),
                vec!["Verworfen".into(), format!("{}/x", f.url)],
                None,
                NSAlertSecondButtonReturn,
                move |shown| {
                    check(
                        "Abbrechen in the endpoint's sheet adds nothing",
                        shown.is_some() && rows(List::Endpoints).len() == before && row_named("Verworfen").is_none(),
                        format!("{:?}", rows(List::Endpoints)),
                    );
                    key(f);
                },
            );
        },
    );
}

fn key(f: Fixture) {
    let Some(row) = row_named("Neu") else {
        check("the new endpoint is listed", false, "");
        return finish(NAME);
    };
    select(List::Endpoints, row);
    through_sheet(
        sel!(ideAgentKey:),
        vec!["sk-neu".into()],
        None,
        NSAlertFirstButtonReturn,
        move |shown| {
            let key = row_named("Neu").map(|row| rows(List::Endpoints)[row][2].clone());
            check(
                "the key sheet names the endpoint and keeps its key in the keychain",
                shown.is_some_and(|texts| texts.iter().any(|t| t.contains("Neu")))
                    && key.as_deref() == Some("im Schlüsselbund"),
                format!("{key:?}"),
            );
            address(f);
        },
    );
}

fn address(f: Fixture) {
    if let Some(row) = row_named("Neu") {
        select(List::Endpoints, row);
    }
    through_sheet(
        sel!(ideAgentAddress:),
        vec!["ftp://werkstatt".into()],
        None,
        NSAlertFirstButtonReturn,
        move |shown| {
            let prefilled = format!("{}/v0", f.url);
            check(
                "the address sheet starts from the endpoint's address and refuses one that is not http",
                shown.is_some_and(|texts| texts.contains(&prefilled)) && status().contains("ftp://werkstatt"),
                status(),
            );
            let moved = format!("{}/v1", f.url);
            through_sheet(
                sel!(ideAgentAddress:),
                vec![moved.clone()],
                None,
                NSAlertFirstButtonReturn,
                move |_| {
                    let now = row_named("Neu").map(|row| rows(List::Endpoints)[row][1].clone());
                    check(
                        "the address sheet keeps a new http address",
                        now.as_deref() == Some(moved.as_str()),
                        format!("{now:?}"),
                    );
                    move_service(f);
                },
            );
        },
    );
}

/// A service at another address offers "−", which puts it back at its own.
fn move_service(f: Fixture) {
    let original = rows(List::Endpoints).first().cloned().unwrap_or_default();
    select(List::Endpoints, 0);
    let removable_before = control(sel!(ideRemoveAgentEndpoint:), None).is_some_and(|c| c.isEnabled());
    let elsewhere = format!("{}/elsewhere", f.url);
    through_sheet(
        sel!(ideAgentAddress:),
        vec![elsewhere.clone()],
        None,
        NSAlertFirstButtonReturn,
        move |_| {
            let moved = rows(List::Endpoints).first().map(|row| row[1].clone());
            select(List::Endpoints, 0);
            let removed = press(sel!(ideRemoveAgentEndpoint:));
            let back = rows(List::Endpoints).first().map(|row| row[1].clone());
            check(
                "a service moved to another address goes back to its own with \"−\", which it only offers then",
                !removable_before
                    && moved.as_deref() == Some(elsewhere.as_str())
                    && removed
                    && back.as_ref() == original.get(1),
                format!("{original:?} {moved:?} {back:?}"),
            );
            forget_endpoint(f);
        },
    );
}

/// An endpoint of the user's own left without address is forgotten.
fn forget_endpoint(f: Fixture) {
    let Some(row) = row_named("Neu") else {
        check("the new endpoint is still listed", false, "");
        return finish(NAME);
    };
    select(List::Endpoints, row);
    through_sheet(
        sel!(ideAgentAddress:),
        vec![String::new()],
        None,
        NSAlertFirstButtonReturn,
        move |_| {
            check(
                "an endpoint of one's own without address is forgotten",
                row_named("Neu").is_none(),
                format!("{:?}", rows(List::Endpoints)),
            );
            defaults(f);
        },
    );
}

fn defaults(f: Fixture) {
    let werkbank = own_agent::endpoints().into_iter().position(|e| e.name == "Werkbank");
    let chosen = werkbank.is_some_and(|row| choose(sel!(ideAgentDefaultEndpoint:), isize::try_from(row).unwrap_or(0)));
    wait_until(
        || {
            let model = Settings::load_user().get(&own_agent::MODEL);
            model.is_some_and(|m| m.model == MODEL)
        },
        move |loaded| {
            check(
                "picking an endpoint for new threads loads its models and takes the first",
                chosen && loaded && status().contains("bietet ein Modell"),
                status(),
            );
            let keyless = rows(List::Endpoints).iter().position(|row| row[2] == "—");
            let refused =
                keyless.is_some_and(|row| choose(sel!(ideAgentDefaultEndpoint:), isize::try_from(row).unwrap_or(0)));
            check(
                "an endpoint without its key says it needs one first",
                refused && status().contains("braucht zuerst einen Schlüssel"),
                status(),
            );
            if let Some(row) = werkbank {
                choose(sel!(ideAgentDefaultEndpoint:), isize::try_from(row).unwrap_or(0));
            }
            let typed = enter(sel!(ideAgentDefaultModel:), "eigenes-modell");
            let model = Settings::load_user().get(&own_agent::MODEL);
            check(
                "a model typed into the field is the one new threads take",
                typed && model.is_some_and(|m| m.model == "eigenes-modell"),
                "",
            );
            let high = choose(sel!(ideAgentEffort:), 3)
                && Settings::load_user().get(&own_agent::EFFORT).as_deref() == Some("high");
            let usual = choose(sel!(ideAgentEffort:), 0) && Settings::load_user().get(&own_agent::EFFORT).is_none();
            check(
                "the effort menu keeps a level, or none for the model's own",
                high && usual,
                "",
            );
            sheet_gone(move || prices(f));
        },
    );
}

fn prices(f: Fixture) {
    through_sheet(
        sel!(ideAddAgentPrice:),
        vec!["gpt-x".into(), "1,5".into(), "6".into()],
        None,
        NSAlertFirstButtonReturn,
        move |shown| {
            let listed = rows(List::Prices);
            check(
                "a price is kept per million tokens, with a comma as decimal point too",
                shown.is_some() && listed == vec![vec!["gpt-x".to_string(), "1.50".into(), "6.00".into()]],
                format!("{listed:?}"),
            );
            through_sheet(
                sel!(ideAddAgentPrice:),
                vec![String::new(), "viel".into(), "1".into()],
                None,
                NSAlertFirstButtonReturn,
                move |_| {
                    check(
                        "a price without model or number is refused and says why",
                        status().contains("braucht ein Modell und zwei Zahlen") && rows(List::Prices).len() == 1,
                        status(),
                    );
                    select(List::Prices, 0);
                    let removed = press(sel!(ideRemoveAgentPrice:));
                    let kept = Settings::load_user().get(&own_agent::PRICES);
                    check(
                        "\"−\" removes the selected price",
                        removed && rows(List::Prices).is_empty() && !kept.models.contains_key("gpt-x"),
                        "",
                    );
                    rules_and_folders(f);
                },
            );
        },
    );
}

fn rules_and_folders(f: Fixture) {
    let project = f.root.display().to_string();
    let listed = rows(List::Rules);
    let rule = listed.iter().position(|row| row.first() == Some(&project));
    if let Some(row) = rule {
        select(List::Rules, row);
    }
    let removed = rule.is_some() && press(sel!(ideRemoveAgentRule:));
    check(
        "a rule of the own agent is listed with its project and \"−\" forgets it",
        removed && own_agent::rules(&f.root).is_empty(),
        format!("{listed:?}"),
    );
    if let Some(tab) = agent_tab() {
        agent::add_folders(&tab, vec![f.root.join("src")]);
    }
    select(List::Folders, 0);
    let removed = press(sel!(ideRemoveAgentFolder:));
    check(
        "\"−\" removes the selected folder of the sandbox",
        removed && rows(List::Folders).is_empty() && Settings::load_user().get(&own_agent::ALLOWED).is_empty(),
        format!("{:?}", rows(List::Folders)),
    );
    let prompt = std::rc::Rc::new(std::cell::RefCell::new(None));
    let seen = std::rc::Rc::clone(&prompt);
    in_any_mode_after(Duration::from_millis(800), move || {
        let mtm = MainThreadMarker::new();
        let panel = mtm.and_then(|mtm| NSApplication::sharedApplication(mtm).modalWindow());
        let panel = panel.and_then(|p| p.downcast::<NSOpenPanel>().ok());
        *seen.borrow_mut() = panel.map(|p| (p.prompt().to_string(), p.canChooseDirectories(), p.canChooseFiles()));
        answer_alert(NSModalResponseCancel);
    });
    let asked = press(sel!(ideAddAgentFolder:));
    let prompt = prompt.borrow().clone();
    check(
        "\"Ordner erlauben …\" asks for folders only, with \"Erlauben\", and Abbrechen adds none",
        asked && prompt == Some(("Erlauben".to_string(), true, false)) && rows(List::Folders).is_empty(),
        format!("{prompt:?}"),
    );
    sheet_gone(move || commit_messages(f));
}

fn commit_messages(f: Fixture) {
    let werkbank = own_agent::endpoints().into_iter().position(|e| e.name == "Werkbank");
    let chosen =
        werkbank.is_some_and(|row| choose(sel!(ideCommitMessageEndpoint:), isize::try_from(row + 1).unwrap_or(0)));
    let commit_model = || Settings::load_user().get(&ide_ui_git::message::source::MODEL);
    wait_until(
        move || commit_model().is_some_and(|m| m.model == MODEL),
        move |loaded| {
            check(
                "picking a provider for commit messages loads its models and takes the first",
                chosen && loaded,
                format!("{:?}", commit_model().map(|m| m.model)),
            );
            let typed = enter(sel!(ideCommitMessageModel:), "kurz");
            check(
                "a model typed for commit messages is kept",
                typed && commit_model().is_some_and(|m| m.model == "kurz"),
                "",
            );
            let automatic = choose(sel!(ideCommitMessageEndpoint:), 0);
            let field = control(sel!(ideCommitMessageModel:), None).and_then(|c| c.downcast::<NSComboBox>().ok());
            check(
                "\"Automatisch\" leaves the model to claude and greys the field out",
                automatic && commit_model().is_none() && field.is_some_and(|f| !f.isEnabled()),
                "",
            );
            let before = crate::settings::commit_message_selftest::allowed();
            let revoked = press(sel!(ideCommitMessageRevoke:));
            let after = crate::settings::commit_message_selftest::allowed();
            check(
                "the permission given to a provider is listed and taken back by its button",
                revoked
                    && before.is_some_and(|(text, on)| text.contains("Werkbank") && on)
                    && after.is_some_and(|(text, on)| text.starts_with("Noch kein") && !on),
                "",
            );
            agents(f);
        },
    );
}

fn agents_line_of(name: &str) -> String {
    crate::settings::agents_lines()
        .into_iter()
        .find(|line| line.contains(name))
        .unwrap_or_default()
}

fn agents(f: Fixture) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show_agents(mtm);
    wait_until(
        || agents_line_of("Codex").contains("Version"),
        move |_| {
            let before = agents_line_of("Codex");
            let rolled = control(sel!(ideAgentsRollback:), Some(0)).is_some_and(|c| act(&c));
            let after = agents_line_of("Codex");
            check(
                "an installed agent offers its update and the version before, and goes back to it",
                before.contains("Version 2.5.0")
                    && before.contains("Auf 2.6.0 aktualisieren")
                    && before.contains("Zurück zu 2.4.0")
                    && rolled
                    && after.contains("Version 2.4.0")
                    && after.contains("Zurück zu 2.5.0"),
                format!("{before} → {after}"),
            );
            let asked = answer_alert_saying(
                Duration::from_millis(600),
                "Codex installieren?".to_string(),
                NSAlertSecondButtonReturn,
            );
            let pressed = control(sel!(ideAgentsInstall:), Some(0)).is_some_and(|c| act(&c));
            check(
                "an update asks first, and Abbrechen installs nothing",
                pressed && asked.get() && agents_line_of("Codex").contains("Version 2.4.0"),
                agents_line_of("Codex"),
            );
            stored_rule(f);
        },
    );
}

fn stored_rule(f: Fixture) {
    let table = window().and_then(|w| {
        all_in::<NSTableView>(&w).into_iter().find(|t| {
            t.accessibilityLabel()
                .is_some_and(|l| l.to_string() == "Gespeicherte Regeln von Claude")
        })
    });
    let place = f.elsewhere.display().to_string();
    let cells = crate::settings::stored_rules().map(|r| r.cells()).unwrap_or_default();
    let row = cells.iter().position(|row| {
        row.first().is_some_and(|p| p.ends_with("rules-elsewhere"))
            && row.get(1).map(String::as_str) == Some(RULE_ELSEWHERE)
    });
    if let (Some(table), Some(row)) = (&table, row) {
        table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
    }
    let removed = row.is_some() && press(sel!(ideRemoveStoredRule:));
    let file = std::fs::read_to_string(f.elsewhere.join(".claude/settings.local.json")).unwrap_or_default();
    check(
        "a folder whose rules Claude stored is listed without a window, and \"−\" takes its rule out",
        removed && !file.contains(RULE_ELSEWHERE),
        format!("{place}: {cells:?}"),
    );
    let led = press(sel!(ideAgentsOwnSettings:));
    check(
        "the Agents tab leads to the own agent's tab",
        led && tabs().is_some_and(|t| t.selectedTabViewItemIndex() == 3),
        "",
    );
    sheet_gone(move || general(f));
}

fn general(f: Fixture) {
    select_tab(0);
    let general_shown =
        || tabs().is_some_and(|t| t.selectedTabViewItemIndex() == 0) && crate::settings::editor_switches().is_some();
    wait_until(general_shown, move |_| {
        let hints = crate::lsp::inlay::enabled();
        let flipped = press(sel!(ideToggleInlayHints:));
        let switched = crate::settings::editor_switches();
        check(
            "the inlay hints' switch turns them over and shows it",
            flipped && crate::lsp::inlay::enabled() != hints && switched.as_ref().is_some_and(|s| s.0 != hints),
            format!("{switched:?}"),
        );
        if !crate::lsp::inlay::enabled() {
            press(sel!(ideToggleInlayHints:));
        }
        let language = ide_lsp::BUILT_IN
            .first()
            .map(|l| l.name().to_string())
            .unwrap_or_default();
        let before = crate::lsp::inlay::enabled_for(&language);
        let menu = window().and_then(|w| {
            all_in::<NSPopUpButton>(&w).into_iter().find(|p| {
                p.itemArray()
                    .iter()
                    .any(|item| item.action() == Some(sel!(ideToggleInlayLanguage:)))
            })
        });
        if let Some(menu) = menu.and_then(|m| m.menu()) {
            menu.performActionForItemAtIndex(1);
        }
        check(
            "the languages' pull-down turns one language's hints over",
            crate::lsp::inlay::enabled_for(&language) != before,
            language,
        );
        let blame = crate::editor::blame_at_line_end();
        let switched = control(sel!(ideToggleBlameAtLineEnd:), None)
            .and_then(|c| c.downcast::<objc2_app_kit::NSButton>().ok())
            .is_some_and(|button| {
                button.setState(if blame {
                    NSControlStateValueOff
                } else {
                    NSControlStateValueOn
                });
                act(&button)
            });
        check(
            "the blame switch keeps what it shows",
            switched && crate::editor::blame_at_line_end() != blame,
            "",
        );
        prediction(f);
    });
}

fn local() -> Option<crate::settings::LocalShown> {
    crate::settings::prediction_local()
}

fn prediction(f: Fixture) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show_prediction(mtm);
    let ollama = index_of(Choice::Ollama);
    let chosen = choose(sel!(idePredictionProvider:), ollama);
    wait_until(
        || local().is_some_and(|l| l.offered.iter().any(|m| m == OLLAMA_MODEL)),
        move |listed| {
            let shown = local();
            check(
                "Ollama on this Mac needs no permission and offers the server's models",
                chosen
                    && listed
                    && shown.as_ref().is_some_and(|l| {
                        l.about.contains("verlässt den Mac nicht") && l.note.contains("Wähle ein Modell")
                    }),
                shown
                    .map(|l| format!("{} | {} | {:?}", l.about, l.note, l.offered))
                    .unwrap_or_default(),
            );
            let typed = enter(sel!(idePredictionModel:), OLLAMA_MODEL);
            let kept = Settings::load_user().get(&predict::OLLAMA);
            check(
                "the model picked is kept and the note goes",
                typed && kept.model == OLLAMA_MODEL && local().is_some_and(|l| l.note.is_empty()),
                format!("{kept:?}"),
            );
            let excluded = enter(sel!(idePredictionExclude:), "*.pem, .env* ,");
            check(
                "the patterns that never go out are kept, split at the commas",
                excluded
                    && Settings::load_user().get(&predict::EXCLUDE) == vec!["*.pem".to_string(), ".env*".into()]
                    && local().is_some_and(|l| l.exclude.starts_with("*.pem")),
                format!("{:?}", Settings::load_user().get(&predict::EXCLUDE)),
            );
            unreachable_server(f);
        },
    );
}

/// A server that does not answer says so where the models would be.
fn unreachable_server(f: Fixture) {
    let gone = "http://127.0.0.1:9";
    let changed = enter(sel!(idePredictionLocal:), gone);
    wait_until(
        || local().is_some_and(|l| !l.note.is_empty() && l.offered.is_empty()),
        move |said| {
            let kept = Settings::load_user().get(&predict::OLLAMA);
            check(
                "a new server address is kept, and a server that does not answer says so",
                changed && said && kept.url == gone && local().is_some_and(|l| l.url == gone),
                local().map(|l| l.note).unwrap_or_default(),
            );
            enter(sel!(idePredictionLocal:), &f.url);
            let lm = choose(sel!(idePredictionProvider:), index_of(Choice::LmStudio));
            check(
                "a model of no known family warns that suggestions may not come",
                lm && local().is_some_and(|l| l.model == "weird-model" && l.note.contains("keiner bekannten")),
                local().map(|l| l.note).unwrap_or_default(),
            );
            sheet_gone(move || copilot(f));
        },
    );
}

fn copilot(f: Fixture) {
    let pressed = choose(sel!(idePredictionProvider:), index_of(Choice::Copilot));
    wait_until(
        || sheet().is_some(),
        move |asked| {
            let texts = sheet().map(|s| sheet_texts(&s)).unwrap_or_default();
            if let (Some(window), Some(sheet)) = (window(), sheet()) {
                window.endSheet_returnCode(&sheet, NSAlertFirstButtonReturn);
            }
            wait_until(
                || {
                    crate::settings::prediction_shown().is_some_and(|(_, copilot, allowed, _)| {
                        allowed.starts_with("Erlaubt") && !copilot.contains("startet")
                    })
                },
                move |allowed| {
                    let shown = crate::settings::prediction_shown();
                    check(
                        "Copilot asks before code goes to GitHub, keeps the permission and says its server is missing",
                        pressed
                            && asked
                            && texts.iter().any(|t| t.contains("senden?"))
                            && allowed
                            && shown.as_ref().is_some_and(|(_, copilot, _, revoke)| {
                                copilot.contains("nicht installiert") && *revoke
                            }),
                        format!("{shown:?}"),
                    );
                    let revoked = press(sel!(idePredictionRevoke:));
                    let shown = crate::settings::prediction_shown();
                    check(
                        "\"Erlaubnisse zurücknehmen\" takes the permission back",
                        revoked
                            && shown
                                .as_ref()
                                .is_some_and(|(_, _, allowed, revoke)| allowed.starts_with("Kein Anbieter") && !revoke),
                        format!("{shown:?}"),
                    );
                    let off = choose(sel!(idePredictionProvider:), index_of(Choice::Off));
                    check(
                        "without a provider the tab says the editor proposes nothing",
                        off && local().is_some_and(|l| l.about.starts_with("Ohne Anbieter")),
                        "",
                    );
                    sheet_gone(move || plus_sheets(f));
                },
            );
        },
    );
}

/// Where `choice` stands in the menu of providers.
fn index_of(choice: Choice) -> isize {
    Choice::ALL
        .iter()
        .position(|c| *c == choice)
        .and_then(|n| isize::try_from(n).ok())
        .unwrap_or(0)
}

fn plus_sheets(_f: Fixture) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show_ci(mtm);
    wait_until(
        || crate::settings::ci_tab().is_some(),
        move |_| {
            through_sheet(
                sel!(ideAddCiHost:),
                Vec::new(),
                None,
                NSAlertSecondButtonReturn,
                move |ci| {
                    if let Some(mtm) = MainThreadMarker::new() {
                        crate::settings::show_servers(mtm);
                    }
                    wait_until(
                        || crate::settings::servers_tab().is_some(),
                        move |_| {
                            through_sheet(
                                sel!(ideAddLanguageServer:),
                                Vec::new(),
                                None,
                                NSAlertSecondButtonReturn,
                                move |servers| {
                                    check(
                                        "\"+\" of the CI hosts and of the language servers asks in a sheet",
                                        ci.is_some() && servers.is_some(),
                                        format!("{ci:?} {servers:?}"),
                                    );
                                    finish(NAME);
                                },
                            );
                        },
                    );
                },
            );
        },
    );
}
