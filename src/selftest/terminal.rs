//! The `terminal` scenario: a terminal tab in a folder's window, keys typed through the view as AppKit delivers them,
//! an input method composing, an image pasted with the shell in front, styled output in the light and the dark palette,
//! a flood of output, a selection dragged beyond the top, a new font size, the shell ending with its tab, and a shell
//! that exits while its tab stays.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use ide_terminal::PasteAction;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSAccessibilityTextAreaRole, NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua,
    NSAppearanceNameDarkAqua, NSBitmapImageRep, NSEventModifierFlags, NSEventType, NSPasteboard, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSData, NSPoint, NSString};

use super::{check, finish, give_up, helpers, snapshot};
use crate::terminal::{TerminalView, inspect};
use crate::util::constant;
use crate::window::WindowController;

const NAME: &str = "terminal";
const PROJECT: &str = "terminal-project";
/// How long the view takes to draw in a new appearance, which leaves its background alike when it already had it.
const APPEARANCE_REDRAW: Duration = Duration::from_millis(400);
/// How often the flood asks whether the main thread answers, which is what it measures.
const SAMPLE: Duration = Duration::from_millis(10);
/// How long the button stays down above the lines, during which the view keeps scrolling back.
const HELD_DRAG: Duration = Duration::from_millis(600);
/// How long the view must stay put after the button came up.
const AFTER_RELEASE: Duration = Duration::from_millis(300);

/// A PNG of two by two orange pixels.
pub(super) const PICTURE: [u8; 73] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xfd, 0xd4, 0x9a, 0x73, 0x00, 0x00, 0x00, 0x10, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xdf, 0xc1, 0x00, 0x44, 0x0c, 0x10, 0x0a, 0x00, 0x2e, 0xce, 0x06, 0x1d,
    0xeb, 0x6e, 0xe9, 0xf1, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// A command line that prints every kind of text the view draws, and a dev server's address.
const STYLES: &str = "printf '\\033[1mbold\\033[0m \\033[3mitalic\\033[0m \\033[1;3mboth\\033[0m \\033[4munder\\033[0m \
                      \\033[4:3mcurly\\033[0m \\033[4:2mdouble\\033[0m \\033[9mstrike\\033[0m \\033[2mdim\\033[0m \
                      \\033[7minverse\\033[0m \\033[31mred \\033[32mgreen \\033[33myellow \\033[34mblue \
                      \\033[35mmagenta \\033[36mcyan\\033[0m \\033[38;2;255;136;0mtrue\\033[48;2;40;90;160mcolour\\033[0m\
                      \\n漢字 ✓ é ü ╭──┬──╮ ╔═╦═╗ ╟─╫─╢ ┄┆╍┉ ▌▐█ ░▒▓\\n  ➜  Local:   http://localhost:5173/\\n'\r";

pub(crate) fn run() {
    helpers::open_project(NAME, PROJECT, &[("README.md", "# Terminal\n")], opened);
}

fn opened(controller: Retained<WindowController>) {
    let root = helpers::root(PROJECT);
    let tabs = controller.tab_count();
    crate::terminal::open_tab(&controller, None);
    let opened = inspect::latest();
    check(
        "a terminal opens in a tab of its own",
        controller.tab_count() == tabs + 1 && opened.is_some(),
        format!("{tabs} then {} tabs", controller.tab_count()),
    );
    let Some((id, _)) = opened else {
        give_up(NAME, "the terminal tab opens");
        return;
    };
    wait(
        id,
        Duration::from_secs(25),
        |view| view.prompts() > 0,
        move |started| {
            check("the login shell starts and prompts", started, screen(id));
            type_command(id, root);
        },
    );
}

fn controller_of(root: &Path) -> Option<Retained<WindowController>> {
    helpers::window_of(root)
}

/// The visible lines of the terminal `id` that hold text, for a check's detail.
fn screen(id: u64) -> String {
    inspect::view(id)
        .map(|view| view.visible_lines())
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ⏎ ")
}

/// Waits up to `limit` for something about the terminal view `id`.
fn wait(id: u64, limit: Duration, done: impl Fn(&TerminalView) -> bool + 'static, then: impl FnOnce(bool) + 'static) {
    helpers::wait_within(limit, move || inspect::view(id).is_some_and(|view| done(&view)), then);
}

