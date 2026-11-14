//! The requests the editor makes: each checks that the server offers it, recounts positions to the server's unit
//! and the answer back to UTF-16, and puts the answer into one shape where the protocol allows several.

use std::sync::Arc;

use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CompletionContext, CompletionItem, CompletionResponse,
    CompletionTextEdit, Diagnostic, DocumentDiagnosticReport, DocumentDiagnosticReportKind,
    DocumentDiagnosticReportResult, DocumentSymbol, DocumentSymbolResponse, FoldingRange, FormattingOptions,
    GotoDefinitionResponse, HoverContents, InlayHint, InsertTextFormat, Location, LocationLink, MarkedString,
    MarkupKind, Position, PrepareRenameResponse, Range, SignatureHelp, SignatureHelpContext, SymbolInformation,
    TextEdit, Uri, WorkspaceEdit,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::capabilities;
use crate::error::Error;
use crate::semantic::{self, Legend, SemanticTokens};
use crate::server::{Event, Mapper, Server, State, Status, Ticket};
use crate::snippet::{self, Snippet};
use crate::uri;

/// The requests this module sends, for error messages.
pub(crate) const METHODS: &[&str] = &[
    "textDocument/hover",
    "textDocument/completion",
    "completionItem/resolve",
    "textDocument/signatureHelp",
    "textDocument/definition",
    "textDocument/references",
    "textDocument/prepareRename",
    "textDocument/rename",
    "textDocument/formatting",
    "textDocument/rangeFormatting",
    "textDocument/codeAction",
    "codeAction/resolve",
    "workspace/executeCommand",
    "textDocument/semanticTokens/full",
    "textDocument/foldingRange",
    "textDocument/documentSymbol",
    "textDocument/diagnostic",
    "textDocument/inlayHint",
    "textDocument/implementation",
    "textDocument/typeDefinition",
    "textDocument/declaration",
    "workspace/symbol",
    "workspaceSymbol/resolve",
    "textDocument/prepareCallHierarchy",
    "callHierarchy/incomingCalls",
    "callHierarchy/outgoingCalls",
    "textDocument/prepareTypeHierarchy",
    "typeHierarchy/supertypes",
    "typeHierarchy/subtypes",
];

/// Hover text as Markdown, with the range it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// The text; plain text from the server is escaped so it reads the same as Markdown.
    pub markdown: String,
    /// The range the text is about.
    pub range: Option<Range>,
}

/// The completion items at a position.
#[derive(Debug, Clone, PartialEq)]
pub struct Completions {
    /// Whether typing further should ask again instead of filtering these.
    pub incomplete: bool,
    /// With UTF-16 ranges.
    pub items: Vec<CompletionItem>,
}

/// What choosing a completion item inserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insertion {
    /// The text and where the cursor goes after it.
    pub snippet: Snippet,
    /// The range to replace when inserting; `None` means the word before the cursor, as the editor sees it.
    pub insert: Option<Range>,
    /// The range to replace when the user asks to overwrite; the same as `insert` unless the server gave both.
    pub replace: Option<Range>,
}

/// What choosing `item` inserts.
pub fn insertion(item: &CompletionItem) -> Insertion {
    let (text, insert, replace) = match &item.text_edit {
        Some(CompletionTextEdit::Edit(edit)) => (edit.new_text.as_str(), Some(edit.range), Some(edit.range)),
        Some(CompletionTextEdit::InsertAndReplace(edit)) => {
            (edit.new_text.as_str(), Some(edit.insert), Some(edit.replace))
        }
        None => (item.insert_text.as_deref().unwrap_or(&item.label), None, None),
    };
    let snippet = if item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
        snippet::parse(text)
    } else {
        let end = u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX);
        Snippet {
            text: text.to_string(),
            tabstops: vec![snippet::Tabstop {
                index: 0,
                spans: vec![snippet::Span::new(end, end)],
                choices: vec![],
            }],
        }
    };
    Insertion {
        snippet,
        insert,
        replace,
    }
}

type Done<T> = Box<dyn FnOnce(Result<T, Error>) + Send>;

