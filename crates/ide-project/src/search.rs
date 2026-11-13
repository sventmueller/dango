//! Project search with ripgrep's matcher and searcher, over the files the ignore rules leave, on several threads, with
//! results handed over file by file as they are found.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;

use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::{DirEntry, WalkState};

use crate::{Cancel, rules, text};

/// Results one search reports at most, counted per match.
pub const DEFAULT_MAX_RESULTS: usize = 20_000;

/// Files larger than this are left out.
pub const DEFAULT_MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;

/// UTF-16 code units of a line shown in a preview.
const PREVIEW_WIDTH: usize = 250;

/// UTF-16 code units shown before the first match when a long line is cut.
const PREVIEW_LEAD: usize = 40;

/// Files that wait between the search threads and the caller.
const QUEUE: usize = 64;

/// What to search for and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOptions {
    /// The text, or with `regex` a regular expression in the syntax of Rust's `regex` crate.
    pub pattern: String,
    /// Whether `pattern` is a regular expression.
    pub regex: bool,
    /// Whether upper and lower case must match exactly.
    pub case_sensitive: bool,
    /// Whether a match must start and end at a word boundary.
    pub whole_word: bool,
    /// Globs in `.gitignore` syntax of the files to search, such as `*.rs` or `src`; empty for all. A file one of them
    /// names is searched even if an ignore file hides it, unless it lies in an ignored folder.
    pub include: Vec<String>,
    /// Globs in `.gitignore` syntax of files and folders to leave out.
    pub exclude: Vec<String>,
    /// Whether to search what the ignore files hide.
    pub include_ignored: bool,
    /// Matches reported at most; the search stops at this many.
    pub max_results: usize,
    /// Files larger than this many bytes are left out.
    pub max_file_size: u64,
}

impl SearchOptions {
    /// A search for the literal `pattern`, ignoring case, in all files the ignore files leave, with the default limits.
    pub fn new(pattern: &str) -> SearchOptions {
        SearchOptions {
            pattern: pattern.to_string(),
            regex: false,
            case_sensitive: false,
            whole_word: false,
            include: Vec::new(),
            exclude: Vec::new(),
            include_ignored: false,
            max_results: DEFAULT_MAX_RESULTS,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        }
    }
}

/// The matching lines of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResult {
    /// The file.
    pub path: PathBuf,
    /// Its matching lines in order.
    pub lines: Vec<LineResult>,
}

/// A line with matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineResult {
    /// The line number, counted from 1.
    pub line: u64,
    /// Where the matches are, in UTF-16 code units from the start of the line.
    pub columns: Vec<Range<usize>>,
    /// The part of the line to show.
    pub preview: Preview,
}

/// The part of a line a result list shows: the whole line if it is short, else a window around the first match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// The text, without the line's indentation.
    pub text: String,
    /// The UTF-16 column in the line where `text` starts; subtract it from `columns` to highlight within `text`.
    pub start: usize,
    /// Whether the line goes on before `text` with more than indentation.
    pub cut_before: bool,
    /// Whether the line goes on after `text`.
    pub cut_after: bool,
}

/// How a search went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Files with at least one match.
    pub files: usize,
    /// Matches reported; a line that matches only an empty match counts once.
    pub matches: usize,
    /// Files left out because they hold binary data.
    pub binary_skipped: usize,
    /// The search stopped at `max_results`, so there may be more.
    pub limit_reached: bool,
    /// The search stopped because it was cancelled.
    pub cancelled: bool,
}

