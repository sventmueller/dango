//! The `stored-rules` scenario (E27): the Agents tab of the settings lists the rules Claude stored for a project, and
//! removing one takes exactly that entry out of the project's `.claude/settings.local.json`.

use std::path::PathBuf;

use objc2::MainThreadMarker;

use super::{check, finish, helpers, snapshot};

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

pub fn run() {
    let files = [(".claude/settings.local.json", SETTINGS)];
    helpers::open_project(NAME, "rules-project", &files, |controller| {
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
        check("the Agents tab has Claude's rules", false, "");
        return finish(NAME);
    };
    let cells = rules.cells();
    let local = ide_ui_agent::scope_name("localSettings");
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
        return finish(NAME);
    };
    rules.remove_row(row);
    let file = std::fs::read_to_string(root.join(".claude/settings.local.json")).unwrap_or_default();
    check(
        "removing a rule takes out exactly its entry",
        file == SETTINGS.replace(",\n      \"Bash(git log:*)\"", ""),
        file.clone(),
    );
    check(
        "the list shows the rule gone",
        !rules.cells().iter().any(|row| row[1] == "Bash(git log:*)")
            && rules.cells().iter().any(|row| row[1] == "Bash(cargo test:*)"),
        format!("{:?}", rules.cells()),
    );
    finish(NAME);
}
