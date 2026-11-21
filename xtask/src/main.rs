//! Build tasks in the language of the project: the app bundle, its signature, the selftest and the release with its
//! disk image, update feed and Homebrew Cask. The Makefile names the entry points.

#![forbid(unsafe_code)]

mod baseline;
mod bundle;
mod cask;
mod ci;
mod claude_latest;
mod coverage;
mod deploy;
mod focus;
mod fuzz;
mod icon;
mod integration;
mod licenses;
mod release;
mod rules;
mod seal;
mod seed;
mod selftest;
mod shell;
mod source;
mod structure;
mod sweep;
mod thread_locals;
mod toolchain;
mod universal;
mod updates;

use std::path::PathBuf;
use std::process::{Command, ExitCode};

pub(crate) use bundle::*;
pub(crate) use icon::*;
pub(crate) use licenses::*;
pub(crate) use selftest::*;
pub(crate) use shell::{Shell, System};
pub(crate) use universal::*;

/// The package that builds the app.
pub(crate) const PACKAGE: &str = "ide-app";

/// The program the package builds, which the bundle carries under the product's name.
pub(crate) const PROGRAM: &str = "ide";

pub(crate) const USAGE: &str = "usage: cargo xtask <task>
  bundle [--release] [--selftest]  build and sign target/<profile>/<name>.app without changing VERSION
  run [--release]                  bundle, then open the app
  selftest [<scenario>…|--offline] bundle the release with the selftest built in (and dictation for its scenario), then run each scenario (probe if none,
                                   every one without network, account or person with --offline); compares the pictures
                                   that have accepted ones in tests/snapshots
  selftest-accept <scenario> [<picture>…]
                                   take the pictures of the last run of <scenario> as its accepted ones (all it already
                                   has if none are named)
  integration [--list] [--paid] [--only <group>[,<group>…]]
                                   the checks against real servers, CLIs and services with this Mac's sign-ins;
                                   --list only says what would run and why the rest is skipped; the groups that use
                                   paid accounts run only with --paid or when --only names them
  release [--notarize] [--layout]  the universal app with dictation, disk image, update feed and Homebrew Cask. Without
                                   --notarize a dry run into target/release-dry-run: signed with the local certificate,
                                   nothing sent to Apple or anywhere, VERSION unchanged; --layout has Finder lay out the
                                   image window, which opens it on screen. With --notarize: Developer ID, notarized and
                                   stapled, into dist/; VERSION unchanged
  universal                        target/universal/ide with dictation for Apple Silicon and Intel; needs rustup
  acknowledgements                 target/Acknowledgements.txt, the license texts the distribution carries
  claude-latest [--signed-in]      install the newest Claude CLI into target/claude-latest and check the contract the
                                   app relies on without a sign-in; --signed-in adds a short turn with haiku (R7)
  fuzz [<target>…|properties]      run the crates' fuzz targets (all if none) for FUZZ_SECONDS each, 60 by default, with
                                   the pinned nightly; needs rustup and cargo-fuzz. properties (also part of all) runs
                                   the property tests with PROPTEST_CASES cases, 10000 by default, and a fresh seed
                                   it prints, or PROPTEST_RNG_SEED
  coverage [--check] [--strict]    line coverage of cargo test and the offline selftest scenarios, instrumented with
                                   rustup's toolchain and cargo-llvm-cov, into target/coverage/, with cargo test
                                   alone and the self-test alone beside it per crate, folder and file; --check fails below
                                   90 % in total or 50 % in any file, or when cargo test alone covers the app crate other
                                   than its floor; --strict when a test or scenario failed
  ci                               cargo fmt, then cargo deny's advisories, check, coverage --check --strict, perf and
                                   fuzz for 20 s per target, fail-fast; a complete run seals the working tree's content
                                   in the local ref refs/ci-seals/<green|red>/<fp>; refuses while the screen is locked
  seal fingerprint [--head]        the fingerprint of the working tree's content (or HEAD's): its git tree without
                                   VERSION, with the commits and changes of ../splat and ../lowspeak
  seal explain                     the seals of HEAD's content here and on origin, and the paths that separate it from
                                   the nearest green seal
  deploy                           on clean main with clean, pushed siblings: notarize the next version, commit VERSION,
                                   upload the draft, push main, its green seal if any and the version tag, then publish
                                   the release
  thread-locals                    fail for a thread-local that docs/thread-locals.md does not allow to stay
  toolchain                        fail when rustc is not the version rust-toolchain.toml pins or a package's
                                   rust-version is not its minor version
  structure                        fail for a Rust file over 1000 lines or a model.rs that uses objc2 (E98)
  sweep [--dry-run]                remove from the build folders cargo's build-dir shares between all workspaces what no
                                   build read lately: every unit of a worktree that is gone, a unit its workspace's
                                   builds have not read for 7 days, every unit of a workspace unread for 30 days, then
                                   the least recently read beyond 80 GB, but none read in the last 2 days; skips a folder
                                   a build holds. make check runs it first
  seed <worktree>|--hook           clone the dependencies this checkout built into the build folder of one of its
                                   worktrees, which then builds only its own crates; the post-checkout hook in
                                   xtask/hooks runs it for git worktree add, Claude Code's PreToolUse hook with --hook
                                   once per worktree, which it reads from the hook's input
  rules                            fail for unlisted stateful statics, mem::forget, unguarded blocks, literal sizes,
                                   unseen thread failures, errors as String, and lookups of global state past their
                                   ratchets in the app's AppKit code, for a discarded spawn anywhere, and for a
                                   self-test scenario that ends early without give_up or skip
  icon                             compile assets/AppIcon.icon into assets/Assets.car and assets/AppIcon.icns; needs
                                   Xcode";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match task(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn task(args: &[String]) -> Result<(), String> {
    let (task, rest) = args.split_first().ok_or(USAGE)?;
    match task.as_str() {
        "bundle" => {
            let [release, selftest] = flags(rest, ["--release", "--selftest"])?;
            let app = bundle(Build::of(release, selftest))?;
            println!("{}", app.display());
            Ok(())
        }
        "run" => {
            let [release] = flags(rest, ["--release"])?;
            let app = bundle(Build::of(release, false))?;
            println!("{}", app.display());
            run(Command::new("open").arg("-n").arg(&app))
        }
        "selftest" => selftest(&scenarios(rest)?),
        "selftest-accept" => match rest.split_first() {
            Some((scenario, pictures)) if is_scenario(scenario) && pictures.iter().all(|p| is_scenario(p)) => {
                baseline::accept(scenario, pictures)
            }
            _ => Err(format!("selftest-accept needs a scenario and picture names\n{USAGE}")),
        },
        "integration" => integration::integration(rest),
        "claude-latest" => claude_latest::claude_latest(rest),
        "fuzz" => fuzz::fuzz(rest),
        "coverage" => coverage::coverage(rest),
        "ci" => ci::ci(rest),
        "seal" => seal::seal(rest),
        "deploy" => deploy::deploy(rest),
        "release" => {
            let [notarize, layout] = flags(rest, ["--notarize", "--layout"])?;
            let out = release::release(release::Options {
                notarize,
                layout: layout || notarize,
            })?;
            println!("{}", out.display());
            Ok(())
        }
        "universal" => {
            flags(rest, [])?;
            for program in universal_programs(&mut System, &[PROGRAM], &Build::Distribution.features())? {
                println!("{}", program.display());
            }
            Ok(())
        }
        "acknowledgements" => {
            flags(rest, [])?;
            let file = target_dir().join(ACKNOWLEDGEMENTS);
            files::write(&file, acknowledgements(&Build::Distribution.features(), None)?)?;
            println!("{}", file.display());
            Ok(())
        }
        "icon" => {
            flags(rest, [])?;
            compile_icon()
        }
        "thread-locals" => {
            flags(rest, [])?;
            thread_locals::check()
        }
        "structure" => {
            flags(rest, [])?;
            structure::check()
        }
        "rules" => {
            flags(rest, [])?;
            rules::check()
        }
        "toolchain" => {
            flags(rest, [])?;
            toolchain::check()
        }
        "sweep" => sweep::sweep(rest),
        "seed" => seed::seed(rest),
        _ => Err(USAGE.into()),
    }
}

