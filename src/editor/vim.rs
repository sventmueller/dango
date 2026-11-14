//! The Vim mode of the editor (B26, E105, E106), off by default, from Editor ▸ Vim-Modus: `ide_vim` decides what a
//! key does on its copy of the text, and this module feeds it the keys and applies what it hands back. Every key goes
//! through the input context, so that dead keys of a German keyboard work; in Normal and Visual mode
//! `insertText:` and `doCommandBySelector:` give their result to Vim instead of the text. Insert mode types as the
//! editor always does, and a pass from `i` to Esc is one undo step. ⌃-keys go to Vim before the menu bar while the
//! editor has the focus. The caret is a block outside Insert mode, drawn by the overlay.

use std::cell::Cell;

use ide_core::settings::{Settings, keys};
use ide_document::{Edit, Unit};
use ide_vim::{Effect, Mode, Outcome, Vim};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSColor, NSEvent, NSEventModifierFlags, NSMenuItem, NSTextField};
use objc2_foundation::{NSRange, NSString};
use splat_i18n::t;

use super::view::EditorTextView;

/// Whether editors use Vim's keys, as the user last chose; read from the settings once.
#[derive(Default)]
pub struct Enabled(Cell<Option<bool>>);

impl Enabled {
    /// Reads the setting again on next use.
    pub fn forget(&self) {
        self.0.set(None);
    }
}

/// What Vim holds for one editor.
#[derive(Default)]
pub struct State {
    vim: Option<Vim>,
    /// Vim's own edits are being applied, which its copy already has.
    applying: bool,
    /// An undo group of an insert pass is open.
    grouping: bool,
    /// The mode shown last, to announce a change.
    shown: Option<Mode>,
    /// The mode in the status bar.
    label: Option<Retained<NSTextField>>,
    /// The system clipboard's change count Vim's registers last took.
    clipboard: isize,
}

fn context() -> Option<std::rc::Rc<crate::context::AppContext>> {
    MainThreadMarker::new().map(crate::context::AppContext::get)
}

/// Whether editors use Vim's keys.
pub fn enabled() -> bool {
    let Some(cx) = context() else { return false };
    let cell = &cx.editor.vim.0;
    cell.get().unwrap_or_else(|| {
        let value = Settings::load_user().get(&keys::VIM_MODE);
        cell.set(Some(value));
        value
    })
}

/// Turns the Vim mode on or off in every editor and remembers the choice.
pub fn toggle() {
    let value = !enabled();
    if let Some(cx) = context() {
        cx.editor.vim.0.set(Some(value));
    }
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::VIM_MODE, &value) {
        crate::diag::report("saving the Vim mode", error);
    }
    for view in super::open_editors() {
        attach(&view);
    }
}

/// Whether `item` is the menu item of the Vim mode, which then gets its check mark.
pub fn validate(item: &NSMenuItem) -> Option<bool> {
    if item.action() != Some(sel!(ideToggleVimMode:)) {
        return None;
    }
    item.setState(if enabled() {
        objc2_app_kit::NSControlStateValueOn
    } else {
        objc2_app_kit::NSControlStateValueOff
    });
    Some(true)
}

/// Gives `view` Vim's state while the mode is on, in Normal mode at its caret, or takes it away.
pub fn attach(view: &EditorTextView) {
    close_group(view);
    let vim = enabled().then(|| {
        let mut vim = Vim::new(&view.document().text().to_string());
        let (line, column) = line_column(view, view.selectedRange().location);
        vim.set_cursor(line, column);
        vim
    });
    view.ivars().vim.borrow_mut().vim = vim;
    show(view);
}

/// The mode of `view`'s Vim, or `None` while the mode is off.
pub fn mode(view: &EditorTextView) -> Option<Mode> {
    view.ivars().vim.borrow().vim.as_ref().map(Vim::mode)
}

/// Text from the input context: Vim's in Normal and Visual mode, else the editor's, which Vim follows for `.`;
/// whether Vim took it.
pub fn insert_text(view: &EditorTextView, text: &AnyObject) -> bool {
    let Some(mode) = mode(view) else { return false };
    let typed = if let Some(s) = text.downcast_ref::<NSString>() {
        s.to_string()
    } else if let Some(s) = text.downcast_ref::<objc2_foundation::NSAttributedString>() {
        s.string().to_string()
    } else {
        return false;
    };
    if mode == Mode::Insert {
        follow_typing(view, |vim| typed.chars().for_each(|c| drop(vim.char(c))));
        return false;
    }
    let typed: String = typed.chars().map(ascii_key).collect();
    run(view, |vim| {
        let mut outcome = Outcome::default();
        for c in typed.chars() {
            let one = vim.char(c);
            outcome.replacements.extend(one.replacements);
            outcome.effects.extend(one.effects);
        }
        outcome
    });
    true
}

