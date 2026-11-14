//! The languages the editor has servers for: which language a file is in, where its project root is, and which
//! programs serve it. Go, Rust and TypeScript have a lookup of their own in the registry; the common servers below are
//! looked for on the PATH; the user's own servers come from the settings and win over the built-in ones.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// A language with a server.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Language {
    /// Go, served by gopls.
    Go,
    /// Rust, served by rust-analyzer.
    Rust,
    /// TypeScript and JavaScript, served by TypeScript 7's own server or vtsls.
    TypeScript,
    /// Python, served by basedpyright, pyright or pylsp.
    Python,
    /// C, C++ and Objective-C, served by clangd.
    C,
    /// Swift, served by sourcekit-lsp.
    Swift,
    /// Shell scripts, served by bash-language-server.
    Shell,
    /// YAML, served by yaml-language-server.
    Yaml,
    /// JSON, served by the JSON server taken from VS Code.
    Json,
    /// CSS, SCSS and Less, served by the CSS server taken from VS Code.
    Css,
    /// HTML, served by the HTML server taken from VS Code.
    Html,
    /// TOML, served by taplo.
    Toml,
    /// Markdown, served by marksman.
    Markdown,
    /// Lua, served by lua-language-server.
    Lua,
    /// A server the user set up in the settings.
    Custom(Arc<CustomServer>),
    /// ESLint, a second server beside the language's for JavaScript and TypeScript (E132).
    Eslint,
    /// Emmet's abbreviations, a second server beside the language's for HTML, CSS and JSX (E109).
    Emmet,
}

/// The built-in languages, in the order lists show them.
pub const BUILT_IN: [Language; 14] = [
    Language::C,
    Language::Css,
    Language::Go,
    Language::Html,
    Language::Json,
    Language::Lua,
    Language::Markdown,
    Language::Python,
    Language::Rust,
    Language::Shell,
    Language::Swift,
    Language::Toml,
    Language::TypeScript,
    Language::Yaml,
];

/// How a common server is found and installed.
pub(crate) struct Common {
    /// The programs that serve the language with their arguments, the preferred first.
    pub(crate) programs: &'static [(&'static str, &'static [&'static str])],
    /// The npm package that brings the first program, which the app can install into its tools folder.
    pub(crate) npm: Option<&'static str>,
    /// Whether the programs come with Apple's developer tools.
    pub(crate) xcode: bool,
    /// The command the user installs the server with, when the app cannot.
    pub(crate) hint: Option<&'static str>,
    /// Files that mark a project root; the nearest wins, and without one the trusted folder is the root.
    pub(crate) markers: &'static [&'static str],
}

/// The files whose presence says that a folder configures ESLint.
const ESLINT_CONFIGS: &[&str] = &[
    "eslint.config.js",
    "eslint.config.mjs",
    "eslint.config.cjs",
    "eslint.config.ts",
    "eslint.config.mts",
    "eslint.config.cts",
    ".eslintrc",
    ".eslintrc.js",
    ".eslintrc.cjs",
    ".eslintrc.json",
    ".eslintrc.yaml",
    ".eslintrc.yml",
];

