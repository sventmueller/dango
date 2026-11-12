//! Requests that lead across the project: implementations, type definitions and declarations of the symbol at a
//! position, the project's symbols by name, and the call and type hierarchies around a symbol.
//!
//! Hierarchy items and project symbols keep the server's own JSON beside their recounted ranges, so that what goes
//! back to the server (to expand an item or resolve a symbol) is exactly what it sent, in its own unit.

use lsp_types::{GotoDefinitionResponse, LocationLink, Position, Range, SymbolKind, Uri};
use serde_json::{Value, json};

use crate::capabilities;
use crate::convert::Positions;
use crate::error::Error;
use crate::features::{at, links};
use crate::server::{Mapper, Server, Ticket};
use crate::uri;

/// A symbol the project search found.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSymbol {
    /// Its name.
    pub name: String,
    /// What kind of symbol it is.
    pub kind: SymbolKind,
    /// The name of what contains it, such as a module or type, if the server said.
    pub container: Option<String>,
    /// Its document.
    pub uri: Uri,
    /// Its range in UTF-16; `None` until `resolve_project_symbol` fills it in, for servers that leave it out.
    pub range: Option<Range>,
    raw: Value,
}

/// Which way a call hierarchy goes from an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Calls {
    /// The functions that call the item.
    Incoming,
    /// The functions the item calls.
    Outgoing,
}

/// Which way a type hierarchy goes from an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Types {
    /// The types the item extends or implements.
    Supertypes,
    /// The types that extend or implement the item.
    Subtypes,
}

/// An entry of a call or type hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyItem {
    /// Its name.
    pub name: String,
    /// What kind of symbol it is.
    pub kind: SymbolKind,
    /// More about it, such as its signature, if the server said.
    pub detail: Option<String>,
    /// Its document.
    pub uri: Uri,
    /// All of it, in UTF-16.
    pub range: Range,
    /// Its name, in UTF-16; where a click leads.
    pub selection_range: Range,
    raw: Value,
}

/// A neighbour of a hierarchy item: a caller, a callee, a supertype or a subtype.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbour {
    /// The caller, callee or type.
    pub item: HierarchyItem,
    /// For calls, where they happen, in UTF-16: in the caller's document for incoming calls, in the asked item's for
    /// outgoing ones. Empty for types.
    pub calls: Vec<Range>,
    /// The document `calls` are in.
    pub calls_in: Uri,
}

impl Positions for ProjectSymbol {
    fn each(&mut self, _: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        let uri = self.uri.clone();
        self.range.each(&uri, f);
    }
}

impl Positions for HierarchyItem {
    fn each(&mut self, _: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        let uri = self.uri.clone();
        self.range.each(&uri, f);
        self.selection_range.each(&uri, f);
    }
}

impl Positions for Neighbour {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.item.each(uri, f);
        let calls_in = self.calls_in.clone();
        self.calls.each(&calls_in, f);
    }
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}

/// A symbol of a `workspace/symbol` answer, as `SymbolInformation` or `WorkspaceSymbol`; `None` if it is neither.
fn project_symbol(raw: Value) -> Option<ProjectSymbol> {
    let location = raw.get("location")?;
    Some(ProjectSymbol {
        name: raw.get("name")?.as_str()?.to_string(),
        kind: parse(raw.get("kind")?)?,
        container: raw.get("containerName").and_then(Value::as_str).map(str::to_string),
        uri: uri::normalize(&parse(location.get("uri")?)?),
        range: location.get("range").and_then(parse),
        raw,
    })
}

/// An item of a hierarchy answer; `None` if it lacks what an item has.
fn hierarchy_item(raw: Value) -> Option<HierarchyItem> {
    Some(HierarchyItem {
        name: raw.get("name")?.as_str()?.to_string(),
        kind: parse(raw.get("kind")?)?,
        detail: raw.get("detail").and_then(Value::as_str).map(str::to_string),
        uri: uri::normalize(&parse(raw.get("uri")?)?),
        range: parse(raw.get("range")?)?,
        selection_range: parse(raw.get("selectionRange")?)?,
        raw,
    })
}

fn items(answer: Option<Vec<Value>>, uri: &Uri, mapper: &mut Mapper) -> Result<Vec<HierarchyItem>, Error> {
    let mut items: Vec<HierarchyItem> = answer
        .unwrap_or_default()
        .into_iter()
        .filter_map(hierarchy_item)
        .collect();
    mapper.recount(uri, &mut items);
    Ok(items)
}

