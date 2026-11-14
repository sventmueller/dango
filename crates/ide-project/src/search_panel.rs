//! What the app's search panel decides without AppKit (E98): which search is current, the matches it found grouped per file,
//! what the status line says, and what opening or replacing a row means.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use ide_core::explain::Explain;

use crate::{Cancel, FileResult, LineResult, SearchError, SearchOptions, Summary};
use splat_i18n::{t, tf};

/// Matches one search shows at most; beyond them the user is asked to narrow it.
pub const LIMIT: usize = 5_000;

/// Which option a toggle button stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    /// Match case.
    Case,
    /// Match whole words only.
    Word,
    /// Read the pattern as a regular expression.
    Regex,
}

impl Toggle {
    /// Every toggle in the order the field shows them.
    pub const ALL: [Toggle; 3] = [Toggle::Case, Toggle::Word, Toggle::Regex];
}

/// How a search ended.
pub enum Outcome {
    /// It ran to its end or to the limit.
    Done(Summary),
    /// The pattern or a file pattern was invalid.
    Failed(SearchError),
}

/// The matches of one file.
pub struct Group {
    /// The file.
    pub path: PathBuf,
    /// The path below the folder, as the row shows it.
    pub relative: String,
    /// Its lines with matches, in order.
    pub lines: Vec<LineResult>,
    /// How many matches the lines hold.
    pub matches: usize,
}

/// Where a row of the list points.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Spot {
    /// A file's row.
    File(PathBuf),
    /// A line of a file's matches, by its position among them.
    Line(PathBuf, usize),
}

impl Spot {
    /// The file the row belongs to.
    pub fn path(&self) -> &Path {
        let (Spot::File(path) | Spot::Line(path, _)) = self;
        path
    }
}

/// Which matches of a file to replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Only {
    /// Every match of the file.
    All,
    /// The matches of one line that the results show, at these columns.
    Line(u64, Vec<Range<usize>>),
}

/// What the search field, its toggles and the file patterns hold.
pub struct Query<'a> {
    /// The pattern.
    pub text: &'a str,
    /// Whether case matters.
    pub case: bool,
    /// Whether only whole words match.
    pub word: bool,
    /// Whether the pattern is a regular expression.
    pub regex: bool,
    /// Comma separated patterns of the files to search.
    pub include: &'a str,
    /// Comma separated patterns of the files to leave out.
    pub exclude: &'a str,
}

impl Query<'_> {
    /// The search this query asks for, at most [`LIMIT`] matches.
    pub fn options(&self) -> SearchOptions {
        let globs = |text: &str| -> Vec<String> {
            text.split(',')
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(str::to_string)
                .collect()
        };
        let mut options = SearchOptions::new(self.text);
        options.case_sensitive = self.case;
        options.whole_word = self.word;
        options.regex = self.regex;
        options.include = globs(self.include);
        options.exclude = globs(self.exclude);
        options.max_results = LIMIT;
        options
    }
}

/// What starting the search for the field's text leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// The text changed again since, or the search on show has these options already.
    Nothing,
    /// The field is empty: the list goes, the tree shows.
    Cleared,
    /// Search `generation` runs; its matches come as they are found.
    Search {
        /// The number of the search that started.
        generation: u64,
    },
}

/// A command of a row's context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    /// Open the row's place in an editor.
    Open,
    /// Replace the row's matches.
    Replace,
}

/// The state of one window's search panel.
#[derive(Default)]
pub struct Model {
    root: PathBuf,
    /// Counts changes of the field and options; a timer set for an older one does nothing.
    pending: u64,
    /// Counts the searches started; what an older one finds is dropped.
    run: u64,
    cancel: Option<Cancel>,
    /// The options of the search on show, to leave it be when a change gives the same options again.
    started: Option<SearchOptions>,
    groups: Vec<Group>,
    /// Each group's position in `groups` by path.
    index: HashMap<PathBuf, usize>,
    matches: usize,
    running: bool,
    outcome: Option<Outcome>,
    /// Files the user folded, which stay folded while more results come in.
    collapsed: HashSet<PathBuf>,
    /// Counts requests to search again after a change; a timer set for an older one does nothing.
    refresh: u64,
    /// The running search replaces the matches on show when it ends, instead of showing its own as they come.
    quiet: bool,
    /// The row the context menu was opened on.
    pub menu_row: isize,
}

