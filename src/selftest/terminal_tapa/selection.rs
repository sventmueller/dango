//! Selecting, copying and pasting: smart selection by double click, copying with formatting, a picture dropped from
//! elsewhere, the sheet before a multi-line paste, and Secure Keyboard Entry from the app menu, after which the
//! scenario ends.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOn, NSEventModifierFlags, NSPasteboard, NSPasteboardTypeHTML,
    NSPasteboardTypePNG, NSPasteboardTypeRTF,
};
use objc2_foundation::NSData;
use splat_i18n::t;
use term_session::{Foreground, Point};

use super::super::check;
use super::{
    Run, after_prompt, command, empty_line, end, inspect, menu_item, pasteboard_text, pasted_text, screen, stop, wait,
};
use crate::terminal::TerminalView;
use crate::util::constant;

/// A command that prints a URL, a path with its line and column, and a bold red word, and the line it prints.
const PRINT: &str = "printf 'https://example.com/tapa-docs src/tapa/main.rs:12:5 \\033[1;31mtapa-bold\\033[0m\\n'\r";
const PRINTED: &str = "https://example.com/tapa-docs src/tapa/main.rs:12:5 tapa-bold";
/// Two pastes of two lines each, the first cancelled and the second sent.
const CANCELLED: &str = "tapa-cancelled-one\ntapa-cancelled-two\n";
const ACCEPTED: &str = "tapa-accepted-one\ntapa-accepted-two\n";

/// A double click selects a whole URL, a path with its line and column, or a word.
pub(super) fn smart(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    view.type_text(PRINT);
    wait(
        &run.clone(),
        10,
        |view| view.drawn_lines().iter().any(|line| line == PRINTED),
        move |drawn| {
            let Some(view) = inspect::view(run.id) else {
                return stop(&run, "the terminal is open");
            };
            let line = view.drawn_lines().iter().position(|line| line == PRINTED);
            let at = |part: &str| {
                let column = PRINTED.find(part).unwrap_or(0) + 2;
                line.and_then(|line| view.double_click_at(Point { line, column }))
            };
            let picked = [at("example"), at("src/"), at("tapa-bold")];
            check(
                "a double click selects a whole URL, a path with its line and column, or a word",
                drawn
                    && picked
                        == [
                            Some("https://example.com/tapa-docs".to_string()),
                            Some("src/tapa/main.rs:12:5".to_string()),
                            Some("tapa-bold".to_string()),
                        ],
                format!("line {line:?}: {picked:?}"),
            );
            formatted(&run);
            dropped_picture(run);
        },
    );
}

/// Copy with Formatting puts the selected bold red word on the pasteboard as text, RTF and HTML.
fn formatted(run: &Run) {
    let chosen = command(run, t("Mit Formatierung kopieren"));
    let rtf = NSPasteboard::generalPasteboard()
        .dataForType(constant!(NSPasteboardTypeRTF))
        .map(|data| String::from_utf8_lossy(&data.to_vec()).into_owned())
        .unwrap_or_default();
    let html = pasteboard_text(constant!(NSPasteboardTypeHTML)).unwrap_or_default();
    let plain = pasted_text();
    check(
        "Copy with Formatting puts the selection on the pasteboard as text, and as RTF and HTML with its bold and its \
         colour",
        chosen
            && plain.as_deref() == Some("tapa-bold")
            && rtf.contains("tapa-bold")
            && rtf.contains("\\b ")
            && html.contains("tapa-bold")
            && html.contains("font-weight: bold")
            && html.contains("color: #"),
        format!("{plain:?}, RTF {rtf:?}, HTML {html:?}"),
    );
}

/// The line typed at the shell's prompt.
fn typed(view: &TerminalView) -> Option<String> {
    view.with_session(|session| session.typed_line())
        .flatten()
        .map(|(line, _)| line.text)
}

/// A picture dropped from elsewhere, PNG data and no file, goes the way of a pasted one: with the shell in front its
/// copy is saved and the copy's quoted path lands on the command line.
fn dropped_picture(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    pasteboard.setData_forType(
        Some(&NSData::with_bytes(&super::super::terminal::PICTURE)),
        constant!(NSPasteboardTypePNG),
    );
    let dropped = view.drop_pasteboard(&pasteboard);
    // SAFETY: `releaseGlobally` takes nothing; objc2 leaves it out because of its name.
    let () = unsafe { msg_send![&*pasteboard, releaseGlobally] };
    let folder = term_session::paste::default_image_dir();
    let saved = move |view: &TerminalView| {
        typed(view).is_some_and(|line| std::path::Path::new(line.trim().trim_matches('\'')).starts_with(&folder))
    };
    wait(&run.clone(), 5, saved, move |arrived| {
        let line = inspect::view(run.id).and_then(|view| typed(&view)).unwrap_or_default();
        let path = line.trim().trim_matches('\'').to_string();
        let png = std::fs::read(&path).ok();
        check(
            "a picture dropped from elsewhere is saved like a pasted one, and with the shell in front its quoted path \
             lands on the command line",
            dropped == (true, true)
                && arrived
                && png.is_some_and(|png| png.starts_with(b"\x89PNG"))
                && path.ends_with(".png"),
            format!("{dropped:?}, {line:?}"),
        );
        if let Some(view) = inspect::view(run.id) {
            empty_line(&view);
        }
        program_reads(run);
    });
}

