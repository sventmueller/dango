//! The `snappy-preview-markdown` scenario: typing into 10 000 lines of Markdown while its preview is open holds the
//! main thread no longer than the same typing with the preview closed: a word at the top of the file and in its
//! middle, and lines at the bottom of the editor, which scroll it and the preview along. The preview shows what was
//! typed without leaving the place it stands at.

use std::path::PathBuf;
use std::time::Duration;

use ide_ui_common::util::stall;
use objc2::rc::Retained;
use objc2_foundation::NSRange;

use super::snappy::measure;
use super::snappy_recolor::{delete, no_longer};
use super::snappy_recolor_markdown::markdown;
use super::{check, finish, helpers};
use crate::editor::{self, EditorTextView};
use crate::preview::seen;

const SCENARIO: &str = "snappy-preview-markdown";
const PROJECT: &str = "snappy-preview-markdown-project";
const FILE: &str = "big.md";
const LINES_PER_SECTION: usize = 5;
/// The line of a section with the paragraph typed into.
const TEXT_LINE: usize = 2;
const LAST_SECTION: &str = "Section 1999";
const SETTLE: Duration = Duration::from_millis(1500);
/// How long the work typing starts may go on, the preview's pause before it lays the text out included.
const KEYSTROKE: Duration = Duration::from_millis(800);
/// How long the first layout of 10 000 lines may take on a busy Mac.
const SHOWN_WITHIN: Duration = Duration::from_secs(30);

/// What is typed.
#[derive(Clone, Copy)]
enum Typing {
    /// Letters inside a word of a paragraph on screen.
    Word,
    /// Lines behind the last line on screen, which scroll the editor.
    Lines,
}

impl Typing {
    fn text(self) -> &'static str {
        match self {
            Typing::Word => "xyzw",
            Typing::Lines => "\nA new line\nAnother new line",
        }
    }

    /// What the preview shows of it, which nothing else in the file holds.
    fn shown(self) -> &'static str {
        match self {
            Typing::Word => "Texyzwxt",
            Typing::Lines => "Another new line",
        }
    }

    /// Where it is typed in what `view` shows.
    fn at(self, view: &EditorTextView) -> Option<usize> {
        let lines = editor::visible_line_range(view);
        match self {
            Typing::Word => {
                let heading = (lines.start / LINES_PER_SECTION + 3) * LINES_PER_SECTION;
                view.line_start(heading + TEXT_LINE).map(|start| start + 2)
            }
            Typing::Lines => view.line_start(lines.end.checked_sub(1)?).map(|start| start - 1),
        }
    }
}

/// How long the main thread was held up by each typing with the preview closed.
#[derive(Clone, Copy)]
struct Plain {
    word: Duration,
    lines: Duration,
}

impl Plain {
    fn of(self, typing: Typing) -> Duration {
        match typing {
            Typing::Word => self.word,
            Typing::Lines => self.lines,
        }
    }
}

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn path() -> PathBuf {
    root().join(FILE)
}

fn big() -> Option<Retained<EditorTextView>> {
    let controller = helpers::window_of(&root())?;
    editor::editor_for(&controller, &path())
}

fn settled(view: &EditorTextView) -> bool {
    view.document().is_highlighted() && editor::colours_painted(view)
}

