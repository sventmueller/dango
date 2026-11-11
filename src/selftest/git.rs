//! The `git` scenario: in a new repository with a committed and a changed file, the Git panel lists the changes,
//! stages a file and a single hunk, discards a change, commits, pushes and fetches through the code its controls
//! run, shows diffs and counts the files on the badge; `flows` goes on with branches, diverged and conflicting
//! pulls and cancelling, `sheets` with a refusing hook and a staged secret. Pictures are taken in light and dark
//! appearance.

mod flows;
mod sheets;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_git::{CommitOptions, Git};
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{NSAppearance, NSApplication, NSButton, NSModalResponse, NSTextField, NSTextView, NSView};

use super::{check, finish, snapshot};
use crate::git_panel::selftest as panel;
use crate::util::{main_after, main_soon, ns};
use crate::window::{self, Panel, WindowController};

const SCENARIO: &str = "git";
const WAIT: Duration = Duration::from_secs(25);
const POLL: Duration = Duration::from_millis(200);
/// Long enough for every view to draw itself again in the other appearance.
const APPEARANCE_SETTLE: Duration = Duration::from_millis(500);

const README: &str = "README.md";
const LIBRARY: &str = "src/lib.rs";
const NOTES: &str = "notes.txt";

pub fn run() {
    let folder = ide_core::paths::home().join("git-project");
    std::thread::spawn(move || {
        let made = make_repository(&folder);
        main_soon(move || match made {
            Ok(()) => open(folder),
            Err(error) => {
                check("a test repository is made", false, error);
                finish(SCENARIO);
            }
        });
    });
}

/// git for the test repositories, without the user's or the system's configuration.
pub(super) fn fixture_git() -> Result<Git, String> {
    let mut environment = crate::environment::wait(Duration::from_secs(15))
        .ok_or("the login shell's environment did not arrive")?
        .to_vec();
    environment.push(("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()));
    environment.push(("GIT_CONFIG_NOSYSTEM".into(), "1".into()));
    Ok(Git::new(Supervisor::global(), environment))
}

/// A repository with a first commit, then a changed line in the README, two changed regions in the library and an
/// untracked file, and a remote beside it that takes pushes to its checked-out branch.
fn make_repository(folder: &Path) -> Result<(), String> {
    let git = fixture_git()?;
    std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
    let repo = git.init(folder).map_err(|e| e.to_string())?;
    let remote = folder.with_file_name("git-remote");
    std::fs::create_dir_all(&remote).map_err(|e| e.to_string())?;
    let upstream = git.init(&remote).map_err(|e| e.to_string())?;
    let user = "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n";
    append(
        &upstream.git_dir().join("config"),
        &format!("{user}[receive]\n\tdenyCurrentBranch = ignore\n"),
    )?;
    append(
        &repo.git_dir().join("config"),
        &format!(
            "{user}[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n",
            upstream.root().display()
        ),
    )?;
    let numbered = |changed: &[usize]| -> String {
        (1..=30)
            .map(|n| {
                if changed.contains(&n) {
                    format!("    let line_{n} = {n} * 2; // changed\n")
                } else {
                    format!("    let line_{n} = {n};\n")
                }
            })
            .collect()
    };
    write(folder, README, "# Selftest\n\nA repository for the Git panel.\n")?;
    write(folder, LIBRARY, &numbered(&[]))?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", CommitOptions::default())
        .map_err(|e| e.to_string())?;
    write(
        folder,
        README,
        "# Selftest\n\nA repository for the Git panel, changed.\n",
    )?;
    write(folder, LIBRARY, &numbered(&[3, 27]))?;
    write(folder, NOTES, "untracked\n")
}

pub(super) fn write(folder: &Path, path: &str, content: &str) -> Result<(), String> {
    std::fs::write(folder.join(path), content).map_err(|e| e.to_string())
}

pub(super) fn append(file: &Path, text: &str) -> Result<(), String> {
    let mut content = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    content.push_str(text);
    std::fs::write(file, content).map_err(|e| e.to_string())
}

/// Runs `job` on the repository at `root` on a thread of its own, then `then` on the main thread.
fn in_background(root: PathBuf, job: fn(&Path) -> Result<(), String>, then: fn(PathBuf)) {
    std::thread::spawn(move || {
        let done = job(&root);
        main_soon(move || match done {
            Ok(()) => then(root),
            Err(error) => {
                check("a step of the test repository works", false, error);
                finish(SCENARIO);
            }
        });
    });
}

fn open(folder: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    let Some(controller) = window_of(&root) else {
        check(
            "the repository opens in a window",
            false,
            format!("{:?}", window::open_projects()),
        );
        finish(SCENARIO);
        return;
    };
    controller.activate_panel(Panel::Git);
    poll(root, Instant::now() + WAIT, lists_changes, listed);
}

pub(super) fn window_of(root: &Path) -> Option<Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root))
}

