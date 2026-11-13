//! Prettier as a formatter beside the language server (E133): whether a project uses it, where its program is, which
//! files it formats, and what an answer of `prettier --stdin-filepath` means.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Who formats a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formatter {
    /// The language server.
    Server,
    /// Prettier.
    Prettier,
}

/// The value of the setting `formatter` that picks Prettier, and the one that picks the language server.
pub const PRETTIER: &str = "prettier";
/// See [`PRETTIER`].
pub const SERVER: &str = "server";

const CONFIG_FILES: &[&str] = &[
    ".prettierrc",
    ".prettierrc.json",
    ".prettierrc.json5",
    ".prettierrc.yaml",
    ".prettierrc.yml",
    ".prettierrc.toml",
    ".prettierrc.js",
    ".prettierrc.cjs",
    ".prettierrc.mjs",
    ".prettierrc.ts",
    ".prettierrc.cts",
    ".prettierrc.mts",
    "prettier.config.js",
    "prettier.config.cjs",
    "prettier.config.mjs",
    "prettier.config.ts",
    "prettier.config.cts",
    "prettier.config.mts",
];

/// Whether the folder `dir` configures Prettier: a configuration file, or the key `prettier` in its `package.json`.
fn configures(dir: &Path) -> bool {
    if CONFIG_FILES.iter().any(|name| dir.join(name).is_file()) {
        return true;
    }
    std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|package| package.get("prettier").is_some())
}

/// Whether `file` of the project at `root` has a Prettier configuration in its folder or one above it up to `root`.
pub fn has_config(file: &Path, root: &Path) -> bool {
    file.ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(root))
        .any(configures)
}

/// Whether Prettier formats files like `file` by itself, without plugins.
pub fn knows(file: &Path) -> bool {
    let extension = file.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    matches!(
        extension.as_deref(),
        Some(
            "js" | "jsx"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "mts"
                | "cts"
                | "json"
                | "json5"
                | "jsonc"
                | "css"
                | "scss"
                | "less"
                | "html"
                | "htm"
                | "vue"
                | "md"
                | "markdown"
                | "mdx"
                | "yaml"
                | "yml"
                | "graphql"
                | "gql"
                | "hbs"
                | "handlebars"
        )
    )
}

/// Who formats `file` of the project at `root` in `language`: the setting `formatter` for the language decides,
/// else Prettier where the project configures it and knows the file, else the language server.
pub fn choose(file: &Path, root: &Path, language: &str, setting: &BTreeMap<String, String>) -> Formatter {
    match setting.get(language).map(String::as_str) {
        Some(PRETTIER) if knows(file) => Formatter::Prettier,
        Some(SERVER) => Formatter::Server,
        _ if knows(file) && has_config(file, root) => Formatter::Prettier,
        _ => Formatter::Server,
    }
}

/// The program to run for `file` of the project at `root` and the folder to run it in: the nearest
/// `node_modules/.bin/prettier` from the file's folder up to `root`, run in the folder of that `node_modules` so that
/// its `.prettierignore` counts, else `prettier` on `path` run in `root`.
pub fn program(file: &Path, root: &Path, path: Option<&str>) -> Option<(PathBuf, PathBuf)> {
    let local = file
        .ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(root))
        .find(|dir| dir.join("node_modules/.bin/prettier").is_file());
    if let Some(dir) = local {
        return Some((dir.join("node_modules/.bin/prettier"), dir.to_path_buf()));
    }
    path?
        .split(':')
        .map(|dir| Path::new(dir).join("prettier"))
        .find(|candidate| candidate.is_file())
        .map(|found| (found, root.to_path_buf()))
}

/// What a run of `prettier --stdin-filepath` ended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The formatted text; for an ignored file, the text as it was.
    Formatted(String),
    /// Prettier failed, with its message.
    Failed(String),
}

