//! The `snappy-recolor` scenario: `/*` typed at the top of 10 000 lines turns every line below into a comment and
//! deleting it turns them back, and the colouring holds the main thread no longer than a keystroke that colours no
//! other line; scrolled to the end meanwhile, the lines on screen show the comment's colour. Measured with the stall
//! detector as `snappy-tree` is.

use std::path::PathBuf;
use std::time::Duration;

use ide_ui_common::util::stall;
use objc2::rc::Retained;
use objc2_foundation::NSRange;
use splat_markdown::code::CodeToken;

use super::snappy::{BUDGET, measure};
use super::{check, finish, give_up, helpers};
use crate::editor::{self, EditorTextView};

const SCENARIO: &str = "snappy-recolor";
const PROJECT: &str = "snappy-recolor-project";
const FILE: &str = "big.js";
const FUNCTIONS: usize = 2000;
/// How much longer than a plain keystroke's the longest main thread step of a recolouring may take: half a frame at
/// 60 Hz.
const EXTRA: Duration = Duration::from_millis(8);
/// How long the app may settle after opening the file before a measured part starts.
const SETTLE: Duration = Duration::from_millis(1500);
/// How long a plain keystroke's work may take to end.
const KEYSTROKE: Duration = Duration::from_millis(500);
/// How long colouring 10 000 lines may take in slices on a busy Mac.
const COLOURED: Duration = Duration::from_secs(20);

/// Ten thousand lines of small functions.
fn code() -> String {
    (0..FUNCTIONS)
        .map(|n| format!("function f{n}(x) {{\n  // f{n}\n  const y = x + {n};\n  return y;\n}}\n"))
        .collect()
}

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn big() -> Option<Retained<EditorTextView>> {
    let controller = helpers::window_of(&root())?;
    editor::editor_for(&controller, &root().join(FILE))
}

/// Whether the word `const` in the lines on screen is all in the colour of `token`.
fn screen_coloured(view: &EditorTextView, token: CodeToken) -> bool {
    let lines = editor::visible_line_range(view);
    let mut consts = lines.filter(|line| line % 5 == 2).peekable();
    consts.peek().is_some()
        && consts.all(|line| {
            view.line_start(line)
                .is_some_and(|start| editor::coloured_as(view, start + 2, token))
        })
}

fn settled(view: &EditorTextView) -> bool {
    view.document().is_highlighted() && editor::colours_painted(view)
}

pub(super) fn delete(view: &EditorTextView, range: NSRange) {
    use objc2_app_kit::NSStandardKeyBindingResponding;
    view.setSelectedRange(range);
    let text_view: &objc2_app_kit::NSTextView = view;
    // SAFETY: A selector of the standard key bindings, which the text view answers, as a key press sends it.
    unsafe { text_view.doCommandBySelector(objc2::sel!(deleteBackward:)) };
}

/// Checks that the longest main thread step since `measure` took at most `EXTRA` more than a plain keystroke's.
pub(super) fn no_longer(name: &'static str, done: bool, plain: Duration) {
    let worst = stall::take_worst();
    check(
        name,
        done && worst <= plain + EXTRA && worst < BUDGET,
        format!(
            "coloured through: {done}, the longest step took {} ms, a plain keystroke's {} ms",
            worst.as_millis(),
            plain.as_millis()
        ),
    );
}

pub(crate) fn run() {
    let code = code();
    helpers::open_project(SCENARIO, PROJECT, &[(FILE, &code)], |_| {
        helpers::wait_for_editor(&root(), &root().join(FILE), |view| {
            let Some(view) = view else {
                return give_up(SCENARIO, "the file of 10 000 lines opens");
            };
            helpers::wait_within(
                COLOURED,
                move || settled(&view),
                |_| helpers::after(SETTLE, plain_keystroke),
            );
        });
    });
}

/// Two keystrokes inside a name, as many as `/*` takes, which change the colour of no other line, for comparison.
fn plain_keystroke() {
    let Some(view) = big() else {
        return give_up(SCENARIO, "the big file opens in an editor");
    };
    measure();
    helpers::type_at(&view, NSRange::new(11, 0), "xy");
    helpers::after(KEYSTROKE, || {
        let plain = stall::take_worst();
        if let Some(view) = big() {
            delete(&view, NSRange::new(11, 2));
        }
        helpers::after(SETTLE, move || open_comment(plain));
    });
}

fn open_comment(plain: Duration) {
    let Some(view) = big() else {
        return give_up(SCENARIO, "the big file opens in an editor");
    };
    check(
        "the lines on screen show their keywords before the edit",
        screen_coloured(&view, CodeToken::Keyword),
        format!("{:?}", editor::visible_line_range(&view)),
    );
    measure();
    helpers::type_at(&view, NSRange::new(0, 0), "/*");
    helpers::wait_within(
        COLOURED,
        || big().is_some_and(|v| settled(&v)),
        move |done| {
            no_longer(
                "typing /* at the top of 10 000 lines colours them in slices as short as a keystroke's",
                done,
                plain,
            );
            scroll_to_end(plain);
        },
    );
}

fn scroll_to_end(plain: Duration) {
    let Some(view) = big() else {
        return give_up(SCENARIO, "the big file opens in an editor");
    };
    measure();
    let near_end = view.line_start(FUNCTIONS * 5 - 40).unwrap_or(0);
    view.scrollRangeToVisible(NSRange::new(near_end, 0));
    helpers::wait_within(
        COLOURED,
        || big().is_some_and(|v| settled(&v) && screen_coloured(&v, CodeToken::Comment)),
        move |shown| {
            check(
                "scrolled to the end, the lines on screen show the comment's colour",
                shown,
                big()
                    .map(|v| format!("{:?}", editor::visible_line_range(&v)))
                    .unwrap_or_default(),
            );
            super::snappy::measured(
                "colouring the lines scrolled to never holds up input",
                "the scroll to the end",
            );
            close_comment(plain);
        },
    );
}

fn close_comment(plain: Duration) {
    let Some(view) = big() else {
        return give_up(SCENARIO, "the big file opens in an editor");
    };
    measure();
    delete(&view, NSRange::new(0, 2));
    helpers::wait_within(
        COLOURED,
        || big().is_some_and(|v| settled(&v) && screen_coloured(&v, CodeToken::Keyword)),
        move |done| {
            no_longer(
                "deleting the /* again colours the lines back in slices as short as a keystroke's",
                done,
                plain,
            );
            finish(SCENARIO);
        },
    );
}