fn type_command(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    view.type_text("echo hello-terminal\r");
    let echoed = |view: &TerminalView| view.visible_lines().iter().any(|line| line == "hello-terminal");
    wait(id, Duration::from_secs(10), echoed, move |echoed| {
        check(
            "keys typed into the view reach the shell and its output shows",
            echoed,
            screen(id),
        );
        wait(
            id,
            Duration::from_secs(10),
            |view| view.prompts() > 1,
            move |_| option_as_meta(id, root),
        );
    });
}

/// ⌥⌫ is Meta-Backspace, which deletes the word before the cursor in a shell.
fn option_as_meta(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    view.type_text("echo kept gone");
    view.press(0x33, "\u{7f}", "\u{7f}", NSEventModifierFlags::Option);
    view.type_text("\r");
    let deleted = |view: &TerminalView| view.visible_lines().iter().any(|line| line == "kept") && view.prompts() > 2;
    wait(id, Duration::from_secs(10), deleted, move |deleted| {
        check("⌥⌫ reaches the shell as Meta and deletes a word", deleted, screen(id));
        input_method(id, root);
    });
}

/// An input method composing Japanese: the kana show at the cursor while composed, the kanji reach the shell once
/// committed.
fn input_method(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    let blank = view.ink_after_cursor(4);
    view.compose("にほん", 3);
    let composing = view.composing();
    let ink = view.ink_after_cursor(4);
    let candidates = view.candidate_rect();
    let near = view.window().is_some_and(|window| {
        let frame = window.frame();
        let inside = |x: f64, from: f64, size: f64| x >= from && x <= from + size;
        inside(candidates.origin.x, frame.origin.x, frame.size.width)
            && inside(candidates.origin.y, frame.origin.y, frame.size.height)
    });
    let kept_back = !view.visible_lines().iter().any(|line| line.contains("にほん"));
    check(
        "text an input method composes shows at the cursor and stays out of the shell",
        composing == Some((0, 3)) && ink > blank && near && kept_back,
        format!("{composing:?}, {blank} then {ink} pixels, candidates at {candidates:?}"),
    );
    view.compose("日本", 2);
    view.commit("日本");
    let committed = |view: &TerminalView| {
        view.composing().is_none() && view.visible_lines().iter().any(|line| line.ends_with("日本"))
    };
    wait(id, Duration::from_secs(5), committed, move |committed| {
        check("the committed text reaches the shell", committed, screen(id));
        if let Some(view) = inspect::view(id) {
            view.press(0x20, "\u{15}", "u", NSEventModifierFlags::Control);
        }
        open_file_link(id, root);
    });
}

/// ⌘-click on `file:line:column` in the output opens the file with the cursor there, and closing it shows the
/// terminal again.
fn open_file_link(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    view.type_text("echo README.md:1:3\r");
    let printed = |view: &TerminalView| view.visible_lines().iter().any(|line| line == "README.md:1:3");
    wait(id, Duration::from_secs(5), printed, move |_| {
        let (Some(view), Some(controller)) = (inspect::view(id), controller_of(&root)) else {
            return give_up(NAME, "the terminal and the folder's window are open");
        };
        let terminal_tab = controller.selected_tab();
        let line = view.visible_lines().iter().position(|line| line == "README.md:1:3");
        if let Some(line) = line {
            let at = view.cell_center(ide_terminal::Point { line, column: 2 });
            view.mouse(NSEventType::LeftMouseDown, at, NSEventModifierFlags::Command);
            view.mouse(NSEventType::LeftMouseUp, at, NSEventModifierFlags::Command);
        }
        let editor = crate::editor::editor_of(&controller, &root.join("README.md"));
        let cursor = editor.as_ref().map(|editor| editor.selectedRange().location);
        if editor.is_some()
            && let Some(tab) = controller.selected_tab()
        {
            controller.close_tab(tab);
        }
        check(
            "⌘-click on file:line:column opens the file there and the terminal comes back after",
            cursor == Some(2) && controller.selected_tab() == terminal_tab,
            format!("line {line:?}, cursor at {cursor:?}"),
        );
        pointer(id, root);
    });
}

