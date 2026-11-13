//! The `debug` scenario (B1, E94, E101): the fifth panel takes the focus with ⌘5, breakpoints set by a click and ⌘\
//! move with edits and draw outlined when switched off, the inspector follows ⌥⌘I and the tab in front, and a session
//! on the scripted adapter stops at a breakpoint: controls and menu items greyed out where the adapter or the moment
//! cannot, the call stack, variables with children, a watch, the console's evaluation, the values at the line ends,
//! the rotor, stepping and stopping, and the VoiceOver audit of the new views.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenuItem, NSView};
use objc2_foundation::NSRange;
use splat_i18n::t;

use super::a11y::keys::{Went, press};
use super::helpers::{editor, text_of, type_at, wait_until, window_of};
use super::{check, finish, snapshot};
use crate::debug::selftest::{self as debug, Command};
use crate::util::main_after;
use crate::window::{Panel, WindowController};

const SCENARIO: &str = "debug";
const PAUSE: Duration = Duration::from_millis(500);

/// The program the scripted adapter walks through, line by line.
pub const PROGRAM: &str = "fn main() {\n    let mut hits = 0;\n    let point = (1, 2);\n    hits += 1;\n    \
                           println!(\"{hits}\");\n    hits += point.0;\n    let line = hits;\n    \
                           println!(\"{line}\");\n    drop(point);\n    let done = true;\n    \
                           assert!(done);\n}\n";

/// A folder of its own under the run's home, with `src/main.rs`, trusted.
pub fn project(name: &str) -> PathBuf {
    let folder = ide_core::paths::home().join(name);
    let _ = std::fs::create_dir_all(folder.join("src"));
    let _ = std::fs::write(folder.join("src/main.rs"), PROGRAM);
    let folder = folder.canonicalize().unwrap_or(folder);
    crate::project::trust(&folder);
    folder
}

/// The scripted adapter for the folder's program: every line takes breakpoints, it cannot restart.
pub fn script(root: &Path) -> ide_dap::fake::Script {
    ide_dap::fake::Script {
        path: root.join("src/main.rs"),
        lines: (1..=13).collect(),
        restart: false,
        output: vec!["hello from the scripted program\n".into()],
        ..ide_dap::fake::Script::default()
    }
}

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = project("debug-project");
    crate::app::open_project(mtm, root.clone());
    let probe = root.clone();
    wait_until(
        move || window_of(&probe).is_some(),
        move |_| main_after(PAUSE, move || focus(root)),
    );
}

fn front(root: &Path) -> Option<Retained<WindowController>> {
    window_of(root)
}

/// The enabled state of the Debug menu's items, by title, as AppKit validates them before the menu opens.
fn debug_menu() -> Vec<(String, bool)> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let main = NSApplication::sharedApplication(mtm).mainMenu();
    let Some(menu) = main
        .and_then(|m| m.itemWithTitle(&crate::util::ns(t("Debuggen"))))
        .and_then(|item| item.submenu())
    else {
        return Vec::new();
    };
    menu.update();
    menu.itemArray()
        .iter()
        .filter(|item| !item.isSeparatorItem())
        .map(|item: Retained<NSMenuItem>| (item.title().to_string(), item.isEnabled()))
        .collect()
}

/// The key equivalent of the Debug menu's item `title`, with its modifiers.
fn shortcut(title: &str) -> Option<(String, usize)> {
    let mtm = MainThreadMarker::new()?;
    let main = NSApplication::sharedApplication(mtm).mainMenu()?;
    let menu = main.itemWithTitle(&crate::util::ns(t("Debuggen")))?.submenu()?;
    let item = menu.itemWithTitle(&crate::util::ns(title))?;
    Some((
        item.keyEquivalent().to_string(),
        item.keyEquivalentModifierMask().bits(),
    ))
}

fn enabled(menu: &[(String, bool)], title: &str) -> Option<bool> {
    menu.iter().find(|(t, _)| t == title).map(|(_, on)| *on)
}