impl Model {
    /// A panel without a search, for the folder `root`.
    pub fn new(root: PathBuf) -> Model {
        Model {
            root,
            ..Model::default()
        }
    }

    /// The folder it searches.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Notes a change of the field or the options; the id the search for it waits under.
    pub fn changed(&mut self) -> u64 {
        self.pending += 1;
        self.pending
    }

    /// Starts the search for `options` that waited under `pending`.
    pub fn start(&mut self, pending: u64, options: &SearchOptions) -> Started {
        if self.pending != pending || self.started.as_ref() == Some(options) {
            return Started::Nothing;
        }
        self.stop();
        let searching = !options.pattern.is_empty();
        self.run += 1;
        self.clear_matches();
        self.outcome = None;
        self.running = searching;
        self.started = searching.then(|| options.clone());
        if searching {
            Started::Search { generation: self.run }
        } else {
            Started::Cleared
        }
    }

    /// Ends the search that runs.
    pub fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
    }

    /// A search went off with `cancel`; one that is not `progressive` shows its matches only once it ends.
    pub fn spawned(&mut self, cancel: Cancel, progressive: bool) {
        self.cancel = Some(cancel);
        self.quiet = !progressive;
    }

    /// The search could not start.
    pub fn spawn_failed(&mut self) {
        self.running = false;
    }

    /// Notes that files or unsaved texts changed; the id the search again waits under, `None` without matches on show.
    pub fn refresh_soon(&mut self) -> Option<u64> {
        self.started.is_some().then(|| self.next_refresh())
    }

    /// The id of a search again that nothing waits for.
    pub fn next_refresh(&mut self) -> u64 {
        self.refresh += 1;
        self.refresh
    }

    /// Searches again for the matches on show, if `refresh` is still the latest request and the options are the same;
    /// the generation of the search.
    pub fn refresh(&mut self, refresh: u64, options: &SearchOptions) -> Option<u64> {
        if self.refresh != refresh || self.started.as_ref() != Some(options) {
            return None;
        }
        self.stop();
        self.run += 1;
        self.running = true;
        Some(self.run)
    }

    /// Takes `files` and maybe the end of search `generation`; whether that changed what shows, which it does not when
    /// another search started since.
    pub fn take(&mut self, generation: u64, files: Vec<FileResult>, outcome: Option<Outcome>) -> bool {
        if self.run != generation || self.started.is_none() {
            return false;
        }
        if self.quiet && outcome.is_some() {
            self.quiet = false;
            self.clear_matches();
        }
        for found in files {
            self.insert(found);
        }
        if let Some(outcome) = outcome {
            self.running = false;
            self.cancel = None;
            self.outcome = Some(outcome);
        }
        true
    }

    fn clear_matches(&mut self) {
        self.groups.clear();
        self.index.clear();
        self.matches = 0;
    }

    /// Puts `found` into the groups in the order of their paths.
    fn insert(&mut self, found: FileResult) {
        let relative = found
            .path
            .strip_prefix(&self.root)
            .unwrap_or(&found.path)
            .to_string_lossy()
            .into_owned();
        let at = self.groups.partition_point(|g| g.relative < relative);
        let matches = count(&found.lines);
        self.matches += matches;
        self.groups.insert(
            at,
            Group {
                path: found.path,
                relative,
                lines: found.lines,
                matches,
            },
        );
        for (position, group) in self.groups.iter().enumerate().skip(at) {
            self.index.insert(group.path.clone(), position);
        }
    }

    /// Whether a search shows its matches, rather than the panel the tree.
    pub fn searching(&self) -> bool {
        self.started.is_some()
    }

    /// Whether the matches on show are those of `options`, or the tree for an empty pattern.
    pub fn shows(&self, options: &SearchOptions) -> bool {
        self.started
            .as_ref()
            .map_or(options.pattern.is_empty(), |started| started == options)
    }

    /// Whether a search still runs.
    pub fn running(&self) -> bool {
        self.running
    }

    /// The matches found, one group per file.
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// The matches of the file at `path`.
    pub fn group(&self, path: &Path) -> Option<&Group> {
        self.groups.get(*self.index.get(path)?)
    }

    /// The match line `spot` points at, the first of its file for a file.
    pub fn line(&self, spot: &Spot) -> Option<&LineResult> {
        let index = match spot {
            Spot::File(_) => 0,
            Spot::Line(_, index) => *index,
        };
        self.group(spot.path())?.lines.get(index)
    }

    /// Whether `spot` is a row of the matches on show.
    pub fn has(&self, spot: &Spot) -> bool {
        match spot {
            Spot::File(path) => self.index.contains_key(path),
            Spot::Line(..) => self.line(spot).is_some(),
        }
    }

    /// How many rows lie below `parent`, the files for the top.
    pub fn children(&self, parent: Option<&Spot>) -> usize {
        match parent {
            None => self.groups.len(),
            Some(Spot::File(path)) => self.group(path).map_or(0, |g| g.lines.len()),
            Some(Spot::Line(..)) => 0,
        }
    }

    /// The row at `index` below `parent`, the files for the top.
    pub fn child(&self, index: usize, parent: Option<&Spot>) -> Option<Spot> {
        match parent {
            None => Some(Spot::File(self.groups.get(index)?.path.clone())),
            Some(Spot::File(path)) => (index < self.group(path)?.lines.len()).then(|| Spot::Line(path.clone(), index)),
            Some(Spot::Line(..)) => None,
        }
    }

    /// The files the user did not fold.
    pub fn unfolded(&self) -> Vec<Spot> {
        self.groups
            .iter()
            .filter(|g| !self.collapsed.contains(&g.path))
            .map(|g| Spot::File(g.path.clone()))
            .collect()
    }

    /// Notes that the user folded the file `path` or opened it again.
    pub fn folded(&mut self, path: PathBuf, collapsed: bool) {
        if collapsed {
            self.collapsed.insert(path);
        } else {
            self.collapsed.remove(&path);
        }
    }

    /// Where opening `spot` goes: the file, its line and the first match's columns.
    pub fn target(&self, spot: &Spot) -> Option<(PathBuf, u64, Range<usize>)> {
        let line = self.line(spot)?;
        Some((
            spot.path().to_path_buf(),
            line.line,
            line.columns.first().cloned().unwrap_or(0..0),
        ))
    }

    /// What replacing at `spot` replaces: every match of a file, or those of one line.
    pub fn replacement(&self, spot: &Spot) -> Option<(PathBuf, Only)> {
        let only = match spot {
            Spot::File(_) => Only::All,
            Spot::Line(..) => {
                let line = self.line(spot)?;
                Only::Line(line.line, line.columns.clone())
            }
        };
        Some((spot.path().to_path_buf(), only))
    }

    /// The files with matches on show.
    pub fn files(&self) -> Vec<PathBuf> {
        self.groups.iter().map(|g| g.path.clone()).collect()
    }

    /// Whether every match on show can be replaced: a search that ended with matches and was not cut at its limit.
    pub fn can_replace_all(&self) -> bool {
        let done = matches!(&self.outcome, Some(Outcome::Done(summary)) if !summary.limit_reached);
        done && !self.running && self.matches > 0
    }

    /// What the status line says about the matches so far, and whether it reports a problem.
    pub fn status(&self) -> (String, bool) {
        match &self.outcome {
            Some(Outcome::Failed(error)) => return (error.explain(), true),
            Some(Outcome::Done(summary)) if summary.limit_reached => {
                return (
                    tf("Mehr als {} Treffer — Suche eingrenzen", &[&LIMIT.to_string()]),
                    false,
                );
            }
            Some(Outcome::Done(_)) if self.groups.is_empty() => return (t("Keine Treffer").to_string(), false),
            _ => {}
        }
        if self.groups.is_empty() {
            return (t("Suche läuft …").to_string(), false);
        }
        let (matches, files) = (self.matches, self.groups.len());
        let text = match (matches, files) {
            (1, 1) => t("1 Treffer in 1 Datei").to_string(),
            (_, 1) => tf("{} Treffer in 1 Datei", &[&matches.to_string()]),
            _ => tf("{} Treffer in {} Dateien", &[&matches.to_string(), &files.to_string()]),
        };
        (text, false)
    }

    /// The question before replacing every match on show by `with`, and what it adds.
    pub fn replace_question(&self, with: &str) -> (String, String) {
        let (matches, files) = (self.matches, self.groups.len());
        let question = match (matches, files) {
            (1, _) => t("1 Treffer in 1 Datei ersetzen?").to_string(),
            (_, 1) => tf("{} Treffer in 1 Datei ersetzen?", &[&matches.to_string()]),
            _ => tf(
                "{} Treffer in {} Dateien ersetzen?",
                &[&matches.to_string(), &files.to_string()],
            ),
        };
        let detail = if with.is_empty() {
            t("Die Treffer werden entfernt. In geöffneten Dateien nimmt ⌘Z das zurück, die übrigen werden gleich gesichert.")
                .to_string()
        } else {
            tf(
                "Durch „{}“. In geöffneten Dateien nimmt ⌘Z das zurück, die übrigen werden gleich gesichert.",
                &[with],
            )
        };
        (question, detail)
    }

    /// The commands of the context menu on `spot`; replacing only while the replace field shows.
    pub fn menu(spot: &Spot, replacing: bool) -> Vec<(&'static str, MenuCommand)> {
        let mut entries = vec![(t("Öffnen"), MenuCommand::Open)];
        if replacing {
            let title = match spot {
                Spot::File(_) => t("Alle in dieser Datei ersetzen"),
                Spot::Line(..) => t("Ersetzen"),
            };
            entries.push((title, MenuCommand::Replace));
        }
        entries
    }
}

