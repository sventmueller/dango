//! The `stored-rules` scenario (E27, E146): the Agents tab of the settings lists the rules Claude stored for a project;
//! removing one goes through `claude edit-permission-rules`, and where the CLI is too old for that writer it takes
//! exactly that entry out of the project's `.claude/settings.local.json` itself.

use std::path::PathBuf;

use objc2::MainThreadMarker;

use super::agent_stand_in::StandIn;
use super::{check, finish, give_up, helpers, snapshot};

const NAME: &str = "stored-rules";

const SETTINGS: &str = r#"{
  "permissions": {
    "allow": [
      "Bash(cargo test:*)",
      "Bash(git log:*)"
    ],
    "deny": ["Bash(git log:*)"]
  },
  "env": {"KEEP": "1"}
}
"#;

pub(crate) fn run() {
    let files = [(".claude/settings.local.json", SETTINGS)];
    helpers::open_project(NAME, "rules-project", &files, |controller| {
        let root = helpers::root("rules-project");
        let Ok(prepared) = StandIn::new(&stand_in().dir, &root) else {
            return give_up(NAME, "the stand-in is written");
        };
        if prepared
            .write("sub-edit-permission-rules.out", "{\"ok\":true}\n")
            .is_err()
        {
            return give_up(NAME, "the stand-in's answer is written");
        }
        ide_ui_agent::set_test_claude(prepared.program());
        crate::settings::show_agents(MainThreadMarker::from(&*controller));
        let root = helpers::root("rules-project");
        helpers::wait_until(
            || {
                crate::settings::stored_rules().is_some_and(|rules| {
                    rules
                        .cells()
                        .iter()
                        .any(|row| row.first().is_some_and(|p| p.ends_with("rules-project")))
                })
            },
            move |_| listed(root),
        );
    });
}

fn listed(root: PathBuf) {
    let Some(rules) = crate::settings::stored_rules() else {
        return give_up(NAME, "the Agents tab has Claude's rules");
    };
    let cells = rules.cells();
    let local = ide_ui_agent::scope_name(&ide_thread::RuleScope::ProjectLocal);
    let ours: Vec<&Vec<String>> = cells
        .iter()
        .filter(|row| row.first().is_some_and(|p| p.ends_with("rules-project")))
        .collect();
    check(
        "the project's rules are listed with the project and where they apply",
        ours.len() == 2
            && ours[0][1] == "Bash(cargo test:*)"
            && ours[1][1] == "Bash(git log:*)"
            && ours.iter().all(|row| row[2] == local),
        format!("{cells:?}"),
    );
    rules.reveal_end();
    if let Some(window) = crate::settings::window() {
        snapshot(&window, NAME);
    }
    let Some(row) = cells.iter().position(|row| row[1] == "Bash(git log:*)") else {
        return give_up(NAME, "the project's rule for git log is listed");
    };
    rules.remove_row(row);
    let sent = stand_in().dir.join("sub-1.stdin");
    helpers::wait_until(move || sent.exists(), move |_| through_writer(root, row));
}

fn stand_in() -> StandIn {
    StandIn {
        dir: ide_core::paths::home().join("claude-rules-stand-in"),
    }
}

/// The removal went to Claude's writer, which the stand-in only notes.
fn through_writer(root: PathBuf, row: usize) {
    let sent: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(stand_in().dir.join("sub-1.stdin")).unwrap_or_default())
            .unwrap_or_default();
    let args = std::fs::read_to_string(stand_in().dir.join("sub-1.args")).unwrap_or_default();
    check(
        "removing a rule asks Claude's own writer to take it out of the file it lives in",
        args == "edit-permission-rules\n--json\n"
            && sent
                == serde_json::json!({"op": "remove", "rule": "Bash(git log:*)", "behavior": "allow",
                    "source": "localSettings"}),
        format!("{args:?} {sent}"),
    );
    let old = stand_in().write("sub-edit-permission-rules.out.err", "error: unknown option '--json'\n");
    let code = stand_in().write("sub-edit-permission-rules.out.code", "1");
    let Some(rules) = crate::settings::stored_rules() else {
        return give_up(NAME, "the Agents tab has Claude's rules");
    };
    if old.is_err() || code.is_err() {
        return give_up(NAME, "the old CLI is written");
    }
    rules.remove_row(row);
    let file = root.join(".claude/settings.local.json");
    let wanted = SETTINGS.replace(",\n      \"Bash(git log:*)\"", "");
    let path = file.clone();
    helpers::wait_until(
        move || std::fs::read_to_string(&path).is_ok_and(|text| text == wanted),
        move |_| without_writer(&file),
    );
}

/// A CLI too old for the writer leaves the removal to the app, which takes out exactly the rule's entry.
fn without_writer(file: &std::path::Path) {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    check(
        "with a CLI too old for the writer, removing a rule takes out exactly its entry",
        text == SETTINGS.replace(",\n      \"Bash(git log:*)\"", ""),
        text.clone(),
    );
    let Some(rules) = crate::settings::stored_rules() else {
        return give_up(NAME, "the Agents tab has Claude's rules");
    };
    rules.reload();
    check(
        "the list shows the rule gone",
        !rules.cells().iter().any(|row| row[1] == "Bash(git log:*)")
            && rules.cells().iter().any(|row| row[1] == "Bash(cargo test:*)"),
        format!("{:?}", rules.cells()),
    );
    finish(NAME);
}