/// What exit `code`, `stdout` and `stderr` of a run mean: 0 is the formatted text, anything else, mostly 2, a failure.
pub fn outcome(code: Option<i32>, stdout: Vec<u8>, stderr: &[u8]) -> Outcome {
    match (code, String::from_utf8(stdout)) {
        (Some(0), Ok(text)) => Outcome::Formatted(text),
        (Some(0), Err(_)) => Outcome::Failed("Prettier answered with text that is not UTF-8".into()),
        _ => {
            let message = String::from_utf8_lossy(stderr);
            let first = message
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("Prettier failed");
            Outcome::Failed(first.trim().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!("ide-prettier-{}-{stamp}", std::process::id()));
        for (path, text) in files {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        root
    }

    #[test]
    fn a_configured_project_formats_what_prettier_knows_with_prettier() {
        let root = project(&[(".prettierrc", "{}"), ("src/a.ts", ""), ("src/b.rs", "")]);
        let none = BTreeMap::new();
        assert_eq!(
            choose(&root.join("src/a.ts"), &root, "typescript", &none),
            Formatter::Prettier
        );
        assert_eq!(choose(&root.join("src/b.rs"), &root, "rust", &none), Formatter::Server);
        let server = BTreeMap::from([("typescript".to_string(), SERVER.to_string())]);
        assert_eq!(
            choose(&root.join("src/a.ts"), &root, "typescript", &server),
            Formatter::Server
        );
    }

    #[test]
    fn without_a_configuration_the_server_formats_unless_the_setting_asks_for_prettier() {
        let root = project(&[("package.json", r#"{"name":"x"}"#), ("a.css", "")]);
        let none = BTreeMap::new();
        assert_eq!(choose(&root.join("a.css"), &root, "css", &none), Formatter::Server);
        let prettier = BTreeMap::from([("css".to_string(), PRETTIER.to_string())]);
        assert_eq!(
            choose(&root.join("a.css"), &root, "css", &prettier),
            Formatter::Prettier
        );
        let keyed = project(&[("package.json", r#"{"prettier":{"semi":false}}"#), ("web/a.css", "")]);
        assert!(has_config(&keyed.join("web/a.css"), &keyed));
    }

    #[test]
    fn the_nearest_local_program_runs_in_the_folder_of_its_node_modules() {
        let root = project(&[
            ("node_modules/.bin/prettier", ""),
            ("apps/web/node_modules/.bin/prettier", ""),
            ("apps/web/src/a.ts", ""),
            ("apps/api/a.ts", ""),
        ]);
        let (program_web, dir_web) = program(&root.join("apps/web/src/a.ts"), &root, None).unwrap();
        assert_eq!(dir_web, root.join("apps/web"));
        assert!(program_web.ends_with("apps/web/node_modules/.bin/prettier"));
        let (_, dir_api) = program(&root.join("apps/api/a.ts"), &root, None).unwrap();
        assert_eq!(dir_api, root);
        let bare = project(&[("a.ts", ""), ("bin/prettier", "")]);
        let path = bare.join("bin").to_string_lossy().into_owned();
        assert_eq!(
            program(&bare.join("a.ts"), &bare, Some(&path)),
            Some((bare.join("bin/prettier"), bare.clone()))
        );
        assert_eq!(program(&bare.join("a.ts"), &bare, Some("/nowhere")), None);
    }

    #[test]
    fn exit_code_two_is_a_failure_with_the_first_line_of_the_message() {
        assert_eq!(
            outcome(Some(0), b"a;\n".to_vec(), b""),
            Outcome::Formatted("a;\n".into())
        );
        assert_eq!(
            outcome(
                Some(2),
                Vec::new(),
                b"\n[error] a.ts: SyntaxError: ';' expected (1:3)\n[error] more"
            ),
            Outcome::Failed("[error] a.ts: SyntaxError: ';' expected (1:3)".into())
        );
        assert!(matches!(outcome(None, Vec::new(), b""), Outcome::Failed(_)));
    }
}
