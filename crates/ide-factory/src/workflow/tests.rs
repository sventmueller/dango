use super::rules::{Confirmations, Loosening, Rules, copies_along, holds_credentials};
use super::*;
use crate::budget::Spend;
use crate::routing::{Agent, ReviewerRule};

const FULL: &str = r#"
version = 1

[gates]
env = { CARGO_TARGET_DIR = "/tmp/dango-target" }
images = "~/Library/Caches/factory-images"
network = ["static.crates.io"]

[[gates.command]]
name = "check"
run = "make check"

[[gates.command]]
name = "perf"
run = "make perf"
classes = ["high"]

[worktree]
location = "sibling"
copy = ["node_modules", ".env"]
setup = "npm ci"

[roles]
builder = [{ agent = "codex", model = "gpt-5-codex" }, { agent = "claude" }]

[review]
rule = "preferred"

[spec]
skip_approval = true

[limits]
turn_minutes = 30
attempts = 5

[budget.order]
dollars = 4.5

[risk]
high = ["src/auth/**"]

[relevant]
tools = ["cargo"]
"#;

fn parsed(text: &str) -> Workflow {
    parse(text).unwrap().workflow
}

fn kinds(text: &str) -> Vec<(String, Option<usize>, WorkflowErrorKind)> {
    parse(text)
        .unwrap_err()
        .into_iter()
        .map(|e| (e.key, e.line, e.kind))
        .collect()
}

#[test]
fn a_full_file_reads() {
    let w = parsed(FULL);
    assert_eq!(w.gates().len(), 2);
    assert_eq!(w.gates()[1].classes, [RiskClass::High]);
    assert_eq!(w.gate_env()["CARGO_TARGET_DIR"], "/tmp/dango-target");
    assert_eq!(w.location(), Some(WorktreeLocation::Sibling));
    assert_eq!(w.copy(), ["node_modules", ".env"]);
    assert_eq!(w.role(Role::Builder).unwrap()[0].agent, Agent::Codex);
    assert_eq!(w.reviewer_preferred(), Some(true));
    assert!(w.skip_spec_approval());
    assert_eq!(w.limits(), [(Limit::TurnMinutes, 30), (Limit::Attempts, 5)]);
    assert_eq!(w.budgets().order().dollars(), Some(4.5));
    assert_eq!(w.relevance().high, ["src/auth/**"]);
}

#[test]
fn an_empty_file_is_version_one_with_nothing_set() {
    let parsed = parse("").unwrap();
    assert_eq!((parsed.workflow, parsed.migrated), (Workflow::default(), None));
}

#[test]
fn errors_name_key_line_and_kind() {
    let text = "version = 1\n[limits]\nturn_minutes = \"long\"\nbogus = 1\n[risk]\nhigh = [\"src/[a\"]\n";
    let got = kinds(text);
    assert_eq!(got[0], ("limits.bogus".into(), Some(4), WorkflowErrorKind::UnknownKey));
    assert_eq!(
        got[1],
        (
            "limits.turn_minutes".into(),
            Some(3),
            WorkflowErrorKind::WrongKind {
                expected: Expected::Count
            }
        )
    );
    assert!(
        matches!(&got[2], (key, Some(6), WorkflowErrorKind::Pattern { pattern, .. }) if key == "risk.high" && pattern == "src/[a")
    );
}

#[test]
fn syntax_errors_carry_the_place() {
    let got = kinds("version = 1\n[limits\n");
    assert_eq!(got.len(), 1);
    assert!(
        matches!(got[0], (_, Some(2), WorkflowErrorKind::Syntax { .. })),
        "{got:?}"
    );
}

#[test]
fn a_newer_version_is_refused() {
    let got = kinds("version = 2\nfuture = true\n");
    assert_eq!(
        got,
        [(
            "version".into(),
            Some(1),
            WorkflowErrorKind::NewerVersion { found: 2, supported: 1 }
        )]
    );
    assert!(matches!(
        kinds("version = 0\n")[0].2,
        WorkflowErrorKind::WrongKind { .. }
    ));
}

