//! The second half of the `agent-actions` scenario: the AI panel's context menu, the overlays of a tab that cannot
//! run (no model, no sign-in, an untrusted folder), sessions without a tab opened from the panel, and the code of a
//! prompt rewound through the CLI's own file history.

use std::path::PathBuf;

use ide_thread::EntryKind;
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertFirstButtonReturn;

use super::agent::{controller_of, picture, tab_of};
use super::agent_actions::{answer_when_shown, answered, soon, stand_in, titles};
use super::agent_stand_in::SESSION;
use super::{check, finish, helpers};
use crate::window::Panel;

const NAME: &str = "agent-actions";

/// The AI panel's context menu renames, bookmarks and archives the session, and shows the archive.
pub(super) fn panel(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    controller.activate_panel(Panel::Agents);
    let row_of = |title: &str| {
        crate::agent_panel::inspect(&controller)
            .and_then(|seen| seen.rows.iter().position(|(_, _, t)| t == title))
            .and_then(|n| isize::try_from(n).ok())
    };
    let tabs = crate::agent::all().len();
    let Some(row) = row_of(&tab.title()) else {
        check("the AI panel lists the session", false, tab.title());
        return finish(NAME);
    };
    let menu = crate::agent_panel::session_menu(&controller, row);
    let shown = menu.as_ref().map(|m| titles(m)).unwrap_or_default();
    check(
        "the context menu of a bookmarked session renames, removes the bookmark, archives and shows the archive",
        shown
            == [
                "Umbenennen …",
                "Lesezeichen entfernen",
                "Archivieren",
                "",
                "Archiv zeigen",
            ],
        format!("{shown:?}"),
    );
    if let Some(menu) = &menu {
        menu.performActionForItemAtIndex(1);
    }
    answer_when_shown(NSAlertFirstButtonReturn, Some("Im Panel benannt"));
    let row = row_of(&tab.title()).unwrap_or(row);
    if let Some(menu) = crate::agent_panel::session_menu(&controller, row) {
        menu.performActionForItemAtIndex(0);
    }
    check(
        "the panel's menu takes the bookmark away and renames the open session",
        !crate::agent_panel::is_bookmarked(tab.thread_id()) && tab.title() == "Im Panel benannt",
        tab.title(),
    );
    let row = row_of("Im Panel benannt").unwrap_or(row);
    if let Some(menu) = crate::agent_panel::session_menu(&controller, row) {
        menu.performActionForItemAtIndex(2);
    }
    let was_archived = archived(tab.thread_id());
    if let Some(menu) = crate::agent_panel::session_menu(&controller, -1) {
        let only = titles(&menu);
        menu.performActionForItemAtIndex(0);
        check(
            "an archived session leaves the list; the menu beside the rows shows the archive",
            was_archived && only == ["Archiv zeigen"] && row_of("Im Panel benannt").is_some(),
            format!("{was_archived}, {only:?}"),
        );
    }
    if let Some(row) = row_of("Im Panel benannt") {
        let back = crate::agent_panel::session_menu(&controller, row)
            .map(|m| titles(&m))
            .unwrap_or_default();
        if let Some(menu) = crate::agent_panel::session_menu(&controller, row) {
            menu.performActionForItemAtIndex(2);
        }
        crate::agent_panel::open_session_row(&controller, usize::try_from(row).unwrap_or(0));
        check(
            "the archive's menu brings a session back, and a click on an open session shows its tab",
            back.get(2).map(String::as_str) == Some("Aus dem Archiv holen")
                && !archived(tab.thread_id())
                && crate::agent::all().len() == tabs,
            format!("{back:?}"),
        );
    }
    let leave = crate::agent_panel::session_menu(&controller, -1);
    let shown = leave.as_ref().map(|m| titles(m)).unwrap_or_default();
    if let Some(menu) = &leave {
        menu.performActionForItemAtIndex(0);
    }
    check(
        "the archive's own menu leads back to the sessions",
        shown == ["Zurück zu den Sitzungen"],
        format!("{shown:?}"),
    );
    picture(&controller, "agent-actions");
    no_model(root, key);
}

