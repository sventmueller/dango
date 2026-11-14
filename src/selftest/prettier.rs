//! The `prettier-save` scenario (E133): in a trusted project with a Prettier configuration, ⌘S formats a JavaScript
//! file with the project's own `node_modules/.bin/prettier`, run in that folder, then saves; an answer for a text that
//! changed meanwhile is dropped and the file saved as it is, and so is a file Prettier fails on, both with a note in the
//! status bar. The program is a stand-in script, so no Node and no network are needed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::settings::{Settings, keys};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;
use objc2_foundation::NSRange;

use super::helpers::{self, wait_until, wait_within};
use super::{check, finish};
use crate::editor::{self, EditorTextView};
use crate::window::{self, WindowController};

const SCENARIO: &str = "prettier-save";
const PROJECT: &str = "prettier-project";
const MESSY: &str = "let  a = 1;;\n";
const TIDY: &str = "let a = 1;\n";
/// Formats by squeezing spaces and semicolons; takes a second for `slow.js` and fails like Prettier on `bad.js`. It
/// writes the folder it runs in next to it, so the scenario sees where it ran.
const STAND_IN: &str = "#!/bin/sh\n\
    pwd > \"$(dirname \"$0\")/ran-in\"\n\
    case \"$2\" in\n\
      *bad.js) cat > /dev/null; echo '[error] bad.js: SyntaxError: Unexpected token (1:5)' >&2; exit 2;;\n\
      *slow.js) sleep 1;;\n\
    esac\n\
    sed -e 's/  */ /g' -e 's/;;*/;/g'\n";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn view(controller: &WindowController, file: &str) -> Option<Retained<EditorTextView>> {
    editor::editor_for(controller, &root().join(file))
}

fn disk(file: &str) -> String {
    std::fs::read_to_string(root().join(file)).unwrap_or_default()
}

fn save(view: &EditorTextView) {
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: An action the editor answers, sent to it with no sender, as ⌘S sends it.
    unsafe { app.sendAction_to_from(objc2::sel!(ideSave:), Some(view), None) };
}

pub fn run() {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::FORMAT_ON_SAVE, &true) {
        check("format on save is turned on", false, error.to_string());
        return finish(SCENARIO);
    }
    helpers::open_project(
        SCENARIO,
        PROJECT,
        &[
            (".prettierrc", "{}\n"),
            ("web/node_modules/.bin/prettier", STAND_IN),
            ("web/src/a.js", MESSY),
            ("web/src/slow.js", MESSY),
            ("web/src/bad.js", MESSY),
            ("notes.rs", MESSY),
        ],
        |controller| {
            let program = root().join("web/node_modules/.bin/prettier");
            let made = std::fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755));
            if let Err(error) = made {
                check("the stand-in is executable", false, error.to_string());
                return finish(SCENARIO);
            }
            formats(controller);
        },
    );
}

fn opened(controller: &WindowController, file: &str, then: impl FnOnce(Retained<EditorTextView>) + 'static) {
    controller.open_file(&root().join(file));
    let controller = objc2::Message::retain(controller);
    let file = file.to_string();
    let seen = controller.clone();
    let wanted = file.clone();
    wait_until(
        move || view(&seen, &wanted).is_some(),
        move |_| match view(&controller, &file) {
            Some(view) => then(view),
            None => {
                check("the file opens in an editor", false, file);
                finish(SCENARIO);
            }
        },
    );
}

fn formats(controller: Retained<WindowController>) {
    let next = controller.clone();
    opened(&controller, "web/src/a.js", move |view| {
        save(&view);
        let watched = view.clone();
        wait_within(
            Duration::from_secs(10),
            move || disk("web/src/a.js") == TIDY && !watched.document().is_dirty(),
            move |saved| {
                let ran_in = std::fs::read_to_string(root().join("web/node_modules/.bin/ran-in")).unwrap_or_default();
                check(
                    "⌘S formats with the project's own Prettier, run in the folder of its node_modules, then saves",
                    saved
                        && view.document().text().to_string() == TIDY
                        && !view.document().is_dirty()
                        && Path::new(ran_in.trim()).ends_with("web"),
                    format!("{:?} / ran in {ran_in:?}", disk("web/src/a.js")),
                );
                changed_meanwhile(next);
            },
        );
    });
}

fn changed_meanwhile(controller: Retained<WindowController>) {
    let next = controller.clone();
    opened(&controller, "web/src/slow.js", move |view| {
        save(&view);
        let end = view.document().text().len_utf16();
        view.setSelectedRange(NSRange::new(end, 0));
        editor::type_text(&view, "//");
        wait_within(
            Duration::from_secs(10),
            || disk("web/src/slow.js").ends_with("//"),
            move |saved| {
                let note = crate::lsp::status_note(&view).unwrap_or_default();
                check(
                    "an answer for a text that changed meanwhile is dropped: saved as it is, with a note",
                    saved && disk("web/src/slow.js") == format!("{MESSY}//") && !note.is_empty(),
                    format!("{:?} / {note:?}", disk("web/src/slow.js")),
                );
                fails(next);
            },
        );
    });
}

fn fails(controller: Retained<WindowController>) {
    let next = controller.clone();
    opened(&controller, "web/src/bad.js", move |view| {
        let end = view.document().text().len_utf16();
        view.setSelectedRange(NSRange::new(end, 0));
        editor::type_text(&view, "x");
        save(&view);
        wait_within(
            Duration::from_secs(10),
            || disk("web/src/bad.js").ends_with('x'),
            move |saved| {
                let note = crate::lsp::status_note(&view).unwrap_or_default();
                check(
                    "a file Prettier fails on is saved as it is, and the status bar says why",
                    saved && disk("web/src/bad.js") == format!("{MESSY}x") && note.contains("SyntaxError"),
                    format!("{:?} / {note:?}", disk("web/src/bad.js")),
                );
                other_language(next);
            },
        );
    });
}

/// A Rust file is not Prettier's: the language server formats it, and without one ⌘S just saves.
fn other_language(controller: Retained<WindowController>) {
    opened(&controller, "notes.rs", move |view| {
        let end = view.document().text().len_utf16();
        view.setSelectedRange(NSRange::new(end, 0));
        editor::type_text(&view, "\n");
        save(&view);
        wait_within(
            Duration::from_secs(5),
            || disk("notes.rs") == format!("{MESSY}\n"),
            |saved| {
                check(
                    "a file Prettier does not know is not handed to it",
                    saved,
                    disk("notes.rs"),
                );
                let _ = window::for_project(&root());
                finish(SCENARIO);
            },
        );
    });
}
