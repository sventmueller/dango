//! The arguments of `cargo xtask compare` and the two apps it knows.

use std::path::PathBuf;

/// An app the comparison measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum App {
    /// This product, as installed from its release.
    Ours,
    Zed,
}

impl App {
    pub(crate) const ALL: [App; 2] = [App::Ours, App::Zed];

    /// The name on the command line and in the report.
    pub(crate) fn name(self) -> &'static str {
        match self {
            App::Ours => ide_product::COMMAND,
            App::Zed => "zed",
        }
    }

    /// The name people know it by.
    pub(crate) fn title(self) -> &'static str {
        match self {
            App::Ours => ide_product::NAME,
            App::Zed => "Zed",
        }
    }

    /// The installed bundle the comparison starts; `COMPARE_DANGO_APP` names another one of ours, such as a build of
    /// this checkout.
    pub(crate) fn bundle(self) -> PathBuf {
        let installed = PathBuf::from("/Applications").join(format!("{}.app", self.title()));
        match self {
            App::Ours => std::env::var_os("COMPARE_DANGO_APP").map_or(installed, PathBuf::from),
            App::Zed => installed,
        }
    }

    pub(crate) fn bundle_id(self) -> &'static str {
        match self {
            App::Ours => ide_product::BUNDLE_ID,
            App::Zed => "dev.zed.Zed",
        }
    }

    /// The program inside the bundle, by which its processes are told apart from every other one.
    pub(crate) fn executable(self) -> PathBuf {
        self.bundle().join("Contents/MacOS").join(match self {
            App::Ours => ide_product::NAME,
            App::Zed => "zed",
        })
    }

    fn named(name: &str) -> Option<App> {
        App::ALL.into_iter().find(|app| app.name() == name)
    }
}

/// A part of the comparison that `--only` can pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    TypingRust,
    TypingMarkdown,
    Scroll,
    Tabs,
    Cold,
}

impl Phase {
    pub(crate) const ALL: [Phase; 5] = [
        Phase::TypingRust,
        Phase::TypingMarkdown,
        Phase::Scroll,
        Phase::Tabs,
        Phase::Cold,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Phase::TypingRust => "typing-rust",
            Phase::TypingMarkdown => "typing-markdown",
            Phase::Scroll => "scroll",
            Phase::Tabs => "tabs",
            Phase::Cold => "cold",
        }
    }
}

/// What the run measures and where it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) apps: Vec<App>,
    /// Samples per latency measurement.
    pub(crate) runs: usize,
    /// Launches for the cold start.
    pub(crate) cold_runs: usize,
    pub(crate) out: PathBuf,
    /// Only check permissions and apps, then end.
    pub(crate) check: bool,
    /// The phases to run, in their fixed order.
    pub(crate) phases: Vec<Phase>,
    /// Save the first sample's frames before and after its input as PNG.
    pub(crate) debug_frames: bool,
}

impl Options {
    pub(crate) fn runs_phase(&self, phase: Phase) -> bool {
        self.phases.contains(&phase)
    }
}

pub(crate) const DEFAULT_RUNS: usize = 30;
pub(crate) const DEFAULT_COLD_RUNS: usize = 5;
const MAX_RUNS: usize = 1000;

/// The options in `args`; `out` is where the report goes without `--out`.
pub(crate) fn parse(args: &[String], out: PathBuf) -> Result<Options, String> {
    let mut options = Options {
        apps: App::ALL.to_vec(),
        runs: DEFAULT_RUNS,
        cold_runs: DEFAULT_COLD_RUNS,
        out,
        check: false,
        phases: Phase::ALL.to_vec(),
        debug_frames: false,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = || rest.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--apps" | "--app" => options.apps = apps(value()?)?,
            "--only" => options.phases = phases(value()?)?,
            "--debug-frames" => options.debug_frames = true,
            "--runs" => options.runs = count(arg, value()?)?,
            "--cold" => options.cold_runs = count(arg, value()?)?,
            "--out" => options.out = PathBuf::from(value()?),
            "--check" => options.check = true,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(options)
}

fn apps(list: &str) -> Result<Vec<App>, String> {
    let mut apps = Vec::new();
    for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let app = App::named(name)
            .ok_or_else(|| format!("unknown app {name}; known are {} and zed", ide_product::COMMAND))?;
        if !apps.contains(&app) {
            apps.push(app);
        }
    }
    if apps.is_empty() {
        return Err("--apps names no app".into());
    }
    Ok(apps)
}

