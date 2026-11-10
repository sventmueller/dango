//! The client against the real servers, found the way the app finds them: with the login shell's environment.
//! They need installed tools or the network and take seconds to index, so they run only on request:
//! `cargo test -p ide-lsp --test real_servers -- --ignored --test-threads 1`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_lsp::lsp_types::{Diagnostic, Position, Uri};
use ide_lsp::{Cancel, Event, Http, Install, Installer, Language, Registry, Server, Source, Status, root, uri, wait};
use ide_process::Supervisor;
use ide_process::shell_env::{capture, user_shell};

/// How long a server may take to index a tiny project.
const PATIENCE: Duration = Duration::from_secs(120);

fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-lsp-real-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::canonicalize(dir).unwrap()
}

fn shell_env(supervisor: &Supervisor, dir: &Path) -> Vec<(String, String)> {
    capture(supervisor, &user_shell(), dir, Duration::from_secs(30)).expect("the login shell's environment")
}

/// The diagnostics the server publishes, by document.
type Published = Arc<Mutex<Vec<(Uri, Vec<Diagnostic>)>>>;

fn start(registry: &Registry, supervisor: &Supervisor, language: Language, root: &Path) -> (Server, Published) {
    let launch = registry.find(language, root, root).expect("a server");
    eprintln!(
        "{language:?}: {} {:?} from {:?}",
        launch.program.display(),
        launch.args,
        launch.source
    );
    let published: Published = Arc::default();
    let sink = published.clone();
    let server = Server::start(supervisor, registry.config(&launch, root), move |event| match event {
        Event::Diagnostics { uri, diagnostics, .. } => sink.lock().unwrap().push((uri, diagnostics)),
        Event::Crashed { stderr, .. } => eprintln!("crashed: {stderr}"),
        _ => {}
    })
    .unwrap();
    assert!(
        server.wait_until_running(PATIENCE),
        "{language:?} did not start: {:?}",
        server.status()
    );
    (server, published)
}

/// Asks until `f` gives something: servers answer empty while they index.
fn eventually<T>(what: &str, f: impl Fn() -> Option<T>) -> T {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(value) = f() {
            return value;
        }
        assert!(Instant::now() < deadline, "no {what} in time");
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn open(server: &Server, path: &Path, language_id: &str) -> Uri {
    let doc = uri::from_path(path);
    server.open(&doc, language_id, 1, &std::fs::read_to_string(path).unwrap());
    doc
}

fn published_errors(published: &Published, doc: &Uri) -> Option<Vec<Diagnostic>> {
    published
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(u, _)| u == doc)
        .map(|(_, d)| d.clone())
        .filter(|d| !d.is_empty())
}

/// Checks hover, completion and definition at the given places of `doc`.
fn check_features(
    server: &Server,
    doc: &Uri,
    hover_at: Position,
    complete_at: Position,
    expect: &str,
    define_at: Position,
    defined_line: u32,
) {
    let hover = eventually("hover", || wait(|r| server.hover(doc, hover_at, r)).ok().flatten());
    eprintln!("hover: {}", hover.markdown);
    assert!(!hover.markdown.is_empty());
    let completions = eventually("completion", || {
        let items = wait(|r| server.completion(doc, complete_at, None, r)).ok()?.items;
        items
            .iter()
            .any(|i| i.label.trim_start().starts_with(expect))
            .then_some(items)
    });
    eprintln!("completion: {} items", completions.len());
    let definition = eventually("definition", || {
        wait(|r| server.definition(doc, define_at, r))
            .ok()
            .filter(|links| !links.is_empty())
    });
    assert_eq!(definition[0].target_selection_range.start.line, defined_line);
    let tokens = eventually("semantic tokens", || {
        wait(|r| server.semantic_tokens(doc, r))
            .ok()
            .filter(|t| !t.tokens.is_empty())
    });
    let kinds: std::collections::BTreeSet<&str> = tokens.tokens.iter().filter_map(|t| tokens.kind(t)).collect();
    eprintln!("semantic tokens: {} of kinds {kinds:?}", tokens.tokens.len());
    assert!(!kinds.is_empty());
}