/// The neighbours in a calls answer: each entry names its item under `key` and its call ranges in `fromRanges`.
fn calls(
    answer: Option<Vec<Value>>,
    direction: Calls,
    asked: &Uri,
    mapper: &mut Mapper,
) -> Result<Vec<Neighbour>, Error> {
    let key = match direction {
        Calls::Incoming => "from",
        Calls::Outgoing => "to",
    };
    let mut found: Vec<Neighbour> = answer
        .unwrap_or_default()
        .into_iter()
        .filter_map(|mut call| {
            let item = hierarchy_item(call.get_mut(key)?.take())?;
            let calls: Vec<Range> = call.get("fromRanges").and_then(parse).unwrap_or_default();
            let calls_in = match direction {
                Calls::Incoming => item.uri.clone(),
                Calls::Outgoing => asked.clone(),
            };
            Some(Neighbour { item, calls, calls_in })
        })
        .collect();
    mapper.recount(asked, &mut found);
    Ok(found)
}

impl Server {
    fn goto(
        &self,
        method: &'static str,
        capability: &'static str,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<LocationLink>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            method,
            capability,
            uri,
            move |state, uri| at(state, uri, position),
            |answer: Option<GotoDefinitionResponse>, uri, mapper| links(answer, uri, mapper),
            Box::new(reply),
        )
    }

    /// Where the symbol at `position` is implemented, such as the implementations of a trait or interface.
    pub fn implementation(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<LocationLink>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.goto(
            "textDocument/implementation",
            "implementationProvider",
            uri,
            position,
            reply,
        )
    }

    /// Where the type of the symbol at `position` is defined.
    pub fn type_definition(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<LocationLink>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.goto(
            "textDocument/typeDefinition",
            "typeDefinitionProvider",
            uri,
            position,
            reply,
        )
    }

    /// Where the symbol at `position` is declared, for languages that tell declaration and definition apart.
    pub fn declaration(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<LocationLink>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.goto("textDocument/declaration", "declarationProvider", uri, position, reply)
    }

    /// The project's symbols that match `query`, as the server ranks them.
    pub fn project_symbols(
        &self,
        query: &str,
        reply: impl FnOnce(Result<Vec<ProjectSymbol>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let params = json!({"query": query});
        let inner = std::sync::Arc::downgrade(self.inner());
        self.call(
            "workspace/symbol",
            Some("workspaceSymbolProvider"),
            |_| Ok(params),
            Box::new(move |answer| {
                reply(answer.and_then(|value| {
                    let list = match value {
                        Value::Null => vec![],
                        Value::Array(list) => list,
                        other => return Err(Error::Malformed(format!("workspace/symbol answered {other}"))),
                    };
                    let mut symbols: Vec<ProjectSymbol> = list.into_iter().filter_map(project_symbol).collect();
                    let mut mapper = inner.upgrade().ok_or(Error::ServerGone)?.mapper();
                    let anywhere = uri::from_path(std::path::Path::new("/"));
                    mapper.recount(&anywhere, &mut symbols);
                    Ok(symbols)
                }));
            }),
        )
    }

    /// Whether the server can fill in the range of a project symbol it sent without one.
    pub fn resolves_project_symbols(&self) -> bool {
        capabilities::option(&self.inner().lock().raw, "workspaceSymbolProvider", "resolveProvider")
    }

    /// `symbol` with its range, for servers that leave the range out of the search; a symbol with a range comes back
    /// as it is.
    pub fn resolve_project_symbol(
        &self,
        symbol: ProjectSymbol,
        reply: impl FnOnce(Result<ProjectSymbol, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        if !self.resolves_project_symbols() {
            return Err(Error::Unsupported("workspaceSymbol/resolve"));
        }
        let inner = std::sync::Arc::downgrade(self.inner());
        let raw = symbol.raw.clone();
        self.call(
            "workspaceSymbol/resolve",
            Some("workspaceSymbolProvider"),
            |_| Ok(raw),
            Box::new(move |answer| {
                reply(answer.and_then(|value| {
                    let mut resolved = project_symbol(value)
                        .ok_or_else(|| Error::Malformed("workspaceSymbol/resolve answered no symbol".into()))?;
                    let mut mapper = inner.upgrade().ok_or(Error::ServerGone)?.mapper();
                    let uri = resolved.uri.clone();
                    mapper.recount(&uri, &mut resolved);
                    Ok(resolved)
                }));
            }),
        )
    }

    /// The call hierarchy items at `position`, usually one: the function there.
    pub fn prepare_call_hierarchy(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<HierarchyItem>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/prepareCallHierarchy",
            "callHierarchyProvider",
            uri,
            move |state, uri| at(state, uri, position),
            items,
            Box::new(reply),
        )
    }

    /// The callers or the callees of `item`, which a prepare or an earlier call of this gave.
    pub fn calls(
        &self,
        item: &HierarchyItem,
        direction: Calls,
        reply: impl FnOnce(Result<Vec<Neighbour>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let method = match direction {
            Calls::Incoming => "callHierarchy/incomingCalls",
            Calls::Outgoing => "callHierarchy/outgoingCalls",
        };
        self.neighbours(
            method,
            "callHierarchyProvider",
            item,
            move |answer, uri, mapper| calls(answer, direction, uri, mapper),
            reply,
        )
    }

    /// The type hierarchy items at `position`, usually one: the type there.
    pub fn prepare_type_hierarchy(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<HierarchyItem>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/prepareTypeHierarchy",
            "typeHierarchyProvider",
            uri,
            move |state, uri| at(state, uri, position),
            items,
            Box::new(reply),
        )
    }

    /// The supertypes or the subtypes of `item`, which a prepare or an earlier call of this gave.
    pub fn types(
        &self,
        item: &HierarchyItem,
        direction: Types,
        reply: impl FnOnce(Result<Vec<Neighbour>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let method = match direction {
            Types::Supertypes => "typeHierarchy/supertypes",
            Types::Subtypes => "typeHierarchy/subtypes",
        };
        self.neighbours(
            method,
            "typeHierarchyProvider",
            item,
            |answer, uri, mapper| {
                let types = items(answer, uri, mapper)?;
                Ok(types
                    .into_iter()
                    .map(|item| Neighbour {
                        calls_in: item.uri.clone(),
                        item,
                        calls: vec![],
                    })
                    .collect())
            },
            reply,
        )
    }

    /// Sends `method` with `item` as the server gave it and turns the answer into neighbours with `finish`.
    fn neighbours(
        &self,
        method: &'static str,
        capability: &'static str,
        item: &HierarchyItem,
        finish: impl FnOnce(Option<Vec<Value>>, &Uri, &mut Mapper) -> Result<Vec<Neighbour>, Error> + Send + 'static,
        reply: impl FnOnce(Result<Vec<Neighbour>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let params = json!({"item": item.raw});
        let asked = item.uri.clone();
        let inner = std::sync::Arc::downgrade(self.inner());
        self.call(
            method,
            Some(capability),
            |_| Ok(params),
            Box::new(move |answer| {
                reply(answer.and_then(|value| {
                    let list: Option<Vec<Value>> =
                        serde_json::from_value(value).map_err(|e| Error::Malformed(e.to_string()))?;
                    let mut mapper = inner.upgrade().ok_or(Error::ServerGone)?.mapper();
                    finish(list, &asked, &mut mapper)
                }));
            }),
        )
    }

    /// Whether the server offers the request behind `capability`, such as `callHierarchyProvider`, statically or by
    /// registering `method`.
    pub fn offers(&self, capability: &str, method: &str) -> bool {
        self.inner().lock().offers(capability, method)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_shapes_of_a_project_symbol_are_read_and_a_missing_range_is_left_for_resolve() {
        let information = json!({
            "name": "Server", "kind": 23, "containerName": "server",
            "location": {"uri": "file:///p/a.rs", "range": {"start": {"line": 3, "character": 4}, "end": {"line": 3, "character": 10}}},
        });
        let symbol = project_symbol(information).unwrap();
        assert_eq!((symbol.name.as_str(), symbol.kind), ("Server", SymbolKind::STRUCT));
        assert_eq!(symbol.container.as_deref(), Some("server"));
        assert_eq!(symbol.range.unwrap().start, Position::new(3, 4));
        let workspace = json!({"name": "run", "kind": 12, "location": {"uri": "file:///p/b.go"}, "data": 7});
        let symbol = project_symbol(workspace).unwrap();
        assert_eq!(symbol.range, None);
        assert_eq!(symbol.raw["data"], 7, "the server's data goes back to it on resolve");
        assert!(project_symbol(json!({"name": "x"})).is_none());
    }

    #[test]
    fn a_hierarchy_item_keeps_what_the_server_sent_even_where_lsp_types_reads_it_wrongly() {
        let r = json!({"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 3}});
        let raw =
            json!({"name": "T", "kind": 5, "tags": [1], "uri": "file:///p/t.ts", "range": r, "selectionRange": r});
        let item = hierarchy_item(raw.clone()).unwrap();
        assert_eq!(item.selection_range.end, Position::new(1, 3));
        assert_eq!(item.raw, raw);
    }
}