/// Why a search could not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// The pattern is empty or not a valid regular expression.
    Pattern(String),
    /// One of the globs is not valid.
    Glob(String),
    /// The folder cannot be read.
    Root(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::Pattern(why) => write!(f, "invalid pattern: {why}"),
            SearchError::Glob(why) => write!(f, "invalid glob: {why}"),
            SearchError::Root(why) => write!(f, "cannot search the folder: {why}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// Searches the files below `root` and hands each file with matches to `on_file` on the calling thread, in no
/// particular order, until all are searched, `cancel` stops it or `max_results` is reached.
pub fn search(
    root: &Path,
    options: &SearchOptions,
    cancel: &Cancel,
    on_file: impl FnMut(FileResult),
) -> Result<Summary, SearchError> {
    search_with(root, options, &[], cancel, on_file)
}

/// The same, with `buffers` searched in place of their files: the texts of open editors that are not saved yet.
pub fn search_with(
    root: &Path,
    options: &SearchOptions,
    buffers: &[(PathBuf, String)],
    cancel: &Cancel,
    mut on_file: impl FnMut(FileResult),
) -> Result<Summary, SearchError> {
    let root = root.canonicalize().map_err(|e| SearchError::Root(e.to_string()))?;
    let buffers: HashMap<PathBuf, &str> = buffers
        .iter()
        .map(|(path, text)| (path.canonicalize().unwrap_or_else(|_| path.clone()), text.as_str()))
        .collect();
    let matcher = build_matcher(options)?;
    if options.max_results == 0 {
        return Ok(Summary {
            limit_reached: true,
            ..Summary::default()
        });
    }
    let mut walker = rules::walker(&root, &root, !options.include_ignored);
    walker
        .overrides(build_overrides(&root, options)?)
        .max_filesize(Some(options.max_file_size));
    let stop = Cancel::new();
    let binary = AtomicUsize::new(0);
    let mut summary = Summary::default();
    std::thread::scope(|scope| {
        let (sender, results) = sync_channel::<FileResult>(QUEUE);
        let walk = walker.build_parallel();
        let context = Context {
            matcher: &matcher,
            options,
            buffers: &buffers,
            cancel,
            stop: &stop,
            binary: &binary,
        };
        scope.spawn(move || {
            walk.run(|| {
                let sender = sender.clone();
                let mut searcher = SearcherBuilder::new()
                    .binary_detection(BinaryDetection::quit(0))
                    .line_number(true)
                    .build();
                Box::new(move |entry| context.visit(entry, &mut searcher, &sender))
            });
        });
        for mut found in &results {
            let left = options.max_results - summary.matches;
            summary.limit_reached = keep_within(&mut found.lines, left);
            summary.matches += count(&found.lines);
            summary.files += 1;
            on_file(found);
            if summary.limit_reached || summary.matches >= options.max_results || cancel.is_cancelled() {
                summary.limit_reached |= summary.matches >= options.max_results;
                break;
            }
        }
        stop.cancel();
        drop(results);
    });
    summary.binary_skipped = binary.load(Ordering::Relaxed);
    summary.cancelled = cancel.is_cancelled();
    Ok(summary)
}

#[derive(Clone, Copy)]
struct Context<'a> {
    matcher: &'a RegexMatcher,
    options: &'a SearchOptions,
    buffers: &'a HashMap<PathBuf, &'a str>,
    cancel: &'a Cancel,
    stop: &'a Cancel,
    binary: &'a AtomicUsize,
}

impl Context<'_> {
    fn visit(
        &self,
        entry: Result<DirEntry, ignore::Error>,
        searcher: &mut Searcher,
        sender: &std::sync::mpsc::SyncSender<FileResult>,
    ) -> WalkState {
        if self.cancel.is_cancelled() || self.stop.is_cancelled() {
            return WalkState::Quit;
        }
        let Ok(entry) = entry else { return WalkState::Continue };
        if !self.searchable(&entry) {
            return WalkState::Continue;
        }
        let mut collector = Collector {
            matcher: self.matcher,
            lines: Vec::new(),
            found: 0,
            budget: self.options.max_results,
            binary: false,
            cancel: self.cancel,
            stop: self.stop,
        };
        // A file that vanished or cannot be read is left out like ripgrep does, without failing the search.
        let _ = match self.buffers.get(entry.path()) {
            Some(text) => searcher.search_slice(self.matcher, text.as_bytes(), &mut collector),
            None => searcher.search_path(self.matcher, entry.path(), &mut collector),
        };
        if collector.binary {
            self.binary.fetch_add(1, Ordering::Relaxed);
            return WalkState::Continue;
        }
        if collector.lines.is_empty() {
            return WalkState::Continue;
        }
        let found = FileResult {
            path: entry.into_path(),
            lines: collector.lines,
        };
        match sender.send(found) {
            Ok(()) => WalkState::Continue,
            Err(_) => WalkState::Quit,
        }
    }

    /// A file, or a link to one within the size limit, which the walk checks only for the link itself.
    fn searchable(&self, entry: &DirEntry) -> bool {
        match entry.file_type() {
            Some(t) if t.is_file() => true,
            Some(t) if t.is_symlink() => {
                fs::metadata(entry.path()).is_ok_and(|m| m.is_file() && m.len() <= self.options.max_file_size)
            }
            _ => false,
        }
    }
}

