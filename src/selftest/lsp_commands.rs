//! The `lsp` scenario's commands, with rust-analyzer still serving the project from its first part, as the
//! menu bar and the keyboard send them: the next and previous problem with its message, hover from the mouse resting
//! and leaving, the context menu's places to go and formatting, the file's symbols and a jump to one, renaming through
//! the sheet, formatting on save, the fixes of a type error from the menu and the bulb and one chosen after the text
//! changed, which is refused with a word; hover, definition, uses, parameter help, refactorings and completion from
//! the menu bar; and the status bar's button for a server of the user's that keeps crashing.

use std::time::Duration;

use ide_core::settings::{Settings, keys};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSTextField, NSView,
};
use objc2_foundation::NSPoint;
use splat_i18n::t;

use super::helpers::{answer_alert_saying, in_any_mode_after, wait_until};
use super::lsp::SCENARIO;
use super::lsp::{
    ANSWER, WAIT, Waited, ask, done, has_mark, main_view, offset_of, poll, project_window, put_cursor, root,
};
use super::{check, give_up, give_up_saying};
use crate::editor::EditorTextView;

const MAIN: &str = "src/main.rs";
const UTIL: &str = "src/util.rs";
/// Long enough for an alert to be up, which the app shows without a state to wait for.
const SETTLE: Duration = Duration::from_millis(400);

/// Sends `action` to `view`, with `sender` if given, as the menu bar or the keyboard does.
fn send(view: &EditorTextView, action: Sel, sender: Option<&objc2::runtime::AnyObject>) {
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: An action the editor answers, sent to it as a menu item or a key equivalent sends it.
    unsafe { app.sendAction_to_from(action, Some(view), sender) };
}

/// A mouse event of `kind` at UTF-16 offset `at` of `view`, or at the window's corner without one.
fn mouse(view: &EditorTextView, kind: NSEventType, at: Option<usize>) -> Option<Retained<NSEvent>> {
    let point = match at {
        Some(at) => {
            let rect = ide_ui_lsp::rect_of(view, at..at + 1)?;
            let center = NSPoint::new(
                rect.origin.x + rect.size.width / 2.0,
                rect.origin.y + rect.size.height / 2.0,
            );
            view.convertPoint_toView(center, None)
        }
        None => NSPoint::new(1.0, 1.0),
    };
    let window = view.window()?.windowNumber();
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind,
        point,
        NSEventModifierFlags::empty(),
        0.0,
        window,
        None,
        0,
        1,
        0.0,
    )
}

fn twice(view: &EditorTextView) -> Option<usize> {
    offset_of(view, "twice", 0, 2)
}

pub(crate) fn run() {
    poll(WAIT, has_mark("E0308"), problems);
}

fn problems(waited: Waited) {
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    put_cursor(&view, 0);
    send(&view, sel!(ideNextProblem:), None);
    let marks = ide_ui_lsp::marks_of(&*view);
    let at_mark = |view: &EditorTextView| {
        let cursor = view.selectedRange().location;
        marks.iter().any(|(range, _)| range.start == cursor)
    };
    let next = (at_mark(&view), ide_ui_lsp::hover_text(&*view));
    send(&view, sel!(idePreviousProblem:), None);
    check(
        "the next and the previous problem put the cursor on one and show its message",
        waited.ok && next.0 && next.1.is_some() && at_mark(&view),
        format!("{waited}: {next:?} {marks:?}"),
    );
    ide_ui_lsp::dismiss_hover(&*view);
    let (Some(at), Some(event)) = (twice(&view), mouse(&view, NSEventType::MouseMoved, twice(&view))) else {
        return give_up(SCENARIO, "main.rs calls twice");
    };
    put_cursor(&view, at);
    ide_ui_lsp::mouse_moved(&*view, &event);
    ask(
        || {
            if let Some(view) = main_view() {
                ide_ui_lsp::hover_rest(&*view);
            }
        },
        || {
            main_view()
                .and_then(|v| ide_ui_lsp::hover_text(&*v))
                .is_some_and(|t| t.contains("fn twice"))
        },
        hovered,
    );
}

