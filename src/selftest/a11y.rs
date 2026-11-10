//! The `a11y` scenario, the VoiceOver check of each phase (E3): it opens the app's main states with fixtures,
//! recordings and stand-ins, never a real agent or the network, walks what VoiceOver gets from every window on screen
//! and reports what it would miss there, what Tab never reaches, and what turns wrongly right to left; `flows` then
//! goes through the main tasks with the keyboard alone. `a11y` runs in the Mac's language, `a11y-rtl` the same in
//! Arabic, right to left; the selftest application answers full keyboard access on for both. Each finding is reported
//! once, in the first state that shows it, and all of them are written to `<scenario>-findings.txt` beside the report.
//! See "Accessibility audit" in docs/development.md.

mod audit;
mod flows;
mod keys;
mod plan;
mod popover;
mod read;
mod states;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use ide_core::settings::Settings;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSUserInterfaceLayoutDirection, NSWindow};

use super::ci::stub::{Service, Stub};
use super::git::{append, fixture_git, write};
use super::{check, finish};
use audit::Finding;

/// The scenario the run is: `a11y` in the Mac's language, `a11y-rtl` in Arabic, right to left.
fn name() -> &'static str {
    if right_to_left_run() { "a11y-rtl" } else { "a11y" }
}

fn right_to_left_run() -> bool {
    std::env::var("IDE_SELFTEST").is_ok_and(|scenario| scenario == "a11y-rtl")
}

thread_local! {
    /// The findings reported so far, so a window that stays open names each only once.
    static REPORTED: RefCell<BTreeSet<Finding>> = const { RefCell::new(BTreeSet::new()) };
    /// The controls Tab leaves out on purpose, by the reason.
    static LEFT_OUT: RefCell<BTreeMap<&'static str, usize>> = const { RefCell::new(BTreeMap::new()) };
}

/// The CI stub the project's origin points at, for as long as the app runs.
static STUB: OnceLock<Arc<Stub>> = OnceLock::new();

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if let Some(path) = findings_file() {
        let _ = std::fs::write(path, "");
    }
    let app = NSApplication::sharedApplication(mtm);
    check(
        "the run reads in the direction of its language",
        right_to_left(mtm) == right_to_left_run(),
        format!("{:?}", app.userInterfaceLayoutDirection()),
    );
    let mac =
        objc2_foundation::NSUserDefaults::standardUserDefaults().integerForKey(&crate::util::ns("AppleKeyboardUIMode"));
    check(
        "full keyboard access is on for the run",
        app.isFullKeyboardAccessEnabled(),
        format!("on this Mac {}", if mac & 2 == 0 { "off" } else { "on" }),
    );
    walk("the welcome window misses nothing");
    std::thread::spawn(|| {
        let made = make_project();
        crate::util::main_soon(move || match made {
            Ok(root) => states::start(root),
            Err(error) => {
                check("the test project is made", false, error);
                finish(name());
            }
        });
    });
}

fn right_to_left(mtm: MainThreadMarker) -> bool {
    NSApplication::sharedApplication(mtm).userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft
}

/// A repository with a committed and a changed file of a language whose server is missing, an untracked file and a
/// file with an access key, whose origin is a CI stub that knows its HEAD.
fn make_project() -> Result<PathBuf, String> {
    let stub = Stub::start(Service::GitHub, None).map_err(|e| e.to_string())?;
    let folder = ide_core::paths::home().join("a11y-project");
    let git = fixture_git()?;
    std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
    let repo = git.init(&folder).map_err(|e| e.to_string())?;
    append(
        &repo.git_dir().join("config"),
        &format!(
            "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n[remote \"origin\"]\n\turl = \
             http://127.0.0.1:{}/acme/widget.git\n",
            stub.port
        ),
    )?;
    write(
        &folder,
        "README.md",
        "# A11y\n\nA project for the accessibility audit.\n",
    )?;
    write(
        &folder,
        states::SCRIPT,
        "export function greet(name: string): string {\n  return name;\n}\n",
    )?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", ide_git::CommitOptions::default())
        .map_err(|e| e.to_string())?;
    let sha = repo.head().map_err(|e| e.to_string())?.unwrap_or_default();
    let branch = repo.current_branch().map_err(|e| e.to_string())?.unwrap_or_default();
    stub.set_head(&sha, &branch);
    write(
        &folder,
        "README.md",
        "# A11y\n\nA project for the accessibility audit, changed.\n",
    )?;
    write(
        &folder,
        states::SCRIPT,
        "export function greet(name: string): string {\n  return `Hello ${name}`;\n}\n",
    )?;
    write(&folder, "notes.txt", "untracked\n")?;
    write(&folder, flows::CREDENTIALS, flows::FAKE_KEY)?;
    Settings::load_user()
        .set(
            &crate::ci_panel::CI_HOSTS,
            &vec![format!("127.0.0.1:{}=github", stub.port)],
        )
        .map_err(|e| e.to_string())?;
    let _ = STUB.set(stub);
    std::fs::create_dir_all(ide_core::paths::home().join(flows::RECENT)).map_err(|e| e.to_string())?;
    Ok(folder.canonicalize().unwrap_or(folder))
}

