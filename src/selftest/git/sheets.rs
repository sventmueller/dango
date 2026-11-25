//! The commit sheets of the `git` scenario: a pre-commit hook that refuses, whose output a sheet shows, and a staged
//! access key, which the secrets check lists in a sheet whose default is to cancel. The hook runs because the
//! scenario trusts its folder.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2_app_kit::{NSAlertFirstButtonReturn, NSAlertSecondButtonReturn};
use splat_i18n::{t, tf};

use super::{
    SCENARIO, answer_sheet, fixture_git, has, in_background, sheet_default_button, sheet_ready, sheet_text, wait_for,
    window_of, write,
};
use crate::selftest::{check, give_up, give_up_saying};
use crate::window::WindowController;
use ide_ui_git::selftest as panel;

const HOOKED: &str = "hooked.txt";
const HOOK_SAYS: &str = "lint: 2 problems in hooked.txt";
const CREDENTIALS: &str = "credentials.txt";
/// An access key in the shape gitleaks' `aws-access-token` rule looks for; it opens nothing.
const FAKE_KEY: &str = "aws_access_key_id = AKIAQYLPMN5HHHFPZAM2\n";
const SECRET_SUBJECT: &str = "Add credentials";
/// How long a sheet takes to slide in and draw.
const SHEET_SETTLE: Duration = Duration::from_millis(800);

pub(crate) fn start(root: PathBuf) {
    let made =
        write_hook(&root, &format!("echo '{HOOK_SAYS}' >&2\nexit 1")).and_then(|()| write(&root, HOOKED, "hook\n"));
    if let Err(error) = made {
        return give_up_saying(SCENARIO, "a refusing pre-commit hook is set up", error);
    }
    in_background(
        root,
        |root| stage(root, HOOKED),
        |root| wait_for(root, |c| staged(c, HOOKED), commit_into_hook),
    );
}

fn write_hook(root: &Path, body: &str) -> Result<(), String> {
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(root.join(".git/hooks")).map_err(|e| e.to_string())?;
    std::fs::write(&hook, format!("#!/bin/sh\n{body}\n")).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

fn stage(root: &Path, file: &str) -> Result<(), String> {
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    repo.stage(&[file]).map_err(|e| e.to_string())
}

fn staged(controller: &WindowController, file: &str) -> bool {
    panel::seen(controller).is_some_and(|s| !s.busy && has(&s.staged, file))
}

fn top(controller: &WindowController) -> Option<String> {
    panel::seen(controller).and_then(|s| s.top).map(|(subject, _)| subject)
}

fn commit_into_hook(root: PathBuf, ok: bool) {
    check("the panel lists a file staged behind its back", ok, HOOKED);
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let before = top(&controller);
    panel::commit(&controller, "Hooked change");
    wait_for(root, sheet_ready, move |root, ok| {
        settle(root, ok, move |root, ok| hook_sheet(root, ok, before));
    });
}

/// Goes on with `then` once a sheet whose buttons are in place has also drawn itself, for its picture.
fn settle(root: PathBuf, ok: bool, then: impl FnOnce(PathBuf, bool) + 'static) {
    super::super::helpers::after(SHEET_SETTLE, move || then(root, ok));
}

/// The picture `name` of the window of `root` with its sheet, taken from the screen: a capture of the sheet's own
/// window comes out empty.
fn sheet_picture(root: &Path, name: &str) {
    let (Some(window), Some(path), Some(mtm)) = (
        window_of(root).and_then(|c| c.window()),
        crate::selftest::picture_path(name),
        objc2::MainThreadMarker::new(),
    ) else {
        return;
    };
    let Some(screen_height) = objc2_app_kit::NSScreen::screens(mtm)
        .firstObject()
        .map(|s| s.frame().size.height)
    else {
        return;
    };
    let frame = window.frame();
    let region = format!(
        "-R{},{},{},{}",
        frame.origin.x,
        screen_height - frame.origin.y - frame.size.height,
        frame.size.width,
        frame.size.height
    );
    crate::selftest::capture(&[&region], &path);
}

/// `before` is the newest commit's subject before the commits that are to be refused.
fn hook_sheet(root: PathBuf, ok: bool, before: Option<String>) {
    sheet_picture(&root, "git-hook-sheet");
    let text = sheet_text(&root);
    check(
        "a refusing pre-commit hook shows its output in a sheet",
        ok && text.contains(HOOK_SAYS),
        text.lines().next().unwrap_or("").to_string(),
    );
    if !ok {
        return give_up(SCENARIO, "a refusing pre-commit hook shows its output in a sheet");
    }
    if !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return give_up(SCENARIO, "the sheet is there to answer");
    }
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let seen = panel::seen(&controller);
    let unchanged = before == top(&controller);
    let named =
        seen.as_ref().and_then(|s| s.problem.clone()) == Some(tf("Der Hook „{}“ hat abgelehnt.", &["pre-commit"]));
    check(
        "the refused commit is not made, the file stays staged and the problem names the hook",
        unchanged && named && seen.as_ref().is_some_and(|s| has(&s.staged, HOOKED)),
        format!("problem {:?}", seen.and_then(|s| s.problem)),
    );
    if let Err(error) = write_hook(&root, "exit 0") {
        return give_up_saying(SCENARIO, "the pre-commit hook lets commits through again", error);
    }
    if !gitleaks_installed() {
        check(
            "the secrets sheet is tested where gitleaks is installed",
            true,
            "skipped: gitleaks is not on the login shell's PATH",
        );
        return super::history::start(root);
    }
    if let Err(error) = write(&root, CREDENTIALS, FAKE_KEY) {
        return give_up_saying(SCENARIO, "a file with an access key is written", error);
    }
    in_background(
        root,
        |root| stage(root, CREDENTIALS),
        move |root| {
            wait_for(
                root,
                |c| staged(c, CREDENTIALS),
                move |root, ok| commit_secret(root, ok, before),
            )
        },
    );
}

