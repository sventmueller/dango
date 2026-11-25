//! The structure the server sees in a document: the symbols around the cursor, shown as breadcrumbs in the status bar,
//! and the ranges that fold.

use std::ops::Range;

use ide_document::Change;
use ide_lsp::Server;
use ide_lsp::lsp_types::{DocumentSymbol, Uri};
use objc2::sel;

use super::ViewState;
use crate::editor::Editor;
use crate::host as editor;
use ide_ui_common::util::main_soon;

/// A symbol of the document and where it is, in UTF-16 code units.
#[derive(Debug, Clone)]
pub(crate) struct Crumb {
    range: Range<usize>,
    /// Where the symbol's name starts, the line a sticky header shows.
    head: Range<usize>,
    name: String,
}

/// The symbols as a list in which every symbol follows the one it is inside.
fn flatten(view: &dyn Editor, symbols: &[DocumentSymbol], out: &mut Vec<Crumb>) {
    for symbol in symbols {
        let range = super::text::range(view.document().text(), symbol.range);
        let head = super::text::range(view.document().text(), symbol.selection_range);
        out.push(Crumb {
            range,
            head,
            name: symbol.name.clone(),
        });
        if let Some(children) = &symbol.children {
            flatten(view, children, out);
        }
    }
}

/// Asks the server for the document's symbols and folding ranges.
pub(crate) fn refresh(view: &dyn Editor, server: &Server, uri: &Uri) {
    let serial = view.lsp().borrow().serial;
    let revision = view.document().revision();
    let _ = server.document_symbols(uri, move |answer| {
        let Ok(symbols) = answer else { return };
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            if view.document().revision() != revision {
                return;
            }
            let mut crumbs = vec![];
            flatten(&*view, &symbols, &mut crumbs);
            view.lsp().borrow_mut().crumbs = crumbs;
            super::status::refresh(&*view);
            view.redraw_gutter();
        });
    });
    let started = server.folding_ranges(uri, move |answer| {
        let Ok(ranges) = answer else { return };
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            let lines = ranges
                .iter()
                .filter_map(|r| Some((usize::try_from(r.start_line).ok()?, usize::try_from(r.end_line).ok()?)))
                .collect();
            view.set_server_folds(Some(lines));
        });
    });
    if started.is_err() {
        view.set_server_folds(None);
    }
}

/// Keeps the symbols on their text while it changes, until the server sends new ones.
pub(crate) fn shift(state: &mut ViewState, change: &Change) {
    for crumb in &mut state.crumbs {
        super::decorate::follow(&mut crumb.range, change);
        super::decorate::follow(&mut crumb.head, change);
    }
}

/// The cursor moved: the breadcrumbs follow once it rests.
pub(crate) fn selection_moved(view: &dyn Editor) {
    if !view.lsp().borrow().crumbs.is_empty() {
        view.later(sel!(ideBreadcrumbs), 0.05);
    }
}

/// The names of the symbols around the cursor, outermost first.
pub(crate) fn breadcrumbs(view: &dyn Editor) -> Vec<String> {
    let cursor = view.selectedRange().location;
    view.lsp()
        .borrow()
        .crumbs
        .iter()
        .filter(|c| c.range.start <= cursor && cursor <= c.range.end)
        .map(|c| c.name.clone())
        .collect()
}

/// The symbols of the document as the line of each name and the line its range ends on, in UTF-16 offsets turned into
/// lines; `None` while the server sent none.
pub fn scopes(view: &dyn Editor) -> Option<Vec<(usize, usize)>> {
    let state = view.lsp().borrow();
    if state.crumbs.is_empty() {
        return None;
    }
    let document = view.document();
    let text = document.text();
    let line = |offset: usize| {
        text.utf16_to_byte(offset.min(text.len_utf16()))
            .ok()
            .map(|b| text.line_of(b))
    };
    Some(
        state
            .crumbs
            .iter()
            .filter_map(|c| Some((line(c.head.start)?, line(c.range.end)?)))
            .collect(),
    )
}
