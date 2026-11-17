//! The folder the agent works in and what its tools reach: the project, the folders the user allows besides it, the
//! login shell's environment, the supervisor its commands run under and the language servers' diagnostics.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use ide_process::Supervisor;
use serde::{Deserialize, Serialize};

use crate::http::Http;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// An error.
    Error,
    /// A warning.
    Warning,
    /// Information.
    Info,
    /// A hint.
    Hint,
}

/// A problem a language server reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The file.
    pub path: PathBuf,
    /// The line, counted from 1.
    pub line: u32,
    /// The column, counted from 1.
    pub column: u32,
    /// How serious it is.
    pub severity: Severity,
    /// What the server says.
    pub message: String,
    /// The server or linter that reports it.
    pub source: Option<String>,
}

/// The language servers' diagnostics, as the app knows them.
pub trait Diagnostics: Send + Sync {
    /// The problems in `path`, or in every file the servers report on; `None` while no server runs for the project.
    fn diagnostics(&self, path: Option<&Path>) -> Option<Vec<Diagnostic>>;
}

/// Where the agent works.
#[derive(Clone)]
pub struct Workspace {
    root: PathBuf,
    allowed: Vec<PathBuf>,
    environment: Vec<(String, String)>,
    supervisor: Supervisor,
    http: Http,
    diagnostics: Option<Arc<dyn Diagnostics>>,
    rules_from: Option<PathBuf>,
    checkpointed: bool,
}

impl Workspace {
    /// The project at `root`, with commands run in `environment` under `supervisor` and web requests over `http`.
    pub fn new(
        root: &Path,
        environment: Vec<(String, String)>,
        supervisor: Supervisor,
        http: Http,
    ) -> std::io::Result<Workspace> {
        Ok(Workspace {
            root: root.canonicalize()?,
            allowed: Vec::new(),
            environment,
            supervisor,
            http,
            diagnostics: None,
            rules_from: None,
            checkpointed: false,
        })
    }

    /// The same, with `folders` reachable besides the project, such as the temporary folder.
    pub fn allow(mut self, folders: &[PathBuf]) -> Workspace {
        self.allowed = folders.iter().map(|folder| real(folder)).collect();
        self
    }

    /// The same, with the language servers' diagnostics.
    pub fn with_diagnostics(mut self, diagnostics: Arc<dyn Diagnostics>) -> Workspace {
        self.diagnostics = Some(diagnostics);
        self
    }

    /// The same, with the project's rules files read from `folder` instead of the project, such as a work order's
    /// configuration frozen from the user's working copy (E172).
    pub fn rules_from(mut self, folder: &Path) -> Workspace {
        self.rules_from = Some(folder.to_path_buf());
        self
    }

    /// The same, for a project whose state a checkpoint keeps before each turn, as a work order's worktree: git that
    /// discards only its own work runs without asking (E173, C47).
    pub fn checkpointed(mut self) -> Workspace {
        self.checkpointed = true;
        self
    }

    /// The folder the project's rules files are read from.
    pub fn rules_root(&self) -> &Path {
        self.rules_from.as_deref().unwrap_or(&self.root)
    }

    /// Whether a checkpoint keeps the project's state before each turn.
    pub fn is_checkpointed(&self) -> bool {
        self.checkpointed
    }

    /// The project folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folders reachable besides the project.
    pub fn allowed(&self) -> &[PathBuf] {
        &self.allowed
    }

    /// The login shell's environment.
    pub(crate) fn environment(&self) -> &[(String, String)] {
        &self.environment
    }

    pub(crate) fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    pub(crate) fn http(&self) -> &Http {
        &self.http
    }

    pub(crate) fn diagnostics(&self) -> Option<&Arc<dyn Diagnostics>> {
        self.diagnostics.as_ref()
    }

    fn variable(&self, name: &str) -> Option<&str> {
        self.environment
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The real path `path` names: relative to the project, `~` for the home folder, symbolic links resolved as far
    /// as the path exists.
    pub fn resolve(&self, path: &str) -> PathBuf {
        let path = path.trim();
        let joined = match path.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                let home = self.variable("HOME").map_or_else(ide_core::paths::home, PathBuf::from);
                home.join(rest.trim_start_matches('/'))
            }
            _ if Path::new(path).is_absolute() => PathBuf::from(path),
            _ => self.root.join(path),
        };
        real(&joined)
    }

    /// Whether `path`, a resolved path, lies in the project or an allowed folder.
    pub fn reaches(&self, path: &Path) -> bool {
        path.starts_with(&self.root) || self.allowed.iter().any(|folder| path.starts_with(folder))
    }

    /// `path` as the user reads it: relative inside the project.
    pub(crate) fn display(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(relative) if relative.as_os_str().is_empty() => ".".into(),
            Ok(relative) => relative.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }

    /// The `sandbox-exec` profile of a command: writes only in the project, the allowed folders and the terminal
    /// devices, no network beyond local sockets; reading stays open.
    pub(crate) fn sandbox_profile(&self) -> String {
        let folders: Vec<PathBuf> = std::iter::once(&self.root).chain(&self.allowed).cloned().collect();
        ide_process::sandbox::Profile::writes_only(&folders).render()
    }
}

