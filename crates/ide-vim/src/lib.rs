//! Vim's modes, keys, motions and operators (E104–E106) without AppKit: the only crate that knows `modalkit`. A
//! [`Vim`] holds a shadow copy of an editor's text with LF line ends only, feeds keys to `modalkit`'s Vim machine,
//! lets its `EditBuffer` carry out operators, registers, marks, visual mode and macros on the copy, and hands back what
//! changed as replacements by line and column in characters, which the editor applies to its own text. Undo and redo
//! go to the editor's own history, and `/`, `?`, `n` and `N` search with Rust's regular expressions.
//!
//! ```
//! use ide_vim::{Mode, Vim};
//!
//! let mut vim = Vim::new("one two\nthree\n");
//! vim.keys("w");
//! assert_eq!(vim.cursor(), (0, 4));
//! let done = vim.keys("dd");
//! assert_eq!(vim.text(), "three\n");
//! assert_eq!(done.replacements.len(), 1);
//! vim.keys("i");
//! assert_eq!(vim.mode(), Mode::Insert);
//! ```

#![warn(missing_docs)]

mod note;
mod search;

use std::str::FromStr;

use modalkit::actions::EditAction;
use modalkit::actions::{Action, Editable, EditorAction, HistoryAction, Jumpable};
use modalkit::editing::application::EmptyInfo;
use modalkit::editing::buffer::{CursorGroupId, EditBuffer};
use modalkit::editing::context::{EditContext, Resolve};
use modalkit::editing::cursor::Cursor;
use modalkit::editing::key::KeyManager;
use modalkit::editing::store::Store;
use modalkit::env::vim::keybindings::default_vim_keys;
use modalkit::key::TerminalKey;
use modalkit::keybindings::BindingMachine;
use modalkit::prelude::{EditTarget, IndentChange, RepeatType, ViewportContext};

pub use note::Note;
use search::Search;

/// The mode the keys are in, as the status bar shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Keys move and run operators.
    Normal,
    /// Keys type text.
    Insert,
    /// Keys replace the characters under the cursor.
    Replace,
    /// A stretch of characters is selected.
    Visual,
    /// Whole lines are selected.
    VisualLine,
    /// A block of columns is selected.
    VisualBlock,
    /// A search pattern or an Ex command is being typed after `/`, `?` or `:`.
    Prompt,
}

/// A change of the text: the characters from `start` to `end`, each a line and a column in characters counted from 0,
/// make way for `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// Where the replaced characters begin.
    pub start: (usize, usize),
    /// Where they end, not included.
    pub end: (usize, usize),
    /// What takes their place.
    pub text: String,
}

/// What the editor does for Vim beyond replacing text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Undo this many steps of the editor's history.
    Undo(usize),
    /// Redo this many steps.
    Redo(usize),
    /// Save the file, as `:w` asks.
    Save,
    /// Close the editor, as `:q` asks; with `true` after saving it, as `:wq` and `:x` ask.
    Close(bool),
    /// Say this in the status bar, such as a command Vim does not support here.
    Note(Note),
    /// Mark the matches of this pattern, or none for `:noh`.
    Highlight(Option<String>),
    /// Put this text on the system clipboard, which a command wrote into `"+` or `"*`, or into the unnamed register
    /// while it is the clipboard.
    Copy(String),
    /// Indent these lines, counted from 0, by this many levels, or take as many away when negative, with the
    /// editor's own rule for a level; `modalkit` leaves indenting to its user.
    Indent(std::ops::Range<usize>, isize),
}

/// What a key or a run of keys did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The changes of the text, in the order they apply.
    pub replacements: Vec<Replacement>,
    /// What else the editor does.
    pub effects: Vec<Effect>,
}

/// Vim's state for one editor.
pub struct Vim {
    keys: KeyManager<TerminalKey, Action<EmptyInfo>, RepeatType>,
    buffer: EditBuffer<EmptyInfo>,
    store: Store<EmptyInfo>,
    group: CursorGroupId,
    viewport: ViewportContext<Cursor>,
    search: Search,
    /// Whether the unnamed register is the system clipboard too (`vimClipboard`).
    shared_clipboard: bool,
}