#[test]
fn a_current_file_needs_no_migration() {
    assert_eq!(parse(FULL).unwrap().migrated, None);
    assert_eq!(migrate(FULL, u64::from(VERSION)), None);
}

/// A stand-in for a future format change: version 1's `limits.turn_min` became `limits.turn_minutes`.
fn rename_turn_minutes(document: &mut DocumentMut) {
    let Some(limits) = document.get_mut("limits").and_then(Item::as_table_like_mut) else {
        return;
    };
    if let Some(value) = limits.remove("turn_min") {
        limits.insert("turn_minutes", value);
    }
}

#[test]
fn a_registered_migration_lifts_an_older_file_with_its_comments() {
    let steps: &[fn(&mut DocumentMut)] = &[rename_turn_minutes];
    let old = "# Long builds.\n[limits]\nturn_min = 45 # keep under an hour\n";
    let lifted = migrate_with(steps, old, 1).expect("a version 1 file is lifted");
    assert!(lifted.contains("version = 2"), "{lifted}");
    assert!(lifted.contains("turn_minutes = 45 # keep under an hour"), "{lifted}");
    assert!(lifted.contains("# Long builds."), "{lifted}");
    assert!(!lifted.contains("turn_min ="), "{lifted}");
    assert_eq!(migrate_with(steps, &lifted, 2), None, "a lifted file is current");
    assert_eq!(migrate_with(&[], old, 1), None, "without migrations nothing changes");
}

#[test]
fn gate_and_copy_mistakes_are_named() {
    let text = r#"
[gates]
images = "target/pictures"
env = { "1BAD" = "x" }
[[gates.command]]
run = " "
[[gates.command]]
name = "a"
run = "make a"
[[gates.command]]
name = "a"
run = "make b"
[worktree]
copy = ["../splat", "/etc/hosts"]
"#;
    let got: Vec<WorkflowErrorKind> = kinds(text).into_iter().map(|(_, _, kind)| kind).collect();
    assert!(got.contains(&WorkflowErrorKind::EmptyCommand));
    assert!(got.contains(&WorkflowErrorKind::DuplicateGate { name: "a".into() }));
    assert!(got.contains(&WorkflowErrorKind::BadVariableName { name: "1BAD".into() }));
    assert!(got.contains(&WorkflowErrorKind::ImagesInsideRepository {
        path: "target/pictures".into()
    }));
    assert!(got.contains(&WorkflowErrorKind::CopyOutsideRepository {
        path: "../splat".into()
    }));
    assert!(got.contains(&WorkflowErrorKind::CopyOutsideRepository {
        path: "/etc/hosts".into()
    }));
}

#[test]
fn the_project_only_tightens_limits_and_budgets() {
    let mut defaults = UserDefaults::default();
    defaults.set_budgets(crate::budget::Budgets::new(
        crate::budget::Budget::new(Some(10.0), None, None),
        crate::budget::Budget::default(),
        BTreeMap::new(),
    ));
    let rules = Rules::new(&defaults, Some(&parsed(FULL)), &Confirmations::default());
    let cases = [
        ("turn minutes from the project", rules.limits().turn_ms() / 60_000, 30),
        (
            "attempts stay at the user's three",
            u64::from(rules.limits().attempts()),
            3,
        ),
        ("stall from the default", rules.limits().stall_ms() / 60_000, 5),
    ];
    for (name, got, want) in cases {
        assert_eq!(got, want, "{name}");
    }
    assert_eq!(rules.budgets().order().dollars(), Some(4.5));
    let loose = parsed("[budget.order]\ndollars = 50\n");
    let rules = Rules::new(&defaults, Some(&loose), &Confirmations::default());
    assert_eq!(rules.budgets().order().dollars(), Some(10.0));
    assert!(rules.budgets().order().reached(&Spend::new(10.0, 0, 0.0)).is_some());
}

