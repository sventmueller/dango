//! The `snappy-resize`, `snappy-tabs` and `snappy-tests` scenarios: dragging the dividers, switching and moving tabs
//! and a test run of 3 000 tests, each step in a run loop turn of its own as the user's input comes, measured with the
//! stall detector as `snappy-tree` is.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

use super::snappy::{measure, measured};
use super::{check, finish, helpers};
use crate::window::{self, Panel, TabId, WindowController};

/// The pause between two steps, about a frame.
const STEP: Duration = Duration::from_millis(16);
/// How long the app may settle after its setup before a measured part starts.
const SETTLE: Duration = Duration::from_millis(1500);

/// Runs `step` with 0 to `count` - 1, each in a turn of its own and drawn before the next, then `then` with how long
/// each step took with its drawing.
fn steps(count: usize, step: impl Fn(usize) + 'static, then: impl FnOnce(Vec<Duration>) + 'static) {
    next(0, count, Rc::new(step), Vec::new(), Box::new(then));
}

fn next(
    at: usize,
    count: usize,
    step: Rc<dyn Fn(usize)>,
    mut times: Vec<Duration>,
    then: Box<dyn FnOnce(Vec<Duration>)>,
) {
    if at == count {
        return then(times);
    }
    let started = Instant::now();
    step(at);
    if let Some(mtm) = MainThreadMarker::new() {
        for window in NSApplication::sharedApplication(mtm)
            .windows()
            .iter()
            .filter(|w| w.isVisible())
        {
            window.displayIfNeeded();
        }
    }
    times.push(started.elapsed());
    helpers::after(STEP, move || next(at + 1, count, step, times, then));
}

/// What the steps took, for a check's detail: the 95th percentile and the longest.
fn spread(times: &[Duration]) -> String {
    let mut sorted = times.to_vec();
    sorted.sort();
    let at = |percent: usize| {
        let index = (sorted.len().saturating_sub(1) * percent + 50) / 100;
        sorted.get(index).map_or(0, Duration::as_millis)
    };
    format!("{} steps, p95 {} ms, longest {} ms", sorted.len(), at(95), at(100))
}

const RESIZE: &str = "snappy-resize";
const RESIZE_PROJECT: &str = "snappy-resize-project";
const NOTES: &str = "notes.md";
const RESIZE_SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000dd";
/// The steps of each drag.
const DRAG: usize = 40;

/// `snappy-resize`: the sidebar, the inspector and the divider between two documents are dragged in 40 steps each,
/// first beside a Markdown file and its preview, then beside an AI tab and a terminal.
pub fn resize() {
    let notes: String = (1..=200)
        .map(|n| format!("## Part {n}\n\nA paragraph of the notes, long enough to wrap in a narrow column.\n\n"))
        .collect();
    helpers::open_project(RESIZE, RESIZE_PROJECT, &[(NOTES, &notes)], |controller| {
        let root = helpers::root(RESIZE_PROJECT);
        controller.activate_panel(Panel::Files);
        let replayed = crate::selftest::replay_agent(&controller, RESIZE_SESSION);
        check("the AI tab plays its recording", replayed, "");
        let agent = controller.tab_ids().last().copied();
        if let Some(agent) = agent {
            controller.open_beside(agent, || crate::terminal::open_tab(&controller, Some(&root)));
        }
        controller.open_file(&root.join(NOTES));
        let probe = root.clone();
        helpers::wait_until(
            move || window::for_project(&probe).is_some_and(|c| c.tab_count() >= 4),
            move |ready| {
                check(
                    "a Markdown file with its preview, an AI tab and a terminal are open",
                    ready,
                    "",
                );
                helpers::after(SETTLE, move || drag_beside_preview(root, agent));
            },
        );
    });
}