/// Calls `then` once `ready` holds for the window of `root`, or with false when time is up.
pub(super) fn poll(root: PathBuf, until: Instant, ready: fn(&WindowController) -> bool, then: fn(PathBuf, bool)) {
    let done = window_of(&root).is_some_and(|c| ready(&c));
    if done || Instant::now() >= until {
        then(root, done);
        return;
    }
    main_after(POLL, move || poll(root, until, ready, then));
}

/// `poll` with the usual time limit.
pub(super) fn wait_for(root: PathBuf, ready: fn(&WindowController) -> bool, then: fn(PathBuf, bool)) {
    poll(root, Instant::now() + WAIT, ready, then);
}

/// Takes the picture `name` of the window of `root` in light, then in dark appearance, then goes on with `then`.
pub(super) fn pictures(root: PathBuf, name: &'static str, then: fn(PathBuf)) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, name);
    }
    let app = NSApplication::sharedApplication(mtm);
    let dark = NSAppearance::appearanceNamed(crate::util::constant!(objc2_app_kit::NSAppearanceNameDarkAqua));
    app.setAppearance(dark.as_deref());
    main_after(APPEARANCE_SETTLE, move || {
        if let Some(window) = window_of(&root).and_then(|c| c.window()) {
            snapshot(&window, &format!("{name}-dark"));
        }
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).setAppearance(None);
        }
        main_after(APPEARANCE_SETTLE, move || then(root));
    });
}

/// `pictures` once the sheet on the window of `root` has slid in; macOS 27 draws a sheet into its window, and a
/// capture of the sheet's own window comes out blank.
pub(super) fn sheet_pictures(root: PathBuf, name: &'static str, then: fn(PathBuf)) {
    main_after(APPEARANCE_SETTLE, move || pictures(root, name, then));
}

/// Ends the sheet on the window of `root` as a click on the button that gives `response` would, after typing `text`
/// into its field; whether there was a sheet.
pub(super) fn answer_sheet(root: &Path, response: NSModalResponse, text: Option<&str>) -> bool {
    let Some(window) = window_of(root).and_then(|c| c.window()) else {
        return false;
    };
    let Some(sheet) = window.attachedSheet() else {
        return false;
    };
    if let (Some(text), Some(content)) = (text, sheet.contentView())
        && let Some(field) = editable_field(&content)
    {
        field.setStringValue(&ns(text));
    }
    window.endSheet_returnCode(&sheet, response);
    true
}

/// The views of the sheet on the window of `root`, depth first.
fn sheet_views(root: &Path) -> Vec<Retained<NSView>> {
    fn collect(view: &NSView, into: &mut Vec<Retained<NSView>>) {
        for subview in view.subviews().iter() {
            collect(&subview, into);
            into.push(subview);
        }
    }
    let mut views = vec![];
    let content = window_of(root)
        .and_then(|c| c.window())
        .and_then(|w| w.attachedSheet())
        .and_then(|sheet| sheet.contentView());
    if let Some(content) = content {
        collect(&content, &mut views);
    }
    views
}