/// An own-agent tab without a provider says what is missing and leads to the settings.
fn no_model(root: PathBuf, key: u64) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(own) = crate::agent::open_own(&controller, None) else {
        return finish(NAME);
    };
    let own = own.key;
    soon(
        "the own agent says it has no model",
        move || tab_of(own).is_some_and(|t| t.problem_name() == Some("no model")),
        move || {
            let shown = tab_of(own).map(|t| t.problem_texts()).unwrap_or_default();
            check(
                "an own-agent tab without a provider says so and offers the settings",
                shown.iter().any(|t| t == "Noch kein Modell") && shown.iter().any(|t| t == "Einstellungen öffnen"),
                format!("{shown:?}"),
            );
            signed_out(root, key);
        },
    );
}

/// A CLI without a sign-in: the new tab says so, signs in in a terminal and starts once the CLI has one.
fn signed_out(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let answer = stand_in().dir.join("answer-initialize.json");
    let usual = std::fs::read_to_string(&answer).unwrap_or_default();
    let _ = std::fs::write(
        &answer,
        r#"{"commands": [], "models": [], "account": {"tokenSource": "none"}}"#,
    );
    let before = crate::agent::all().len();
    tab.new_session();
    let Some(new) = crate::agent::all()
        .last()
        .map(|t| t.key)
        .filter(|_| crate::agent::all().len() > before)
    else {
        check("a new session opens a tab", false, "");
        return finish(NAME);
    };
    soon(
        "the signed-out CLI asks for a sign-in",
        move || tab_of(new).is_some_and(|t| t.problem_name() == Some("sign in")),
        move || {
            let Some(tab) = tab_of(new) else { return finish(NAME) };
            let shown = tab.problem_texts();
            let terminals = controller_of(&root).map_or(0, |c| c.tab_titles().len());
            let signed = tab.press_problem("claude im Terminal anmelden");
            let opened = controller_of(&root).map_or(0, |c| c.tab_titles().len()) > terminals;
            let _ = std::fs::write(&answer, usual);
            tab.press_problem("Erneut versuchen");
            soon(
                "the CLI starts once it has a sign-in",
                move || tab_of(new).is_some_and(|t| t.is_live() && t.problem_name().is_none()),
                move || {
                    check(
                        "a CLI without a sign-in says so, signs in in a terminal of the folder and starts again",
                        shown.iter().any(|t| t == "Anmeldung nötig") && signed && opened,
                        format!("{shown:?}, terminal {opened}"),
                    );
                    reopen(root, key);
                },
            );
        },
    );
}

/// The id of a session the CLI saved from a terminal in the scenario's folder.
const TERMINAL_SESSION: &str = "51e1f7e5-6000-4000-8000-000000000001";

/// Sessions without a tab open from the AI panel: one of the app's, resumed, and one the CLI saved from a terminal,
/// with the conversation of its transcript.
fn reopen(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let title = tab.title();
    if let Some(id) = controller.tab_with_key(&tab.tab_key()) {
        controller.close_tab(id);
    }
    let project = crate::agent_panel::claude_config_dir()
        .join("projects")
        .join(ide_claude::history::project_folder(&root));
    let _ = std::fs::create_dir_all(&project);
    let _ = std::fs::write(
        project.join(format!("{TERMINAL_SESSION}.jsonl")),
        "{\"type\":\"user\",\"uuid\":\"51e1f7e5-6000-4000-8000-0000000000a1\",\"message\":{\"role\":\"user\",\"content\":\"Aus dem Terminal\"}}\n",
    );
    let (folder, wanted) = (root.clone(), title.clone());
    let listed = move || {
        controller_of(&folder).is_some_and(|controller| {
            controller.activate_panel(Panel::Agents);
            let rows = crate::agent_panel::inspect(&controller)
                .map(|seen| seen.rows)
                .unwrap_or_default();
            rows.iter()
                .any(|(kind, open, t)| *kind == "AI" && !*open && *t == wanted)
                && rows.iter().any(|(kind, _, _)| *kind == "CLI")
        })
    };
    helpers::wait_until(listed, move |_| {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        controller.activate_panel(Panel::Agents);
        let rows = crate::agent_panel::inspect(&controller)
            .map(|seen| seen.rows)
            .unwrap_or_default();
        let ours = rows
            .iter()
            .position(|(kind, open, t)| *kind == "AI" && !*open && *t == title);
        let terminal = rows.iter().position(|(kind, _, _)| *kind == "CLI");
        let (Some(ours), Some(terminal)) = (ours, terminal) else {
            check(
                "the panel lists the closed session and the terminal's",
                false,
                format!("{rows:?}"),
            );
            return finish(NAME);
        };
        let _ = std::fs::remove_file(stand_in().dir.join("args.txt"));
        crate::agent_panel::open_session_row(&controller, ours);
        soon(
            "the closed session opens again from the panel",
            || {
                stand_in().args().iter().any(|a| *a == format!("--resume={SESSION}"))
                    && crate::agent::all()
                        .iter()
                        .any(|t| t.session_id().as_deref() == Some(SESSION) && t.is_live())
            },
            move || {
                let _ = std::fs::remove_file(stand_in().dir.join("args.txt"));
                if let Some(controller) = controller_of(&root) {
                    crate::agent_panel::open_session_row(&controller, terminal);
                }
                soon(
                    "the terminal's session opens with its conversation",
                    || {
                        stand_in()
                            .args()
                            .iter()
                            .any(|a| *a == format!("--resume={TERMINAL_SESSION}"))
                            && crate::agent::all().iter().any(|t| {
                                t.is_live()
                                    && t.entry_ids(
                                        |k| matches!(k, EntryKind::UserMessage(m) if m.text == "Aus dem Terminal"),
                                    )
                                    .len()
                                        == 1
                            })
                    },
                    move || {
                        check(
                            "a closed session and one the CLI saved from a terminal open from the panel, resumed with their conversation",
                            true,
                            "",
                        );
                        cli_rewind();
                    },
                );
            },
        );
    });
}