fn gitleaks_installed() -> bool {
    crate::environment::get()
        .and_then(|env| {
            env.iter()
                .find(|(name, _)| name == "PATH")
                .map(|(_, path)| path.clone())
        })
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("gitleaks").is_file()))
}

fn commit_secret(root: PathBuf, ok: bool, before: Option<String>) {
    if !ok {
        return give_up_saying(
            SCENARIO,
            "the panel lists the file with the access key staged",
            CREDENTIALS,
        );
    }
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    panel::commit(&controller, SECRET_SUBJECT);
    wait_for(root, sheet_ready, move |root, ok| {
        settle(root, ok, move |root, ok| secrets_sheet(root, ok, before));
    });
}

fn secrets_sheet(root: PathBuf, ok: bool, before: Option<String>) {
    sheet_picture(&root, "git-secrets-sheet");
    let text = sheet_text(&root);
    check(
        "the secrets check lists rule and place before a commit",
        ok && text.contains(&format!("{CREDENTIALS}:1")) && text.contains("aws-access-token"),
        text.lines().next().unwrap_or("").to_string(),
    );
    let default = sheet_default_button(&root);
    check(
        "Abbrechen is the default of the secrets sheet",
        default.as_deref() == Some(t("Abbrechen")),
        format!("{default:?}"),
    );
    if !ok {
        return give_up(SCENARIO, "the secrets check lists rule and place before a commit");
    }
    if !answer_sheet(&root, NSAlertSecondButtonReturn, None) {
        return give_up(SCENARIO, "the sheet is there to answer");
    }
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| !s.busy),
        move |root, ok| {
            secrets_cancelled(root, ok, before);
        },
    );
}

fn secrets_cancelled(root: PathBuf, ok: bool, before: Option<String>) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let unchanged = before == top(&controller);
    let staged = panel::seen(&controller).is_some_and(|s| has(&s.staged, CREDENTIALS));
    check(
        "cancelling at the secrets sheet leaves the commit undone and the file staged",
        ok && unchanged && staged,
        format!("top {:?}", top(&controller)),
    );
    panel::commit(&controller, SECRET_SUBJECT);
    wait_for(root, WindowController::has_sheet, secrets_again);
}

fn secrets_again(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return give_up(SCENARIO, "the secrets sheet asks again");
    }
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| !s.busy) && top(c).as_deref() == Some(SECRET_SUBJECT),
        committed_anyway,
    );
}

fn committed_anyway(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    check(
        "Trotzdem committen commits despite the finding",
        ok,
        format!("top {:?}", top(&controller)),
    );
    super::history::start(root);
}
