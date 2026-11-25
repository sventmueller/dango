//! A folder for the scenarios that run orders: a git repository with `files` in its first commit, opened trusted, its
//! workflow file read and its gate commands confirmed as the Leitstand's dialog does (E190).

use std::path::PathBuf;
use std::time::Duration;

use super::super::helpers::{self, wait_within};
use super::super::{give_up, give_up_saying};
use crate::leitstand::Command;
use crate::util::main_soon;

/// How long the folder's factory may take to read the workflow file, under the load of parallel runs.
const READ: Duration = Duration::from_secs(120);
const CONFIRM: Duration = Duration::from_secs(20);

/// Makes the folder `project` with `files`, commits them, opens it trusted, confirms its gate commands and calls
/// `then` with its root; a step that fails ends `scenario`.
pub(super) fn ready(
    scenario: &'static str,
    project: &str,
    files: &[(&str, &str)],
    then: impl FnOnce(PathBuf) + Send + 'static,
) {
    let root = match helpers::make_project(project, files) {
        Ok(root) => root,
        Err(error) => return give_up_saying(scenario, "the scenario's folder is made", error.to_string()),
    };
    std::thread::spawn(move || {
        let made = super::order::commit(&root);
        main_soon(move || match made {
            Ok(()) => opened(scenario, helpers::open_trusted(&root), then),
            Err(error) => give_up_saying(scenario, "the test repository has its first commit", error),
        });
    });
}

fn opened(scenario: &'static str, root: PathBuf, then: impl FnOnce(PathBuf) + 'static) {
    let probe = root.clone();
    wait_within(
        READ,
        move || crate::context::app().leitstand().has_gates(&probe),
        move |read| {
            let cx = crate::context::app();
            let Some((hash, _)) = cx.leitstand().gates(&root).filter(|_| read) else {
                return give_up(scenario, "the folder's factory reads the workflow file");
            };
            cx.leitstand().command(&cx, &root, Command::ConfirmGates(hash));
            let probe = root.clone();
            wait_within(
                CONFIRM,
                move || {
                    crate::context::app()
                        .leitstand()
                        .gates(&probe)
                        .is_some_and(|(_, confirmed)| confirmed)
                },
                move |confirmed| {
                    if !confirmed {
                        return give_up(scenario, "the gate command is confirmed");
                    }
                    then(root);
                },
            );
        },
    );
}