/// The code of a prompt the app took no checkpoint before goes back through the CLI's own file history.
fn cli_rewind() {
    let tab = crate::agent::all()
        .into_iter()
        .find(|t| t.session_id().as_deref() == Some(TERMINAL_SESSION));
    let prompt = tab.as_ref().and_then(|t| {
        t.entry_ids(|k| matches!(k, EntryKind::UserMessage(m) if m.text == "Aus dem Terminal"))
            .first()
            .copied()
    });
    let (Some(tab), Some(prompt)) = (tab, prompt) else {
        check("the terminal's session shows its prompt", false, "");
        return finish(NAME);
    };
    let before = answered().len();
    answer_when_shown(NSAlertFirstButtonReturn, None);
    if let Some(menu) = tab.rewind_menu_of(prompt) {
        menu.performActionForItemAtIndex(1);
    }
    soon(
        "the CLI's file history reports",
        move || answered().len() > before,
        move || {
            let asked = stand_in().requests("rewind_files");
            check(
                "without a checkpoint of the app the code goes back through the CLI's file history, which says what it did",
                asked
                    .iter()
                    .any(|r| r["user_message_id"] == "51e1f7e5-6000-4000-8000-0000000000a1")
                    && answered().last().map(String::as_str) == Some("Code zurückgespult"),
                format!("{asked:?}, {:?}", answered()),
            );
            untrusted();
        },
    );
}

/// A folder not trusted yet: its AI tab starts nothing until the user trusts it.
fn untrusted() {
    let Some(mtm) = MainThreadMarker::new() else {
        return finish(NAME);
    };
    let folder = ide_core::paths::home().join("untrusted-project");
    let _ = std::fs::create_dir_all(&folder);
    let folder = folder.canonicalize().unwrap_or(folder);
    super::helpers::open_beside(mtm, &folder);
    let Some(controller) = controller_of(&folder) else {
        check("the untrusted folder opens beside", false, "");
        return finish(NAME);
    };
    let Some(tab) = crate::agent::open_tab(&controller, None) else {
        return finish(NAME);
    };
    let shown = tab.problem_texts();
    let blocked = !tab.is_live() && tab.problem_name() == Some("untrusted");
    tab.press_problem("Ordner vertrauen");
    let key = tab.key;
    soon(
        "the trusted folder's session starts",
        move || tab_of(key).is_some_and(|t| t.is_live()),
        move || {
            check(
                "an untrusted folder's tab starts nothing until the user trusts the folder",
                blocked && shown.iter().any(|t| t == "Ordner nicht vertraut") && crate::project::is_trusted(&folder),
                format!("{shown:?}"),
            );
            finish(NAME);
        },
    );
}

fn archived(id: &ide_thread::ThreadId) -> bool {
    ide_thread::ThreadIndex::open(&ide_thread::ThreadIndex::default_dir())
        .get(id)
        .is_some_and(|e| e.archived)
}
