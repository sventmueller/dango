//! The `language-servers` scenario: the settings' language servers tab lists every built-in language with where its
//! server is or what is missing; a server added through the sheet takes the files of its extensions, one without a
//! command is refused with a reason, and removing it gives the files back.

use ide_core::settings::Settings;
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertFirstButtonReturn;
use std::path::Path;

use super::helpers::wait_until;
use super::{check, finish, snapshot};
use crate::settings::servers_selftest as tab;
use ide_ui_lsp::{LANGUAGE_SERVERS, language_name};

const NAME: &str = "language-servers";

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show_servers(mtm);
    let count = isize::try_from(ide_lsp::BUILT_IN.len()).unwrap_or(0);
    wait_until(
        move || crate::settings::servers_tab().is_some_and(|owner| tab::rows(&owner, false).1 == count),
        listed,
    );
}

fn listed(ok: bool) {
    let Some(owner) = crate::settings::servers_tab() else {
        check("the settings show a language servers tab", false, "");
        return finish(NAME);
    };
    let (rows, _) = tab::rows(&owner, false);
    check(
        "the tab lists every built-in language with where its server is or what is missing",
        ok && rows.iter().all(|row| row.len() == 2 && !row[1].is_empty())
            && rows.iter().any(|row| row[0] == "Python")
            && rows.iter().any(|row| row[0] == "C/C++"),
        format!("{rows:?}"),
    );
    if let Some(window) = crate::settings::window() {
        snapshot(&window, "settings-servers");
    }
    tab::click_add(&owner);
    wait_until(sheet_shown, |_| refuse());
}

fn refuse() {
    let Some(owner) = crate::settings::servers_tab() else {
        return finish(NAME);
    };
    let answered = tab::answer_add(&owner, ["Elixir", "ex", "  "], NSAlertFirstButtonReturn);
    let problem = tab::problem(&owner);
    check(
        "a server without a command is refused with a reason",
        answered && problem.is_some() && tab::rows(&owner, true).1 == 0,
        format!("{problem:?}"),
    );
    tab::click_add(&owner);
    wait_until(sheet_shown, |_| add());
}

fn add() {
    let Some(owner) = crate::settings::servers_tab() else {
        return finish(NAME);
    };
    let answered = tab::answer_add(
        &owner,
        ["Elixir", ".ex, exs", "elixir-ls --stdio"],
        NSAlertFirstButtonReturn,
    );
    let (rows, shown) = tab::rows(&owner, true);
    let stored = Settings::load_user().get(&LANGUAGE_SERVERS);
    check(
        "an added server is listed and kept in the user's settings",
        answered
            && shown == 1
            && rows[0] == ["Elixir", "ex, exs", "elixir-ls --stdio"]
            && stored.first().is_some_and(|s| s.extensions == ["ex", "exs"]),
        format!("{rows:?} {stored:?}"),
    );
    check(
        "the added server takes the files of its extensions, the built-in ones keep theirs",
        language_name(Path::new("lib/app.exs")).as_deref() == Some("Elixir")
            && language_name(Path::new("main.py")).as_deref() == Some("Python"),
        format!("{:?}", language_name(Path::new("lib/app.exs"))),
    );
    if let Some(window) = crate::settings::window() {
        snapshot(&window, "settings-servers-custom");
    }
    tab::remove(&owner, 0);
    wait_until(given_back, |_| removed());
}

/// The settings window shows the sheet of a server to add.
fn sheet_shown() -> bool {
    crate::settings::window().is_some_and(|w| w.attachedSheet().is_some())
}

fn shown_custom() -> isize {
    crate::settings::servers_tab().map_or(-1, |owner| tab::rows(&owner, true).1)
}

fn given_back() -> bool {
    shown_custom() == 0
        && Settings::load_user().get(&LANGUAGE_SERVERS).is_empty()
        && language_name(Path::new("lib/app.exs")).is_none()
}

fn removed() {
    check(
        "removing the server gives its files back",
        given_back(),
        format!("{}", shown_custom()),
    );
    finish(NAME);
}
