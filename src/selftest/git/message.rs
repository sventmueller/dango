//! The `git-message` scenario (E88, E101) with the stub provider, which sends nothing anywhere: the first request asks
//! for consent and says what goes out; the message appears piece by piece with concrete progress, says that AI wrote
//! it, and one undo brings the draft back; the stub got exactly the checked text, which "Gesendeten Inhalt zeigen"
//! shows; "Neu erzeugen" asks no more; a change of the user's ends the note. A secret in an unstaged change sends
//! nothing and names its file and line. Taking the consent back in the settings asks again. Pictures in light and dark
//! appearance, and the accessibility audit of the window.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_agent::stub::{Answer, Stub};
use ide_agent::{Endpoint, ProviderKind};
use ide_core::settings::Settings;
use ide_git::CommitOptions;
use objc2::sel;
use objc2_app_kit::NSAlertFirstButtonReturn;
use splat_i18n::{t, tf};

use super::stash::sheet_labels;
use super::{answer_sheet, fixture_git, pictures, sheet_ready, sheet_text, window_of, write};
use crate::selftest::helpers::wait_on;
use crate::selftest::{check, finish, snapshot};
use crate::util::main_soon;
use ide_ui_agent::own_agent::ModelChoice;
use ide_ui_git::message::source::{CONSENT, MODEL};
use ide_ui_git::probe;
use ide_ui_git::selftest as panel;

const SCENARIO: &str = "git-message";
const README: &str = "README.md";
const LIBRARY: &str = "src/lib.rs";
const DRAFT: &str = "my draft";
/// The pieces the stub answers with, one after another.
const PIECES: [&str; 6] = ["Describe", " the", " project", " in", " the", " README"];
const ENDPOINT: &str = "commit-stub";
/// An access key in the shape gitleaks' `aws-access-token` rule looks for; it opens nothing.
const FAKE_KEY: &str = "aws_access_key_id = AKIAQYLPMN5HHHFPZAM2\n";

thread_local! {
    static STUB: RefCell<Option<Stub>> = const { RefCell::new(None) };
}

fn answer() -> String {
    let mut text = String::new();
    for piece in PIECES {
        let event = serde_json::json!({"choices": [{"index": 0, "delta": {"content": piece}}]});
        text.push_str(&format!("data: {event}\n\n"));
    }
    text.push_str("data: {\"choices\": [{\"index\": 0, \"delta\": {}, \"finish_reason\": \"stop\"}]}\n\n");
    text.push_str("data: [DONE]\n\n");
    text
}

fn written() -> String {
    PIECES.concat()
}

/// The requests the stub got.
fn requests() -> Vec<ide_agent::stub::Seen> {
    STUB.with(|stub| stub.borrow().as_ref().map(Stub::seen).unwrap_or_default())
}

pub fn run() {
    let stub = match Stub::start(|_| Answer::events_slowly(&answer(), Duration::from_millis(150))) {
        Ok(stub) => stub,
        Err(error) => {
            check("the stub provider starts", false, error.to_string());
            return finish(SCENARIO);
        }
    };
    let base = format!("{}/v1", stub.url());
    STUB.with(|cell| *cell.borrow_mut() = Some(stub));
    ide_ui_agent::own_agent::save_endpoint(Endpoint::new(ENDPOINT, ProviderKind::OpenAiCompatible, "Stub", &base));
    let mut settings = Settings::load_user();
    let chosen = settings.set(
        &MODEL,
        &Some(ModelChoice {
            endpoint: ENDPOINT.into(),
            model: "stub-model".into(),
        }),
    );
    let cleared = settings.set(&CONSENT, &Vec::new());
    check(
        "the stub is the model for commit messages",
        chosen.is_ok() && cleared.is_ok(),
        "",
    );
    let folder = crate::selftest::helpers::folder("message-project");
    std::thread::spawn(move || {
        let made = make_repository(&folder);
        main_soon(move || match made {
            Ok(()) => open(folder),
            Err(error) => {
                check("a test repository is made", false, error);
                finish(SCENARIO);
            }
        });
    });
}

/// Three commits, then a staged change of the README and an unstaged one of the library.
fn make_repository(folder: &Path) -> Result<(), String> {
    let git = fixture_git()?;
    std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
    let repo = git.init(folder).map_err(|e| e.to_string())?;
    super::append(
        &repo.git_dir().join("config"),
        "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n",
    )?;
    for (n, subject) in ["Start the project", "Add the library", "Fix a typo"]
        .iter()
        .enumerate()
    {
        write(folder, README, &format!("# Project\n\nstep {n}\n"))?;
        write(folder, LIBRARY, &format!("pub fn step() -> u32 {{ {n} }}\n"))?;
        repo.stage_all().map_err(|e| e.to_string())?;
        repo.commit(subject, CommitOptions::default())
            .map_err(|e| e.to_string())?;
    }
    write(folder, README, "# Project\n\nWhat it does, explained.\n")?;
    repo.stage(&[README]).map_err(|e| e.to_string())?;
    write(folder, LIBRARY, "pub fn step() -> u32 { 4 }\n")
}