/// The number of the section a line of the preview belongs to.
fn section(line: &str) -> Option<usize> {
    line.split(|c: char| !c.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

/// Types `typing` where it goes on screen; where it went.
fn insert(view: &EditorTextView, typing: Typing) -> Option<NSRange> {
    let at = typing.at(view)?;
    view.setSelectedRange(NSRange::new(at, 0));
    editor::type_text(view, typing.text());
    helpers::post_event();
    Some(NSRange::new(at, typing.text().len()))
}

fn top(view: &EditorTextView) -> f64 {
    view.visibleRect().origin.y
}

pub fn run() {
    let text = markdown();
    helpers::open_project(SCENARIO, PROJECT, &[(FILE, &text)], |_| {
        helpers::wait_for_editor(&root(), &path(), |view| {
            let Some(view) = view else {
                check("the Markdown file of 10 000 lines opens", false, "");
                return finish(SCENARIO);
            };
            helpers::wait_within(
                SHOWN_WITHIN,
                move || settled(&view) && seen::shows(&path(), LAST_SECTION),
                |shown| {
                    check("the preview that opens with the file shows all of it", shown, "");
                    let closed = big().is_some_and(|view| crate::preview::close_beside(&view, false));
                    check("the preview closes for typing without it", closed, "");
                    helpers::after(SETTLE, || {
                        plain(Typing::Word, |word| {
                            plain(Typing::Lines, move |lines| reopen(Plain { word, lines }));
                        });
                    });
                },
            );
        });
    });
}

/// Types `typing` with the preview closed and hands on how long it held up the main thread.
fn plain(typing: Typing, then: impl FnOnce(Duration) + 'static) {
    let Some(view) = big() else { return finish(SCENARIO) };
    measure();
    let typed = insert(&view, typing);
    helpers::after(KEYSTROKE, move || {
        let took = stall::take_worst();
        if let (Some(view), Some(typed)) = (big(), typed) {
            delete(&view, typed);
        }
        helpers::after(SETTLE, move || then(took));
    });
}

fn reopen(plain: Plain) {
    if let Some(view) = big() {
        view.scrollRangeToVisible(NSRange::new(0, 0));
        crate::preview::open_beside(&view, false);
    }
    helpers::wait_within(
        SHOWN_WITHIN,
        || seen::shows(&path(), LAST_SECTION),
        move |shown| {
            check("the preview opens again and shows all of the file", shown, "");
            helpers::after(SETTLE, move || {
                typed_into(
                    Typing::Word,
                    plain,
                    "typing at the top of 10 000 lines of Markdown with the preview open holds the main thread no \
                     longer than without it",
                    scroll_to_middle,
                );
            });
        },
    );
}

fn scroll_to_middle(plain: Plain) {
    let Some(view) = big() else { return finish(SCENARIO) };
    let middle = view.line_start(view.document().text().line_count() / 2).unwrap_or(0);
    view.scrollRangeToVisible(NSRange::new(middle, 0));
    view.displayIfNeeded();
    helpers::after(SETTLE, move || {
        seen::settle(&path());
        helpers::after(SETTLE, move || type_in_middle(plain));
    });
}

fn type_in_middle(plain: Plain) {
    let before = seen::top_line(&path());
    check(
        "the preview follows the editor to the middle of the file",
        before
            .as_deref()
            .and_then(section)
            .is_some_and(|n| (900..1100).contains(&n)),
        format!("{before:?}"),
    );
    typed_into(
        Typing::Word,
        plain,
        "typing in the middle of 10 000 lines of Markdown with the preview open holds the main thread no longer than \
         without it",
        move |plain| {
            let after = seen::top_line(&path());
            check(
                "the preview stays where it stood while it shows the typed text",
                after.is_some() && after == before,
                format!("before {before:?}, after {after:?}"),
            );
            type_lines(plain);
        },
    );
}

fn type_lines(plain: Plain) {
    let Some(view) = big() else { return finish(SCENARIO) };
    let (editor_before, preview_before) = (top(&view), seen::scrolled(&path()));
    typed_into(
        Typing::Lines,
        plain,
        "typing lines at the bottom of the editor with the preview open, which scrolls both, holds the main thread no \
         longer than without the preview",
        move |_| {
            let preview_after = seen::scrolled(&path());
            check(
                "the preview scrolls with the editor the lines scrolled",
                preview_after
                    .zip(preview_before)
                    .is_some_and(|(after, before)| after > before),
                format!("{preview_before:?} → {preview_after:?}"),
            );
            finish(SCENARIO);
        },
    );
    let editor_after = big().map(|v| top(&v));
    check(
        "the lines typed at the bottom scroll the editor",
        editor_after.is_some_and(|after| after > editor_before),
        format!("{editor_before} → {editor_after:?}"),
    );
}

/// Types `typing` on screen, waits for the preview to show it and for the work it started to end, checks the longest
/// main thread step against the same typing without the preview, deletes the typing again and goes on with `then`.
fn typed_into(typing: Typing, plain: Plain, name: &'static str, then: impl FnOnce(Plain) + 'static) {
    let Some(view) = big() else { return finish(SCENARIO) };
    measure();
    let typed = insert(&view, typing);
    helpers::wait_within(
        SHOWN_WITHIN,
        move || seen::shows(&path(), typing.shown()),
        move |shown| {
            helpers::after(KEYSTROKE, move || {
                no_longer(name, shown, plain.of(typing));
                if let (Some(view), Some(typed)) = (big(), typed) {
                    delete(&view, typed);
                }
                helpers::wait_within(
                    SHOWN_WITHIN,
                    move || !seen::shows(&path(), typing.shown()),
                    move |gone| {
                        check("the preview no longer shows the typing once it is deleted", gone, "");
                        helpers::after(SETTLE, move || then(plain));
                    },
                );
            });
        },
    );
}