/// With ⌘ held over a link the pointer is the pointing hand; without ⌘ or off the link it is the I-beam.
fn pointer(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    view.type_text("echo https://example.com/docs\r");
    let printed = |view: &TerminalView| {
        view.visible_lines()
            .iter()
            .any(|line| line == "https://example.com/docs")
    };
    wait(id, Duration::from_secs(5), printed, move |_| {
        let Some(view) = inspect::view(id) else {
            return give_up(NAME, "the terminal is open");
        };
        let line = view
            .visible_lines()
            .iter()
            .position(|line| line == "https://example.com/docs");
        let (none, command) = (NSEventModifierFlags::empty(), NSEventModifierFlags::Command);
        let seen = line.map(|line| {
            let on_link = view.cell_center(ide_terminal::Point { line, column: 10 });
            let off_link = view.cell_center(ide_terminal::Point { line, column: 60 });
            view.mouse(NSEventType::MouseMoved, on_link, none);
            let plain = view.hand();
            view.modifiers(0x37, command);
            let pressed = view.hand();
            view.mouse(NSEventType::MouseMoved, off_link, command);
            let away = view.hand();
            view.mouse(NSEventType::MouseMoved, on_link, command);
            let back = view.hand();
            view.modifiers(0x37, none);
            [plain, pressed, away, back, view.hand()]
        });
        check(
            "⌘ over a link shows the pointing hand, without ⌘ or off the link the I-beam",
            seen == Some([false, true, false, true, false]),
            format!("plain, ⌘, ⌘ off the link, ⌘ back, released: {seen:?}"),
        );
        paste_image(id, root);
    });
}

fn paste_image(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    let tiff =
        NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&PICTURE)).and_then(|rep| rep.TIFFRepresentation());
    if let Some(tiff) = &tiff {
        pasteboard.setData_forType(Some(tiff), constant!(NSPasteboardTypeTIFF));
    }
    let action = view.paste_image_from(&pasteboard);
    // SAFETY: `releaseGlobally` takes nothing; objc2 leaves it out because of its name.
    let () = unsafe { msg_send![&*pasteboard, releaseGlobally] };
    let path = match &action {
        Some(PasteAction::InsertPath(path)) => Some(path.clone()),
        _ => None,
    };
    let stored = path.as_ref().and_then(|path| std::fs::read(path).ok());
    check(
        "an image pasted with the shell in front is stored as PNG and its path pasted",
        stored.is_some_and(|png| png.starts_with(b"\x89PNG")),
        format!("{action:?}"),
    );
    let name = path
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let shown =
        move |view: &TerminalView| !name.is_empty() && view.visible_lines().iter().any(|line| line.contains(&name));
    wait(id, Duration::from_secs(5), shown, move |shown| {
        check("the path lands on the command line", shown, screen(id));
        show_styles(id, root);
    });
}

fn show_styles(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    // ⌃U empties the command line the pasted path is on.
    view.press(0x20, "\u{15}", "u", NSEventModifierFlags::Control);
    view.type_text(STYLES);
    let printed = |view: &TerminalView| {
        view.visible_lines().iter().any(|line| line.starts_with("bold italic")) && view.dev_server_banner().is_some()
    };
    wait(id, Duration::from_secs(10), printed, move |printed| {
        let banner = inspect::view(id)
            .and_then(|view| view.dev_server_banner())
            .unwrap_or_default();
        check(
            "styled output prints and a dev server's address raises the banner",
            printed && banner.contains("http://localhost:5173/"),
            format!("{banner} | {}", screen(id)),
        );
        pictures(id, root);
    });
}