/// How many links one path may lead through before it names nothing; macOS itself follows 32.
const MAX_LINKS: usize = 40;

/// `path` with `.` and `..` resolved by name and symbolic links resolved through its longest existing part, also a
/// link to something that does not exist yet, which a write through it would create; empty when its links do not end.
fn real(path: &Path) -> PathBuf {
    let mut plain = lexical(path);
    for _ in 0..=MAX_LINKS {
        let mut existing = plain.clone();
        let mut rest = Vec::new();
        while std::fs::symlink_metadata(&existing).is_err() {
            let Some(name) = existing.file_name().map(std::ffi::OsStr::to_os_string) else {
                break;
            };
            rest.push(name);
            if !existing.pop() {
                break;
            }
        }
        let mut out = match existing.canonicalize() {
            Ok(real) => real,
            Err(_) => match dangling_target(&existing) {
                Some(target) => {
                    plain = target;
                    plain.extend(rest.iter().rev());
                    continue;
                }
                None => existing,
            },
        };
        out.extend(rest.iter().rev());
        return out;
    }
    PathBuf::new()
}

/// Where the link at `link`, which does not resolve, points: its target against the real folder it lies in.
fn dangling_target(link: &Path) -> Option<PathBuf> {
    let target = std::fs::read_link(link).ok()?;
    let folder = link.parent()?.canonicalize().ok()?;
    Some(lexical(&folder.join(target)))
}

/// `path` with `.` and `..` resolved by name.
fn lexical(path: &Path) -> PathBuf {
    let mut plain = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                plain.pop();
            }
            Component::CurDir => {}
            other => plain.push(other),
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(name: &str) -> (ide_core::testing::Scratch, Workspace) {
        let dir = ide_core::testing::scratch(&format!("agent-ws-{name}"));
        std::fs::create_dir_all(dir.join("project/src")).unwrap();
        std::fs::create_dir_all(dir.join("other")).unwrap();
        let ws = Workspace::new(
            &dir.join("project"),
            vec![("HOME".into(), dir.display().to_string())],
            Supervisor::new(),
            Http::new(),
        )
        .unwrap();
        (dir, ws)
    }

    #[test]
    fn paths_resolve_against_the_project_and_cannot_climb_out_unnoticed() {
        let (dir, ws) = workspace("paths");
        let root = ws.root().to_path_buf();
        assert_eq!(ws.resolve("src/new.rs"), root.join("src/new.rs"));
        assert!(ws.reaches(&ws.resolve("src/../src/x/y.rs")));
        assert!(!ws.reaches(&ws.resolve("../other/z")));
        assert!(!ws.reaches(&ws.resolve("~/other")));
        std::os::unix::fs::symlink(dir.join("other"), root.join("link")).ok();
        assert!(
            !ws.reaches(&ws.resolve("link/escape.txt")),
            "a link out of the project leads out"
        );
        let allowed = ws.clone().allow(&[dir.join("other")]);
        assert!(allowed.reaches(&allowed.resolve("link/escape.txt")));
        assert_eq!(ws.display(&root.join("src/a.rs")), "src/a.rs");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_link_to_a_file_not_there_yet_leads_where_it_points() {
        let (dir, ws) = workspace("dangling");
        let root = ws.root().to_path_buf();
        std::os::unix::fs::symlink(dir.join("other/planted.plist"), root.join("notes.md")).unwrap();
        let planted = ws.resolve("notes.md");
        assert!(
            !ws.reaches(&planted),
            "a write through the link lands outside: {planted:?}"
        );
        assert_eq!(planted.file_name(), Some(std::ffi::OsStr::new("planted.plist")));
        std::os::unix::fs::symlink("src/later.rs", root.join("inner")).unwrap();
        assert_eq!(ws.resolve("inner"), root.join("src/later.rs"));
        std::os::unix::fs::symlink("circle", root.join("circle")).unwrap();
        assert!(!ws.reaches(&ws.resolve("circle")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_sandbox_profile_names_the_project_and_quotes_its_path() {
        let (dir, ws) = workspace("profile");
        let profile = ws.sandbox_profile();
        assert!(profile.contains(&format!("(subpath \"{}\")", ws.root().display())));
        assert!(profile.contains("(deny network*)"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_work_order_reads_its_rules_from_the_frozen_copy_and_discards_its_own_work() {
        let (dir, ws) = workspace("order");
        std::fs::write(ws.root().join("AGENTS.md"), "Changed by the builder.\n").unwrap();
        std::fs::write(dir.join("other/AGENTS.md"), "Run make check.\n").unwrap();
        assert_eq!(ws.rules_root(), ws.root());
        assert!(!ws.is_checkpointed());
        let order = ws.clone().rules_from(&dir.join("other")).checkpointed();
        let rules = crate::prompt::project_rules(order.rules_root());
        assert_eq!(rules, [("AGENTS.md".to_string(), "Run make check.\n".to_string())]);
        let call = serde_json::json!({"command": "git reset --hard"});
        let discard = crate::tools::action("run_command", &call, &order);
        assert!(matches!(discard, Ok(crate::policy::Action::Discard(_))), "{discard:?}");
        let asked = crate::tools::action("run_command", &call, &ws);
        assert!(matches!(asked, Ok(crate::policy::Action::Command(_))), "{asked:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
