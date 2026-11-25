//! Rust tests read from the sources without building: the packages a bare `cargo test` or `cargo nextest run` in the
//! folder tests, by Cargo's rule for default members; each package's targets by Cargo's layout and the `[lib]`,
//! `[[bin]]` and `[[test]]` paths of the manifest; and from each target's root the module tree its `mod` items make,
//! `#[path]` included and modules behind a feature left out, with the functions under a test attribute. Inline modules
//! are told apart by indentation, which assumes rustfmt's layout; what this gets wrong, the first listing or run
//! corrects.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use crate::{Location, Test, TestId, Tool};

/// What a test program is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Lib,
    Bin(String),
    Test(String),
}

/// What the tests need of a `Cargo.toml`.
#[derive(Debug, Default)]
struct Manifest {
    package: Option<String>,
    lib: Option<String>,
    lib_path: Option<String>,
    /// `[[bin]]` and `[[test]]` targets that name their path.
    declared: Vec<(String, Target)>,
    members: Option<Vec<String>>,
    default_members: Option<Vec<String>>,
    exclude: Vec<String>,
}

/// What a line starts in, as far as strings and comments go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lexed {
    Code,
    Text,
    /// A raw string closed by a quote and this many `#`.
    Raw(usize),
    /// A block comment this deep.
    Comment(usize),
}

/// A `mod name;` item: the inline modules around it and its `#[path]`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Child {
    inline: Vec<String>,
    name: String,
    path: Option<String>,
}

/// What a Rust file holds: its tests as the inline modules around each ending with its name, by line from 1, and
/// the module files it declares.
#[derive(Debug, Default)]
struct Scan {
    tests: Vec<(Vec<String>, u32)>,
    children: Vec<Child>,
}

pub(crate) fn tests(tool: Tool, root: &Path) -> Vec<Test> {
    let mut out = Vec::new();
    for dir in packages(root) {
        let Some(manifest) = Manifest::read(&dir) else { continue };
        let Some(package) = manifest.package.clone() else {
            continue;
        };
        for (target, file) in targets(&dir, &package, &manifest) {
            let mut found = Vec::new();
            walk(&file, &[], true, &mut HashSet::new(), &mut found);
            let program = package_of(tool, &package, &manifest, &target);
            out.extend(found.into_iter().map(|(name, path, line)| Test {
                id: TestId {
                    package: program.clone(),
                    file: None,
                    name: name.join("::"),
                },
                location: Some(Location { path, line }),
            }));
        }
    }
    out
}

/// The test functions of a Rust file, each as the inline modules around it ending with its name, and its line from 1.
pub(crate) fn in_file(text: &str) -> Vec<(Vec<String>, u32)> {
    scan(text).tests
}