/// A command of the input context, such as Esc or Return: Vim's outside Insert mode, and Esc there too; whether Vim
/// took it. Esc closes the completion list, the signature window, the prediction, dictation and further carets first,
/// which their own handlers before this one do.
pub fn command(view: &EditorTextView, command: Sel) -> bool {
    let Some(mode) = mode(view) else { return false };
    let key = if command == sel!(cancelOperation:) {
        "<Esc>"
    } else if command == sel!(insertNewline:) {
        "<Enter>"
    } else if command == sel!(deleteBackward:) {
        "<BS>"
    } else if command == sel!(insertTab:) {
        "<Tab>"
    } else if command == sel!(moveLeft:) {
        "<Left>"
    } else if command == sel!(moveRight:) {
        "<Right>"
    } else if command == sel!(moveUp:) {
        "<Up>"
    } else if command == sel!(moveDown:) {
        "<Down>"
    } else {
        return mode != Mode::Insert;
    };
    if mode == Mode::Insert {
        if key != "<Esc>" {
            follow_typing(view, |vim| drop(vim.keys(key)));
            return false;
        }
        leave_insert(view);
        return true;
    }
    run(view, |vim| vim.keys(key));
    true
}

/// The letters Vim takes with ⌃ in Normal and Visual mode (E105); the others stay the app's.
const NORMAL_CONTROL: &str = "rvodufbeyaxg";
/// The letters Vim takes with ⌃ in Insert mode.
const INSERT_CONTROL: &str = "orwu";

/// The commands of the shortcut table whose ⌃-shortcut the Vim mode takes in Normal mode, which the menus still
/// show.
#[cfg(test)]
fn shadowed() -> Vec<&'static str> {
    ide_core::keymap::DEFAULTS
        .iter()
        .filter(|binding| {
            binding.key.strip_prefix("ctrl+").is_some_and(|rest| {
                rest.chars().count() == 1 && rest.chars().all(|c| NORMAL_CONTROL.contains(c) || c == '[')
            })
        })
        .map(|binding| binding.command)
        .collect()
}

/// A key with ⌃ while the editor has the focus: Vim's in Normal and Visual mode, and ⌃O, ⌃R, ⌃W and ⌃U in Insert
/// mode, before the menu bar sees it (A6); ⌃[ and ⌃C are Esc. Whether Vim took it.
pub fn control_key(view: &EditorTextView, event: &NSEvent) -> bool {
    let Some(mode) = mode(view) else { return false };
    let flags = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    if !flags.contains(NSEventModifierFlags::Control) || flags.contains(NSEventModifierFlags::Command) {
        return false;
    }
    let focused = view.window().and_then(|w| w.firstResponder()).is_some_and(|r| {
        std::ptr::eq(
            objc2::rc::Retained::as_ptr(&r).cast::<u8>(),
            std::ptr::from_ref(view).cast::<u8>(),
        )
    });
    if !focused {
        return false;
    }
    let Some(c) = event
        .charactersIgnoringModifiers()
        .and_then(|s| s.to_string().chars().next())
        .map(|c| ascii_key(c).to_ascii_lowercase())
    else {
        return false;
    };
    if c == '[' || c == 'c' {
        if mode == Mode::Insert {
            leave_insert(view);
        } else {
            run(view, |vim| vim.keys("<Esc>"));
        }
        return true;
    }
    let taken = if mode == Mode::Insert {
        INSERT_CONTROL
    } else {
        NORMAL_CONTROL
    };
    if !taken.contains(c) {
        return false;
    }
    if mode == Mode::Insert {
        sync_copy(view);
    }
    let key = format!("<C-{}>", c.to_ascii_uppercase());
    run(view, |vim| vim.keys(&key));
    true
}

/// The editor's text changed by something other than Vim, such as the agent, the file or the user typing in Insert
/// mode: Vim's copy follows, but not keystroke by keystroke while it types along in Insert mode.
pub fn changed(view: &EditorTextView) {
    let (applying, inserting) = {
        let state = view.ivars().vim.borrow();
        (
            state.applying,
            state.vim.as_ref().is_some_and(|v| v.mode() == Mode::Insert),
        )
    };
    if applying || inserting {
        return;
    }
    if view.ivars().vim.borrow().vim.is_some() {
        sync_copy(view);
    }
}