/// The text in the text views of the sheet on the window of `root`, such as git's output or the findings.
fn sheet_text(root: &Path) -> String {
    sheet_views(root)
        .iter()
        .filter_map(|view| view.downcast_ref::<NSTextView>().map(|text| text.string().to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the sheet on the window of `controller` has laid out its buttons, which AppKit does after it begins.
pub(super) fn sheet_ready(controller: &WindowController) -> bool {
    controller
        .project()
        .is_some_and(|root| sheet_default_button(&root).is_some())
}

/// The title of the button Return presses in the sheet on the window of `root`.
fn sheet_default_button(root: &Path) -> Option<String> {
    // An alert begun while the app is not active keeps its default here and clears the button's key equivalent.
    let sheet = window_of(root).and_then(|c| c.window()).and_then(|w| w.attachedSheet());
    if let Some(cell) = sheet.and_then(|sheet| sheet.defaultButtonCell()) {
        return Some(cell.title().to_string());
    }
    sheet_views(root).iter().find_map(|view| {
        let button = view.downcast_ref::<NSButton>()?;
        (button.keyEquivalent().to_string() == "\r").then(|| button.title().to_string())
    })
}

fn editable_field(view: &NSView) -> Option<Retained<NSTextField>> {
    for subview in view.subviews().iter() {
        if let Some(field) = subview.downcast_ref::<NSTextField>()
            && field.isEditable()
        {
            return Some(field.retain());
        }
        if let Some(found) = editable_field(&subview) {
            return Some(found);
        }
    }
    None
}

fn names(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn has(paths: &[PathBuf], name: &str) -> bool {
    paths.iter().any(|p| p == Path::new(name))
}

fn lists_changes(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|seen| [README, LIBRARY, NOTES].iter().all(|name| has(&seen.changes, name)))
}

fn listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    check(
        "the Git panel lists the changed files",
        ok,
        seen.as_ref().map_or_else(
            || "no panel".into(),
            |s| {
                format!(
                    "{}: {} ({} commits, {} rows)",
                    s.phase,
                    names(&s.changes),
                    s.commits,
                    s.rows
                )
            },
        ),
    );
    let tip = controller.symbol_tip(Panel::Git);
    check(
        "the Git symbol carries no badge, its tooltip counts the changed files",
        tip == (Some(splat_i18n::tf("{}, {} Änderungen", &[Panel::Git.title(), "3"])), 0),
        format!("{tip:?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "git-changes");
    }
    if !ok {
        return finish(SCENARIO);
    }
    let tabs = controller.tab_count();
    panel::preview_diff(&controller, Path::new(README), false);
    panel::preview_diff(&controller, Path::new(LIBRARY), false);
    let previews = controller.tab_previews();
    let shown: Vec<String> = previews
        .iter()
        .map(|(title, preview, _)| if *preview { format!("[{title}]") } else { title.clone() })
        .collect();
    check(
        "a single click on a change opens its diff in a tab of its own",
        controller.tab_count() == tabs + 2
            && previews.iter().all(|(_, preview, _)| !*preview)
            && previews.iter().any(|(title, _, _)| title.starts_with("lib.rs")),
        shown.join(", "),
    );
    panel::stage(&controller, Path::new(README));
    wait_for(root, readme_staged, staged);
}

fn readme_staged(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|seen| has(&seen.staged, README) && !has(&seen.changes, README))
}

fn staged(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    check(
        "staging moves a file to Vorgemerkt",
        ok,
        seen.as_ref().map_or_else(String::new, |s| {
            format!("staged: {}; changes: {}", names(&s.staged), names(&s.changes))
        }),
    );
    let tip = controller.symbol_tip(Panel::Git);
    check(
        "a staged file still counts once in the Git symbol's tooltip",
        tip == (Some(splat_i18n::tf("{}, {} Änderungen", &[Panel::Git.title(), "3"])), 0),
        format!("{tip:?}"),
    );
    pictures(root, "git-panel", |root| {
        if let Some(controller) = window_of(&root) {
            panel::show_diff(&controller, Path::new(README), true);
        }
        wait_for(root, readme_diff_read, readme_diff);
    });
}

fn readme_diff_read(controller: &WindowController) -> bool {
    panel::diff_counts(controller, Path::new(README), true).is_some_and(|(rows, _)| rows > 0)
}

fn readme_diff(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let counts = panel::diff_counts(&controller, Path::new(README), true);
    check(
        "the diff tab shows the staged change",
        ok && counts.is_some_and(|(_, hunks)| hunks == 1),
        format!("{counts:?} (rows, hunks)"),
    );
    pictures(root, "git-diff", |root| {
        if let Some(controller) = window_of(&root) {
            panel::show_diff(&controller, Path::new(LIBRARY), false);
        }
        wait_for(root, library_diff_read, library_diff);
    });
}

fn library_diff_read(controller: &WindowController) -> bool {
    panel::diff_counts(controller, Path::new(LIBRARY), false).is_some_and(|(_, hunks)| hunks == 2)
}

fn library_diff(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let counts = panel::diff_counts(&controller, Path::new(LIBRARY), false);
    check(
        "a file with two changed regions shows two hunks",
        ok,
        format!("{counts:?} (rows, hunks)"),
    );
    if !ok {
        return finish(SCENARIO);
    }
    panel::stage_hunk(&controller, Path::new(LIBRARY), 0);
    wait_for(root, one_hunk_staged, hunk_staged);
}

fn one_hunk_staged(controller: &WindowController) -> bool {
    let in_both = panel::seen(controller).is_some_and(|seen| has(&seen.staged, LIBRARY) && has(&seen.changes, LIBRARY));
    in_both && panel::diff_counts(controller, Path::new(LIBRARY), false).is_some_and(|(_, hunks)| hunks == 1)
}

fn hunk_staged(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let counts = panel::diff_counts(&controller, Path::new(LIBRARY), false);
    check(
        "staging one hunk leaves the other unstaged",
        ok,
        format!("unstaged diff {counts:?} (rows, hunks)"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "git-hunk");
    }
    panel::discard(&controller, Path::new(LIBRARY));
    let asked = controller.has_sheet();
    check("discarding asks first", asked, "");
    if !asked || !answer_sheet(&root, objc2_app_kit::NSAlertFirstButtonReturn, None) {
        return finish(SCENARIO);
    }
    wait_for(root, library_discarded, discarded);
}

fn library_discarded(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|seen| has(&seen.staged, LIBRARY) && !has(&seen.changes, LIBRARY))
}

fn discarded(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let text = std::fs::read_to_string(root.join(LIBRARY)).unwrap_or_default();
    check(
        "discarding drops the unstaged change of a tracked file and keeps the staged one",
        ok && text.contains("let line_27 = 27;\n") && text.contains("let line_3 = 3 * 2;"),
        names(&panel::seen(&controller).map(|s| s.changes).unwrap_or_default()),
    );
    panel::commit(&controller, "");
    let problem = panel::seen(&controller).and_then(|s| s.problem);
    check(
        "a commit without a message is refused inline",
        problem.as_deref() == Some(splat_i18n::t("Die Commit-Nachricht fehlt.")),
        format!("{problem:?}"),
    );
    pictures(root, "git-problem", |root| {
        if let Some(controller) = window_of(&root) {
            panel::commit(&controller, "Change from the selftest");
        }
        wait_for(root, committed_twice, committed);
    });
}

fn committed_twice(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|seen| seen.commits == 2 && seen.staged.is_empty() && !seen.busy)
}

