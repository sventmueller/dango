//! `cargo xtask integration`, behind `make test-integration`: the checks against the real language servers, agent
//! CLIs, git and CI services of this Mac with its sign-ins, which `make check` leaves out (docs/plan.md,
//! "Verifikation"). Every step names what it needs; a step whose program, sign-in or network is missing is skipped with
//! the reason and never fails the run. Each group says whether it uses the owner's paid accounts; those groups run only
//! when `--only` names them or `--paid` asks for them all.

mod probe;
mod steps;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

pub(crate) use probe::{Machine, Probe};
pub(crate) use steps::GROUPS;

use crate::*;

/// Steps that are selected and run together.
pub(crate) struct Group {
    /// The name `--only` takes.
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    /// Whether it starts a coding agent signed in to one of the owner's paid accounts.
    pub(crate) paid: bool,
    /// What it uses: accounts, downloads, tokens.
    pub(crate) uses: &'static str,
    pub(crate) steps: fn(&Context) -> Vec<Step>,
}

/// What the steps are put together with.
pub(crate) struct Context<'a> {
    pub(crate) probe: &'a dyn Probe,
    /// Where the ACP tests leave their recordings.
    pub(crate) recordings: PathBuf,
}

pub(crate) struct Step {
    pub(crate) name: String,
    pub(crate) needs: Vec<Need>,
    pub(crate) action: Action,
}

/// What a step needs on this Mac; without it the step is skipped.
pub(crate) enum Need {
    /// A program on the login shell's PATH.
    Program(&'static str),
    /// At least one of these programs on the login shell's PATH.
    AnyProgram(&'static [&'static str]),
    /// rust-analyzer on the login shell's PATH or through rustup, as the language server registry looks for it.
    RustAnalyzer,
    /// Apple's developer tools, which bring clangd and sourcekit-lsp.
    DeveloperTools,
    /// macOS lets debuggers take control of other processes without asking for a password each time
    /// (`DevToolsSecurity -enable`).
    Debugging,
    /// A host that takes connections on port 443.
    Network(&'static str),
    /// A sign-in with `service`, shown by any of `evidence`.
    SignIn {
        service: &'static str,
        evidence: &'static [Evidence],
    },
    /// A token the app's CI finds for `host` (`ide_ci::Auth`): `tool`, gh or glab, signed in to it, or the variable
    /// of the login shell that tool reads for it.
    CiToken { tool: &'static str, host: String },
    /// Something found missing while the step was put together.
    Unmet(String),
}

/// What shows a sign-in without using it.
pub(crate) enum Evidence {
    /// A variable of the login shell.
    Var(&'static str),
    /// A file under the home folder.
    File(&'static str),
    /// An item of the login keychain, by its service; only its presence is asked.
    Keychain(&'static str),
}

pub(crate) enum Action {
    /// One test that is ignored in `make check`, run alone with its output shown.
    Test {
        package: &'static str,
        file: &'static str,
        name: &'static str,
        env: Vec<(String, String)>,
    },
    /// A selftest scenario, with these variables for the app.
    Selftest {
        scenario: &'static str,
        env: Vec<(String, String)>,
    },
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (env, command) = match self {
            Action::Test {
                package,
                file,
                name,
                env,
            } => (
                env,
                format!("cargo test -p {package} --test {file} -- --ignored --exact {name}"),
            ),
            Action::Selftest { scenario, env } => (env, format!("cargo xtask selftest {scenario}")),
        };
        for (key, value) in env {
            write!(f, "{key}={value} ")?;
        }
        f.write_str(&command)
    }
}

/// How a step went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Passed,
    Skipped(String),
    Failed(String),
}

/// What `cargo xtask integration` was asked for.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Options {
    /// Only say what would run and what is skipped why.
    pub(crate) list: bool,
    /// The groups to run; the unpaid ones when empty.
    pub(crate) only: Vec<String>,
    /// The paid groups run as well.
    pub(crate) paid: bool,
}

impl Options {
    pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
        let mut options = Options::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--list" => options.list = true,
                "--paid" => options.paid = true,
                "--only" => {
                    let value = args.next().ok_or("--only needs a group")?;
                    for name in value.split(',').filter(|n| !n.is_empty()) {
                        if !GROUPS.iter().any(|g| g.name == name) {
                            let known: Vec<&str> = GROUPS.iter().map(|g| g.name).collect();
                            return Err(format!("no group {name:?}; the groups are {}", known.join(", ")));
                        }
                        if !options.only.iter().any(|o| o == name) {
                            options.only.push(name.to_string());
                        }
                    }
                }
                other => return Err(format!("unknown argument {other}\n{USAGE}")),
            }
        }
        Ok(options)
    }

    pub(crate) fn selects(&self, group: &Group) -> bool {
        let named = self.only.iter().any(|name| name == group.name);
        named || (self.paid && group.paid) || (self.only.is_empty() && !group.paid)
    }

    /// What the run says about the owner's paid accounts before it starts: the paid groups it runs, else those it
    /// leaves out unasked.
    pub(crate) fn paid_notice(&self) -> Option<String> {
        let names = |selected: bool| -> Vec<&str> {
            GROUPS
                .iter()
                .filter(|g| g.paid && self.selects(g) == selected)
                .map(|g| g.name)
                .collect()
        };
        let starting = names(true);
        if !starting.is_empty() {
            return Some(format!(
                "paid groups in this run, which use the owner's accounts: {}",
                starting.join(", ")
            ));
        }
        let left_out = names(false);
        (self.only.is_empty() && !left_out.is_empty()).then(|| {
            format!(
                "not run, as they use the owner's paid accounts: {}; name them in --only or add --paid",
                left_out.join(", ")
            )
        })
    }
}