/// ⌘5 shows the debug panel and puts the focus into its list; with no session, every control and menu item that acts
/// on one is greyed out.
fn focus(root: PathBuf) {
    let Some(controller) = front(&root) else {
        check("the folder opens", false, "");
        return finish(SCENARIO);
    };
    check(
        "the activity bar has six panels and settings",
        controller.inspect().activity_buttons == 7,
        format!("{} buttons", controller.inspect().activity_buttons),
    );
    let Some(window) = controller.window() else {
        return finish(SCENARIO);
    };
    let mut frame = window.frame();
    frame.size.width = 1700.0;
    window.setFrame_display(frame, true);
    let went = press(&window, "5", 23, NSEventModifierFlags::Command);
    let outline = debug::outline(&controller);
    let focused = window
        .firstResponder()
        .and_then(|r| r.downcast::<NSView>().ok())
        .zip(outline)
        .is_some_and(|(focus, outline)| focus.isDescendantOf(&outline));
    check(
        "⌘5 shows the debug panel and puts the focus into its list",
        matches!(went, Went::Menu(_)) && controller.inspect().panel == Some(Panel::Debug) && focused,
        format!("{went}, {:?}, focus in the list: {focused}", controller.inspect().panel),
    );
    let seen = debug::seen(&controller).unwrap_or_default();
    let menu = debug_menu();
    check(
        "without a session every control and the menu's session commands are greyed out",
        seen.controls.iter().all(|on| !on)
            && [
                t("Fortsetzen"),
                t("Überspringen"),
                t("Hineinspringen"),
                t("Heraus"),
                t("Neu starten"),
                t("Stopp"),
            ]
            .iter()
            .all(|title| enabled(&menu, title) == Some(false))
            && enabled(&menu, t("Starten")) == Some(true),
        format!("{:?} / {menu:?}", seen.controls),
    );
    check(
        "the start row offers the current program and attaching",
        seen.configurations
            .iter()
            .any(|c| c == t("Aktuelles Programm debuggen"))
            && seen.configurations.iter().any(|c| c == t("An Prozess anhängen …")),
        format!("{:?}", seen.configurations),
    );
    layout(&controller);
    controller.open_file(&root.join("src/main.rs"));
    main_after(PAUSE, move || breakpoints(root));
}

/// The panel in the sidebar's layout: Debug above CI, which comes last; the controls as large and as far apart as
/// the search panel's, the triangle before a section's title, the note flush with the panel's title, and the start
/// row above the list.
fn layout(controller: &WindowController) {
    let order = controller.shown_symbols();
    check(
        "the activity bar lists Debug above CI, which comes last",
        order.ends_with(&[Panel::Debug, Panel::Ci]),
        format!("{order:?}"),
    );
    let debug = controller.panel_title_row().map(|(_, row)| row);
    controller.activate_panel(Panel::Search);
    let search = controller.panel_title_row().map(|(_, row)| row);
    controller.activate_panel(Panel::Debug);
    let (Some(debug), Some(search)) = (debug, search) else {
        return check("the debug and search panels have title rows", false, "");
    };
    let same_size = |row: &crate::window::panels::TitleRow, other: &crate::window::panels::TitleRow| {
        let first = other.button_symbols.first().cloned().flatten();
        !row.button_symbols.is_empty()
            && row
                .button_symbols
                .iter()
                .all(|symbols| symbols.is_some() && symbols.as_deref() == first.as_deref())
    };
    check(
        "the controls are as large and as close together as the search panel's buttons, on the title's line",
        same_size(&debug, &search)
            && same_size(&search, &debug)
            && debug.button_spacing == search.button_spacing
            && (debug.title_middle - debug.buttons_middle).abs() < 3.0,
        format!(
            "{} and {} buttons, {:.1} and {:.1} pt apart",
            debug.button_symbols.len(),
            search.button_symbols.len(),
            debug.button_spacing,
            search.button_spacing
        ),
    );
    let seen = debug::layout(controller);
    let Some(seen) = seen else {
        return check("the debug panel lays out", false, "");
    };
    check(
        "a section's triangle comes before its title, and the note below starts where the panel's title does",
        seen.chevron.1 <= seen.section_title
            && (seen.chevron.0 - debug.title_start).abs() < 3.0
            && (seen.note - debug.title_start).abs() < 3.0,
        format!("{seen:?}, panel title at {:.1}", debug.title_start),
    );
    check(
        "the start row sits above the list",
        seen.start_top < seen.list_top && seen.start_top < 12.0,
        format!("{seen:?}"),
    );
}

