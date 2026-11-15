use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use super::*;

/// A Mac made up for a test: what it has is listed, everything else is missing.
#[derive(Default)]
struct Fake {
    programs: HashSet<&'static str>,
    vars: HashMap<&'static str, &'static str>,
    process: HashMap<&'static str, String>,
    files: HashSet<PathBuf>,
    hosts: HashSet<&'static str>,
    /// Calls that succeed, as `program arg arg…` with the program's file name.
    succeeding: HashSet<String>,
    origins: HashMap<PathBuf, &'static str>,
    clones: Vec<PathBuf>,
    asked: RefCell<Vec<String>>,
}

const HOME: &str = "/Users/someone";

impl Probe for Fake {
    fn program(&self, name: &str) -> Option<PathBuf> {
        self.programs.contains(name).then(|| Path::new("/opt/bin").join(name))
    }

    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).map(ToString::to_string)
    }

    fn process_var(&self, name: &str) -> Option<String> {
        self.process.get(name).cloned()
    }

    fn home(&self) -> PathBuf {
        PathBuf::from(HOME)
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    fn reachable(&self, host: &str) -> bool {
        self.hosts.contains(host)
    }

    fn succeeds(&self, program: &Path, args: &[&str]) -> bool {
        let name = program.file_name().unwrap_or_default().to_string_lossy();
        let call = format!("{name} {}", args.join(" "));
        self.asked.borrow_mut().push(call.clone());
        self.succeeding.contains(&call)
    }

    fn origin(&self, repository: &Path) -> Option<String> {
        self.asked.borrow_mut().push(format!("origin {}", repository.display()));
        self.origins.get(repository).map(ToString::to_string)
    }

    fn clones(&self) -> Vec<PathBuf> {
        self.clones.clone()
    }
}

fn home(path: &str) -> PathBuf {
    Path::new(HOME).join(path)
}

/// A Mac with every tool, sign-in and host the steps ask for.
fn complete() -> Fake {
    let mut fake = Fake::default();
    fake.programs.extend([
        "gopls",
        "go",
        "rust-analyzer",
        "node",
        "npm",
        "git",
        "gh",
        "glab",
        "claude",
        "codex",
        "gemini",
        "agy",
        "codex-acp",
        "taplo",
        "marksman",
        "lua-language-server",
        "ollama",
        "lms",
        "dlv",
        "cargo",
    ]);
    fake.vars.extend([("GEMINI_API_KEY", "g"), ("OPENAI_API_KEY", "o")]);
    fake.files
        .extend([home(".local/bin/claude"), home(".claude/.credentials.json")]);
    fake.files.insert(home(".config/github-copilot/apps.json"));
    fake.hosts
        .extend(["registry.npmjs.org", "github.com", "proxy.golang.org", "dl.google.com"]);
    fake.hosts.insert("api.githubcopilot.com");
    fake.succeeding.extend([
        "sh -c /usr/sbin/DevToolsSecurity -status | /usr/bin/grep -q 'currently enabled'".to_string(),
        "gh auth status --hostname github.com".to_string(),
        "glab auth status --hostname gitlab.example.org".to_string(),
        "xcode-select -p".to_string(),
        "ollama show qwen2.5-coder:1.5b".to_string(),
    ]);
    fake.origins
        .insert(home("Repos/splat"), "https://github.com/someone/splat.git");
    fake.origins
        .insert(home("Repos/work"), "ssh://git@gitlab.example.org:2222/team/work.git");
    fake.clones = vec![home("Repos/splat"), home("Repos/work")];
    fake
}

