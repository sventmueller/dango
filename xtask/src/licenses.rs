//! The license texts of everything the distributed app contains, which their licenses ask to go along with it: the
//! app's own once it has one, those of the crates in its program, the sibling repositories' among them, of code
//! inside crates under licenses of its own, such as whisper.cpp, and Sparkle's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use ide_product::NAME;

use crate::*;

/// The file in Contents/Resources.
pub(crate) const ACKNOWLEDGEMENTS: &str = "Acknowledgements.txt";

/// Of the licenses a crate offers as alternatives, the first one of these is taken.
const PREFERRED: [&str; 8] = [
    "MIT",
    "Zlib",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "0BSD",
    "Unlicense",
    "Apache-2.0",
];

/// The MIT terms, for a crate that names MIT and ships no text of it.
const MIT_TERMS: &str = "Permission is hereby granted, free of charge, to any person obtaining a copy of this software and \
associated documentation files (the \"Software\"), to deal in the Software without restriction, including without \
limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, \
and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the \
Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE \
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR \
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR \
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.";

/// A phrase only the full text of a license holds. BSD stands for both of its kinds.
const MARKS: [(&str, &str); 11] = [
    (
        "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION",
        "Apache-2.0",
    ),
    ("Boost Software License", "BSL-1.0"),
    ("CC0 1.0 Universal", "CC0-1.0"),
    (
        "Community Data License Agreement - Permissive - Version 2.0",
        "CDLA-Permissive-2.0",
    ),
    ("UNICODE LICENSE V3", "Unicode-3.0"),
    ("UNICODE, INC. LICENSE AGREEMENT", "Unicode-DFS-2016"),
    ("Permission is hereby granted, free of charge", "MIT"),
    ("Redistribution and use in source and binary forms", "BSD"),
    ("provided 'as-is'", "Zlib"),
    (
        "free and unencumbered software released into the public domain",
        "Unlicense",
    ),
    (
        "Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee",
        "0BSD",
    ),
];

/// Code inside a crate under a license of its own, which the crate's license files leave out: the crate, the license
/// file within it and what the code is.
const EMBEDDED: [(&str, &str, &str); 2] = [
    ("whisper-rs-sys", "whisper.cpp/LICENSE", "whisper.cpp"),
    (
        "lowspeak",
        "src/transcription/parakeet/LICENSE-FluidAudio",
        "FluidAudio code translated to Rust",
    ),
];

/// What the app carries from other projects, under their licenses: the license file in the repository and what it
/// covers; `assets/seti/README.md` names the versions.
const VENDORED: [(&str, &str); 2] = [
    ("assets/seti/LICENSE-seti-ui.md", "Seti UI file icons"),
    (
        "assets/seti/LICENSE-vscode.txt",
        "the Seti file icon theme of Visual Studio Code",
    ),
];

/// A crate in the program, from a line of `cargo tree` in the format `{p}|{l}|{r}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Crate {
    name: String,
    version: String,
    path: Option<PathBuf>,
    license: String,
    repository: String,
}

fn parse_crate(line: &str) -> Option<Crate> {
    let mut fields = line.split('|');
    let package = fields.next()?;
    let license = fields.next()?.trim().to_string();
    let repository = fields.next().unwrap_or("").trim().to_string();
    let (name, rest) = package.split_once(" v")?;
    let mut rest = rest.splitn(2, ' ');
    let version = rest
        .next()
        .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))?
        .to_string();
    let path = rest
        .next()
        .and_then(|p| p.strip_prefix('(')?.strip_suffix(')'))
        .filter(|p| p.starts_with('/'))
        .map(PathBuf::from);
    Some(Crate {
        name: name.to_string(),
        version,
        path,
        license,
        repository,
    })
}

/// The crates linked into the program with `features` for either processor, without the workspace's own.
fn crates(root: &Path, features: &[&str]) -> Result<Vec<Crate>, String> {
    let mut found = BTreeSet::new();
    for (target, _) in UNIVERSAL {
        let mut tree = Command::new(env!("CARGO"));
        tree.current_dir(root).arg("tree");
        if !features.is_empty() {
            tree.args(["--features", &features.join(",")]);
        }
        let tree = stdout(tree.args([
            "--locked",
            "--offline",
            "--package",
            PACKAGE,
            "--edges",
            "normal,no-proc-macro",
            "--target",
            target,
            "--prefix",
            "none",
            "--no-dedupe",
            "--format",
            "{p}|{l}|{r}",
        ]))?;
        for line in tree.lines().filter(|l| !l.trim().is_empty()) {
            found.insert(parse_crate(line).ok_or_else(|| format!("cargo tree: {line:?}"))?);
        }
    }
    Ok(found
        .into_iter()
        .filter(|c| !c.path.as_ref().is_some_and(|p| p.starts_with(root)))
        .collect())
}