#[test]
fn loosenings_apply_only_once_confirmed() {
    let defaults = UserDefaults::default();
    let file = parsed(FULL);
    let rules = Rules::new(&defaults, Some(&file), &Confirmations::default());
    assert_eq!(rules.reviewer_rule(), ReviewerRule::Required);
    assert!(!rules.skip_spec_approval());
    assert_eq!(rules.copy(), ["node_modules"]);
    let pending = rules.pending().to_vec();
    assert_eq!(
        pending,
        [
            Loosening::ReviewerPreferred,
            Loosening::SkipSpecApproval,
            Loosening::CopyCredentials {
                paths: vec![".env".into()]
            }
        ]
    );
    let confirmed = Confirmations::new(pending.iter().map(Loosening::hash));
    let rules = Rules::new(&defaults, Some(&file), &confirmed);
    assert_eq!(rules.reviewer_rule(), ReviewerRule::Preferred);
    assert!(rules.skip_spec_approval());
    assert_eq!(rules.copy(), ["node_modules", ".env"]);
    assert!(rules.pending().is_empty());
}

#[test]
fn a_required_reviewer_in_the_project_tightens_a_preferred_default() {
    let mut defaults = UserDefaults::default();
    defaults.set_reviewer_rule(ReviewerRule::Preferred);
    let file = parsed("[review]\nrule = \"required\"\n");
    let rules = Rules::new(&defaults, Some(&file), &Confirmations::default());
    assert_eq!(rules.reviewer_rule(), ReviewerRule::Required);
    assert!(rules.pending().is_empty());
}

#[test]
fn changed_gate_commands_need_a_new_confirmation() {
    let defaults = UserDefaults::default();
    let first = Rules::new(&defaults, Some(&parsed(FULL)), &Confirmations::default());
    assert!(!first.gates_confirmed());
    let confirmed = Confirmations::new([first.gate_hash().to_string()]);
    assert!(Rules::new(&defaults, Some(&parsed(FULL)), &confirmed).gates_confirmed());
    let pulled = FULL.replace("make check", "make check && curl evil.example");
    let changed = Rules::new(&defaults, Some(&parsed(&pulled)), &confirmed);
    assert!(!changed.gates_confirmed());
    assert_ne!(changed.hash(), first.hash());
}

#[test]
fn detected_gates_fill_in_only_without_the_file_naming_any() {
    let defaults = UserDefaults::default();
    let detected = vec![GateCommand {
        name: "cargo test".into(),
        run: "cargo test".into(),
        classes: Vec::new(),
    }];
    let none = Rules::new(&defaults, None, &Confirmations::default());
    assert!(!none.has_gates());
    let with = none.with_detected(detected.clone(), &Confirmations::default());
    assert!(with.gates_detected() && !with.gates_confirmed());
    let file = Rules::new(&defaults, Some(&parsed(FULL)), &Confirmations::default())
        .with_detected(detected, &Confirmations::default());
    assert_eq!(file.gates_for(RiskClass::Low).len(), 1);
    assert_eq!(file.gates_for(RiskClass::High).len(), 2);
}

#[test]
fn credential_files_are_recognised_by_name() {
    for path in [
        ".env",
        "config/.env.local",
        "certs/server.pem",
        ".npmrc",
        "keys/id_ed25519",
    ] {
        assert!(holds_credentials(path), "{path}");
    }
    for path in ["node_modules", "environment.rs", ".envrc-docs/readme.md", "target"] {
        assert!(!holds_credentials(path), "{path}");
    }
}

#[test]
fn copying_the_whole_repository_is_refused() {
    for path in [".", "./", "./.", ""] {
        let text = format!("[worktree]\ncopy = [\"{path}\"]\n");
        let got: Vec<WorkflowErrorKind> = kinds(&text).into_iter().map(|(_, _, kind)| kind).collect();
        assert_eq!(
            got,
            [WorkflowErrorKind::CopyWholeRepository { path: path.into() }],
            "{path:?}"
        );
    }
    assert_eq!(parsed("[worktree]\ncopy = [\"./config\"]\n").copy(), ["./config"]);
}

