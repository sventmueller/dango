//! The editor as the language servers reach it: a text view with its document, its window's folder and the state
//! this module keeps for it, and the few things of the editor's own that a server's answer changes.

use ide_ui_common::window::WindowId;
use std::cell::{Ref, RefCell, RefMut};
use std::ops::{Deref, Range};
use std::path::PathBuf;

use ide_document::{Change, Document};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2_app_kit::{NSStackView, NSTextStorage, NSTextView};

use super::ViewState;

/// An editor; it is the text view it shows its document in.
pub trait Editor: Deref<Target = NSTextView> {
    /// Tells this editor apart from every other one in callbacks that cannot hold a view.
    fn serial(&self) -> crate::EditorSerial;
    /// What the language servers keep about this editor.
    fn lsp(&self) -> &RefCell<ViewState>;
    fn document(&self) -> Ref<'_, Document>;
    fn document_mut(&self) -> RefMut<'_, Document>;
    /// The folder of the editor's window, if it shows one.
    fn project_root(&self) -> Option<PathBuf>;
    /// The editor's window, by its id.
    fn window_id(&self) -> Option<WindowId>;
    /// Sends the editor `selector` after `delay` seconds, once however often it is asked meanwhile.
    fn later(&self, selector: Sel, delay: f64);
    fn storage(&self) -> Option<Retained<NSTextStorage>>;
    /// Makes `changes` of the document in the text view, as one undo step.
    fn apply_model_changes(&self, changes: &[Change]);
    /// Marks the match the search panel revealed again, which painting colours took off.
    fn paint_found(&self);
    /// Gives the UTF-16 range `range` the editor's own colours again, which the server's then paint over.
    fn paint_under_server(&self, range: &Range<usize>);
    /// Draws the line numbers anew, which show the diagnostics.
    fn redraw_gutter(&self);
    /// The diagnostics changed; the line ends show them anew.
    fn diagnostics_changed(&self);
    /// Draws the minimap anew, which shows the diagnostics and colours.
    fn redraw_minimap(&self);
    /// The UTF-16 range of the lines on screen, with a margin.
    fn visible_utf16(&self) -> Range<usize>;
    /// Whether a fold hides the UTF-16 offset `offset`.
    fn hidden_by_fold(&self, offset: usize) -> bool;
    /// The server's folding ranges, as first line and last hidden line, or `None` for the indentation's.
    fn set_server_folds(&self, ranges: Option<Vec<(usize, usize)>>);
    /// Saves the document as ⌘S does, once formatting is done.
    fn save_soon(&self);
    /// The document's file moved; tab, title and colours follow.
    fn path_changed(&self);
    /// The indentation a formatter should keep: its width and whether it uses spaces.
    fn formatting_options(&self) -> (u32, bool);
    /// Lays the overlay of fold placeholders and further carets out again.
    fn follow_overlay(&self);
    /// Adds the editor's own items, such as its indentation, to the status bar `stack`.
    fn status_items(&self, stack: &NSStackView);
    /// Shows `hints` as the editor's inlay hints; those at offsets outside `kept` stay when `kept` is given.
    fn set_inlay_hints(&self, hints: Vec<Hint>, kept: Option<Range<usize>>);
    /// Takes the inlay hints out.
    fn clear_inlay_hints(&self);
    /// Whether a debugger stopped in the program of the editor's file, which then shows values on hover.
    fn debugger_paused(&self) -> bool;
    /// Shows the debugger's value of the expression at the UTF-16 offset `index`; whether it did.
    fn debugger_hover(&self, index: usize) -> bool;
}

/// The marker of the main thread, on which every editor lives.
pub fn marker(editor: &dyn Editor) -> objc2::MainThreadMarker {
    let view: &NSTextView = editor;
    objc2::MainThreadOnly::mtm(view)
}

/// An inlay hint as the editor shows it.
pub struct Hint {
    /// The UTF-16 offset it belongs to.
    pub at: usize,
    pub text: String,
    pub tooltip: Option<String>,
    /// Room for a space before or after it, as the server asks.
    pub pad_left: bool,
    pub pad_right: bool,
    /// Where a click on it goes: a file and a range in it.
    pub target: Option<(PathBuf, ide_lsp::lsp_types::Range)>,
}