/// A click in the gutter and ⌘\ set breakpoints, a switched-off one draws outlined, and typed and removed lines move
/// them.
fn breakpoints(root: PathBuf) {
    let file = root.join("src/main.rs");
    let (Some(controller), Some(view)) = (front(&root), front(&root).and_then(|c| editor(&c, "main.rs"))) else {
        check("the program opens in an editor", false, "");
        return finish(SCENARIO);
    };
    let clicked = crate::debug::gutter_click(&view, 3);
    let line_five = text_of(&view).lines().take(5).map(|l| l.len() + 1).sum::<usize>();
    view.setSelectedRange(NSRange::new(line_five, 0));
    // macOS moves ⌘\ to the key in the same place on the Mac's keyboard layout, such as ⌘# on a German one.
    let key = shortcut(t("Haltepunkt an der Cursorzeile")).map_or_else(|| "\\".to_string(), |(key, _)| key);
    let went = controller
        .window()
        .map(|w| press(&w, &key, 42, NSEventModifierFlags::Command));
    let lines = |controller: &WindowController| -> Vec<usize> {
        debug::seen(controller)
            .map(|s| s.breakpoints.iter().map(|b| b.1).collect())
            .unwrap_or_default()
    };
    check(
        "a click beside line 4 and ⌘\\ on line 6 set breakpoints there",
        clicked
            && lines(&controller) == [3, 5]
            && matches!(went, Some(Went::Menu(_)))
            && ide_core::keymap::Keymap::default().key("debug.toggleBreakpoint") == Some("cmd+\\"),
        format!(
            "{:?} after {}, shortcut {:?}",
            lines(&controller),
            went.map(|w| w.to_string()).unwrap_or_default(),
            shortcut(t("Haltepunkt an der Cursorzeile"))
        ),
    );
    debug::switch_breakpoint(&controller, &file, 5, false);
    let (off, _) = debug::gutter(&view, 5);
    let (on, _) = debug::gutter(&view, 3);
    check(
        "a switched-off breakpoint is drawn outlined, an active one filled",
        debug::is_outlined(off) && on.is_some() && !debug::is_outlined(on),
        format!("{off:?} / {on:?}"),
    );
    debug::switch_breakpoint(&controller, &file, 5, true);
    type_at(&view, NSRange::new(0, 0), "// a\n// b\n");
    let after_typing = lines(&controller);
    delete(&view, NSRange::new(0, 5));
    let after_removing = lines(&controller);
    check(
        "breakpoints move with the lines typed above them and back with the lines removed",
        after_typing == [5, 7] && after_removing == [4, 6],
        format!("{after_typing:?}, then {after_removing:?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "debug-breakpoints");
    }
    delete(&view, NSRange::new(0, 5));
    crate::editor::save(&view, false);
    main_after(PAUSE, move || inspector(root));
}

/// ⌥⌘I shows the inspector; it comes along to another tab with its width, and ⌥⌘I folds it again.
fn inspector(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    let Some(window) = controller.window() else {
        return finish(SCENARIO);
    };
    controller.show_inspector(false);
    let went = press(
        &window,
        "i",
        34,
        NSEventModifierFlags::Command | NSEventModifierFlags::Option,
    );
    let shown = controller.inspector_shown();
    let width = controller.inspector_width();
    check(
        "⌥⌘I shows the inspector",
        matches!(went, Went::Menu(_)) && shown,
        format!("{went}, shown {shown}, {width:.0} pt"),
    );
    main_after(PAUSE, move || inspector_seen(root, width));
}

/// The inspector ⌥⌘I unfolded lies inside the window once the tab has been laid out.
fn inspector_seen(root: PathBuf, width: f64) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    let inside = debug::inspector_page(&controller)
        .zip(controller.window())
        .map(|(page, window)| {
            let frame = page.convertRect_toView(page.bounds(), None);
            let content = window.contentLayoutRect();
            let right = frame.origin.x + frame.size.width;
            (
                page.window().is_some() && frame.size.width >= 100.0 && right <= content.size.width + 1.0,
                frame,
            )
        });
    check(
        "the inspector ⌥⌘I unfolded lies inside the window",
        inside.is_some_and(|(ok, _)| ok),
        format!("{inside:?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "debug-inspector-key");
    }
    let editor_tab = controller.selected_tab();
    controller.open_welcome();
    let page = debug::inspector_page(&controller);
    let moved = page
        .as_ref()
        .and_then(|p| p.window())
        .zip(controller.window())
        .is_some_and(|(a, b)| std::ptr::eq(&*a, &*b));
    let again = controller.inspector_width();
    check(
        "the inspector comes along to the tab in front, as wide as it was",
        controller.inspector_shown() && moved && (again - width).abs() < 2.0,
        format!(
            "shown {}, in the tab {moved}, {again:.0} pt",
            controller.inspector_shown()
        ),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "debug-inspector");
    }
    if let Some(tab) = editor_tab {
        controller.select_tab_by_id(tab);
    }
    let folded = controller.window().map(|w| {
        press(
            &w,
            "i",
            34,
            NSEventModifierFlags::Command | NSEventModifierFlags::Option,
        )
    });
    check(
        "⌥⌘I folds the inspector again",
        !controller.inspector_shown(),
        folded.map(|w| w.to_string()).unwrap_or_default(),
    );
    // The fold animates; the session starts once it is done, as a user's next step would.
    let starting = root.clone();
    main_after(Duration::from_millis(1500), move || {
        if let Some(controller) = front(&starting) {
            debug::start_fake(&controller, script(&starting));
        }
    });
    let probe = root.clone();
    wait_until(
        move || controller_paused(&probe),
        move |ok| {
            check(
                "a session on the scripted adapter stops at the first breakpoint",
                ok,
                "",
            );
            main_after(PAUSE, move || paused(root));
        },
    );
}