/// Why `need` is not met on this Mac, if it is not.
pub(crate) fn unmet(need: &Need, probe: &dyn Probe) -> Option<String> {
    match need {
        Need::Program(name) => probe
            .program(name)
            .is_none()
            .then(|| format!("{name} is not on the login shell's PATH")),
        Need::AnyProgram(names) => names
            .iter()
            .all(|name| probe.program(name).is_none())
            .then(|| format!("none of {} is on the login shell's PATH", names.join(", "))),
        Need::RustAnalyzer => {
            let through_rustup = || {
                probe
                    .program("rustup")
                    .is_some_and(|rustup| probe.succeeds(&rustup, &["which", "rust-analyzer"]))
            };
            (probe.program("rust-analyzer").is_none() && !through_rustup())
                .then(|| "rust-analyzer is neither on the login shell's PATH nor installed through rustup".into())
        }
        Need::DeveloperTools => (!probe.succeeds(Path::new("/usr/bin/xcode-select"), &["-p"]))
            .then(|| "Apple's developer tools are not installed".into()),
        Need::Debugging => (!probe.succeeds(
            Path::new("/bin/sh"),
            &["-c", "/usr/sbin/DevToolsSecurity -status | /usr/bin/grep -q 'currently enabled'"],
        ))
        .then(|| "developer mode is off, so macOS asks for a password at every launch; turn it on with sudo DevToolsSecurity -enable".into()),
        Need::Network(host) => (!probe.reachable(host)).then(|| format!("{host} cannot be reached")),
        Need::SignIn { service, evidence } => (!evidence.iter().any(|e| shows(e, probe))).then(|| {
            let looked: Vec<String> = evidence.iter().map(describe).collect();
            format!("no sign-in with {service}: none of {}", looked.join(", "))
        }),
        Need::CiToken { tool, host } => (!ci_token(tool, host, probe))
            .then(|| format!("{tool} is not signed in to {host}, and the login shell holds no token for it")),
        Need::Unmet(why) => Some(why.clone()),
    }
}

/// Whether the app's CI finds a token for `host` through `tool` or the variables it reads, each only for its host.
pub(crate) fn ci_token(tool: &str, host: &str, probe: &dyn Probe) -> bool {
    let set = |name: &str| probe.var(name).is_some_and(|value| !value.trim().is_empty());
    let by_variable = match tool {
        "gh" => host == "github.com" && (set("GH_TOKEN") || set("GITHUB_TOKEN")),
        "glab" => {
            let named = probe.var("GITLAB_HOST").filter(|h| !h.trim().is_empty());
            let meant = named.map_or_else(|| "gitlab.com".to_string(), |h| remote_host(&h).unwrap_or(h));
            meant == host && set("GITLAB_TOKEN")
        }
        _ => false,
    };
    by_variable
        || probe
            .program(tool)
            .is_some_and(|path| probe.succeeds(&path, &["auth", "status", "--hostname", host]))
}