fn stop(server: &Server) {
    server.stop();
    assert!(server.wait_for(Duration::from_secs(10), |s| s == Status::Stopped));
}

#[test]
#[ignore = "needs gopls and go on the login shell's PATH"]
fn gopls_reports_diagnostics_and_answers_hover_completion_and_definition() {
    let dir = project(
        "go",
        &[
            ("go.mod", "module example.com/t\n\ngo 1.22\n"),
            (
                "main.go",
                "package main\n\nimport \"fmt\"\n\nfunc greet() string { return \"hi\" }\n\nfunc main() {\n\tfmt.Println(greet())\n\tvar x int = \"no\"\n\tfmt.\n}\n",
            ),
        ],
    );
    let supervisor = Supervisor::new();
    let registry = Registry::new(&supervisor, shell_env(&supervisor, &dir));
    assert_eq!(root(Language::Go, &dir.join("main.go"), &dir), dir);
    let (server, published) = start(&registry, &supervisor, Language::Go, &dir);
    let doc = open(&server, &dir.join("main.go"), "go");
    let errors = eventually("diagnostics", || published_errors(&published, &doc));
    eprintln!(
        "diagnostics: {:?}",
        errors
            .iter()
            .map(|d| (d.range.start.line, &d.message))
            .collect::<Vec<_>>()
    );
    check_features(
        &server,
        &doc,
        Position::new(7, 6),
        Position::new(9, 5),
        "Println",
        Position::new(7, 14),
        4,
    );
    stop(&server);
}

#[test]
#[ignore = "needs rust-analyzer through rustup or on the login shell's PATH"]
fn rust_analyzer_reports_diagnostics_and_answers_hover_completion_and_definition() {
    let dir = project(
        "rust",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/main.rs",
                "fn greet() -> String {\n    String::from(\"hi\")\n}\n\nfn main() {\n    let s = greet();\n    let n: u32 = \"no\";\n    String::new();\n}\n",
            ),
        ],
    );
    let supervisor = Supervisor::new();
    let registry = Registry::new(&supervisor, shell_env(&supervisor, &dir));
    let (server, published) = start(&registry, &supervisor, Language::Rust, &dir);
    let doc = open(&server, &dir.join("src/main.rs"), "rust");
    check_features(
        &server,
        &doc,
        Position::new(5, 13),
        Position::new(7, 12),
        "new",
        Position::new(5, 13),
        0,
    );
    let errors = eventually("diagnostics", || published_errors(&published, &doc));
    assert!(errors.iter().any(|d| d.range.start.line == 6), "{errors:?}");
    stop(&server);
}

const TS_MAIN: &str =
    "function greet(): string {\n  return \"hi\";\n}\n\nconst s = greet();\nconst n: number = \"no\";\ns.\n";

fn ts_project(name: &str, typescript: &str, env: &[(String, String)]) -> PathBuf {
    let dir = project(
        name,
        &[
            ("package.json", "{\"name\": \"t\", \"private\": true}\n"),
            (
                "tsconfig.json",
                "{\"compilerOptions\": {\"strict\": true, \"noEmit\": true}}\n",
            ),
            ("src/main.ts", TS_MAIN),
        ],
    );
    let npm = env
        .iter()
        .find(|(k, _)| k == "PATH")
        .and_then(|(_, path)| path.split(':').map(|d| Path::new(d).join("npm")).find(|p| p.exists()))
        .expect("npm on the login shell's PATH");
    let output = std::process::Command::new(npm)
        .args(["install", "--no-fund", "--no-audit", typescript])
        .current_dir(&dir)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    dir
}