/// Which of `known` the arguments name; any other argument is an error.
pub(crate) fn flags<const N: usize>(args: &[String], known: [&str; N]) -> Result<[bool; N], String> {
    if let Some(unknown) = args.iter().find(|a| !known.contains(&a.as_str())) {
        return Err(format!("unknown argument {unknown}\n{USAGE}"));
    }
    Ok(known.map(|flag| args.iter().any(|a| a == flag)))
}

/// The scenarios the arguments name, or the default one.
pub(crate) fn scenarios(args: &[String]) -> Result<Vec<String>, String> {
    if let Some(odd) = args.iter().find(|a| !is_scenario(a) && *a != OFFLINE) {
        return Err(format!("{odd:?} is no scenario name\n{USAGE}"));
    }
    if args.is_empty() {
        Ok(vec![DEFAULT_SCENARIO.to_string()])
    } else {
        Ok(args.to_vec())
    }
}

pub(crate) fn root() -> PathBuf {
    // `cargo run` sets the variable for the run as well; the compiled-in path is wrong for a binary built in another
    // checkout whose target folder was copied here.
    let manifest =
        std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    manifest.parent().expect("xtask lives in the workspace").to_path_buf()
}

/// Where cargo builds: `CARGO_TARGET_DIR` if set, else `target/`.
pub(crate) fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root().join("target"), |dir| root().join(dir))
}