impl Server {
    /// Sends `method` about `uri` if the server offers `capability`; `finish` turns the parsed answer into what the
    /// caller gets.
    pub(crate) fn ask<R: DeserializeOwned, T: Send + 'static>(
        &self,
        method: &'static str,
        capability: &'static str,
        uri: &Uri,
        params: impl FnOnce(&State, &Uri) -> Result<Value, Error>,
        finish: impl FnOnce(R, &Uri, &mut Mapper) -> Result<T, Error> + Send + 'static,
        reply: Done<T>,
    ) -> Result<Ticket, Error> {
        let uri = uri::normalize(uri);
        let inner = Arc::downgrade(self.inner());
        let doc = uri.clone();
        self.call(
            method,
            Some(capability),
            |state| params(state, &uri),
            Box::new(move |answer| {
                reply(answer.and_then(|value| {
                    let parsed: R = serde_json::from_value(value).map_err(|e| Error::Malformed(e.to_string()))?;
                    let mut mapper = inner.upgrade().ok_or(Error::ServerGone)?.mapper();
                    finish(parsed, &doc, &mut mapper)
                }));
            }),
        )
    }

    /// The hover text at `position`.
    pub fn hover(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Option<Hover>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/hover",
            "hoverProvider",
            uri,
            move |state, uri| at(state, uri, position),
            |hover: Option<lsp_types::Hover>, uri, mapper| {
                Ok(hover.map(|hover| {
                    let mut range = hover.range;
                    mapper.recount(uri, &mut range);
                    Hover {
                        markdown: markdown(hover.contents),
                        range,
                    }
                }))
            },
            Box::new(reply),
        )
    }

    /// The completion items at `position`; `context` says what triggered the request.
    pub fn completion(
        &self,
        uri: &Uri,
        position: Position,
        context: Option<CompletionContext>,
        reply: impl FnOnce(Result<Completions, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/completion",
            "completionProvider",
            uri,
            move |state, uri| {
                let mut params = at(state, uri, position)?;
                if let Some(context) = context {
                    params["context"] = json!(context);
                }
                Ok(params)
            },
            |answer: Option<CompletionResponse>, uri, mapper| {
                let (incomplete, mut items) = match answer {
                    None => (false, vec![]),
                    Some(CompletionResponse::Array(items)) => (false, items),
                    Some(CompletionResponse::List(list)) => (list.is_incomplete, list.items),
                };
                mapper.recount(uri, &mut items);
                Ok(Completions { incomplete, items })
            },
            Box::new(reply),
        )
    }

    /// `item` with the details the server leaves out of the list, such as its documentation.
    pub fn resolve_completion(
        &self,
        uri: &Uri,
        item: CompletionItem,
        reply: impl FnOnce(Result<CompletionItem, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "completionItem/resolve",
            "completionProvider",
            uri,
            move |state, uri| {
                if !capabilities::option(&state.raw, "completionProvider", "resolveProvider") {
                    return Err(Error::Unsupported("completionItem/resolve"));
                }
                Ok(json!(state.positions_to_server(uri, item)))
            },
            |mut item: CompletionItem, uri, mapper| {
                mapper.recount(uri, &mut item);
                Ok(item)
            },
            Box::new(reply),
        )
    }

    /// The signatures of the call around `position`.
    pub fn signature_help(
        &self,
        uri: &Uri,
        position: Position,
        context: Option<SignatureHelpContext>,
        reply: impl FnOnce(Result<Option<SignatureHelp>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/signatureHelp",
            "signatureHelpProvider",
            uri,
            move |state, uri| {
                let mut params = at(state, uri, position)?;
                if let Some(context) = context {
                    params["context"] = json!(context);
                }
                Ok(params)
            },
            |help: Option<SignatureHelp>, _, _| Ok(help),
            Box::new(reply),
        )
    }

    /// Where the symbol at `position` is defined.
    pub fn definition(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Vec<LocationLink>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/definition",
            "definitionProvider",
            uri,
            move |state, uri| at(state, uri, position),
            links,
            Box::new(reply),
        )
    }

    /// Where the symbol at `position` is used.
    pub fn references(
        &self,
        uri: &Uri,
        position: Position,
        include_declaration: bool,
        reply: impl FnOnce(Result<Vec<Location>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/references",
            "referencesProvider",
            uri,
            move |state, uri| {
                let mut params = at(state, uri, position)?;
                params["context"] = json!({"includeDeclaration": include_declaration});
                Ok(params)
            },
            |answer: Option<Vec<Location>>, uri, mapper| {
                let mut locations: Vec<Location> = answer
                    .unwrap_or_default()
                    .into_iter()
                    .map(|l| Location::new(uri::normalize(&l.uri), l.range))
                    .collect();
                mapper.recount(uri, &mut locations);
                Ok(locations)
            },
            Box::new(reply),
        )
    }

    /// Whether the symbol at `position` can be renamed, with its range and a suggested name; `None` means it cannot.
    pub fn prepare_rename(
        &self,
        uri: &Uri,
        position: Position,
        reply: impl FnOnce(Result<Option<PrepareRenameResponse>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/prepareRename",
            "renameProvider",
            uri,
            move |state, uri| {
                if !capabilities::option(&state.raw, "renameProvider", "prepareProvider") {
                    return Err(Error::Unsupported("textDocument/prepareRename"));
                }
                at(state, uri, position)
            },
            |mut answer: Option<PrepareRenameResponse>, uri, mapper| {
                mapper.recount(uri, &mut answer);
                Ok(answer)
            },
            Box::new(reply),
        )
    }

    /// The edits that rename the symbol at `position` to `new_name`.
    pub fn rename(
        &self,
        uri: &Uri,
        position: Position,
        new_name: &str,
        reply: impl FnOnce(Result<Option<WorkspaceEdit>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let new_name = new_name.to_string();
        self.ask(
            "textDocument/rename",
            "renameProvider",
            uri,
            move |state, uri| {
                let mut params = at(state, uri, position)?;
                params["newName"] = Value::String(new_name);
                Ok(params)
            },
            |mut edit: Option<WorkspaceEdit>, uri, mapper| {
                mapper.recount(uri, &mut edit);
                Ok(edit)
            },
            Box::new(reply),
        )
    }

    /// The edits that format the whole document.
    pub fn format(
        &self,
        uri: &Uri,
        options: FormattingOptions,
        reply: impl FnOnce(Result<Vec<TextEdit>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/formatting",
            "documentFormattingProvider",
            uri,
            move |state, uri| {
                state.doc(uri)?;
                Ok(json!({"textDocument": {"uri": uri}, "options": options}))
            },
            edits,
            Box::new(reply),
        )
    }

    /// The edits that format `range`.
    pub fn format_range(
        &self,
        uri: &Uri,
        range: Range,
        options: FormattingOptions,
        reply: impl FnOnce(Result<Vec<TextEdit>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/rangeFormatting",
            "documentRangeFormattingProvider",
            uri,
            move |state, uri| {
                state.doc(uri)?;
                let range = state.positions_to_server(uri, range);
                Ok(json!({"textDocument": {"uri": uri}, "range": range, "options": options}))
            },
            edits,
            Box::new(reply),
        )
    }

    /// The code actions for `range`, given the diagnostics there; `only` limits their kinds.
    pub fn code_actions(
        &self,
        uri: &Uri,
        range: Range,
        diagnostics: Vec<Diagnostic>,
        only: Option<Vec<CodeActionKind>>,
        reply: impl FnOnce(Result<Vec<CodeActionOrCommand>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/codeAction",
            "codeActionProvider",
            uri,
            move |state, uri| {
                state.doc(uri)?;
                let range = state.positions_to_server(uri, range);
                let diagnostics = state.positions_to_server(uri, diagnostics);
                let mut context = json!({"diagnostics": diagnostics, "triggerKind": 1});
                if let Some(only) = only {
                    context["only"] = json!(only);
                }
                Ok(json!({"textDocument": {"uri": uri}, "range": range, "context": context}))
            },
            |answer: Option<Vec<CodeActionOrCommand>>, uri, mapper| {
                let mut actions = answer.unwrap_or_default();
                mapper.recount(uri, &mut actions);
                Ok(actions)
            },
            Box::new(reply),
        )
    }

    /// `action` with its edit, for servers that compute edits only when an action is chosen.
    pub fn resolve_code_action(
        &self,
        uri: &Uri,
        action: CodeAction,
        reply: impl FnOnce(Result<CodeAction, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "codeAction/resolve",
            "codeActionProvider",
            uri,
            move |state, uri| {
                if !capabilities::option(&state.raw, "codeActionProvider", "resolveProvider") {
                    return Err(Error::Unsupported("codeAction/resolve"));
                }
                Ok(json!(state.positions_to_server(uri, action)))
            },
            |mut action: CodeAction, uri, mapper| {
                mapper.recount(uri, &mut action);
                Ok(action)
            },
            Box::new(reply),
        )
    }

    /// Runs a server command, such as one a code action names; edits it makes arrive as `Event::ApplyEdit`.
    pub fn execute_command(
        &self,
        command: &str,
        arguments: Vec<Value>,
        reply: impl FnOnce(Result<Value, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let params = json!({"command": command, "arguments": arguments});
        self.call(
            "workspace/executeCommand",
            Some("executeCommandProvider"),
            |_| Ok(params),
            Box::new(reply),
        )
    }

    /// The folding ranges of the document, in whole lines.
    pub fn folding_ranges(
        &self,
        uri: &Uri,
        reply: impl FnOnce(Result<Vec<FoldingRange>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/foldingRange",
            "foldingRangeProvider",
            uri,
            document,
            |ranges: Option<Vec<FoldingRange>>, _, _| Ok(ranges.unwrap_or_default()),
            Box::new(reply),
        )
    }

    /// The document's symbols as a tree; servers that send a flat list get one level.
    pub fn document_symbols(
        &self,
        uri: &Uri,
        reply: impl FnOnce(Result<Vec<DocumentSymbol>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/documentSymbol",
            "documentSymbolProvider",
            uri,
            document,
            |answer: Option<DocumentSymbolResponse>, uri, mapper| {
                let mut symbols = match answer {
                    None => vec![],
                    Some(DocumentSymbolResponse::Nested(symbols)) => symbols,
                    Some(DocumentSymbolResponse::Flat(flat)) => flat.into_iter().map(nested).collect(),
                };
                mapper.recount(uri, &mut symbols);
                Ok(symbols)
            },
            Box::new(reply),
        )
    }

    /// The inlay hints in `range`.
    pub fn inlay_hints(
        &self,
        uri: &Uri,
        range: Range,
        reply: impl FnOnce(Result<Vec<InlayHint>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        self.ask(
            "textDocument/inlayHint",
            "inlayHintProvider",
            uri,
            move |state, uri| {
                state.doc(uri)?;
                let range = state.positions_to_server(uri, range);
                Ok(json!({"textDocument": {"uri": uri}, "range": range}))
            },
            |answer: Option<Vec<InlayHint>>, uri, mapper| {
                let mut hints = answer.unwrap_or_default();
                mapper.recount(uri, &mut hints);
                Ok(hints)
            },
            Box::new(reply),
        )
    }

    /// The document's diagnostics, pulled; diagnostics of related documents arrive as `Event::Diagnostics`.
    pub fn pull_diagnostics(
        &self,
        uri: &Uri,
        reply: impl FnOnce(Result<Vec<Diagnostic>, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let weak = Arc::downgrade(self.inner());
        self.ask(
            "textDocument/diagnostic",
            "diagnosticProvider",
            uri,
            |state, uri| {
                let doc = state.doc(uri)?;
                let mut params = json!({"textDocument": {"uri": uri}});
                let provider = state
                    .raw
                    .get("diagnosticProvider")
                    .or_else(|| state.registered("textDocument/diagnostic"));
                if let Some(identifier) = provider.and_then(|p| p.get("identifier")) {
                    params["identifier"] = identifier.clone();
                }
                if let Some((result_id, _)) = &doc.pulled {
                    params["previousResultId"] = Value::String(result_id.clone());
                }
                Ok(params)
            },
            move |answer: DocumentDiagnosticReportResult, uri, mapper| {
                let DocumentDiagnosticReportResult::Report(report) = answer else {
                    return Err(Error::Malformed("a partial diagnostic report without a token".into()));
                };
                let inner = weak.upgrade().ok_or(Error::ServerGone)?;
                let (result_id, items, related) = match report {
                    DocumentDiagnosticReport::Full(full) => {
                        let mut items = full.full_document_diagnostic_report.items;
                        mapper.recount(uri, &mut items);
                        (
                            full.full_document_diagnostic_report.result_id,
                            items,
                            full.related_documents,
                        )
                    }
                    DocumentDiagnosticReport::Unchanged(unchanged) => {
                        let kept = inner
                            .lock()
                            .documents
                            .get(uri)
                            .and_then(|doc| doc.pulled.as_ref().map(|(_, items)| items.clone()))
                            .unwrap_or_default();
                        let id = unchanged.unchanged_document_diagnostic_report.result_id;
                        (Some(id), kept, unchanged.related_documents)
                    }
                };
                if let Some(doc) = inner.lock().documents.get_mut(uri) {
                    doc.pulled = result_id.map(|id| (id, items.clone()));
                }
                for (related_uri, kind) in related.unwrap_or_default() {
                    if let DocumentDiagnosticReportKind::Full(report) = kind {
                        let related_uri = uri::normalize(&related_uri);
                        let mut diagnostics = report.items;
                        mapper.recount(&related_uri, &mut diagnostics);
                        inner.emit(Event::Diagnostics {
                            uri: related_uri,
                            version: None,
                            diagnostics,
                        });
                    }
                }
                Ok(items)
            },
            Box::new(reply),
        )
    }

    /// The document's semantic tokens; after the first answer only the changes are asked for, if the server can.
    pub fn semantic_tokens(
        &self,
        uri: &Uri,
        reply: impl FnOnce(Result<SemanticTokens, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let uri = uri::normalize(uri);
        let (method, params, legend, previous) = {
            let state = self.inner().lock();
            if state.status != Status::Running {
                return Err(Error::NotRunning);
            }
            let provider = state
                .raw
                .get("semanticTokensProvider")
                .filter(|p| p.is_object())
                .or_else(|| state.registered("textDocument/semanticTokens"))
                .filter(|p| capabilities::offers(p, "full"))
                .ok_or(Error::Unsupported("textDocument/semanticTokens/full"))?;
            let legend: Legend = serde_json::from_value(provider.get("legend").cloned().unwrap_or(Value::Null))
                .map_err(|e| Error::Malformed(e.to_string()))?;
            let delta = capabilities::option(provider, "full", "delta");
            let previous = state.doc(&uri)?.tokens.clone().unwrap_or_default();
            match (&previous.result_id, delta) {
                (Some(id), true) => (
                    "textDocument/semanticTokens/full/delta",
                    json!({"textDocument": {"uri": uri}, "previousResultId": id}),
                    legend,
                    previous,
                ),
                _ => (
                    "textDocument/semanticTokens/full",
                    json!({"textDocument": {"uri": uri}}),
                    legend,
                    previous,
                ),
            }
        };
        let weak = Arc::downgrade(self.inner());
        let legend = Arc::new(legend);
        self.call(
            method,
            None,
            |_| Ok(params),
            Box::new(move |answer| {
                let Some(inner) = weak.upgrade() else {
                    return reply(Err(Error::ServerGone));
                };
                let next = answer.and_then(|value| semantic::apply(&previous, value));
                let (text, encoding) = {
                    let mut state = inner.lock();
                    let encoding = state.encoding;
                    let text = state.documents.get_mut(&uri).map(|doc| {
                        doc.tokens = next.as_ref().ok().cloned();
                        doc.text.clone()
                    });
                    (text, encoding)
                };
                reply(next.map(|next| semantic::decode(&next.data, legend, text.as_ref(), encoding)));
            }),
        )
    }
}

/// The targets of a definition-like answer as links, whichever of its three shapes the server chose.
pub(crate) fn links(
    answer: Option<GotoDefinitionResponse>,
    uri: &Uri,
    mapper: &mut Mapper,
) -> Result<Vec<LocationLink>, Error> {
    let link = |location: Location| LocationLink {
        origin_selection_range: None,
        target_uri: uri::normalize(&location.uri),
        target_range: location.range,
        target_selection_range: location.range,
    };
    let mut links: Vec<LocationLink> = match answer {
        None => vec![],
        Some(GotoDefinitionResponse::Scalar(location)) => vec![link(location)],
        Some(GotoDefinitionResponse::Array(locations)) => locations.into_iter().map(link).collect(),
        Some(GotoDefinitionResponse::Link(links)) => links
            .into_iter()
            .map(|l| LocationLink {
                target_uri: uri::normalize(&l.target_uri),
                ..l
            })
            .collect(),
    };
    mapper.recount(uri, &mut links);
    Ok(links)
}

pub(crate) fn at(state: &State, uri: &Uri, position: Position) -> Result<Value, Error> {
    Ok(json!({"textDocument": {"uri": uri}, "position": state.to_server(uri, position)?}))
}

fn document(state: &State, uri: &Uri) -> Result<Value, Error> {
    state.doc(uri)?;
    Ok(json!({"textDocument": {"uri": uri}}))
}

fn edits(answer: Option<Vec<TextEdit>>, uri: &Uri, mapper: &mut Mapper) -> Result<Vec<TextEdit>, Error> {
    let mut edits = answer.unwrap_or_default();
    mapper.recount(uri, &mut edits);
    Ok(edits)
}

#[expect(
    deprecated,
    reason = "lsp-types still has the deprecated field, which must be filled in"
)]
fn nested(symbol: SymbolInformation) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name,
        detail: symbol.container_name,
        kind: symbol.kind,
        tags: symbol.tags,
        deprecated: symbol.deprecated,
        range: symbol.location.range,
        selection_range: symbol.location.range,
        children: None,
    }
}