impl Vim {
    /// Vim on a copy of `text`, in Normal mode at its start.
    pub fn new(text: &str) -> Vim {
        let mut buffer = EditBuffer::from_str(String::from("editor"), &shadow(text));
        let group = buffer.create_group();
        Vim {
            keys: KeyManager::new(default_vim_keys::<EmptyInfo>()),
            buffer,
            store: Store::default(),
            group,
            viewport: ViewportContext::default(),
            search: Search::default(),
            shared_clipboard: false,
        }
    }

    /// Makes the unnamed register the system clipboard too, or Vim's own again (E106).
    pub fn share_clipboard(&mut self, shared: bool) {
        self.shared_clipboard = shared;
    }

    /// Takes the system clipboard's `text` into `"+` and `"*`, and into the unnamed register while it is shared;
    /// the editor calls it before a key that may paste.
    pub fn set_clipboard(&mut self, text: &str) {
        self.store.registers.set_system_text(text);
        if self.shared_clipboard && self.store.registers.unnamed_text() != text {
            self.store.registers.set_unnamed_text(text);
        }
    }

    /// Takes `text` as the editor's text again after a change Vim did not make, such as the agent's or the file's,
    /// keeping the cursor where it was as far as the text reaches; where the first change begins, if any.
    pub fn resync(&mut self, text: &str) -> Option<(usize, usize)> {
        let cursor = self.cursor();
        let before = self.buffer.get().clone();
        self.buffer.set_text(shadow(text));
        self.set_cursor(cursor.0, cursor.1);
        difference(before.as_rope(), self.buffer.get().as_rope()).map(|r| r.start)
    }

    /// The copy of the text, with LF line ends.
    pub fn text(&self) -> String {
        self.buffer.get_text()
    }

    /// The cursor as a line and a column in characters, counted from 0.
    pub fn cursor(&mut self) -> (usize, usize) {
        let cursor = self.buffer.get_leader(self.group);
        (cursor.y, cursor.x)
    }

    /// Puts the cursor at `line` and `column`, as a click or the editor's own move does.
    pub fn set_cursor(&mut self, line: usize, column: usize) {
        let lines = self.buffer.get_lines().max(1);
        let line = line.min(lines - 1);
        let column = column.min(self.buffer.get_columns(line));
        self.buffer.set_leader(self.group, Cursor::new(line, column));
    }

    /// The mode the keys are in.
    pub fn mode(&self) -> Mode {
        if self.search.is_open() {
            return Mode::Prompt;
        }
        mode_of(self.keys.show_mode().as_deref())
    }

    /// Feeds the character `c` as typed, `<` and spaces included, which [`Vim::keys`] would read as key names.
    pub fn char(&mut self, c: char) -> Outcome {
        use modalkit::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        self.key(TerminalKey::from(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))
    }

    /// The selection of Visual mode as its first and last character, each a line and a column; `None` outside it.
    pub fn selection(&mut self) -> Option<((usize, usize), (usize, usize))> {
        if !matches!(self.mode(), Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
            return None;
        }
        let (a, b, _) = self.buffer.get_leader_selection(self.group)?;
        let (first, last) = if (a.y, a.x) <= (b.y, b.x) { (a, b) } else { (b, a) };
        Some(((first.y, first.x), (last.y, last.x)))
    }

    /// What the prompt after `/`, `?` or `:` shows while it is open.
    pub fn prompt(&self) -> Option<String> {
        self.search.shown()
    }

    /// Feeds the keys written as Vim writes them, such as `dd`, `ciw<Esc>` or `<C-R>`, and says what they did.
    pub fn keys(&mut self, keys: &str) -> Outcome {
        let mut outcome = Outcome::default();
        for key in parse(keys) {
            let one = self.key(key);
            outcome.replacements.extend(one.replacements);
            outcome.effects.extend(one.effects);
        }
        outcome
    }

