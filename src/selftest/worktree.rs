//! The `worktree` scenario (I1): "+" ▸ AI-Thread ▸ "In eigenem Worktree" lists Claude, the own agent and the ACP
//! agents and makes a worktree in the app's data folder with a replayed thread working there; what the thread writes
//! stays out of the user's copy and shows in the Git panel, which reads an unchanged worktree from its cache and a
//! saved one anew, its diff against the base opens read only, its files open in tabs marked as the worktree's, taking
//! it over merges it and closes its diff tabs, a clash stops at the conflict section and is aborted, an open thread
//! keeps its worktree, a closed one opens again, removing asks first, and a second thread, run by the ACP test agent,
//! keeps its branch with its work when the worktree goes. No agent installed on the Mac starts: ACP launches are the
//! test agent.

use ide_core::LockIgnoringPoison;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ide_git::CommitOptions;
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertFirstButtonReturn;

use super::git::{
    answer_sheet, append, fixture_git, pictures, sheet_pictures, sheet_ready, wait_for, window_of, write,
};
use super::{check, finish};
use crate::git_panel::selftest::{self as panel, Control, TreeAction};
use crate::util::main_soon;
use crate::window::{Panel, WindowController};

const SCENARIO: &str = "worktree";
const TITLE: &str = "Login reparieren";
const BRANCH: &str = "thread/login-reparieren";
const LIBRARY: &str = "src/lib.rs";
const NOTES: &str = "notes.md";
const README: &str = "README.md";
const SECOND: &str = "Zweiter Thread";
const SECOND_BRANCH: &str = "thread/zweiter-thread";
const EXTRA: &str = "extra.txt";
/// The place of Codex among the worktree menu's agents: Claude, the ACP agents, the own agent.
const CODEX: usize = 1;

/// The worktree the scenario made.
static TREE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The panel's reading counts before a step.
static COUNTS: Mutex<(usize, usize)> = Mutex::new((0, 0));

fn counts_before() -> (usize, usize) {
    *COUNTS.lock_ignoring_poison()
}

fn note_counts() {
    *COUNTS.lock_ignoring_poison() = crate::worktrees::read_counts();
}

fn tree() -> PathBuf {
    TREE.lock_ignoring_poison().clone().unwrap_or_default()
}

