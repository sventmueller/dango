//! Searching contents with the project search and finding files by glob, both honouring the ignore files.

use std::path::PathBuf;
use std::time::SystemTime;

use ide_project::{SearchOptions, search};
use serde_json::Value;

use ide_thread::ToolOutput;

use super::{Outcome, number, required, text};
use crate::notes::ToolNote;
use crate::workspace::Workspace;

/// Matches reported when the call names no limit.
const DEFAULT_MATCHES: u64 = 100;
/// The most matches one call reports.
const MAX_MATCHES: u64 = 500;
/// The most files one glob lists.
const MAX_FILES: usize = 500;

pub(super) fn grep(input: &Value, workspace: &Workspace) -> Result<Outcome, ToolNote> {
    let pattern = required(input, "pattern")?;
    let place = text(input, "path").map_or_else(|| workspace.root().to_path_buf(), |p| workspace.resolve(p));
    let (root, file) = if place.is_file() {
        let name = place.file_name().map(|n| n.to_string_lossy().into_owned());
        (place.parent().map(PathBuf::from).unwrap_or(place.clone()), name)
    } else {
        (place.clone(), None)
    };
    let mut options = SearchOptions::new(pattern);
    options.regex = !input.get("literal").and_then(Value::as_bool).unwrap_or(false);
    options.case_sensitive = input.get("case_sensitive").and_then(Value::as_bool).unwrap_or(true);
    options.max_results =
        usize::try_from(number(input, "max_results").unwrap_or(DEFAULT_MATCHES).min(MAX_MATCHES)).unwrap_or(100);
    options.include = file
        .into_iter()
        .chain(text(input, "glob").map(str::to_string))
        .collect();
    let mut lines = Vec::new();
    let summary = search(&root, &options, &ide_project::Cancel::new(), |found| {
        let shown = workspace.display(&found.path);
        for line in found.lines {
            lines.push((shown.clone(), line.line, line.preview.text));
        }
    })
    .map_err(|e| ToolNote::Pattern(e.to_string()))?;
    lines.sort();
    let results = u64::try_from(lines.len()).unwrap_or(u64::MAX);
    if lines.is_empty() {
        return Ok(Outcome {
            results: Some(0),
            ..Outcome::quiet(super::NO_MATCHES)
        });
    }
    let found: Vec<String> = lines
        .into_iter()
        .map(|(path, line, preview)| format!("{path}:{line}: {preview}"))
        .collect();
    let mut content = found.join("\n");
    if summary.limit_reached {
        content.push_str(&format!(
            "\n(stopped at {} matches; narrow the search for more)",
            options.max_results
        ));
    }
    Ok(Outcome {
        content,
        output: Some(ToolOutput::Text(found.join("\n"))),
        results: Some(results),
        ..Outcome::default()
    })
}

pub(super) fn glob(input: &Value, workspace: &Workspace) -> Result<Outcome, ToolNote> {
    let pattern = required(input, "pattern")?;
    let base = text(input, "path").map_or_else(|| workspace.root().to_path_buf(), |p| workspace.resolve(p));
    let matcher = globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|e| ToolNote::Pattern(e.to_string()))?
        .compile_matcher();
    let mut found: Vec<(SystemTime, PathBuf)> = ignore::WalkBuilder::new(&base)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .filter(|entry| {
            entry
                .path()
                .strip_prefix(&base)
                .is_ok_and(|relative| matcher.is_match(relative))
        })
        .map(|entry| {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (modified, entry.into_path())
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let total = found.len();
    if total == 0 {
        return Ok(Outcome {
            results: Some(0),
            ..Outcome::quiet(super::NO_FILES)
        });
    }
    let listed: Vec<String> = found
        .iter()
        .take(MAX_FILES)
        .map(|(_, path)| workspace.display(path))
        .collect();
    let mut content = listed.join("\n");
    if total > MAX_FILES {
        content.push_str(&format!("\n({} more files; narrow the pattern)", total - MAX_FILES));
    }
    Ok(Outcome {
        content,
        output: Some(ToolOutput::Text(listed.join("\n"))),
        results: u64::try_from(total).ok(),
        ..Outcome::default()
    })
}
