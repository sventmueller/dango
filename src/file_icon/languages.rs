//! Which of VS Code's languages a file is, by the extensions, file names and name patterns VS Code's built-in language
//! extensions declare; for the languages the Seti theme gives an icon by language rather than by extension.

/// A language: its identifier, its extensions without the dot, its file names, and patterns of names that also make
/// it, each a beginning and an ending; all lower case.
struct Language {
    id: &'static str,
    extensions: &'static [&'static str],
    names: &'static [&'static str],
    patterns: &'static [(&'static str, &'static str)],
}

const fn language(id: &'static str, extensions: &'static [&'static str]) -> Language {
    Language {
        id,
        extensions,
        names: &[],
        patterns: &[],
    }
}

const LANGUAGES: &[Language] = &[
    language("bat", &["bat", "cmd"]),
    language("c", &["c", "i"]),
    language("clojure", &["clj", "cljs", "cljc", "cljx", "clojure", "edn"]),
    language("coffeescript", &["coffee", "cson", "iced"]),
    language(
        "cpp",
        &[
            "cpp", "cppm", "cc", "ccm", "cxx", "cxxm", "c++", "c++m", "hpp", "hh", "hxx", "h++", "h", "ii", "ino",
            "inl", "ipp", "ixx", "tpp", "txx",
        ],
    ),
    language("csharp", &["cs", "csx", "cake"]),
    language("css", &["css"]),
    language("cuda-cpp", &["cu", "cuh"]),
    language("dart", &["dart"]),
    Language {
        id: "dockercompose",
        extensions: &[],
        names: &[
            "compose.yaml",
            "compose.yml",
            "docker-compose.yaml",
            "docker-compose.yml",
        ],
        patterns: &[
            ("compose.", ".yml"),
            ("compose.", ".yaml"),
            ("docker-compose.", ".yml"),
            ("docker-compose.", ".yaml"),
        ],
    },
    Language {
        id: "dockerfile",
        extensions: &["dockerfile", "containerfile"],
        names: &["dockerfile", "containerfile"],
        patterns: &[("dockerfile.", ""), ("containerfile.", "")],
    },
    Language {
        id: "dotenv",
        extensions: &["env"],
        names: &[".env"],
        patterns: &[(".env.", "")],
    },
    language("elixir", &["ex", "exs"]),
    language("elm", &["elm"]),
    language("erb", &["erb", "rhtml"]),
    language("fsharp", &["fs", "fsi", "fsx", "fsscript"]),
    Language {
        id: "git-commit",
        extensions: &[],
        names: &["commit_editmsg", "merge_msg"],
        patterns: &[],
    },
    language("go", &["go"]),
    language("groovy", &["groovy", "gvy", "gradle", "jenkinsfile", "nf"]),
    language("haml", &["haml"]),
    language("handlebars", &["handlebars", "hbs", "hjs"]),
    language("haskell", &["hs", "lhs"]),
    language(
        "html",
        &[
            "html", "htm", "shtml", "xhtml", "xht", "mdoc", "jsp", "asp", "aspx", "jshtm", "volt", "ejs",
        ],
    ),
    Language {
        id: "ignore",
        extensions: &[
            "gitignore_global",
            "gitignore",
            "git-blame-ignore-revs",
            "npmignore",
            "dockerignore",
        ],
        names: &[],
        patterns: &[],
    },
    language("jade", &["jade", "pug"]),
    language("java", &["java", "jav"]),
    language("javascript", &["js", "es6", "mjs", "cjs", "pac"]),
    language("javascriptreact", &["jsx"]),
    language(
        "json",
        &[
            "json",
            "bowerrc",
            "jscsrc",
            "webmanifest",
            "js.map",
            "css.map",
            "ts.map",
            "har",
            "jslintrc",
            "jsonld",
            "geojson",
            "ipynb",
            "vuerc",
        ],
    ),
    Language {
        id: "jsonc",
        extensions: &[
            "jsonc",
            "eslintrc",
            "eslintrc.json",
            "jsfmtrc",
            "jshintrc",
            "swcrc",
            "hintrc",
            "babelrc",
        ],
        names: &[
            "tsconfig.json",
            "jsconfig.json",
            "settings.json",
            "launch.json",
            "tasks.json",
            "keybindings.json",
            "extensions.json",
            ".devcontainer.json",
        ],
        patterns: &[("tsconfig.", ".json"), ("jsconfig.", ".json")],
    },
    language("jsonl", &["jsonl", "ndjson"]),
    language("julia", &["jl"]),
    language("latex", &["latex", "ltx", "ctx"]),
    language("less", &["less"]),
    language("lua", &["lua"]),
    Language {
        id: "makefile",
        extensions: &["mak", "mk"],
        names: &["makefile", "gnumakefile", "ocamlmakefile"],
        patterns: &[],
    },
    language(
        "markdown",
        &[
            "md", "mkd", "mdwn", "mdown", "markdown", "markdn", "mdtxt", "mdtext", "workbook",
        ],
    ),
    language("objective-c", &["m"]),
    language("objective-cpp", &["mm"]),
    language("perl", &["pl", "pm", "pod", "t", "psgi"]),
    language("php", &["php", "php4", "php5", "phtml", "ctp"]),
    language("powershell", &["ps1", "psm1", "psd1", "pssc", "psrc"]),
    Language {
        id: "properties",
        extensions: &[
            "properties",
            "cfg",
            "conf",
            "directory",
            "gitattributes",
            "gitconfig",
            "gitmodules",
            "editorconfig",
            "repo",
            "ini",
        ],
        names: &["gitconfig", ".npmrc"],
        patterns: &[],
    },
    language(
        "python",
        &["py", "rpy", "pyw", "cpy", "gyp", "gypi", "pyi", "ipy", "pyt"],
    ),
    language("r", &["r", "rhistory", "rprofile", "rt"]),
    language("razor", &["cshtml", "razor"]),
    Language {
        id: "ruby",
        extensions: &["rb", "rbx", "rjs", "gemspec", "rake", "ru", "podspec", "rbi"],
        names: &[
            "rakefile",
            "gemfile",
            "guardfile",
            "podfile",
            "capfile",
            "fastfile",
            "appfile",
            "brewfile",
        ],
        patterns: &[],
    },
    language("rust", &["rs"]),
    language("sass", &["sass"]),
    language("scss", &["scss"]),
    language(
        "shellscript",
        &[
            "sh",
            "bash",
            "bashrc",
            "bash_aliases",
            "bash_profile",
            "bash_login",
            "ebuild",
            "profile",
            "bash_logout",
            "xprofile",
            "zsh",
            "zshrc",
            "zprofile",
            "zlogin",
            "zlogout",
            "zshenv",
            "zsh-theme",
            "ksh",
            "csh",
            "cshrc",
            "tcshrc",
            "yashrc",
            "install",
        ],
    ),
    language("sql", &["sql", "dsql"]),
    language("swift", &["swift"]),
    language("tex", &["tex", "sty", "cls", "dtx", "ins"]),
    language("typescript", &["ts", "cts", "mts"]),
    language("typescriptreact", &["tsx"]),
    language("vue", &["vue"]),
    language(
        "xml",
        &[
            "xml",
            "xsd",
            "ascx",
            "atom",
            "axml",
            "axaml",
            "bpmn",
            "csl",
            "csproj",
            "dtd",
            "fsproj",
            "iml",
            "nuspec",
            "opml",
            "plist",
            "props",
            "rss",
            "storyboard",
            "svg",
            "targets",
            "vbproj",
            "vcxproj",
            "xaml",
            "xib",
            "xliff",
            "xlf",
            "xul",
        ],
    ),
    language("yaml", &["yml", "eyaml", "eyml", "yaml", "cff"]),
];

/// VS Code's language identifier for a lower-case file name: by the whole name, by a pattern, by its extensions from
/// the longest.
pub fn language_of(lower: &str) -> Option<&'static str> {
    if let Some(language) = LANGUAGES.iter().find(|l| l.names.contains(&lower)) {
        return Some(language.id);
    }
    if let Some(language) = LANGUAGES.iter().find(|l| {
        l.patterns.iter().any(|(start, end)| {
            lower.len() > start.len() + end.len() && lower.starts_with(start) && lower.ends_with(end)
        })
    }) {
        return Some(language.id);
    }
    lower
        .match_indices('.')
        .map(|(at, _)| &lower[at + 1..])
        .find_map(|extension| LANGUAGES.iter().find(|l| l.extensions.contains(&extension)))
        .map(|language| language.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_known_by_name_beginning_or_extension() {
        let cases = [
            ("main.rs", Some("rust")),
            ("app.tsx", Some("typescriptreact")),
            ("tsconfig.base.json", Some("jsonc")),
            ("makefile", Some("makefile")),
            ("dockerfile.dev", Some("dockerfile")),
            ("docker-compose.override.yml", Some("dockercompose")),
            ("compose.ts", Some("typescript")),
            (".env.local", Some("dotenv")),
            ("lexer.h", Some("cpp")),
            ("gemfile", Some("ruby")),
            ("notes.markdown", Some("markdown")),
            ("version", None),
        ];
        for (name, language) in cases {
            assert_eq!(language_of(name), language, "{name}");
        }
    }

    #[test]
    fn every_language_is_one_seti_or_vs_code_knows_and_named_in_lower_case() {
        for language in LANGUAGES {
            let patterns = language.patterns.iter().flat_map(|(start, end)| [start, end]);
            for word in language.extensions.iter().chain(language.names).chain(patterns) {
                assert_eq!(*word, word.to_lowercase(), "{}", language.id);
            }
        }
        let theme: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/seti/vs-seti-icon-theme.json")).unwrap();
        let known = theme["languageIds"].as_object().unwrap();
        let iconless: [&str; 0] = [];
        for language in LANGUAGES {
            assert!(
                known.contains_key(language.id) || iconless.contains(&language.id),
                "{} has no icon in the theme",
                language.id
            );
        }
    }
}
