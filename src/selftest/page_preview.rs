//! The `page-preview` scenario (E131): a page of a trusted folder opens in a browser tab from the app's own server,
//! loads an ES module and a stylesheet by an address from the root, and reloads once a file it uses is saved. What the
//! server refuses (no cookie, hidden files, symlinks out of the folder) the tests of `ide_project::site` check.

use std::time::Duration;

use objc2::rc::Retained;

use super::helpers::{self, wait_within};
use super::{check, finish, give_up_saying, snapshot};
use crate::window::WindowController;

const SCENARIO: &str = "page-preview";
const PROJECT: &str = "page-project";
const PAGE: &str = "<!doctype html><title>start</title><link rel=stylesheet href=/css/app.css>\
                    <h1>Preview</h1><script type=module src=/js/main.js></script>";
const STYLE: &str = "h1 { color: rgb(1, 2, 3); }";
const MODULE: &str = "import { word } from './word.js';\n\
                      document.title = word + ' ' + getComputedStyle(document.querySelector('h1')).color;\n";

fn titles(controller: &WindowController) -> Vec<String> {
    controller.tab_titles()
}

pub(crate) fn run() {
    helpers::open_project(
        SCENARIO,
        PROJECT,
        &[
            ("site/index.html", PAGE),
            ("css/app.css", STYLE),
            ("js/main.js", MODULE),
            ("js/word.js", "export const word = 'first';\n"),
            (".env", "SECRET=1\n"),
        ],
        opened,
    );
}

fn opened(controller: Retained<WindowController>) {
    let root = helpers::root(PROJECT);
    crate::site_preview::open(&controller, &root.join("site/index.html"));
    let wanted = "first rgb(1, 2, 3)";
    let seen = controller.clone();
    wait_within(
        Duration::from_secs(20),
        move || titles(&seen).iter().any(|t| t == wanted),
        move |loaded| {
            check(
                "the page loads its ES module and its stylesheet by addresses from the folder's root",
                loaded,
                titles(&controller).join(", "),
            );
            if let Some(window) = controller.window() {
                snapshot(&window, "page-preview");
            }
            reloads(controller);
        },
    );
}

fn reloads(controller: Retained<WindowController>) {
    let root = helpers::root(PROJECT);
    if let Err(error) = std::fs::write(root.join("js/word.js"), "export const word = 'second';\n") {
        return give_up_saying(SCENARIO, "the module is saved again", error.to_string());
    }
    let wanted = "second rgb(1, 2, 3)";
    let seen = controller.clone();
    wait_within(
        Duration::from_secs(20),
        move || titles(&seen).iter().any(|t| t == wanted),
        move |reloaded| {
            let running = objc2::MainThreadMarker::new()
                .map(crate::site_preview::running)
                .unwrap_or_default();
            check(
                "saving a file the page uses reloads the tab, from one server of the folder",
                reloaded && running.len() == 1,
                format!("{} / {running:?}", titles(&controller).join(", ")),
            );
            finish(SCENARIO);
        },
    );
}