fn committed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    check(
        "a commit after the secrets check lands in the history",
        ok,
        seen.as_ref().map_or_else(String::new, |s| {
            format!(
                "{} commits; staged: {}; problem: {:?}",
                s.commits,
                names(&s.staged),
                s.problem
            )
        }),
    );
    let rows = seen.map_or(0, |s| s.rows);
    panel::refresh_ages(&controller);
    let after = panel::seen(&controller).map_or(0, |s| s.rows);
    check(
        "saying the commits' ages again keeps the list",
        rows == after,
        format!("{rows} then {after}"),
    );
    if !ok {
        return finish(SCENARIO);
    }
    panel::click(&controller, panel::Control::Push);
    wait_for(root, idle_with_upstream, pushed);
}

fn idle_with_upstream(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|seen| !seen.busy && (seen.upstream.is_some() || seen.problem.is_some()))
}

fn pushed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    check(
        "a first push goes to origin and sets the upstream",
        ok && seen
            .as_ref()
            .is_some_and(|s| s.upstream.is_some() && s.problem.is_none()),
        seen.as_ref()
            .map_or_else(String::new, |s| format!("{:?}, problem {:?}", s.upstream, s.problem)),
    );
    panel::click(&controller, panel::Control::Fetch);
    wait_for(root, |c| panel::seen(c).is_some_and(|s| !s.busy), fetched);
}

fn fetched(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let problem = panel::seen(&controller).and_then(|s| s.problem);
    check(
        "fetch runs and ends without a problem",
        ok && problem.is_none(),
        format!("{problem:?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "git-pushed");
    }
    flows::start(root);
}

/// Opens a folder without git that the user does not trust yet.
fn open_plain_folder() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let plain = ide_core::paths::home().join("plain-folder");
    let _ = std::fs::create_dir_all(&plain);
    crate::app::open_project(mtm, plain.clone());
    let plain = plain.canonicalize().unwrap_or(plain);
    wait_for(plain, |c| phase_is(c, "Untrusted"), untrusted);
}

fn phase_is(controller: &WindowController, phase: &str) -> bool {
    panel::seen(controller).is_some_and(|seen| seen.phase == phase)
}

fn untrusted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    controller.activate_panel(Panel::Git);
    check("an untrusted folder runs no git and says why", ok, "");
    let tip = controller.symbol_tip(Panel::Git);
    check(
        "an untrusted folder counts no changes on the Git symbol",
        tip == (Some(Panel::Git.title().to_string()), 0),
        format!("{tip:?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "git-untrusted");
    }
    crate::project::trust(&root);
    crate::git_panel::trust_changed(&root);
    wait_for(root, |c| phase_is(c, "NotRepository"), not_a_repository);
}

fn not_a_repository(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a trusted folder without git offers to make a repository", ok, "");
    if let Some(window) = controller.window() {
        snapshot(&window, "git-no-repository");
    }
    finish(SCENARIO);
}
