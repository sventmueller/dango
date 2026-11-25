//! The `debug-restore` scenario (Review C18), run twice in one home: the first launch sets breakpoints, one with a
//! condition, a watch expression, shows the debug panel, runs a session on the scripted adapter to its end, and quits
//! as the user does; the second checks that breakpoints, watch, panel and the console come back, the console as a
//! finished history that takes no input, and that no session started by itself.

use std::path::PathBuf;

use objc2::MainThreadMarker;

use super::debug::{project, script};
use super::helpers::{wait_until, window_of};
use super::{check, finish, give_up};
use crate::debug::probe::{self as debug, Command};
use crate::window::Panel;

const SCENARIO: &str = "debug-restore";
const CONDITION: &str = "hits > 1";

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = project("debug-restore");
    if std::env::var("IDE_SELFTEST_LAUNCH").as_deref() == Ok("quit") {
        crate::app::open_project(&crate::context::app(), mtm, root.clone());
        let probe = root.clone();
        wait_until(
            move || window_of(&probe).and_then(|c| debug::seen(&c)).is_some(),
            move |_| arrange(root),
        );
    } else {
        let probe = root.clone();
        wait_until(
            move || {
                window_of(&probe)
                    .and_then(|c| debug::seen(&c))
                    .is_some_and(|s| !s.consoles.is_empty() && s.breakpoints.len() == 2 && !s.watches.is_empty())
                    && window_of(&probe).is_some_and(|c| c.inspect().panel == Some(Panel::Debug))
            },
            move |_| came_back(root),
        );
    }
}

fn arrange(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder opens");
    };
    let file = root.join("src/main.rs");
    debug::toggle_breakpoint(&controller, &file, 3);
    debug::toggle_breakpoint(&controller, &file, 6);
    debug::set_condition(&controller, &file, 6, CONDITION);
    debug::add_watch(&controller, "hits");
    controller.activate_panel(Panel::Debug);
    debug::start_fake(&controller, script(&root));
    let probe = root.clone();
    wait_until(
        move || {
            window_of(&probe)
                .and_then(|c| debug::seen(&c))
                .is_some_and(|s| s.paused.is_some())
        },
        move |ok| {
            check("the session stops before the quit", ok, "");
            let Some(controller) = window_of(&root) else {
                return give_up(SCENARIO, "the folder's window is open");
            };
            debug::evaluate(&controller, "hits");
            debug::run(&controller, Command::Stop);
            let probe = root.clone();
            wait_until(
                move || {
                    window_of(&probe)
                        .and_then(|c| debug::seen(&c))
                        .is_some_and(|s| s.consoles.iter().all(|(_, _, input)| !input))
                },
                move |ok| {
                    check("the session ends before the quit", ok, "");
                    finish(SCENARIO);
                },
            );
        },
    );
}

fn came_back(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window comes back");
    };
    let seen = debug::seen(&controller).unwrap_or_default();
    let file = root.join("src/main.rs");
    check(
        "the breakpoints come back, the second with its condition",
        seen.breakpoints == [(file.clone(), 3, true), (file.clone(), 6, true)]
            && debug::condition(&controller, &file, 6).as_deref() == Some(CONDITION),
        format!("{:?}", seen.breakpoints),
    );
    check(
        "the watch expression comes back",
        seen.watches.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>() == ["hits"],
        format!("{:?}", seen.watches),
    );
    check(
        "the debug panel comes back as the panel shown",
        controller.inspect().panel == Some(Panel::Debug),
        format!("{:?}", controller.inspect().panel),
    );
    check(
        "the console comes back as the finished history, without input",
        seen.consoles.len() == 1
            && seen.consoles.iter().all(|(_, text, input)| {
                !input && text.contains("hello from the scripted program") && text.contains("› hits")
            }),
        format!("{:?}", seen.consoles),
    );
    check(
        "no session starts or attaches by itself",
        seen.sessions.is_empty() && seen.controls.iter().all(|on| !on),
        format!("{:?}, {:?}", seen.sessions, seen.controls),
    );
    if let Some(window) = controller.window() {
        super::snapshot(&window, "debug-restored");
    }
    finish(SCENARIO);
}
