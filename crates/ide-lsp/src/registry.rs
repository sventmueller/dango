//! Where the program of a language's server is: found on the login shell's PATH, through `rustup`, `go env` or
//! `xcrun`, in the project, in the app's tools folder, or as the user's settings name it.
//!
//! Only for trusted folders. Finding and starting a server runs code the project controls: `rustup` honours the
//! project's toolchain file, gopls and rust-analyzer run the project's build files, and the TypeScript server may be
//! the project's own `node_modules/.bin/tsc`. The caller decides about trust before it calls anything here; for an
//! untrusted folder it must not.

use ide_core::LockIgnoringPoison;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use ide_process::{Kind, Supervisor};
use serde_json::{Value, json};

use crate::encoding::Encoding;
use crate::language::{Common, CustomServer, Language};
use crate::rpc::{Connection, Handler, Responder};
use crate::server::ServerConfig;
use ide_process::tool::{self, Helper};

/// How long a helper such as `go env` or `rustup which` may take.
const HELPER_TIMEOUT: Duration = Duration::from_secs(20);

/// How long the TypeScript compiler may take to answer `initialize` when probed.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

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
    /// From Apple's developer tools, through `xcrun`.
    Xcode,
    /// As the user's settings name it.
    Settings,
}

/// How to start a server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Launch {
    pub(crate) language: Language,
    pub program: PathBuf,
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
    /// The ESLint server, built with npm from a fixed tag of `microsoft/vscode-eslint` whose archive has a known
    /// digest (E132, R38).
    Eslint,
    /// A package from npm, with `npm install` into the tools folder.
    Npm {
        /// The package, such as `@vtsls/language-server`.
        package: &'static str,
        /// The program it brings, such as `vtsls`.
        program: &'static str,
    },
}

/// The tag of `microsoft/vscode-eslint` the ESLint server is built from; an update is a new tag with a new digest.
pub(crate) const ESLINT_TAG: &str = "release/3.0.34";
/// The SHA-256 of GitHub's archive of `ESLINT_TAG`.
pub(crate) const ESLINT_DIGEST: &str = "f82fb2aab96eede6273af2d9aafeb9a29c5723c5af5a9819e37c61d7f51cc052";

/// Where the built ESLint server lies in the tools folder `tools`.
pub(crate) fn eslint_server(tools: &Path) -> PathBuf {
    tools
        .join("vscode-eslint")
        .join(ESLINT_TAG.replace('/', "-"))
        .join("server/out/eslintServer.js")
}

/// The folder of the .NET installation whose `dotnet` is `program`: the folder of the real program behind links,
/// or the `libexec` beside Homebrew's script.
fn dotnet_root(program: &Path) -> Option<PathBuf> {
    let real = fs::canonicalize(program).ok()?;
    let dir = real.parent()?;
    [dir.to_path_buf(), dir.parent()?.join("libexec")]
        .into_iter()
        .find(|root| root.join("host/fxr").is_dir())
}

/// vtsls, the TypeScript server for projects whose compiler does not speak the protocol.
pub const VTSLS: Install = Install::Npm {
    package: "@vtsls/language-server",
    program: "vtsls",
};

/// No server was found.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct NotFound {
    pub language: Language,
    /// The server's program, as the status bar names it.
    pub program: String,
    /// What the app can install, if anything.
    pub install: Option<Install>,
    /// A program that installing or running it needs and the login shell's PATH lacks, such as `go` or `node`.
    pub missing: Option<&'static str>,
    /// The command the user installs the server with, when the app cannot.
    pub hint: Option<&'static str>,
}

impl NotFound {
    fn new(language: &Language, program: &str, install: Option<Install>) -> NotFound {
        NotFound {
            language: language.clone(),
            program: program.to_string(),
            install,
            missing: None,
            hint: None,
        }
    }
}

