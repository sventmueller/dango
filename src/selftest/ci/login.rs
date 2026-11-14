//! The `ci-login` scenario, signing in through the browser on the stubs. GitLab: the browser button, the app id asked
//! for once, the port the browser returns to taken by another program, a second window that takes the waiting login
//! over and lets the port go when it closes, the authorization with PKCE whose return the test plays as the browser
//! would, and signing out. GitHub: the device code sheet, stopping it, and the flow through to a token.

use ide_core::LockIgnoringPoison;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use ide_ci::oauth;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAlertSecondButtonReturn};
use splat_i18n::t;

use super::super::git::{answer_sheet, pictures, sheet_pictures, wait_for, window_of};
use super::super::helpers::wait_on_within;
use super::super::{check, finish};
use super::stub::{DEVICE_TOKEN, USER_CODE};
use super::{TOKEN, describe, stubs};
use crate::ci_panel::selftest as panel;
use crate::window::{Panel, WindowController};

pub(super) const SCENARIO: &str = "ci-login";
const GITLAB_APP: &str = "selftest-app";
const GITHUB_APP: &str = "selftest-gh";
/// How soon a closing window must let the callback port go.
const RELEASE: Duration = Duration::from_secs(3);

/// Holds the port the browser returns to, as another program would.
static BLOCKER: Mutex<Option<TcpListener>> = Mutex::new(None);

pub(super) fn start(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let phase = panel::seen(&controller).map(|s| s.phase).unwrap_or_default();
    check(
        "a GitLab that wants a token asks to sign in",
        ok && phase == "SignIn",
        describe(&controller),
    );
    check(
        "a self-managed GitLab offers the browser login with an app the user names",
        panel::browser_button(&controller) == Some(true),
        format!("{:?}", panel::browser_button(&controller)),
    );
    match TcpListener::bind(("127.0.0.1", oauth::CALLBACK_PORT)) {
        Ok(listener) => *BLOCKER.lock_ignoring_poison() = Some(listener),
        Err(error) => check("the browser's port is free for the test", false, error.to_string()),
    }
    panel::browser_login(&controller);
    wait_for(root, WindowController::has_sheet, app_sheet);
}

fn app_sheet(root: PathBuf, ok: bool) {
    check("the browser login asks for the host's app id first", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, Some(GITLAB_APP)) {
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| {
            panel::seen(c)
                .and_then(|s| s.problem)
                .is_some_and(|p| p.contains(&oauth::CALLBACK_PORT.to_string()))
        },
        port_taken,
    );
}

fn port_taken(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "a taken return port says so and no login waits",
        ok && panel::login(&controller).is_none(),
        describe(&controller),
    );
    pictures(root, "ci-login-port", |root| {
        BLOCKER.lock_ignoring_poison().take();
        if let Some(controller) = window_of(&root) {
            panel::browser_login(&controller);
        }
        wait_for(root, |_| authorize_url().is_some(), authorize_opened);
    });
}

/// The last sign-in page the panel opened for GitLab.
fn authorize_url() -> Option<String> {
    panel::opened()
        .into_iter()
        .rev()
        .find(|url| url.contains("/gitlab/oauth/authorize?"))
}

fn query(url: &str, name: &str) -> Option<String> {
    url.split_once('?')?
        .1
        .split('&')
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        .map(str::to_string)
}

fn authorize_opened(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let url = authorize_url().unwrap_or_default();
    let fitting = query(&url, "client_id").as_deref() == Some(GITLAB_APP)
        && query(&url, "redirect_uri").as_deref() == Some("http%3A%2F%2F127.0.0.1%3A47819%2Fcallback")
        && query(&url, "code_challenge_method").as_deref() == Some("S256")
        && query(&url, "scope").as_deref() == Some("api");
    check(
        "the browser opens GitLab's sign-in with the remembered app id, the loopback return and a S256 challenge",
        ok && fitting && !controller.has_sheet() && panel::login(&controller) == Some(None),
        &url,
    );
    let account = panel::account_menu(&controller);
    check(
        "while the browser is out the account menu says so and offers to stop",
        account
            .iter()
            .any(|(line, _)| line == t("Warte auf die Anmeldung im Browser …"))
            && account.last() == Some(&(t("Abbrechen").to_string(), true)),
        format!("{account:?}"),
    );
    pictures(root, "ci-login-wait", |root| {
        let twin = crate::selftest::helpers::folder("ci-gitlab-twin");
        match copy_tree(&root, &twin).and_then(|()| twin.canonicalize()) {
            Ok(twin) => open_beside(twin),
            Err(error) => {
                check("a second window on the same repository opens", false, error.to_string());
                finish(SCENARIO);
            }
        }
    });
}