struct Collector<'a> {
    matcher: &'a RegexMatcher,
    lines: Vec<LineResult>,
    found: usize,
    budget: usize,
    binary: bool,
    cancel: &'a Cancel,
    stop: &'a Cancel,
}

impl Sink for Collector<'_> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, found: &SinkMatch<'_>) -> Result<bool, io::Error> {
        if self.cancel.is_cancelled() || self.stop.is_cancelled() {
            return Ok(false);
        }
        let line = trim_line_end(found.bytes());
        let left = self.budget - self.found;
        let mut spans = Vec::new();
        // The matcher of grep-regex cannot fail.
        let _ = self.matcher.find_iter(line, |m| {
            if !m.is_empty() {
                spans.push(m.start()..m.end());
            }
            spans.len() < left
        });
        self.found += spans.len().max(1);
        self.lines
            .push(line_result(line, found.line_number().unwrap_or(0), &spans));
        Ok(self.found < self.budget)
    }

    fn binary_data(&mut self, _searcher: &Searcher, _offset: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}

pub(crate) fn build_matcher(options: &SearchOptions) -> Result<RegexMatcher, SearchError> {
    if options.pattern.is_empty() {
        return Err(SearchError::Pattern("the pattern is empty".into()));
    }
    RegexMatcherBuilder::new()
        .case_insensitive(!options.case_sensitive)
        .word(options.whole_word)
        .fixed_strings(!options.regex)
        .line_terminator(Some(b'\n'))
        .build(&options.pattern)
        .map_err(|e| SearchError::Pattern(e.to_string()))
}

fn build_overrides(root: &Path, options: &SearchOptions) -> Result<Override, SearchError> {
    let glob_error = |e: ignore::Error| SearchError::Glob(e.to_string());
    let mut builder = OverrideBuilder::new(root);
    for glob in options.include.iter().map(|g| g.trim()).filter(|g| !g.is_empty()) {
        builder.add(glob).map_err(glob_error)?;
        // A folder's name alone includes what is in it, as in VS Code.
        builder
            .add(&format!("{}/**", glob.trim_end_matches('/')))
            .map_err(glob_error)?;
    }
    for glob in options.exclude.iter().map(|g| g.trim()).filter(|g| !g.is_empty()) {
        builder.add(&format!("!{glob}")).map_err(glob_error)?;
    }
    builder.build().map_err(glob_error)
}