/// Starts `cat`, which reads the pastes without bracketed paste, so they would run as typed.
fn program_reads(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let prompts = view.prompts();
    view.type_text("cat\r");
    let reading = |view: &TerminalView| {
        matches!(view.foreground(), Foreground::Other(name) if name == "cat")
            && view
                .with_session(|session| !session.input_modes().bracketed_paste)
                .unwrap_or(false)
    };
    wait(&run.clone(), 10, reading, move |reading| {
        if !reading {
            return stop(&run, "cat reads in the terminal");
        }
        paste_cancelled(run, prompts);
    });
}

fn has_sheet(view: &TerminalView) -> bool {
    view.window().is_some_and(|window| window.attachedSheet().is_some())
}

/// The first multi-line paste asks in a sheet, which Cancel ends.
fn paste_cancelled(run: Run, prompts: usize) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let asked = view.paste_text_for_selftest(CANCELLED);
    wait(&run.clone(), 5, has_sheet, move |shown| {
        let cancelled = inspect::view(run.id).is_some_and(|view| view.answer_sheet(false));
        wait(
            &run.clone(),
            5,
            |view| !has_sheet(view),
            move |_| paste_accepted(run, prompts, asked && shown && cancelled),
        );
    });
}

/// The second asks too, and Paste sends every line of it; nothing of the cancelled one arrived before it.
fn paste_accepted(run: Run, prompts: usize, cancelled: bool) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let asked = view.paste_text_for_selftest(ACCEPTED);
    wait(&run.clone(), 5, has_sheet, move |shown| {
        let accepted = inspect::view(run.id).is_some_and(|view| view.answer_sheet(true));
        let arrived = |view: &TerminalView| {
            let lines = view.visible_lines();
            ["tapa-accepted-one", "tapa-accepted-two"]
                .iter()
                .all(|line| lines.iter().any(|shown| shown == line))
        };
        wait(&run.clone(), 5, arrived, move |arrived| {
            let leaked = inspect::view(run.id)
                .is_some_and(|view| view.visible_lines().iter().any(|line| line.contains("tapa-cancelled")));
            check(
                "a multi-line paste into a program shows the sheet first; Cancel sends nothing, Paste sends every line",
                cancelled && asked && shown && accepted && arrived && !leaked,
                format!(
                    "cancelled {cancelled}, asked {asked}, accepted {accepted}, cancelled text arrived {leaked} | {}",
                    screen(run.id)
                ),
            );
            if let Some(view) = inspect::view(run.id) {
                view.press(0x08, "\u{3}", "c", NSEventModifierFlags::Control);
            }
            after_prompt(run, prompts, secure_entry);
        });
    });
}

/// The app delegate, which answers the app menu's commands.
fn delegate(mtm: MainThreadMarker) -> Option<Retained<AnyObject>> {
    let delegate = NSApplication::sharedApplication(mtm).delegate()?;
    let object: &AnyObject = delegate.as_ref();
    Some(objc2::Message::retain(object))
}

/// Secure Keyboard Entry from the app menu turns on and off and its item's tick follows; it ends as it was.
fn secure_entry(run: Run) {
    let Some(mtm) = MainThreadMarker::new() else {
        return stop(&run, "the scenario runs on the main thread");
    };
    let (Some(item), Some(delegate)) = (menu_item(mtm, sel!(ideSecureKeyboardEntry:), |_| true), delegate(mtm)) else {
        return stop(&run, "the app menu has Secure Keyboard Entry");
    };
    let before = term_ui::secure::manual();
    let choose = || {
        // SAFETY: The app delegate answers the item's action, which takes the item, and validates it with a BOOL.
        let () = unsafe { msg_send![&*delegate, ideSecureKeyboardEntry: &*item] };
        // SAFETY: As above.
        let _: bool = unsafe { msg_send![&*delegate, validateMenuItem: &*item] };
        (term_ui::secure::manual(), item.state() == NSControlStateValueOn)
    };
    let switched = choose();
    let back = choose();
    if term_ui::secure::manual() != before {
        term_ui::secure::set_manual(before);
    }
    check(
        "Secure Keyboard Entry from the app menu turns on and off, and its item's tick follows",
        switched == (!before, !before) && back == (before, before),
        format!(
            "{before}, then {switched:?}, then {back:?}; held for a terminal now {}",
            term_ui::secure::active()
        ),
    );
    end(&run);
}
