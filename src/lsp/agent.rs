//! The language servers' diagnostics as the own agent reads them: kept per file as the servers publish them, also for
//! files no editor shows, so the agent's worker threads can read them without the main thread.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ide_lsp::Language;
use ide_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};

static PUBLISHED: Mutex<BTreeMap<PathBuf, Vec<ide_agent::Diagnostic>>> = Mutex::new(BTreeMap::new());

/// The project roots and languages of the servers that run, by their real path.
static SERVED: Mutex<Vec<(PathBuf, Language)>> = Mutex::new(Vec::new());

fn real(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// A server for `language` runs in `root`.
pub(super) fn started(root: &Path, language: Language) {
    SERVED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((real(root), language));
}

/// The server for `language` in `root` stopped: what it published goes, since nothing keeps it current any more.
pub(super) fn stopped(root: &Path, language: &Language) {
    let root = real(root);
    let served_here = |s: &(PathBuf, Language)| s.0 == root && s.1 == *language;
    let mut served = SERVED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(at) = served.iter().position(served_here) {
        served.remove(at);
    }
    if served.iter().any(served_here) {
        return;
    }
    PUBLISHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|path, _| !path.starts_with(&root) || super::language_of(path).is_none_or(|(l, _)| l != *language));
}

/// The server published `diagnostics` for `path`, in place of what it said before.
pub(super) fn publish(path: &Path, diagnostics: &[Diagnostic]) {
    // The agent's tools name files by their real path, which a server started in a linked folder does not.
    let path = &real(path);
    let mut published = PUBLISHED.lock().unwrap_or_else(|e| e.into_inner());
    if diagnostics.is_empty() {
        published.remove(path);
        return;
    }
    let converted = diagnostics
        .iter()
        .map(|d| {
            let code = match &d.code {
                Some(NumberOrString::String(code)) => format!(" [{code}]"),
                Some(NumberOrString::Number(code)) => format!(" [{code}]"),
                None => String::new(),
            };
            ide_agent::Diagnostic {
                path: path.to_path_buf(),
                line: d.range.start.line.saturating_add(1),
                column: d.range.start.character.saturating_add(1),
                severity: match d.severity {
                    Some(DiagnosticSeverity::WARNING) => ide_agent::Severity::Warning,
                    Some(DiagnosticSeverity::INFORMATION) => ide_agent::Severity::Info,
                    Some(DiagnosticSeverity::HINT) => ide_agent::Severity::Hint,
                    _ => ide_agent::Severity::Error,
                },
                message: format!("{}{code}", d.message),
                source: d.source.clone(),
            }
        })
        .collect();
    published.insert(path.to_path_buf(), converted);
}

/// The diagnostics of the files below one project folder.
struct Published {
    root: PathBuf,
}

impl ide_agent::Diagnostics for Published {
    fn diagnostics(&self, path: Option<&Path>) -> Option<Vec<ide_agent::Diagnostic>> {
        let served = SERVED.lock().unwrap_or_else(|e| e.into_inner());
        if !served
            .iter()
            .any(|(root, _)| root.starts_with(&self.root) || self.root.starts_with(root))
        {
            return None;
        }
        let published = PUBLISHED.lock().unwrap_or_else(|e| e.into_inner());
        let found = published
            .iter()
            .filter(|(file, _)| path.map_or_else(|| file.starts_with(&self.root), |p| *file == p))
            .flat_map(|(_, found)| found.iter().cloned())
            .collect();
        Some(found)
    }
}

/// The diagnostics of the project at `root`, for the own agent's diagnostics tool.
pub fn agent_diagnostics(root: &Path) -> Arc<dyn ide_agent::Diagnostics> {
    Arc::new(Published { root: real(root) })
}

#[cfg(test)]
mod tests {
    use ide_lsp::lsp_types::{Position, Range};

    use super::*;

    #[test]
    fn the_agent_reads_what_the_running_servers_published_below_its_project() {
        let root = std::env::temp_dir().join(format!("ide-lsp-agent-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let file = root.join("src/lib.rs");
        std::fs::write(&file, "fn x() {}\n").unwrap();
        let problem = Diagnostic {
            range: Range::new(Position::new(0, 3), Position::new(0, 4)),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("dead_code".into())),
            source: Some("rustc".into()),
            message: "function `x` is never used".into(),
            ..Diagnostic::default()
        };
        let source = agent_diagnostics(&root);
        assert!(source.diagnostics(None).is_none(), "no server runs yet");
        started(&root, Language::Rust);
        publish(&file, std::slice::from_ref(&problem));
        let found = source.diagnostics(None).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            (
                found[0].line,
                found[0].column,
                found[0].severity,
                found[0].message.as_str()
            ),
            (
                1,
                4,
                ide_agent::Severity::Warning,
                "function `x` is never used [dead_code]"
            )
        );
        assert!(source.diagnostics(Some(&root.join("src/other.rs"))).unwrap().is_empty());
        assert!(
            agent_diagnostics(&std::env::temp_dir().join("elsewhere"))
                .diagnostics(None)
                .is_none()
        );
        publish(&file, &[]);
        assert!(source.diagnostics(None).unwrap().is_empty());

        publish(&file, &[problem]);
        started(&root, Language::TypeScript);
        stopped(&root, &Language::TypeScript);
        assert_eq!(
            source.diagnostics(None).unwrap().len(),
            1,
            "another language's server stopped"
        );
        stopped(&root, &Language::Rust);
        assert!(source.diagnostics(None).is_none(), "the server stopped");
        started(&root, Language::Rust);
        assert!(
            source.diagnostics(None).unwrap().is_empty(),
            "what the stopped server said is gone"
        );
        stopped(&root, &Language::Rust);
        let _ = std::fs::remove_dir_all(root);
    }
}