fn shows(evidence: &Evidence, probe: &dyn Probe) -> bool {
    match evidence {
        Evidence::Var(name) => probe.var(name).is_some_and(|value| !value.is_empty()),
        Evidence::File(path) => probe.exists(&probe.home().join(path)),
        Evidence::Keychain(service) => probe.succeeds(
            Path::new("/usr/bin/security"),
            &["find-generic-password", "-s", service],
        ),
    }
}

fn describe(evidence: &Evidence) -> String {
    match evidence {
        Evidence::Var(name) => format!("${name}"),
        Evidence::File(path) => format!("~/{path}"),
        Evidence::Keychain(service) => format!("the keychain item {service:?}"),
    }
}

/// A step with the reason it is skipped, if it is.
pub(crate) struct Planned {
    pub(crate) step: Step,
    pub(crate) skip: Option<String>,
}

/// The steps of the groups `options` selects, each with why it is skipped, if it is.
pub(crate) fn plan(options: &Options, cx: &Context) -> Vec<(&'static Group, Vec<Planned>)> {
    GROUPS
        .iter()
        .filter(|group| options.selects(group))
        .map(|group| {
            let steps = (group.steps)(cx)
                .into_iter()
                .map(|step| {
                    let skip = step.needs.iter().find_map(|need| unmet(need, cx.probe));
                    Planned { step, skip }
                })
                .collect();
            (group, steps)
        })
        .collect()
}

pub(crate) fn heading(group: &Group) -> String {
    let paid = if group.paid {
        "uses the owner's paid accounts"
    } else {
        "no paid account"
    };
    format!("{}: {} ({paid}; {})", group.name, group.title, group.uses)
}

/// What `--list` prints for one group.
pub(crate) fn listing(group: &Group, steps: &[Planned]) -> Vec<String> {
    let mut lines = vec![heading(group)];
    for planned in steps {
        match &planned.skip {
            None => lines.push(format!("  run   {}: {}", planned.step.name, planned.step.action)),
            Some(why) => lines.push(format!("  skip  {}: {why}", planned.step.name)),
        }
    }
    lines
}

/// Runs the checks `args` ask for, or lists them; the error names the steps that failed.
pub(crate) fn integration(args: &[String]) -> Result<(), String> {
    let options = Options::parse(args)?;
    let machine = Machine::new();
    let cx = Context {
        probe: &machine,
        recordings: target_dir().join("integration/acp"),
    };
    carry_out(&options, &cx, &mut perform)
}

/// Lists the steps `options` select or has `perform` run each that is not skipped, then reports.
fn carry_out(options: &Options, cx: &Context, perform: &mut dyn FnMut(&Action) -> Outcome) -> Result<(), String> {
    let plan = plan(options, cx);
    if let Some(notice) = options.paid_notice() {
        println!("{notice}");
    }
    if options.list {
        for (group, steps) in &plan {
            println!("{}", listing(group, steps).join("\n"));
        }
        return Ok(());
    }
    let mut results = Vec::new();
    for (group, steps) in plan {
        println!("== {}", heading(group));
        for planned in steps {
            let outcome = match planned.skip {
                Some(why) => Outcome::Skipped(why),
                None => {
                    println!("-> {}: {}", planned.step.name, planned.step.action);
                    perform(&planned.step.action)
                }
            };
            println!("   {}: {}", planned.step.name, summary(&outcome));
            results.push((group.name, planned.step.name, outcome));
        }
    }
    report(&results)
}

fn summary(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Passed => "passed".into(),
        Outcome::Skipped(why) => format!("skipped: {why}"),
        Outcome::Failed(why) => format!("FAILED: {why}"),
    }
}