/// Where cargo unpacked a crate from the registry, or the folder of a path dependency.
fn source(c: &Crate) -> Result<PathBuf, String> {
    if let Some(path) = &c.path {
        return Ok(path.clone());
    }
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")))
        .ok_or("neither CARGO_HOME nor HOME is set")?;
    let registries = home.join("registry/src");
    files::read_dir(&registries)?
        .flatten()
        .map(|registry| registry.path().join(format!("{}-{}", c.name, c.version)))
        .find(|dir| dir.is_dir())
        .ok_or_else(|| format!("{} {}: no source in {}", c.name, c.version, registries.display()))
}

/// The licenses a crate is used under: the preferred one of each set of alternatives in its SPDX expression, and every
/// license it requires together with another.
fn chosen(expression: &str) -> Result<Vec<String>, String> {
    let not_understood = || format!("license {expression:?} is not understood");
    let required: Vec<&str> = expression.split(" AND ").map(str::trim).collect();
    required
        .iter()
        .map(|part| {
            let alternatives = match part.strip_prefix('(').and_then(|p| p.strip_suffix(')')) {
                Some(inner) => inner,
                // SPDX binds AND before OR, so `A OR B AND C` is not a choice between all three.
                None if required.len() > 1 && part.contains(" OR ") => return Err(not_understood()),
                None => part,
            };
            let options: Vec<&str> = alternatives
                .split(" OR ")
                .flat_map(|o| o.split('/'))
                .map(str::trim)
                .collect();
            if let Some(preferred) = PREFERRED.iter().find(|p| options.contains(p)) {
                return Ok((*preferred).to_string());
            }
            let valid = |o: &&str| !o.is_empty() && o.chars().all(|c| c.is_ascii_alphanumeric() || "-.+".contains(c));
            if !options.iter().all(valid) {
                return Err(not_understood());
            }
            Ok(options[0].to_string())
        })
        .collect()
}

fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Which license a file holds the full text of. The phrase that comes first decides, since a file may go on with the
/// licenses of code its crate took in, and the Boost license repeats MIT's phrase after its name.
fn kind(text: &str) -> Option<&'static str> {
    // Some crates ship their license as a source comment.
    let uncommented: Vec<&str> = text
        .lines()
        .map(|line| line.trim_start().trim_start_matches(['/', '#', '*']))
        .collect();
    let text = words(&uncommented.join("\n"));
    let (at, license) = MARKS
        .iter()
        .filter_map(|(mark, license)| text.find(mark).map(|at| (at, *license)))
        .min_by_key(|(at, _)| *at)?;
    // ISC is 0BSD's wording with the condition to keep the notice.
    if license == "0BSD" && text[at..].contains("provided that the above copyright notice and this permission notice") {
        return Some("ISC");
    }
    if license != "BSD" {
        return Some(license);
    }
    let conditions = &text[at..];
    let conditions = conditions
        .find("THIS SOFTWARE IS PROVIDED")
        .map_or(conditions, |end| &conditions[..end]);
    Some(if conditions.contains("Neither the name") {
        "BSD-3-Clause"
    } else {
        "BSD-2-Clause"
    })
}

/// The names in the `authors` of a crate's manifest, without their addresses.
fn authors(manifest: &str) -> Vec<String> {
    let Some((_, rest)) = manifest.split_once("\nauthors = [") else {
        return Vec::new();
    };
    let list = rest.split(']').next().unwrap_or("");
    list.split('"')
        .skip(1)
        .step_by(2)
        .map(|author| author.split(" <").next().unwrap_or(author).trim().to_string())
        .filter(|author| !author.is_empty())
        .collect()
}

