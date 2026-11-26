//! The suggestions and the input field: a command for the history, its suggestion at the prompt, a switch of the
//! Terminal settings page that takes the suggestion away at once, and the field from the View menu with the
//! history's suggestion, the completion menu, the command's colours and Return.

use objc2::sel;
use splat_i18n::t;

use super::super::{check, snapshot};
use super::{
    Run, after_prompt, command, empty_line, find_view, has_line, inspect, pane_of, press_on, screen, stop, wait,
};

/// The command the history keeps, and what of it the suggestions leave to type.
const RECORDED: &str = "echo tapa-history-word";
const TYPED: &str = "echo tapa-h";
const SUGGESTED: &str = "istory-word";
/// A line in the field whose words are of every part the colours tell apart, the last one no command at all.
const COLOURED: &str = "git status --short | tapa-no-such-command";

/// Runs a command, which the history keeps for the suggestions after it.
pub(super) fn record(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let before = view.prompts();
    view.type_text(&format!("{RECORDED}\r"));
    after_prompt(run, before, raw_suggestion);
}

/// What the view suggests without the field, as its hook prints it: the suggestion first, `None` for none.
fn raw(run: &Run) -> String {
    inspect::view(run.id)
        .map(|view| view.raw_suggestion())
        .unwrap_or_default()
}

fn suggests(run: &Run) -> bool {
    raw(run).starts_with(&format!("Some({SUGGESTED:?})"))
}

/// Without the field, the line typed at the prompt gets the history's suggestion after the cursor.
fn raw_suggestion(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    view.type_text(TYPED);
    let shown = run.clone();
    wait(
        &run.clone(),
        10,
        move |_| suggests(&shown),
        move |suggested| {
            check(
                "without the input field, the history's suggestion for the line typed at the prompt shows after the \
                 cursor",
                suggested,
                format!("{} | {}", raw(&run), screen(run.id)),
            );
            suggestions_off(run);
        },
    );
}

/// The switch of the settings page acts on the open terminal at once, off and on again.
fn suggestions_off(run: Run) {
    let found = super::switch(sel!(ideToggleTerminalSuggestions:), false);
    let off = run.clone();
    wait(
        &run.clone(),
        5,
        move |_| raw(&off).starts_with("None"),
        move |gone| {
            let without = raw(&run);
            let again = super::switch(sel!(ideToggleTerminalSuggestions:), true);
            let on = run.clone();
            wait(
                &run.clone(),
                5,
                move |_| suggests(&on),
                move |back| {
                    check(
                        "switching suggestions off on the Terminal settings page takes the open terminal's suggestion \
                         away at once, and switching them on brings it back",
                        found && again && gone && back,
                        format!("off: {without}; on: {}", raw(&run)),
                    );
                    if let Some(view) = inspect::view(run.id) {
                        empty_line(&view);
                    }
                    let typed = |view: &crate::terminal::TerminalView| {
                        view.with_session(|session| session.typed_line())
                            .flatten()
                            .is_none_or(|(line, _)| line.text.is_empty())
                    };
                    wait(&run.clone(), 5, typed, move |_| show_field(run));
                },
            );
        },
    );
}

/// The View menu's command shows the input field, which takes the keyboard.
fn show_field(run: Run) {
    let chosen = command(&run, t("Eingabefeld ein-/ausblenden"));
    wait(
        &run.clone(),
        5,
        |view| view.field_shown(),
        move |shown| {
            let focused = inspect::view(run.id).is_some_and(|view| field_has_keys(&view));
            let stored = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::TERMINAL_INPUT_FIELD);
            check(
                "the View menu's command shows the input field at the prompt with the keyboard in it",
                chosen && shown && focused,
                format!("chosen {chosen}, shown {shown}, focused {focused}; new terminals with the field {stored}"),
            );
            let switch = inspect::view(run.id)
                .and_then(|view| view.window())
                .map(|window| crate::window::toolbar::field_switch(&window))
                .unwrap_or_default();
            check(
                "with the terminal in front the toolbar's input field switch is enabled and shows the field on",
                switch == format!("{} (enabled, accent)", t("Eingabefeld an")),
                switch,
            );
            field_suggestion(run);
        },
    );
}

/// Whether the text view of the input field of `view` has the keyboard.
fn field_has_keys(view: &crate::terminal::TerminalView) -> bool {
    let (Some(field), Some(window)) = (field_text(view), view.window()) else {
        return false;
    };
    window.firstResponder().is_some_and(|responder| {
        let responder: &objc2::runtime::AnyObject = &responder;
        let field: &objc2::runtime::AnyObject = &field;
        std::ptr::eq(responder, field)
    })
}