/// Opens `twin` in a window of its own; opening a project otherwise replaces the one in the front window.
fn open_beside(twin: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&twin);
    WindowController::open(mtm, Some(twin.clone())).activate_panel(Panel::Ci);
    wait_for(twin, super::ready, twin_ready);
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// How many GitLab sign-in pages the panels opened.
fn authorize_count() -> usize {
    panel::opened()
        .iter()
        .filter(|url| url.contains("/gitlab/oauth/authorize?"))
        .count()
}

fn twin_ready(twin: PathBuf, ok: bool) {
    let Some(controller) = window_of(&twin) else {
        return finish(SCENARIO);
    };
    let phase = panel::seen(&controller).map(|s| s.phase).unwrap_or_default();
    check(
        "a second window on the same GitLab asks to sign in",
        ok && phase == "SignIn",
        describe(&controller),
    );
    panel::browser_login(&controller);
    wait_for(
        twin,
        |c| authorize_count() >= 2 && panel::login(c) == Some(None),
        taken_over,
    );
}

fn taken_over(twin: PathBuf, ok: bool) {
    let first = window_of(&stubs().gitlab_root);
    let stopped = first.as_ref().is_some_and(|c| {
        panel::login(c).is_none()
            && panel::seen(c).and_then(|s| s.problem).as_deref()
                == Some(t(
                    "Die Anmeldung wurde beendet, weil in einem anderen Fenster eine begonnen hat.",
                ))
    });
    check(
        "a login in another window stops the waiting one, says so there, and gets the port without a wait",
        ok && stopped,
        first.as_ref().map(|c| describe(c)).unwrap_or_default(),
    );
    if let Some(window) = window_of(&twin).and_then(|c| c.window()) {
        window.performClose(None);
    }
    wait_on_within(stubs().gitlab_root.clone(), RELEASE, |_| port_free(), port_released);
}

fn port_free() -> bool {
    TcpListener::bind(("127.0.0.1", oauth::CALLBACK_PORT)).is_ok()
}

fn port_released(root: PathBuf, ok: bool) {
    check(
        "closing a window stops its waiting login and lets the callback port go at once",
        ok,
        format!("within {} s", RELEASE.as_secs()),
    );
    if let Some(controller) = window_of(&root) {
        panel::browser_login(&controller);
    }
    wait_for(
        root,
        |c| authorize_count() >= 3 && panel::login(c) == Some(None),
        signing_in_again,
    );
}

fn signing_in_again(root: PathBuf, ok: bool) {
    check("the first window starts its login again afterwards", ok, "");
    let state = authorize_url().and_then(|url| query(&url, "state")).unwrap_or_default();
    std::thread::spawn(move || browser_returns(&format!("/callback?code=selftest-code&state={state}")));
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.phase == "Ready" && s.account.as_deref() == Some("lab-selftest")),
        gitlab_signed_in,
    );
}

/// What the browser sends when GitLab sends it back to the app.
fn browser_returns(target: &str) {
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", oauth::CALLBACK_PORT)) {
        let _ = write!(stream, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        let _ = stream.read_to_string(&mut String::new());
    }
}