/// Walks every window on screen as the state `state` leaves them; the check fails with what is new there.
fn walk(state: &'static str) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let rtl = right_to_left(mtm);
    let mut found = Vec::new();
    for window in NSApplication::sharedApplication(mtm).windows().iter() {
        if !audited(&window) {
            continue;
        }
        let elements = audit::elements(&window);
        found.extend(audit::window_findings(&window));
        found.extend(audit::findings(&elements));
        if rtl {
            found.extend(audit::direction_findings(&window));
        }
        let (unreached, left_out) = keys::unreached(&window, &elements);
        found.extend(unreached);
        LEFT_OUT.with(|all| {
            for (reason, count) in left_out {
                *all.borrow_mut().entry(reason).or_default() += count;
            }
        });
    }
    let new: Vec<Finding> = REPORTED.with(|reported| {
        let mut reported = reported.borrow_mut();
        found.into_iter().filter(|f| reported.insert(f.clone())).collect()
    });
    write_findings(state, &new);
    check(state, new.is_empty(), summary(&new));
}

/// Whether `window` is one a person sees and works in, not a tooltip or the frame of a menu.
fn audited(window: &NSWindow) -> bool {
    let class = window.class().name().to_string_lossy().into_owned();
    window.isVisible() && window.frame().size.width > 1.0 && !class.contains("ToolTip") && !class.contains("Menu")
}

/// The findings, those of the same element and problem in one line with their number and the first place.
fn summary(found: &[Finding]) -> String {
    let mut grouped: BTreeMap<(&str, &str), (usize, &str)> = BTreeMap::new();
    for finding in found {
        let entry = grouped
            .entry((&finding.problem, &finding.what))
            .or_insert((0, &finding.path));
        entry.0 += 1;
    }
    grouped
        .into_iter()
        .map(|((problem, what), (count, path))| {
            let times = if count > 1 {
                format!(" ×{count}")
            } else {
                String::new()
            };
            format!("{problem}: {what}{times} at {path}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The file beside the report that lists every finding, one per line: state, problem, element, path.
fn findings_file() -> Option<PathBuf> {
    let report = std::env::var_os("IDE_SELFTEST_REPORT")?;
    Some(PathBuf::from(report).with_file_name(format!("{}-findings.txt", name())))
}

fn write_findings(state: &str, found: &[Finding]) {
    let Some(path) = findings_file() else { return };
    let file = std::fs::OpenOptions::new().create(true).append(true).open(&path);
    if let Ok(mut file) = file {
        for finding in found {
            let _ = writeln!(file, "{state}\t{}\t{}\t{}", finding.problem, finding.what, finding.path);
        }
    }
}

/// Reports what Tab leaves out on purpose and ends the run.
fn end() {
    let left_out = LEFT_OUT.with(|all| {
        all.borrow()
            .iter()
            .map(|(reason, count)| format!("{count} for {reason}"))
            .collect::<Vec<_>>()
            .join("; ")
    });
    check("controls outside the Tab loop have keys of their own", true, left_out);
    finish(name());
}

/// A picture of the window of `root` as `a11y-<name>.png`, which shows the layout right to left.
fn picture(root: &Path, name: &str) {
    if let Some(window) = super::helpers::window_of(root).and_then(|c| c.window()) {
        super::snapshot(&window, &format!("{}-{name}", self::name()));
    }
}

/// The window of the folder `root`; its absence fails the run.
fn window_of(root: &Path) -> Option<objc2::rc::Retained<crate::window::WindowController>> {
    let controller = super::helpers::window_of(root);
    if controller.is_none() {
        check("the folder keeps its window", false, root.display().to_string());
        finish(name());
    }
    controller
}
