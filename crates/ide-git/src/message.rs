//! What a model gets to write a commit message (E88): the staged diff, or every change when nothing is staged, the
//! branch and the last subjects, built once into one text. The secrets check reads exactly that text from standard
//! input, and only a text it passed can be had for sending; nothing changes it after.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::diff::{Change, DiffSource, FileDiff, LineKind};
use crate::error::Result;
use crate::repo::Repository;

/// How many lines of one file's diff the request takes; the rest is named, not sent.
pub(crate) const LINES_PER_FILE: usize = 200;
/// How many subjects of the history the request takes, for the style and language of the message.
pub const SUBJECTS: usize = 10;

/// What the model is asked, ahead of the changes.
const INSTRUCTIONS: &str = "Write a git commit message for the changes below. Use the language and the style of the \
recent commit subjects. Begin with a subject line of at most 72 characters; add a blank line and a short body only \
when the change needs explaining. Answer with the message alone, without quotes or code fences.";

/// Where a line of the request comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A line of a changed file, relative to the root, numbered as in the file after the change, or before it for a
    /// removed line.
    File {
        /// The file.
        path: PathBuf,
        /// Its line.
        line: u32,
    },
    /// A line of the request outside the files, such as a subject of the history.
    Request {
        /// The line of the request, from 1.
        line: u32,
    },
}

/// The request as built, before the secrets check.
#[derive(Debug)]
pub struct MessageRequest {
    text: String,
    /// The place of each line of `text`.
    places: Vec<Option<(PathBuf, u32)>>,
    files: usize,
    staged: bool,
}

/// A request the secrets check passed; what is sent is its text, which nothing can change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRequest {
    text: String,
    files: usize,
    staged: bool,
}

/// What the secrets check made of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestScan {
    /// gitleaks is not on the login shell's PATH, so nothing may be sent.
    NotInstalled,
    /// Nothing was found; the request may go out.
    Clean(CheckedRequest),
    /// Possible secrets; their values are not included.
    Found(Vec<RequestFinding>),
}

/// A possible secret in a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFinding {
    /// The gitleaks rule, such as `aws-access-token`.
    pub rule: String,
    /// What the rule looks for.
    pub description: String,
    /// Where it is.
    pub place: Place,
}

impl MessageRequest {
    /// The text the check reads.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How many changed files it holds.
    pub fn files(&self) -> usize {
        self.files
    }

    /// Whether it holds the staged changes rather than every change.
    pub fn staged(&self) -> bool {
        self.staged
    }

    /// Where line `line` of the text, counted from 1, comes from.
    fn place(&self, line: u32) -> Place {
        let index = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
        match self.places.get(index) {
            Some(Some((path, line))) => Place::File {
                path: path.clone(),
                line: *line,
            },
            _ => Place::Request { line },
        }
    }
}

impl CheckedRequest {
    /// The text to send, exactly as checked.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How many changed files it holds.
    pub fn files(&self) -> usize {
        self.files
    }
}

impl Repository {
    /// The request for a commit message: what is staged, or when nothing is, every change with untracked files, each
    /// file cut at [`LINES_PER_FILE`] lines; with the branch and the last [`SUBJECTS`] subjects.
    pub fn message_request(&self) -> Result<MessageRequest> {
        let staged = self.has_staged_changes()?;
        let diffs = if staged {
            self.diff(DiffSource::Staged, &[] as &[PathBuf])?
        } else {
            let mut diffs = self.diff(DiffSource::Unstaged, &[] as &[PathBuf])?;
            for path in self.untracked(&[])? {
                diffs.push(self.diff_untracked(&path)?);
            }
            diffs
        };
        let branch = self.current_branch()?;
        let subjects = self.subjects()?;
        let mut builder = Builder::default();
        builder.line(INSTRUCTIONS, None);
        builder.line("", None);
        builder.line(
            &format!("Branch: {}", branch.as_deref().unwrap_or("(detached HEAD)")),
            None,
        );
        if !subjects.is_empty() {
            builder.line("", None);
            builder.line("Recent commit subjects, newest first:", None);
            for subject in &subjects {
                builder.line(&format!("- {subject}"), None);
            }
        }
        builder.line("", None);
        builder.line(
            if staged {
                "Staged changes:"
            } else {
                "Changes (nothing is staged):"
            },
            None,
        );
        for diff in &diffs {
            builder.file(diff);
        }
        Ok(MessageRequest {
            text: builder.text,
            places: builder.places,
            files: diffs.len(),
            staged,
        })
    }

    /// Runs the secrets check over the text of `request` as gitleaks reads standard input; only a clean check gives
    /// the request back for sending.
    pub fn scan_request(&self, request: MessageRequest) -> Result<RequestScan> {
        let Some(findings) = self.gitleaks(&["stdin"], Some(request.text.as_bytes().to_vec()))? else {
            return Ok(RequestScan::NotInstalled);
        };
        if findings.is_empty() {
            return Ok(RequestScan::Clean(CheckedRequest {
                text: request.text,
                files: request.files,
                staged: request.staged,
            }));
        }
        Ok(RequestScan::Found(
            findings
                .into_iter()
                .map(|f| RequestFinding {
                    rule: f.rule,
                    description: f.description,
                    place: request.place(f.line),
                })
                .collect(),
        ))
    }

