//! The servers the editor knows (Go, Rust, TypeScript): which language a file is in, where its project root is, and
//! where the server's program is.
//!
//! Only for trusted folders. Finding and starting a server runs code the project controls: `rustup` honours the
//! project's toolchain file, gopls and rust-analyzer run the project's build files, and the TypeScript server may be
//! the project's own `node_modules/.bin/tsc`. The caller decides about trust before it calls anything here; for an
//! untrusted folder it must not.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use ide_process::{Kind, Supervisor};
use serde_json::{Value, json};

use crate::encoding::Encoding;
use crate::rpc::{Connection, Handler, Responder};
use crate::server::ServerConfig;
use crate::tool::{self, Exits, Job};

/// How long a helper such as `go env` or `rustup which` may take.
const HELPER_TIMEOUT: Duration = Duration::from_secs(20);

/// How long the TypeScript compiler may take to answer `initialize` when probed.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// A language with a known server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Go, served by gopls.
    Go,
    /// Rust, served by rust-analyzer.
    Rust,
    /// TypeScript and JavaScript, served by TypeScript 7's own server or vtsls.
    TypeScript,
}

impl Language {
    /// The language of `path` and the protocol's language id for the document.
    pub fn of_path(path: &Path) -> Option<(Language, &'static str)> {
        let name = path.file_name()?.to_str()?;
        match name {
            "go.mod" => return Some((Language::Go, "go.mod")),
            "go.work" => return Some((Language::Go, "go.work")),
            _ => {}
        }
        Some(match path.extension()?.to_str()? {
            "go" => (Language::Go, "go"),
            "rs" => (Language::Rust, "rust"),
            "ts" | "mts" | "cts" => (Language::TypeScript, "typescript"),
            "tsx" => (Language::TypeScript, "typescriptreact"),
            "js" | "mjs" | "cjs" => (Language::TypeScript, "javascript"),
            "jsx" => (Language::TypeScript, "javascriptreact"),
            _ => return None,
        })
    }

    /// The name users read.
    pub fn name(self) -> &'static str {
        match self {
            Language::Go => "Go",
            Language::Rust => "Rust",
            Language::TypeScript => "TypeScript",
        }
    }
}

/// The project root of `file` for `language`, looked for between the file and `trusted`, the trusted folder it is
/// in; `trusted` itself when nothing marks a root. The search never leaves `trusted`.
///
/// Go: the nearest `go.work`, else the nearest `go.mod`. Rust: the topmost `Cargo.toml` with a `[workspace]`, else
/// the nearest `Cargo.toml`. TypeScript: the nearest `tsconfig.json` or `jsconfig.json`, else `package.json`.
pub fn root(language: Language, file: &Path, trusted: &Path) -> PathBuf {
    let start = if file.is_dir() {
        file
    } else {
        file.parent().unwrap_or(file)
    };
    let dirs: Vec<&Path> = start.ancestors().take_while(|dir| dir.starts_with(trusted)).collect();
    let has = |dir: &&Path, name: &str| dir.join(name).is_file();
    let found = match language {
        Language::Go => dirs
            .iter()
            .find(|d| has(d, "go.work"))
            .or_else(|| dirs.iter().find(|d| has(d, "go.mod"))),
        Language::Rust => dirs
            .iter()
            .rev()
            .find(|d| is_workspace(&d.join("Cargo.toml")))
            .or_else(|| dirs.iter().find(|d| has(d, "Cargo.toml"))),
        Language::TypeScript => dirs
            .iter()
            .find(|d| has(d, "tsconfig.json") || has(d, "jsconfig.json"))
            .or_else(|| dirs.iter().find(|d| has(d, "package.json"))),
    };
    found.map_or_else(|| trusted.to_path_buf(), |dir| dir.to_path_buf())
}

/// Whether the manifest at `path` declares a workspace.
fn is_workspace(path: &Path) -> bool {
    fs::read_to_string(path).is_ok_and(|text| {
        text.lines().map(str::trim).any(|line| {
            line.strip_prefix("[workspace")
                .is_some_and(|rest| rest.starts_with(']') || rest.starts_with('.'))
        })
    })
}