/// Gathers the tests of the module in `file` named `module`, and of the module files it declares; `owns_folder` where
/// its modules' files lie beside it, as for a crate root or a `mod.rs`.
fn walk(
    file: &Path,
    module: &[String],
    owns_folder: bool,
    seen: &mut HashSet<PathBuf>,
    out: &mut Vec<(Vec<String>, PathBuf, u32)>,
) {
    if !seen.insert(file.to_path_buf()) {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    let found = scan(&text);
    for (inner, line) in found.tests {
        out.push(([module, &inner].concat(), file.to_path_buf(), line));
    }
    let beside = file.parent().unwrap_or(Path::new("")).to_path_buf();
    let folder = if owns_folder {
        beside.clone()
    } else {
        beside.join(file.file_stem().unwrap_or_default())
    };
    for child in found.children {
        let base = child.inline.iter().fold(folder.clone(), |dir, m| dir.join(m));
        let (path, owns) = match &child.path {
            Some(path) if child.inline.is_empty() => (beside.join(path), true),
            Some(path) => (base.join(path), true),
            None if base.join(format!("{}.rs", child.name)).is_file() => {
                (base.join(format!("{}.rs", child.name)), false)
            }
            None => (base.join(&child.name).join("mod.rs"), true),
        };
        let mut name = [module, &child.inline].concat();
        name.push(child.name);
        walk(&path, &name, owns, seen, out);
    }
}

fn scan(text: &str) -> Scan {
    let mut out = Scan::default();
    let mut modules: Vec<(usize, String, bool)> = Vec::new();
    let mut attributes: Vec<String> = Vec::new();
    let mut lexed = Lexed::Code;
    for (n, line) in text.lines().enumerate() {
        let start = lexed;
        lexed = after(line, lexed);
        let trimmed = line.trim();
        if start != Lexed::Code || trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        while modules.last().is_some_and(|(at, _, _)| *at >= indent) {
            modules.pop();
        }
        if trimmed.starts_with("#[") {
            attributes.push(trimmed.to_string());
            continue;
        }
        let attributes = std::mem::take(&mut attributes);
        let left_out = modules.iter().any(|(_, _, out)| *out) || attributes.iter().any(|a| leaves_out(a));
        let inline = || modules.iter().map(|(_, name, _)| name.clone()).collect::<Vec<_>>();
        match module_item(trimmed) {
            Some((name, true)) => modules.push((indent, name, left_out)),
            Some((name, false)) if !left_out => out.children.push(Child {
                inline: inline(),
                name,
                path: attributes.iter().find_map(|a| path_attribute(a)),
            }),
            Some(_) => {}
            None if !left_out && attributes.iter().any(|a| attribute_path(a).ends_with("test")) => {
                if let Some(name) = crate::board::function_name(trimmed, "fn ") {
                    let mut path = inline();
                    path.push(name);
                    out.tests.push((path, u32::try_from(n + 1).unwrap_or(u32::MAX)));
                }
            }
            None => {}
        }
    }
    out
}

/// What the line `line` leaves open, given what it starts in.
fn after(line: &str, mut state: Lexed) -> Lexed {
    let bytes = line.as_bytes();
    let at = |i: usize| bytes.get(i).copied().unwrap_or(0);
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut i = 0;
    while i < bytes.len() {
        match state {
            Lexed::Text if at(i) == b'\\' => i += 1,
            Lexed::Text if at(i) == b'"' => state = Lexed::Code,
            Lexed::Raw(hashes) if at(i) == b'"' && (1..=hashes).all(|h| at(i + h) == b'#') => {
                state = Lexed::Code;
                i += hashes;
            }
            Lexed::Comment(depth) if at(i) == b'*' && at(i + 1) == b'/' => {
                state = if depth > 1 {
                    Lexed::Comment(depth - 1)
                } else {
                    Lexed::Code
                };
                i += 1;
            }
            Lexed::Comment(depth) if at(i) == b'/' && at(i + 1) == b'*' => {
                state = Lexed::Comment(depth + 1);
                i += 1;
            }
            Lexed::Code => match at(i) {
                b'/' if at(i + 1) == b'/' => return state,
                b'/' if at(i + 1) == b'*' => {
                    state = Lexed::Comment(1);
                    i += 1;
                }
                b'"' => state = Lexed::Text,
                b'r' if i == 0 || !ident(at(i - 1)) || (at(i - 1) == b'b' && (i == 1 || !ident(at(i - 2)))) => {
                    let quote = (i + 1..).find(|j| at(*j) != b'#').unwrap_or(i + 1);
                    if at(quote) == b'"' {
                        state = Lexed::Raw(quote - i - 1);
                        i = quote;
                    }
                }
                b'\'' if at(i + 1) == b'\\' => {
                    i = (i + 3..bytes.len()).find(|j| at(*j) == b'\'').unwrap_or(i);
                }
                b'\'' if at(i + 2) == b'\'' => i += 2,
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    state
}

/// The path of an attribute, such as `tokio::test` of `#[tokio::test(flavor = "multi_thread")]`.
fn attribute_path(attribute: &str) -> &str {
    attribute
        .trim_start_matches("#[")
        .split(['(', ']', '='])
        .next()
        .unwrap_or_default()
        .trim()
}

/// Whether the attribute leaves its item out of a test build without features: a `cfg` on `not(test)`, or on a
/// feature unless `any(test, …)` takes it in anyway.
fn leaves_out(attribute: &str) -> bool {
    let taken_in = attribute.contains("any(") && attribute.contains("test");
    attribute_path(attribute) == "cfg"
        && (attribute.contains("not(test)") || (attribute.contains("feature") && !taken_in))
}

fn path_attribute(attribute: &str) -> Option<String> {
    (attribute_path(attribute) == "path").then(|| quoted(attribute).into_iter().next())?
}

/// The name of the module that `line` declares, such as `pub(crate) mod tests {`, with whether it opens inline.
fn module_item(line: &str) -> Option<(String, bool)> {
    let mut rest = line;
    if let Some(after) = rest.strip_prefix("pub") {
        rest = after.trim_start();
        if rest.starts_with('(') {
            rest = rest.split_once(')')?.1.trim_start();
        }
    }
    let rest = rest.strip_prefix("mod ")?.trim_start();
    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    match rest[name.len()..].trim() {
        _ if name.is_empty() => None,
        "{" => Some((name, true)),
        ";" => Some((name, false)),
        _ => None,
    }
}

/// The packages a bare test run in `root` tests: the default members, else the root package, else every member.
fn packages(root: &Path) -> Vec<PathBuf> {
    let Some(manifest) = Manifest::read(root) else {
        return Vec::new();
    };
    let patterns = match (&manifest.default_members, &manifest.members) {
        (Some(chosen), _) => chosen.clone(),
        (None, _) if manifest.package.is_some() => vec![".".to_string()],
        (None, Some(all)) => all.clone(),
        (None, None) => Vec::new(),
    };
    let excluded: Vec<PathBuf> = manifest.exclude.iter().map(|e| root.join(e)).collect();
    let mut out: Vec<PathBuf> = Vec::new();
    for pattern in patterns {
        for dir in expand(root, &pattern) {
            if dir.join("Cargo.toml").is_file() && !excluded.iter().any(|e| dir.starts_with(e)) && !out.contains(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

/// The folders a member pattern such as `crates/*` names.
fn expand(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut found = vec![root.to_path_buf()];
    for part in Path::new(pattern).components() {
        let Component::Normal(part) = part else { continue };
        let part = part.to_string_lossy();
        if !part.contains('*') {
            for dir in &mut found {
                dir.push(part.as_ref());
            }
            continue;
        }
        let mut next = Vec::new();
        for dir in &found {
            let mut matching: Vec<PathBuf> = listed(dir)
                .into_iter()
                .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| matches(&part, &n.to_string_lossy())))
                .collect();
            matching.sort();
            next.extend(matching);
        }
        found = next;
    }
    found
}

/// Whether `name` matches `pattern`, where `*` stands for any run of characters.
fn matches(pattern: &str, name: &str) -> bool {
    let mut pieces = pattern.split('*');
    let first = pieces.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = pieces.collect();
    for (i, piece) in pieces.iter().enumerate() {
        if i + 1 == pieces.len() {
            return rest.ends_with(piece);
        }
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    rest.is_empty()
}

fn listed(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// The targets of the package in `dir` with their root files: those the manifest names by path, and those Cargo finds
/// by its layout.
fn targets(dir: &Path, package: &str, manifest: &Manifest) -> Vec<(Target, PathBuf)> {
    let mut out: Vec<(Target, PathBuf)> = manifest
        .declared
        .iter()
        .map(|(path, target)| (target.clone(), dir.join(path)))
        .collect();
    let mut add = |target: Target, file: PathBuf| {
        if file.is_file() && !out.iter().any(|(_, known)| *known == file) {
            out.push((target, file));
        }
    };
    add(
        Target::Lib,
        dir.join(manifest.lib_path.as_deref().unwrap_or("src/lib.rs")),
    );
    add(Target::Bin(package.to_string()), dir.join("src/main.rs"));
    for (folder, make) in [
        ("src/bin", Target::Bin as fn(String) -> Target),
        ("tests", Target::Test as fn(String) -> Target),
    ] {
        for path in listed(&dir.join(folder)) {
            let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            if path.extension().is_some_and(|e| e == "rs") {
                add(make(name), path);
            } else if path.is_dir() {
                add(make(name), path.join("main.rs"));
            }
        }
    }
    out
}

/// What `tool` calls the test program of `target`: Cargo names it by its file, always with `_`, nextest by its
/// binary id.
fn package_of(tool: Tool, package: &str, manifest: &Manifest, target: &Target) -> String {
    match (tool, target) {
        (Tool::Nextest, Target::Lib) => package.to_string(),
        (Tool::Nextest, Target::Bin(name)) => crate::nextest::binary_id(package, "bin", name),
        (Tool::Nextest, Target::Test(name)) => crate::nextest::binary_id(package, "test", name),
        (_, Target::Lib) => manifest.lib.as_deref().unwrap_or(package).replace('-', "_"),
        (_, Target::Bin(name) | Target::Test(name)) => name.replace('-', "_"),
    }
}

impl Manifest {
    fn read(dir: &Path) -> Option<Manifest> {
        let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
        Some(Manifest::parse(&text))
    }

    fn parse(text: &str) -> Manifest {
        let mut manifest = Manifest::default();
        let mut section = String::new();
        let mut declared: Option<(bool, Option<String>, Option<String>)> = None;
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            let line = line.split(" #").next().unwrap_or(line).trim();
            if line.starts_with('[') {
                manifest.declare(declared.take());
                section = line.trim_matches(['[', ']']).trim().to_string();
                if section == "bin" || section == "test" {
                    declared = Some((section == "bin", None, None));
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let mut value = value.trim().to_string();
            if value.starts_with('[') {
                while !value.contains(']') {
                    let Some(more) = lines.next() else { break };
                    value.push_str(more.split(" #").next().unwrap_or(more));
                }
            }
            let strings = quoted(&value);
            let first = strings.first().cloned();
            match (section.as_str(), key.trim()) {
                ("package", "name") => manifest.package = first,
                ("lib", "name") => manifest.lib = first,
                ("lib", "path") => manifest.lib_path = first,
                ("bin" | "test", "name") => {
                    if let Some(d) = declared.as_mut() {
                        d.1 = first;
                    }
                }
                ("bin" | "test", "path") => {
                    if let Some(d) = declared.as_mut() {
                        d.2 = first;
                    }
                }
                ("workspace", "members") => manifest.members = Some(strings),
                ("workspace", "default-members") => manifest.default_members = Some(strings),
                ("workspace", "exclude") => manifest.exclude = strings,
                _ => {}
            }
        }
        manifest.declare(declared);
        manifest
    }

    fn declare(&mut self, declared: Option<(bool, Option<String>, Option<String>)>) {
        if let Some((bin, Some(name), Some(path))) = declared {
            let path = path.trim_start_matches("./").to_string();
            self.declared
                .push((path, if bin { Target::Bin(name) } else { Target::Test(name) }));
        }
    }
}

/// The double-quoted strings in a TOML value or an attribute.
fn quoted(value: &str) -> Vec<String> {
    value.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

#[cfg(test)]
mod tests;
