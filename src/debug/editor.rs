//! The debugger in the editor (E94): the paused line drawn as a band with an arrow in the gutter, the values of the
//! paused frame's variables at the ends of the lines that use them (E81), values when the mouse rests on a name, and
//! breakpoints following the edits of their file.

use std::path::{Path, PathBuf};

use ide_document::{Change, Unit};
use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::NSColor;
use splat_i18n::tf;

use ide_ui_debug::DebugPanel;
use ide_ui_debug::model::breakpoints::Look;
use objc2_app_kit::NSMenu;

use crate::editor::EditorTextView;
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::window::WindowController;

/// How many lines above the paused one get the values of the variables they use.
const INLINE_LINES: usize = 30;
/// How long a value at a line's end may get before it is cut.
const INLINE_LENGTH: usize = 60;

/// The debugger of `editor`'s window and the file the editor shows.
pub fn panel_and_path(editor: &EditorTextView) -> Option<(Retained<DebugPanel>, PathBuf)> {
    let controller = editor.controller()?;
    let path = editor.document().path()?.to_path_buf();
    Some((ide_ui_debug::of(controller.handle())?, path))
}

/// The editors of `controller`'s window.
fn editors_of(controller: &WindowController) -> Vec<Retained<EditorTextView>> {
    crate::editor::all_editors()
        .into_iter()
        .filter(|editor| editor.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
        .collect()
}

/// Shows the debugger's breakpoints, paused line and values in every editor of `controller`'s window.
pub fn refresh_editors(controller: &WindowController) {
    let Some(panel) = ide_ui_debug::of(controller.handle()) else {
        return;
    };
    for editor in editors_of(controller) {
        show_values(&panel, &editor);
        crate::editor::redraw_gutter(&editor);
        crate::editor::follow_overlay(&editor);
    }
}

/// Closes the value popovers of `controller`'s editors, which speak of a frame that is gone once the program runs on.
pub fn forget_values(controller: &WindowController) {
    for editor in editors_of(controller) {
        ide_ui_lsp::dismiss_hover(&*editor);
    }
}

/// A click on the line numbers of `editor` at `line`: sets or clears a breakpoint there; whether the editor has a
/// file and a debugger to keep it.
pub fn gutter_click(editor: &EditorTextView, line: usize) -> bool {
    let (Some(controller), Some(path)) = (editor.controller(), editor.document().path().map(Path::to_path_buf)) else {
        return false;
    };
    ide_ui_debug::gutter::click(controller.handle(), &path, line)
}

/// The context menu of the line numbers of `editor` at `line`.
pub fn gutter_menu(editor: &EditorTextView, line: usize) -> Option<Retained<NSMenu>> {
    let controller = editor.controller()?;
    let path = editor.document().path()?.to_path_buf();
    ide_ui_debug::gutter::menu(controller.handle(), &path, line)
}

/// How the line `line` of `editor` is marked in the gutter: its breakpoint, and whether the program is paused there.
pub fn gutter_mark(editor: &EditorTextView, line: usize) -> (Option<Look>, bool) {
    let (Some(controller), Some(path)) = (editor.controller(), editor.document().path().map(Path::to_path_buf)) else {
        return (None, false);
    };
    ide_ui_debug::gutter::mark(controller.handle(), &path, line)
}

/// How the lines of an editor are marked in the gutter.
#[derive(Default)]
pub struct GutterMarks {
    looks: std::collections::HashMap<usize, Look>,
    paused: Option<usize>,
}

impl GutterMarks {
    /// The breakpoint of `line`, and whether the program is paused there.
    pub fn at(&self, line: usize) -> (Option<Look>, bool) {
        (self.looks.get(&line).copied(), self.paused == Some(line))
    }
}

/// How the lines of `editor` are marked in the gutter, for drawing all lines on screen at once.
pub fn gutter_marks(editor: &EditorTextView) -> GutterMarks {
    let Some(controller) = editor.controller() else {
        return GutterMarks::default();
    };
    let document = editor.document();
    let Some(path) = document.path() else {
        return GutterMarks::default();
    };
    let (looks, paused) = ide_ui_debug::gutter::marks(controller.handle(), path);
    GutterMarks { looks, paused }
}

/// An edit of `editor`'s text moves the breakpoints of its file.
pub fn edited(editor: &EditorTextView, change: &Change) {
    if let Some((panel, path)) = panel_and_path(editor) {
        panel.text_changed(&path, change);
    }
}

/// The line the program is paused at in `editor`, if it shows the paused frame's file.
pub fn paused_line(editor: &EditorTextView) -> Option<usize> {
    let (panel, path) = panel_and_path(editor)?;
    let model = panel.read();
    let frame = model.paused_at()?;
    (frame.path.as_deref() == Some(&path)).then_some(frame.line)
}

/// Draws the paused line's band in `editor`'s coordinates, under the overlay's other marks; the gutter's arrow says
/// the same without colour.
pub fn draw_paused_line(editor: &EditorTextView) {
    let Some(rect) = paused_line(editor).and_then(|line| crate::editor::laid_out_line_rect(editor, line)) else {
        return;
    };
    NSColor::systemGreenColor().colorWithAlphaComponent(0.16).setFill();
    objc2_app_kit::NSRectFillUsingOperation(rect, objc2_app_kit::NSCompositingOperation::SourceOver);
}

/// The values of the paused frame's variables at the ends of the lines above the paused one that use them, in the
/// editor that shows its file; other editors lose theirs.
fn show_values(panel: &DebugPanel, editor: &EditorTextView) {
    let wanted = {
        let model = panel.read();
        let path = editor.document().path().map(std::path::Path::to_path_buf);
        match model.paused_at() {
            Some(frame) if frame.path.is_some() && frame.path == path => Some((frame.line, model.variables.locals())),
            _ => None,
        }
    };
    let Some((line, locals)) = wanted.filter(|(_, locals)| !locals.is_empty()) else {
        if !insets::of(editor, Layer::Debug).is_empty() {
            insets::clear(editor, Layer::Debug);
        }
        return;
    };
    let shown = values_at_line_ends(editor, line, &locals);
    insets::set(editor, Layer::Debug, shown);
}

fn values_at_line_ends(editor: &EditorTextView, paused: usize, locals: &[(String, String)]) -> Vec<Inset> {
    let document = editor.document();
    let text = document.text();
    let mut insets = Vec::new();
    for line in paused.saturating_sub(INLINE_LINES)..=paused {
        let Some(content) = text.line(line) else { continue };
        let used: Vec<String> = locals
            .iter()
            .filter(|(name, _)| uses(&content, name))
            .map(|(name, value)| format!("{name} = {}", shorten(value)))
            .collect();
        if used.is_empty() {
            continue;
        }
        let Ok(start) = text.offset(line, 0, Unit::Utf16) else {
            continue;
        };
        let Ok(at) = text.byte_to_utf16(start) else { continue };
        let mut inset = Inset::new(at, used.join(", "), Place::LineEnd);
        inset.tooltip = Some(tf("Werte in Zeile {}", &[&(line + 1).to_string()]));
        insets.push(inset);
    }
    insets
}

/// Whether `line` uses `name` as a word of its own.
fn uses(line: &str, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(name).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + name.len()..].chars().next();
        before.is_none_or(|c| !word(c)) && after.is_none_or(|c| !word(c))
    })
}

