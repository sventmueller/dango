//! The `editor-blame` scenario (E86): blame at the end of the cursor's line after a rest and in the column before the
//! line numbers, with lines that are not committed, saved or not; the popover with the whole message, copying the
//! hash, the commit in the Git panel's graph and its diff; and nothing of it outside a trusted folder.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSPoint, NSRange};
use splat_i18n::t;

use super::helpers::wait_until;
use super::{check, finish, snapshot};
use crate::editor::blame_probe as blame;
use crate::editor::insets::{self, Layer};
use crate::editor::{self, EditorTextView};
use crate::util::main_after;
use crate::window;

const SCENARIO: &str = "editor-blame";
const FILE: &str = "notes.txt";

fn root() -> PathBuf {
    let folder = ide_core::paths::home().join("blame-project");
    let _ = std::fs::create_dir_all(&folder);
    folder.canonicalize().unwrap_or(folder)
}

fn blamed() -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(FILE))
}

/// Runs git in `dir` as `author`, `days` ago, without the user's configuration.
fn git(dir: &Path, author: &str, days: u64, args: &[&str]) -> Result<String, String> {
    let when = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .saturating_sub(days * 86_400);
    let output = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", author)
        .env("GIT_AUTHOR_EMAIL", format!("{}@example.invalid", author.to_lowercase()))
        .env("GIT_COMMITTER_NAME", author)
        .env("GIT_COMMITTER_EMAIL", "selftest@example.invalid")
        .env("GIT_AUTHOR_DATE", format!("@{when} +0000"))
        .env("GIT_COMMITTER_DATE", format!("@{when} +0000"))
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Two commits: Ada writes three lines ten days ago, Bea changes the second and adds a fourth three days ago.
fn make(dir: &Path) -> Result<String, String> {
    std::fs::write(dir.join(FILE), "alpha\nbeta\ngamma\n").map_err(|e| e.to_string())?;
    git(dir, "Ada", 10, &["init", "--quiet", "--initial-branch=main"])?;
    git(dir, "Ada", 10, &["add", FILE])?;
    git(dir, "Ada", 10, &["commit", "--quiet", "-m", "Write the first lines"])?;
    std::fs::write(dir.join(FILE), "alpha\nBETA\ngamma\ndelta\n").map_err(|e| e.to_string())?;
    git(
        dir,
        "Bea",
        3,
        &[
            "commit",
            "--quiet",
            "-am",
            "Change the second line\n\nAnd add a fourth, for the self-test.",
        ],
    )?;
    git(dir, "Bea", 3, &["rev-parse", "HEAD"])
}

thread_local! {
    static HEAD: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn head() -> String {
    HEAD.with(|h| h.borrow().clone())
}

fn picture(name: &str) {
    if let Some(window) = window::for_project(&root()).and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

fn line_end_text(view: &EditorTextView) -> Option<String> {
    insets::of(view, Layer::Blame).first().map(|i| i.text.clone())
}

fn column(view: &EditorTextView) -> Vec<(usize, String)> {
    blame::column(view).map(|c| c.shown()).unwrap_or_default()
}

pub fn run() {
    let dir = root();
    match make(&dir) {
        Ok(hash) => HEAD.with(|h| *h.borrow_mut() = hash),
        Err(error) => {
            check("a repository with two commits is made", false, error);
            return finish(SCENARIO);
        }
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&dir);
    crate::app::open_project(mtm, dir.clone());
    if let Some(controller) = window::for_project(&dir) {
        controller.open_file(&dir.join(FILE));
    }
    wait_until(
        || blamed().is_some_and(|v| crate::editor::blame_probe::line(&v, 1, false).is_some()),
        |ok| {
            check("blame of the file is read in the background", ok, "");
            line_end();
        },
    );
}

/// After a rest on Bea's line, its end reads "Bea, vor 3 Tagen · Change the second line".
fn line_end() {
    let Some(view) = blamed() else { return finish(SCENARIO) };
    crate::editor::set_blame_at_line_end(true);
    let at = view.line_start(1).unwrap_or(0);
    view.setSelectedRange(NSRange::new(at, 0));
    wait_until(
        || view_line_end().is_some_and(|text| text.contains("Bea")),
        |_| {
            let Some(view) = blamed() else { return finish(SCENARIO) };
            let text = line_end_text(&view).unwrap_or_default();
            check(
                "at the end of the cursor's line after a rest: author, age and subject",
                text == splat_i18n::tf(
                    "{}, {} · {}",
                    &["Bea", &crate::git_panel::ago(3 * 86_400), "Change the second line"],
                ),
                text,
            );
            if let Some(window) = view.window() {
                window.displayIfNeeded();
            }
            let drawn = insets::shown(&view);
            check(
                "it is drawn after the line's end, not in the text",
                drawn.iter().any(|s| s.layer == Layer::Blame && !s.in_gap) && insets::kerns(&view).is_empty(),
                format!("{drawn:?}"),
            );
            picture("blame-line-end");
            blame::toggle_column(&view);
            main_after(Duration::from_millis(300), column_blocks);
        },
    );
}

fn view_line_end() -> Option<String> {
    blamed().and_then(|v| line_end_text(&v))
}

fn column_blocks() {
    let Some(view) = blamed() else { return finish(SCENARIO) };
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
    let shown = column(&view);
    let authors: Vec<(usize, bool)> = shown
        .iter()
        .map(|(line, text)| (*line, text.starts_with("Bea")))
        .collect();
    check(
        "\"Blame zeigen\": one entry per block before the line numbers",
        authors == [(0, false), (1, true), (2, false), (3, true)],
        format!("{shown:?}"),
    );
    picture("blame-column");
    super::helpers::type_at(&view, NSRange::new(0, 0), "new first line\n");
    let second = view.line_start(3).unwrap_or(0);
    super::helpers::type_at(&view, NSRange::new(second, 0), "edited ");
    view.setSelectedRange(NSRange::new(0, 0));
    wait_until(
        || {
            blamed().is_some_and(|v| {
                let shown = column(&v);
                shown.first().is_some_and(|(_, text)| text == t("Nicht committet")) && shown.len() >= 4
            })
        },
        unsaved,
    );
}

/// Unsaved lines read "Nicht committet"; the others keep their commits, moved down by the inserted line.
fn unsaved(ok: bool) {
    let Some(view) = blamed() else { return finish(SCENARIO) };
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
    let shown = column(&view);
    let uncommitted = t("Nicht committet");
    let expected = [
        (0, true, false),
        (1, false, false),
        (2, false, true),
        (3, true, false),
        (4, false, true),
    ];
    let seen: Vec<(usize, bool, bool)> = shown
        .iter()
        .map(|(line, text)| (*line, text == uncommitted, text.starts_with("Bea")))
        .collect();
    check(
        "with unsaved changes, new and edited lines read \"Nicht committet\" and the rest keep their commits",
        ok && view.document().is_dirty() && seen == expected,
        format!("{shown:?}"),
    );
    picture("blame-unsaved");
    wait_until(
        || view_line_end().is_some_and(|text| text == t("Nicht committet")),
        |ok| {
            check(
                "the line end of an unsaved new line reads \"Nicht committet\"",
                ok,
                view_line_end().unwrap_or_default(),
            );
            popover();
        },
    );
}

fn popover() {
    let Some(view) = blamed() else { return finish(SCENARIO) };
    let at = view.line_start(2).unwrap_or(0);
    view.setSelectedRange(NSRange::new(at, 0));
    blame::show_for_cursor(&view);
    wait_until(
        || {
            blamed().and_then(|v| blame::popover_texts(&v)).is_some_and(|texts| {
                texts
                    .iter()
                    .any(|text| text.contains("And add a fourth, for the self-test."))
            })
        },
        |ok| {
            let Some(view) = blamed() else { return finish(SCENARIO) };
            let texts = blame::popover_texts(&view).unwrap_or_default();
            check(
                "\"Blame der Zeile zeigen\": a popover with the whole message and its three buttons",
                ok && [t("Commit-ID kopieren"), t("Im Graph zeigen"), t("Diff zeigen")]
                    .iter()
                    .all(|button| texts.iter().any(|text| text == button)),
                texts.join(" | "),
            );
            picture("blame-popover");
            blame::copy_hash(&view);
            // SAFETY: The string type is AppKit's constant.
            let copied =
                unsafe { NSPasteboard::generalPasteboard().stringForType(objc2_app_kit::NSPasteboardTypeString) }
                    .map(|s| s.to_string());
            check(
                "\"Commit-ID kopieren\" puts the full hash on the pasteboard",
                copied.as_deref() == Some(head().as_str()),
                format!("{copied:?}"),
            );
            blame::show_for_cursor(&view);
            blame::show_in_graph(&view);
            wait_until(
                || {
                    window::for_project(&root())
                        .and_then(|c| crate::git_panel::selected_commit(&c))
                        .is_some_and(|hash| hash == head())
                },
                graph,
            );
        },
    );
}

fn graph(ok: bool) {
    let selected = window::for_project(&root()).and_then(|c| crate::git_panel::selected_commit(&c));
    check(
        "\"Im Graph zeigen\" selects the commit in the Git panel's history",
        ok,
        format!("{selected:?}"),
    );
    picture("blame-graph");
    let Some(view) = blamed() else { return finish(SCENARIO) };
    blame::show_for_cursor(&view);
    blame::show_diff(&view);
    let short: String = head().chars().take(7).collect();
    wait_until(
        move || {
            window::for_project(&root())
                .is_some_and(|c| c.tab_titles().iter().any(|t| t.contains(FILE) && t.contains(&short)))
        },
        clicks,
    );
}

/// A click on the line end opens the popover; outside a trusted folder blame does not run.
fn clicks(ok: bool) {
    let titles = window::for_project(&root()).map(|c| c.tab_titles()).unwrap_or_default();
    check(
        "\"Diff zeigen\" opens the commit's diff of the file in a tab",
        ok,
        titles.join(", "),
    );
    let Some(controller) = window::for_project(&root()) else {
        return finish(SCENARIO);
    };
    controller.open_file(&root().join(FILE));
    main_after(Duration::from_millis(900), || {
        let Some(view) = blamed() else { return finish(SCENARIO) };
        if let Some(window) = view.window() {
            window.displayIfNeeded();
        }
        let target = insets::shown(&view).into_iter().find(|s| s.layer == Layer::Blame);
        if let Some(target) = &target {
            let point = NSPoint::new(
                target.rect.origin.x + 4.0,
                target.rect.origin.y + target.rect.size.height / 2.0,
            );
            super::insets::click_at(&view, point);
        }
        check(
            "a click on the blame at the line's end opens its popover",
            target.is_some() && blame::popover_texts(&view).is_some(),
            format!("{target:?}"),
        );
        blame::dismiss(&view);
        if let (Some(target), Some(window)) = (&target, view.window()) {
            let point = NSPoint::new(
                target.rect.origin.x + 6.0,
                target.rect.origin.y + target.rect.size.height / 2.0,
            );
            let moved = objc2_app_kit::NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                objc2_app_kit::NSEventType::MouseMoved,
                view.convertPoint_toView(point, None),
                objc2_app_kit::NSEventModifierFlags::empty(),
                0.0,
                window.windowNumber(),
                None,
                0,
                0,
                0.0,
            );
            if let Some(moved) = moved {
                view.mouseMoved(&moved);
            }
        }
        main_after(Duration::from_millis(1200), || {
            let Some(view) = blamed() else { return finish(SCENARIO) };
            check(
                "resting the mouse on the blame at the line's end opens its popover",
                blame::popover_texts(&view).is_some(),
                "",
            );
            untrusted(&view);
        });
    });
}

fn untrusted(view: &EditorTextView) {
    crate::project::distrust(&root());
    let validated = {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let item = crate::util::item(mtm, t("Blame zeigen"), Some(objc2::sel!(ideToggleBlameColumn:)), "");
        // SAFETY: The editor answers validateMenuItem: with a bool for the item, which lives for the call.
        let enabled: bool = unsafe { objc2::msg_send![view, validateMenuItem: &*item] };
        enabled
    };
    check(
        "outside a trusted folder \"Blame zeigen\" is greyed out",
        !validated,
        "",
    );
    crate::project::trust(&root());
    crate::editor::set_blame_at_line_end(false);
    let saved = editor::save_all();
    check("every editor is saved before quitting", saved, "");
    finish(SCENARIO);
}