fn context(probe: &dyn Probe) -> Context<'_> {
    Context {
        probe,
        recordings: PathBuf::from("/tmp/recordings"),
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

#[test]
fn the_options_are_a_listing_and_known_groups() {
    assert_eq!(Options::parse(&[]).unwrap(), Options::default());
    let options = Options::parse(&strings(&[
        "--list", "--only", "lsp,git", "--only", "ci", "--only", "git",
    ]))
    .unwrap();
    assert!(options.list && !options.paid);
    assert_eq!(options.only, ["lsp", "git", "ci"]);
    assert!(Options::parse(&strings(&["--paid"])).unwrap().paid);
    assert!(Options::parse(&strings(&["--only"])).is_err());
    assert!(
        Options::parse(&strings(&["--only", "lsp,nope"]))
            .unwrap_err()
            .contains("nope")
    );
    assert!(Options::parse(&strings(&["--dry-run"])).is_err());
}

#[test]
fn a_missing_program_host_or_sign_in_is_named_as_the_reason() {
    let bare = Fake::default();
    assert_eq!(
        unmet(&Need::Program("gopls"), &bare).as_deref(),
        Some("gopls is not on the login shell's PATH")
    );
    assert_eq!(
        unmet(&Need::AnyProgram(&["codex", "agy"]), &bare).as_deref(),
        Some("none of codex, agy is on the login shell's PATH")
    );
    assert_eq!(
        unmet(&Need::Network("github.com"), &bare).as_deref(),
        Some("github.com cannot be reached")
    );
    assert_eq!(
        unmet(&steps::GEMINI, &bare).as_deref(),
        Some("no sign-in with Gemini: none of $GEMINI_API_KEY, $GOOGLE_API_KEY, ~/.gemini/oauth_creds.json")
    );
    let login = Need::CiToken {
        tool: "glab",
        host: "gitlab.example.org".into(),
    };
    assert_eq!(
        unmet(&login, &bare).as_deref(),
        Some("glab is not signed in to gitlab.example.org, and the login shell holds no token for it")
    );
    assert_eq!(unmet(&Need::Unmet("why".into()), &complete()).as_deref(), Some("why"));
    assert_eq!(unmet(&login, &complete()), None);
}

#[test]
fn a_ci_token_comes_from_the_tool_or_from_the_variable_meant_for_the_host() {
    let mut fake = Fake::default();
    assert!(!ci_token("gh", "github.com", &fake));
    fake.vars.insert("GITHUB_TOKEN", "t");
    assert!(ci_token("gh", "github.com", &fake));
    assert!(!ci_token("gh", "github.example.org", &fake));
    fake.vars.insert("GITLAB_TOKEN", "t");
    assert!(
        ci_token("glab", "gitlab.com", &fake),
        "GITLAB_TOKEN alone is gitlab.com's"
    );
    assert!(!ci_token("glab", "gitlab.example.org", &fake));
    fake.vars.insert("GITLAB_HOST", "https://gitlab.example.org");
    assert!(ci_token("glab", "gitlab.example.org", &fake));
    assert!(!ci_token("glab", "gitlab.com", &fake));
}

#[test]
fn rust_analyzer_counts_on_the_path_and_through_rustup() {
    let mut fake = Fake::default();
    assert!(unmet(&Need::RustAnalyzer, &fake).is_some());
    fake.programs.insert("rustup");
    assert!(
        unmet(&Need::RustAnalyzer, &fake).is_some(),
        "rustup without the component"
    );
    fake.succeeding.insert("rustup which rust-analyzer".into());
    assert_eq!(unmet(&Need::RustAnalyzer, &fake), None);
    let mut on_path = Fake::default();
    on_path.programs.insert("rust-analyzer");
    assert_eq!(unmet(&Need::RustAnalyzer, &on_path), None);
}

#[test]
fn the_developer_tools_count_when_xcode_select_names_them() {
    let mut fake = Fake::default();
    assert!(unmet(&Need::DeveloperTools, &fake).is_some());
    fake.succeeding.insert("xcode-select -p".into());
    assert_eq!(unmet(&Need::DeveloperTools, &fake), None);
}

#[test]
fn debugging_counts_only_when_developer_mode_is_on() {
    let mut fake = Fake::default();
    assert!(unmet(&Need::Debugging, &fake).is_some());
    fake.succeeding
        .insert("sh -c /usr/sbin/DevToolsSecurity -status | /usr/bin/grep -q 'currently enabled'".into());
    assert_eq!(unmet(&Need::Debugging, &fake), None);
}

#[test]
fn a_sign_in_shows_by_a_variable_a_file_or_a_keychain_item_without_being_read() {
    let claude = steps::CLAUDE;
    let mut fake = Fake::default();
    assert!(unmet(&claude, &fake).is_some());
    fake.vars.insert("ANTHROPIC_API_KEY", "");
    assert!(unmet(&claude, &fake).is_some(), "an empty variable is no sign-in");
    fake.succeeding
        .insert("security find-generic-password -s Claude Code-credentials".into());
    assert_eq!(unmet(&claude, &fake), None);
    let mut file = Fake::default();
    file.files.insert(home(".claude/.credentials.json"));
    assert_eq!(unmet(&claude, &file), None);
}

fn everything() -> Options {
    Options::parse(&strings(&["--paid"])).unwrap()
}

fn selected(args: &[&str]) -> Vec<&'static str> {
    let options = Options::parse(&strings(args)).unwrap();
    GROUPS.iter().filter(|g| options.selects(g)).map(|g| g.name).collect()
}