fn drag_beside_preview(root: PathBuf, agent: Option<TabId>) {
    let Some(controller) = window::for_project(&root) else {
        return finish(RESIZE);
    };
    let notes = controller.tab_with_key(&root.join(NOTES).to_string_lossy());
    measure();
    drags(&controller, notes, move |times| {
        measured(
            "dragging the sidebar, the inspector and the divider beside a preview never holds up input",
            &spread(&times),
        );
        let Some(controller) = window::for_project(&root) else {
            return finish(RESIZE);
        };
        if let Some(agent) = agent {
            controller.select_tab_by_id(agent);
        }
        helpers::after(SETTLE, move || {
            let Some(controller) = window::for_project(&root) else {
                return finish(RESIZE);
            };
            measure();
            drags(&controller, agent, |times| {
                measured(
                    "dragging the sidebar, the inspector and the divider beside an AI tab and a terminal never holds \
                     up input",
                    &spread(&times),
                );
                finish(RESIZE);
            });
        });
    });
}

/// Drags the sidebar, the inspector and the divider of the tab of document `id` of `controller`'s window, `DRAG`
/// steps each, then folds the inspector away again.
fn drags(controller: &WindowController, id: Option<TabId>, then: impl FnOnce(Vec<Duration>) + 'static) {
    let window = controller.id();
    let with = move |f: &dyn Fn(&WindowController)| {
        if let Some(controller) = crate::host::window(window) {
            f(&controller);
        }
    };
    let swing = |at: usize| (at % 20) as f64 / 19.0;
    steps(
        DRAG * 3,
        move |at| {
            with(&|c| match at / DRAG {
                0 => c.set_sidebar_width(240.0 + 160.0 * swing(at)),
                1 => c.set_inspector_width(220.0 + 200.0 * swing(at)),
                _ => {
                    if let Some(id) = id {
                        c.drag_share_of(id, 0.3 + 0.4 * swing(at));
                    }
                }
            });
        },
        move |times| {
            if let Some(controller) = crate::host::window(window) {
                controller.show_inspector(false);
            }
            then(times);
        },
    );
}

const TABS: &str = "snappy-tabs";
const TABS_PROJECT: &str = "snappy-tabs-project";
/// The tabs switched and moved.
const TAB_COUNT: usize = 20;
const LARGE: &str = "large.txt";
/// The line of the large file the caret goes to.
const LARGE_LINE: usize = 30_000;

/// `snappy-tabs`: 20 tabs are switched through and moved around one by one; then a file of more than 1 MB opens
/// with the caret on one of its last lines.
pub fn tabs() {
    let files: Vec<(String, String)> = (0..TAB_COUNT)
        .map(|n| (format!("tab{n}.rs"), format!("// tab {n}\nfn main() {{}}\n")))
        .collect();
    let named: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    helpers::open_project(TABS, TABS_PROJECT, &named, |controller| {
        let root = helpers::root(TABS_PROJECT);
        for n in 0..TAB_COUNT {
            controller.open_file(&root.join(format!("tab{n}.rs")));
        }
        let probe = root.clone();
        helpers::wait_until(
            move || window::for_project(&probe).is_some_and(|c| c.tab_count() >= TAB_COUNT),
            move |ready| {
                check("20 files open in tabs of their own", ready, "");
                helpers::after(SETTLE, move || switch_tabs(root));
            },
        );
    });
}

fn switch_tabs(root: PathBuf) {
    let Some(controller) = window::for_project(&root) else {
        return finish(TABS);
    };
    let window = controller.id();
    let with = move |f: &dyn Fn(&WindowController)| {
        if let Some(controller) = crate::host::window(window) {
            f(&controller);
        }
    };
    // Each tab comes to the front once first, which lays out and draws its window for the first time as opening it does.
    steps(
        TAB_COUNT,
        move |_| with(&|c| c.select_next_tab()),
        move |_| {
            let before = crate::host::window(window)
                .map(|c| c.window_tab_ids())
                .unwrap_or_default();
            measure();
            steps(
                TAB_COUNT * 2,
                move |at| {
                    with(&|c| {
                        if at % 2 == 0 {
                            c.select_next_tab();
                        } else {
                            c.move_front_tab_to((at * 7) % TAB_COUNT);
                        }
                    });
                },
                move |times| {
                    measured(
                        "switching through 20 tabs and moving them around never holds up input",
                        &spread(&times),
                    );
                    let after = crate::host::window(window)
                        .map(|c| c.window_tab_ids())
                        .unwrap_or_default();
                    let (mut old, mut new) = (before.concat(), after.concat());
                    let moved = old != new;
                    old.sort_unstable_by_key(|id| id.get());
                    new.sort_unstable_by_key(|id| id.get());
                    check(
                        "moving tabs changes their order and keeps every one",
                        moved && old == new,
                        format!("{before:?} → {after:?}"),
                    );
                    open_large(root);
                },
            );
        },
    );
}