fn shorten(value: &str) -> String {
    let flat: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= INLINE_LENGTH {
        return flat;
    }
    let cut: String = flat.chars().take(INLINE_LENGTH).collect();
    format!("{cut}…")
}

/// The expression under the mouse at UTF-16 offset `index` of `editor`: the name there with the field accesses in front
/// of it, such as `self.items`, and its range.
pub fn expression_at(editor: &EditorTextView, index: usize) -> Option<(String, std::ops::Range<usize>)> {
    let document = editor.document();
    let text = document.text();
    let byte = text.utf16_to_byte(index).ok()?;
    let line = text.line_of(byte);
    let start = text.offset(line, 0, Unit::Utf16).ok()?;
    let content = text.line(line)?;
    let column = byte - start;
    let (from, to) = expression_bounds(&content, column)?;
    let range = text.byte_to_utf16(start + from).ok()?..text.byte_to_utf16(start + to).ok()?;
    Some((content[from..to].to_string(), range))
}

/// The bytes of `line` that make the expression around `column`: a name, with `.name` parts before it.
fn expression_bounds(line: &str, column: usize) -> Option<(usize, usize)> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    if !line.is_char_boundary(column) || !line[column..].chars().next().is_some_and(word) {
        return None;
    }
    let to = column + line[column..].find(|c: char| !word(c)).unwrap_or(line.len() - column);
    let from = line[..column]
        .rfind(|c: char| !(word(c) || c == '.'))
        .map_or(0, |at| at + 1);
    let from = from + line[from..].find(|c: char| c != '.').unwrap_or(0);
    let expression = &line[from..to];
    (!expression.is_empty() && !expression.starts_with(|c: char| c.is_ascii_digit())).then_some((from, to))
}

/// The mouse rested on `index` of `editor`: while the program is paused, the value of what is there shows in the
/// hover popover; whether the debugger took the hover.
pub fn hover(editor: &EditorTextView, index: usize) -> bool {
    let Some((panel, _)) = panel_and_path(editor) else {
        return false;
    };
    if panel.read().frame().is_none() {
        return false;
    }
    let Some((expression, range)) = expression_at(editor, index) else {
        return false;
    };
    let serial = editor.ivars().serial;
    panel.evaluate_for_hover(&expression.clone(), move |value| {
        if let Some(editor) = crate::editor::find_editor(serial) {
            ide_ui_lsp::show_hover(&*editor, range, &format!("`{expression}` = `{value}`"));
        }
    })
}

/// Whether `editor` shows a file while its window's debugger is paused, so the mouse resting on a name asks for its
/// value.
pub fn paused_in(editor: &EditorTextView) -> bool {
    panel_and_path(editor).is_some_and(|(panel, _)| panel.read().frame().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_variable_counts_only_as_a_word_of_its_own() {
        assert!(uses("let total = n + 1;", "n"));
        assert!(uses("n", "n"));
        assert!(!uses("let name = 1;", "n"));
        assert!(!uses("let x_n = 1;", "n"));
        assert!(uses("println!(\"{total}\")", "total"));
    }

    #[test]
    fn the_expression_under_the_mouse_takes_its_fields_along() {
        let line = "    let x = self.items.len() + count;";
        let at = |needle: &str| line.find(needle).unwrap();
        let bounds = |column| expression_bounds(line, column).map(|(from, to)| &line[from..to]);
        assert_eq!(bounds(at("items") + 2), Some("self.items"));
        assert_eq!(bounds(at("count")), Some("count"));
        assert_eq!(bounds(at("self")), Some("self"));
        assert_eq!(bounds(at("=")), None);
        assert_eq!(
            expression_bounds("x = 42", 4),
            None,
            "a number is no expression to watch"
        );
    }

    #[test]
    fn long_values_are_cut_and_flattened() {
        assert_eq!(shorten("Point {\n    x: 1,\n}"), "Point { x: 1, }");
        let long = "a".repeat(INLINE_LENGTH + 5);
        assert!(shorten(&long).ends_with('…'));
        assert_eq!(shorten(&long).chars().count(), INLINE_LENGTH + 1);
    }
}