/// Whether ESLint in `root` reads the flat configuration of ESLint 9: a file `eslint.config.*`, or an installed ESLint
/// of version 9 or later without an `.eslintrc*` (as Zed decides).
pub fn flat_config(root: &Path) -> bool {
    if ESLINT_CONFIGS.iter().take(6).any(|name| root.join(name).is_file()) {
        return true;
    }
    if ESLINT_CONFIGS.iter().skip(6).any(|name| root.join(name).is_file()) {
        return false;
    }
    fs::read_to_string(root.join("node_modules/eslint/package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|package| package.get("version")?.as_str()?.split('.').next()?.parse::<u32>().ok())
        .is_some_and(|major| major >= 9)
}

/// Whether the project of `path` within `root` uses ESLint: a configuration file in a folder up to `root`, or
/// `eslint` among a `package.json`'s dependencies.
pub fn uses_eslint(path: &Path, root: &Path) -> bool {
    path.ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(root))
        .any(|dir| {
            if ESLINT_CONFIGS.iter().any(|name| dir.join(name).is_file()) {
                return true;
            }
            fs::read_to_string(dir.join("package.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .is_some_and(|package| {
                    ["dependencies", "devDependencies"]
                        .iter()
                        .any(|field| package.get(field).and_then(|d| d.get("eslint")).is_some())
                        || package.get("eslintConfig").is_some()
                })
        })
}

/// The second servers for `path` in the trusted folder `root` (E140): ESLint for JavaScript and TypeScript where the
/// project uses it, Emmet for HTML, CSS and JSX while `emmet` is on.
pub fn companions(path: &Path, root: &Path, emmet: bool) -> Vec<Language> {
    let extension = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    let extension = extension.as_deref().unwrap_or_default();
    let mut out = Vec::new();
    let script = matches!(extension, "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts");
    if script && uses_eslint(path, root) {
        out.push(Language::Eslint);
    }
    if emmet && matches!(extension, "html" | "htm" | "css" | "scss" | "less" | "jsx" | "tsx") {
        out.push(Language::Emmet);
    }
    out
}

impl Language {
    /// The built-in language of `path` and the protocol's language id for the document.
    pub fn of_path(path: &Path) -> Option<(Language, &'static str)> {
        let name = path.file_name()?.to_str()?;
        match name {
            "go.mod" => return Some((Language::Go, "go.mod")),
            "go.work" => return Some((Language::Go, "go.work")),
            _ => {}
        }
        Some(match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "go" => (Language::Go, "go"),
            "rs" => (Language::Rust, "rust"),
            "ts" | "mts" | "cts" => (Language::TypeScript, "typescript"),
            "tsx" => (Language::TypeScript, "typescriptreact"),
            "js" | "mjs" | "cjs" => (Language::TypeScript, "javascript"),
            "jsx" => (Language::TypeScript, "javascriptreact"),
            "py" | "pyi" => (Language::Python, "python"),
            "c" | "h" => (Language::C, "c"),
            "cc" | "cpp" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "h++" | "ipp" => (Language::C, "cpp"),
            "m" => (Language::C, "objective-c"),
            "mm" => (Language::C, "objective-cpp"),
            "swift" => (Language::Swift, "swift"),
            "sh" | "bash" => (Language::Shell, "shellscript"),
            "yaml" | "yml" => (Language::Yaml, "yaml"),
            "json" => (Language::Json, "json"),
            "jsonc" => (Language::Json, "jsonc"),
            "css" => (Language::Css, "css"),
            "scss" => (Language::Css, "scss"),
            "less" => (Language::Css, "less"),
            "html" | "htm" => (Language::Html, "html"),
            "toml" => (Language::Toml, "toml"),
            "md" | "markdown" => (Language::Markdown, "markdown"),
            "lua" => (Language::Lua, "lua"),
            _ => return None,
        })
    }

    /// The language of `path` and the document's language id: a server of `custom` that takes its extension first,
    /// else the built-in one.
    pub fn detect(path: &Path, custom: &[Arc<CustomServer>]) -> Option<(Language, String)> {
        let extension = path.extension().and_then(|e| e.to_str());
        if let Some(server) = extension.and_then(|e| custom.iter().find(|s| s.serves(e))) {
            return Some((Language::Custom(server.clone()), server.language_id()));
        }
        Language::of_path(path).map(|(language, id)| (language, id.to_string()))
    }

    /// The name users read.
    pub fn name(&self) -> &str {
        match self {
            Language::Go => "Go",
            Language::Rust => "Rust",
            Language::TypeScript => "TypeScript",
            Language::Python => "Python",
            Language::C => "C/C++",
            Language::Swift => "Swift",
            Language::Shell => "Shell",
            Language::Yaml => "YAML",
            Language::Json => "JSON",
            Language::Css => "CSS",
            Language::Html => "HTML",
            Language::Toml => "TOML",
            Language::Markdown => "Markdown",
            Language::Lua => "Lua",
            Language::Custom(server) => &server.name,
            Language::Eslint => "ESLint",
            Language::Emmet => "Emmet",
        }
    }

    /// Whether it is a second server beside a document's own (E140).
    pub fn is_companion(&self) -> bool {
        matches!(self, Language::Eslint | Language::Emmet)
    }

    /// Whether its servers search the project's symbols; the servers of data and style formats do not, so a project
    /// symbol search does not start them.
    pub fn searches_project(&self) -> bool {
        !matches!(
            self,
            Language::Json
                | Language::Yaml
                | Language::Toml
                | Language::Css
                | Language::Html
                | Language::Eslint
                | Language::Emmet
        )
    }

    /// How the server of a common language is found; `None` for Go, Rust, TypeScript and the user's own servers.
    pub(crate) fn common(&self) -> Option<Common> {
        const VSCODE: Option<&str> = Some("vscode-langservers-extracted");
        let common = |programs, npm, markers| Common {
            programs,
            npm,
            xcode: false,
            hint: None,
            markers,
        };
        let brew = |program, hint, markers| Common {
            programs: program,
            npm: None,
            xcode: false,
            hint: Some(hint),
            markers,
        };
        let xcode = |programs, markers| Common {
            programs,
            npm: None,
            xcode: true,
            hint: Some("xcode-select --install"),
            markers,
        };
        Some(match self {
            Language::Python => common(
                &[
                    ("basedpyright-langserver", &["--stdio"]),
                    ("pyright-langserver", &["--stdio"]),
                    ("pylsp", &[]),
                ],
                Some("basedpyright"),
                &[
                    "pyproject.toml",
                    "pyrightconfig.json",
                    "setup.py",
                    "setup.cfg",
                    "Pipfile",
                    "requirements.txt",
                ],
            ),
            Language::Shell => common(
                &[("bash-language-server", &["start"])],
                Some("bash-language-server"),
                &[],
            ),
            Language::Yaml => common(
                &[("yaml-language-server", &["--stdio"])],
                Some("yaml-language-server"),
                &[],
            ),
            Language::Json => common(&[("vscode-json-language-server", &["--stdio"])], VSCODE, &[]),
            Language::Css => common(&[("vscode-css-language-server", &["--stdio"])], VSCODE, &[]),
            Language::Html => common(&[("vscode-html-language-server", &["--stdio"])], VSCODE, &[]),
            Language::C => xcode(
                &[("clangd", &[])],
                &["compile_commands.json", "compile_flags.txt", ".clangd"],
            ),
            Language::Swift => xcode(&[("sourcekit-lsp", &[])], &["Package.swift"]),
            Language::Toml => brew(
                &[("taplo", &["lsp", "stdio"])],
                "brew install taplo",
                &[".taplo.toml", "taplo.toml"],
            ),
            Language::Markdown => brew(
                &[("marksman", &["server"])],
                "brew install marksman",
                &[".marksman.toml"],
            ),
            Language::Lua => brew(
                &[("lua-language-server", &[])],
                "brew install lua-language-server",
                &[".luarc.json", ".luarc.jsonc"],
            ),
            Language::Emmet => common(
                &[("emmet-language-server", &["--stdio"])],
                Some("@olrtg/emmet-language-server"),
                &[],
            ),
            Language::Go | Language::Rust | Language::TypeScript | Language::Custom(_) | Language::Eslint => {
                return None;
            }
        })
    }
}