/// What the notice says after `replaced` matches in `files` files were replaced; nothing if none were.
pub fn replaced_note(replaced: usize, files: usize) -> Option<String> {
    match (replaced, files) {
        (0, _) => None,
        (1, _) => Some(t("1 Treffer ersetzt").to_string()),
        (n, 1) => Some(tf("{} Treffer in 1 Datei ersetzt", &[&n.to_string()])),
        (n, files) => Some(tf(
            "{} Treffer in {} Dateien ersetzt",
            &[&n.to_string(), &files.to_string()],
        )),
    }
}

/// Matches a line counts: its match ranges, or one for a line that matched only empty.
fn count(lines: &[LineResult]) -> usize {
    lines.iter().map(|l| l.columns.len().max(1)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Preview;

    fn query(text: &str) -> SearchOptions {
        Query {
            text,
            case: false,
            word: false,
            regex: false,
            include: " *.rs, ,src/** ",
            exclude: "",
        }
        .options()
    }

    fn found(path: &str, lines: &[(u64, usize)]) -> FileResult {
        FileResult {
            path: PathBuf::from(path),
            lines: lines
                .iter()
                .map(|&(line, matches)| LineResult {
                    line,
                    columns: (0..matches).map(|i| i * 4..i * 4 + 3).collect(),
                    preview: Preview {
                        text: String::new(),
                        start: 0,
                        cut_before: false,
                        cut_after: false,
                    },
                })
                .collect(),
        }
    }

    fn done(limit_reached: bool) -> Option<Outcome> {
        Some(Outcome::Done(Summary {
            limit_reached,
            ..Summary::default()
        }))
    }

    fn searching(model: &mut Model, text: &str) -> u64 {
        let pending = model.changed();
        let Started::Search { generation } = model.start(pending, &query(text)) else {
            panic!("no search started");
        };
        generation
    }

    #[test]
    fn the_patterns_are_split_at_commas_and_trimmed() {
        let options = query("x");
        assert_eq!(options.include, ["*.rs", "src/**"]);
        assert!(options.exclude.is_empty());
        assert_eq!(options.max_results, LIMIT);
    }

    #[test]
    fn only_the_latest_change_starts_and_the_same_options_do_not_start_again() {
        let mut model = Model::new("/p".into());
        let old = model.changed();
        let new = model.changed();
        assert_eq!(model.start(old, &query("a")), Started::Nothing);
        assert_eq!(model.start(new, &query("a")), Started::Search { generation: 1 });
        let again = model.changed();
        assert_eq!(model.start(again, &query("a")), Started::Nothing);
        let empty = model.changed();
        assert_eq!(model.start(empty, &query("")), Started::Cleared);
        assert!(!model.searching());
    }

    #[test]
    fn matches_are_grouped_in_path_order_and_an_older_search_is_dropped() {
        let mut model = Model::new("/p".into());
        let first = searching(&mut model, "a");
        let second = searching(&mut model, "b");
        assert!(!model.take(first, vec![found("/p/z.rs", &[(1, 1)])], None));
        assert!(model.take(
            second,
            vec![found("/p/b.rs", &[(1, 2)]), found("/p/a.rs", &[(3, 0)])],
            None
        ));
        let relative: Vec<&str> = model.groups().iter().map(|g| g.relative.as_str()).collect();
        assert_eq!(relative, ["a.rs", "b.rs"]);
        assert_eq!(model.status(), (tf("{} Treffer in {} Dateien", &["3", "2"]), false));
        assert!(!model.can_replace_all());
        assert!(model.take(second, vec![], done(false)));
        assert!(model.can_replace_all());
        assert!(!model.running());
    }

    #[test]
    fn rows_and_their_targets_follow_the_groups() {
        let mut model = Model::new("/p".into());
        let generation = searching(&mut model, "a");
        model.take(generation, vec![found("/p/a.rs", &[(4, 2), (9, 1)])], done(false));
        let file = Spot::File("/p/a.rs".into());
        assert_eq!(model.children(None), 1);
        assert_eq!(model.child(0, None), Some(file.clone()));
        assert_eq!(model.children(Some(&file)), 2);
        assert_eq!(model.child(1, Some(&file)), Some(Spot::Line("/p/a.rs".into(), 1)));
        assert_eq!(model.child(2, Some(&file)), None);
        assert_eq!(model.target(&file), Some(("/p/a.rs".into(), 4, 0..3)));
        let line = Spot::Line("/p/a.rs".into(), 1);
        assert_eq!(
            model.replacement(&line),
            Some(("/p/a.rs".into(), Only::Line(9, std::iter::once(0..3).collect())))
        );
        assert_eq!(model.replacement(&file), Some(("/p/a.rs".into(), Only::All)));
        assert!(!model.has(&Spot::Line("/p/a.rs".into(), 2)));
        model.folded("/p/a.rs".into(), true);
        assert!(model.unfolded().is_empty());
    }

    #[test]
    fn a_search_again_replaces_the_matches_only_when_it_ends() {
        let mut model = Model::new("/p".into());
        let generation = searching(&mut model, "a");
        model.take(generation, vec![found("/p/a.rs", &[(1, 1)])], done(false));
        let refresh = model.refresh_soon().expect("matches show");
        assert_eq!(model.refresh(refresh, &query("b")), None);
        let again = model.refresh(refresh, &query("a")).expect("same options");
        model.spawned(Cancel::new(), false);
        assert_eq!(model.files(), [PathBuf::from("/p/a.rs")]);
        model.take(again, vec![found("/p/b.rs", &[(2, 1)])], done(false));
        assert_eq!(model.files(), [PathBuf::from("/p/b.rs")]);
    }

    #[test]
    fn the_status_says_why_nothing_shows() {
        let mut model = Model::new("/p".into());
        let generation = searching(&mut model, "a");
        assert_eq!(model.status(), (t("Suche läuft …").to_string(), false));
        model.take(generation, vec![], done(false));
        assert_eq!(model.status(), (t("Keine Treffer").to_string(), false));
        let generation = searching(&mut model, "(");
        model.take(
            generation,
            vec![],
            Some(Outcome::Failed(SearchError::Pattern("x".into()))),
        );
        assert!(model.status().1);
        let generation = searching(&mut model, "b");
        model.take(generation, vec![found("/p/a.rs", &[(1, 1)])], done(true));
        assert!(!model.can_replace_all());
    }

    #[test]
    fn the_menu_offers_replacing_only_with_the_replace_field() {
        let line = Spot::Line("/p/a.rs".into(), 0);
        assert_eq!(Model::menu(&line, false).len(), 1);
        assert_eq!(Model::menu(&line, true)[1], (t("Ersetzen"), MenuCommand::Replace));
        assert_eq!(replaced_note(0, 0), None);
        assert_eq!(replaced_note(1, 1), Some(t("1 Treffer ersetzt").to_string()));
    }
}