    /// Feeds one key.
    pub(crate) fn key(&mut self, key: TerminalKey) -> Outcome {
        let mut effects = Vec::new();
        if self.search.is_open() || (self.mode() == Mode::Normal && search::opens(&key)) {
            let (cursor, text) = (self.cursor(), self.text());
            match self.search.key(&key, &text, cursor) {
                search::Step::Typing => {}
                search::Step::Jump(line, column, pattern) => {
                    self.set_cursor(line, column);
                    effects.push(Effect::Highlight(Some(pattern)));
                }
                search::Step::Command(command) => effects.extend(self.ex(&command)),
                search::Step::Failed(note) => effects.push(Effect::Note(note)),
            }
            return Outcome {
                replacements: Vec::new(),
                effects,
            };
        }
        if self.mode() == Mode::Normal && search::repeats(&key) {
            let (cursor, text) = (self.cursor(), self.text());
            if let Some((line, column)) = self.search.again(&key, &text, cursor) {
                self.set_cursor(line, column);
            }
            return Outcome::default();
        }
        let name = key.to_string();
        let normal = self.mode() == Mode::Normal;
        // A clone of the rope shares its nodes; it becomes text only when an action may have changed it.
        let before = self.buffer.get().clone();
        self.keys.input_key(key);
        let mut changes = false;
        let unnamed = self.store.registers.unnamed_text();
        while let Some((action, context)) = self.keys.pop() {
            changes |= may_change_text(&action, &context);
            effects.extend(self.act(&action, &context));
        }
        if let Some(copied) = self.store.registers.take_system_written() {
            effects.push(Effect::Copy(copied));
        } else if self.shared_clipboard {
            let now = self.store.registers.unnamed_text();
            if now != unnamed {
                effects.push(Effect::Copy(now));
            }
        }
        let replacement = if changes {
            difference(before.as_rope(), self.buffer.get().as_rope())
        } else {
            None
        };
        if normal && self.mode() == Mode::Normal {
            self.settle_cursor(&name, replacement.as_ref());
        }
        Outcome {
            replacements: replacement.into_iter().collect(),
            effects,
        }
    }

    /// Puts the cursor where Vim leaves it after `key` in Normal mode, where `modalkit` leaves it elsewhere: one
    /// further after `~`, on the last character typed after `.` repeated an insertion.
    fn settle_cursor(&mut self, key: &str, replacement: Option<&Replacement>) {
        let (line, column) = self.cursor();
        let last = self.buffer.get_columns(line).saturating_sub(1);
        match (key, replacement) {
            ("~", Some(_)) => self.set_cursor(line, (column + 1).min(last)),
            (".", Some(replaced)) if !replaced.text.is_empty() && !replaced.text.contains('\n') => {
                let end = replaced.start.1 + replaced.text.chars().count();
                self.set_cursor(replaced.start.0, end.saturating_sub(1));
            }
            _ => {}
        }
    }

    /// Carries out one action of the machine on the copy, or says what the editor does for it.
    fn act(&mut self, action: &Action<EmptyInfo>, context: &EditContext) -> Vec<Effect> {
        let ctx = (self.group, &self.viewport, context);
        match action {
            Action::NoOp => Vec::new(),
            Action::Editor(EditorAction::History(HistoryAction::Undo(count))) => {
                vec![Effect::Undo(context.resolve(count))]
            }
            Action::Editor(EditorAction::History(HistoryAction::Redo(count))) => {
                vec![Effect::Redo(context.resolve(count))]
            }
            // Undo is the editor's (E106); `modalkit`'s own history keeps a copy of the text per checkpoint, which
            // costs half a second in a 10 MB file.
            Action::Editor(EditorAction::History(HistoryAction::Checkpoint)) => Vec::new(),
            Action::Editor(EditorAction::Edit(edit, target))
                if matches!(context.resolve(edit), EditAction::Indent(_)) =>
            {
                let EditAction::Indent(change) = context.resolve(edit) else {
                    return Vec::new();
                };
                let levels = match change {
                    IndentChange::Increase(count) => context.resolve(&count).cast_signed(),
                    IndentChange::Decrease(count) => -context.resolve(&count).cast_signed(),
                    IndentChange::Auto => 0,
                };
                vec![Effect::Indent(self.indented_lines(target, context), levels)]
            }
            Action::Editor(editor) => match self.buffer.editor_command(editor, &ctx, &mut self.store) {
                Ok(_) => Vec::new(),
                Err(error) => vec![Effect::Note(Note::of_edit(&error))],
            },
            Action::Macro(act) => match self.keys.macro_command(act, context, &mut self.store) {
                Ok(_) => Vec::new(),
                Err(error) => vec![Effect::Note(Note::of_edit(&error))],
            },
            Action::Repeat(kind) => {
                self.keys.repeat(kind.clone(), Some(context.clone()));
                Vec::new()
            }
            Action::Jump(list, dir, count) => match self.buffer.jump(*list, *dir, context.resolve(count), &ctx) {
                Ok(_) => Vec::new(),
                Err(error) => vec![Effect::Note(Note::of_ui(&error))],
            },
            _ => Vec::new(),
        }
    }

