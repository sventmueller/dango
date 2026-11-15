//! What the client tells a server it can do, and reading back what the server offers.

use lsp_types::ServerCapabilities;
use serde_json::{Map, Value, json};

use crate::encoding::Encoding;

/// The semantic token types the editor colours; servers may add their own in their legend.
pub(crate) const TOKEN_TYPES: &[&str] = &[
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "macro",
    "keyword",
    "modifier",
    "comment",
    "string",
    "number",
    "regexp",
    "operator",
    "decorator",
    "label",
];

/// The semantic token modifiers the editor knows.
pub(crate) const TOKEN_MODIFIERS: &[&str] = &[
    "declaration",
    "definition",
    "readonly",
    "static",
    "deprecated",
    "abstract",
    "async",
    "modification",
    "documentation",
    "defaultLibrary",
];

/// The client capabilities sent with `initialize`.
pub(crate) fn client(encodings: &[Encoding], client_watches_files: bool) -> Value {
    let encodings: Vec<String> = encodings.iter().map(|e| e.kind().as_str().to_string()).collect();
    json!({
        "general": {
            "positionEncodings": encodings,
            "staleRequestSupport": {"cancel": true, "retryOnContentModified": []},
        },
        "window": {
            "workDoneProgress": true,
            "showMessage": {"messageActionItem": {"additionalPropertiesSupport": false}},
            "showDocument": {"support": true},
        },
        "workspace": workspace(client_watches_files),
        "textDocument": text_document(),
    })
}

fn workspace(client_watches_files: bool) -> Value {
    json!({
        "applyEdit": true,
        "workspaceEdit": {
            "documentChanges": true,
            "resourceOperations": ["create", "rename", "delete"],
            "failureHandling": "abort",
        },
        "configuration": true,
        "workspaceFolders": true,
        "didChangeConfiguration": {"dynamicRegistration": false},
        // A server that may not register watchers watches files itself where it can (rust-analyzer does).
        "didChangeWatchedFiles": {"dynamicRegistration": client_watches_files, "relativePatternSupport": true},
        "executeCommand": {"dynamicRegistration": false},
        "semanticTokens": {"refreshSupport": true},
        "inlayHint": {"refreshSupport": true},
        "diagnostics": {"refreshSupport": true},
        "codeLens": {"refreshSupport": true},
        "foldingRange": {"refreshSupport": true},
        "symbol": {
            "symbolKind": {"valueSet": (1..=26).collect::<Vec<u32>>()},
            "tagSupport": {"valueSet": [1]},
            "resolveSupport": {"properties": ["location.range"]},
        },
    })
}

fn text_document() -> Value {
    json!({
        "synchronization": {"dynamicRegistration": false, "didSave": true, "willSave": false, "willSaveWaitUntil": false},
        "publishDiagnostics": {
            "relatedInformation": true,
            "versionSupport": true,
            "tagSupport": {"valueSet": [1, 2]},
            "codeDescriptionSupport": true,
            "dataSupport": true,
        },
        "diagnostic": {"dynamicRegistration": true, "relatedDocumentSupport": true},
        "hover": {"dynamicRegistration": false, "contentFormat": ["markdown", "plaintext"]},
        "completion": completion(),
        "signatureHelp": {
            "signatureInformation": {
                "documentationFormat": ["markdown", "plaintext"],
                "parameterInformation": {"labelOffsetSupport": true},
                "activeParameterSupport": true,
            },
            "contextSupport": true,
        },
        "definition": {"linkSupport": true},
        "implementation": {"linkSupport": true},
        "typeDefinition": {"linkSupport": true},
        "declaration": {"linkSupport": true},
        "callHierarchy": {},
        "typeHierarchy": {},
        "references": {},
        "documentSymbol": {
            "hierarchicalDocumentSymbolSupport": true,
            "symbolKind": {"valueSet": (1..=26).collect::<Vec<u32>>()},
            "tagSupport": {"valueSet": [1]},
        },
        "formatting": {},
        "rangeFormatting": {},
        "rename": {"prepareSupport": true, "prepareSupportDefaultBehavior": 1},
        "codeAction": {
            "codeActionLiteralSupport": {"codeActionKind": {"valueSet": [
                "", "quickfix", "refactor", "refactor.extract", "refactor.inline", "refactor.rewrite", "source",
                "source.organizeImports", "source.fixAll",
            ]}},
            "isPreferredSupport": true,
            "disabledSupport": true,
            "dataSupport": true,
            "resolveSupport": {"properties": ["edit"]},
        },
        "foldingRange": {"lineFoldingOnly": true, "foldingRangeKind": {"valueSet": ["comment", "imports", "region"]}},
        "semanticTokens": {
            "dynamicRegistration": true,
            "requests": {"range": false, "full": {"delta": true}},
            "tokenTypes": TOKEN_TYPES,
            "tokenModifiers": TOKEN_MODIFIERS,
            "formats": ["relative"],
            "overlappingTokenSupport": false,
            "multilineTokenSupport": false,
            "serverCancelSupport": true,
            "augmentsSyntaxTokens": true,
        },
        "inlayHint": {"dynamicRegistration": false},
    })
}