/// The editor lost the focus or its document: an open insert pass ends as one undo step.
pub fn focus_lost(view: &EditorTextView) {
    close_group(view);
}

/// A change from outside, which `apply_model_changes` makes, never joins the undo step of an insert pass (E24).
pub fn before_model_change(view: &EditorTextView) {
    close_group(view);
}

fn follow_typing(view: &EditorTextView, f: impl FnOnce(&mut Vim)) {
    if let Some(vim) = view.ivars().vim.borrow_mut().vim.as_mut() {
        f(vim);
    }
}

/// Esc in Insert mode: the copy takes what the editor's own typing made, such as auto-closed brackets, then Vim
/// leaves Insert mode and the pass becomes one undo step.
fn leave_insert(view: &EditorTextView) {
    sync_copy(view);
    run(view, |vim| vim.keys("<Esc>"));
    close_group(view);
}

/// Puts the editor's text into Vim's copy, its caret with it.
fn sync_copy(view: &EditorTextView) {
    let text = view.document().text().to_string();
    let (line, column) = line_column(view, view.selectedRange().location);
    if let Some(vim) = view.ivars().vim.borrow_mut().vim.as_mut() {
        let _ = vim.resync(&text);
        vim.set_cursor(line, column);
    }
}

/// Gives Vim's `"+`, `"*` and, with `vimClipboard`, its unnamed register the system clipboard, when it changed.
fn load_clipboard(view: &EditorTextView) {
    let pasteboard = objc2_app_kit::NSPasteboard::generalPasteboard();
    let count = pasteboard.changeCount();
    let mut state = view.ivars().vim.borrow_mut();
    if state.clipboard == count {
        return;
    }
    state.clipboard = count;
    // SAFETY: AppKit's constant for plain text, a valid pasteboard type.
    let kind = unsafe { objc2_app_kit::NSPasteboardTypeString };
    let text = pasteboard
        .stringForType(kind)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let shared = Settings::load_user().get(&keys::VIM_CLIPBOARD);
    if let Some(vim) = state.vim.as_mut() {
        vim.share_clipboard(shared);
        vim.set_clipboard(&text);
    }
}

/// Runs `f` on `view`'s Vim and applies what it did to the editor.
fn run(view: &EditorTextView, f: impl FnOnce(&mut Vim) -> Outcome) {
    load_clipboard(view);
    let before = mode(view);
    let outcome = {
        let mut state = view.ivars().vim.borrow_mut();
        let Some(vim) = state.vim.as_mut() else { return };
        f(vim)
    };
    let after = mode(view);
    if before != Some(Mode::Insert) && after == Some(Mode::Insert) {
        open_group(view);
    }
    apply(view, &outcome);
    for effect in outcome.effects {
        perform(view, effect);
    }
    show(view);
}

/// Makes Vim's replacements in the editor's document, each an undo step unless an insert pass groups them.
fn apply(view: &EditorTextView, outcome: &Outcome) {
    if outcome.replacements.is_empty() {
        return;
    }
    let ending = if view.document().text().to_string().contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    view.ivars().vim.borrow_mut().applying = true;
    for replacement in &outcome.replacements {
        let start = utf16_of(view, replacement.start);
        let end = utf16_of(view, replacement.end);
        let text = replacement.text.replace('\n', ending);
        let change = view.document_mut().apply(Edit {
            range_utf16: start..end,
            text,
        });
        match change {
            Ok(change) => view.apply_vim_change(&change),
            Err(error) => crate::diag::report("applying a Vim edit", error),
        }
    }
    view.ivars().vim.borrow_mut().applying = false;
}