    /// The lines an indent with `target` reaches: the selection's in Visual mode, else the count of lines from the
    /// cursor's.
    fn indented_lines(&mut self, target: &EditTarget, context: &EditContext) -> std::ops::Range<usize> {
        let line = self.cursor().0;
        if let Some(selection) = self.buffer.get_leader_selection(self.group) {
            let (a, b, _) = selection;
            let (first, last) = (a.y.min(b.y), a.y.max(b.y));
            self.buffer.set_leader(self.group, Cursor::new(first, 0));
            return first..last + 1;
        }
        let count = match target {
            EditTarget::Range(_, _, count) | EditTarget::Motion(_, count) => context.resolve(count),
            _ => 1,
        };
        line..(line + count).min(self.buffer.get_lines())
    }

    /// Runs the Ex command `command`, typed after `:`.
    fn ex(&mut self, command: &str) -> Vec<Effect> {
        let command = command.trim();
        match command {
            "w" => vec![Effect::Save],
            "q" | "q!" => vec![Effect::Close(false)],
            "wq" | "x" => vec![Effect::Save, Effect::Close(true)],
            "noh" | "nohlsearch" => vec![Effect::Highlight(None)],
            line if line.parse::<usize>().is_ok() => {
                let line = line.parse::<usize>().unwrap_or(1).saturating_sub(1);
                self.set_cursor(line, 0);
                Vec::new()
            }
            _ => match search::substitute(command, &self.text(), self.cursor().0) {
                Some(Ok(text)) => {
                    let cursor = self.cursor();
                    self.buffer.set_text(text);
                    self.set_cursor(cursor.0, 0);
                    Vec::new()
                }
                Some(Err(note)) => vec![Effect::Note(note)],
                None => vec![Effect::Note(Note::UnsupportedCommand(command.to_string()))],
            },
        }
    }
}

/// Whether `action` can change the text, so that only those compare the text before and after.
fn may_change_text(action: &Action<EmptyInfo>, context: &EditContext) -> bool {
    match action {
        Action::Editor(EditorAction::Edit(edit, _)) => {
            !matches!(context.resolve(edit), EditAction::Motion | EditAction::Yank(..))
        }
        Action::Editor(EditorAction::InsertText(_) | EditorAction::Selection(_) | EditorAction::Complete(..)) => true,
        Action::Macro(_) | Action::Repeat(_) => true,
        _ => false,
    }
}