/// License texts with the kind each shows, and the texts of NOTICE files.
type Found = (Vec<(&'static str, String)>, Vec<String>);

/// The license and notice files in `dir`, each license with the kind its text shows.
fn license_files(dir: &Path) -> Result<Found, String> {
    let mut paths: Vec<PathBuf> = files::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    let (mut licenses, mut notices) = (Vec::new(), Vec::new());
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        if ["LICENSE", "LICENCE", "COPYING", "UNLICENSE"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            let text = files::read_to_string(&path)?;
            if let Some(license) = kind(&text) {
                licenses.push((license, text));
            }
        } else if name.starts_with("NOTICE") {
            notices.push(files::read_to_string(&path)?);
        }
    }
    Ok((licenses, notices))
}

/// The license files of a crate: its own, or for a crate of another workspace, such as Splat's, that one's.
fn crate_license_files(c: &Crate, dir: &Path) -> Result<Found, String> {
    let found = license_files(dir)?;
    if !found.0.is_empty() || c.path.is_none() {
        return Ok(found);
    }
    for parent in dir.ancestors().skip(1) {
        let manifest = parent.join("Cargo.toml");
        if manifest.exists() && files::read_to_string(&manifest)?.contains("[workspace]") {
            return license_files(parent);
        }
    }
    Ok(found)
}

/// The texts a crate is used under, from its license files `found`. A crate that names MIT and ships no text of it
/// gets the MIT terms with the holders its manifest names; one that names Apache 2.0 and ships no text gets
/// `apache`, the license's text as another crate of the program ships it, since it names no holder.
fn texts(c: &Crate, dir: &Path, found: &Found, apache: Option<&str>) -> Result<Vec<String>, String> {
    let (licenses, notices) = found;
    // Unicode-3.0 replaced Unicode-DFS-2016, and crates name the one while shipping the other.
    let same = |a: &str, b: &str| a == b || (a.starts_with("Unicode-") && b.starts_with("Unicode-"));
    let mut out: Vec<String> = Vec::new();
    for license in chosen(&c.license)? {
        let before = out.len();
        for (_, text) in licenses.iter().filter(|(l, _)| same(l, &license)) {
            if !out.iter().any(|t| words(t) == words(text)) {
                out.push(text.clone());
            }
        }
        if out.len() == before
            && license == "Apache-2.0"
            && let Some(apache) = apache
        {
            out.push(apache.to_string());
        }
        if out.len() == before && license == "Unlicense" {
            // The Unlicense asks for no text to go along, as Lowspeak's acknowledgements put it.
            out.push("Released into the public domain under the Unlicense (https://unlicense.org).".to_string());
        }
        if out.len() == before {
            if license != "MIT" {
                return Err(format!(
                    "{} {}: no {license} text in {}",
                    c.name,
                    c.version,
                    dir.display()
                ));
            }
            let named = authors(&files::read_to_string(dir.join("Cargo.toml"))?);
            let holders = if !named.is_empty() {
                named.join(", ")
            } else if !c.repository.is_empty() {
                format!("the authors of {}", c.repository)
            } else {
                format!("the authors of {}", c.name)
            };
            out.push(format!("Copyright (c) {holders}\n\n{MIT_TERMS}"));
        }
        // Apache 2.0 asks for a NOTICE file to go along; the other licenses do not.
        if license == "Apache-2.0" {
            out.extend(notices.iter().cloned());
        }
    }
    Ok(out)
}

/// A license text as written, without the blank lines and trailing spaces around and in it.
fn tidy(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.join("\n").trim_matches('\n').to_string()
}

/// The app's own license texts, from LICENSE files at the repository's root; none while it has none.
fn own_license(root: &Path) -> Result<Option<String>, String> {
    let (mut licenses, _) = license_files(root)?;
    if licenses.is_empty() {
        return Ok(None);
    }
    licenses.sort_by_key(|(kind, _)| *kind != "MIT");
    let texts: Vec<String> = licenses.iter().map(|(_, text)| tidy(text)).collect();
    Ok(Some(texts.join("\n\n")))
}

/// The text of `ACKNOWLEDGEMENTS` for the program built with `features`: the app's license first, then each license
/// text once, under the components it covers. Sparkle's license comes from `sparkle`, the unpacked release, when the
/// bundle carries the framework.
pub(crate) fn acknowledgements(features: &[&str], sparkle: Option<&Path>) -> Result<String, String> {
    let root = root();
    let mut found = Vec::new();
    for c in crates(&root, features)? {
        let dir = source(&c)?;
        let files = crate_license_files(&c, &dir)?;
        found.push((c, dir, files));
    }
    let apache = found
        .iter()
        .flat_map(|(_, _, (licenses, _))| licenses)
        .find(|(kind, _)| *kind == "Apache-2.0")
        .map(|(_, text)| text.clone());
    let mut covered: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (c, dir, files) in &found {
        let text = texts(c, dir, files, apache.as_deref())?
            .iter()
            .map(|t| tidy(t))
            .collect::<Vec<_>>()
            .join("\n\n");
        covered
            .entry(text)
            .or_default()
            .push(format!("{} {}", c.name, c.version));
    }
    for (c, dir, _) in &found {
        for (_, file, what) in EMBEDDED.iter().filter(|(name, ..)| *name == c.name) {
            let text = files::read_to_string(dir.join(file))?;
            covered
                .entry(tidy(&text))
                .or_default()
                .push(format!("{what} (in {} {})", c.name, c.version));
        }
    }
    for (file, what) in VENDORED {
        let text = files::read_to_string(root.join(file))?;
        covered.entry(tidy(&text)).or_default().push(what.to_string());
    }
    if let Some(sparkle) = sparkle {
        let text = files::read_to_string(sparkle.join("LICENSE"))?;
        covered
            .entry(tidy(&text))
            .or_default()
            .push(format!("Sparkle {}", updates::SPARKLE_VERSION));
    }
    for (library, text) in render_licenses(&root.join("../splat/resources/js"))? {
        covered
            .entry(tidy(&text))
            .or_default()
            .push(format!("{library} (in the renderer of formulas and diagrams)"));
    }
    let mut sections: Vec<(Vec<String>, String)> = covered
        .into_iter()
        .map(|(text, mut names)| {
            names.sort_by_key(|n| n.to_lowercase());
            (names, text)
        })
        .collect();
    sections.sort_by_key(|(names, _)| names[0].to_lowercase());
    let mut out = own_license(&root)?.map_or_else(String::new, |own| format!("## {NAME}\n\n{own}\n"));
    for (names, text) in sections {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("## {}\n\n{text}\n", names.join(", ")));
    }
    Ok(out)
}

