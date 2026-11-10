//! The language servers' diagnostics as the own agent reads them: kept per file as the servers publish them, also for
//! files no editor shows, so the agent's worker threads can read them without the main thread.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ide_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};

static PUBLISHED: Mutex<BTreeMap<PathBuf, Vec<ide_agent::Diagnostic>>> = Mutex::new(BTreeMap::new());

/// The server published `diagnostics` for `path`, in place of what it said before.
pub(super) fn publish(path: &Path, diagnostics: &[Diagnostic]) {
    // The agent's tools name files by their real path, which a server started in a linked folder does not.
    let path = &path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
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
    fn diagnostics(&self, path: Option<&Path>) -> Vec<ide_agent::Diagnostic> {
        let published = PUBLISHED.lock().unwrap_or_else(|e| e.into_inner());
        published
            .iter()
            .filter(|(file, _)| path.map_or_else(|| file.starts_with(&self.root), |p| *file == p))
            .flat_map(|(_, found)| found.iter().cloned())
            .collect()
    }
}

/// The diagnostics of the project at `root`, for the own agent's diagnostics tool.
pub fn agent_diagnostics(root: &Path) -> Arc<dyn ide_agent::Diagnostics> {
    Arc::new(Published {
        root: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
    })
}

#[cfg(test)]
mod tests {
    use ide_lsp::lsp_types::{Position, Range};

    use super::*;

    #[test]
    fn the_agent_reads_what_the_servers_published_below_its_project() {
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
        publish(&file, &[problem]);
        let source = agent_diagnostics(&root);
        let found = source.diagnostics(None);
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
        assert!(source.diagnostics(Some(&root.join("src/other.rs"))).is_empty());
        assert!(
            agent_diagnostics(&std::env::temp_dir().join("elsewhere"))
                .diagnostics(None)
                .is_empty()
        );
        publish(&file, &[]);
        assert!(source.diagnostics(None).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