fn gitlab_signed_in(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let exchange = stubs()
        .gitlab
        .seen()
        .into_iter()
        .find(|s| s.target == "/gitlab/oauth/token")
        .map(|s| s.body)
        .unwrap_or_default();
    let challenge = authorize_url().and_then(|url| query(&url, "code_challenge"));
    let verifier = query(&format!("?{exchange}"), "code_verifier").unwrap_or_default();
    check(
        "the browser's code and the verifier of its challenge buy a token, and the pipeline shows again",
        ok && exchange.contains("grant_type=authorization_code")
            && exchange.contains("code=selftest-code")
            && challenge == Some(oauth::challenge(&verifier)),
        format!("{}; exchange {exchange:?}", describe(&controller)),
    );
    let bearer = format!("Bearer {TOKEN}");
    let used = stubs()
        .gitlab
        .seen()
        .iter()
        .rev()
        .take(3)
        .any(|s| s.authorization.as_deref() == Some(&bearer));
    check("the token from the browser login reads the API", used, "");
    pictures(root, "ci-login-done", |root| {
        if let Some(controller) = window_of(&root) {
            panel::sign_out(&controller);
        }
        wait_for(
            root,
            |c| panel::seen(c).is_some_and(|s| s.phase == "SignIn"),
            signed_out,
        );
    });
}

fn signed_out(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "signing out forgets the token and asks to sign in again",
        ok,
        describe(&controller),
    );
    super::open(stubs().github_root.clone(), github_open);
}

fn github_open(root: PathBuf, ok: bool) {
    check("the GitHub repository's runs are read", ok, "");
    if let Some(controller) = window_of(&root) {
        panel::browser_login(&controller);
    }
    wait_for(root, WindowController::has_sheet, github_app_sheet);
}

fn github_app_sheet(root: PathBuf, ok: bool) {
    check("GitHub Enterprise asks for the host's OAuth app first", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, Some(GITHUB_APP)) {
        return finish(SCENARIO);
    }
    wait_for(root, code_shown, code_sheet);
}

fn code_shown(controller: &WindowController) -> bool {
    controller.has_sheet() && panel::login(controller) == Some(Some(USER_CODE.to_string()))
}

fn code_sheet(root: PathBuf, ok: bool) {
    check("the device flow shows its code in a sheet", ok, "");
    if !ok {
        return finish(SCENARIO);
    }
    sheet_pictures(root, "ci-device-code", |root| {
        answer_sheet(&root, NSAlertSecondButtonReturn, None);
        wait_for(root, |c| panel::login(c).is_none(), code_cancelled);
    });
}

fn code_cancelled(root: PathBuf, ok: bool) {
    let polled = stubs()
        .github
        .seen()
        .iter()
        .any(|s| s.target == "/login/oauth/access_token");
    check(
        "stopping at the code ends the login before it asks for a token",
        ok && !polled,
        "",
    );
    if let Some(controller) = window_of(&root) {
        panel::browser_login(&controller);
    }
    wait_for(root, code_shown, code_again);
}

fn code_again(root: PathBuf, ok: bool) {
    check("the second login skips the app id it remembered", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.account.as_deref() == Some("octo-selftest")),
        github_signed_in,
    );
}

fn github_signed_in(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let noted = panel::opened();
    let seen = stubs().github.seen();
    let asked = seen
        .iter()
        .filter(|s| s.target == "/login/device/code")
        .map(|s| s.body.clone())
        .collect::<Vec<_>>();
    let polls = seen.iter().filter(|s| s.target == "/login/oauth/access_token").count();
    check(
        "the code goes to the pasteboard, the device page opens, and polling past \"pending\" gives a token",
        ok && noted.contains(&format!("pasteboard:{USER_CODE}"))
            && noted.iter().any(|u| u.ends_with("/login/device"))
            && asked
                .iter()
                .all(|b| b == &format!("client_id={GITHUB_APP}&scope=repo%20workflow"))
            && polls == 2,
        format!("{}; asked {asked:?}, {polls} polls", describe(&controller)),
    );
    let bearer = format!("Bearer {DEVICE_TOKEN}");
    let own_host = seen.iter().any(|s| s.authorization.as_deref() == Some(&bearer))
        && stubs()
            .gitlab
            .seen()
            .iter()
            .all(|s| s.authorization.as_deref() != Some(&bearer));
    check("the device flow's token reads its own host only", own_host, "");
    pictures(root, "ci-device-done", |_| finish(SCENARIO));
}