/// The text view of the input field of `view`.
fn field_text(view: &crate::terminal::TerminalView) -> Option<objc2::rc::Retained<objc2_app_kit::NSView>> {
    let pane = pane_of(view)?;
    find_view(&pane, &|candidate| {
        candidate.class().name().to_bytes() == b"TermInputFieldText"
    })
}

/// Text typed into the field gets the history's suggestion after it.
fn field_suggestion(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    view.type_into_field(TYPED);
    wait(
        &run.clone(),
        5,
        |view| view.field_contents() == (TYPED.to_string(), SUGGESTED.to_string()),
        move |suggested| {
            let contents = inspect::view(run.id).map(|view| view.field_contents());
            check(
                "text typed into the input field shows the history's suggestion after it",
                suggested,
                format!("{contents:?}"),
            );
            completion_menu(run);
        },
    );
}

/// Tab opens the completion menu, ↓ moves through it and Escape closes it.
fn completion_menu(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    term_suggest::warm_spec("git");
    let (folder, home) = (view.cwd(), term_base::home());
    let parsed = move |_: &crate::terminal::TerminalView| {
        term_suggest::complete("git che", &folder, &home, false)
            .candidates
            .len()
            >= 2
    };
    wait(&run.clone(), 10, parsed, move |parsed| {
        let Some(view) = inspect::view(run.id) else {
            return stop(&run, "the terminal is open");
        };
        view.type_into_field("git che");
        view.menu_drive(0);
        let opened = view.menu_entries().unwrap_or_default();
        view.menu_drive(1);
        let moved = view.menu_entries().unwrap_or_default();
        if let Some(window) = view.window() {
            snapshot(&window, "terminal-tapa-menu");
        }
        if let Some(field) = field_text(&view) {
            press_on(&field, 53, "\u{1b}");
        }
        let closed = view.menu_entries().is_none();
        let listed = ["checkout", "cherry-pick"]
            .iter()
            .all(|name| opened.iter().any(|entry| entry == name));
        check(
            "Tab in the input field opens the completion menu with the spec's candidates, ↓ moves through it and \
             Escape closes it",
            parsed && listed && moved.len() == opened.len() && moved.first() != opened.first() && closed,
            format!("{opened:?}, then {moved:?}, closed {closed}"),
        );
        colours(run);
    });
}

/// The field tells the parts of its command apart and marks a command the shell does not know.
fn colours(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    view.type_into_field(COLOURED);
    let expected = [
        "git:Command",
        "status:Subcommand",
        "--short:Option",
        "|:Operator",
        "tapa-no-such-command:Unknown",
    ];
    wait(
        &run.clone(),
        10,
        move |view| {
            let parts = view.field_parts();
            expected.iter().all(|part| parts.iter().any(|shown| shown == part))
        },
        move |coloured| {
            let parts = inspect::view(run.id).map(|view| view.field_parts()).unwrap_or_default();
            check(
                "the input field colours the command by its parts and marks a command the shell does not know",
                coloured,
                parts.join(" "),
            );
            submit(run);
        },
    );
}

/// Return in the field runs its command in the shell and empties the field.
fn submit(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let before = view.prompts();
    view.type_into_field("echo tapa-from-field");
    let pressed = field_text(&view).map(|field| press_on(&field, 36, "\r")).is_some();
    let done = move |view: &crate::terminal::TerminalView| {
        has_line(view, "tapa-from-field") && view.prompts() > before && view.field_shown()
    };
    wait(&run.clone(), 10, done, move |ran| {
        let contents = inspect::view(run.id).map(|view| view.field_contents());
        check(
            "Return in the input field runs its command in the shell and empties the field",
            pressed && ran && contents.as_ref().is_some_and(|(text, _)| text.is_empty()),
            format!("{contents:?} | {}", screen(run.id)),
        );
        hide_field(run);
    });
}

/// The View menu's command hides the field again, and the keyboard goes back to the terminal.
fn hide_field(run: Run) {
    let chosen = command(&run, t("Eingabefeld ein-/ausblenden"));
    wait(
        &run.clone(),
        5,
        |view| !view.field_shown(),
        move |hidden| {
            let back = inspect::view(run.id).is_some_and(|view| {
                view.window()
                    .and_then(|window| window.firstResponder())
                    .is_some_and(|responder| {
                        let responder: &objc2::runtime::AnyObject = &responder;
                        let own: &objc2::runtime::AnyObject = &view;
                        std::ptr::eq(responder, own)
                    })
            });
            check(
                "the View menu's command hides the input field again and gives the keyboard back to the terminal",
                chosen && hidden && back,
                format!("chosen {chosen}, hidden {hidden}, terminal has the keyboard {back}"),
            );
            super::blocks::switch_on(run);
        },
    );
}