fn trim_line_end(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn count(lines: &[LineResult]) -> usize {
    lines.iter().map(|l| l.columns.len().max(1)).sum()
}

/// Cuts `lines` to at most `left` matches; whether anything was cut.
fn keep_within(lines: &mut Vec<LineResult>, left: usize) -> bool {
    let mut used = 0;
    for (index, line) in lines.iter_mut().enumerate() {
        let here = line.columns.len().max(1);
        if used + here > left {
            if used < left {
                line.columns.truncate(left - used);
                lines.truncate(index + 1);
            } else {
                lines.truncate(index);
            }
            return true;
        }
        used += here;
    }
    false
}

fn line_result(line: &[u8], number: u64, spans: &[Range<usize>]) -> LineResult {
    let offsets: Vec<usize> = spans.iter().flat_map(|s| [s.start, s.end]).collect();
    let columns: Vec<Range<usize>> = text::columns(line, &offsets).chunks(2).map(|c| c[0]..c[1]).collect();
    let shown = String::from_utf8_lossy(line);
    let focus = columns.first().map_or(0, |c| c.start);
    LineResult {
        line: number,
        preview: preview(&shown, focus),
        columns,
    }
}

fn preview(line: &str, focus: usize) -> Preview {
    let total = text::width(line);
    let (mut start, end) = if total <= PREVIEW_WIDTH {
        (0, total)
    } else {
        let start = focus.saturating_sub(PREVIEW_LEAD).min(total - PREVIEW_WIDTH);
        (start, start + PREVIEW_WIDTH)
    };
    let window = &line[text::byte_at(line, start)..text::byte_at(line, end)];
    let cut_before = start > 0;
    let mut shown = window;
    if !cut_before {
        let indent = text::width(window) - text::width(window.trim_start());
        start = indent.min(focus);
        shown = &window[text::byte_at(window, start)..];
    }
    Preview {
        text: shown.trim_end().to_string(),
        start,
        cut_before,
        cut_after: end < total,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::testing::{one, scratch, write};

    fn project(name: &str) -> ide_core::testing::Scratch {
        let root = scratch(name);
        write(&root, ".gitignore", "target/\n");
        write(
            &root,
            "src/main.rs",
            "fn main() {\n    let needle = 1;\n    println!(\"Needle\");\n}\n",
        );
        write(&root, "src/lib.rs", "pub fn needles() {}\n");
        write(&root, "docs/a.md", "needle in docs\r\n");
        write(&root, "target/x.rs", "needle\n");
        write(&root, "unicode.txt", "äöü 😀 needle\n");
        write(
            &root,
            "long.txt",
            &format!("{}needle{}\n", "x".repeat(1000), "y".repeat(1000)),
        );
        fs::write(root.join("bin.dat"), b"needle\0binary").unwrap();
        root
    }

    /// The relative path of every file with matches and its line numbers.
    fn run(root: &Path, options: &SearchOptions) -> (BTreeMap<String, Vec<u64>>, Summary) {
        let mut found = BTreeMap::new();
        let summary = search(root, options, &Cancel::new(), |file| {
            let relative = file.path.strip_prefix(root).unwrap().to_str().unwrap().to_string();
            found.insert(relative, file.lines.iter().map(|l| l.line).collect());
        })
        .unwrap();
        (found, summary)
    }

    fn files(found: &BTreeMap<String, Vec<u64>>) -> Vec<&str> {
        found.keys().map(String::as_str).collect()
    }

    #[test]
    fn a_literal_search_ignores_case_and_skips_ignored_and_binary_files() {
        let root = project("search-literal");
        let (found, summary) = run(&root, &SearchOptions::new("needle"));
        assert_eq!(
            files(&found),
            ["docs/a.md", "long.txt", "src/lib.rs", "src/main.rs", "unicode.txt"]
        );
        assert_eq!(found["src/main.rs"], [2, 3]);
        assert_eq!(summary.binary_skipped, 1);
        assert_eq!((summary.files, summary.matches), (5, 6));
        assert!(!summary.limit_reached && !summary.cancelled);
    }

    #[test]
    fn unsaved_buffers_are_searched_in_place_of_their_files() {
        let root = project("search-buffers");
        let buffers = [
            (root.join("src/lib.rs"), "pub fn haystack() {}\n".to_string()),
            (
                root.join("docs/../docs/a.md"),
                "nothing\nstill nothing\nneedle at last\n".to_string(),
            ),
            (root.join("target/x.rs"), "needle\n".to_string()),
        ];
        let mut found = BTreeMap::new();
        search_with(&root, &SearchOptions::new("needle"), &buffers, &Cancel::new(), |file| {
            let relative = file.path.strip_prefix(&root).unwrap().to_str().unwrap().to_string();
            found.insert(relative, file.lines.iter().map(|l| l.line).collect::<Vec<_>>());
        })
        .unwrap();
        assert!(!found.contains_key("src/lib.rs"), "the buffer has no match any more");
        assert_eq!(found["docs/a.md"], [3], "the buffer's lines count, not the file's");
        assert!(
            !found.contains_key("target/x.rs"),
            "a buffer of an ignored file stays ignored"
        );
        assert_eq!(found["src/main.rs"], [2, 3]);
    }

    #[test]
    fn case_whole_words_and_regular_expressions_narrow_the_search() {
        let root = project("search-modes");
        let mut options = SearchOptions::new("Needle");
        options.case_sensitive = true;
        assert_eq!(run(&root, &options).0["src/main.rs"], [3]);

        let mut options = SearchOptions::new("needle");
        options.whole_word = true;
        assert!(!run(&root, &options).0.contains_key("src/lib.rs"));

        let mut options = SearchOptions::new(r"fn \w+\(\)");
        options.regex = true;
        assert_eq!(files(&run(&root, &options).0), ["src/lib.rs", "src/main.rs"]);
        let literal = SearchOptions::new(r"fn \w+\(\)");
        assert!(run(&root, &literal).0.is_empty());

        options.pattern = "(unclosed".into();
        assert!(matches!(
            search(&root, &options, &Cancel::new(), |_| ()),
            Err(SearchError::Pattern(_))
        ));
        assert!(matches!(
            search(&root, &SearchOptions::new(""), &Cancel::new(), |_| ()),
            Err(SearchError::Pattern(_))
        ));
    }

    #[test]
    fn globs_choose_files_and_ignored_files_can_be_included() {
        let root = project("search-globs");
        let mut options = SearchOptions::new("needle");
        options.include = vec!["*.rs".into()];
        assert_eq!(files(&run(&root, &options).0), ["src/lib.rs", "src/main.rs"]);
        options.include = vec!["src".into()];
        assert_eq!(files(&run(&root, &options).0), ["src/lib.rs", "src/main.rs"]);
        options.include = vec![];
        options.exclude = vec!["docs".into(), "*.txt".into()];
        assert_eq!(files(&run(&root, &options).0), ["src/lib.rs", "src/main.rs"]);
        options.exclude = vec![];
        options.include_ignored = true;
        assert!(run(&root, &options).0.contains_key("target/x.rs"));
        options.include = vec!["[".into()];
        assert!(matches!(
            search(&root, &options, &Cancel::new(), |_| ()),
            Err(SearchError::Glob(_))
        ));
    }

    #[test]
    fn files_over_the_size_limit_are_left_out() {
        let root = project("search-size");
        let mut options = SearchOptions::new("needle");
        options.max_file_size = 100;
        assert!(!run(&root, &options).0.contains_key("long.txt"));
    }

    #[test]
    fn the_search_stops_at_the_result_limit_or_when_cancelled() {
        let root = project("search-limit");
        let mut options = SearchOptions::new("needle");
        options.max_results = 2;
        let (found, summary) = run(&root, &options);
        assert_eq!(summary.matches, 2);
        assert!(summary.limit_reached);
        assert_eq!(found.values().map(Vec::len).sum::<usize>(), 2);

        let cancel = Cancel::new();
        cancel.cancel();
        let mut calls = 0;
        let summary = search(&root, &SearchOptions::new("needle"), &cancel, |_| calls += 1).unwrap();
        assert!(summary.cancelled);
        assert_eq!(calls, 0);
    }

    #[test]
    fn columns_count_utf16_and_previews_show_a_window_of_long_lines() {
        let root = project("search-columns");
        let mut lines = BTreeMap::new();
        search(&root, &SearchOptions::new("needle"), &Cancel::new(), |file| {
            let name = file.path.file_name().unwrap().to_str().unwrap().to_string();
            lines.insert(name, file.lines[0].clone());
        })
        .unwrap();
        assert_eq!(lines["unicode.txt"].columns, one(7..13));
        assert_eq!(lines["unicode.txt"].preview.text, "äöü 😀 needle");

        let long = &lines["long.txt"];
        assert_eq!(long.columns, one(1000..1006));
        let preview = &long.preview;
        assert!(preview.cut_before && preview.cut_after);
        assert_eq!(preview.start, 960);
        assert_eq!(text::width(&preview.text), PREVIEW_WIDTH);
        assert_eq!(&preview.text[40..46], "needle");

        let indented = &lines["main.rs"];
        assert_eq!(indented.columns, one(8..14));
        assert_eq!(
            (indented.preview.text.as_str(), indented.preview.start),
            ("let needle = 1;", 4)
        );
        assert!(!indented.preview.cut_before);
        assert_eq!(lines["a.md"].preview.text, "needle in docs");
    }
}