/// The window in the light and then the dark appearance, each as a picture.
fn pictures(id: u64, root: PathBuf) {
    let appear = move |name: &NSString| {
        if let Some(window) = inspect::view(id).and_then(|view| view.window()) {
            window.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
        }
    };
    let picture = move |file: &str| {
        let view = inspect::view(id)?;
        let window = view.window()?;
        snapshot(&window, file);
        view.background()
    };
    appear(constant!(NSAppearanceNameAqua));
    helpers::after(APPEARANCE_REDRAW, move || {
        let light = picture("terminal-light");
        appear(constant!(NSAppearanceNameDarkAqua));
        let background = move || inspect::view(id).and_then(|view| view.background());
        helpers::wait_until(
            move || background() != light,
            move |_| {
                let dark = picture("terminal-dark");
                let brightness = |rgb: Option<ide_terminal::Rgb>| {
                    rgb.map_or(0, |c| u32::from(c.r) + u32::from(c.g) + u32::from(c.b))
                };
                check(
                    "the colours follow the light and the dark appearance",
                    light.is_some() && dark.is_some() && brightness(light) > brightness(dark),
                    format!("{light:?} then {dark:?}"),
                );
                accessibility(id);
                if !hold(id) {
                    flood(id, root);
                }
            },
        );
    });
}

/// With `IDE_SELFTEST_HOLD` set, writes the window's number next to the report and leaves the window open for a real
/// capture (`screencapture -l`), since the pictures drawn from the views miss what the system renders into layers.
fn hold(id: u64) -> bool {
    let (Some(_), Some(report)) = (
        std::env::var_os("IDE_SELFTEST_HOLD"),
        std::env::var_os("IDE_SELFTEST_REPORT"),
    ) else {
        return false;
    };
    let number = inspect::view(id)
        .and_then(|view| view.window())
        .map(|window| window.windowNumber());
    if let Some(number) = number {
        let _ = std::fs::write(
            PathBuf::from(report).with_file_name("terminal.window"),
            number.to_string(),
        );
    }
    number.is_some()
}

/// What VoiceOver reads: a text area labelled as the terminal with the visible lines as its value.
fn accessibility(id: u64) {
    let Some(view) = inspect::view(id) else { return };
    // SAFETY: The accessibility getters take nothing and return a string or nil.
    let role: Option<Retained<NSString>> = unsafe { msg_send![&*view, accessibilityRole] };
    // SAFETY: As above.
    let label: Option<Retained<NSString>> = unsafe { msg_send![&*view, accessibilityLabel] };
    // SAFETY: The value getter takes nothing and returns an object or nil.
    let value: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, accessibilityValue] };
    let value = value
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|text| text.to_string());
    check(
        "VoiceOver reads the screen as a text area",
        role.as_deref() == Some(constant!(NSAccessibilityTextAreaRole))
            && label.is_some_and(|label| label.to_string().starts_with(splat_i18n::t("Terminal")))
            && value.as_deref().is_some_and(|text| text.contains("hello-terminal")),
        format!("{role:?}"),
    );
}

/// A flood of output, during which the main thread must keep answering.
fn flood(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    let prompts = view.prompts();
    view.lock_times();
    // `seq` alone writes line by line to a terminal; through `cat` the output comes in blocks, as from a build.
    view.type_text("seq 1 1500000 | cat; echo flood-done\r");
    let started = Instant::now();
    let flood = Flood {
        id,
        prompts,
        started,
        last: started,
        worst: Duration::ZERO,
    };
    measure(flood, root);
}

/// A flood under way: the prompts before it, when it started, and the longest time the main thread did not answer.
#[derive(Clone, Copy)]
struct Flood {
    id: u64,
    prompts: usize,
    started: Instant,
    last: Instant,
    worst: Duration,
}

/// Checks every 10 ms, without touching the screen, whether the shell prompts again after the flood.
fn measure(mut flood: Flood, root: PathBuf) {
    let now = Instant::now();
    flood.worst = flood.worst.max(now - flood.last);
    flood.last = now;
    let id = flood.id;
    let prompted = inspect::view(id).is_some_and(|view| view.prompts() > flood.prompts);
    if prompted || now - flood.started > Duration::from_secs(30) {
        let done = inspect::view(id).is_some_and(|view| view.visible_lines().iter().any(|line| line == "flood-done"));
        let locks = inspect::view(id).map(|view| view.lock_times()).unwrap_or_default();
        check(
            "the window keeps answering while 17 MB of output arrive",
            done && flood.worst < Duration::from_millis(50),
            format!(
                "{:.1} s, longest pause {} ms; a snapshot waited up to {:.1} ms for the screen and held it {:.1} ms",
                (now - flood.started).as_secs_f64(),
                flood.worst.as_millis(),
                locks.wait.as_secs_f64() * 1000.0,
                locks.hold.as_secs_f64() * 1000.0,
            ),
        );
        autoscroll(id, root);
        return;
    }
    helpers::after(SAMPLE, move || measure(flood, root));
}