pub fn run() {
    let folder = crate::selftest::helpers::folder("worktree-project");
    let launch = std::env::current_exe().ok().map(|program| {
        ide_acp::Launch::new(
            "test",
            "Test",
            program,
            Vec::new(),
            vec![
                ("IDE_ACP_TEST_AGENT".into(), "basic".into()),
                ("PATH".into(), "/usr/bin:/bin".into()),
            ],
            &folder,
        )
    });
    crate::agent::set_test_launch(launch);
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

fn make_repository(folder: &Path) -> Result<(), String> {
    let git = fixture_git()?;
    std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
    let repo = git.init(folder).map_err(|e| e.to_string())?;
    append(
        &repo.git_dir().join("config"),
        "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n",
    )?;
    write(folder, README, "# Worktree\n\nOne line.\n")?;
    write(folder, LIBRARY, "pub fn login() {}\n")?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", CommitOptions::default())
        .map(drop)
        .map_err(|e| e.to_string())
}

fn open(folder: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    if let Some(controller) = window_of(&root) {
        controller.activate_panel(Panel::Git);
    }
    wait_for(root, crate::worktrees::can_start, ready);
}

fn ready(root: PathBuf, ok: bool) {
    check(
        "a trusted repository with a commit offers a thread in its own worktree",
        ok,
        "",
    );
    let Some(controller) = window_of(&root).filter(|_| ok) else {
        return finish(SCENARIO);
    };
    let offered = controller.worktree_menu_titles();
    let installed = |agent: crate::agent::Agent| crate::agent::installed(&agent);
    let expected: Vec<String> = installed(crate::agent::Agent::Claude)
        .then(|| "Claude".to_string())
        .into_iter()
        .chain(crate::agent::installed_presets().into_iter().map(|(_, name)| name))
        .chain(installed(crate::agent::Agent::Own).then(|| splat_i18n::t("Eigener Agent").to_string()))
        .map(|name| format!("{name} …"))
        .collect();
    check(
        "the worktree menu offers the installed agents: Claude, the ACP agents, the own agent last",
        offered == expected && expected.len() > 3,
        offered.join(", "),
    );
    crate::worktrees::ask_and_open(&controller, 0);
    wait_for(root, sheet_ready, |root, ok| {
        check("the thread's title is asked for first", ok, "");
        sheet_pictures(root, "worktree-new", |root| {
            answer_sheet(&root, NSAlertFirstButtonReturn, Some(TITLE));
            wait_for(root, created, made);
        });
    });
}

fn created(controller: &WindowController) -> bool {
    panel::worktrees(controller)
        .first()
        .is_some_and(|tree| crate::agent::working_in(&tree.0))
}

fn made(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let trees = panel::worktrees(&controller);
    let Some((path, branch, ..)) = trees.first().cloned().filter(|_| ok) else {
        check("the worktree and its thread start", false, format!("{trees:?}"));
        return finish(SCENARIO);
    };
    *TREE.lock_ignoring_poison() = Some(path.clone());
    let data = crate::worktrees::folder(&root);
    let data = data.canonicalize().unwrap_or(data);
    check(
        "the worktree lies in the app's data folder for the project, never inside the repository, on its own branch",
        path.starts_with(&data) && !path.starts_with(&root) && branch == BRANCH,
        format!("{} on {branch}", path.display()),
    );
    let tab = crate::agent::all().into_iter().find(|tab| tab.cwd() == path);
    check(
        "the thread's session works in the worktree and carries its title",
        tab.is_some_and(|tab| tab.title() == TITLE),
        "",
    );
    let written = write(&path, LIBRARY, "pub fn login() {\n    check_password();\n}\n")
        .and_then(|()| write(&path, NOTES, "What the thread found.\n"));
    if let Err(error) = written {
        check("the thread writes into its worktree", false, error);
        return finish(SCENARIO);
    }
    crate::worktrees::turn_ended(&controller, &path);
    wait_for(
        root,
        |c| panel::worktrees(c).first().is_some_and(|t| t.4.len() == 2),
        listed,
    );
}

fn listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let files = panel::worktrees(&controller)
        .first()
        .map(|t| t.4.clone())
        .unwrap_or_default();
    let clean = panel::seen(&controller).is_some_and(|s| s.changes.is_empty() && s.staged.is_empty());
    let untouched = std::fs::read_to_string(root.join(LIBRARY)).is_ok_and(|text| text == "pub fn login() {}\n");
    check(
        "after the thread's turn the panel lists what its worktree changed while the user's copy stays clean",
        ok && files == [PathBuf::from(NOTES), PathBuf::from(LIBRARY)] && clean && untouched,
        format!("{files:?}"),
    );
    note_counts();
    crate::git_panel::refresh(&controller);
    wait_for(root, |_| crate::worktrees::read_counts().0 > counts_before().0, cached);
}

fn cached(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let (before, now) = (counts_before(), crate::worktrees::read_counts());
    check(
        "a refresh with nothing new in the worktree takes its files from the cache",
        ok && now.1 == before.1,
        format!("{before:?} → {now:?}"),
    );
    panel::show_tree_diff(&controller, &tree(), Path::new(LIBRARY));
    wait_for(
        root,
        |c| panel::tree_diff_counts(c, &tree(), Path::new(LIBRARY)).is_some_and(|(_, hunks)| hunks > 0),
        diffed,
    );
}

fn diffed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let titles = controller.tab_titles();
    check(
        "the worktree's file opens as a diff against the thread's base",
        ok && titles.iter().any(|t| t == "lib.rs (Thread)"),
        format!("{titles:?}"),
    );
    let rows = panel::tree_rows(&controller, &tree());
    check(
        "the worktree's row opens to the files it changed",
        rows == [PathBuf::from(NOTES), PathBuf::from(LIBRARY)],
        format!("{rows:?}"),
    );
    panel::open_tree_file(&controller, &tree(), Path::new(LIBRARY));
    wait_for(
        root,
        |c| c.tab_titles().iter().any(|t| t == "lib.rs · Worktree login-reparieren"),
        |root, ok| {
            check("a file opened from the worktree says so in its tab", ok, "");
            save_in_editor(root);
        },
    );
}

/// Types a line into the worktree's file in its editor tab and saves it.
fn save_in_editor(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let Some(view) = super::helpers::editor(&controller, "lib.rs · Worktree login-reparieren") else {
        check(
            "the worktree's file has an editor",
            false,
            format!("{:?}", controller.tab_titles()),
        );
        return finish(SCENARIO);
    };
    note_counts();
    let end = super::helpers::utf16_len(&super::helpers::text_of(&view));
    super::helpers::type_at(&view, objc2_foundation::NSRange::new(end, 0), "// saved\n");
    crate::editor::save_view(&view);
    wait_for(
        root,
        |_| crate::worktrees::read_counts().1 > counts_before().1,
        |root, ok| {
            let saved = std::fs::read_to_string(tree().join(LIBRARY)).unwrap_or_default();
            check(
                "saving the worktree's file in the editor reads that worktree again",
                ok && saved.contains("// saved"),
                saved,
            );
            pictures(root, "worktree-panel", take_over);
        },
    );
}