/// `text` with LF line ends only, which the copy keeps so that `$` and `x` never reach a CR (E104).
fn shadow(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The keys of `keys`, written as Vim writes them: single characters and names in angle brackets such as `<Esc>`.
pub(crate) fn parse(keys: &str) -> Vec<TerminalKey> {
    let mut out = Vec::new();
    let mut rest = keys;
    while let Some(first) = rest.chars().next() {
        if first == '<'
            && let Some(end) = rest.find('>')
            && let Ok(key) = TerminalKey::from_str(&rest[..=end])
        {
            out.push(key);
            rest = &rest[end + 1..];
            continue;
        }
        if let Ok(key) = TerminalKey::from_str(&first.to_string()) {
            out.push(key);
        }
        rest = &rest[first.len_utf8()..];
    }
    out
}

/// The mode a `show_mode` text such as `-- INSERT --` names.
fn mode_of(shown: Option<&str>) -> Mode {
    let shown = shown.unwrap_or_default();
    if shown.contains("VISUAL LINE") {
        Mode::VisualLine
    } else if shown.contains("VISUAL BLOCK") {
        Mode::VisualBlock
    } else if shown.contains("VISUAL") || shown.contains("SELECT") {
        Mode::Visual
    } else if shown.contains("REPLACE") {
        Mode::Replace
    } else if shown.contains("INSERT") {
        Mode::Insert
    } else {
        Mode::Normal
    }
}

/// The one replacement that turns `before` into `after`: what lies between their common start and common end. A rope
/// cloned before an edit shares every leaf the edit left alone, so leaves at the same address are skipped whole and
/// the work grows with the change, not with the file, which may run to megabytes (E25).
fn difference(before: &ropey::Rope, after: &ropey::Rope) -> Option<Replacement> {
    let prefix = common_start(before.chunks(), after.chunks());
    let (old_len, new_len) = (before.len_bytes(), after.len_bytes());
    if prefix == old_len && prefix == new_len {
        return None;
    }
    let most = old_len.min(new_len) - prefix;
    let suffix = common_start(
        before.chunks_at_byte(old_len).0.reversed().map(Reversed),
        after.chunks_at_byte(new_len).0.reversed().map(Reversed),
    )
    .min(most);
    let (start, end) = (char_floor(before, prefix), char_ceil(before, old_len - suffix));
    let new_end = char_ceil(after, new_len - (old_len - end));
    let new_start = char_floor(after, start);
    Some(Replacement {
        start: position(before, start),
        end: position(before, end),
        text: after.byte_slice(new_start..new_end).to_string(),
    })
}

/// A chunk read from its end, for comparing ropes backwards.
struct Reversed<'a>(&'a str);

/// Bytes read in one direction from a chunk of a rope.
trait Piece {
    fn address(&self) -> (*const u8, usize);
    fn byte(&self, index: usize) -> u8;
}

impl Piece for &str {
    fn address(&self) -> (*const u8, usize) {
        (self.as_ptr(), self.len())
    }

    fn byte(&self, index: usize) -> u8 {
        self.as_bytes()[index]
    }
}

impl Piece for Reversed<'_> {
    fn address(&self) -> (*const u8, usize) {
        (self.0.as_ptr(), self.0.len())
    }

    fn byte(&self, index: usize) -> u8 {
        self.0.as_bytes()[self.0.len() - 1 - index]
    }
}

/// How many bytes two runs of chunks share at their start; chunks at the same address count whole.
fn common_start<P: Piece>(mut a: impl Iterator<Item = P>, mut b: impl Iterator<Item = P>) -> usize {
    let (mut ca, mut cb) = (a.next(), b.next());
    let (mut ia, mut ib, mut same) = (0usize, 0usize, 0usize);
    loop {
        let (Some(pa), Some(pb)) = (ca.as_ref(), cb.as_ref()) else {
            return same;
        };
        if ia == 0 && ib == 0 && pa.address() == pb.address() {
            same += pa.address().1;
            (ca, cb) = (a.next(), b.next());
            continue;
        }
        if ia == pa.address().1 {
            (ca, ia) = (a.next(), 0);
            continue;
        }
        if ib == pb.address().1 {
            (cb, ib) = (b.next(), 0);
            continue;
        }
        if pa.byte(ia) != pb.byte(ib) {
            return same;
        }
        same += 1;
        ia += 1;
        ib += 1;
    }
}

/// The start of the character that byte `index` of `rope` lies in.
fn char_floor(rope: &ropey::Rope, index: usize) -> usize {
    rope.char_to_byte(rope.byte_to_char(index))
}

/// The start of the next character at or after byte `index` of `rope`.
fn char_ceil(rope: &ropey::Rope, index: usize) -> usize {
    let char = rope.byte_to_char(index);
    let floor = rope.char_to_byte(char);
    if floor == index {
        index
    } else {
        rope.char_to_byte(char + 1)
    }
}

/// The line and column in characters of byte `index` of `rope`, a character boundary.
fn position(rope: &ropey::Rope, index: usize) -> (usize, usize) {
    let char = rope.byte_to_char(index);
    let line = rope.char_to_line(char);
    (line, char - rope.line_to_char(line))
}

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod tests;
