//! The client against the real servers, found the way the app finds them: with the login shell's environment.
//! They need installed tools or the network and take seconds to index, so they run only on request:
//! `cargo test -p ide-lsp --test real_servers -- --ignored --test-threads 1`.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_lsp::lsp_types::{Diagnostic, Position, Uri};
use ide_lsp::{Cancel, Event, Http, Install, Installer, Language, Registry, Server, Source, Status, root, uri, wait};
use ide_process::Supervisor;
use ide_process::shell_env::{capture, user_shell};

/// How long a server may take to index a tiny project.
const PATIENCE: Duration = Duration::from_secs(120);

fn project(name: &str, files: &[(&str, &str)]) -> ide_core::testing::Scratch {
    let dir = ide_core::testing::scratch(&format!("lsp-real-{name}"));
    for (file, text) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

fn shell_env(supervisor: &Supervisor, dir: &Path) -> Vec<(String, String)> {
    capture(supervisor, &user_shell(), dir, Duration::from_secs(30)).expect("the login shell's environment")
}

/// The diagnostics the server publishes, by document.
type Published = Arc<Mutex<Vec<(Uri, Vec<Diagnostic>)>>>;

fn start(registry: &Registry, supervisor: &Supervisor, language: Language, root: &Path) -> (Server, Published) {
    let launch = registry.find(&language, root, root).expect("a server");
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
    assert_eq!(root(&Language::Go, &dir.join("main.go"), &dir), dir);
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

fn ts_project(name: &str, typescript: &str, env: &[(String, String)]) -> ide_core::testing::Scratch {
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
    let launch = registry.find(&Language::TypeScript, &dir, &dir).unwrap();
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
    let installer = Installer::new(
        &supervisor,
        env.clone(),
        tools.to_path_buf(),
        Arc::new(Http::new("language server installer")),
    );
    let program = installer
        .install(ide_lsp::VTSLS, &Cancel::new(), &mut |p| eprintln!("{p:?}"))
        .unwrap();
    assert!(program.ends_with("node_modules/.bin/vtsls"));
    let registry = Registry::with_tools(&supervisor, env, tools.to_path_buf());
    let launch = registry.find(&Language::TypeScript, &dir, &dir).unwrap();
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
    let installer = Installer::new(
        &supervisor,
        env,
        tools.to_path_buf(),
        Arc::new(Http::new("language server installer")),
    );
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
    let installer = Installer::new(
        &supervisor,
        env,
        tools.to_path_buf(),
        Arc::new(Http::new("language server installer")),
    );
    let program = installer
        .install(Install::Gopls, &Cancel::new(), &mut |p| eprintln!("{p:?}"))
        .unwrap();
    assert_eq!(program, tools.join("bin/gopls"));
}

/// The document's diagnostics, published or, from a server that offers it, pulled as the editor does.
fn diagnostics(server: &Server, published: &Published, doc: &Uri) -> Vec<Diagnostic> {
    eventually("diagnostics", || {
        published_errors(published, doc)
            .or_else(|| wait(|r| server.pull_diagnostics(doc, r)).ok().filter(|d| !d.is_empty()))
    })
}

/// The names of the document's symbols at the top level, once the server has some.
fn symbols(server: &Server, doc: &Uri) -> Vec<String> {
    eventually("symbols", || {
        wait(|r| server.document_symbols(doc, r)).ok().filter(|s| !s.is_empty())
    })
    .into_iter()
    .map(|s| s.name)
    .collect()
}

#[test]
#[ignore = "needs Apple's developer tools"]
fn clangd_from_the_developer_tools_reports_diagnostics_and_symbols() {
    let dir = project(
        "c",
        &[
            ("compile_flags.txt", "-std=c17\n"),
            (
                "main.c",
                "int twice(int n) { return n * 2; }\n\nint main(void) {\n    int x = undeclared;\n    return twice(x);\n}\n",
            ),
        ],
    );
    let supervisor = Supervisor::new();
    let registry = Registry::new(&supervisor, shell_env(&supervisor, &dir));
    let launch = registry.find(&Language::C, &dir, &dir).unwrap();
    assert!(
        !launch.program.starts_with("/usr/bin"),
        "the stand-in in /usr/bin is resolved: {launch:?}"
    );
    let (server, published) = start(&registry, &supervisor, Language::C, &dir);
    let doc = open(&server, &dir.join("main.c"), "c");
    let errors = diagnostics(&server, &published, &doc);
    assert!(errors.iter().any(|d| d.range.start.line == 3), "{errors:?}");
    let names = symbols(&server, &doc);
    assert!(names.iter().any(|n| n.starts_with("twice")), "{names:?}");
    stop(&server);
}

#[test]
#[ignore = "needs Apple's developer tools"]
fn sourcekit_lsp_reports_diagnostics_in_a_swift_package() {
    let dir = project(
        "swift",
        &[
            (
                "Package.swift",
                "// swift-tools-version:5.9\nimport PackageDescription\n\nlet package = Package(name: \"T\", targets: [.executableTarget(name: \"T\")])\n",
            ),
            (
                "Sources/T/main.swift",
                "func twice(_ n: Int) -> Int { n * 2 }\nlet x: Int = \"no\"\nprint(twice(x))\n",
            ),
        ],
    );
    let supervisor = Supervisor::new();
    let registry = Registry::new(&supervisor, shell_env(&supervisor, &dir));
    assert_eq!(root(&Language::Swift, &dir.join("Sources/T/main.swift"), &dir), dir);
    let (server, published) = start(&registry, &supervisor, Language::Swift, &dir);
    let doc = open(&server, &dir.join("Sources/T/main.swift"), "swift");
    let errors = diagnostics(&server, &published, &doc);
    assert!(errors.iter().any(|d| d.range.start.line == 1), "{errors:?}");
    let names = symbols(&server, &doc);
    assert!(names.iter().any(|n| n.starts_with("twice")), "{names:?}");
    stop(&server);
}

#[test]
#[ignore = "needs node and npm on the login shell's PATH and the network to install the servers"]
fn the_npm_servers_install_into_the_tools_folder_and_serve_their_languages() {
    let supervisor = Supervisor::new();
    let env = shell_env(&supervisor, &std::env::temp_dir());
    let tools = project("tools-npm", &[]);
    let dir = project(
        "npm-languages",
        &[
            (
                "a.py",
                "def twice(n: int) -> int:\n    return n * 2\n\nx: int = \"no\"\n",
            ),
            ("a.json", "{\"a\": 1,, }\n"),
            ("a.yaml", "a: [1, 2\nb: 3\n"),
            ("a.css", "a { colr: red; }\n"),
            ("a.html", "<html><body><h1 id=\"top\">Hi</h1></body></html>\n"),
            ("a.sh", "greet() {\n  echo hi\n}\n"),
            (
                "a.php",
                "<?php\nfunction twice(int $n): int { return $n * 2; }\necho twice(;\n",
            ),
        ],
    );
    let registry = Registry::with_tools(&supervisor, env.clone(), tools.to_path_buf());
    let installer = Installer::new(
        &supervisor,
        env,
        tools.to_path_buf(),
        Arc::new(Http::new("language server installer")),
    );
    let cases = [
        (Language::Css, "a.css", "css", true),
        (Language::Python, "a.py", "python", true),
        (Language::Json, "a.json", "json", true),
        (Language::Yaml, "a.yaml", "yaml", true),
        (Language::Html, "a.html", "html", false),
        (Language::Shell, "a.sh", "shellscript", false),
        (Language::Php, "a.php", "php", true),
    ];
    for (language, file, id, diagnoses) in cases {
        if let Err(missing) = registry.find(&language, &dir, &dir) {
            let install = missing.install.expect("an npm package");
            installer
                .install(install, &Cancel::new(), &mut |p| eprintln!("{p:?}"))
                .unwrap();
            assert_eq!(registry.find(&language, &dir, &dir).unwrap().source, Source::Tools);
        }
        let (server, published) = start(&registry, &supervisor, language.clone(), &dir);
        let doc = open(&server, &dir.join(file), id);
        if diagnoses {
            let errors = diagnostics(&server, &published, &doc);
            eprintln!("{language:?}: {errors:?}");
        } else {
            let names = symbols(&server, &doc);
            eprintln!("{language:?}: {names:?}");
        }
        stop(&server);
    }
}

/// Starts the server of `language` in `dir`, opens `file` and expects an error on line `line` and a symbol `name`.
fn serves(language: Language, dir: &Path, file: &str, id: &str, line: u32, name: &str) {
    let supervisor = Supervisor::new();
    let registry = Registry::new(&supervisor, shell_env(&supervisor, dir));
    let (server, published) = start(&registry, &supervisor, language.clone(), dir);
    let doc = open(&server, &dir.join(file), id);
    let errors = diagnostics(&server, &published, &doc);
    assert!(
        errors.iter().any(|d| d.range.start.line == line),
        "{language:?}: {errors:?}"
    );
    let names = symbols(&server, &doc);
    assert!(names.iter().any(|n| n.starts_with(name)), "{language:?}: {names:?}");
    stop(&server);
}

#[test]
#[ignore = "needs ruby-lsp and a Ruby of version 3 or later on the login shell's PATH"]
fn ruby_lsp_reports_diagnostics_and_symbols() {
    let dir = project("ruby", &[("a.rb", "def twice(n)\n  n * 2\nend\n\nputs(twice(1)\n")]);
    serves(Language::Ruby, &dir, "a.rb", "ruby", 4, "twice");
}

#[test]
#[ignore = "needs csharp-ls and the .NET SDK"]
fn csharp_ls_reports_diagnostics_and_symbols_in_a_project() {
    let dir = project(
        "csharp",
        &[
            (
                "T.csproj",
                "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net10.0</TargetFramework>\n  </PropertyGroup>\n</Project>\n",
            ),
            (
                "Program.cs",
                "class Program\n{\n    static void Main()\n    {\n        int x = \"no\";\n    }\n}\n",
            ),
        ],
    );
    serves(Language::CSharp, &dir, "Program.cs", "csharp", 4, "Program");
}

#[test]
#[ignore = "needs jdtls and a Java of version 21 or later, as Homebrew installs them"]
fn jdtls_reports_diagnostics_and_symbols_in_a_lone_file() {
    let dir = project(
        "java",
        &[(
            "A.java",
            "public class A {\n    static int twice(int n) { return n * 2; }\n    static int x = \"no\";\n}\n",
        )],
    );
    serves(Language::Java, &dir, "A.java", "java", 2, "A");
}

#[test]
#[ignore = "needs taplo, marksman and lua-language-server on the login shell's PATH, as Homebrew installs them"]
fn the_homebrew_servers_serve_toml_markdown_and_lua() {
    let dir = project(
        "brew-languages",
        &[
            ("a.toml", "[a]\nb = 1\nb = 2\n"),
            ("a.md", "# Title\n\n## Part\n\nSee [missing](#nowhere).\n"),
            (
                "a.lua",
                "local function twice(n)\n  return n * 2\nend\nprint(twice(undefined_name))\n",
            ),
        ],
    );
    let supervisor = Supervisor::new();
    let registry = Registry::new(&supervisor, shell_env(&supervisor, &dir));
    let cases = [
        (Language::Toml, "a.toml", "toml", true),
        (Language::Markdown, "a.md", "markdown", false),
        (Language::Lua, "a.lua", "lua", true),
    ];
    for (language, file, id, diagnoses) in cases {
        let (server, published) = start(&registry, &supervisor, language.clone(), &dir);
        let doc = open(&server, &dir.join(file), id);
        if diagnoses {
            let errors = diagnostics(&server, &published, &doc);
            eprintln!("{language:?}: {errors:?}");
        } else {
            let names = symbols(&server, &doc);
            assert!(names.iter().any(|n| n.contains("Title")), "{names:?}");
        }
        stop(&server);
    }
}