fn perform(view: &EditorTextView, effect: Effect) {
    match effect {
        Effect::Undo(count) | Effect::Redo(count) => {
            let undo = matches!(effect, Effect::Undo(_));
            if let Some(manager) = view.undoManager() {
                for _ in 0..count {
                    if undo { manager.undo() } else { manager.redo() }
                }
            }
            // Vim puts the cursor where the undone change began, AppKit after it.
            let text = view.document().text().to_string();
            if let Some(vim) = view.ivars().vim.borrow_mut().vim.as_mut()
                && let Some((line, column)) = vim.resync(&text)
            {
                vim.set_cursor(line, column);
            }
        }
        Effect::Save => crate::lsp::save(view),
        Effect::Close(_) => {
            let app = objc2_app_kit::NSApplication::sharedApplication(view.mtm());
            // SAFETY: The window's close command with no sender, as the menu item sends it to the first responder.
            unsafe { app.sendAction_to_from(sel!(performClose:), None, None) };
        }
        Effect::Message(text) => crate::lsp::status::note(view, &text),
        Effect::Copy(text) => {
            let pasteboard = objc2_app_kit::NSPasteboard::generalPasteboard();
            pasteboard.clearContents();
            // SAFETY: AppKit's constant for plain text, a valid pasteboard type.
            let kind = unsafe { objc2_app_kit::NSPasteboardTypeString };
            pasteboard.setString_forType(&crate::util::ns(&text), kind);
            view.ivars().vim.borrow_mut().clipboard = pasteboard.changeCount();
        }
        Effect::Highlight(pattern) => highlight(view, pattern.as_deref()),
        Effect::Indent(lines, levels) => super::indent::shift_lines(view, lines, levels),
    }
}

/// Marks the match under the caret of the last search, or nothing after `:noh`.
fn highlight(view: &EditorTextView, pattern: Option<&str>) {
    let Some(pattern) = pattern else {
        view.clear_found();
        return;
    };
    let Ok(regex) = regex::Regex::new(pattern) else { return };
    let caret = view.selectedRange().location;
    let text = view.document().text().to_string();
    let byte = view.document().text().utf16_to_byte(caret).unwrap_or(0);
    if let Some(found) = regex.find_at(&text, byte.min(text.len())) {
        let (line, start) = line_column(view, caret);
        let length = found.as_str().encode_utf16().count();
        view.select_columns(line, start..start + length, false);
    }
}

/// Shows Vim's caret, selection and mode in the editor and its status bar, and says a new mode for VoiceOver.
fn show(view: &EditorTextView) {
    let (mode, cursor, selection, prompt) = {
        let mut state = view.ivars().vim.borrow_mut();
        match state.vim.as_mut() {
            Some(vim) => (Some(vim.mode()), Some(vim.cursor()), vim.selection(), vim.prompt()),
            None => (None, None, None, None),
        }
    };
    let insert = matches!(mode, None | Some(Mode::Insert));
    let color = if insert {
        NSColor::textColor()
    } else {
        NSColor::clearColor()
    };
    view.setInsertionPointColor(Some(&color));
    if mode.is_some() {
        let range = match (selection, cursor) {
            (Some((first, last)), _) => {
                let start = utf16_of(view, first);
                let end = utf16_of(view, (last.0, last.1 + 1));
                NSRange::new(start, end.saturating_sub(start))
            }
            (None, Some(at)) => NSRange::new(utf16_of(view, at), 0),
            (None, None) => view.selectedRange(),
        };
        view.setSelectedRange(range);
        view.scrollRangeToVisible(NSRange::new(range.location, 0));
    }
    let text = match (mode, prompt) {
        (_, Some(prompt)) => prompt,
        (Some(mode), None) => mode_name(mode).to_string(),
        (None, None) => String::new(),
    };
    let label = view.ivars().vim.borrow().label.clone();
    if let Some(label) = label {
        label.setStringValue(&crate::util::ns(&text));
        label.setHidden(mode.is_none());
    }
    let changed = view.ivars().vim.borrow().shown != mode;
    view.ivars().vim.borrow_mut().shown = mode;
    if changed && let Some(mode) = mode {
        ide_ui_common::panel::announce(view.mtm(), mode_name(mode));
    }
    super::overlay::redraw(view);
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => t("NORMAL"),
        Mode::Insert => t("EINFÜGEN"),
        Mode::Replace => t("ERSETZEN"),
        Mode::Visual => t("VISUELL"),
        Mode::VisualLine => t("VISUELL ZEILE"),
        Mode::VisualBlock => t("VISUELL BLOCK"),
        Mode::Prompt => "",
    }
}

/// Adds the mode's label to the editor's status bar `stack`, hidden while the Vim mode is off.
pub fn status_items(mtm: MainThreadMarker, view: &EditorTextView, stack: &objc2_app_kit::NSStackView) {
    let label = crate::util::label(mtm, "");
    // SAFETY: A constant AppKit defines, read on the main thread.
    let weight = unsafe { objc2_app_kit::NSFontWeightSemibold };
    label.setFont(Some(&objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(
        ide_ui_common::style::SMALL,
        weight,
    )));
    label.setTextColor(Some(&NSColor::secondaryLabelColor()));
    label.setHidden(true);
    stack.addView_inGravity(&label, objc2_app_kit::NSStackViewGravity::Leading);
    view.ivars().vim.borrow_mut().label = Some(label);
}

