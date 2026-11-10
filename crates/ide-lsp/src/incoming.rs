//! What a server sends on its own: notifications become events, requests are answered here or handed to the app.

use std::sync::Arc;

use lsp_types::{
    ApplyWorkspaceEditParams, ApplyWorkspaceEditResponse, LogMessageParams, MessageActionItem, MessageType,
    ProgressParams, ProgressParamsValue, PublishDiagnosticsParams, RegistrationParams, ShowDocumentParams,
    ShowDocumentResult, ShowMessageParams, ShowMessageRequestParams, UnregistrationParams,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::rpc::Responder;
use crate::server::{Event, Inner, Refresh, Reply};
use crate::uri;

/// JSON-RPC's code for a method the receiver does not know.
const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC's code for parameters of the wrong shape.
const INVALID_PARAMS: i64 = -32602;

pub(crate) fn notification(inner: &Arc<Inner>, method: &str, params: Value) {
    match method {
        "textDocument/publishDiagnostics" => {
            let Some(params) = parse::<PublishDiagnosticsParams>(params) else {
                return;
            };
            let uri = uri::normalize(&params.uri);
            let mut diagnostics = params.diagnostics;
            inner.mapper().recount(&uri, &mut diagnostics);
            inner.emit(Event::Diagnostics {
                uri,
                version: params.version,
                diagnostics,
            });
        }
        "$/progress" => {
            if let Some(ProgressParams {
                token,
                value: ProgressParamsValue::WorkDone(progress),
            }) = parse(params)
            {
                inner.emit(Event::Progress { token, progress });
            }
        }
        "window/logMessage" => {
            if let Some(LogMessageParams { typ, message }) = parse(params) {
                inner.emit(Event::Log { level: typ, message });
            }
        }
        "$/logTrace" => {
            if let Some(message) = params.get("message").and_then(Value::as_str) {
                inner.emit(Event::Log {
                    level: MessageType::LOG,
                    message: message.to_string(),
                });
            }
        }
        "window/showMessage" => {
            if let Some(ShowMessageParams { typ, message }) = parse(params) {
                inner.emit(Event::Message { level: typ, message });
            }
        }
        _ => {}
    }
}

pub(crate) fn request(inner: &Arc<Inner>, method: &str, params: Value, responder: Responder) {
    match method {
        "workspace/configuration" => responder.ok(inner.configuration(&params)),
        "window/workDoneProgress/create" => responder.ok(Value::Null),
        "workspace/workspaceFolders" => {
            let root = &inner.config.root;
            let name = root
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            responder.ok(json!([{"uri": uri::from_path(root), "name": name}]));
        }
        "client/registerCapability" => register(inner, params, responder),
        "client/unregisterCapability" => unregister(inner, params, responder),
        "workspace/applyEdit" => apply_edit(inner, params, responder),
        "window/showMessageRequest" => {
            let Some(params) = parse::<ShowMessageRequestParams>(params) else {
                return responder.fail(INVALID_PARAMS, "not ShowMessageRequestParams");
            };
            inner.emit(Event::MessageRequest {
                level: params.typ,
                message: params.message,
                actions: params.actions.unwrap_or_default(),
                reply: Reply::new(responder, None::<MessageActionItem>),
            });
        }
        "window/showDocument" => {
            let Some(params) = parse::<ShowDocumentParams>(params) else {
                return responder.fail(INVALID_PARAMS, "not ShowDocumentParams");
            };
            let mut selection = params.selection;
            inner.mapper().recount(&params.uri, &mut selection);
            inner.emit(Event::ShowDocument {
                uri: params.uri,
                external: params.external == Some(true),
                take_focus: params.take_focus == Some(true),
                selection,
                reply: Reply::new(responder, ShowDocumentResult { success: false }),
            });
        }
        _ => match refresh(method) {
            Some(kind) => {
                responder.ok(Value::Null);
                inner.emit(Event::Refresh(kind));
            }
            None => responder.fail(METHOD_NOT_FOUND, &format!("{method} is not supported")),
        },
    }
}

fn refresh(method: &str) -> Option<Refresh> {
    Some(match method {
        "workspace/semanticTokens/refresh" => Refresh::SemanticTokens,
        "workspace/inlayHint/refresh" => Refresh::InlayHints,
        "workspace/diagnostic/refresh" => Refresh::Diagnostics,
        "workspace/codeLens/refresh" => Refresh::CodeLenses,
        "workspace/foldingRange/refresh" => Refresh::FoldingRanges,
        _ => return None,
    })
}

fn register(inner: &Arc<Inner>, params: Value, responder: Responder) {
    let Some(params) = parse::<RegistrationParams>(params) else {
        return responder.fail(INVALID_PARAMS, "not RegistrationParams");
    };
    inner.lock().registrations.extend(
        params
            .registrations
            .into_iter()
            .map(|r| (r.id, (r.method, r.register_options.unwrap_or(Value::Null)))),
    );
    responder.ok(Value::Null);
    inner.emit(Event::CapabilitiesChanged);
}

fn unregister(inner: &Arc<Inner>, params: Value, responder: Responder) {
    let Some(params) = parse::<UnregistrationParams>(params) else {
        return responder.fail(INVALID_PARAMS, "not UnregistrationParams");
    };
    {
        let mut state = inner.lock();
        for gone in params.unregisterations {
            state.registrations.remove(&gone.id);
        }
    }
    responder.ok(Value::Null);
    inner.emit(Event::CapabilitiesChanged);
}

fn apply_edit(inner: &Arc<Inner>, params: Value, responder: Responder) {
    let Some(params) = parse::<ApplyWorkspaceEditParams>(params) else {
        return responder.fail(INVALID_PARAMS, "not ApplyWorkspaceEditParams");
    };
    let mut edit = params.edit;
    let root = uri::from_path(&inner.config.root);
    inner.mapper().recount(&root, &mut edit);
    let fallback = ApplyWorkspaceEditResponse {
        applied: false,
        failure_reason: Some("the editor did not apply the edit".to_string()),
        failed_change: None,
    };
    inner.emit(Event::ApplyEdit {
        label: params.label,
        edit,
        reply: Reply::new(responder, fallback),
    });
}

fn parse<T: DeserializeOwned>(params: Value) -> Option<T> {
    serde_json::from_value(params).ok()
}