/// Deletes `range` of `view` as the Delete key does.
fn delete(view: &crate::editor::EditorTextView, range: NSRange) {
    view.setSelectedRange(range);
    let text_view: &objc2_app_kit::NSTextView = view;
    // SAFETY: `deleteBackward:` takes any sender; none is given.
    unsafe { objc2_app_kit::NSStandardKeyBindingResponding::deleteBackward(text_view, None) };
    super::helpers::post_event();
}

fn controller_paused(root: &Path) -> bool {
    front(root)
        .and_then(|c| debug::seen(&c))
        .is_some_and(|s| s.paused.is_some() && !s.locals.is_empty())
}

/// Stopped at the breakpoint: what the controls, the menu, the panel and the inspector show.
fn paused(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    let seen = debug::seen(&controller).unwrap_or_default();
    check(
        "the session stops on line 4 with its call stack and the innermost scope's variables",
        seen.paused.as_ref().is_some_and(|(_, line)| *line == 3) && !seen.frames.is_empty() && !seen.locals.is_empty(),
        format!("{:?}, {:?}, {:?}", seen.paused, seen.frames, seen.locals),
    );
    let menu = debug_menu();
    check(
        "paused, stepping and stopping are enabled, restarting greyed out as the adapter cannot",
        seen.controls == [true, true, true, true, false, true]
            && enabled(&menu, t("Fortsetzen")) == Some(true)
            && enabled(&menu, t("Neu starten")) == Some(false)
            && enabled(&menu, t("Stopp")) == Some(true),
        format!("{:?} / {menu:?}", seen.controls),
    );
    let activity = crate::debug::activity(&controller);
    check(
        "the activity view lists the debug session with how it stands",
        activity
            .iter()
            .any(|(_, name, state)| name == "main.rs" && state.starts_with(t("Angehalten: {}").trim_end_matches("{}"))),
        format!("{activity:?}"),
    );
    check(
        "the inspector shows itself as the session starts",
        controller.inspector_shown() && controller.inspector_width() > 100.0,
        format!(
            "{:.0} pt, {:?}",
            controller.inspector_width(),
            controller.inspector_states()
        ),
    );
    if let Some(view) = editor(&controller, "main.rs") {
        let (look, paused) = debug::gutter(&view, 3);
        let values = debug::inline_values(&view);
        let (rotors, entries) = debug::rotors(&view);
        check(
            "the paused line carries the arrow beside its breakpoint, and its values stand at the line ends",
            paused && look.is_some() && values.iter().any(|v| v.contains("hits = ")),
            format!("{look:?}, paused {paused}, {values:?}"),
        );
        check(
            "the rotor \"Haltepunkte\" lists the breakpoints and the paused line",
            rotors.iter().any(|r| r == t("Haltepunkte")) && entries.len() == 2,
            format!("{rotors:?}, {entries:?}"),
        );
        let point = text_of(&view)
            .find("point.0")
            .map(|at| super::helpers::utf16_len(&text_of(&view)[..at]) + 1);
        let expression = point.and_then(|at| debug::expression_at(&view, at));
        check(
            "the mouse resting on a name asks the session for its value",
            expression.as_deref() == Some("point") && point.is_some_and(|at| debug::hover(&view, at)),
            format!("{expression:?}"),
        );
    }
    debug::add_watch(&controller, "point.x");
    debug::open_variable(&controller, "point");
    debug::evaluate(&controller, "hits");
    let probe = root.clone();
    wait_until(
        move || {
            front(&probe).is_some_and(|c| {
                let seen = debug::seen(&c).unwrap_or_default();
                seen.watches.iter().all(|(_, v)| v.is_some())
                    && !debug::children_of(&c, "point").is_empty()
                    && seen.consoles.iter().any(|(_, text, _)| text.contains("› hits\n"))
            })
        },
        move |ok| inspected(root, ok),
    );
}