fn hovered(waited: Waited) {
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    let shown = ide_ui_lsp::hover_text(&*view);
    if let Some(away) = mouse(&view, NSEventType::MouseMoved, None) {
        ide_ui_lsp::mouse_moved(&*view, &away);
    }
    check(
        "the mouse resting on a name shows its signature, and leaving closes it",
        waited.ok && ide_ui_lsp::hover_text(&*view).is_none(),
        format!("{waited}: {shown:?}"),
    );
    let menu = mouse(&view, NSEventType::RightMouseDown, twice(&view)).and_then(|event| view.menuForEvent(&event));
    let actions: Vec<(String, bool)> = menu
        .map(|menu| {
            menu.itemArray()
                .iter()
                .filter_map(|item| {
                    let aimed = item.target().is_some_and(|target| {
                        std::ptr::eq(
                            std::ptr::from_ref(&*target).cast::<u8>(),
                            std::ptr::from_ref(&**view).cast::<u8>(),
                        )
                    });
                    item.action()
                        .map(|action| (action.name().to_string_lossy().into_owned(), aimed))
                })
                .collect()
        })
        .unwrap_or_default();
    let names: Vec<&str> = actions.iter().map(|(name, _)| name.as_str()).collect();
    check(
        "the context menu starts with the places to go and offers formatting, all aimed at this editor",
        names.first() == Some(&"ideDefinition:")
            && names.contains(&"ideFormat:")
            && actions
                .iter()
                .filter(|(name, _)| name.starts_with("ide"))
                .all(|(_, aimed)| *aimed),
        format!("{names:?}"),
    );
    ask(
        || {
            if let Some(view) = main_view() {
                send(&view, sel!(ideSymbols:), None);
            }
        },
        || ide_ui_lsp::shown_symbols().is_some(),
        symbols,
    );
}