fn open(folder: PathBuf) {
    let Some((root, _)) = super::open_repository(&folder, SCENARIO) else {
        return;
    };
    wait_on(
        root,
        |c| panel::seen(c).is_some_and(|s| s.staged.len() == 1 && s.changes.len() == 1),
        listed,
    );
}

fn listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    probe::set_draft(&controller, DRAFT);
    let seen = probe::message(&controller);
    check(
        "the button in the commit field and its menu item are on with a model and changes",
        ok && seen.button && probe::can(&controller, sel!(ideGitMessage:)),
        seen.tip,
    );
    check(
        "nothing sent yet: the sent content cannot be shown",
        !probe::can(&controller, sel!(ideGitShowSent:)),
        "",
    );
    probe::generate(&controller);
    wait_on(root.clone(), sheet_ready, consent_asked);
}

fn consent_asked(root: PathBuf, ok: bool) {
    let says = sheet_labels(&root);
    check(
        "the first request to a provider outside the Mac asks and says what goes there",
        ok && says.contains(&tf("Änderungen an {} senden?", &["Stub"])) && says.contains("gitleaks"),
        says.lines().next().unwrap_or("").to_string(),
    );
    check("nothing went out before the answer", requests().is_empty(), "");
    super::sheet_pictures(root, "git-message-consent", |root| {
        if !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
            return finish(SCENARIO);
        }
        wait_on(
            root.clone(),
            |c| {
                let seen = probe::message(c);
                seen.running && !seen.text.is_empty() && seen.text != written()
            },
            streaming,
        );
    });
}

fn streaming(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = probe::message(&controller);
    check(
        "the message appears piece by piece while the field takes no typing",
        ok && written().starts_with(&seen.text) && !seen.editable,
        seen.text.clone(),
    );
    check(
        "the progress says concretely what happens and offers to cancel",
        seen.progress == t("Fasst 1 geänderte Datei zusammen …") && panel::seen(&controller).is_some_and(|s| s.busy),
        seen.progress,
    );
    wait_on(root.clone(), |c| !probe::message(c).running, done);
}

fn done(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = probe::message(&controller);
    check(
        "the whole message is in the field, editable, and says that AI wrote it",
        ok && seen.text == written() && seen.editable && seen.written,
        format!("{:?}, written {}", seen.text, seen.written),
    );
    let requests = requests();
    let body = requests.first().map(|r| r.json()).unwrap_or_default();
    let content = body["messages"][1]["content"].as_str().unwrap_or("").to_string();
    let sent = seen.sent.clone().unwrap_or_default();
    check(
        "exactly the checked text went out, once",
        requests.len() == 1 && !sent.is_empty() && content == sent,
        format!("{} requests, {} characters", requests.len(), content.len()),
    );
    check(
        "the request holds the staged diff, the branch and the subjects, not the unstaged change",
        sent.contains("+What it does, explained.")
            && sent.contains("Branch: ")
            && sent.contains("- Fix a typo")
            && !sent.contains("{ 4 }"),
        "",
    );
    pictures(root, "git-message", |root| {
        let Some(controller) = window_of(&root) else {
            return finish(SCENARIO);
        };
        probe::show_sent(&controller);
        wait_on(root.clone(), sheet_ready, sent_shown);
    });
}

fn sent_shown(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let shown = sheet_text(&root);
    let sent = probe::message(&controller).sent.unwrap_or_default();
    check(
        "Gesendeten Inhalt zeigen shows exactly the checked text",
        ok && shown == sent,
        "",
    );
    super::sheet_pictures(root, "git-message-sent", |root| {
        answer_sheet(&root, NSAlertFirstButtonReturn, None);
        let Some(controller) = window_of(&root) else {
            return finish(SCENARIO);
        };
        probe::undo(&controller);
        let seen = probe::message(&controller);
        check("one undo brings the draft back", seen.text == DRAFT, seen.text);
        probe::generate(&controller);
        wait_on(
            root.clone(),
            |c| !probe::message(c).running && probe::message(c).text == written(),
            regenerated,
        );
    });
}

