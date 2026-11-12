//! Every crate under `crates/` has a README with a Mermaid diagram of its control flow (docs/phase-11.md, E100).

use std::path::PathBuf;

fn crates() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates");
    let mut crates: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("Cargo.toml").is_file())
        .collect();
    crates.sort();
    crates
}

#[test]
fn every_crate_has_a_readme_with_a_mermaid_diagram() {
    let crates = crates();
    assert!(!crates.is_empty(), "no crates found");
    let missing: Vec<String> = crates
        .iter()
        .filter_map(|dir| {
            let readme = dir.join("README.md");
            match std::fs::read_to_string(&readme) {
                Err(_) => Some(format!("{}: no README.md", dir.display())),
                Ok(text) if !text.lines().any(|line| line.trim() == "```mermaid") => {
                    Some(format!("{}: no ```mermaid block", readme.display()))
                }
                Ok(_) => None,
            }
        })
        .collect();
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