fn phases(list: &str) -> Result<Vec<Phase>, String> {
    let names: Vec<&str> = list.split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
    if let Some(unknown) = names.iter().find(|n| !Phase::ALL.iter().any(|p| p.name() == **n)) {
        let known: Vec<&str> = Phase::ALL.iter().map(|p| p.name()).collect();
        return Err(format!("unknown phase {unknown}; known are {}", known.join(", ")));
    }
    let phases: Vec<Phase> = Phase::ALL.into_iter().filter(|p| names.contains(&p.name())).collect();
    if phases.is_empty() {
        return Err("--only names no phase".into());
    }
    Ok(phases)
}

fn count(flag: &str, value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(n) if (1..=MAX_RUNS).contains(&n) => Ok(n),
        _ => Err(format!("{flag} takes a number from 1 to {MAX_RUNS}, not {value:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn without_arguments_both_apps_are_measured_thirty_times() {
        let options = parse(&[], PathBuf::from("target/compare")).unwrap();
        assert_eq!(options.apps, [App::Ours, App::Zed]);
        assert_eq!((options.runs, options.cold_runs, options.check), (30, 5, false));
        assert_eq!(options.out, PathBuf::from("target/compare"));
    }

    #[test]
    fn the_options_pick_apps_counts_and_folder() {
        let apps = format!("zed, {},zed", ide_product::COMMAND);
        let args = strings(&[
            "--apps", &apps, "--runs", "7", "--cold", "2", "--out", "/tmp/c", "--check",
        ]);
        let options = parse(&args, PathBuf::new()).unwrap();
        assert_eq!(options.apps, [App::Zed, App::Ours]);
        assert_eq!((options.runs, options.cold_runs, options.check), (7, 2, true));
        assert_eq!(options.out, PathBuf::from("/tmp/c"));
    }

    #[test]
    fn only_picks_phases_in_their_order_and_app_picks_one_app() {
        let args = strings(&["--app", "zed", "--only", "cold,typing-rust", "--debug-frames"]);
        let options = parse(&args, PathBuf::new()).unwrap();
        assert_eq!(options.apps, [App::Zed]);
        assert_eq!(options.phases, [Phase::TypingRust, Phase::Cold]);
        assert!(options.debug_frames && options.runs_phase(Phase::Cold) && !options.runs_phase(Phase::Tabs));
        assert_eq!(parse(&[], PathBuf::new()).unwrap().phases, Phase::ALL);
        assert!(parse(&strings(&["--only", "typing"]), PathBuf::new()).is_err());
        assert!(parse(&strings(&["--only", ","]), PathBuf::new()).is_err());
    }

    #[test]
    fn a_wrong_option_is_refused() {
        for args in [
            &["--apps", "vscode"][..],
            &["--apps", ","],
            &["--runs", "0"],
            &["--runs", "many"],
            &["--cold"],
            &["--fast"],
        ] {
            assert!(parse(&strings(args), PathBuf::new()).is_err(), "{args:?}");
        }
    }

    #[test]
    fn each_app_runs_from_its_installed_bundle() {
        assert_eq!(
            App::Zed.executable(),
            PathBuf::from("/Applications/Zed.app/Contents/MacOS/zed")
        );
        assert!(App::Ours.executable().starts_with("/Applications"));
        assert_ne!(App::Ours.bundle_id(), App::Zed.bundle_id());
    }
}