/// Finds servers with the login shell's environment, never the app's own.
pub struct Registry {
    supervisor: Supervisor,
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
    pub fn find(&self, language: &Language, root: &Path, trusted: &Path) -> Result<Launch, NotFound> {
        match language {
            Language::Go => self.gopls(),
            Language::Rust => self.rust_analyzer(root),
            Language::TypeScript => self.typescript(root, trusted),
            Language::Custom(server) => self.custom(language, server),
            Language::Eslint => self.eslint(),
            other => match other.common() {
                Some(common) => self.common(other, &common),
                None => Err(NotFound::new(other, other.name(), None)),
            },
        }
    }

    /// The configuration to start `launch` in `root` with.
    pub fn config(&self, launch: &Launch, root: &Path) -> ServerConfig {
        let mut config = ServerConfig::new(&launch.label, &launch.program, root, self.env.clone());
        config.args.clone_from(&launch.args);
        match launch.language {
            Language::Go => {
                // gopls leaves semantic tokens and every kind of inlay hint off unless asked.
                let options = json!({
                    "semanticTokens": true,
                    "hints": {
                        "assignVariableTypes": true,
                        "compositeLiteralFields": true,
                        "constantValues": true,
                        "functionTypeParameters": true,
                        "parameterNames": true,
                        "rangeVariableTypes": true,
                    },
                });
                config.settings = json!({"gopls": options.clone()});
                config.initialization_options = Some(options);
            }
            Language::Rust => {
                // Priming analyses every crate of the dependency graph up front, the largest part of rust-analyzer's
                // memory; without it only what the open files reach is analysed.
                // Its project symbol search finds only types unless told otherwise.
                let options = json!({
                    "cachePriming": {"enable": false},
                    "workspace": {"symbol": {"search": {"kind": "all_symbols"}}},
                });
                config.settings = json!({"rust-analyzer": options.clone()});
                config.initialization_options = Some(options);
            }
            Language::Json | Language::Css | Language::Html => {
                // The servers taken from VS Code format only when asked to at the start.
                config.initialization_options = Some(json!({"provideFormatter": true}));
                // Without settings the CSS and JSON servers check nothing.
                if launch.language == Language::Css {
                    let validate = json!({"validate": true});
                    config.settings = json!({"css": validate, "scss": validate, "less": validate});
                } else if launch.language == Language::Json {
                    config.settings = json!({"json": {"validate": {"enable": true}}});
                }
            }
            Language::Yaml => {
                // Otherwise the server fetches the schema catalog from schemastore.org whenever it starts.
                config.settings = json!({"yaml": {"schemaStore": {"enable": false}}});
            }
            Language::Eslint => config.settings = eslint_settings(root, self.path()),
            Language::Php => {
                // Otherwise intelephense keeps its index in the temporary folder and its global state in the home
                // folder itself.
                let storage = ide_core::paths::cache_dir().join("intelephense").display().to_string();
                config.initialization_options = Some(json!({"storagePath": storage, "globalStoragePath": storage}));
            }
            Language::CSharp if tool::var(&self.env, "DOTNET_ROOT").is_none() => {
                // A tool from `dotnet tool install` finds the runtime only in /usr/local/share/dotnet unless
                // DOTNET_ROOT names it; Homebrew's `dotnet` is a script that sets it for itself alone.
                if let Some(dotnet) = tool::which("dotnet", self.path()).and_then(|d| dotnet_root(&d)) {
                    config.env.push(("DOTNET_ROOT".into(), dotnet.display().to_string()));
                }
            }
            Language::Java => {
                // jdtls keeps its workspace, one per project, in the folder `-data` names, by default in the
                // temporary folder.
                let data = ide_core::paths::cache_dir()
                    .join("jdtls")
                    .join(ide_core::paths::project_key(root));
                config.args.extend(["-data".into(), data.display().to_string()]);
            }
            _ => {}
        }
        config
    }

    /// The ESLint server the app built into its tools folder, run with the login shell's node.
    fn eslint(&self) -> Result<Launch, NotFound> {
        let node = tool::which("node", self.path());
        let script = eslint_server(&self.tools);
        match node {
            Some(node) if script.is_file() => Ok(Launch {
                language: Language::Eslint,
                program: node,
                args: vec![script.display().to_string(), "--stdio".into()],
                label: "eslint".into(),
                source: Source::Tools,
            }),
            Some(_) => Err(NotFound::new(&Language::Eslint, "ESLint", Some(Install::Eslint))),
            None => Err(NotFound {
                missing: Some("node"),
                ..NotFound::new(&Language::Eslint, "ESLint", Some(Install::Eslint))
            }),
        }
    }