    /// The subjects of the last commits of HEAD, newest first.
    fn subjects(&self) -> Result<Vec<String>> {
        if self.head()?.is_none() {
            return Ok(Vec::new());
        }
        let count = format!("--max-count={SUBJECTS}");
        let output = self.git().ok(self.call([
            "-c",
            "log.showSignature=false",
            "log",
            "--no-color",
            &count,
            "--format=%s",
            "HEAD",
            "--",
        ]))?;
        Ok(output
            .text()
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect())
    }
}

/// The text of a request with the place of each of its lines.
#[derive(Default)]
struct Builder {
    text: String,
    places: Vec<Option<(PathBuf, u32)>>,
}

impl Builder {
    /// Adds `text` as one line; a line break inside it would shift the places, so it is written as `\n`.
    fn line(&mut self, text: &str, place: Option<(PathBuf, u32)>) {
        self.text.push_str(&text.replace('\n', "\\n"));
        self.text.push('\n');
        self.places.push(place);
    }

    fn file(&mut self, diff: &FileDiff) {
        let path = diff.path().to_path_buf();
        self.line("", None);
        self.line(&header(diff), None);
        if diff.binary {
            self.line("(binary file)", None);
            return;
        }
        let mut written = 0;
        let total: usize = diff.hunks.iter().map(|h| h.lines.len() + 1).sum();
        'hunks: for hunk in &diff.hunks {
            if written >= LINES_PER_FILE {
                break;
            }
            let mut head = format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            );
            if !hunk.section.is_empty() {
                let _ = write!(head, " {}", hunk.section);
            }
            self.line(&head, None);
            written += 1;
            for line in &hunk.lines {
                if written >= LINES_PER_FILE {
                    break 'hunks;
                }
                let (marker, number) = match line.kind {
                    LineKind::Context => (' ', line.new_number),
                    LineKind::Added => ('+', line.new_number),
                    LineKind::Removed => ('-', line.old_number),
                };
                let place = number.map(|n| (path.clone(), n));
                self.line(&format!("{marker}{}", line.text()), place);
                written += 1;
            }
        }
        if total > written {
            self.line(&format!("[{} more lines of this file left out]", total - written), None);
        }
    }
}

/// The line that names a file and what happened to it.
fn header(diff: &FileDiff) -> String {
    let path = shown(diff.path());
    match (diff.change, diff.old_path.as_deref()) {
        (Change::Added, _) => format!("New file: {path}"),
        (Change::Deleted, _) => format!("Deleted file: {path}"),
        (Change::Renamed { .. }, Some(from)) => format!("Renamed file: {} -> {path}", shown(from)),
        (Change::Copied { .. }, Some(from)) => format!("Copied file: {} -> {path}", shown(from)),
        (Change::Unmerged, _) => format!("File with conflicts: {path}"),
        _ => format!("Changed file: {path}"),
    }
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{Hunk, Line};

    fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Line {
        Line {
            kind,
            old_number: old,
            new_number: new,
            content: text.as_bytes().to_vec(),
            missing_newline: false,
        }
    }

    #[test]
    fn every_line_of_a_file_keeps_its_place_and_a_long_file_is_cut() {
        let mut lines = vec![
            line(LineKind::Context, Some(4), Some(4), "same"),
            line(LineKind::Removed, Some(5), None, "old"),
            line(LineKind::Added, None, Some(5), "new\nline"),
        ];
        let count = u32::try_from(LINES_PER_FILE).unwrap();
        lines.extend((0..count).map(|n| line(LineKind::Added, None, Some(6 + n), "more")));
        let diff = FileDiff {
            old_path: Some("a b.rs".into()),
            new_path: Some("a b.rs".into()),
            change: Change::Modified,
            old_mode: None,
            new_mode: None,
            binary: false,
            hunks: vec![Hunk {
                old_start: 4,
                old_count: 2,
                new_start: 4,
                new_count: 2 + count,
                section: "fn main()".into(),
                lines,
            }],
        };
        let mut builder = Builder::default();
        builder.line("head", None);
        builder.file(&diff);
        let request = MessageRequest {
            places: builder.places,
            text: builder.text,
            files: 1,
            staged: true,
        };
        let text: Vec<&str> = request.text().lines().collect();
        assert_eq!(text.len(), request.places.len());
        assert_eq!(text[2], "Changed file: a b.rs");
        assert_eq!(text[3], "@@ -4,2 +4,202 @@ fn main()");
        assert_eq!(text[6], "+new\\nline");
        let find = |wanted: &str| u32::try_from(text.iter().position(|l| *l == wanted).unwrap() + 1).unwrap();
        assert_eq!(
            request.place(find("-old")),
            Place::File {
                path: "a b.rs".into(),
                line: 5
            }
        );
        assert_eq!(request.place(1), Place::Request { line: 1 });
        assert!(text.last().unwrap().ends_with("more lines of this file left out]"));
        let file_lines = text.len() - 3;
        assert_eq!(file_lines, LINES_PER_FILE + 1);
    }
}
