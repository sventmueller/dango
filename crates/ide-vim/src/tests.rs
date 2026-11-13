use std::time::{Duration, Instant};

use super::*;

/// One case: the text before, the cursor before, the keys, the text after and the cursor after.
struct Case {
    text: &'static str,
    at: (usize, usize),
    keys: &'static str,
    after: &'static str,
    cursor: (usize, usize),
}

const CASES: &[Case] = &[
    // Motions
    Case {
        text: "one two three\n",
        at: (0, 0),
        keys: "w",
        after: "one two three\n",
        cursor: (0, 4),
    },
    Case {
        text: "one two three\n",
        at: (0, 8),
        keys: "b",
        after: "one two three\n",
        cursor: (0, 4),
    },
    Case {
        text: "one two three\n",
        at: (0, 0),
        keys: "2w",
        after: "one two three\n",
        cursor: (0, 8),
    },
    Case {
        text: "one two three\n",
        at: (0, 0),
        keys: "e",
        after: "one two three\n",
        cursor: (0, 2),
    },
    Case {
        text: "one two three\n",
        at: (0, 4),
        keys: "$",
        after: "one two three\n",
        cursor: (0, 12),
    },
    Case {
        text: "  indented\n",
        at: (0, 8),
        keys: "^",
        after: "  indented\n",
        cursor: (0, 2),
    },
    Case {
        text: "  indented\n",
        at: (0, 8),
        keys: "0",
        after: "  indented\n",
        cursor: (0, 0),
    },
    Case {
        text: "a\nb\nc\n",
        at: (0, 0),
        keys: "j",
        after: "a\nb\nc\n",
        cursor: (1, 0),
    },
    Case {
        text: "a\nb\nc\n",
        at: (2, 0),
        keys: "gg",
        after: "a\nb\nc\n",
        cursor: (0, 0),
    },
    Case {
        text: "a\nb\nc\n",
        at: (0, 0),
        keys: "G",
        after: "a\nb\nc\n",
        cursor: (2, 0),
    },
    Case {
        text: "f(a, b)\n",
        at: (0, 0),
        keys: "f,",
        after: "f(a, b)\n",
        cursor: (0, 3),
    },
    Case {
        text: "f(a, b)\n",
        at: (0, 1),
        keys: "%",
        after: "f(a, b)\n",
        cursor: (0, 6),
    },
    // Operators with motions
    Case {
        text: "one\ntwo\nthree\n",
        at: (1, 0),
        keys: "dd",
        after: "one\nthree\n",
        cursor: (1, 0),
    },
    Case {
        text: "one two three\n",
        at: (0, 0),
        keys: "dw",
        after: "two three\n",
        cursor: (0, 0),
    },
    Case {
        text: "one two three\n",
        at: (0, 4),
        keys: "d$",
        after: "one \n",
        cursor: (0, 3),
    },
    Case {
        text: "one two\n",
        at: (0, 0),
        keys: "x",
        after: "ne two\n",
        cursor: (0, 0),
    },
    Case {
        text: "a\nb\nc\nd\n",
        at: (0, 0),
        keys: "2dd",
        after: "c\nd\n",
        cursor: (0, 0),
    },
    Case {
        text: "one\ntwo\n",
        at: (0, 0),
        keys: "yyp",
        after: "one\none\ntwo\n",
        cursor: (1, 0),
    },
    Case {
        text: "one two\n",
        at: (0, 0),
        keys: "cwnew<Esc>",
        after: "new two\n",
        cursor: (0, 2),
    },
    Case {
        text: "abc\n",
        at: (0, 0),
        keys: "~",
        after: "Abc\n",
        cursor: (0, 1),
    },
    Case {
        text: "a b\n",
        at: (0, 0),
        keys: "rX",
        after: "X b\n",
        cursor: (0, 0),
    },
    // Text objects
    Case {
        text: "say hello world\n",
        at: (0, 6),
        keys: "ciwbye<Esc>",
        after: "say bye world\n",
        cursor: (0, 6),
    },
    Case {
        text: "f(a, b)\n",
        at: (0, 3),
        keys: "di(",
        after: "f()\n",
        cursor: (0, 2),
    },
    Case {
        text: "x = \"str\";\n",
        at: (0, 6),
        keys: "ci\"s<Esc>",
        after: "x = \"s\";\n",
        cursor: (0, 5),
    },
    Case {
        text: "one two three\n",
        at: (0, 5),
        keys: "daw",
        after: "one three\n",
        cursor: (0, 4),
    },
    // Repeat
    Case {
        text: "a b c\n",
        at: (0, 0),
        keys: "ciwX<Esc>w.",
        after: "X X c\n",
        cursor: (0, 2),
    },
    Case {
        text: "a\nb\nc\n",
        at: (0, 0),
        keys: "dd.",
        after: "c\n",
        cursor: (0, 0),
    },
    // Insert forms
    Case {
        text: "abc\n",
        at: (0, 1),
        keys: "aX<Esc>",
        after: "abXc\n",
        cursor: (0, 2),
    },
    Case {
        text: "abc\n",
        at: (0, 1),
        keys: "AX<Esc>",
        after: "abcX\n",
        cursor: (0, 3),
    },
    Case {
        text: "  abc\n",
        at: (0, 4),
        keys: "IX<Esc>",
        after: "  Xabc\n",
        cursor: (0, 2),
    },
    Case {
        text: "a\nb\n",
        at: (0, 0),
        keys: "oX<Esc>",
        after: "a\nX\nb\n",
        cursor: (1, 0),
    },
    Case {
        text: "a\nb\n",
        at: (1, 0),
        keys: "OX<Esc>",
        after: "a\nX\nb\n",
        cursor: (1, 0),
    },
    // Visual
    Case {
        text: "one two\n",
        at: (0, 0),
        keys: "vlld",
        after: " two\n",
        cursor: (0, 0),
    },
    Case {
        text: "a\nb\nc\n",
        at: (0, 0),
        keys: "Vjd",
        after: "c\n",
        cursor: (0, 0),
    },
    // Registers and macros
    Case {
        text: "one\ntwo\n",
        at: (0, 0),
        keys: "\"ayyj\"ap",
        after: "one\ntwo\none\n",
        cursor: (2, 0),
    },
    Case {
        text: "a\nb\nc\n",
        at: (0, 0),
        keys: "qqA;<Esc>jq@q",
        after: "a;\nb;\nc\n",
        cursor: (2, 0),
    },
    // Search and Ex
    Case {
        text: "alpha\nbeta\ngamma beta\n",
        at: (0, 0),
        keys: "/beta<Enter>",
        after: "alpha\nbeta\ngamma beta\n",
        cursor: (1, 0),
    },
    Case {
        text: "alpha\nbeta\ngamma beta\n",
        at: (0, 0),
        keys: "/beta<Enter>n",
        after: "alpha\nbeta\ngamma beta\n",
        cursor: (2, 6),
    },
    Case {
        text: "alpha\nbeta\ngamma beta\n",
        at: (2, 8),
        keys: "?beta<Enter>",
        after: "alpha\nbeta\ngamma beta\n",
        cursor: (2, 6),
    },
    Case {
        text: "x x\ny x\n",
        at: (0, 0),
        keys: ":%s/x/z/g<Enter>",
        after: "z z\ny z\n",
        cursor: (0, 0),
    },
    Case {
        text: "a\nb\nc\n",
        at: (0, 0),
        keys: ":3<Enter>",
        after: "a\nb\nc\n",
        cursor: (2, 0),
    },
];