/// A language server the user set up in the settings, for the files with its extensions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomServer {
    /// The language's name, as the status bar shows it.
    pub name: String,
    /// The file extensions it serves, without the dot.
    pub extensions: Vec<String>,
    /// The command line that starts it on standard input and output: the program, by name on the login shell's
    /// PATH or by path, then its arguments.
    pub command: String,
}

impl CustomServer {
    /// Whether it serves files with `extension`.
    pub fn serves(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|e| e.trim().trim_start_matches('.').eq_ignore_ascii_case(extension))
    }

    /// The protocol's language id for its documents: the name in lower case, without spaces.
    pub fn language_id(&self) -> String {
        self.name
            .chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect()
    }

    /// The program and its arguments; double quotes keep spaces inside a word.
    pub fn words(&self) -> Vec<String> {
        let mut words = vec![];
        let mut word = String::new();
        let (mut quoted, mut started) = (false, false);
        for c in self.command.chars() {
            match c {
                '"' => (quoted, started) = (!quoted, true),
                c if c.is_whitespace() && !quoted => {
                    if started {
                        words.push(std::mem::take(&mut word));
                    }
                    started = false;
                }
                c => {
                    word.push(c);
                    started = true;
                }
            }
        }
        if started {
            words.push(word);
        }
        words
    }
}