fn regenerated(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "Neu erzeugen writes again without asking once more",
        ok && requests().len() == 2 && !controller.has_sheet(),
        format!("{} requests", requests().len()),
    );
    probe::type_text(&controller, "\n\nMore words.");
    let seen = probe::message(&controller);
    check(
        "the user's first change ends the note that AI wrote the message",
        !seen.written && seen.text.ends_with("More words."),
        "",
    );
    let typed = seen.text;
    probe::generate(&controller);
    wait_on(
        root.clone(),
        |c| {
            let seen = probe::message(c);
            seen.running && !seen.editable && !seen.text.is_empty()
        },
        move |root, ok| {
            if let Some(controller) = window_of(&root) {
                probe::cancel(&controller);
            }
            wait_on(
                root.clone(),
                |c| !probe::message(c).running,
                move |root, done| cancelled(root, ok && done, typed),
            );
        },
    );
}

fn cancelled(root: PathBuf, ok: bool, typed: String) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = probe::message(&controller);
    check(
        "cancelling while it writes puts back the message from before",
        ok && seen.text == typed && seen.editable && !seen.written,
        seen.text,
    );
    std::thread::spawn(move || {
        let done = plant_secret(&root);
        main_soon(move || match done {
            Ok(()) => wait_on(
                root.clone(),
                |c| panel::seen(c).is_some_and(|s| s.staged.is_empty() && !s.busy),
                with_secret,
            ),
            Err(error) => {
                check("a secret is planted in an unstaged change", false, error);
                finish(SCENARIO);
            }
        });
    });
}

/// Unstages everything and puts an access key into the library, which is then only an unstaged change.
fn plant_secret(root: &Path) -> Result<(), String> {
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    repo.unstage_all().map_err(|e| e.to_string())?;
    write(root, LIBRARY, &format!("pub fn step() -> u32 {{ 4 }}\n{FAKE_KEY}"))
}

fn with_secret(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("the panel shows nothing staged", ok, "");
    probe::generate(&controller);
    wait_on(
        root.clone(),
        |c| !probe::message(c).running && panel::seen(c).is_some_and(|s| s.problem.is_some() && !s.busy),
        secret_found,
    );
}

fn secret_found(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let problem = panel::seen(&controller).and_then(|s| s.problem).unwrap_or_default();
    let seen = probe::message(&controller);
    check(
        "a secret in an unstaged change sends nothing and names file and line",
        ok && requests().len() == 3
            && problem
                == tf(
                    "Nichts gesendet: Möglicherweise ein Secret in {} ({}).",
                    &[&format!("{LIBRARY}:2"), "aws-access-token"],
                ),
        problem.clone(),
    );
    check(
        "the button says why nothing went out",
        seen.tip == problem && seen.text.ends_with("More words."),
        seen.tip,
    );
    pictures(root, "git-message-secret", |root| {
        let Some(mtm) = objc2::MainThreadMarker::new() else {
            return;
        };
        crate::settings::show_agent(mtm);
        wait_on(
            root.clone(),
            |_| crate::settings::commit_message_selftest::allowed().is_some(),
            revoke,
        );
    });
}

fn revoke(root: PathBuf, ok: bool) {
    let allowed = crate::settings::commit_message_selftest::allowed();
    check(
        "the settings name the provider the user allowed and can take it back",
        ok && allowed.as_ref().is_some_and(|(text, on)| text.contains("Stub") && *on),
        format!("{allowed:?}"),
    );
    if let Some(window) = crate::settings::window() {
        snapshot(&window, "git-message-settings");
        window.close();
    }
    crate::settings::commit_message_selftest::revoke();
    let after_revoke = crate::settings::commit_message_selftest::allowed();
    check(
        "taking the consent back leaves no provider allowed",
        after_revoke.is_some_and(|(_, on)| !on) && Settings::load_user().get(&CONSENT).is_empty(),
        "",
    );
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    if let Some(window) = controller.window() {
        window.makeKeyAndOrderFront(None);
    }
    std::fs::write(root.join(LIBRARY), "pub fn step() -> u32 { 5 }\n").ok();
    probe::generate(&controller);
    wait_on(root.clone(), sheet_ready, asked_again);
}

fn asked_again(root: PathBuf, ok: bool) {
    check(
        "without the consent the next request asks again",
        ok,
        sheet_labels(&root),
    );
    super::answer_sheet(&root, objc2_app_kit::NSAlertSecondButtonReturn, None);
    let Some(window) = window_of(&root).and_then(|c| c.window()) else {
        return finish(SCENARIO);
    };
    check("cancelling the consent sends nothing", requests().len() == 3, "");
    let findings = crate::selftest::a11y::audit::findings(&crate::selftest::a11y::audit::elements(&window));
    check(
        "the accessibility audit finds nothing in the window with the commit message",
        findings.is_empty(),
        findings
            .iter()
            .map(|f| format!("{}: {} ({})", f.problem, f.what, f.path))
            .collect::<Vec<_>>()
            .join("; "),
    );
    finish(SCENARIO);
}