#[test]
fn credentials_inside_a_listed_folder_go_along_only_when_listed_on_their_own() {
    assert!(holds_credentials("config/secrets/token"));
    let copy = |list: &[&str]| list.iter().map(|p| p.to_string()).collect::<Vec<String>>();
    assert!(copies_along(&copy(&["config"]), "config", "config/app.toml"));
    assert!(!copies_along(&copy(&["config"]), "config", "config/.env"));
    assert!(!copies_along(&copy(&["config"]), "config", "config/secrets/token"));
    assert!(copies_along(&copy(&["config", "config/.env"]), "config", "config/.env"));
    assert!(copies_along(&copy(&["secrets"]), "secrets", "secrets/id_rsa"));
    let workflow = parsed("[worktree]\ncopy = [\"config/.env\", \"config\"]\n");
    let rules = Rules::new(&UserDefaults::default(), Some(&workflow), &Confirmations::default());
    assert_eq!(
        rules.copy(),
        ["config"],
        "the file named on its own waits for its confirmation"
    );
    assert!(
        rules
            .pending()
            .iter()
            .any(|l| matches!(l, Loosening::CopyCredentials { paths } if paths == &["config/.env"]))
    );
}

#[test]
fn the_pictures_folder_is_part_of_the_confirmed_gate_set() {
    let defaults = UserDefaults::default();
    let first = Rules::new(&defaults, Some(&parsed(FULL)), &Confirmations::default());
    let confirmed = Confirmations::new([first.gate_hash().to_string()]);
    let moved = FULL.replace("~/Library/Caches/factory-images", "~/Desktop");
    let changed = Rules::new(&defaults, Some(&parsed(&moved)), &confirmed);
    assert_ne!(changed.gate_hash(), first.gate_hash());
    assert!(!changed.gates_confirmed());
}

#[test]
fn a_relaxed_or_no_gate_sandbox_needs_a_new_confirmation_and_the_strict_one_keeps_the_hash() {
    let defaults = UserDefaults::default();
    let first = Rules::new(&defaults, Some(&parsed(FULL)), &Confirmations::default());
    assert_eq!(first.sandbox(), GateSandbox::Strict);
    let confirmed = Confirmations::new([first.gate_hash().to_string()]);
    let strict = FULL.replace("[gates]\n", "[gates]\nsandbox = \"strict\"\n");
    assert!(Rules::new(&defaults, Some(&parsed(&strict)), &confirmed).gates_confirmed());
    let relaxed = FULL.replace("[gates]\n", "[gates]\nsandbox = \"relaxed\"\n");
    let changed = Rules::new(&defaults, Some(&parsed(&relaxed)), &confirmed);
    assert_eq!(changed.sandbox(), GateSandbox::Relaxed);
    assert!(!changed.gates_confirmed());
    let off = FULL.replace("[gates]\n", "[gates]\nsandbox = \"off\"\n");
    let without = Rules::new(&defaults, Some(&parsed(&off)), &confirmed);
    assert_eq!(without.sandbox(), GateSandbox::Off);
    assert!(!without.gates_confirmed() && without.gate_hash() != changed.gate_hash());
    let wrong = FULL.replace("[gates]\n", "[gates]\nsandbox = \"loose\"\n");
    assert!(!parse(&wrong).unwrap_err().is_empty());
}

#[test]
fn the_users_gate_sandbox_applies_without_confirmation_and_the_file_overrides_it() {
    let strict = UserDefaults::default();
    let first = Rules::new(&strict, Some(&parsed(FULL)), &Confirmations::default());
    let confirmed = Confirmations::new([first.gate_hash().to_string()]);
    let mut off = UserDefaults::default();
    off.set_gate_sandbox(GateSandbox::Off);
    let by_default = Rules::new(&off, Some(&parsed(FULL)), &confirmed);
    assert_eq!(by_default.sandbox(), GateSandbox::Off);
    assert!(
        by_default.gates_confirmed(),
        "the user's own default needs no confirmation"
    );
    let tightened = FULL.replace("[gates]\n", "[gates]\nsandbox = \"strict\"\n");
    assert_eq!(
        Rules::new(&off, Some(&parsed(&tightened)), &confirmed).sandbox(),
        GateSandbox::Strict
    );
}