/// Prints every step's outcome and fails if one failed.
fn report(results: &[(&str, String, Outcome)]) -> Result<(), String> {
    println!("== integration");
    for (group, step, outcome) in results {
        println!("  {group} / {step}: {}", summary(outcome));
    }
    let count = |f: fn(&Outcome) -> bool| results.iter().filter(|(_, _, o)| f(o)).count();
    let failed: Vec<String> = results
        .iter()
        .filter(|(_, _, o)| matches!(o, Outcome::Failed(_)))
        .map(|(group, step, _)| format!("{group} / {step}"))
        .collect();
    println!(
        "  {} passed, {} skipped, {} failed",
        count(|o| *o == Outcome::Passed),
        count(|o| matches!(o, Outcome::Skipped(_))),
        failed.len()
    );
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("integration failed: {}", failed.join(", ")))
    }
}

fn perform(action: &Action) -> Outcome {
    match action {
        Action::Test {
            package,
            file,
            name,
            env,
        } => match shown(&mut test_command(package, file, name, env)) {
            Ok((success, lines)) => judge_test(success, &lines),
            Err(e) => Outcome::Failed(e),
        },
        Action::Selftest { scenario, env } => match selftest_with(&[(*scenario).to_string()], env) {
            Err(e) => Outcome::Failed(e),
            Ok(()) => {
                let file = target_dir().join("selftest").join(format!("{scenario}.json"));
                let written = std::fs::read(file).ok();
                match parse_report(scenario, written.as_deref()) {
                    Ok(report) => judge_selftest(&report),
                    Err(e) => Outcome::Failed(e),
                }
            }
        },
    }
}

/// The ignored test `name` in the test file `file` of `package`, alone, with its output shown and `env` set.
fn test_command(package: &str, file: &str, name: &str, env: &[(String, String)]) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(root())
        .args([
            "test",
            "-p",
            package,
            "--test",
            file,
            "--",
            "--ignored",
            "--exact",
            name,
        ])
        .args(["--nocapture", "--test-threads", "1"])
        .envs(env.iter().map(|(key, value)| (key, value)));
    command
}

/// Runs `command`, showing its output as it comes, and keeps the lines; whether it succeeded.
pub(crate) fn shown(command: &mut Command) -> Result<(bool, Vec<String>), String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{command:?}: {e}"))?;
    let lines = Arc::new(Mutex::new(Vec::new()));
    let errors = child.stderr.take().map(|stderr| {
        let lines = Arc::clone(&lines);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("{line}");
                lines.lock().unwrap_or_else(|e| e.into_inner()).push(line);
            }
        })
    });
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("{line}");
            lines.lock().unwrap_or_else(|e| e.into_inner()).push(line);
        }
    }
    if let Some(errors) = errors {
        let _ = errors.join();
    }
    let status = child.wait().map_err(|e| format!("{command:?}: {e}"))?;
    let lines = std::mem::take(&mut *lines.lock().unwrap_or_else(|e| e.into_inner()));
    Ok((status.success(), lines))
}

/// How an ignored test went by its output: it must have run, alone, and may have said why it skipped itself.
pub(crate) fn judge_test(success: bool, lines: &[String]) -> Outcome {
    if !success {
        return Outcome::Failed("the test failed".into());
    }
    if !lines.iter().any(|line| line.trim() == "running 1 test") {
        return Outcome::Failed("no such ignored test ran".into());
    }
    lines
        .iter()
        .find_map(|line| line.trim().strip_prefix("integration skip: "))
        .map_or(Outcome::Passed, |why| Outcome::Skipped(why.to_string()))
}

/// How a scenario that passed went: skipped, when every check it reports says it was skipped.
pub(crate) fn judge_selftest(report: &Report) -> Outcome {
    let skipped: Vec<&str> = report
        .checks
        .iter()
        .filter_map(|check| check.detail.strip_prefix("skipped: "))
        .collect();
    if !skipped.is_empty() && skipped.len() == report.checks.len() {
        Outcome::Skipped(skipped.join("; "))
    } else {
        Outcome::Passed
    }
}

/// The host of a git remote URL: `https://host/…`, `ssh://user@host:port/…` or `user@host:path`.
pub(crate) fn remote_host(url: &str) -> Option<String> {
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest,
        None if !url.contains('/') || url.find(':') < url.find('/') => url,
        None => return None,
    };
    let authority = rest.split('/').next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_lowercase())
}

#[cfg(test)]
mod tests;
