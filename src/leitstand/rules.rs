//! The rule set of a folder (B39, E161, E190): the user's defaults tightened by `.dango/factory.toml` and the
//! templates under `.dango/factory/`, read from the user's working copy and never from an order's worktree (E172),
//! with the gate commands `ide-test` detects where the file names none (E110) and the loosenings the user confirmed
//! on this Mac. Only for trusted folders (E22), since the file starts commands; reading blocks and runs on the disk
//! lane.

use std::collections::BTreeMap;
use std::path::Path;

use ide_core::explain::Explain;
use ide_factory::workflow::rules::{Confirmations, Rules};
use ide_factory::workflow::{
    GateCommand, TEMPLATE_DIR, UserDefaults, WORKFLOW_FILE, WorkflowError, WorkflowErrorKind, parse,
};
use ide_factory::{InputKind, Stage};
use ide_test::Tool;

use super::{Delivery, Leitstand};

/// The stage of phase 13 this build ships.
pub(super) const STAGE: Stage = Stage::One;

/// The rules of the folder at `root` for `defaults` and `confirmed`, with `path` the login shell's PATH.
pub(super) fn load(root: &Path, defaults: &UserDefaults, confirmed: &Confirmations, path: Option<&str>) -> Rules {
    let rules = match std::fs::read_to_string(root.join(WORKFLOW_FILE)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Rules::new(defaults, None, confirmed),
        Err(error) => Rules::invalid(
            defaults,
            vec![WorkflowError {
                key: String::new(),
                line: None,
                column: None,
                kind: WorkflowErrorKind::Syntax {
                    message: error.explain(),
                },
            }],
        ),
        Ok(text) => match parse(&text) {
            Ok(parsed) => Rules::new(defaults, Some(&parsed.workflow), confirmed),
            Err(errors) => Rules::invalid(defaults, errors),
        },
    };
    rules
        .with_detected(detected(root, path), confirmed)
        .with_templates(templates(root))
}

/// The command that runs every test of `tool` in the gate sandbox, whose output names how many ran.
fn command(tool: Tool) -> &'static str {
    match tool {
        Tool::Cargo => "cargo test --workspace",
        Tool::Nextest => "cargo nextest run --workspace",
        Tool::Go => "go test ./...",
        Tool::Pytest => "python3 -m pytest",
        Tool::Vitest => "npx vitest run",
        Tool::Jest => "npx jest",
    }
}

/// The gate commands of the tools `ide-test` finds in the project (E110).
fn detected(root: &Path, path: Option<&str>) -> Vec<GateCommand> {
    ide_test::tools(root, path)
        .into_iter()
        .map(|tool| GateCommand {
            name: tool.name().to_string(),
            run: command(tool).to_string(),
            classes: Vec::new(),
        })
        .collect()
}

/// The prompt templates of the project by file name, such as `build.md`.
fn templates(root: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(root.join(TEMPLATE_DIR)) else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|x| x == "md"))
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some((name, std::fs::read_to_string(&path).ok()?))
        })
        .collect()
}

/// The user's defaults as the settings hold them.
pub(super) fn defaults() -> UserDefaults {
    ide_core::settings::Settings::load_user().get(&super::DEFAULTS)
}

impl Leitstand {
    /// Reads the rules of `root` again and hands them to its factory when they differ from what it holds.
    pub(crate) fn reload_rules(&self, root: &Path) {
        let Some(confirmed) = self.folders.borrow().get(root).map(|f| f.confirmations.clone()) else {
            return;
        };
        if !crate::project::is_trusted(root) {
            return;
        }
        let root = root.to_path_buf();
        let defaults = defaults();
        let back = self.back();
        let queued = ide_core::lane::DISK.try_run(move || {
            let login = crate::environment::wait(std::time::Duration::from_secs(15)).unwrap_or_default();
            let path = login
                .iter()
                .find(|(name, _)| name == "PATH")
                .map(|(_, value)| value.as_str());
            let rules = load(&root, &defaults, &confirmed, path);
            back.deliver(Delivery::Input(root, Box::new(InputKind::Rules(Box::new(rules)))));
        });
        if queued.is_err() {
            ide_core::diag::report("reading a workflow file", "the disk lane took no more work");
        }
    }

    pub(super) fn reload_all_rules(&self) {
        for root in self.roots() {
            self.reload_rules(&root);
        }
    }

    /// Whether `rules` would change what the factory of `root` holds, so an unchanged file leaves no step behind.
    pub(super) fn rules_differ(&self, root: &Path, rules: &Rules) -> bool {
        self.folders
            .borrow()
            .get(root)
            .is_none_or(|folder| folder.factory.rules() != rules || folder.factory.errors() != rules.errors())
    }

    /// Stores the confirmation `hash` for `root` on this Mac (E190) and reads the rules again with it.
    pub(super) fn confirm(&self, root: &Path, hash: String) {
        let (project, confirmations) = {
            let mut folders = self.folders.borrow_mut();
            let Some(folder) = folders.get_mut(root) else { return };
            folder.confirmations.add(hash);
            (folder.project.clone(), folder.confirmations.clone())
        };
        let queued = ide_core::lane::DISK.try_run(move || super::store::save_confirmations(&project, &confirmations));
        if queued.is_err() {
            ide_core::diag::report("keeping a confirmation", "the disk lane took no more work");
        }
        self.reload_rules(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("leitstand-rules-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".dango/factory")).unwrap();
        dir
    }

    #[test]
    fn the_file_its_templates_and_the_detected_tools_make_the_rules() {
        let root = folder("file");
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let defaults = UserDefaults::default();
        let detected = load(&root, &defaults, &Confirmations::default(), None);
        assert!(detected.gates_detected() && detected.has_gates() && !detected.gates_confirmed());
        std::fs::write(
            root.join(WORKFLOW_FILE),
            "[[gates.command]]\nname = \"check\"\nrun = \"make check\"\n",
        )
        .unwrap();
        std::fs::write(root.join(".dango/factory/build.md"), "Build {{title}}").unwrap();
        let first = load(&root, &defaults, &Confirmations::default(), None);
        assert!(!first.gates_detected() && first.errors().is_empty());
        let confirmed = Confirmations::new([first.gate_hash().to_string()]);
        let rules = load(&root, &defaults, &confirmed, None);
        assert!(rules.gates_confirmed());
        assert_ne!(rules.hash(), Rules::new(&defaults, None, &confirmed).hash());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_invalid_file_gives_its_errors_and_none_of_its_rules() {
        let root = folder("invalid");
        std::fs::write(root.join(WORKFLOW_FILE), "[[gates.command]\n").unwrap();
        let rules = load(&root, &UserDefaults::default(), &Confirmations::default(), None);
        assert!(!rules.errors().is_empty());
        assert!(!rules.has_gates());
        let _ = std::fs::remove_dir_all(&root);
    }
}