pub(crate) fn run(cmd: &mut Command) -> Result<(), String> {
    System.run(cmd)
}

pub(crate) fn stdout(cmd: &mut Command) -> Result<String, String> {
    System.stdout(cmd)
}

/// Major, minor and build number, digits only.
pub(crate) fn is_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// The current version from VERSION. Only deploy changes it.
pub(crate) fn version() -> Result<String, String> {
    let path = root().join("VERSION");
    let text = files::read_to_string(&path)?;
    let version = text.trim();
    if is_version(version) {
        Ok(version.to_string())
    } else {
        Err(format!("{}: {version:?} is not major.minor.build", path.display()))
    }
}

/// The version after `version`: the same major and minor, the next build number.
pub(crate) fn following(version: &str) -> Result<String, String> {
    if !is_version(version) {
        return Err(format!("{version:?} is not major.minor.build"));
    }
    let (head, build) = version.rsplit_once('.').ok_or("no build number")?;
    let build: u64 = build.parse().map_err(|e| format!("build number {build}: {e}"))?;
    let next = build.checked_add(1).ok_or("the build number is at its maximum")?;
    Ok(format!("{head}.{next}"))
}

/// `std::fs` with the path in every error.
pub(crate) mod files {
    use std::fs;
    use std::path::Path;

    fn at<T>(path: &Path, result: std::io::Result<T>) -> Result<T, String> {
        result.map_err(|e| format!("{}: {e}", path.display()))
    }

    pub(crate) fn write(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<(), String> {
        at(path.as_ref(), fs::write(path.as_ref(), content))
    }

    pub(crate) fn read_to_string(path: impl AsRef<Path>) -> Result<String, String> {
        at(path.as_ref(), fs::read_to_string(path.as_ref()))
    }

    pub(crate) fn read_dir(path: impl AsRef<Path>) -> Result<fs::ReadDir, String> {
        at(path.as_ref(), fs::read_dir(path.as_ref()))
    }

    pub(crate) fn create_dir_all(path: impl AsRef<Path>) -> Result<(), String> {
        at(path.as_ref(), fs::create_dir_all(path.as_ref()))
    }

    pub(crate) fn remove_dir_all(path: impl AsRef<Path>) -> Result<(), String> {
        at(path.as_ref(), fs::remove_dir_all(path.as_ref()))
    }

    pub(crate) fn remove_file(path: impl AsRef<Path>) -> Result<(), String> {
        at(path.as_ref(), fs::remove_file(path.as_ref()))
    }

    pub(crate) fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), String> {
        at(from.as_ref(), fs::rename(from.as_ref(), to.as_ref()))
    }