fn run(case: &Case, crlf: bool) -> Option<String> {
    let text = if crlf {
        case.text.replace('\n', "\r\n")
    } else {
        case.text.to_string()
    };
    let mut vim = Vim::new(&text);
    vim.set_cursor(case.at.0, case.at.1);
    vim.keys(case.keys);
    let (after, cursor) = (vim.text(), vim.cursor());
    (after != case.after || cursor != case.cursor).then(|| {
        format!(
            "{:?} on {:?}{}: {after:?} at {cursor:?}, wanted {:?} at {:?}",
            case.keys,
            case.text,
            if crlf { " (CRLF)" } else { "" },
            case.after,
            case.cursor
        )
    })
}

#[test]
fn every_operator_motion_and_text_object_does_what_vim_does_with_lf_and_crlf() {
    let wrong: Vec<String> = CASES
        .iter()
        .flat_map(|case| [run(case, false), run(case, true)])
        .flatten()
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_replacements_turn_the_editors_text_into_the_copy() {
    let mut vim = Vim::new("say hello world\n");
    vim.set_cursor(0, 6);
    let done = vim.keys("ciwbye<Esc>");
    let mut text = String::from("say hello world\n");
    for replacement in &done.replacements {
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let offset = |(line, column): (usize, usize)| -> usize {
            lines[..line].iter().map(|l| l.len()).sum::<usize>()
                + lines[line].chars().take(column).map(char::len_utf8).sum::<usize>()
        };
        let (start, end) = (offset(replacement.start), offset(replacement.end));
        text.replace_range(start..end, &replacement.text);
    }
    assert_eq!(text, vim.text());
}

#[test]
fn undo_and_redo_go_to_the_editor_and_ex_commands_to_the_app() {
    let mut vim = Vim::new("a\n");
    assert_eq!(vim.keys("u").effects, [Effect::Undo(1)]);
    assert_eq!(vim.keys("3u").effects, [Effect::Undo(3)]);
    assert_eq!(vim.keys("<C-R>").effects, [Effect::Redo(1)]);
    assert_eq!(vim.keys(":wq<Enter>").effects, [Effect::Save, Effect::Close(true)]);
    assert_eq!(vim.keys(":x<Enter>").effects, [Effect::Save, Effect::Close(true)]);
    assert_eq!(vim.keys(":noh<Enter>").effects, [Effect::Highlight(None)]);
    assert!(matches!(
        vim.keys(":vsplit<Enter>").effects.as_slice(),
        [Effect::Message(_)]
    ));
}

#[test]
fn the_modes_show_as_the_status_bar_names_them() {
    let mut vim = Vim::new("abc\n");
    assert_eq!(vim.mode(), Mode::Normal);
    vim.keys("i");
    assert_eq!(vim.mode(), Mode::Insert);
    vim.keys("<Esc>v");
    assert_eq!(vim.mode(), Mode::Visual);
    vim.keys("<Esc>V");
    assert_eq!(vim.mode(), Mode::VisualLine);
    vim.keys("<Esc><C-V>");
    assert_eq!(vim.mode(), Mode::VisualBlock);
    vim.keys("<Esc>R");
    assert_eq!(vim.mode(), Mode::Replace);
    vim.keys("<Esc>/");
    assert_eq!(vim.mode(), Mode::Prompt);
}

#[test]
fn a_change_from_outside_is_taken_without_losing_the_cursor() {
    let mut vim = Vim::new("one\ntwo\n");
    vim.set_cursor(1, 2);
    vim.resync("one\ntwo\nthree\n");
    assert_eq!(vim.cursor(), (1, 2));
    vim.resync("x\n");
    assert_eq!(vim.cursor(), (0, 1));
}

/// The gate of E104: a 10 MB file stays within the keystroke budget of E25 with the copy, motions and an edit.
#[test]
#[ignore = "a timing budget; make perf runs it"]
fn a_ten_megabyte_file_keeps_a_keystroke_under_sixteen_milliseconds() {
    let line = "    let value = compute(argument, other_argument); // a comment\n";
    let text: String = line.repeat(10 * 1024 * 1024 / line.len());
    let built = Instant::now();
    let mut vim = Vim::new(&text);
    let setup = built.elapsed();
    vim.set_cursor(80_000, 0);
    let mut slowest = Duration::ZERO;
    for keys in ["w", "w", "b", "j", "k", "x", "dd", "u"] {
        let started = Instant::now();
        vim.keys(keys);
        slowest = slowest.max(started.elapsed());
    }

    eprintln!("copy of 10 MB built in {setup:?}, slowest key {slowest:?}");
    assert!(slowest < Duration::from_millis(16), "slowest key {slowest:?}");
}

#[test]
fn indenting_goes_to_the_editor_with_the_lines_it_reaches() {
    let mut vim = Vim::new("a\nb\nc\nd\n");
    vim.set_cursor(1, 0);
    assert_eq!(vim.keys(">>").effects, [Effect::Indent(1..2, 1)]);
    assert_eq!(vim.keys("2<<").effects, [Effect::Indent(1..3, -1)]);
    vim.set_cursor(0, 0);
    assert_eq!(vim.keys("Vj>").effects, [Effect::Indent(0..2, 1)]);
    assert_eq!(vim.mode(), Mode::Normal);
}

#[test]
fn the_plus_register_and_a_shared_unnamed_one_are_the_clipboard() {
    let mut vim = Vim::new("one\ntwo\n");
    assert_eq!(vim.keys("\"+yy").effects, [Effect::Copy("one\n".into())]);
    assert!(vim.keys("yy").effects.is_empty(), "the unnamed register is Vim's own");
    vim.set_clipboard("from mac");
    vim.keys("\"+p");
    assert_eq!(vim.text(), "ofrom macne\ntwo\n");
    let mut shared = Vim::new("one\n");
    shared.share_clipboard(true);
    assert_eq!(shared.keys("yw").effects, [Effect::Copy("one".into())]);
    shared.set_clipboard("X");
    shared.keys("P");
    assert_eq!(shared.text(), "Xone\n");
}