fn inspected(root: PathBuf, ok: bool) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    let seen = debug::seen(&controller).unwrap_or_default();
    check(
        "a watch is evaluated, a variable opens to its children, and the console evaluates in the paused frame",
        ok,
        format!(
            "{:?}, point: {:?}, consoles {:?}",
            seen.watches,
            debug::children_of(&controller, "point"),
            seen.consoles
        ),
    );
    if let Some(window) = controller.window() {
        if let Some(content) = window.contentView() {
            content.layoutSubtreeIfNeeded();
        }
        snapshot(&window, "debug-paused");
        audit(&window);
    }
    debug::run(&controller, Command::StepOver);
    let probe = root.clone();
    wait_until(
        move || {
            front(&probe)
                .and_then(|c| debug::seen(&c))
                .is_some_and(|s| s.paused.as_ref().is_some_and(|(_, line)| *line == 4))
        },
        move |ok| {
            check("Step Over stops on the next line", ok, "");
            main_after(PAUSE, move || continue_and_stop(root));
        },
    );
}

/// The VoiceOver audit of the panel, the inspector and the console (E101).
fn audit(window: &objc2_app_kit::NSWindow) {
    use super::a11y::audit;
    let ours = [
        t("Debug-Sitzungen und Haltepunkte"),
        t("Variablen und überwachte Ausdrücke"),
        t("Debug-Konsole"),
        t("Fortsetzen"),
        t("Überspringen"),
        t("Hineinspringen"),
        t("Heraus"),
        t("Neu starten"),
        t("Stopp"),
        t("Startkonfiguration"),
    ];
    let elements: Vec<audit::Element> = audit::elements(window)
        .into_iter()
        .filter(|e| ours.iter().any(|name| e.path.contains(name) || e.name.contains(name)))
        .collect();
    let findings = audit::findings(&elements);
    check(
        "VoiceOver misses nothing in the debug panel, the inspector and the console",
        !elements.is_empty() && findings.is_empty(),
        format!(
            "{} elements; {}",
            elements.len(),
            findings
                .iter()
                .map(|f| format!("{}: {} at {}", f.problem, f.what, f.path))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    );
}

/// Continue runs to the second breakpoint; Stop ends the session, greys the controls out and finishes the console.
fn continue_and_stop(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    debug::run(&controller, Command::Continue);
    let probe = root.clone();
    wait_until(
        move || {
            front(&probe)
                .and_then(|c| debug::seen(&c))
                .is_some_and(|s| s.paused.as_ref().is_some_and(|(_, line)| *line == 5))
        },
        move |ok| {
            check("Continue runs on to the next breakpoint", ok, "");
            let Some(controller) = front(&root) else {
                return finish(SCENARIO);
            };
            debug::run(&controller, Command::Stop);
            let probe = root.clone();
            wait_until(
                move || {
                    front(&probe)
                        .and_then(|c| debug::seen(&c))
                        .is_some_and(|s| s.consoles.iter().all(|(_, _, input)| !input))
                },
                move |ok| stopped(root, ok),
            );
        },
    );
}

fn stopped(root: PathBuf, ok: bool) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    let seen = debug::seen(&controller).unwrap_or_default();
    check(
        "Stop ends the session: its console is finished, every control greyed out",
        ok && seen.controls.iter().all(|on| !on)
            && seen
                .consoles
                .iter()
                .any(|(_, text, _)| text.contains("hello from the scripted program")),
        format!("{:?}, {:?}", seen.controls, seen.consoles),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "debug-stopped");
    }
    waiting(root);
}