fn take_over(root: PathBuf) {
    if let Some(controller) = window_of(&root) {
        panel::tree_action(&controller, &tree(), TreeAction::TakeOver);
    }
    wait_for(root, sheet_ready, |root, ok| {
        check("taking the work over asks how", ok, "");
        answer_sheet(&root, NSAlertFirstButtonReturn, None);
        wait_for(root, merged, taken);
    });
}

fn merged(controller: &WindowController) -> bool {
    let top = panel::seen(controller).and_then(|s| s.top).map(|t| t.0);
    let tree = panel::worktrees(controller).first().cloned();
    top.as_deref() == Some(&format!("Merge branch '{BRANCH}'")) && tree.is_some_and(|t| t.2 == 0 && t.4.is_empty())
}

fn taken(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let library = std::fs::read_to_string(root.join(LIBRARY)).unwrap_or_default();
    check(
        "taking over commits the thread's work on its branch and merges it into the user's branch",
        ok && library.contains("check_password") && root.join(NOTES).exists(),
        format!("{:?}", panel::worktrees(&controller)),
    );
    let titles = controller.tab_titles();
    check(
        "taking over closes the thread's diff tabs, whose changes are the user's branch's now",
        !titles.iter().any(|t| t == "lib.rs (Thread)") && titles.iter().any(|t| t.starts_with("lib.rs · Worktree")),
        format!("{titles:?}"),
    );
    std::thread::spawn(move || {
        let clashed = clash(&root);
        main_soon(move || match clashed {
            Ok(()) => clash_written(root),
            Err(error) => {
                check("the user and the thread change the same line", false, error);
                finish(SCENARIO);
            }
        });
    });
}

/// The thread and the user change the same line; the user commits theirs.
fn clash(root: &Path) -> Result<(), String> {
    write(&tree(), README, "# Worktree\n\nThe thread's line.\n")?;
    write(root, README, "# Worktree\n\nThe user's line.\n")?;
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("Mine", CommitOptions::default())
        .map(drop)
        .map_err(|e| e.to_string())
}

fn clash_written(root: PathBuf) {
    if let Some(controller) = window_of(&root) {
        crate::worktrees::turn_ended(&controller, &tree());
        panel::tree_action(&controller, &tree(), TreeAction::TakeOver);
    }
    wait_for(root, sheet_ready, |root, _| {
        answer_sheet(&root, NSAlertFirstButtonReturn, None);
        wait_for(root, conflicting, conflicted);
    });
}

fn conflicting(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|s| s.conflicts == [PathBuf::from(README)] && s.operation.is_some())
}

fn conflicted(root: PathBuf, ok: bool) {
    check("a clash stops the merge with the file in the conflict section", ok, "");
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    panel::click(&controller, Control::Abort);
    wait_for(root, sheet_ready, |root, _| {
        answer_sheet(&root, NSAlertFirstButtonReturn, None);
        wait_for(
            root,
            |c| panel::seen(c).is_some_and(|s| s.operation.is_none() && s.conflicts.is_empty()),
            aborted,
        );
    });
}

fn aborted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let readme = std::fs::read_to_string(root.join(README)).unwrap_or_default();
    check(
        "aborting the merge brings the user's branch back",
        ok && readme.contains("The user's line."),
        "",
    );
    panel::tree_action(&controller, &tree(), TreeAction::Remove);
    let problem = panel::seen(&controller).and_then(|s| s.problem).unwrap_or_default();
    check(
        "a worktree its thread still works in stays",
        problem.contains(TITLE) && !controller.has_sheet() && tree().exists(),
        &problem,
    );
    close_thread(&controller, &tree());
    wait_for(root, |_| !crate::agent::working_in(&tree()), thread_closed);
}

/// Closes the AI tab that works in `path`, as its close button does.
fn close_thread(controller: &WindowController, path: &Path) {
    let tab = crate::agent::all().into_iter().find(|tab| tab.cwd() == path);
    if let Some(id) = tab.and_then(|tab| controller.tab_with_key(&tab.tab_key())) {
        controller.close_tab(id);
    }
}

fn thread_closed(root: PathBuf, ok: bool) {
    check("closing the thread's tab ends its session", ok, "");
    if let Some(controller) = window_of(&root) {
        panel::tree_action(&controller, &tree(), TreeAction::OpenThread);
    }
    wait_for(root, |_| crate::agent::working_in(&tree()), reopened);
}