/// Draws the block caret outside Insert mode, over the character under the caret.
pub fn draw_block(view: &EditorTextView) {
    if matches!(mode(view), None | Some(Mode::Insert)) {
        return;
    }
    let at = view.selectedRange();
    if at.length > 0 {
        return;
    }
    let length = view.document().text().len_utf16();
    let rect = if at.location < length {
        crate::lsp::rect_of(view, at.location..at.location + 1)
    } else {
        crate::lsp::rect_of(view, at.location..at.location)
    };
    let Some(mut rect) = rect else { return };
    if rect.size.width < 2.0 {
        rect.size.width = view.ivars().font_size.get() * 0.6;
    }
    NSColor::textColor().colorWithAlphaComponent(0.45).setFill();
    objc2_app_kit::NSBezierPath::fillRect(rect);
}

/// Opens the undo group of an insert pass, which closes on Esc, a lost focus, another document or a change from
/// outside (E24).
fn open_group(view: &EditorTextView) {
    if view.ivars().vim.borrow().grouping {
        return;
    }
    if let Some(manager) = view.undoManager() {
        manager.setGroupsByEvent(false);
        manager.beginUndoGrouping();
        view.ivars().vim.borrow_mut().grouping = true;
    }
}

fn close_group(view: &EditorTextView) {
    if !view.ivars().vim.borrow().grouping {
        return;
    }
    view.ivars().vim.borrow_mut().grouping = false;
    if let Some(manager) = view.undoManager() {
        if manager.groupingLevel() > 0 {
            manager.endUndoGrouping();
        }
        manager.setGroupsByEvent(true);
    }
}

/// The line and column in characters of UTF-16 offset `at` of `view`'s text.
fn line_column(view: &EditorTextView, at: usize) -> (usize, usize) {
    let document = view.document();
    let text = document.text();
    let byte = text.utf16_to_byte(at.min(text.len_utf16())).unwrap_or(0);
    let line = text.line_of(byte);
    let start = text.offset(line, 0, Unit::Utf8).unwrap_or(0);
    let column = text.to_string()[start..byte].chars().count();
    (line, column)
}

/// The UTF-16 offset of `(line, column)` in characters of `view`'s text, kept within the line.
fn utf16_of(view: &EditorTextView, (line, column): (usize, usize)) -> usize {
    let document = view.document();
    let text = document.text();
    let line = line.min(text.line_count().saturating_sub(1));
    let byte = text
        .offset(line, column, Unit::Utf32)
        .or_else(|_| text.offset(line, 0, Unit::Utf32))
        .unwrap_or(0);
    text.byte_to_utf16(byte).unwrap_or(0)
}

/// The key of the US layout at the place of `c` where a keyboard's layout is not Latin, so that `j` is `j` on a
/// Russian, Greek or Hebrew keyboard too; Latin characters stay.
fn ascii_key(c: char) -> char {
    const RUSSIAN: &str = "йцукенгшщзхъфывапролджэячсмитьбю";
    const US: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,.";
    const GREEK: &str = ";ςερτυθιοπ[]ασδφγηξκλ΄'ζχψωβνμ,.";
    const HEBREW: &str = "/'קראטוןםפ][שדגכעיחלךף,זסבהנמצתץ";
    let lower: Vec<char> = c.to_lowercase().collect();
    let Some(&lower) = lower.first() else { return c };
    for layout in [RUSSIAN, GREEK, HEBREW] {
        if let Some(index) = layout.chars().position(|k| k == lower) {
            let mapped = US.chars().nth(index).unwrap_or(c);
            return if c.is_uppercase() {
                mapped.to_ascii_uppercase()
            } else {
                mapped
            };
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::ascii_key;

    #[test]
    fn the_vim_mode_shadows_only_go_to_line_among_the_apps_shortcuts() {
        assert_eq!(super::shadowed(), ["edit.goToLine"]);
    }

    #[test]
    fn a_non_latin_layout_reads_as_the_us_keys_at_the_same_place() {
        assert_eq!(ascii_key('о'), 'j');
        assert_eq!(ascii_key('л'), 'k');
        assert_eq!(ascii_key('Ш'), 'I');
        assert_eq!(ascii_key('ξ'), 'j');
        assert_eq!(ascii_key('j'), 'j');
        assert_eq!(ascii_key('^'), '^');
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
