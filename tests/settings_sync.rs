//! Every setting the code declares is in the table of what the sync takes along (E113, docs/phase-12.md), with the
//! same class, and every entry of the table is still declared.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The settings that follow the user to their other Macs.
const SHARED: &[&str] = &[
    "agentEffort",
    "agentModel",
    "agentPrices",
    "blameAtLineEnd",
    "ciHosts",
    "ciOAuthClients",
    "commitMessageModel",
    "diagnosticsAtLineEnd",
    "dictation",
    "dictationWithFn",
    "editPrediction",
    "editPredictionExclude",
    "editPredictionOffFor",
    "fontSize",
    "formatOnSave",
    "formatter",
    "hiddenSidebarPanels",
    "inlayHints",
    "inlayHintsOffFor",
    "markdownPreviewBeside",
    "minimap",
    "optionAsMeta",
    "previewTabs",
    "reopenProjects",
    "searchEngine",
    "showIgnoredFiles",
    "showWhitespace",
    "stickyScrollLines",
];

/// The settings that stay on each Mac: paths, local addresses, consents and reminders.
const PER_MAC: &[&str] = &[
    "agentAllowedFolders",
    "agentEndpoints",
    "agentRules",
    "commitMessageConsent",
    "dictationLowspeakOffered",
    "editPredictionConsent",
    "editPredictionLmStudio",
    "editPredictionOllama",
    "languageServers",
];

/// The entries of the state file, which are never synced.
const STATE: &[&str] = &[
    "agentChoices",
    "claudeRuleFolders",
    "dictationAccessibilityAsked",
    "openProjects",
    "openWindows",
    "recentProjects",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !path.ends_with("target") {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The settings the code declares, by name, with their class as the declaration says it.
fn declared() -> BTreeMap<String, &'static str> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("crates"), &mut files);
    let mut found = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ").replace("( ", "(");
        for (marker, class) in [
            ("Key::user(\"", None),
            ("Key::project(\"", None),
            ("StateKey::new(\"", Some("state")),
        ] {
            for piece in flat.split(marker).skip(1) {
                let Some((name, rest)) = piece.split_once('"') else {
                    continue;
                };
                if name.contains(' ') || name.is_empty() {
                    continue;
                }
                let class = class.unwrap_or_else(|| {
                    let sharing = rest
                        .trim_start_matches([',', ' '])
                        .split([',', ' '])
                        .next()
                        .unwrap_or_default();
                    if sharing.ends_with("Sharing::Shared") {
                        "shared"
                    } else if sharing.ends_with("Sharing::PerMac") {
                        "per-mac"
                    } else {
                        "unknown"
                    }
                });
                found.insert(name.to_string(), class);
            }
        }
    }
    found
}

#[test]
fn every_setting_is_in_the_sync_table_with_its_class() {
    let declared = declared();
    let mut table = BTreeMap::new();
    for (names, class) in [(SHARED, "shared"), (PER_MAC, "per-mac"), (STATE, "state")] {
        for name in names {
            table.insert((*name).to_string(), class);
        }
    }
    let missing: Vec<_> = declared.keys().filter(|name| !table.contains_key(*name)).collect();
    assert!(
        missing.is_empty(),
        "settings not in the sync table of E113: {missing:?}"
    );
    let gone: Vec<_> = table.keys().filter(|name| !declared.contains_key(*name)).collect();
    assert!(
        gone.is_empty(),
        "the sync table names settings the code no longer declares: {gone:?}"
    );
    let differing: Vec<_> = declared
        .iter()
        .filter(|(name, class)| table.get(*name) != Some(class))
        .collect();
    assert!(
        differing.is_empty(),
        "declared with another class than the table says: {differing:?}"
    );
}