/// Where a server's program was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// On the login shell's PATH.
    Path,
    /// In `$(go env GOPATH)/bin`.
    GoPath,
    /// Through `rustup`, for the project's toolchain.
    Rustup,
    /// In the app's tools folder, installed by the app.
    Tools,
    /// The project's own TypeScript compiler, which speaks the protocol since TypeScript 7.
    Project,
}

/// How to start a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// For which language.
    pub language: Language,
    /// The program.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
    /// What the activity view shows.
    pub label: String,
    /// Where it was found.
    pub source: Source,
}

/// What the app can install when a server is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// gopls, with `go install`.
    Gopls,
    /// rust-analyzer, from its GitHub releases.
    RustAnalyzer,
    /// vtsls, with `npm install`.
    Vtsls,
}

/// No server was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotFound {
    /// For which language.
    pub language: Language,
    /// What would provide one.
    pub install: Install,
    /// A program that installing or running it needs and the login shell's PATH lacks, such as `go` or `node`.
    pub missing: Option<&'static str>,
}

/// Finds servers with the login shell's environment, never the app's own.
pub struct Registry {
    supervisor: Supervisor,
    exits: Exits,
    env: Vec<(String, String)>,
    tools: PathBuf,
    /// Whether a TypeScript compiler speaks the protocol, by its path, for its modification time.
    probes: Mutex<HashMap<PathBuf, (Option<SystemTime>, bool)>>,
}

impl Registry {
    /// A registry using `env`, the login shell's environment, and the app's tools folder.
    pub fn new(supervisor: &Supervisor, env: Vec<(String, String)>) -> Registry {
        Registry::with_tools(supervisor, env, default_tools())
    }

    /// A registry with another tools folder, for tests.
    pub fn with_tools(supervisor: &Supervisor, env: Vec<(String, String)>, tools: PathBuf) -> Registry {
        Registry {
            supervisor: supervisor.clone(),
            exits: Exits::watch(supervisor),
            env,
            tools,
            probes: Mutex::default(),
        }
    }

    /// The tools folder.
    pub fn tools(&self) -> &Path {
        &self.tools
    }

    /// The server for `language` in `root`, which lies in the trusted folder `trusted`. Blocks while it runs helpers
    /// such as `rustup which`; call it off the main thread.
    pub fn find(&self, language: Language, root: &Path, trusted: &Path) -> Result<Launch, NotFound> {
        match language {
            Language::Go => self.gopls(),
            Language::Rust => self.rust_analyzer(root),
            Language::TypeScript => self.typescript(root, trusted),
        }
    }

    /// The configuration to start `launch` in `root` with.
    pub fn config(&self, launch: &Launch, root: &Path) -> ServerConfig {
        let mut config = ServerConfig::new(&launch.label, &launch.program, root, self.env.clone());
        config.args.clone_from(&launch.args);
        if launch.language == Language::Go {
            // gopls leaves semantic tokens off unless asked.
            config.settings = json!({"gopls": {"semanticTokens": true}});
            config.initialization_options = Some(json!({"semanticTokens": true}));
        }
        config
    }

    fn path(&self) -> Option<&str> {
        tool::var(&self.env, "PATH")
    }

    fn home(&self) -> PathBuf {
        tool::var(&self.env, "HOME").map_or_else(ide_core::paths::home, PathBuf::from)
    }

