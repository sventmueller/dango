//! What the language servers report: diagnostics, progress, messages and requests of their own, by slot.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their parent's names"
)]
use super::*;
use ide_lsp::Event;

pub(super) fn event_arrived(folder: &Path, id: u64, event: Event) {
    let Some(slot) = slot(folder, id) else { return };
    match event {
        Event::Diagnostics { uri, diagnostics, .. } => diagnostics_arrived(folder, id, &uri, &diagnostics),
        #[cfg(feature = "selftest")]
        Event::Log { message, .. } if std::env::var_os("IDE_SELFTEST_SERVER_LOG").is_some() => {
            eprintln!("server log [{}]: {message}", slot.language.name());
        }
        Event::Notice { method, .. } => {
            let text = notice_text(&slot.language, &method);
            ide_ui_common::diag::report("language server notice", &method);
            *slot.message.borrow_mut() = Some(text);
            for view in views_of(folder, id) {
                status::refresh(&*view);
            }
        }
        Event::Status(Status::Running) => {
            for view in views_of(folder, id) {
                refresh(&*view);
            }
        }
        Event::Progress { token, progress } => {
            let token = match token {
                ide_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
                ide_lsp::lsp_types::NumberOrString::String(s) => s,
            };
            #[cfg(feature = "selftest")]
            slot.progress_at.set(Some(std::time::Instant::now()));
            let mut all = slot.progress.borrow_mut();
            match progress {
                WorkDoneProgress::Begin(begin) => {
                    let text = progress_text(&begin.title, begin.message.as_deref(), begin.percentage);
                    all.insert(token, (begin.title, text));
                }
                WorkDoneProgress::Report(report) => {
                    let title = all.get(&token).map(|(title, _)| title.clone()).unwrap_or_default();
                    let text = progress_text(&title, report.message.as_deref(), report.percentage);
                    all.insert(token, (title, text));
                }
                WorkDoneProgress::End(_) => {
                    all.remove(&token);
                }
            }
        }
        Event::Message { level, message } => {
            if level == MessageType::ERROR || level == MessageType::WARNING {
                *slot.message.borrow_mut() = Some(message.clone());
            }
            ide_ui_common::diag::report("language server", message);
        }
        Event::Crashed { stderr, restarting, .. } => {
            let tail: String = stderr.lines().rev().take(5).collect::<Vec<_>>().join(" | ");
            ide_ui_common::diag::report("language server crashed", format!("restarting: {restarting}; {tail}"));
        }
        Event::Refresh(Refresh::SemanticTokens | Refresh::Diagnostics) => {
            for view in views_of(folder, id) {
                refresh(&*view);
            }
        }
        Event::Refresh(Refresh::InlayHints) => {
            for view in views_of(folder, id) {
                inlay::forget(&*view);
                inlay::request(&*view);
            }
        }
        Event::ApplyEdit { edit, reply, .. } => edits::apply_workspace_edit(folder, &edit, move |applied| {
            reply.send(ide_lsp::lsp_types::ApplyWorkspaceEditResponse {
                applied: applied.is_ok(),
                failed_change: applied
                    .as_ref()
                    .err()
                    .and_then(|f| f.index)
                    .and_then(|i| u32::try_from(i).ok()),
                failure_reason: applied.as_ref().err().map(|f| f.message.clone()),
            });
            if let Err(failure) = applied {
                actions::failed(failure.message);
            }
        }),
        Event::MessageRequest {
            message,
            actions,
            reply,
            ..
        } => edits::ask(folder, &message, actions, reply),
        Event::ShowDocument {
            uri, selection, reply, ..
        } => {
            let shown = uri::to_path(&uri).is_some_and(|path| navigate::show(folder, &path, selection));
            reply.send(ide_lsp::lsp_types::ShowDocumentResult { success: shown });
        }
        Event::Status(_) | Event::Log { .. } | Event::CapabilitiesChanged | Event::Refresh(_) => {}
    }
    refresh_slot(folder, id);
}

fn diagnostics_arrived(folder: &Path, source: u64, uri: &Uri, diagnostics: &[Diagnostic]) {
    let _interval = ide_process::signpost::begin(ide_process::signpost::name!("diagnostics"));
    let Some(path) = uri::to_path(uri) else { return };
    agent::publish(&path, diagnostics);
    for view in super::host::all_editors() {
        if view.document().path() == Some(&path) && view.project_root().as_deref() == Some(folder) {
            decorate::set_diagnostics_from(&*view, source, diagnostics);
        }
    }
}

/// What the status bar says for a server's own request `method`, such as ESLint missing in the project (E132).
fn notice_text(language: &Language, method: &str) -> String {
    match method {
        "eslint/noLibrary" => splat_i18n::t("ESLint im Projekt nicht installiert").to_string(),
        "eslint/noConfig" => splat_i18n::t("ESLint findet keine Konfiguration").to_string(),
        "eslint/probeFailed" => splat_i18n::t("ESLint konnte das Projekt nicht prüfen").to_string(),
        _ => tf("{}: {}", &[language.name(), method]),
    }
}