fn reopened(root: PathBuf, ok: bool) {
    check("„Thread öffnen“ starts a thread in the worktree again", ok, "");
    if let Some(controller) = window_of(&root) {
        close_thread(&controller, &tree());
    }
    wait_for(
        root,
        |_| !crate::agent::working_in(&tree()),
        |root, _| {
            if let Some(controller) = window_of(&root) {
                panel::tree_action(&controller, &tree(), TreeAction::Remove);
            }
            wait_for(root, WindowController::has_sheet, |root, ok| {
                let cleared = window_of(&root)
                    .and_then(|c| panel::seen(&c))
                    .is_some_and(|s| s.problem.is_none());
                check(
                    "removing a closed thread's worktree asks first, and the refusal while it was open is gone",
                    ok && cleared,
                    "",
                );
                sheet_pictures(root, "worktree-remove", |root| {
                    answer_sheet(&root, NSAlertFirstButtonReturn, None);
                    wait_for(root, |c| panel::worktrees(c).is_empty(), removed);
                });
            });
        },
    );
}

/// Whether `branch` exists in the repository at `root`.
fn branch_exists(root: &Path, branch: &str) -> bool {
    fixture_git()
        .and_then(|git| git.discover(root).map_err(|e| e.to_string()))
        .and_then(|repo| repo.resolve(&format!("refs/heads/{branch}")).map_err(|e| e.to_string()))
        .is_ok_and(|found| found.is_some())
}

/// How many commits `branch` has beyond the current branch of the repository at `root`, and how many it lacks.
fn ahead_behind(root: &Path, branch: &str) -> Result<(usize, usize), String> {
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    repo.ahead_behind("HEAD", branch).map_err(|e| e.to_string())
}

fn removed(root: PathBuf, ok: bool) {
    std::thread::spawn(move || {
        let kept = branch_exists(&root, BRANCH);
        main_soon(move || {
            let path = tree();
            check(
                "removing takes the folder and the app's note away and keeps a branch with work not merged",
                ok && !path.exists() && !path.with_extension("json").exists() && kept,
                path.display().to_string(),
            );
            second(root);
        });
    });
}

/// A second thread, whose branch stays when its worktree goes.
fn second(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    crate::worktrees::ask_and_open(&controller, CODEX);
    wait_for(root, sheet_ready, |root, _| {
        answer_sheet(&root, NSAlertFirstButtonReturn, Some(SECOND));
        wait_for(root, created, second_made);
    });
}

fn second_made(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let Some(path) = panel::worktrees(&controller)
        .first()
        .map(|t| t.0.clone())
        .filter(|_| ok)
    else {
        check("a second thread starts in a worktree of its own", false, "");
        return finish(SCENARIO);
    };
    *TREE.lock_ignoring_poison() = Some(path.clone());
    wait_for(
        root,
        |_| {
            let tree = tree();
            crate::agent::all()
                .into_iter()
                .any(|tab| tab.cwd() == tree && tab.is_live())
        },
        second_live,
    );
}

fn second_live(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let path = tree();
    let tab = crate::agent::all().into_iter().find(|tab| tab.cwd() == path);
    let noted = crate::worktrees::list(&root)
        .into_iter()
        .find(|t| t.path == path)
        .map(|t| crate::agent::Agent::of_thread(&t.agent).name().to_string());
    let name = tab.map(|tab| tab.agent_name()).unwrap_or_default();
    check(
        "a worktree thread runs the ACP agent picked for it, whose session opens in the worktree",
        ok && name == "Codex" && noted.as_deref() == Some("Codex"),
        format!("{name}, noted {noted:?}"),
    );
    if let Err(error) = write(&path, EXTRA, "left uncommitted\n") {
        check("the second thread writes into its worktree", false, error);
        return finish(SCENARIO);
    }
    close_thread(&controller, &path);
    wait_for(
        root,
        |_| !crate::agent::working_in(&tree()),
        |root, _| {
            if let Some(controller) = window_of(&root) {
                panel::tree_action(&controller, &tree(), TreeAction::KeepBranch);
            }
            wait_for(root, |c| panel::worktrees(c).is_empty(), kept);
        },
    );
}

fn kept(root: PathBuf, ok: bool) {
    std::thread::spawn(move || {
        let counted = ahead_behind(&root, SECOND_BRANCH);
        main_soon(move || {
            check(
                "„Als Branch behalten“ commits what the thread left on its branch and takes the worktree away",
                ok && counted == Ok((1, 0)) && !tree().exists(),
                format!("ahead and behind {counted:?}, {}", tree().display()),
            );
            finish(SCENARIO);
        });
    });
}