/// The project root of `file` for `language`, looked for between the file and `trusted`, the trusted folder it is
/// in; `trusted` itself when nothing marks a root. The search never leaves `trusted`.
///
/// Go: the nearest `go.work`, else the nearest `go.mod`. Rust: the topmost `Cargo.toml` with a `[workspace]`, else
/// the nearest `Cargo.toml`. TypeScript: the nearest `tsconfig.json` or `jsconfig.json`, else `package.json`. The
/// common languages: the nearest of their marker files. The user's own servers: the trusted folder.
pub fn root(language: &Language, file: &Path, trusted: &Path) -> PathBuf {
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
        Language::Custom(_) => None,
        other => {
            let markers = other.common().map_or(&[][..], |c| c.markers);
            dirs.iter().find(|d| markers.iter().any(|m| has(d, m)))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[&str]) -> ide_core::testing::Scratch {
        let dir = ide_core::testing::scratch("lsp-root");
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
        assert_eq!(Language::of_path(Path::new("a.py")), Some((Language::Python, "python")));
        assert_eq!(Language::of_path(Path::new("a.HPP")), Some((Language::C, "cpp")));
        assert_eq!(
            Language::of_path(Path::new("a.mm")),
            Some((Language::C, "objective-cpp"))
        );
        assert_eq!(Language::of_path(Path::new("x.yml")), Some((Language::Yaml, "yaml")));
        assert_eq!(
            Language::of_path(Path::new("README.md")),
            Some((Language::Markdown, "markdown"))
        );
        assert_eq!(Language::of_path(Path::new("Makefile")), None);
        assert_eq!(Language::of_path(Path::new("x.zsh")), None);
    }

    #[test]
    fn every_built_in_language_has_a_way_to_its_server() {
        for language in BUILT_IN {
            let common = language.common();
            let own_lookup = matches!(language, Language::Go | Language::Rust | Language::TypeScript);
            assert_eq!(common.is_none(), own_lookup, "{language:?}");
            if let Some(common) = common {
                assert!(!common.programs.is_empty(), "{language:?}");
                assert!(
                    common.npm.is_some() || common.hint.is_some(),
                    "{language:?} tells how to get its server"
                );
            }
        }
    }

    #[test]
    fn the_users_server_wins_over_the_built_in_one_for_its_extensions() {
        let pylsp = Arc::new(CustomServer {
            name: "My Python".into(),
            extensions: vec![".py".into(), "PYW".into()],
            command: "pylsp".into(),
        });
        let custom = [pylsp.clone()];
        assert_eq!(
            Language::detect(Path::new("a.py"), &custom),
            Some((Language::Custom(pylsp.clone()), "mypython".into()))
        );
        assert_eq!(
            Language::detect(Path::new("a.pyw"), &custom).map(|(l, _)| l),
            Some(Language::Custom(pylsp))
        );
        assert_eq!(
            Language::detect(Path::new("a.rs"), &custom),
            Some((Language::Rust, "rust".into()))
        );
        assert_eq!(Language::detect(Path::new("a.ex"), &custom), None);
    }

    #[test]
    fn a_command_line_splits_into_words_with_quotes_keeping_spaces() {
        let words = |command: &str| {
            CustomServer {
                name: "X".into(),
                extensions: vec![],
                command: command.into(),
            }
            .words()
        };
        assert_eq!(words("  jdtls  -data ~/x "), ["jdtls", "-data", "~/x"]);
        assert_eq!(
            words("\"/Applications/My Tools/srv\" --stdio \"\""),
            ["/Applications/My Tools/srv", "--stdio", ""]
        );
        assert!(words("   ").is_empty());
    }

    #[test]
    fn a_go_work_wins_over_the_nearer_go_mod() {
        let dir = tree(&["go.work", "svc/go.mod", "svc/cmd/main.go", "other/x.go"]);
        assert_eq!(root(&Language::Go, &dir.join("svc/cmd/main.go"), &dir), dir);
        let no_work = tree(&["svc/go.mod", "svc/cmd/main.go"]);
        assert_eq!(
            root(&Language::Go, &no_work.join("svc/cmd/main.go"), &no_work),
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
            root(&Language::Rust, &dir.join("ws/crates/a/src/lib.rs"), &dir),
            dir.join("ws")
        );
        assert_eq!(
            root(&Language::Rust, &dir.join("solo/src/main.rs"), &dir),
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
            root(&Language::TypeScript, &dir.join("app/src/main.ts"), &dir),
            dir.join("app")
        );
        assert_eq!(
            root(&Language::TypeScript, &dir.join("lib/x.js"), &dir),
            dir.join("lib")
        );
        let inner = dir.join("app/src");
        assert_eq!(
            root(&Language::TypeScript, &inner.join("main.ts"), &inner),
            inner,
            "never above the trusted folder"
        );
    }

    #[test]
    fn a_common_language_takes_its_nearest_marker_and_else_the_trusted_folder() {
        let dir = tree(&["svc/pyproject.toml", "svc/app/main.py", "loose/x.py", "notes/a.md"]);
        assert_eq!(
            root(&Language::Python, &dir.join("svc/app/main.py"), &dir),
            dir.join("svc")
        );
        assert_eq!(root(&Language::Python, &dir.join("loose/x.py"), &dir), dir);
        assert_eq!(root(&Language::Markdown, &dir.join("notes/a.md"), &dir), dir);
        let custom = Language::Custom(Arc::new(CustomServer {
            name: "X".into(),
            extensions: vec!["py".into()],
            command: "x".into(),
        }));
        assert_eq!(root(&custom, &dir.join("svc/app/main.py"), &dir), dir);
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
}

#[cfg(test)]
mod companion_tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> ide_core::testing::Scratch {
        let root = ide_core::testing::scratch("companions");
        for (path, text) in files {
            let file = root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
        root
    }

    #[test]
    fn eslint_joins_scripts_of_projects_that_use_it_and_emmet_markup_and_styles() {
        let root = project(&[
            ("web/package.json", r#"{"devDependencies":{"eslint":"^9"}}"#),
            ("web/a.tsx", ""),
            ("plain/b.ts", ""),
            ("page.html", ""),
        ]);
        assert_eq!(
            companions(&root.join("web/a.tsx"), &root, true),
            [Language::Eslint, Language::Emmet]
        );
        assert_eq!(companions(&root.join("web/a.tsx"), &root, false), [Language::Eslint]);
        assert_eq!(
            companions(&root.join("plain/b.ts"), &root, true),
            Vec::<Language>::new()
        );
        assert_eq!(companions(&root.join("page.html"), &root, true), [Language::Emmet]);
        assert!(Language::Eslint.is_companion() && !Language::Rust.is_companion());
    }

    #[test]
    fn the_flat_configuration_follows_the_file_and_else_the_installed_version() {
        assert!(flat_config(&project(&[("eslint.config.ts", "")])));
        assert!(!flat_config(&project(&[(".eslintrc.json", "{}")])));
        assert!(flat_config(&project(&[(
            "node_modules/eslint/package.json",
            r#"{"version":"9.12.0"}"#
        )])));
        assert!(!flat_config(&project(&[(
            "node_modules/eslint/package.json",
            r#"{"version":"8.57.0"}"#
        )])));
    }
}