fn completion() -> Value {
    // No `completionList.itemDefaults`: lsp-types 0.97 has no field for them and would drop them silently.
    json!({
        "completionItem": {
            "snippetSupport": true,
            "commitCharactersSupport": true,
            "documentationFormat": ["markdown", "plaintext"],
            "deprecatedSupport": true,
            "preselectSupport": true,
            "tagSupport": {"valueSet": [1]},
            "insertReplaceSupport": true,
            "resolveSupport": {"properties": ["documentation", "detail", "additionalTextEdits"]},
            "insertTextModeSupport": {"valueSet": [1, 2]},
            "labelDetailsSupport": true,
        },
        "completionItemKind": {"valueSet": (1..=25).collect::<Vec<u32>>()},
        "contextSupport": true,
    })
}

/// What a server's capabilities say as `lsp_types` reads them, and the fields it could not read.
pub(crate) struct Read {
    /// Every field that reads; `None` only when the capabilities are not an object at all.
    pub(crate) capabilities: Option<ServerCapabilities>,
    /// The fields left out, with why.
    pub(crate) unreadable: Vec<(String, serde_json::Error)>,
}

/// Reads `raw` field by field, so one field a server words against the protocol does not cost all the others.
pub(crate) fn read(raw: &Value) -> Read {
    let Value::Object(fields) = raw else {
        let unreadable = serde_json::from_value::<ServerCapabilities>(raw.clone()).err();
        return Read {
            capabilities: None,
            unreadable: unreadable
                .map(|e| ("capabilities".to_string(), e))
                .into_iter()
                .collect(),
        };
    };
    if let Ok(capabilities) = serde_json::from_value(raw.clone()) {
        return Read {
            capabilities: Some(capabilities),
            unreadable: Vec::new(),
        };
    }
    let mut readable = Map::new();
    let mut unreadable = Vec::new();
    for (key, value) in fields {
        let alone = Value::Object(Map::from_iter([(key.clone(), value.clone())]));
        match serde_json::from_value::<ServerCapabilities>(alone) {
            Ok(_) => {
                readable.insert(key.clone(), value.clone());
            }
            Err(error) => unreadable.push((key.clone(), error)),
        }
    }
    Read {
        capabilities: serde_json::from_value(Value::Object(readable)).ok(),
        unreadable,
    }
}

/// Whether `capabilities` offer `key`: present and neither `false` nor `null`.
pub(crate) fn offers(capabilities: &Value, key: &str) -> bool {
    !matches!(capabilities.get(key), None | Some(Value::Null | Value::Bool(false)))
}

/// Whether the provider under `key` sets `option` to `true`.
pub(crate) fn option(capabilities: &Value, key: &str, option: &str) -> bool {
    capabilities
        .get(key)
        .and_then(|p| p.get(option))
        .and_then(Value::as_bool)
        == Some(true)
}

#[cfg(test)]
mod tests {
    use lsp_types::{ClientCapabilities, PositionEncodingKind};

    use super::*;

    #[test]
    fn the_capabilities_are_what_lsp_types_reads() {
        let value = client(&[Encoding::Utf16, Encoding::Utf8], false);
        let parsed: ClientCapabilities = serde_json::from_value(value).unwrap();
        assert_eq!(
            parsed.general.unwrap().position_encodings.unwrap(),
            vec![PositionEncodingKind::UTF16, PositionEncodingKind::UTF8]
        );
        let text = parsed.text_document.unwrap();
        assert_eq!(
            text.completion.unwrap().completion_item.unwrap().snippet_support,
            Some(true)
        );
        let tokens = text.semantic_tokens.unwrap();
        assert_eq!(tokens.token_types.len(), TOKEN_TYPES.len());
        assert_eq!(text.folding_range.unwrap().line_folding_only, Some(true));
        assert_eq!(text.implementation.unwrap().link_support, Some(true));
        assert!(text.call_hierarchy.is_some() && text.type_hierarchy.is_some());
        let workspace = parsed.workspace.unwrap();
        assert_eq!(workspace.apply_edit, Some(true));
        let resolve = workspace.symbol.unwrap().resolve_support.unwrap();
        assert_eq!(resolve.properties, ["location.range"]);
    }

    #[test]
    fn capabilities_keep_the_fields_that_read_when_one_does_not() {
        let read = read(&json!({"hoverProvider": true, "renameProvider": "yes please", "definitionProvider": true}));
        let capabilities = read.capabilities.unwrap();
        assert!(capabilities.hover_provider.is_some() && capabilities.definition_provider.is_some());
        assert!(capabilities.rename_provider.is_none());
        let names: Vec<_> = read.unreadable.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(names, ["renameProvider"]);
    }

    #[test]
    fn capabilities_that_read_whole_name_nothing_and_no_object_reads_as_none() {
        let whole = read(&json!({"hoverProvider": true, "experimental": {"x": 1}}));
        assert!(whole.capabilities.is_some() && whole.unreadable.is_empty());
        let missing = read(&Value::Null);
        assert!(missing.capabilities.is_none());
        assert_eq!(missing.unreadable.len(), 1);
    }

    #[test]
    fn a_provider_counts_when_it_is_true_or_an_object() {
        let caps = json!({"hoverProvider": true, "renameProvider": {"prepareProvider": true}, "x": false, "y": null});
        assert!(offers(&caps, "hoverProvider"));
        assert!(offers(&caps, "renameProvider"));
        assert!(!offers(&caps, "x") && !offers(&caps, "y") && !offers(&caps, "z"));
        assert!(option(&caps, "renameProvider", "prepareProvider"));
        assert!(!option(&caps, "hoverProvider", "prepareProvider"));
    }
}