    fn path(&self) -> Option<&str> {
        tool::var(&self.env, "PATH")
    }

    fn home(&self) -> PathBuf {
        tool::var(&self.env, "HOME").map_or_else(ide_core::paths::home, PathBuf::from)
    }

    fn job(&self) -> Helper<'_> {
        Helper {
            supervisor: &self.supervisor,
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

    /// A common language's server: on the PATH, from Apple's developer tools, or installed into the tools folder.
    fn common(&self, language: &Language, common: &Common) -> Result<Launch, NotFound> {
        for (name, args) in common.programs {
            let Some(program) = tool::which(name, self.path()) else {
                continue;
            };
            // /usr/bin/clangd and its kind only stand in for the developer tools, and ask to install them when
            // they are missing.
            if common.xcode && program.starts_with("/usr/bin") {
                if let Some(real) = self.xcrun(name) {
                    return Ok(Registry::launch(language.clone(), real, args, Source::Xcode));
                }
                continue;
            }
            return Ok(Registry::launch(language.clone(), program, args, Source::Path));
        }
        let home = self.home();
        for dir in common.dirs {
            for (name, args) in common.programs {
                let program = home.join(dir).join(name);
                if tool::is_executable(&program) {
                    return Ok(Registry::launch(language.clone(), program, args, Source::Path));
                }
            }
        }
        let (first, args) = common.programs[0];
        if let Some(package) = common.npm {
            let install = Install::Npm {
                package,
                program: first,
            };
            if tool::which("node", self.path()).is_none() {
                return Err(NotFound {
                    missing: Some("node"),
                    ..NotFound::new(language, first, Some(install))
                });
            }
            let ours = node_bin(&self.tools, first);
            if tool::is_executable(&ours) {
                return Ok(Registry::launch(language.clone(), ours, args, Source::Tools));
            }
            return Err(NotFound::new(language, first, Some(install)));
        }
        Err(NotFound {
            hint: common.hint,
            ..NotFound::new(language, first, None)
        })
    }

    /// Where `xcrun` finds `name` in the selected developer tools; `None` without them, never asking to install them.
    fn xcrun(&self, name: &str) -> Option<PathBuf> {
        let home = self.home();
        let selected = self
            .job()
            .run(Path::new("/usr/bin/xcode-select"), &["-p"], &home, &mut |_| {})
            .ok()?;
        if !selected.succeeded() {
            return None;
        }
        let found = self
            .job()
            .run(Path::new("/usr/bin/xcrun"), &["--find", name], &home, &mut |_| {})
            .ok()
            .filter(tool::Output::succeeded)?;
        let program = PathBuf::from(found.stdout.trim());
        tool::is_executable(&program).then_some(program)
    }

    /// A server the user set up: its program by path, with `~` for the home folder, or by name on the PATH or in the
    /// tools folder.
    fn custom(&self, language: &Language, server: &CustomServer) -> Result<Launch, NotFound> {
        let words = server.words();
        let Some((first, args)) = words.split_first() else {
            return Err(NotFound::new(language, &server.command, None));
        };
        let program = if first.contains('/') {
            let path = match first.strip_prefix("~/") {
                Some(rest) => self.home().join(rest),
                None => PathBuf::from(first),
            };
            tool::is_executable(&path).then_some(path)
        } else {
            tool::which(first, self.path()).or_else(|| {
                let ours = tools_bin(&self.tools, first);
                tool::is_executable(&ours).then_some(ours)
            })
        };
        let Some(program) = program else {
            return Err(NotFound::new(language, first, None));
        };
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        Ok(Registry::launch(language.clone(), program, &args, Source::Settings))
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
            missing: go.is_none().then_some("go"),
            ..NotFound::new(&Language::Go, "gopls", Some(Install::Gopls))
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
        Err(NotFound::new(
            &Language::Rust,
            "rust-analyzer",
            Some(Install::RustAnalyzer),
        ))
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
                missing: Some("node"),
                ..NotFound::new(&Language::TypeScript, "vtsls", Some(VTSLS))
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
        let ours = node_bin(&self.tools, "vtsls");
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
        Err(NotFound::new(&Language::TypeScript, "vtsls", Some(VTSLS)))
    }

    /// Whether the TypeScript compiler `tsc` answers `initialize` when started with `--lsp --stdio`; remembered
    /// until the file changes.
    fn speaks_lsp(&self, tsc: &Path, root: &Path) -> bool {
        let key = fs::canonicalize(tsc).unwrap_or_else(|_| tsc.to_path_buf());
        let stamp = fs::metadata(&key).and_then(|m| m.modified()).ok();
        if let Some((seen, answer)) = self.probes.lock_ignoring_poison().get(&key)
            && *seen == stamp
        {
            return *answer;
        }
        let answer = probe(&self.supervisor, &self.env, tsc, root);
        self.probes.lock_ignoring_poison().insert(key, (stamp, answer));
        answer
    }
}

/// The app's tools folder: `<data folder>/tools`.
pub fn default_tools() -> PathBuf {
    ide_core::paths::tools_dir()
}

/// Where a single-file server lives in the tools folder.
pub(crate) fn tools_bin(tools: &Path, name: &str) -> PathBuf {
    tools.join("bin").join(name)
}

/// Where npm puts the program `name` of a package in the tools folder.
pub(crate) fn node_bin(tools: &Path, name: &str) -> PathBuf {
    tools.join("node_modules/.bin").join(name)
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

/// What the ESLint server asks for with an empty section (E132): it checks nothing without a complete object.
pub fn eslint_settings(root: &Path, path: Option<&str>) -> serde_json::Value {
    let node_path = tool::which("node", path)
        .and_then(|node| node.parent().map(|bin| bin.join("../lib/node_modules")))
        .map(|p| p.display().to_string());
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    json!({
        "validate": "on",
        "run": "onType",
        "packageManager": "npm",
        "useESLintClass": false,
        "useFlatConfig": crate::language::flat_config(root),
        "workingDirectory": {"mode": "auto"},
        "workspaceFolder": {"uri": crate::uri::from_path(root).as_str(), "name": name},
        "nodePath": node_path,
        "format": false,
        "quiet": false,
        "onIgnoredFiles": "off",
        "options": {},
        "rulesCustomizations": [],
        "problems": {"shortenToSingleLine": false},
        "codeAction": {
            "disableRuleComment": {"enable": true, "location": "separateLine", "commentStyle": "line"},
            "showDocumentation": {"enable": true}
        },
        "codeActionOnSave": {"enable": false, "mode": "all"}
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(files: &[&str]) -> ide_core::testing::Scratch {
        let dir = ide_core::testing::scratch("lsp-registry");
        for file in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "").unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
        dir
    }

    fn registry(tools: &Path, path: &str) -> Registry {
        let env = vec![
            ("PATH".to_string(), path.to_string()),
            ("HOME".to_string(), "/nonexistent".to_string()),
        ];
        Registry::with_tools(&Supervisor::new(), env, tools.to_path_buf())
    }

    #[test]
    fn servers_are_looked_for_on_the_given_path_and_in_the_tools_folder_only() {
        let tools = tools(&["bin/gopls"]);
        let registry = registry(&tools, "/nonexistent");
        let found = registry.find(&Language::Go, &tools, &tools).unwrap();
        assert_eq!((found.program, found.source), (tools.join("bin/gopls"), Source::Tools));
        let missing = registry.find(&Language::Rust, &tools, &tools).unwrap_err();
        assert_eq!(missing.install, Some(Install::RustAnalyzer));
        let no_node = registry.find(&Language::TypeScript, &tools, &tools).unwrap_err();
        assert_eq!(no_node.missing, Some("node"));
    }

    #[test]
    fn a_common_server_is_found_on_the_path_in_its_order_of_preference() {
        let tools = tools(&["path/pylsp", "path/pyright-langserver"]);
        let registry = registry(&tools, &tools.join("path").display().to_string());
        let found = registry.find(&Language::Python, &tools, &tools).unwrap();
        assert_eq!(found.program, tools.join("path/pyright-langserver"));
        assert_eq!((found.args, found.source), (vec!["--stdio".to_string()], Source::Path));
    }

    #[test]
    fn a_missing_npm_server_is_offered_for_installation_and_found_there_afterwards() {
        let node = tools(&["path/node"]);
        let path = node.join("path").display().to_string();
        let missing = registry(&node, &path).find(&Language::Json, &node, &node).unwrap_err();
        assert_eq!(
            missing.install,
            Some(Install::Npm {
                package: "vscode-langservers-extracted",
                program: "vscode-json-language-server"
            })
        );
        assert_eq!(missing.missing, None);
        let installed = tools(&["path/node", "node_modules/.bin/vscode-css-language-server"]);
        let path = installed.join("path").display().to_string();
        let found = registry(&installed, &path)
            .find(&Language::Css, &installed, &installed)
            .unwrap();
        assert_eq!(found.source, Source::Tools);
        let no_node = registry(&installed, "/nonexistent")
            .find(&Language::Css, &installed, &installed)
            .unwrap_err();
        assert_eq!(no_node.missing, Some("node"), "the installed server needs node to run");
    }

    #[test]
    fn a_server_the_app_cannot_install_names_the_command_that_does() {
        let tools = tools(&[]);
        let missing = registry(&tools, "/nonexistent")
            .find(&Language::Markdown, &tools, &tools)
            .unwrap_err();
        assert_eq!(
            (missing.program.as_str(), missing.install, missing.hint),
            ("marksman", None, Some("brew install marksman"))
        );
    }

    #[test]
    fn the_users_server_runs_as_its_command_line_says() {
        let tools = tools(&["path/elixir-ls", "My Tools/srv"]);
        let registry = registry(&tools, &tools.join("path").display().to_string());
        let server = |command: String| {
            Language::Custom(std::sync::Arc::new(CustomServer {
                name: "Elixir".into(),
                extensions: vec!["ex".into()],
                command,
            }))
        };
        let by_name = registry
            .find(&server("elixir-ls --stdio".into()), &tools, &tools)
            .unwrap();
        assert_eq!(by_name.program, tools.join("path/elixir-ls"));
        assert_eq!(
            (by_name.args, by_name.source),
            (vec!["--stdio".to_string()], Source::Settings)
        );
        let by_path = registry
            .find(
                &server(format!("\"{}\"", tools.join("My Tools/srv").display())),
                &tools,
                &tools,
            )
            .unwrap();
        assert_eq!(by_path.program, tools.join("My Tools/srv"));
        let missing = registry.find(&server("nowhere -x".into()), &tools, &tools).unwrap_err();
        assert_eq!((missing.program.as_str(), missing.install), ("nowhere", None));
    }

    /// Writes executable shell scripts under `dir`, by path relative to it.
    fn scripts(dir: &Path, scripts: &[(&str, &str)]) {
        for (name, body) in scripts {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
    }

    fn with_home(tools: &Path, path: &str, home: &Path) -> Registry {
        let env = vec![
            ("PATH".to_string(), format!("{path}:/usr/bin:/bin")),
            ("HOME".to_string(), home.display().to_string()),
        ];
        Registry::with_tools(&Supervisor::new(), env, tools.to_path_buf())
    }

    #[test]
    fn gopls_is_found_in_the_gopath_go_names_or_in_the_home_folders_go() {
        let dir = tools(&["gopath/bin/gopls", "home/go/bin/gopls"]);
        let gopath = dir.join("gopath").display().to_string();
        scripts(
            &dir,
            &[("path/go", &format!("[ \"$1 $2\" = 'env GOPATH' ] && echo ':{gopath}'"))],
        );
        let with_go = with_home(
            &dir.join("tools"),
            &dir.join("path").display().to_string(),
            &dir.join("home"),
        );
        let found = with_go.find(&Language::Go, &dir, &dir).unwrap();
        assert_eq!(
            (found.program, found.source),
            (dir.join("gopath/bin/gopls"), Source::GoPath)
        );
        let without_go = with_home(&dir.join("tools"), "/nonexistent", &dir.join("home"));
        let found = without_go.find(&Language::Go, &dir, &dir).unwrap();
        assert_eq!(
            (found.program, found.source),
            (dir.join("home/go/bin/gopls"), Source::GoPath)
        );
        let nowhere = with_home(&dir.join("tools"), "/nonexistent", &dir.join("nohome"));
        let missing = nowhere.find(&Language::Go, &dir, &dir).unwrap_err();
        assert_eq!((missing.missing, missing.install), (Some("go"), Some(Install::Gopls)));
    }

    #[test]
    fn rust_analyzer_comes_from_rustup_for_the_projects_toolchain_or_a_working_one_on_the_path() {
        let dir = tools(&["toolchain/bin/rust-analyzer"]);
        let program = dir.join("toolchain/bin/rust-analyzer").display().to_string();
        scripts(
            &dir,
            &[
                (
                    "home/.cargo/bin/rustup",
                    &format!("[ \"$1 $2\" = 'which rust-analyzer' ] && echo '{program}'"),
                ),
                (
                    "proxy/rust-analyzer",
                    "echo 'error: rust-analyzer is not installed' >&2\nexit 1",
                ),
                ("real/rust-analyzer", "echo 'rust-analyzer 1.0.0'"),
            ],
        );
        let tools = dir.join("tools");
        let rustup = with_home(&tools, "/nonexistent", &dir.join("home"));
        let found = rustup.find(&Language::Rust, &dir, &dir).unwrap();
        assert_eq!(
            (found.program, found.source),
            (dir.join("toolchain/bin/rust-analyzer"), Source::Rustup)
        );
        let real = with_home(&tools, &dir.join("real").display().to_string(), &dir.join("nohome"));
        let found = real.find(&Language::Rust, &dir, &dir).unwrap();
        assert_eq!(
            (found.program, found.source),
            (dir.join("real/rust-analyzer"), Source::Path)
        );
        let proxy = with_home(&tools, &dir.join("proxy").display().to_string(), &dir.join("nohome"));
        let missing = proxy.find(&Language::Rust, &dir, &dir).unwrap_err();
        assert_eq!(
            missing.install,
            Some(Install::RustAnalyzer),
            "the failing proxy does not count"
        );
    }

    #[test]
    fn typescript_falls_back_to_vtsls_when_the_projects_compiler_does_not_speak_the_protocol() {
        let dir = tools(&["path/node"]);
        scripts(
            &dir,
            &[
                (
                    "project/node_modules/.bin/tsc",
                    "echo 'error TS5023: Unknown compiler option --lsp' >&2\nexit 1",
                ),
                ("vtsls-path/vtsls", ""),
            ],
        );
        let project = dir.join("project/src");
        fs::create_dir_all(&project).unwrap();
        let tools = dir.join("tools");
        let path = format!("{}:{}", dir.join("path").display(), dir.join("vtsls-path").display());
        let found = with_home(&tools, &path, &dir)
            .find(&Language::TypeScript, &project, &dir)
            .unwrap();
        assert_eq!(
            (found.program, found.source),
            (dir.join("vtsls-path/vtsls"), Source::Path)
        );
        assert_eq!(found.args, ["--stdio"]);
        let node_only = with_home(&tools, &dir.join("path").display().to_string(), &dir);
        let missing = node_only.find(&Language::TypeScript, &project, &dir).unwrap_err();
        assert_eq!(
            (missing.program.as_str(), missing.install, missing.missing),
            ("vtsls", Some(VTSLS), None)
        );
        scripts(&tools, &[("node_modules/.bin/vtsls", "")]);
        let found = node_only.find(&Language::TypeScript, &project, &dir).unwrap();
        assert_eq!(
            (found.program, found.source),
            (tools.join("node_modules/.bin/vtsls"), Source::Tools)
        );
    }

    #[test]
    fn clangd_from_usr_bin_only_counts_through_the_selected_developer_tools() {
        let tools = tools(&[]);
        let registry = with_home(&tools, "/nonexistent", &tools);
        match registry.find(&Language::C, &tools, &tools) {
            Ok(found) => {
                assert_eq!(found.source, Source::Xcode);
                assert!(!found.program.starts_with("/usr/bin"), "{}", found.program.display());
            }
            Err(missing) => assert_eq!(missing.hint, Some("xcode-select --install")),
        }
        scripts(&tools, &[("path/clangd", "")]);
        let own = with_home(&tools, &tools.join("path").display().to_string(), &tools);
        let found = own.find(&Language::C, &tools, &tools).unwrap();
        assert_eq!((found.program, found.source), (tools.join("path/clangd"), Source::Path));
    }

    #[test]
    fn csharp_ls_is_found_where_dotnet_installs_tools_without_it_on_the_path() {
        let dir = tools(&[]);
        let registry = with_home(&dir, "/nonexistent", &dir);
        let missing = registry.find(&Language::CSharp, &dir, &dir).unwrap_err();
        assert_eq!(missing.hint, Some("dotnet tool install --global csharp-ls"));
        scripts(&dir, &[(".dotnet/tools/csharp-ls", "")]);
        let found = registry.find(&Language::CSharp, &dir, &dir).unwrap();
        assert_eq!(found.program, dir.join(".dotnet/tools/csharp-ls"));
    }

    #[test]
    fn csharp_ls_is_told_where_homebrews_dotnet_keeps_the_runtime() {
        let dir = tools(&["Cellar/dotnet/libexec/host/fxr/11.0.0/libhostfxr.dylib"]);
        scripts(&dir, &[("Cellar/dotnet/bin/dotnet", "")]);
        fs::create_dir_all(dir.join("bin")).unwrap();
        std::os::unix::fs::symlink(dir.join("Cellar/dotnet/bin/dotnet"), dir.join("bin/dotnet")).unwrap();
        let registry = with_home(&dir, &dir.join("bin").display().to_string(), &dir);
        let launch = Registry::launch(Language::CSharp, dir.join("csharp-ls"), &[], Source::Path);
        let env = registry.config(&launch, &dir).env;
        let root = env
            .iter()
            .find(|(k, _)| k == "DOTNET_ROOT")
            .map(|(_, v)| PathBuf::from(v));
        assert_eq!(root, Some(fs::canonicalize(dir.join("Cellar/dotnet/libexec")).unwrap()));
    }

    #[test]
    fn servers_start_with_the_settings_they_need_to_do_their_work() {
        let tools = tools(&[]);
        let registry = registry(&tools, "/nonexistent");
        let config = |language: Language| {
            let launch = Registry::launch(language, PathBuf::from("/x/server"), &["--stdio"], Source::Path);
            registry.config(&launch, &tools)
        };
        let go = config(Language::Go);
        assert_eq!(go.settings["gopls"]["semanticTokens"], true);
        assert_eq!(go.initialization_options.unwrap()["hints"]["parameterNames"], true);
        let rust = config(Language::Rust);
        assert_eq!(rust.settings["rust-analyzer"]["cachePriming"]["enable"], false);
        assert_eq!(rust.args, ["--stdio"]);
        let css = config(Language::Css);
        assert_eq!(css.initialization_options.unwrap()["provideFormatter"], true);
        assert_eq!(css.settings["less"]["validate"], true);
        let json = config(Language::Json);
        assert_eq!(json.initialization_options.unwrap()["provideFormatter"], true);
        assert_eq!(json.settings["json"]["validate"]["enable"], true);
        assert_eq!(config(Language::Yaml).settings["yaml"]["schemaStore"]["enable"], false);
        let python = config(Language::Python);
        assert_eq!((python.settings, python.initialization_options), (Value::Null, None));
        let php = config(Language::Php).initialization_options.unwrap();
        assert!(php["storagePath"].as_str().unwrap().ends_with("/intelephense"), "{php}");
        let java = config(Language::Java).args;
        assert_eq!(java[..2], ["--stdio", "-data"]);
        assert!(java[2].contains("/jdtls/"), "{java:?}");
        assert_eq!(registry.tools(), tools);
    }
}