    fn job(&self) -> Job<'_> {
        Job {
            supervisor: &self.supervisor,
            exits: &self.exits,
            env: &self.env,
            timeout: HELPER_TIMEOUT,
            cancel: None,
        }
    }

    fn launch(language: Language, program: PathBuf, args: &[&str], source: Source) -> Launch {
        let label = program
            .file_name()
            .map_or_else(|| program.display().to_string(), |n| n.to_string_lossy().into_owned());
        Launch {
            language,
            program,
            args: args.iter().map(ToString::to_string).collect(),
            label,
            source,
        }
    }

    fn gopls(&self) -> Result<Launch, NotFound> {
        if let Some(program) = tool::which("gopls", self.path()) {
            return Ok(Registry::launch(Language::Go, program, &[], Source::Path));
        }
        let go = tool::which("go", self.path());
        let gopath = match &go {
            Some(go) => self
                .job()
                .run(go, &["env", "GOPATH"], &self.home(), &mut |_| {})
                .ok()
                .filter(tool::Output::succeeded)
                .map(|output| output.stdout.trim().to_string())
                .unwrap_or_default(),
            None => self.home().join("go").display().to_string(),
        };
        let in_gopath = gopath
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(|dir| Path::new(dir).join("bin/gopls"))
            .find(|p| tool::is_executable(p));
        if let Some(program) = in_gopath {
            return Ok(Registry::launch(Language::Go, program, &[], Source::GoPath));
        }
        let ours = tools_bin(&self.tools, "gopls");
        if tool::is_executable(&ours) {
            return Ok(Registry::launch(Language::Go, ours, &[], Source::Tools));
        }
        Err(NotFound {
            language: Language::Go,
            install: Install::Gopls,
            missing: go.is_none().then_some("go"),
        })
    }

    fn rust_analyzer(&self, root: &Path) -> Result<Launch, NotFound> {
        let rustup = tool::which("rustup", self.path()).or_else(|| {
            let candidate = self.home().join(".cargo/bin/rustup");
            tool::is_executable(&candidate).then_some(candidate)
        });
        if let Some(rustup) = rustup
            && let Ok(output) = self.job().run(&rustup, &["which", "rust-analyzer"], root, &mut |_| {})
            && output.succeeded()
        {
            let program = PathBuf::from(output.stdout.trim());
            if tool::is_executable(&program) {
                return Ok(Registry::launch(Language::Rust, program, &[], Source::Rustup));
            }
        }
        // rustup puts a proxy called rust-analyzer on PATH that fails while the component is not installed.
        if let Some(program) = tool::which("rust-analyzer", self.path())
            && self.runs(&program, root)
        {
            return Ok(Registry::launch(Language::Rust, program, &[], Source::Path));
        }
        let ours = tools_bin(&self.tools, "rust-analyzer");
        if tool::is_executable(&ours) {
            return Ok(Registry::launch(Language::Rust, ours, &[], Source::Tools));
        }
        Err(NotFound {
            language: Language::Rust,
            install: Install::RustAnalyzer,
            missing: None,
        })
    }

    /// Whether `program --version` succeeds.
    fn runs(&self, program: &Path, dir: &Path) -> bool {
        self.job()
            .run(program, &["--version"], dir, &mut |_| {})
            .is_ok_and(|output| output.succeeded())
    }

    fn typescript(&self, root: &Path, trusted: &Path) -> Result<Launch, NotFound> {
        if tool::which("node", self.path()).is_none() {
            return Err(NotFound {
                language: Language::TypeScript,
                install: Install::Vtsls,
                missing: Some("node"),
            });
        }
        let tsc = root
            .ancestors()
            .take_while(|dir| dir.starts_with(trusted))
            .map(|dir| dir.join("node_modules/.bin/tsc"))
            .find(|p| tool::is_executable(p));
        if let Some(tsc) = tsc
            && self.speaks_lsp(&tsc, root)
        {
            let mut launch = Registry::launch(Language::TypeScript, tsc, &["--lsp", "--stdio"], Source::Project);
            launch.label = "tsc --lsp".to_string();
            return Ok(launch);
        }
        let ours = vtsls_bin(&self.tools);
        if tool::is_executable(&ours) {
            return Ok(Registry::launch(
                Language::TypeScript,
                ours,
                &["--stdio"],
                Source::Tools,
            ));
        }
        if let Some(program) = tool::which("vtsls", self.path()) {
            return Ok(Registry::launch(
                Language::TypeScript,
                program,
                &["--stdio"],
                Source::Path,
            ));
        }
        Err(NotFound {
            language: Language::TypeScript,
            install: Install::Vtsls,
            missing: None,
        })
    }

    /// Whether the TypeScript compiler `tsc` answers `initialize` when started with `--lsp --stdio`; remembered
    /// until the file changes.
    fn speaks_lsp(&self, tsc: &Path, root: &Path) -> bool {
        let key = fs::canonicalize(tsc).unwrap_or_else(|_| tsc.to_path_buf());
        let stamp = fs::metadata(&key).and_then(|m| m.modified()).ok();
        if let Some((seen, answer)) = self.probes.lock().unwrap_or_else(|e| e.into_inner()).get(&key)
            && *seen == stamp
        {
            return *answer;
        }
        let answer = probe(&self.supervisor, &self.env, tsc, root);
        self.probes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, (stamp, answer));
        answer
    }
}

