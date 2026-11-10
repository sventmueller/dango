//! Every interface text the code asks for has a translation, with the same placeholders.
//!
//! English must be complete at all times. The other 33 languages are checked with `IDE_I18N_RELEASE=1`, which the
//! release requires (docs/plan.md, E15); `IDE_I18N_LANGUAGES=fr,es` limits that check to the languages named.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const LANGUAGES: [&str; 35] = [
    "ar", "ca", "cs", "da", "de", "el", "en", "es", "fi", "fr", "he", "hi", "hr", "hu", "id", "it", "ja", "ko", "ms",
    "nl", "no", "pl", "pt_BR", "pt_PT", "ro", "ru", "sk", "sl", "sv", "th", "tr", "uk", "vi", "zh-Hans", "zh-Hant",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The string literal at the start of `s`, unescaped, if `s` starts with one.
fn literal(s: &str) -> Option<String> {
    let mut chars = s.strip_prefix('"')?.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                '\n' => {
                    let rest: String = chars.clone().collect();
                    let skipped = rest.len() - rest.trim_start().len();
                    for _ in 0..rest[..skipped].chars().count() {
                        chars.next();
                    }
                }
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// The keys of `t("…")`, `tf("…", …)` and `n("…")` calls in `source`.
fn keys_in(source: &str) -> Vec<String> {
    let mut keys = vec![];
    for call in ["t(", "tf(", "n("] {
        let mut from = 0;
        while let Some(found) = source[from..].find(call) {
            let at = from + found;
            from = at + call.len();
            let before = source[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            if let Some(key) = literal(source[at + call.len()..].trim_start()) {
                keys.push(key);
            }
        }
    }
    keys
}

fn used_keys() -> BTreeSet<String> {
    let mut files = vec![];
    rust_files(&root().join("src"), &mut files);
    rust_files(&root().join("crates"), &mut files);
    // The render path of the previews asks for its texts in the app's tables (docs/splat-render.md).
    rust_files(&root().join("../splat/crates/splat-render/src"), &mut files);
    files
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .flat_map(|source| keys_in(&source))
        .collect()
}

fn entries(language: &str) -> Option<Vec<(String, String)>> {
    let path = root()
        .join("resources")
        .join(format!("{language}.lproj/Localizable.strings"));
    let text = std::fs::read_to_string(path).ok()?;
    Some(splat_i18n::parse(&text).into_iter().collect())
}

fn table(language: &str) -> Option<BTreeMap<String, String>> {
    entries(language).map(|entries| entries.into_iter().collect())
}

fn placeholders(s: &str) -> usize {
    s.matches("{}").count()
}

fn problems_in(language: &str, used: &BTreeSet<String>) -> Vec<String> {
    let Some(table) = table(language) else {
        return vec![format!("{language}: no Localizable.strings")];
    };
    let mut problems = vec![];
    for key in used {
        match table.get(key) {
            None => problems.push(format!("{language}: missing {key:?}")),
            Some(text) if placeholders(text) != placeholders(key) => {
                problems.push(format!("{language}: {key:?} has other placeholders: {text:?}"));
            }
            Some(_) => {}
        }
    }
    for key in table.keys().filter(|k| !used.contains(*k)) {
        problems.push(format!("{language}: stale {key:?}"));
    }
    let mut seen = BTreeSet::new();
    for (key, _) in entries(language).unwrap_or_default() {
        if !seen.insert(key.clone()) {
            problems.push(format!("{language}: {key:?} is there twice"));
        }
    }
    problems
}

#[test]
fn the_scanner_finds_keys_in_every_kind_of_call() {
    let source = "t(\"Eins\"); tf(\n    \"Zwei {}\", &[x]); n(\"Drei \\\"x\\\"\"); format(\"nein\"); at(\"nein\")";
    assert_eq!(keys_in(source), vec!["Eins", "Zwei {}", "Drei \"x\""]);
}

#[test]
fn english_translates_every_text() {
    let used = used_keys();
    assert!(used.len() > 40, "only {} keys found", used.len());
    let problems = problems_in("en", &used);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_language_translates_every_text_for_a_release() {
    if std::env::var_os("IDE_I18N_RELEASE").is_none() {
        return;
    }
    let used = used_keys();
    let named = std::env::var("IDE_I18N_LANGUAGES").unwrap_or_default();
    let named: Vec<&str> = named.split(',').map(str::trim).filter(|l| !l.is_empty()).collect();
    if let Some(unknown) = named.iter().find(|l| !LANGUAGES.contains(l)) {
        panic!("{unknown} is not one of the app's languages");
    }
    let problems: Vec<String> = LANGUAGES
        .iter()
        .filter(|l| **l != "de" && (named.is_empty() || named.contains(l)))
        .flat_map(|language| problems_in(language, &used))
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
