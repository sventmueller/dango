//! The `inlay-hints` scenario (E83): inlay hints from the real rust-analyzer and gopls, drawn in gaps of their lines
//! (E81); a hint with a place jumps there; hints follow an edit and scrolling; the View menu and the settings turn
//! them off for all editors or one language; and a keystroke in 10 000 lines with hints on stays within E25's budget.
//! Without a server on this Mac its part reports itself skipped.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::{NSPoint, NSRange};

use super::{check, finish, helpers, snapshot};
use crate::editor::insets::{self, Layer};
use crate::editor::{self, EditorTextView};
use crate::window;

const SCENARIO: &str = "inlay-hints";
/// How often hints are asked for again while a server that has just started answers with none.
const ASK_AGAIN: Duration = Duration::from_secs(2);
/// The longest a server may take to start and answer with hints.
const WAIT: Duration = Duration::from_secs(150);
/// The whole scenario's time, below the runner's limit for it in `xtask/src/selftest.rs`.
const BUDGET: Duration = Duration::from_secs(560);

static END: helpers::Deadline = helpers::Deadline::new();

const RUST_MAIN: &str = "mod big;\n\nfn scale(value: u32, factor: u32) -> u32 {\n    value * factor\n}\n\nfn main() {\n    let total = scale(21, 2);\n    let words = vec![\"a\", \"b\"];\n    println!(\"{total} {words:?} {}\", big::f0(1));\n}\n";
const GO_MAIN: &str = "package main\n\nimport \"fmt\"\n\nfunc add(left, right int) int {\n\treturn left + right\n}\n\nfunc main() {\n\ttotal := add(1, 2)\n\tfmt.Println(total)\n}\n";

const RUST_PROJECT: &str = "inlay-rust";
const GO_PROJECT: &str = "inlay-go";

fn rust_root() -> PathBuf {
    helpers::root(RUST_PROJECT)
}

fn go_root() -> PathBuf {
    helpers::root(GO_PROJECT)
}

/// Ten thousand lines of small functions, each with a `let` whose type rust-analyzer shows.
fn big_rust() -> String {
    (0..2000)
        .map(|n| format!("pub fn f{n}(x: u32) -> u32 {{\n    // f{n}\n    let y = x + {n};\n    y\n}}\n"))
        .collect()
}

fn view(root: &Path, file: &str) -> Option<Retained<EditorTextView>> {
    let controller = window::for_project(root)?;
    editor::editor_for(&controller, &root.join(file))
}

fn offset_of(view: &EditorTextView, needle: &str, into: usize) -> Option<usize> {
    let text = view.document().text().to_string();
    let byte = text.find(needle)?;
    Some(text[..byte].encode_utf16().count() + into)
}

fn hints(view: &EditorTextView) -> Vec<(usize, String, bool)> {
    crate::editor::insets::inlay_shown(view)
}

fn has_hint(
    root: PathBuf,
    file: &'static str,
    needle: &'static str,
    into: usize,
    label: &'static str,
) -> impl Fn() -> bool {
    move || {
        view(&root, file).is_some_and(|v| {
            let at = offset_of(&v, needle, into);
            hints(&v)
                .iter()
                .any(|(a, text, _)| Some(*a) == at && text.contains(label))
        })
    }
}

fn redraw(view: &EditorTextView) {
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
}

