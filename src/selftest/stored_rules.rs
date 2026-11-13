//! The `stored-rules` scenario (E27): the Agents tab of the settings lists the rules Claude stored for a project, and
//! removing one takes exactly that entry out of the project's `.claude/settings.local.json`.

use std::path::PathBuf;
use std::time::Duration;

use objc2::MainThreadMarker;

use super::{check, finish, helpers, snapshot};
use crate::util::main_after;

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
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("rules-project");
    std::fs::create_dir_all(root.join(".claude")).expect("scratch project");
    std::fs::write(root.join(".claude/settings.local.json"), SETTINGS).expect("Claude's settings");
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(400), move || {
        let Some(controller) = helpers::window_of(&root) else {
            check("the project's window opens", false, "");
            return finish(NAME);
        };
        crate::settings::show_agents(MainThreadMarker::from(&*controller));
        main_after(Duration::from_millis(500), move || listed(root));
    });
}

fn listed(root: PathBuf) {
    let Some(rules) = crate::settings::stored_rules() else {
        check("the Agents tab has Claude's rules", false, "");
        return finish(NAME);
    };
    let cells = rules.cells();
    let local = crate::agent::scope_name("localSettings");
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