/// A start that goes unanswered, as while macOS asks for the password of Developer Tools Access, says so after two
/// seconds and can be stopped (R23).
fn waiting(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return finish(SCENARIO);
    };
    let slow = ide_dap::fake::Script {
        launch_delay_ms: 30_000,
        ..script(&root)
    };
    debug::start_fake(&controller, slow);
    let probe = root.clone();
    wait_until(
        move || {
            front(&probe).and_then(|c| debug::seen(&c)).is_some_and(|s| {
                s.sessions
                    .iter()
                    .any(|(_, phase)| phase == t("Wartet auf Freigabe durch macOS …"))
            })
        },
        move |ok| {
            let Some(controller) = front(&root) else {
                return finish(SCENARIO);
            };
            let seen = debug::seen(&controller).unwrap_or_default();
            check(
                "a start without an answer says it waits for macOS, with Stop at hand",
                ok && seen.controls.last() == Some(&true),
                format!("{:?}, {:?}", seen.sessions, seen.controls),
            );
            if let Some(window) = controller.window() {
                snapshot(&window, "debug-waiting");
            }
            debug::run(&controller, Command::Stop);
            let probe = root.clone();
            wait_until(
                move || {
                    front(&probe).and_then(|c| debug::seen(&c)).is_some_and(|s| {
                        s.controls.iter().all(|on| !on) && s.consoles.iter().all(|(_, _, input)| !input)
                    })
                },
                move |ok| {
                    check("Stop ends a start that waits", ok, "");
                    finish(SCENARIO);
                },
            );
        },
    );
}