fn picture(root: &Path, name: &str) {
    if let Some(window) = window::for_project(root).and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

/// Calls `then` with whether `ready` held, as soon as it does or once `within` or the scenario's budget passed; asks
/// the editor `again` for hints every `ASK_AGAIN` meanwhile, as a server that has just started answers with none.
fn poll(
    within: Duration,
    again: Option<(PathBuf, &'static str)>,
    ready: impl Fn() -> bool + 'static,
    then: impl FnOnce(bool, Duration) + 'static,
) {
    let asked = std::cell::Cell::new(Instant::now());
    let ready = move || {
        if ready() {
            return true;
        }
        if let Some((root, file)) = &again
            && asked.get().elapsed() >= ASK_AGAIN
            && let Some(view) = view(root, file)
        {
            ide_ui_lsp::inlay::request(&*view);
            asked.set(Instant::now());
        }
        false
    };
    END.poll(within, ready, then);
}

pub fn run() {
    END.start(BUDGET);
    let big = big_rust();
    let written = helpers::make_project(
        RUST_PROJECT,
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"inlay\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            ("src/main.rs", RUST_MAIN),
            ("src/big.rs", &big),
        ],
    );
    let root = match written {
        Ok(root) => root,
        Err(error) => {
            check("a cargo project is made", false, error.to_string());
            return finish(SCENARIO);
        }
    };
    helpers::open_trusted(&root);
    let Some(controller) = window::for_project(&root) else {
        check("the project opens", false, "");
        return finish(SCENARIO);
    };
    controller.open_file(&root.join("src/main.rs"));
    poll(
        WAIT,
        Some((root.clone(), "src/main.rs")),
        has_hint(root, "src/main.rs", "total", 5, "u32"),
        rust_hints,
    );
}

fn rust_hints(ok: bool, waited: Duration) {
    let root = rust_root();
    let Some(view) = view(&root, "src/main.rs") else {
        return finish(SCENARIO);
    };
    if !ok && ide_ui_lsp::server_running(&*view).is_err() {
        check(
            "skipped: rust-analyzer missing",
            true,
            format!("{:?}", ide_ui_lsp::server_running(&*view)),
        );
        return go();
    }
    redraw(&view);
    let found = hints(&view);
    let param = offset_of(&view, "21", 0);
    check(
        "rust-analyzer: a type hint after a let and a parameter name before an argument",
        ok && found
            .iter()
            .any(|(at, text, _)| Some(*at) == param && text.contains("value")),
        format!("after {:.1} s: {found:?}", waited.as_secs_f64()),
    );
    let total = offset_of(&view, "total", 5);
    let drawn = insets::shown(&view);
    check(
        "the hint shows in a gap after its name, smaller and apart from the text",
        drawn.iter().any(|s| s.layer == Layer::Inlay && s.in_gap)
            && insets::kerns(&view)
                .iter()
                .any(|(range, width)| Some(range.end) == total && *width > 10.0),
        format!("{} drawn, kerns {:?}", drawn.len(), insets::kerns(&view)),
    );
    let rotor = insets::rotor_items(&view);
    check(
        "VoiceOver's rotor \"Hinweise\" lists the hints with their line",
        rotor
            .iter()
            .any(|item| item.label.contains("u32") && item.label.contains('8')),
        format!("{rotor:?}"),
    );
    picture(&root, "inlay-rust");
    tooltips(&view);
    jump(&view);
}

fn tooltips(view: &EditorTextView) {
    let shown = insets::shown(view);
    let with_tip: Vec<_> = insets::of(view, Layer::Inlay)
        .into_iter()
        .enumerate()
        .filter(|(_, inset)| inset.tooltip.is_some())
        .collect();
    let all_reach = with_tip.iter().all(|(index, inset)| {
        shown
            .iter()
            .find(|s| s.layer == Layer::Inlay && s.index == *index)
            .is_none_or(|s| {
                let point = objc2_foundation::NSPoint::new(s.rect.origin.x + 2.0, s.rect.origin.y + 2.0);
                insets::tooltip(view, point) == inset.tooltip
            })
    });
    check(
        "a hint's tooltip is the server's tooltip, where it sends one",
        all_reach,
        format!("{} of the hints carry a tooltip", with_tip.len()),
    );
}

/// A click on a hint whose label names a place opens that place.
fn jump(view: &EditorTextView) {
    let root = rust_root();
    let target = insets::shown(view).into_iter().find(|s| {
        s.layer == Layer::Inlay
            && insets::of(view, Layer::Inlay)
                .get(s.index)
                .is_some_and(|i| i.action.is_some())
    });
    let Some(target) = target else {
        check(
            "a hint with a place to jump to is shown",
            false,
            format!("{:?}", hints(view)),
        );
        return edit();
    };
    let point = objc2_foundation::NSPoint::new(
        target.rect.origin.x + target.rect.size.width / 2.0,
        target.rect.origin.y + target.rect.size.height / 2.0,
    );
    let before = window::for_project(&root).map(|c| c.tab_titles()).unwrap_or_default();
    let hit = insets::hits(view, point);
    let label = insets::of(view, Layer::Inlay).get(target.index).map(|i| i.text.clone());
    view.setSelectedRange(NSRange::new(0, 0));
    super::insets::click_at(view, point);
    let moved = move || {
        let opened = window::for_project(&rust_root()).is_some_and(|c| c.tab_titles().len() > before.len());
        opened || self::view(&rust_root(), "src/main.rs").is_some_and(|v| v.selectedRange().location > 0)
    };
    poll(Duration::from_secs(20), None, moved, move |ok, _| {
        let titles = window::for_project(&rust_root())
            .map(|c| c.tab_titles())
            .unwrap_or_default();
        let selection = self::view(&rust_root(), "src/main.rs").map(|v| v.selectedRange().location);
        check(
            "a click on a hint with a place jumps there, in this file or another",
            hit && ok,
            format!(
                "{label:?}: tabs {}, selection in main.rs {selection:?}",
                titles.join(", ")
            ),
        );
        if let Some(controller) = window::for_project(&rust_root()) {
            controller.open_file(&rust_root().join("src/main.rs"));
        }
        edit();
    });
}

/// After an edit the hints of the new line arrive.
fn edit() {
    let root = rust_root();
    let Some(view) = view(&root, "src/main.rs") else {
        return finish(SCENARIO);
    };
    let Some(at) = offset_of(&view, "    let words", 0) else {
        return finish(SCENARIO);
    };
    super::helpers::type_at(&view, NSRange::new(at, 0), "    let extra = total + 1;\n");
    poll(
        Duration::from_secs(60),
        None,
        has_hint(root, "src/main.rs", "extra", 5, "u32"),
        |ok, waited| {
            check(
                "after an edit the hints are asked for again and the new line gets its hint",
                ok,
                format!("{:.1} s", waited.as_secs_f64()),
            );
            switches();
        },
    );
}

fn find_item(menu: &NSMenu, action: objc2::runtime::Sel) -> Option<Retained<NSMenuItem>> {
    menu.itemArray().iter().find_map(|item| {
        if item.action() == Some(action) {
            return Some(item);
        }
        item.submenu().and_then(|submenu| find_item(&submenu, action))
    })
}

/// The View menu's item for `action`, validated by `view` as the menu bar does: its title, whether it is ticked and
/// whether it is enabled.
fn menu_state(view: &EditorTextView, action: objc2::runtime::Sel) -> Option<(String, bool, bool)> {
    let mtm = MainThreadMarker::new()?;
    let main = NSApplication::sharedApplication(mtm).mainMenu()?;
    let item = find_item(&main, action)?;
    // SAFETY: The editor answers validateMenuItem: with a bool for the item, which lives for the call.
    let enabled: bool = unsafe { objc2::msg_send![view, validateMenuItem: &*item] };
    Some((
        item.title().to_string(),
        item.state() == objc2_app_kit::NSControlStateValueOn,
        enabled,
    ))
}

/// The View menu and the settings turn hints off for every editor and for one language.
fn switches() {
    let root = rust_root();
    let Some(view) = view(&root, "src/main.rs") else {
        return finish(SCENARIO);
    };
    let Some(mtm) = MainThreadMarker::new() else { return };
    let on = menu_state(&view, objc2::sel!(ideToggleInlayHints:));
    let language = menu_state(&view, objc2::sel!(ideToggleInlayHintsLanguage:));
    check(
        "the View menu has \"Inlay-Hinweise\" ticked and one for the editor's language",
        on.as_ref().is_some_and(|(_, ticked, enabled)| *ticked && *enabled)
            && language
                .as_ref()
                .is_some_and(|(title, ticked, _)| title.contains("Rust") && *ticked),
        format!("{on:?}, {language:?}"),
    );
    ide_ui_lsp::inlay::toggle();
    crate::settings::show(mtm);
    let settings = crate::settings::editor_switches();
    let off = menu_state(&view, objc2::sel!(ideToggleInlayHints:));
    check(
        "switched off in the View menu, the hints and their gaps go, and the settings show it",
        hints(&view).is_empty()
            && insets::kerns(&view).is_empty()
            && off.is_some_and(|(_, ticked, _)| !ticked)
            && settings.as_ref().is_some_and(|(hints, _, _)| !hints),
        format!("{:?}, settings {settings:?}", hints(&view)),
    );
    ide_ui_lsp::inlay::toggle();
    ide_ui_lsp::inlay::toggle_language("Rust");
    crate::settings::show(mtm);
    let settings = crate::settings::editor_switches();
    let language = menu_state(&view, objc2::sel!(ideToggleInlayHintsLanguage:));
    check(
        "switched off for Rust, a Rust editor shows none and the settings leave Rust unticked",
        hints(&view).is_empty()
            && language.is_some_and(|(_, ticked, _)| !ticked)
            && settings.as_ref().is_some_and(|(hints, languages, _)| {
                *hints && !languages.iter().any(|l| l == "Rust") && languages.iter().any(|l| l == "Go")
            }),
        format!("{settings:?}"),
    );
    ide_ui_lsp::inlay::toggle_language("Rust");
    if let Some(window) = crate::settings::window() {
        window.orderOut(None);
    }
    poll(
        Duration::from_secs(30),
        Some((root.clone(), "src/main.rs")),
        has_hint(root, "src/main.rs", "total", 5, "u32"),
        |ok, _| {
            check("switched on again, the hints come back", ok, "");
            scroll_and_budget();
        },
    );
}

/// Scrolled to the middle of 10 000 lines, the hints of the lines in view arrive; typing there with hints on stays
/// within a frame.
fn scroll_and_budget() {
    let root = rust_root();
    let Some(controller) = window::for_project(&root) else {
        return finish(SCENARIO);
    };
    controller.open_file(&root.join("src/big.rs"));
    let Some(big) = view(&root, "src/big.rs") else {
        check("the big file opens", false, "");
        return go();
    };
    let middle = big.line_start(5000).unwrap_or(0);
    let byte = big.document().text().utf16_to_byte(middle).unwrap_or(0);
    editor::scroll_to_byte(&big, byte);
    poll(
        Duration::from_secs(90),
        None,
        || view_hints_near(5000),
        |ok, waited| {
            let root = rust_root();
            let shown = view(&root, "src/big.rs").map(|v| hints(&v).len()).unwrap_or(0);
            check(
                "scrolled to line 5000 of 10 000, the hints of the lines in view arrive",
                ok,
                format!("{shown} hints after {:.1} s", waited.as_secs_f64()),
            );
            picture(&root, "inlay-big");
            scroll_away();
        },
    );
}

/// Scrolled on to line 7000, the hints of line 5000 stay, and the overlay draws them when AppKit asks it for their
/// lines off screen, as it does ahead of the visible part while scrolling.
fn scroll_away() {
    let root = rust_root();
    let Some(big) = view(&root, "src/big.rs") else {
        return budget();
    };
    let at = big.line_start(7000).unwrap_or(0);
    editor::scroll_to_byte(&big, big.document().text().utf16_to_byte(at).unwrap_or(0));
    poll(
        Duration::from_secs(60),
        None,
        || view_hints_near(7000),
        |arrived, _| {
            let root = rust_root();
            let Some(big) = view(&root, "src/big.rs") else {
                return budget();
            };
            check(
                "hints of lines scrolled away from stay while the text is unchanged",
                arrived && view_hints_near(5000),
                format!("hints near 7000: {arrived}, near 5000: {}", view_hints_near(5000)),
            );
            let y = |line: usize| {
                let at = big.line_start(line).unwrap_or(0);
                editor::byte_y(&big, big.document().text().utf16_to_byte(at).unwrap_or(0)).unwrap_or(0.0)
            };
            let (top, bottom) = (y(5000), y(5030));
            let off_screen = objc2_foundation::NSRect::new(
                objc2_foundation::NSPoint::new(0.0, top),
                objc2_foundation::NSSize::new(big.bounds().size.width, (bottom - top).max(1.0)),
            );
            let drawn = editor::insets::drawn_in(&big, off_screen);
            check(
                "asked to draw lines off screen that have hints, the overlay draws them",
                drawn > 0,
                format!("{drawn} hints in lines 5000 to 5030 while line 7000 is in view"),
            );
            budget();
        },
    );
}

fn view_hints_near(line: usize) -> bool {
    let root = rust_root();
    view(&root, "src/big.rs").is_some_and(|v| {
        let (Some(from), Some(to)) = (v.line_start(line), v.line_start(line + 30)) else {
            return false;
        };
        hints(&v).iter().any(|(at, _, _)| (from..to).contains(at))
    })
}

fn budget() {
    let root = rust_root();
    let Some(big) = view(&root, "src/big.rs") else {
        return go();
    };
    let Some(line) = big.line_start(5001) else {
        return go();
    };
    let end = offset_of(&big, "// f1000\n", 8).unwrap_or(line);
    big.setSelectedRange(NSRange::new(end, 0));
    let shown = hints(&big).len();
    let saved = std::cell::RefCell::new(insets::of(&big, Layer::Inlay));
    let ((median, slowest), (plain_median, plain_slowest)) = super::insets::compare(
        &big,
        || insets::set(&big, Layer::Inlay, saved.borrow().clone()),
        || {
            *saved.borrow_mut() = insets::of(&big, Layer::Inlay);
            insets::clear(&big, Layer::Inlay);
        },
    );
    check(
        "E25: the server's hints add under 3 ms to a keystroke's median in 10 000 lines",
        shown > 0 && median.saturating_sub(plain_median) < Duration::from_millis(3),
        format!(
            "{shown} hints; with them median {median:?}, slowest {slowest:?}; without median {plain_median:?}, slowest {plain_slowest:?}"
        ),
    );
    let (with, slowest) = scroll_frames(&big);
    let kept = insets::of(&big, Layer::Inlay);
    insets::clear(&big, Layer::Inlay);
    let (without, plain_slowest) = scroll_frames(&big);
    insets::set(&big, Layer::Inlay, kept);
    check(
        "E25: scrolling through 10 000 lines with hints draws each step within a frame",
        with < Duration::from_millis(16),
        format!(
            "with hints median {with:?}, slowest {slowest:?}; without median {without:?}, slowest {plain_slowest:?}"
        ),
    );
    go();
}

/// The median and the slowest time of sixty scroll steps of half a page, each drawn at once as AppKit would.
fn scroll_frames(view: &EditorTextView) -> (Duration, Duration) {
    let Some(window) = view.window() else {
        return (Duration::ZERO, Duration::ZERO);
    };
    let start = view.visibleRect().origin;
    let step = view.visibleRect().size.height / 2.0;
    let mut times: Vec<Duration> = (1..=60)
        .map(|i| {
            let began = Instant::now();
            view.scrollPoint(NSPoint::new(start.x, start.y + step * f64::from(i)));
            window.displayIfNeeded();
            began.elapsed()
        })
        .collect();
    view.scrollPoint(start);
    times.sort();
    (times[times.len() / 2], times[times.len() - 1])
}

/// The same with gopls in a Go module.
fn go() {
    let root = match helpers::make_project(
        GO_PROJECT,
        &[("go.mod", "module inlay\n\ngo 1.21\n"), ("main.go", GO_MAIN)],
    ) {
        Ok(root) => root,
        Err(error) => {
            check("a Go module is made", false, error.to_string());
            return done();
        }
    };
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&root);
    super::helpers::open_beside(mtm, &root);
    let Some(controller) = window::for_project(&root) else {
        check("the Go module opens", false, "");
        return done();
    };
    controller.open_file(&root.join("main.go"));
    poll(
        WAIT,
        Some((root.clone(), "main.go")),
        has_hint(root, "main.go", "add(1", 4, "left"),
        |ok, waited| {
            let root = go_root();
            let Some(view) = view(&root, "main.go") else {
                return done();
            };
            if !ok && ide_ui_lsp::server_running(&*view).is_err() {
                check(
                    "skipped: gopls missing",
                    true,
                    format!("{:?}", ide_ui_lsp::server_running(&*view)),
                );
                return done();
            }
            redraw(&view);
            let found = hints(&view);
            let total = offset_of(&view, "total", 5);
            check(
                "gopls: parameter names before the arguments and the type of a new variable",
                ok && found
                    .iter()
                    .any(|(at, text, _)| Some(*at) == total && text.contains("int")),
                format!("after {:.1} s: {found:?}", waited.as_secs_f64()),
            );
            let in_gap = |v: &EditorTextView| insets::shown(v).iter().any(|s| s.layer == Layer::Inlay && s.in_gap);
            let probe = root.clone();
            helpers::wait_until(
                move || self::view(&probe, "main.go").is_some_and(|v| in_gap(&v)),
                move |shown| {
                    let kerns = self::view(&root, "main.go")
                        .map(|v| insets::kerns(&v))
                        .unwrap_or_default();
                    check("gopls's hints show in gaps", shown, format!("{kerns:?}"));
                    picture(&root, "inlay-go");
                    done();
                },
            );
        },
    );
}

fn done() {
    let saved = editor::save_all();
    check("every editor is saved before quitting", saved, "");
    finish(SCENARIO);
}