fn open_large(root: PathBuf) {
    let path = root.join(LARGE);
    let text: String = (1..=40_000)
        .map(|n| format!("line {n:05} of a file larger than the main thread reads itself\n"))
        .collect();
    if let Err(error) = std::fs::write(&path, text) {
        check("the large file is written", false, error.to_string());
        return finish(TABS);
    }
    let Some(controller) = window::for_project(&root) else {
        return finish(TABS);
    };
    crate::editor::reveal_position(&controller, &path, LARGE_LINE, 1);
    let probe = (root.clone(), path.clone());
    helpers::wait_until(
        move || caret_line(&probe.0, &probe.1) == Some(LARGE_LINE),
        move |placed| {
            check(
                "a file of more than 1 MB opens in its tab with the caret on the line asked for",
                placed,
                format!("{:?}", caret_line(&root, &path)),
            );
            finish(TABS);
        },
    );
}

/// The line, from 1, of the caret in the editor of `path` in the window of `root`.
fn caret_line(root: &Path, path: &Path) -> Option<usize> {
    let controller = window::for_project(root)?;
    let view = crate::editor::editor_for(&controller, path)?;
    let at = view.selectedRange().location;
    let document = view.document();
    let text = document.text();
    let byte = text.utf16_to_byte(at.min(text.len_utf16())).ok()?;
    Some(text.line_of(byte) + 1)
}

const TESTS: &str = "snappy-tests";
const TESTS_PROJECT: &str = "snappy-tests-project";
/// The subtests of the run.
const SUBTESTS: usize = 3_000;
/// `go test` compiles first, which takes a while on a busy Mac.
const RUN: Duration = Duration::from_secs(180);
const MANY: &str = "package many\n\nimport (\n\t\"fmt\"\n\t\"testing\"\n\t\"time\"\n)\n\nfunc TestMany(t *testing.T) {\n\tfor i := 0; i < 3000; i++ {\n\t\tt.Run(fmt.Sprintf(\"case_%04d\", i), func(t *testing.T) {\n\t\t\ttime.Sleep(time.Millisecond)\n\t\t})\n\t}\n}\n";

/// `snappy-tests`: the Tests panel follows a run of 3 000 tests while it polls the tool's output.
pub fn tests() {
    helpers::open_project(
        TESTS,
        TESTS_PROJECT,
        &[
            ("go.mod", "module example.com/many\n\ngo 1.22\n"),
            ("many_test.go", MANY),
        ],
        |controller| {
            controller.activate_panel(Panel::Tests);
            let root = helpers::root(TESTS_PROJECT);
            let probe = root.clone();
            helpers::wait_within(
                RUN,
                move || rows(&probe).iter().any(|(_, label, _)| label == "TestMany"),
                move |listed| {
                    check("the Tests panel lists the Go package's test", listed, "");
                    helpers::after(SETTLE, move || run_many(root));
                },
            );
        },
    );
}

fn rows(root: &Path) -> Vec<(usize, String, ide_test::State)> {
    window::for_project(root)
        .and_then(|c| crate::tests_panel::panel_of(&c))
        .map(|p| p.shown())
        .unwrap_or_default()
}

fn run_many(root: PathBuf) {
    let Some(controller) = window::for_project(&root) else {
        return finish(TESTS);
    };
    measure();
    crate::tests_panel::run_all(&controller);
    let probe = root.clone();
    let passed = move || {
        let rows = rows(&probe);
        rows.len() > SUBTESTS && rows.iter().all(|(_, _, state)| *state == ide_test::State::Passed)
    };
    helpers::wait_within(RUN, passed, move |passed| {
        check(
            "the Tests panel shows the 3 000 tests of the run as passed",
            passed,
            format!("{} rows", rows(&root).len()),
        );
        measured(
            "following a run of 3 000 tests never holds up input",
            "the run of 3 000 tests",
        );
        finish(TESTS);
    });
}