    pub(crate) fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<u64, String> {
        at(from.as_ref(), fs::copy(from.as_ref(), to.as_ref()))
    }

    /// A folder only its owner can enter.
    pub(crate) fn create_private_dir(path: impl AsRef<Path>) -> Result<(), String> {
        let mut builder = fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        at(path.as_ref(), builder.create(path.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn versions_have_three_numbers() {
        assert!(is_version("0.1.0") && is_version("12.3.456"));
        assert!(!is_version("1.0") && !is_version("1.0.x") && !is_version("1..1") && !is_version("1.0.1.2"));
        assert!(!is_version("") && !is_version("1.0.-1") && !is_version(" 1.0.1"));
    }

    #[test]
    fn a_deploy_raises_only_the_build_number() {
        assert_eq!(following("0.1.0").unwrap(), "0.1.1");
        assert_eq!(following("2.3.99").unwrap(), "2.3.100");
        assert!(following("1.0").is_err() && following("1.0.x").is_err());
        assert!(following(&format!("1.0.{}", u64::MAX)).is_err());
    }

    #[test]
    fn the_version_file_of_the_repository_is_valid() {
        assert!(version().is_ok());
    }

    #[test]
    fn a_task_takes_only_the_options_it_knows() {
        assert_eq!(flags(&[], ["--release", "--selftest"]).unwrap(), [false, false]);
        assert_eq!(
            flags(&strings(&["--selftest"]), ["--release", "--selftest"]).unwrap(),
            [false, true]
        );
        assert_eq!(flags(&strings(&["--release"]), ["--release"]).unwrap(), [true]);
        assert!(flags(&strings(&["--selftest"]), ["--release"]).is_err());
        assert!(flags(&strings(&["--relase"]), ["--release"]).is_err());
    }

    #[test]
    fn a_task_with_an_option_it_does_not_know_does_nothing_but_explain() {
        assert_eq!(task(&[]).unwrap_err(), USAGE);
        assert_eq!(task(&strings(&["no-such-task"])).unwrap_err(), USAGE);
        assert_eq!(task(&strings(&["seal"])).unwrap_err(), USAGE);
        assert_eq!(task(&strings(&["seal", "mint"])).unwrap_err(), USAGE);
        for args in [
            &["bundle", "--fast"][..],
            &["run", "--selftest"],
            &["claude-latest", "--paid"],
            &["coverage", "--html"],
            &["ci", "--fast"],
            &["deploy", "--force"],
            &["seal", "fingerprint", "--tree"],
            &["seal", "explain", "x"],
            &["release", "--upload"],
            &["universal", "--release"],
            &["acknowledgements", "x"],
            &["icon", "x"],
            &["thread-locals", "x"],
            &["structure", "x"],
            &["rules", "x"],
            &["sweep", "--force"],
        ] {
            let refused = task(&strings(args)).unwrap_err();
            assert!(refused.starts_with("unknown argument "), "{args:?}: {refused}");
            assert!(refused.ends_with(USAGE), "{args:?}");
        }
        assert!(
            task(&strings(&["selftest", "../escape"]))
                .unwrap_err()
                .contains("is no scenario name")
        );
        assert!(task(&strings(&["integration", "--only"])).is_err());
        assert!(
            task(&strings(&["fuzz", "no-such-target"]))
                .unwrap_err()
                .contains("no fuzz target")
        );
        assert_eq!(task(&strings(&["thread-locals"])), Ok(()));
    }

    #[test]
    fn the_selftest_runs_the_probe_unless_told_otherwise() {
        assert_eq!(scenarios(&[]).unwrap(), ["probe"]);
        assert_eq!(
            scenarios(&strings(&["probe", "editor-typing"])).unwrap(),
            ["probe", "editor-typing"]
        );
        assert!(scenarios(&strings(&["--release"])).is_err());
        assert_eq!(scenarios(&strings(&[OFFLINE])).unwrap(), [OFFLINE]);
        assert!(scenarios(&strings(&["../escape"])).is_err());
    }
}