#[test]
fn paid_groups_run_only_when_named_or_asked_for_and_the_run_says_so() {
    assert_eq!(selected(&[]), ["lsp", "git", "ci", "dap", "predict"]);
    assert_eq!(
        selected(&["--paid"]),
        [
            "lsp", "git", "ci", "dap", "claude", "paste", "acp", "predict", "copilot"
        ]
    );
    assert_eq!(selected(&["--only", "acp"]), ["acp"]);
    assert_eq!(
        selected(&["--only", "lsp", "--paid"]),
        ["lsp", "claude", "paste", "acp", "copilot"]
    );
    let notice = |args: &[&str]| Options::parse(&strings(args)).unwrap().paid_notice();
    assert_eq!(
        notice(&[]).as_deref(),
        Some(
            "not run, as they use the owner's paid accounts: claude, paste, acp, copilot; name them in --only or add \
             --paid"
        )
    );
    assert_eq!(
        notice(&["--only", "claude,git"]).as_deref(),
        Some("paid groups in this run, which use the owner's accounts: claude")
    );
    assert_eq!(notice(&["--only", "git"]), None);
    let fake = complete();
    let plan = plan(&Options::default(), &context(&fake));
    assert!(plan.iter().all(|(group, _)| !group.paid));
}

#[test]
fn on_a_bare_mac_every_step_is_skipped_with_a_reason() {
    let bare = Fake::default();
    let plan = plan(&everything(), &context(&bare));
    assert_eq!(plan.len(), GROUPS.len());
    for (group, steps) in &plan {
        assert!(!steps.is_empty(), "{} has steps", group.name);
        for planned in steps {
            assert!(
                planned.skip.as_ref().is_some_and(|why| !why.is_empty()),
                "{} / {} runs on a bare Mac",
                group.name,
                planned.step.name
            );
        }
    }
}

#[test]
fn with_everything_in_place_every_step_runs() {
    let fake = complete();
    for (group, steps) in plan(&everything(), &context(&fake)) {
        for planned in steps {
            assert_eq!(planned.skip, None, "{} / {}", group.name, planned.step.name);
        }
    }
}

#[test]
fn only_the_selected_groups_are_put_together_and_probed() {
    let fake = complete();
    let options = Options::parse(&strings(&["--only", "lsp"])).unwrap();
    let plan = plan(&options, &context(&fake));
    assert_eq!(plan.iter().map(|(g, _)| g.name).collect::<Vec<_>>(), ["lsp"]);
    assert!(
        fake.asked
            .borrow()
            .iter()
            .all(|call| !call.starts_with("origin") && !call.contains("auth")),
        "{:?}",
        fake.asked.borrow()
    );
}