fn symbols(waited: Waited) {
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    let menu: Option<Retained<NSMenu>> = ide_ui_lsp::shown_symbols();
    let titles: Vec<String> = menu
        .iter()
        .flat_map(|menu| {
            menu.itemArray()
                .iter()
                .map(|item| item.title().to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    let main = menu.and_then(|menu| {
        menu.itemArray()
            .iter()
            .find(|item| item.title().to_string() == "ƒ main")
    });
    if let Some(item) = &main {
        send(&view, sel!(ideJumpToOffset:), Some(item.as_ref()));
    }
    let wanted = offset_of(&view, "main()", 0, 0);
    check(
        "Symbole lists the file's functions, and choosing one puts the cursor on its name",
        waited.ok && titles.iter().any(|t| t == "ƒ ugly") && Some(view.selectedRange().location) == wanted,
        format!(
            "{waited}: {titles:?}, cursor {} for {wanted:?}",
            view.selectedRange().location
        ),
    );
    let Some(at) = twice(&view) else {
        return give_up(SCENARIO, "main.rs calls twice");
    };
    ask(
        move || {
            if let Some(view) = main_view() {
                put_cursor(&view, at);
                send(&view, sel!(ideRename:), None);
            }
        },
        || project_window().is_some_and(|c| c.has_sheet()),
        rename_sheet,
    );
}

/// The editable field of the sheet in front.
fn field_in(view: &NSView) -> Option<Retained<NSTextField>> {
    if let Some(field) = view.downcast_ref::<NSTextField>()
        && field.isEditable()
    {
        return Some(objc2::Message::retain(field));
    }
    view.subviews().iter().find_map(|child| field_in(&child))
}

fn rename_sheet(waited: Waited) {
    let window = project_window().and_then(|c| c.window());
    let sheet = window.as_ref().and_then(|w| w.attachedSheet());
    let field = sheet.as_ref().and_then(|s| s.contentView()).and_then(|v| field_in(&v));
    let offered = field.as_ref().map(|f| f.stringValue().to_string());
    if let (Some(window), Some(sheet), Some(field)) = (&window, &sheet, &field) {
        field.setStringValue(&crate::util::ns("thrice"));
        window.endSheet_returnCode(sheet, NSAlertFirstButtonReturn);
    }
    poll(
        ANSWER,
        || std::fs::read_to_string(root().join(UTIL)).is_ok_and(|disk| disk.contains("pub fn thrice")),
        move |renamed| {
            check(
                "the rename sheet offers the current name, and Umbenennen renames in every file",
                waited.ok && renamed.ok && offered.as_deref() == Some("twice"),
                format!("{waited}, {renamed}: offered {offered:?}"),
            );
            format_on_save();
        },
    );
}

fn set_format_on_save(on: bool) {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::FORMAT_ON_SAVE, &on) {
        check("the setting is written", false, error.to_string());
    }
}

/// With "Beim Sichern formatieren" on, ⌘S formats first and then saves.
fn format_on_save() {
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    set_format_on_save(true);
    let end = view.document().text().len_utf16();
    ide_ui_lsp::apply_text_edits(&*view, vec![(end..end, "fn  late( )  { }\n".to_string())]);
    let late = || main_view().is_some_and(|v| v.document().text().to_string().contains("fn  late( )"));
    wait_until(late, |_| {
        ask(
            || {
                if let Some(view) = main_view() {
                    send(&view, sel!(ideSave:), None);
                }
            },
            || {
                let disk = std::fs::read_to_string(root().join(MAIN)).unwrap_or_default();
                disk.contains("fn late() {}") && main_view().is_some_and(|v| !v.document().is_dirty())
            },
            saved,
        );
    });
}

fn saved(waited: Waited) {
    set_format_on_save(false);
    let disk = std::fs::read_to_string(root().join(MAIN)).unwrap_or_default();
    check(
        "saving with formatting on writes the formatted text",
        waited.ok,
        format!("{waited}: {}", disk.lines().last().unwrap_or_default()),
    );
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    let Some(at) = offset_of(&view, "\"text\"", 0, 1) else {
        return give_up(SCENARIO, "main.rs holds the string literal to fix");
    };
    ide_ui_lsp::code_action_menu::forget();
    ask(
        move || {
            if let Some(view) = main_view() {
                put_cursor(&view, at);
                send(&view, sel!(ideQuickFix:), None);
            }
        },
        || {
            main_view()
                .and_then(|v| ide_ui_lsp::code_action_menu::shown(&*v))
                .is_some_and(|titles| titles.first().is_some_and(|first| first != t("Keine Aktionen")))
        },
        bulb,
    );
}

/// The bulb on the line of the type error opens its fixes as well.
fn bulb(waited: Waited) {
    let menu = main_view().and_then(|v| ide_ui_lsp::code_action_menu::shown(&*v));
    check(
        "Schnelle Lösung lists the fixes of a type error",
        waited.ok,
        format!("{waited}: {menu:?}"),
    );
    ide_ui_lsp::code_action_menu::forget();
    poll(
        ANSWER,
        || main_view().is_some_and(|v| ide_ui_lsp::code_action_menu::bulb(&*v).is_some()),
        |shown| {
            let pressed = main_view().is_some_and(|v| ide_ui_lsp::code_action_menu::press_bulb(&*v));
            check(
                "the bulb shows on the line with fixes",
                shown.ok && pressed,
                shown.to_string(),
            );
            poll(
                ANSWER,
                || {
                    main_view()
                        .and_then(|v| ide_ui_lsp::code_action_menu::shown(&*v))
                        .is_some_and(|titles| titles.first().is_some_and(|first| first != t("Keine Aktionen")))
                },
                stale_action,
            );
        },
    );
}

/// A code action chosen after the text changed is not applied, and the user is told.
fn stale_action(waited: Waited) {
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    let titles = ide_ui_lsp::code_action_menu::shown(&*view).unwrap_or_default();
    let Some(first) = titles.first().cloned() else {
        return give_up_saying(
            SCENARIO,
            "the quick fixes of a type error are listed",
            waited.to_string(),
        );
    };
    let before = view.document().text().to_string();
    let end = view.document().text().len_utf16();
    ide_ui_lsp::apply_text_edits(&*view, vec![(end..end, "\n".to_string())]);
    let told = answer_alert_saying(t("Änderung nicht angewandt").to_string(), NSAlertFirstButtonReturn);
    let chosen = ide_ui_lsp::code_action_menu::click(&first);
    in_any_mode_after(SETTLE * 3, move || {
        let unchanged = main_view().is_some_and(|v| v.document().text().to_string() == format!("{before}\n"));
        check(
            "a code action chosen after the text changed is refused with a word, and nothing else changes",
            chosen && told.get() && unchanged,
            format!("{waited}: chose {first:?} {chosen}, told {}", told.get()),
        );
        commands();
    });
}

/// Puts the cursor of main.rs on `needle`, `into` code units in, and sends `action` there.
fn at_main(needle: &'static str, into: usize, action: Sel) -> impl FnOnce() + 'static {
    move || {
        if let Some(view) = main_view()
            && let Some(at) = offset_of(&view, needle, 0, into)
        {
            put_cursor(&view, at);
            send(&view, action, None);
        }
    }
}

/// The commands of the menu bar that ask the server: hover, definition, uses and parameter help.
fn commands() {
    ide_ui_lsp::code_action_menu::forget();
    ask(
        at_main("thrice", 2, sel!(ideHover:)),
        || {
            main_view()
                .and_then(|v| ide_ui_lsp::hover_text(&*v))
                .is_some_and(|t| t.contains("fn thrice"))
        },
        |hover| {
            check(
                "Info zeigen shows the signature at the cursor",
                hover.ok,
                hover.to_string(),
            );
            ask(
                at_main("thrice", 2, sel!(ideDefinition:)),
                || {
                    project_window().is_some_and(|c| {
                        crate::editor::editor_for(&c, &root().join(UTIL))
                            .is_some_and(|v| v.selectedRange().location == 7)
                    })
                },
                defined,
            );
        },
    );
}

fn defined(waited: Waited) {
    check(
        "Gehe zur Definition opens util.rs at the function",
        waited.ok,
        waited.to_string(),
    );
    if let Some(controller) = project_window() {
        controller.open_file(&root().join(MAIN));
    }
    ask(
        at_main("thrice", 2, sel!(ideReferences:)),
        || !ide_ui_lsp::listed().is_empty(),
        |uses| {
            let listed = ide_ui_lsp::listed();
            ide_ui_lsp::close_list();
            check(
                "Verwendungen finden lists the uses",
                uses.ok && listed.len() >= 2,
                format!("{uses}: {listed:?}"),
            );
            ask(
                at_main("thrice(21", 7, sel!(ideSignatureHelp:)),
                || main_view().is_some_and(|v| ide_ui_lsp::signatures::shown(&*v).is_some()),
                |help| {
                    let shown = main_view().and_then(|v| ide_ui_lsp::signatures::shown(&*v));
                    check(
                        "Parameterhilfe shows the signature of the call around the cursor",
                        help.ok && shown.as_ref().is_some_and(|(label, _)| label.contains("thrice")),
                        format!("{help}: {shown:?}"),
                    );
                    refactor();
                },
            );
        },
    );
}

/// Refactorings and completion from the menu bar: their menus and list open at the cursor.
fn refactor() {
    ide_ui_lsp::code_action_menu::forget();
    ask(
        at_main("thrice(21", 2, sel!(ideRefactor:)),
        || main_view().is_some_and(|v| ide_ui_lsp::code_action_menu::shown(&*v).is_some()),
        |refactor| {
            let titles = main_view().and_then(|v| ide_ui_lsp::code_action_menu::shown(&*v));
            check(
                "Refaktorisieren opens the refactorings at the cursor",
                refactor.ok && titles.as_ref().is_some_and(|t| !t.is_empty()),
                format!("{refactor}: {titles:?}"),
            );
            ide_ui_lsp::code_action_menu::forget();
            ask(
                at_main("util::thrice(21", 6, sel!(ideComplete:)),
                || main_view().is_some_and(|v| !ide_ui_lsp::completion::shown(&*v).is_empty()),
                |listed| {
                    let shown = main_view()
                        .map(|v| ide_ui_lsp::completion::shown(&*v))
                        .unwrap_or_default();
                    check(
                        "Vervollständigen lists what the module offers",
                        listed.ok && shown.iter().any(|label| label.starts_with("thrice")),
                        format!("{listed}: {shown:?}"),
                    );
                    custom_servers();
                    if let Some(view) = main_view() {
                        use objc2_app_kit::NSStandardKeyBindingResponding;
                        let text_view: &objc2_app_kit::NSTextView = &view;
                        // SAFETY: Esc's selector of the standard key bindings, which closes the list as the key does.
                        unsafe { text_view.doCommandBySelector(sel!(cancelOperation:)) };
                    }
                },
            );
        },
    );
}

fn set_servers(servers: Vec<ide_lsp::CustomServer>) {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&ide_ui_lsp::LANGUAGE_SERVERS, &servers) {
        check("the language servers are written", false, error.to_string());
    }
    ide_ui_lsp::servers_changed();
}

fn editor_of(file: &str) -> Option<Retained<EditorTextView>> {
    crate::editor::editor_for(&*project_window()?, &root().join(file))
}

fn server_line(file: &str) -> String {
    editor_of(file)
        .map(|v| ide_ui_lsp::status::shown(&*v).1)
        .unwrap_or_default()
}

/// A server of the user's that keeps crashing says so in the status bar, whose button starts it again.
fn custom_servers() {
    set_servers(vec![ide_lsp::CustomServer {
        name: "Crash".into(),
        extensions: vec!["crash".into()],
        command: "/usr/bin/false".into(),
    }]);
    let path = root().join("x.crash");
    let written = std::fs::write(&path, "text\n").is_ok();
    if let Some(controller) = project_window() {
        controller.open_file(&path);
    }
    let crashed = splat_i18n::tf("{} ist wiederholt abgestürzt", &["false"]);
    let wanted = crashed.clone();
    poll(
        WAIT,
        move || server_line("x.crash") == wanted,
        move |waited| {
            let crash = server_line("x.crash");
            if let Some(view) = editor_of("x.crash") {
                ide_ui_lsp::status_action(&*view);
            }
            let restarted = server_line("x.crash");
            check(
                "a server of the user's that keeps crashing says so and its button starts it again",
                written && waited.ok && crash == crashed && restarted != crashed,
                format!("{waited}: {crash:?} → {restarted:?}"),
            );
            set_servers(Vec::new());
            done();
        },
    );
}
