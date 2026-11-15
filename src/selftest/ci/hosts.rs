//! The `ci-settings` scenario: the CI tab of the settings lists the host rules and OAuth apps; removing the GitLab
//! stub's rule makes its host unknown to the open panel, adding it back through the sheet, given as a URL, makes it
//! known again, a host the app cannot use is refused, and removing an OAuth app forgets it.

use std::path::PathBuf;
use std::time::Duration;

use ide_core::settings::Settings;
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertFirstButtonReturn;

use super::super::git::wait_for;
use super::super::helpers;
use super::super::{check, finish, give_up, give_up_saying, snapshot};
use super::stubs;
use crate::ci_panel::{CI_HOSTS, CI_OAUTH_CLIENTS, selftest as panel};
use crate::settings::{CiList, ci_selftest as tab};
use crate::window::WindowController;

pub(super) const SCENARIO: &str = "ci-settings";
/// Long enough for the settings window to draw itself again in the other appearance.
const SETTLE: Duration = Duration::from_millis(600);

fn gitlab_place() -> String {
    format!("127.0.0.1:{}/gitlab", stubs().gitlab.port)
}

pub(super) fn start(root: PathBuf, ok: bool) {
    check("the GitLab stub's host rule lets the panel ask to sign in", ok, "");
    let app = format!("127.0.0.1:{}=selftest-app", stubs().gitlab.port);
    if let Err(error) = Settings::load_user().set(&CI_OAUTH_CLIENTS, &vec![app]) {
        return give_up_saying(SCENARIO, "an OAuth app is set up", error.to_string());
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show_ci(mtm);
    helpers::wait_until(|| crate::settings::ci_tab().is_some(), move |_| listed(root));
}

fn listed(root: PathBuf) {
    let Some(owner) = crate::settings::ci_tab() else {
        return give_up(SCENARIO, "the settings show a CI tab");
    };
    let (hosts, host_rows) = tab::rows(&owner, CiList::Hosts);
    let (apps, app_rows) = tab::rows(&owner, CiList::Apps);
    let github = format!("127.0.0.1:{}", stubs().github.port);
    check(
        "the CI tab lists both host rules with their service and the OAuth app",
        host_rows == 2
            && hosts.contains(&[github, "GitHub".into()])
            && hosts.contains(&[gitlab_place(), "GitLab".into()])
            && app_rows == 1
            && apps[0][1] == "selftest-app",
        format!("{hosts:?} {apps:?}"),
    );
    let bars = crate::settings::window()
        .map(|w| super::super::helpers::list_bars(&w))
        .unwrap_or_default();
    check(
        "each list's + and − sit right under it with macOS's own pictures",
        bars.len() == 2 && bars.iter().all(|(gap, names)| native_bar(*gap, names)),
        format!("{bars:?}"),
    );
    settings_pictures("settings-ci", move || {
        if let Some(owner) = crate::settings::ci_tab() {
            let (hosts, _) = tab::rows(&owner, CiList::Hosts);
            let row = hosts.iter().position(|r| r[0] == gitlab_place()).unwrap_or(0);
            tab::remove(&owner, CiList::Hosts, row);
        }
        wait_for(root, |c| phase(c) == "Unknown", host_removed);
    });
}

fn phase(controller: &WindowController) -> String {
    panel::seen(controller).map(|s| s.phase).unwrap_or_default()
}

fn rules() -> Vec<String> {
    Settings::load_user().get(&CI_HOSTS)
}

fn host_removed(root: PathBuf, ok: bool) {
    check(
        "removing a host rule makes its host unknown to the open panel",
        ok && rules().len() == 1,
        format!("{:?}", rules()),
    );
    super::super::git::pictures(root.clone(), "ci-unknown", add_host);
}

fn add_host(root: PathBuf) {
    if let Some(owner) = crate::settings::ci_tab() {
        tab::click_add(&owner);
    }
    wait_for(
        root,
        |_| settings_sheet(),
        |root, ok| {
            let answered = ok
                && crate::settings::ci_tab().is_some_and(|owner| {
                    tab::answer_add(
                        &owner,
                        &format!(" http://{}/ ", gitlab_place()),
                        1,
                        NSAlertFirstButtonReturn,
                    )
                });
            check("the add button asks for a host in a sheet", answered, "");
            wait_for(root, |c| phase(c) == "SignIn", host_added);
        },
    );
}

fn settings_sheet() -> bool {
    crate::settings::window().is_some_and(|w| w.attachedSheet().is_some())
}

fn host_added(root: PathBuf, ok: bool) {
    let entry = format!("{}=gitlab", gitlab_place());
    check(
        "a host given as a URL is kept as a rule and the panel knows its service again",
        ok && rules().contains(&entry) && rules().len() == 2,
        format!("{:?}", rules()),
    );
    if let Some(owner) = crate::settings::ci_tab() {
        tab::click_add(&owner);
    }
    wait_for(
        root,
        |_| settings_sheet(),
        |_root, ok| {
            let answered = ok
                && crate::settings::ci_tab()
                    .is_some_and(|owner| tab::answer_add(&owner, "bad host!", 0, NSAlertFirstButtonReturn));
            let problem = crate::settings::ci_tab().and_then(|owner| tab::problem(&owner));
            check(
                "a host the app cannot use is refused with a reason",
                answered && problem.as_deref().is_some_and(|p| p.contains("bad host!")) && rules().len() == 2,
                format!("{problem:?}"),
            );
            settings_pictures("settings-ci-problem", remove_app);
        },
    );
}

fn remove_app() {
    let Some(owner) = crate::settings::ci_tab() else {
        return give_up(SCENARIO, "the settings show the CI page");
    };
    tab::remove(&owner, CiList::Apps, 0);
    let (apps, shown) = tab::rows(&owner, CiList::Apps);
    let kept: Vec<String> = Settings::load_user().get(&CI_OAUTH_CLIENTS);
    check(
        "removing an OAuth app forgets it, so the next browser login asks again",
        apps.is_empty() && shown == 0 && kept.is_empty(),
        format!("{kept:?}"),
    );
    finish(SCENARIO);
}

/// Whether a list's buttons touch its border or overlap it, and the first ones show AppKit's add and remove pictures.
pub(crate) fn native_bar(gap: f64, names: &[String]) -> bool {
    (-2.5..=0.5).contains(&gap)
        && !names.is_empty()
        && names
            .iter()
            .take_while(|name| name.ends_with("Template"))
            .all(|name| name == "NSAddTemplate" || name == "NSRemoveTemplate")
        && names.first().is_some_and(|name| name.ends_with("Template"))
}

/// Pictures of the settings window in light and dark appearance, then `then`.
fn settings_pictures(name: &'static str, then: impl FnOnce() + 'static) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if let Some(window) = crate::settings::window() {
        snapshot(&window, name);
    }
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    let dark =
        objc2_app_kit::NSAppearance::appearanceNamed(crate::util::constant!(objc2_app_kit::NSAppearanceNameDarkAqua));
    app.setAppearance(dark.as_deref());
    helpers::after(SETTLE, move || {
        if let Some(window) = crate::settings::window() {
            snapshot(&window, &format!("{name}-dark"));
        }
        if let Some(mtm) = MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(mtm).setAppearance(None);
        }
        helpers::after(SETTLE, then);
    });
}