/// The app's tools folder: `<data folder>/tools`.
pub fn default_tools() -> PathBuf {
    ide_core::paths::data_dir().join("tools")
}

/// Where a single-file server lives in the tools folder.
pub(crate) fn tools_bin(tools: &Path, name: &str) -> PathBuf {
    tools.join("bin").join(name)
}

/// Where npm puts vtsls in the tools folder.
pub(crate) fn vtsls_bin(tools: &Path) -> PathBuf {
    tools.join("node_modules/.bin/vtsls")
}

/// Answers the few requests a server might send before `initialize` is answered.
struct Quiet;

impl Handler for Quiet {
    fn notification(&self, _: &str, _: Value) {}

    fn request(&self, _: &str, _: Value, responder: Responder) {
        responder.ok(Value::Null);
    }

    fn closed(&self) {}
}

/// Starts `program --lsp --stdio` and asks `initialize`; true if it answers with capabilities. TypeScript 6 and
/// older answer `--lsp` with an unknown-option error and end.
fn probe(supervisor: &Supervisor, env: &[(String, String)], program: &Path, root: &Path) -> bool {
    let mut command = std::process::Command::new(program);
    command
        .args(["--lsp", "--stdio"])
        .current_dir(root)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let Ok(spawned) = supervisor.spawn(command, Kind::LanguageServer, "tsc --lsp (probe)") else {
        return false;
    };
    let grace = Duration::from_millis(500);
    let (Some(stdin), Some(stdout)) = (spawned.stdin, spawned.stdout) else {
        supervisor.stop(spawned.id, grace);
        return false;
    };
    let Ok(connection) = Connection::start(stdout, stdin, std::sync::Arc::new(Quiet), "probe") else {
        supervisor.stop(spawned.id, grace);
        return false;
    };
    let params = json!({
        "processId": std::process::id(),
        "rootUri": crate::uri::from_path(root),
        "capabilities": {"general": {"positionEncodings": [Encoding::Utf16.kind()]}},
    });
    let answer = ask(&connection, "initialize", params, PROBE_TIMEOUT);
    let speaks = answer.is_some_and(|result| result.get("capabilities").is_some_and(Value::is_object));
    if speaks {
        let _ = ask(&connection, "shutdown", Value::Null, grace);
        let _ = connection.notify("exit", Value::Null);
    }
    connection.end_input();
    supervisor.stop(spawned.id, grace);
    speaks
}

