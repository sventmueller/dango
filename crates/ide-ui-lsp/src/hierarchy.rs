//! The call and type hierarchies of the symbol at the cursor (E85), which the search panel shows as a tree; its
//! children are asked for only when the tree opens an entry.

use std::path::PathBuf;

use ide_lsp::lsp_types::{Range, SymbolKind};
use ide_lsp::{Calls, HierarchyItem, Neighbour, Server, Ticket, Types};

use super::{server_of, text};
use crate::editor::Editor;
use crate::host as editor;
use ide_ui_common::util::main_soon;

/// Which hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub enum HierarchyKind {
    Calls,
    Types,
}

/// Which way a hierarchy goes from its root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub enum Direction {
    Callers,
    Callees,
    Supertypes,
    Subtypes,
}

impl HierarchyKind {
    /// The two ways the hierarchy can go, the one it starts with first.
    pub fn directions(self) -> [Direction; 2] {
        match self {
            HierarchyKind::Calls => [Direction::Callers, Direction::Callees],
            HierarchyKind::Types => [Direction::Supertypes, Direction::Subtypes],
        }
    }
}

/// The hierarchy around a symbol: the items the server prepared and the server that knows their neighbours.
#[derive(Clone)]
pub struct Hierarchy {
    /// Whether it is the hierarchy of calls or of types.
    pub kind: HierarchyKind,
    /// The items the server prepared for the symbol.
    pub roots: Vec<HierarchyItem>,
    /// The window folder whose editors open the places.
    pub folder: PathBuf,
    server: Server,
}

impl Hierarchy {
    /// Asks for the neighbours of `item` in `direction`; `done` hears them on the main thread.
    pub fn neighbours(
        &self,
        item: &HierarchyItem,
        direction: Direction,
        done: impl FnOnce(Vec<Neighbour>) + Send + 'static,
    ) -> Option<Ticket> {
        let reply = move |answer: Result<Vec<Neighbour>, ide_lsp::Error>| {
            if let Err(error) = &answer
                && !matches!(error, ide_lsp::Error::Cancelled)
            {
                ide_ui_common::diag::report("asking for a hierarchy", error);
            }
            let found = answer.unwrap_or_default();
            main_soon(move || done(found));
        };
        let asked = match direction {
            Direction::Callers => self.server.calls(item, Calls::Incoming, reply),
            Direction::Callees => self.server.calls(item, Calls::Outgoing, reply),
            Direction::Supertypes => self.server.types(item, Types::Supertypes, reply),
            Direction::Subtypes => self.server.types(item, Types::Subtypes, reply),
        };
        asked.ok()
    }

    /// Opens `path` with `range` selected in the window of the hierarchy's folder.
    pub fn open(&self, path: &std::path::Path, range: Range) {
        super::navigate::show(&self.folder, path, Some(range));
    }
}

/// Shows the call hierarchy of the function at the cursor in the search panel.
pub fn call_hierarchy(view: &dyn Editor) {
    prepare(view, HierarchyKind::Calls);
}

/// Shows the type hierarchy of the type at the cursor in the search panel.
pub fn type_hierarchy(view: &dyn Editor) {
    prepare(view, HierarchyKind::Types);
}

/// Asks the server of `view` for the hierarchy of `kind` around the symbol at the cursor; the search panel shows it.
fn prepare(view: &dyn Editor, kind: HierarchyKind) {
    let Some((server, uri)) = server_of(&editor::state(), view) else {
        return;
    };
    let Some(folder) = view.project_root() else {
        return;
    };
    let Some(position) = text::position(view.document().text(), view.selectedRange().location) else {
        return;
    };
    let serial = view.lsp().borrow().serial;
    let asking = server.clone();
    let reply = move |answer: Result<Vec<HierarchyItem>, ide_lsp::Error>| {
        let roots = answer.unwrap_or_default();
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            if roots.is_empty() {
                objc2_app_kit::NSBeep();
                return;
            }
            if let Some(window) = view.window_id() {
                editor::show_hierarchy(
                    window,
                    Hierarchy {
                        kind,
                        roots,
                        folder,
                        server: asking,
                    },
                );
            }
        });
    };
    let asked = match kind {
        HierarchyKind::Calls => server.prepare_call_hierarchy(&uri, position, reply),
        HierarchyKind::Types => server.prepare_type_hierarchy(&uri, position, reply),
    };
    if asked.is_err() {
        objc2_app_kit::NSBeep();
    }
}

/// The SF Symbol for a kind of symbol, which only decorates its name.
pub fn kind_symbol(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR | SymbolKind::OPERATOR => "function",
        SymbolKind::STRUCT | SymbolKind::CLASS | SymbolKind::OBJECT => "cube",
        SymbolKind::INTERFACE => "square.on.square.dashed",
        SymbolKind::ENUM | SymbolKind::ENUM_MEMBER => "list.bullet",
        SymbolKind::MODULE | SymbolKind::NAMESPACE | SymbolKind::PACKAGE | SymbolKind::FILE => "shippingbox",
        SymbolKind::TYPE_PARAMETER => "t.square",
        SymbolKind::FIELD | SymbolKind::PROPERTY => "rectangle.split.3x1",
        _ => "character",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_of_symbol_has_its_picture_and_each_hierarchy_its_two_ways() {
        let cases = [
            (SymbolKind::OPERATOR, "function"),
            (SymbolKind::OBJECT, "cube"),
            (SymbolKind::INTERFACE, "square.on.square.dashed"),
            (SymbolKind::ENUM_MEMBER, "list.bullet"),
            (SymbolKind::FILE, "shippingbox"),
            (SymbolKind::TYPE_PARAMETER, "t.square"),
            (SymbolKind::PROPERTY, "rectangle.split.3x1"),
            (SymbolKind::STRING, "character"),
        ];
        for (kind, symbol) in cases {
            assert_eq!(kind_symbol(kind), symbol, "{kind:?}");
        }
        assert_eq!(
            HierarchyKind::Calls.directions(),
            [Direction::Callers, Direction::Callees]
        );
        assert_eq!(
            HierarchyKind::Types.directions(),
            [Direction::Supertypes, Direction::Subtypes]
        );
    }
}