#[test]
fn the_ci_steps_read_the_clones_the_variables_name_or_the_first_fitting_one() {
    let mut fake = complete();
    fake.origins
        .insert(home("Repos/other"), "git@gitlab.other.org:team/other.git");
    fake.clones = vec![home("Repos/other"), home("Repos/splat"), home("Repos/work")];
    let steps = steps_of(&fake, "ci");
    assert_eq!(
        steps[1].action.to_string(),
        format!("IDE_CI_LIVE_REPO={HOME}/Repos/work cargo xtask selftest ci-live"),
        "a host glab is not signed in to is passed over"
    );
    fake.process
        .insert("IDE_INTEGRATION_GITLAB_REPO", home("Repos/other").display().to_string());
    fake.process
        .insert("IDE_INTEGRATION_GITHUB_REPO", home("Repos/work").display().to_string());
    let plan = plan(&Options::parse(&strings(&["--only", "ci"])).unwrap(), &context(&fake));
    let skips: Vec<String> = plan[0].1.iter().filter_map(|p| p.skip.clone()).collect();
    assert_eq!(
        skips,
        [
            format!(
                "{HOME}/Repos/work has its origin on gitlab.example.org, not github.com; set IDE_INTEGRATION_GITHUB_REPO"
            ),
            "glab is not signed in to gitlab.other.org, and the login shell holds no token for it".to_string(),
        ]
    );
}

fn steps_of(probe: &dyn Probe, group: &str) -> Vec<Step> {
    let group = GROUPS.iter().find(|g| g.name == group).unwrap();
    (group.steps)(&context(probe))
}

#[test]
fn the_groups_that_start_coding_agents_say_they_use_paid_accounts() {
    let paid: Vec<&str> = GROUPS.iter().filter(|g| g.paid).map(|g| g.name).collect();
    assert_eq!(paid, ["claude", "paste", "acp", "copilot"]);
    let group = GROUPS.iter().find(|g| g.name == "acp").unwrap();
    assert!(heading(group).contains("uses the owner's paid accounts"));
    let fake = complete();
    let options = Options::parse(&strings(&["--only", "git"])).unwrap();
    let (group, steps) = plan(&options, &context(&fake)).remove(0);
    let listed = listing(group, &steps);
    assert!(listed[0].starts_with("git: ") && listed[0].contains("no paid account"));
    assert!(listed[1].starts_with("  run   checkpoint restore"), "{listed:?}");
}

#[test]
fn every_step_names_an_ignored_test_or_a_scenario_that_exists() {
    let fake = complete();
    let scenarios = files::read_to_string(root().join("src/selftest/scenarios.rs")).unwrap();
    for group in GROUPS {
        for step in (group.steps)(&context(&fake)) {
            match step.action {
                Action::Test {
                    package, file, name, ..
                } => {
                    let source =
                        files::read_to_string(root().join(format!("crates/{package}/tests/{file}.rs"))).unwrap();
                    let at = source
                        .find(&format!("fn {name}()"))
                        .unwrap_or_else(|| panic!("no test {name}"));
                    let attributes = source[..at].rsplit("\n\n").next().unwrap_or_default();
                    assert!(attributes.contains("#[ignore"), "{name} is not ignored");
                }
                Action::Selftest { scenario, .. } => {
                    assert!(
                        scenarios.contains(&format!("(\"{scenario}\",")),
                        "no scenario {scenario}"
                    );
                }
            }
        }
    }
}

#[test]
fn remote_urls_give_their_host() {
    for (url, host) in [
        ("https://github.com/someone/splat.git", Some("github.com")),
        ("https://user@GitHub.com:443/someone/splat", Some("github.com")),
        (
            "ssh://git@gitlab.example.org:2222/team/work.git",
            Some("gitlab.example.org"),
        ),
        ("git@gitlab.example.org:team/work.git", Some("gitlab.example.org")),
        ("/Users/someone/Repos/local", None),
        ("", None),
    ] {
        assert_eq!(remote_host(url).as_deref(), host, "{url}");
    }
}

#[test]
fn an_ignored_test_counts_only_when_it_ran_and_may_say_it_skipped_itself() {
    let ran = strings(&["", "running 1 test", "test x ... ok"]);
    assert_eq!(judge_test(true, &ran), Outcome::Passed);
    assert!(matches!(judge_test(false, &ran), Outcome::Failed(_)));
    assert!(matches!(
        judge_test(true, &strings(&["running 0 tests"])),
        Outcome::Failed(_)
    ));
    let skipped = strings(&[
        "running 1 test",
        "integration skip: agy asks for a sign-in",
        "test x ... ok",
    ]);
    assert_eq!(
        judge_test(true, &skipped),
        Outcome::Skipped("agy asks for a sign-in".into())
    );
}