fn markdown(contents: HoverContents) -> String {
    let marked = |s: MarkedString| match s {
        MarkedString::String(text) => text,
        MarkedString::LanguageString(code) => format!("```{}\n{}\n```", code.language, code.value),
    };
    match contents {
        HoverContents::Scalar(s) => marked(s),
        HoverContents::Array(list) => list.into_iter().map(marked).collect::<Vec<_>>().join("\n\n"),
        HoverContents::Markup(markup) => match markup.kind {
            MarkupKind::Markdown => markup.value,
            MarkupKind::PlainText => escape(&markup.value),
        },
    }
}

/// `text` with every character Markdown could read as syntax escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use lsp_types::{InsertReplaceEdit, LanguageString, MarkupContent};

    use super::*;

    #[test]
    fn hover_contents_of_every_shape_become_markdown() {
        let code = MarkedString::LanguageString(LanguageString {
            language: "go".into(),
            value: "func f()".into(),
        });
        let list = HoverContents::Array(vec![code, MarkedString::String("*doc*".into())]);
        assert_eq!(markdown(list), "```go\nfunc f()\n```\n\n*doc*");
        let plain = HoverContents::Markup(MarkupContent {
            kind: MarkupKind::PlainText,
            value: "a_b *c*".into(),
        });
        assert_eq!(markdown(plain), r"a\_b \*c\*");
    }

    #[test]
    fn a_completion_inserts_its_edit_its_insert_text_or_its_label() {
        let r = |a, b| Range::new(Position::new(0, a), Position::new(0, b));
        let snippet = CompletionItem {
            label: "Println".into(),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            text_edit: Some(CompletionTextEdit::InsertAndReplace(InsertReplaceEdit {
                new_text: "Println(${1:a})".into(),
                insert: r(4, 6),
                replace: r(4, 9),
            })),
            ..CompletionItem::default()
        };
        let chosen = insertion(&snippet);
        assert_eq!(chosen.snippet.text, "Println(a)");
        assert_eq!((chosen.insert, chosen.replace), (Some(r(4, 6)), Some(r(4, 9))));
        let plain = CompletionItem {
            label: "len".into(),
            insert_text: Some("len($1)".into()),
            ..CompletionItem::default()
        };
        assert_eq!(
            insertion(&plain).snippet.text,
            "len($1)",
            "plain text keeps dollar signs"
        );
        assert_eq!(insertion(&plain).snippet.tabstops[0].spans, [snippet::Span::new(7, 7)]);
        let label = CompletionItem {
            label: "x😀".into(),
            ..CompletionItem::default()
        };
        assert_eq!(insertion(&label).snippet.tabstops[0].spans, [snippet::Span::new(3, 3)]);
    }
}