fn ask(connection: &Connection, method: &str, params: Value, timeout: Duration) -> Option<Value> {
    let (tx, rx) = std::sync::mpsc::channel();
    connection
        .request(
            method,
            params,
            timeout,
            Box::new(move |answer| {
                let _ = tx.send(answer);
            }),
        )
        .ok()?;
    rx.recv().ok()?.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ide-lsp-root-{}-{}",
            std::process::id(),
            files.join("+").replace('/', "_")
        ));
        let _ = fs::remove_dir_all(&dir);
        for file in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let text = if file.ends_with("ws/Cargo.toml") {
                "[package]\n\n[workspace]\nmembers = []\n"
            } else {
                ""
            };
            fs::write(path, text).unwrap();
        }
        dir
    }

    #[test]
    fn languages_and_document_ids_follow_the_file_name() {
        assert_eq!(Language::of_path(Path::new("a/b.go")), Some((Language::Go, "go")));
        assert_eq!(Language::of_path(Path::new("go.mod")), Some((Language::Go, "go.mod")));
        assert_eq!(
            Language::of_path(Path::new("x.tsx")),
            Some((Language::TypeScript, "typescriptreact"))
        );
        assert_eq!(
            Language::of_path(Path::new("x.mjs")),
            Some((Language::TypeScript, "javascript"))
        );
        assert_eq!(Language::of_path(Path::new("lib.rs")), Some((Language::Rust, "rust")));
        assert_eq!(Language::of_path(Path::new("README.md")), None);
    }

    #[test]
    fn a_go_work_wins_over_the_nearer_go_mod() {
        let dir = tree(&["go.work", "svc/go.mod", "svc/cmd/main.go", "other/x.go"]);
        assert_eq!(root(Language::Go, &dir.join("svc/cmd/main.go"), &dir), dir);
        let no_work = tree(&["svc/go.mod", "svc/cmd/main.go"]);
        assert_eq!(
            root(Language::Go, &no_work.join("svc/cmd/main.go"), &no_work),
            no_work.join("svc")
        );
    }

    #[test]
    fn the_topmost_cargo_workspace_wins_and_a_lone_package_is_its_own_root() {
        let dir = tree(&[
            "ws/Cargo.toml",
            "ws/crates/a/Cargo.toml",
            "ws/crates/a/src/lib.rs",
            "solo/Cargo.toml",
            "solo/src/main.rs",
        ]);
        assert_eq!(
            root(Language::Rust, &dir.join("ws/crates/a/src/lib.rs"), &dir),
            dir.join("ws")
        );
        assert_eq!(
            root(Language::Rust, &dir.join("solo/src/main.rs"), &dir),
            dir.join("solo")
        );
    }

    #[test]
    fn a_tsconfig_wins_over_a_package_json_and_the_search_stays_inside_the_trusted_folder() {
        let dir = tree(&[
            "package.json",
            "app/tsconfig.json",
            "app/src/main.ts",
            "lib/package.json",
            "lib/x.js",
        ]);
        assert_eq!(
            root(Language::TypeScript, &dir.join("app/src/main.ts"), &dir),
            dir.join("app")
        );
        assert_eq!(root(Language::TypeScript, &dir.join("lib/x.js"), &dir), dir.join("lib"));
        let inner = dir.join("app/src");
        assert_eq!(
            root(Language::TypeScript, &inner.join("main.ts"), &inner),
            inner,
            "never above the trusted folder"
        );
    }

    #[test]
    fn workspace_tables_are_told_from_lookalikes() {
        let dir = tree(&["a/Cargo.toml"]);
        let manifest = dir.join("a/Cargo.toml");
        for (text, expected) in [
            ("[workspace]\n", true),
            ("  [workspace.dependencies]\n", true),
            ("# [workspace]\n[package]\n", false),
            ("[workspaces]\n", false),
        ] {
            fs::write(&manifest, text).unwrap();
            assert_eq!(is_workspace(&manifest), expected, "{text:?}");
        }
    }

    #[test]
    fn servers_are_looked_for_on_the_given_path_and_in_the_tools_folder_only() {
        let tools = tree(&["bin/gopls"]);
        let gopls = tools.join("bin/gopls");
        fs::set_permissions(&gopls, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let env = vec![
            ("PATH".to_string(), "/nonexistent".to_string()),
            ("HOME".to_string(), "/nonexistent".to_string()),
        ];
        let registry = Registry::with_tools(&Supervisor::new(), env, tools.clone());
        let found = registry.find(Language::Go, &tools, &tools).unwrap();
        assert_eq!((found.program, found.source), (gopls, Source::Tools));
        let missing = registry.find(Language::Rust, &tools, &tools).unwrap_err();
        assert_eq!(missing.install, Install::RustAnalyzer);
        let no_node = registry.find(Language::TypeScript, &tools, &tools).unwrap_err();
        assert_eq!(no_node.missing, Some("node"));
    }
}