#[test]
fn a_scenario_that_skipped_each_of_its_checks_counts_as_skipped() {
    let check = |detail: &str| Check {
        name: "n".into(),
        ok: true,
        detail: detail.into(),
    };
    let report = |checks| Report {
        scenario: "ci-live".into(),
        ending: Ending::Ran,
        checks,
        appearance: None,
    };
    assert_eq!(
        judge_selftest(&report(vec![check("skipped: SignIn, no usable login")])),
        Outcome::Skipped("SignIn, no usable login".into())
    );
    assert_eq!(
        judge_selftest(&report(vec![check("skipped: one"), check("3 runs")])),
        Outcome::Passed
    );
    assert_eq!(judge_selftest(&report(vec![check("3 runs")])), Outcome::Passed);
}

#[test]
fn a_listing_runs_nothing_and_a_run_performs_only_the_steps_that_are_not_skipped() {
    let fake = complete();
    let mut performed = Vec::new();
    let listing = Options::parse(&strings(&["--list", "--paid"])).unwrap();
    carry_out(&listing, &context(&fake), &mut |action| {
        performed.push(action.to_string());
        Outcome::Passed
    })
    .unwrap();
    assert!(performed.is_empty());

    let bare = Fake::default();
    carry_out(&everything(), &context(&bare), &mut |action| {
        performed.push(action.to_string());
        Outcome::Failed("ran".into())
    })
    .unwrap();
    assert!(performed.is_empty(), "a bare Mac skips every step and passes");

    let only_git = Options::parse(&strings(&["--only", "git"])).unwrap();
    let steps = steps_of(&fake, "git");
    let failed = carry_out(&only_git, &context(&fake), &mut |action| {
        performed.push(action.to_string());
        if performed.len() == 1 {
            Outcome::Failed("broke".into())
        } else {
            Outcome::Skipped("not today".into())
        }
    })
    .unwrap_err();
    assert_eq!(failed, format!("integration failed: git / {}", steps[0].name));
    assert_eq!(performed.len(), steps.len());
    assert_eq!(performed[0], steps[0].action.to_string());
}

#[test]
fn an_ignored_test_runs_alone_with_its_variables_and_its_output_is_kept() {
    let env = vec![("IDE_X".to_string(), "1".to_string())];
    let command = test_command("ide-lsp", "servers", "gopls_answers", &env);
    let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(
        args,
        [
            "test",
            "-p",
            "ide-lsp",
            "--test",
            "servers",
            "--",
            "--ignored",
            "--exact",
            "gopls_answers",
            "--nocapture",
            "--test-threads",
            "1"
        ]
    );
    assert!(
        command
            .get_envs()
            .any(|(key, value)| key == "IDE_X" && value == Some("1".as_ref()))
    );
    assert_eq!(command.get_current_dir(), Some(root().as_path()));

    let mut talking = Command::new("/bin/sh");
    talking.args(["-c", "echo running 1 test; echo said >&2; exit 3"]);
    let (success, mut lines) = shown(&mut talking).unwrap();
    lines.sort();
    assert!(!success);
    assert_eq!(lines, ["running 1 test", "said"]);
    assert!(shown(&mut Command::new("/nonexistent/cargo")).is_err());
}

#[test]
fn the_report_counts_the_outcomes_and_names_each_failed_step() {
    let results = [
        ("git", "status".to_string(), Outcome::Passed),
        ("ci", "gh".to_string(), Outcome::Skipped("no token".into())),
        ("lsp", "gopls".to_string(), Outcome::Failed("crashed".into())),
        ("dap", "lldb".to_string(), Outcome::Failed("hung".into())),
    ];
    assert_eq!(
        report(&results).unwrap_err(),
        "integration failed: lsp / gopls, dap / lldb"
    );
    assert!(report(&results[..2]).is_ok());
    assert_eq!(summary(&results[1].2), "skipped: no token");
    assert_eq!(summary(&results[2].2), "FAILED: crashed");
}