fn check_typescript(server: &Server, dir: &Path, published: &Published) {
    let doc = open(server, &dir.join("src/main.ts"), "typescript");
    check_features(
        server,
        &doc,
        Position::new(4, 11),
        Position::new(6, 2),
        "charAt",
        Position::new(4, 11),
        0,
    );
    let errors = eventually("diagnostics", || {
        published_errors(published, &doc).or_else(|| {
            wait(|r| server.pull_diagnostics(&doc, r))
                .ok()
                .filter(|d| !d.is_empty())
        })
    });
    assert!(errors.iter().any(|d| d.range.start.line == 5), "{errors:?}");
}

#[test]
#[ignore = "needs node and npm on the login shell's PATH and the network to install typescript@7"]
fn typescript_7_serves_the_project_with_its_own_compiler() {
    let supervisor = Supervisor::new();
    let env = shell_env(&supervisor, &std::env::temp_dir());
    let dir = ts_project("ts7", "typescript@7", &env);
    let registry = Registry::new(&supervisor, env);
    let launch = registry.find(Language::TypeScript, &dir, &dir).unwrap();
    assert_eq!(launch.source, Source::Project);
    assert_eq!(launch.args, ["--lsp", "--stdio"]);
    let (server, published) = start(&registry, &supervisor, Language::TypeScript, &dir);
    check_typescript(&server, &dir, &published);
    stop(&server);
}

#[test]
#[ignore = "needs node and npm on the login shell's PATH and the network to install typescript@5 and vtsls"]
fn an_older_typescript_falls_back_to_vtsls_installed_into_the_tools_folder() {
    let supervisor = Supervisor::new();
    let env = shell_env(&supervisor, &std::env::temp_dir());
    let dir = ts_project("ts5", "typescript@5", &env);
    let tools = project("tools-vtsls", &[]);
    let installer = Installer::new(&supervisor, env.clone(), tools.clone(), Arc::new(Http::new()));
    let program = installer
        .install(Install::Vtsls, &Cancel::new(), &mut |p| eprintln!("{p:?}"))
        .unwrap();
    assert!(program.ends_with("node_modules/.bin/vtsls"));
    let registry = Registry::with_tools(&supervisor, env, tools);
    let launch = registry.find(Language::TypeScript, &dir, &dir).unwrap();
    assert_eq!(launch.source, Source::Tools, "TypeScript 5 does not speak the protocol");
    let (server, published) = start(&registry, &supervisor, Language::TypeScript, &dir);
    check_typescript(&server, &dir, &published);
    stop(&server);
}

#[test]
#[ignore = "downloads rust-analyzer from GitHub"]
fn rust_analyzer_is_installed_from_its_release_and_runs() {
    let supervisor = Supervisor::new();
    let tools = project("tools-ra", &[]);
    let env = vec![("PATH".to_string(), "/usr/bin:/bin".to_string())];
    let installer = Installer::new(&supervisor, env, tools.clone(), Arc::new(Http::new()));
    let mut last = None;
    let program = installer
        .install(Install::RustAnalyzer, &Cancel::new(), &mut |p| last = Some(p))
        .unwrap();
    assert_eq!(program, tools.join("bin/rust-analyzer"));
    let version = std::process::Command::new(&program).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("rust-analyzer"));
    assert_eq!(last, Some(ide_lsp::Progress::Step(ide_lsp::Step::Done)));
}

#[test]
#[ignore = "needs go on the login shell's PATH and the network; compiles gopls"]
fn gopls_is_installed_with_go_install_into_the_tools_folder() {
    let supervisor = Supervisor::new();
    let tools = project("tools-gopls", &[]);
    let env = shell_env(&supervisor, &tools);
    let installer = Installer::new(&supervisor, env, tools.clone(), Arc::new(Http::new()));
    let program = installer
        .install(Install::Gopls, &Cancel::new(), &mut |p| eprintln!("{p:?}"))
        .unwrap();
    assert_eq!(program, tools.join("bin/gopls"));
}