/// A selection dragged above the lines keeps scrolling back while the button is held.
fn autoscroll(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return give_up(NAME, "the terminal is open");
    };
    let start = view.cell_center(ide_terminal::Point { line: 3, column: 2 });
    let above = NSPoint::new(start.x, -60.0);
    view.mouse(NSEventType::LeftMouseDown, start, NSEventModifierFlags::empty());
    view.mouse(NSEventType::LeftMouseDragged, above, NSEventModifierFlags::empty());
    helpers::after(HELD_DRAG, move || {
        let Some(view) = inspect::view(id) else {
            return give_up(NAME, "the terminal is open");
        };
        let back = view.scrolled_back();
        view.mouse(NSEventType::LeftMouseUp, above, NSEventModifierFlags::empty());
        helpers::after(AFTER_RELEASE, move || {
            let still = inspect::view(id).map_or(0, |view| view.scrolled_back());
            check(
                "a selection dragged above the lines scrolls back until the button comes up",
                back > 2 && still == back,
                format!("{back} lines back, {still} after release"),
            );
            font_size(id, root);
        });
    });
}

/// A new font size from the settings reaches the open terminal, whose grid follows.
fn font_size(id: u64, root: PathBuf) {
    let grid = || inspect::view(id).and_then(|view| view.grid());
    let before = grid();
    crate::terminal::set_font_size(20.0);
    let larger = grid();
    crate::terminal::set_font_size(ide_core::settings::keys::FONT_SIZE.default_value());
    let back = grid();
    check(
        "a new font size reaches the open terminal and resizes its grid",
        matches!((before, larger), (Some(b), Some(l)) if l.0 < b.0 && l.1 < b.1) && back == before,
        format!("{before:?} then {larger:?} then {back:?}"),
    );
    close_tab(id, root);
}

fn close_tab(id: u64, root: PathBuf) {
    let (Some(view), Some(controller)) = (inspect::view(id), controller_of(&root)) else {
        return give_up(NAME, "the terminal and the folder's window are open");
    };
    let shell = view.shell();
    drop(view);
    let tabs = controller.tab_count();
    // SAFETY: The action takes its sender, which may be nil.
    let () = unsafe { msg_send![&*controller, ideCloseTab: None::<&AnyObject>] };
    let after = controller.tab_count();
    let gone = move || {
        inspect::view(id).is_none()
            && shell.is_some_and(|pid| Supervisor::global().running().iter().all(|process| process.pid != pid))
    };
    helpers::wait_within(Duration::from_secs(8), gone, move |gone| {
        check(
            "closing the tab drops the view and ends the shell",
            gone && after + 1 == tabs,
            format!("{tabs} then {after} tabs; shell {shell:?}"),
        );
        exit_shell(root);
    });
}

/// A second terminal whose shell exits: its tab stays and says so.
fn exit_shell(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    crate::terminal::open_tab(&controller, None);
    let Some((id, _)) = inspect::latest() else {
        return give_up(NAME, "the terminal tab opens");
    };
    wait(
        id,
        Duration::from_secs(25),
        |view| view.prompts() > 0,
        move |_| {
            let tabs = controller_of(&root).map_or(0, |controller| controller.tab_count());
            if let Some(view) = inspect::view(id) {
                view.type_text("exit\r");
            }
            wait(id, Duration::from_secs(10), TerminalView::has_exited, move |exited| {
                let kept = controller_of(&root).map_or(0, |controller| controller.tab_count());
                check(
                    "a shell that exits keeps its tab",
                    exited && kept == tabs,
                    format!("{tabs} then {kept} tabs"),
                );
                if let Some(window) = inspect::view(id).and_then(|view| view.window()) {
                    snapshot(&window, "terminal-exited");
                }
                finish(NAME);
            });
        },
    );
}