/// The license texts of the libraries the bundle takes from splat's renderer page, `LICENSE-<library>` beside them,
/// each with its library's name.
fn render_licenses(js: &Path) -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    for entry in files::read_dir(js)? {
        let entry = entry.map_err(|e| format!("{}: {e}", js.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(library) = name.strip_prefix("LICENSE-")
            && !crate::bundle::FORMULA_FIELD.contains(&name.as_str())
        {
            found.push((library.to_string(), files::read_to_string(entry.path())?));
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_of_cargo_tree_names_the_crate_its_folder_and_its_license() {
        let registry = parse_crate("objc2 v0.6.5|MIT|https://github.com/madsmtm/objc2").unwrap();
        assert_eq!(
            (
                registry.name.as_str(),
                registry.version.as_str(),
                registry.path,
                registry.license.as_str()
            ),
            ("objc2", "0.6.5", None, "MIT")
        );
        let path = parse_crate("comrak v0.59.0 (/r/vendor/comrak)|BSD-2-Clause|").unwrap();
        assert_eq!(path.path, Some(PathBuf::from("/r/vendor/comrak")));
        assert!(parse_crate("no version|MIT|").is_none() && parse_crate("objc2").is_none());
    }

    #[test]
    fn of_alternatives_the_preferred_license_is_taken_and_of_requirements_all() {
        assert_eq!(chosen("Apache-2.0 OR MIT").unwrap(), ["MIT"]);
        assert_eq!(chosen("MIT/Apache-2.0").unwrap(), ["MIT"]);
        assert_eq!(chosen("Apache-2.0 OR BSL-1.0").unwrap(), ["BSL-1.0"]);
        assert_eq!(
            chosen("(MIT OR Apache-2.0) AND Unicode-3.0").unwrap(),
            ["MIT", "Unicode-3.0"]
        );
        assert!(chosen("MIT OR Apache-2.0 AND Unicode-3.0").is_err());
        assert!(chosen("Apache-2.0 WITH LLVM-exception").is_err());
        assert_eq!(
            chosen("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT").unwrap(),
            ["MIT"]
        );
        assert!(chosen("").is_err());
    }

    #[test]
    fn a_license_file_is_known_by_its_text_and_boost_is_not_taken_for_mit() {
        assert_eq!(
            kind("Permission is hereby granted,\nfree of charge, to any"),
            Some("MIT")
        );
        assert_eq!(
            kind("Boost Software License\n\nPermission is hereby granted, free of charge"),
            Some("BSL-1.0")
        );
        assert_eq!(
            kind(
                "Redistribution and use in source and binary forms\n3. Neither the name of\nTHIS SOFTWARE IS PROVIDED"
            ),
            Some("BSD-3-Clause")
        );
        assert_eq!(kind(MIT_TERMS), Some("MIT"));
        let isc = "Permission to use, copy, modify, and/or distribute this software for any\npurpose with or without fee \
                   is hereby granted, provided that the above\ncopyright notice and this permission notice appear in all copies.";
        assert_eq!(kind(isc), Some("ISC"));
        assert_eq!(
            kind(&isc.lines().map(|l| format!("// {l}\n")).collect::<String>()),
            Some("ISC")
        );
        assert_eq!(
            kind(
                "Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted."
            ),
            Some("0BSD")
        );
        assert_eq!(
            kind("Licensed under the Apache License, Version 2.0 or the MIT license"),
            None
        );
    }

    #[test]
    fn the_authors_of_a_manifest_lose_their_addresses() {
        let manifest =
            "[package]\nname = \"x\"\nauthors = [\n    \"Mads Marquart <mads@marquart.dk>\",\n    \"Mary\",\n]\n";
        assert_eq!(authors(manifest), ["Mads Marquart", "Mary"]);
        assert!(authors("[package]\nname = \"x\"\n").is_empty());
    }

    #[test]
    fn a_license_text_keeps_its_lines_and_loses_the_blank_ones_around_it() {
        assert_eq!(
            tidy("\n\r\n   Apache License  \r\n\n  1. Definitions.\t\n\n"),
            "   Apache License\n\n  1. Definitions."
        );
    }

    /// Reads `cargo tree` and the crates' sources, all on this Mac, as `make check` may.
    #[test]
    fn every_component_of_the_program_comes_with_its_license_text() {
        let text = acknowledgements(&Build::Distribution.features(), None).unwrap();
        assert!(
            text.starts_with(&format!("## {NAME}\n\nMIT License"))
                && words(&text).contains("Apache License Version 2.0"),
            "the app's own licenses come first, MIT before Apache as in Splat"
        );
        let lines = format!("\n{text}");
        let section = |name: &str| {
            lines
                .split("\n## ")
                .find(|s| {
                    s.lines()
                        .next()
                        .is_some_and(|names| names.split(", ").any(|n| n.starts_with(name)))
                })
                .map(words)
                .unwrap_or_else(|| panic!("no section names {name}"))
        };
        assert!(section("comrak ").contains("binary forms, with or without modification"));
        assert!(section("objc2-app-kit ").contains("Copyright (c) the authors of https://github.com/madsmtm/objc2"));
        assert!(section("splat-i18n ").contains("Permission is hereby granted"));
        assert!(section("Seti UI file icons").contains("Copyright (c) 2014 Jesse Weed"));
        assert!(section("the Seti file icon theme").contains("Copyright (c) 2015 - present Microsoft Corporation"));
        assert!(section("mermaid (in the renderer").contains("Permission is hereby granted"));
        assert!(section("mathjax (in the renderer").contains("Apache License"));
        assert!(!text.contains("mathlive"), "the formula field stays in splat");
        assert!(section("lowspeak ").contains("Permission is hereby granted"));
        assert!(section("whisper.cpp (in whisper-rs-sys ").contains("ggml authors"));
        assert!(section("FluidAudio code translated to Rust (in lowspeak ").contains("Apache License"));
        let names: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("## "))
            .flat_map(|l| l.split(", "))
            .collect();
        assert!(names.len() > 100, "{} components", names.len());
        assert!(
            !names.iter().any(|n| n.starts_with("ide-")),
            "the workspace's crates are the app"
        );
        assert!(
            !names.iter().any(|n| n.starts_with("syn ")),
            "a crate of a procedural macro is not in the program"
        );
    }
}
