//! The files of a project for quick open: found by a parallel walk that honours the ignore files, patched from the
//! watcher, matched fuzzily on several threads.

use ide_core::{IgnoringPoison, LockIgnoringPoison};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashSet};
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use ignore::{DirEntry, ParallelVisitor, ParallelVisitorBuilder, WalkState};

use crate::fuzzy::{Pattern, Scratch};
use crate::watch::{Changes, WatchEvent};
use crate::{Cancel, rules};

/// Paths matched on one thread; a larger index is split over several.
const CHUNK: usize = 16_384;

/// Paths matched between two looks at the cancel token.
const CANCEL_STRIDE: usize = 1_024;

/// A file that matches a quick-open query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The path relative to the project folder, with `/` between folders.
    pub path: String,
    /// Higher is better.
    pub(crate) score: i32,
    /// The matched characters of `path` as ranges of UTF-16 code units, for highlighting.
    pub ranges: Vec<Range<usize>>,
}

/// The files of one project that ignore rules do not hide, as paths relative to its folder.
#[derive(Debug, Clone)]
pub struct FileIndex {
    root: PathBuf,
    /// Sorted, so the files below a folder lie together.
    files: Vec<Arc<str>>,
}

impl FileIndex {
    /// Walks `root` on several threads; fails with `Interrupted` when `cancel` stops it.
    pub fn build(root: &Path, cancel: &Cancel) -> io::Result<FileIndex> {
        let root = root.canonicalize()?;
        let files = walk_parallel(&root, cancel)?;
        Ok(FileIndex { root, files })
    }

    /// The project folder with links resolved.
    #[cfg(test)]
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// How many files the index holds.
    pub(crate) fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the index holds no file.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Whether the index holds the file at `relative`.
    #[cfg(test)]
    pub(crate) fn contains(&self, relative: &str) -> bool {
        self.files.binary_search_by(|f| (**f).cmp(relative)).is_ok()
    }

    /// The relative paths in byte order.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|f| &**f)
    }

    /// Brings the index up to date with what the watcher saw; `Interrupted` leaves it as it was.
    #[cfg(test)]
    pub(crate) fn apply(&mut self, event: &WatchEvent, cancel: &Cancel) -> io::Result<()> {
        match self.plan(event, cancel)? {
            Plan::Rebuild => self.files = walk_parallel(&self.root, cancel)?,
            Plan::Edit(edit) => self.commit(edit),
        }
        Ok(())
    }

    /// Up to `limit` files matching `query`, best first; empty for an empty query or when `cancel` stops it.
    pub(crate) fn query(&self, query: &str, limit: usize, cancel: &Cancel) -> Vec<Hit> {
        let Some(pattern) = Pattern::new(query) else {
            return Vec::new();
        };
        if limit == 0 {
            return Vec::new();
        }
        let threads = std::thread::available_parallelism().map_or(1, usize::from);
        let chunk = CHUNK.max(self.files.len().div_ceil(threads));
        let mut best: Vec<Ranked> = if self.files.len() <= chunk {
            best_of(&self.files, &pattern, limit, cancel)
        } else {
            std::thread::scope(|scope| {
                let workers: Vec<_> = self
                    .files
                    .chunks(chunk)
                    .map(|part| scope.spawn(|| best_of(part, &pattern, limit, cancel)))
                    .collect();
                workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
            })
        };
        if cancel.is_cancelled() {
            return Vec::new();
        }
        best.sort_unstable_by(|a, b| b.cmp(a));
        best.truncate(limit);
        best.into_iter()
            .filter_map(|ranked| {
                let path = ranked.2.0;
                let found = pattern.find(&path)?;
                Some(Hit {
                    path: path.to_string(),
                    score: found.score,
                    ranges: found.ranges,
                })
            })
            .collect()
    }

    /// Works out from the disk what `event` changed, without changing the index yet.
    fn plan(&self, event: &WatchEvent, cancel: &Cancel) -> io::Result<Plan> {
        let WatchEvent::Changed(changes) = event else {
            return Ok(Plan::Rebuild);
        };
        let subtrees: Vec<String> = changes.subtrees.iter().filter_map(|d| self.relative(d)).collect();
        if changes.rules_changed || subtrees.iter().any(String::is_empty) {
            return Ok(Plan::Rebuild);
        }
        let mut edit = Edit::default();
        let mut matcher = rules::matcher(&self.root);
        for subtree in &subtrees {
            if matcher.matched(subtree, true).is_ignore() {
                continue;
            }
            edit.gone_below.push(format!("{subtree}/"));
            if self.root.join(subtree).is_dir() {
                edit.walk(&self.root, subtree, cancel)?;
            }
        }
        let inside_subtree = |dir: &str| subtrees.iter().any(|s| Path::new(dir).starts_with(s));
        for dir in changes.dirs.iter().filter_map(|d| self.relative(d)) {
            if inside_subtree(&dir) || (!dir.is_empty() && matcher.matched(&dir, true).is_ignore()) {
                continue;
            }
            self.compare(&dir, &mut matcher, &mut edit, cancel)?;
        }
        Ok(Plan::Edit(edit))
    }

    /// Notes how the files directly in `dir` and its subfolders differ from the disk.
    fn compare(
        &self,
        dir: &str,
        matcher: &mut ignore::IncrementalIgnore,
        edit: &mut Edit,
        cancel: &Cancel,
    ) -> io::Result<()> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let Ok(items) = fs::read_dir(self.root.join(dir)) else {
            edit.gone_below.push(prefix);
            return Ok(());
        };
        let mut indexed_files = HashSet::new();
        let mut indexed_dirs = HashSet::new();
        let below = self.below(&prefix);
        let mut at = below.start;
        while at < below.end {
            let rest = &self.files[at][prefix.len()..];
            match rest.split_once('/') {
                Some((sub, _)) => {
                    indexed_dirs.insert(sub);
                    at = self.below(&format!("{prefix}{sub}/")).end;
                }
                None => {
                    indexed_files.insert(rest);
                    at += 1;
                }
            }
        }
        let mut disk_files = HashSet::new();
        let mut disk_dirs = HashSet::new();
        for item in items.flatten() {
            let Some(name) = item.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Ok(file_type) = item.file_type() else { continue };
            if name == rules::GIT_DIR
                || matcher
                    .matched(format!("{prefix}{name}"), file_type.is_dir())
                    .is_ignore()
            {
                continue;
            }
            if file_type.is_dir() {
                disk_dirs.insert(name);
            } else if is_indexed_file(&item.path(), file_type) {
                disk_files.insert(name);
            }
        }
        for gone in indexed_files.iter().filter(|name| !disk_files.contains(**name)) {
            edit.gone.insert(format!("{prefix}{gone}"));
        }
        for new in disk_files.iter().filter(|name| !indexed_files.contains(name.as_str())) {
            edit.added.push(format!("{prefix}{new}").into());
        }
        for gone in indexed_dirs.iter().filter(|name| !disk_dirs.contains(**name)) {
            edit.gone_below.push(format!("{prefix}{gone}/"));
        }
        for new in disk_dirs.iter().filter(|name| !indexed_dirs.contains(name.as_str())) {
            edit.walk(&self.root, &format!("{prefix}{new}"), cancel)?;
        }
        Ok(())
    }

    fn commit(&mut self, edit: Edit) {
        let mut gone: Vec<Range<usize>> = edit
            .gone_below
            .iter()
            .map(|prefix| self.below(prefix))
            .filter(|range| !range.is_empty())
            .collect();
        gone.extend(
            edit.gone
                .iter()
                .filter_map(|path| self.files.binary_search_by(|f| (**f).cmp(path.as_str())).ok())
                .map(|at| at..at + 1),
        );
        if !gone.is_empty() {
            let mut dropped = vec![false; self.files.len()];
            for range in gone {
                dropped[range].fill(true);
            }
            let mut keep = dropped.iter().map(|d| !d);
            self.files.retain(|_| keep.next().unwrap_or(true));
        }
        if edit.added.is_empty() {
            return;
        }
        let mut added = edit.added;
        added.sort_unstable();
        added.dedup();
        let old = std::mem::take(&mut self.files);
        self.files = merge(old, added);
    }

    /// The positions of the files whose paths start with `prefix`.
    fn below(&self, prefix: &str) -> Range<usize> {
        let start = self.files.partition_point(|f| &**f < prefix);
        let end = start + self.files[start..].partition_point(|f| f.starts_with(prefix));
        start..end
    }

    fn relative(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.root).ok()?.to_str().map(str::to_string)
    }
}

/// What the index must do to catch up with the disk.
enum Plan {
    Rebuild,
    Edit(Edit),
}

#[derive(Default)]
struct Edit {
    gone: BTreeSet<String>,
    gone_below: Vec<String>,
    added: Vec<Arc<str>>,
    walked: HashSet<String>,
}

impl Edit {
    /// Adds the files below `relative`, unless an earlier step of the same edit already walked it.
    fn walk(&mut self, root: &Path, relative: &str, cancel: &Cancel) -> io::Result<()> {
        if self.walked.insert(relative.to_string()) {
            self.added.extend(walk_one(root, relative, cancel)?);
        }
        Ok(())
    }
}

/// A path ranked for a query: score, then shorter, then earlier in byte order.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Ranked(i32, Reverse<usize>, Reverse<Arc<str>>);

fn best_of(chunk: &[Arc<str>], pattern: &Pattern, limit: usize, cancel: &Cancel) -> Vec<Ranked> {
    let mut scratch = Scratch::default();
    let mut kept: BinaryHeap<Reverse<Ranked>> = BinaryHeap::with_capacity(limit.min(chunk.len()) + 1);
    for (n, path) in chunk.iter().enumerate() {
        if n % CANCEL_STRIDE == 0 && cancel.is_cancelled() {
            return Vec::new();
        }
        if let Some(score) = pattern.score(path, &mut scratch) {
            kept.push(Reverse(Ranked(score, Reverse(path.len()), Reverse(path.clone()))));
            if kept.len() > limit {
                kept.pop();
            }
        }
    }
    kept.into_iter().map(|Reverse(ranked)| ranked).collect()
}

fn merge(old: Vec<Arc<str>>, added: Vec<Arc<str>>) -> Vec<Arc<str>> {
    let mut merged = Vec::with_capacity(old.len() + added.len());
    let mut old = old.into_iter().peekable();
    let mut added = added.into_iter().peekable();
    loop {
        let next = match (old.peek(), added.peek()) {
            (Some(a), Some(b)) if a < b => old.next(),
            (Some(a), Some(b)) if a == b => {
                added.next();
                old.next()
            }
            (_, Some(_)) => added.next(),
            (Some(_), None) => old.next(),
            (None, None) => break,
        };
        merged.extend(next);
    }
    merged
}

/// A regular file, or a link to anything but a folder: links are listed, never entered.
fn is_indexed_file(path: &Path, file_type: fs::FileType) -> bool {
    file_type.is_file() || (file_type.is_symlink() && fs::metadata(path).is_ok_and(|m| !m.is_dir()))
}

fn relative_file(root: &Path, entry: &DirEntry) -> Option<Arc<str>> {
    let file_type = entry.file_type()?;
    if entry.depth() == 0 || !is_indexed_file(entry.path(), file_type) {
        return None;
    }
    entry.path().strip_prefix(root).ok()?.to_str().map(Arc::from)
}

fn walk_one(root: &Path, relative: &str, cancel: &Cancel) -> io::Result<Vec<Arc<str>>> {
    let mut found = Vec::new();
    for entry in rules::walker(root, &root.join(relative), true).build() {
        if cancel.is_cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        found.extend(entry.ok().and_then(|entry| relative_file(root, &entry)));
    }
    Ok(found)
}

fn walk_parallel(root: &Path, cancel: &Cancel) -> io::Result<Vec<Arc<str>>> {
    let found = Mutex::new(Vec::new());
    let mut collect = Collect {
        root,
        cancel,
        found: &found,
    };
    rules::walker(root, root, true).build_parallel().visit(&mut collect);
    if cancel.is_cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let mut files = found.into_inner().ignoring_poison().concat();
    files.sort_unstable();
    Ok(files)
}

struct Collect<'a> {
    root: &'a Path,
    cancel: &'a Cancel,
    found: &'a Mutex<Vec<Vec<Arc<str>>>>,
}

impl<'a> ParallelVisitorBuilder<'a> for Collect<'a> {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 'a> {
        Box::new(Visitor {
            root: self.root,
            cancel: self.cancel,
            found: self.found,
            mine: Vec::new(),
        })
    }
}

struct Visitor<'a> {
    root: &'a Path,
    cancel: &'a Cancel,
    found: &'a Mutex<Vec<Vec<Arc<str>>>>,
    mine: Vec<Arc<str>>,
}

impl ParallelVisitor for Visitor<'_> {
    fn visit(&mut self, entry: Result<DirEntry, ignore::Error>) -> WalkState {
        if self.cancel.is_cancelled() {
            return WalkState::Quit;
        }
        self.mine
            .extend(entry.ok().and_then(|entry| relative_file(self.root, &entry)));
        WalkState::Continue
    }
}

impl Drop for Visitor<'_> {
    fn drop(&mut self) {
        let mine = std::mem::take(&mut self.mine);
        self.found.lock_ignoring_poison().push(mine);
    }
}

/// A finished walk over the whole project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indexed {
    /// How many files the index holds.
    pub(crate) files: usize,
    /// How long the walk took.
    pub(crate) took: Duration,
}

type Answer = Box<dyn FnOnce(Vec<Hit>) + Send>;

struct Asked {
    query: String,
    limit: usize,
    cancel: Cancel,
    answer: Answer,
}

#[derive(Default)]
struct Pending {
    rescan: bool,
    changes: Changes,
}

struct Shared {
    root: PathBuf,
    index: RwLock<Option<FileIndex>>,
    /// Whether the first walk is done, readable without waiting for the index lock.
    ready: AtomicBool,
    pending: Mutex<Pending>,
    /// Wakes the index thread for `pending`.
    work: Condvar,
    asked: Mutex<Option<Asked>>,
    /// Wakes the query thread for `asked` or a finished first walk.
    queries: Condvar,
    closed: Cancel,
}

/// Quick open for one project: keeps its index on a thread of its own and answers queries on another, the latest
/// query first.
pub struct QuickOpen {
    shared: Arc<Shared>,
}

/// Folders one pending update may name before a full walk is cheaper.
const MAX_PENDING_DIRS: usize = 10_000;

impl QuickOpen {
    /// Starts indexing `root`; `on_indexed` runs on the index thread after every full walk.
    pub fn start(root: &Path, on_indexed: impl Fn(Indexed) + Send + 'static) -> io::Result<QuickOpen> {
        let shared = Arc::new(Shared {
            root: root.canonicalize()?,
            index: RwLock::new(None),
            ready: AtomicBool::new(false),
            pending: Mutex::new(Pending {
                rescan: true,
                changes: Changes::default(),
            }),
            work: Condvar::new(),
            asked: Mutex::new(None),
            queries: Condvar::new(),
            closed: Cancel::new(),
        });
        let indexer = shared.clone();
        std::thread::Builder::new().name("ide-index".into()).spawn(move || {
            ide_process::priority::set_qos(ide_process::priority::Qos::Utility);
            indexer.keep_index(&on_indexed);
        })?;
        let answerer = shared.clone();
        std::thread::Builder::new()
            .name("ide-quick-open".into())
            .spawn(move || {
                ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
                answerer.answer_queries();
            })?;
        Ok(QuickOpen { shared })
    }

    /// Passes a report of the watcher on to the index.
    pub fn changed(&self, event: &WatchEvent) {
        let mut pending = self.shared.pending.lock_ignoring_poison();
        match event {
            WatchEvent::Changed(changes) if !changes.rules_changed => {
                pending.changes.dirs.extend(changes.dirs.iter().cloned());
                pending.changes.subtrees.extend(changes.subtrees.iter().cloned());
                if pending.changes.dirs.len() + pending.changes.subtrees.len() > MAX_PENDING_DIRS {
                    pending.rescan = true;
                }
            }
            _ => pending.rescan = true,
        }
        drop(pending);
        self.shared.work.notify_all();
    }

    /// Answers `query` with up to `limit` hits on the query thread, once the first walk is done. A query that a newer
    /// one replaces before it is answered gets no answer.
    pub fn query(&self, query: &str, limit: usize, answer: impl FnOnce(Vec<Hit>) + Send + 'static) {
        let mut asked = self.shared.asked.lock_ignoring_poison();
        if let Some(earlier) = asked.take() {
            earlier.cancel.cancel();
        }
        *asked = Some(Asked {
            query: query.to_string(),
            limit,
            cancel: Cancel::new(),
            answer: Box::new(answer),
        });
        drop(asked);
        self.shared.queries.notify_all();
    }
}

impl Drop for QuickOpen {
    fn drop(&mut self) {
        self.shared.closed.cancel();
        if let Some(asked) = self.shared.asked.lock_ignoring_poison().take() {
            asked.cancel.cancel();
        }
        self.shared.queries.notify_all();
        drop(self.shared.pending.lock_ignoring_poison());
        self.shared.work.notify_all();
    }
}

impl Shared {
    fn keep_index(&self, on_indexed: &dyn Fn(Indexed)) {
        while let Some(work) = self.next_work() {
            let plan = match self.index.read().ignoring_poison().as_ref() {
                Some(index) if !work.rescan => index.plan(&WatchEvent::Changed(work.changes), &self.closed),
                _ => Ok(Plan::Rebuild),
            };
            match plan {
                Ok(Plan::Rebuild) => self.rebuild(on_indexed),
                Ok(Plan::Edit(edit)) => {
                    if let Some(index) = self.index.write().ignoring_poison().as_mut() {
                        index.commit(edit);
                    }
                }
                Err(_) => {}
            }
        }
    }

    /// Waits for changes to apply; `None` once quick open is dropped.
    fn next_work(&self) -> Option<Pending> {
        let mut pending = self.pending.lock_ignoring_poison();
        while !pending.rescan && pending.changes == Changes::default() && !self.closed.is_cancelled() {
            pending = self.work.wait(pending).ignoring_poison();
        }
        (!self.closed.is_cancelled()).then(|| std::mem::take(&mut *pending))
    }

    fn rebuild(&self, on_indexed: &dyn Fn(Indexed)) {
        let started = Instant::now();
        let Ok(index) = FileIndex::build(&self.root, &self.closed) else {
            return;
        };
        let indexed = Indexed {
            files: index.len(),
            took: started.elapsed(),
        };
        *self.index.write().ignoring_poison() = Some(index);
        self.ready.store(true, Ordering::Release);
        // Taking the lock first means the query thread is either before its check or waiting.
        drop(self.asked.lock_ignoring_poison());
        self.queries.notify_all();
        ide_core::lane::contained(|| on_indexed(indexed));
    }

    fn answer_queries(&self) {
        loop {
            let mut asked = self.asked.lock_ignoring_poison();
            while (asked.is_none() || !self.ready.load(Ordering::Acquire)) && !self.closed.is_cancelled() {
                asked = self.queries.wait(asked).ignoring_poison();
            }
            if self.closed.is_cancelled() {
                return;
            }
            let Some(Asked {
                query,
                limit,
                cancel,
                answer,
            }) = asked.take()
            else {
                continue;
            };
            drop(asked);
            let hits = match self.index.read().ignoring_poison().as_ref() {
                Some(index) => index.query(&query, limit, &cancel),
                None => continue,
            };
            if !cancel.is_cancelled() {
                ide_core::lane::contained(|| answer(hits));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use super::*;
    use crate::testing::{compute_factor, one, scratch, walk_factor, write};

    /// `dirs` folders of `per_dir` files each, plus as many again below an ignored folder.
    fn generate(name: &str, dirs: usize, per_dir: usize) -> ide_core::testing::Scratch {
        let root = scratch(name);
        write(&root, ".gitignore", "build/\n");
        for d in 0..dirs {
            for top in ["src", "build"] {
                let dir = root.join(format!("{top}/module{d}"));
                fs::create_dir_all(&dir).unwrap();
                for f in 0..per_dir {
                    fs::write(dir.join(format!("file{f}.rs")), "").unwrap();
                }
            }
        }
        root
    }

    /// Indexes `root` within `budget` and queries it within a tenth of that, with a query every file matches.
    fn assert_within(files: usize, budget: Duration, root: &Path) {
        let builds = if files > 100_000 { 3 } else { 5 };
        let mut times = Vec::new();
        let mut built = None;
        for _ in 0..builds {
            let started = Instant::now();
            let index = FileIndex::build(root, &Cancel::new()).unwrap();
            times.push(started.elapsed());
            built = Some(index);
        }
        let index = built.unwrap();
        ide_core::perf::durations(&format!("ide-project: indexing {files} files"), &times);
        times.sort();
        let took = times[times.len() / 2];
        assert_eq!(index.len(), files + 1);
        assert!(index.files().all(|f| !f.starts_with("build/")));
        assert!(
            took < budget * walk_factor(),
            "{files} files took {took:?}, budget {budget:?}"
        );
        // The best of several runs, so other processes competing for the cores do not fail the bound.
        let queried = (0..5)
            .map(|_| {
                let started = Instant::now();
                let hits = index.query("srcfile", 50, &Cancel::new());
                assert_eq!(hits.len(), 50);
                started.elapsed()
            })
            .min()
            .unwrap_or_default();
        ide_core::perf::durations(&format!("ide-project: querying {files} files, best of 5"), &[queried]);
        assert!(
            queried < budget / 10 * compute_factor(),
            "{files} files queried in {queried:?}"
        );
        eprintln!("{files} files: indexed in {took:?}, queried in {queried:?}");
    }

    #[test]
    #[ignore = "a timing budget (E25), run by make perf on an idle Mac"]
    fn twenty_thousand_files_index_within_a_tenth_of_the_budget_for_two_hundred_thousand() {
        let root = generate("index-20k", 200, 100);
        assert_within(20_000, Duration::from_millis(100), &root);
    }

    #[test]
    #[ignore = "writes 400 000 files; run by make perf"]
    fn two_hundred_thousand_files_index_within_a_second() {
        let root = generate("index-200k", 2_000, 100);
        assert_within(200_000, Duration::from_secs(1), &root);
    }

    #[test]
    fn a_query_ranks_hits_and_reports_where_they_matched() {
        let root = generate("index-query", 50, 20);
        write(&root, "README.md", "");
        let index = FileIndex::build(&root, &Cancel::new()).unwrap();
        let hits = index.query("module7 file12", 5, &Cancel::new());
        assert_eq!(hits[0].path, "src/module7/file12.rs");
        assert_eq!(hits.len(), 5);
        assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
        let readme = index.query("readme", 1, &Cancel::new());
        assert_eq!(
            (readme[0].path.as_str(), readme[0].ranges.clone()),
            ("README.md", one(0..6))
        );
        assert!(index.query("", 10, &Cancel::new()).is_empty());
        let cancelled = Cancel::new();
        cancelled.cancel();
        assert!(index.query("file", 10, &cancelled).is_empty());
        assert_eq!(
            FileIndex::build(&root, &cancelled).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn changes_from_the_watcher_add_and_remove_files_and_whole_folders() {
        let root = generate("index-patch", 3, 3);
        let mut index = FileIndex::build(&root, &Cancel::new()).unwrap();
        write(&root, "src/module0/new.rs", "");
        fs::remove_file(root.join("src/module1/file0.rs")).unwrap();
        fs::remove_dir_all(root.join("src/module2")).unwrap();
        write(&root, "src/fresh/deep/x.rs", "");
        write(&root, "build/module0/ignored.rs", "");
        let changes = Changes {
            dirs: [
                root.join("src/module0"),
                root.join("src/module1"),
                root.join("src"),
                root.join("build/module0"),
            ]
            .into(),
            ..Changes::default()
        };
        index.apply(&WatchEvent::Changed(changes), &Cancel::new()).unwrap();
        assert!(index.contains("src/module0/new.rs"));
        assert!(!index.contains("src/module1/file0.rs"));
        assert!(index.contains("src/module1/file1.rs"));
        assert!(!index.files().any(|f| f.starts_with("src/module2/")));
        assert!(index.contains("src/fresh/deep/x.rs"));
        assert!(!index.files().any(|f| f.starts_with("build/")));
        let mut sorted: Vec<&str> = index.files().collect();
        sorted.sort_unstable();
        assert_eq!(sorted, index.files().collect::<Vec<_>>());
    }

    #[test]
    fn a_folder_moved_in_whole_and_a_changed_ignore_file_are_read_again() {
        let root = generate("index-subtree", 2, 2);
        let mut index = FileIndex::build(&root, &Cancel::new()).unwrap();
        fs::remove_file(root.join("src/module0/file0.rs")).unwrap();
        write(&root, "src/module0/replaced.rs", "");
        let changes = Changes {
            subtrees: [root.join("src/module0")].into(),
            ..Changes::default()
        };
        index.apply(&WatchEvent::Changed(changes), &Cancel::new()).unwrap();
        assert!(index.contains("src/module0/replaced.rs"));
        assert!(!index.contains("src/module0/file0.rs"));

        write(&root, ".gitignore", "build/\nsrc/module1/\n");
        let changes = Changes {
            rules_changed: true,
            ..Changes::default()
        };
        index.apply(&WatchEvent::Changed(changes), &Cancel::new()).unwrap();
        assert!(!index.files().any(|f| f.starts_with("src/module1/")));
    }

    #[test]
    fn quick_open_answers_after_indexing_and_follows_the_watcher() {
        let root = generate("index-service", 5, 5);
        let (indexed, built) = channel();
        let quick = QuickOpen::start(&root, move |i| indexed.send(i).unwrap()).unwrap();
        let (sender, answers) = channel();
        let first = sender.clone();
        quick.query("module3 file4", 3, move |hits| first.send(hits).unwrap());
        let hits = answers.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(hits[0].path, "src/module3/file4.rs");
        assert_eq!(built.recv_timeout(Duration::from_secs(10)).unwrap().files, 26);

        write(&root, "src/module3/brand_new.rs", "");
        quick.changed(&WatchEvent::Changed(Changes {
            dirs: [root.join("src/module3")].into(),
            ..Changes::default()
        }));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let again = sender.clone();
            quick.query("brandnew", 1, move |hits| again.send(hits).unwrap());
            let hits = answers.recv_timeout(Duration::from_secs(10)).unwrap();
            if hits.first().is_some_and(|h| h.path == "src/module3/brand_new.rs") {
                break;
            }
            assert!(Instant::now() < deadline, "the new file never showed up");
        }
    }

    #[test]
    fn a_rescan_reads_everything_again_and_a_folder_gone_takes_its_files_along() {
        let root = generate("index-rescan", 2, 2);
        let mut index = FileIndex::build(&root, &Cancel::new()).unwrap();
        assert_eq!(index.root(), root);
        assert!(!index.is_empty());
        assert!(index.query("file", 0, &Cancel::new()).is_empty(), "no room for any hit");

        write(&root, "src/module1/later.rs", "");
        index
            .apply(&WatchEvent::Rescan(crate::RescanReason::Dropped), &Cancel::new())
            .unwrap();
        assert!(index.contains("src/module1/later.rs"));

        fs::remove_dir_all(root.join("src/module0")).unwrap();
        write(&root, "build/fresh/hidden.rs", "");
        let changes = Changes {
            dirs: [root.join("src/module0")].into(),
            subtrees: [root.join("build/fresh")].into(),
            ..Changes::default()
        };
        index.apply(&WatchEvent::Changed(changes), &Cancel::new()).unwrap();
        assert!(
            !index.files().any(|f| f.starts_with("src/module0/")),
            "{:?}",
            index.files().collect::<Vec<_>>()
        );
        assert!(
            !index.contains("build/fresh/hidden.rs"),
            "an ignored folder that appears stays out"
        );
        assert!(index.contains("src/module1/file0.rs"));

        let empty = scratch("index-empty");
        assert!(FileIndex::build(&empty, &Cancel::new()).unwrap().is_empty());
    }

    #[test]
    fn a_cancelled_update_leaves_the_index_as_it_was() {
        let root = generate("index-cancel", 2, 2);
        let mut index = FileIndex::build(&root, &Cancel::new()).unwrap();
        let before: Vec<String> = index.files().map(str::to_string).collect();
        write(&root, "src/added/one.rs", "");
        let cancelled = Cancel::new();
        cancelled.cancel();
        let changes = Changes {
            subtrees: [root.join("src/added")].into(),
            ..Changes::default()
        };
        let error = index.apply(&WatchEvent::Changed(changes), &cancelled).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(index.files().collect::<Vec<_>>(), before);
    }

    #[test]
    fn a_newer_query_replaces_one_not_yet_answered_and_a_rescan_reindexes() {
        // Enough files that the walk is still running when both queries arrive.
        let root = generate("index-replace", 40, 25);
        let (indexed, built) = channel();
        let quick = QuickOpen::start(&root, move |i| indexed.send(i).unwrap()).unwrap();
        let (sender, answers) = channel();
        let first = sender.clone();
        quick.query("module0", 1, move |_| first.send("first").unwrap());
        let second = sender.clone();
        quick.query("module1", 1, move |_| second.send("second").unwrap());
        assert_eq!(answers.recv_timeout(Duration::from_secs(10)).unwrap(), "second");
        assert!(
            answers.recv_timeout(Duration::from_millis(200)).is_err(),
            "the replaced query got no answer"
        );
        assert_eq!(built.recv_timeout(Duration::from_secs(10)).unwrap().files, 1_001);

        write(&root, "src/module2/rescanned.rs", "");
        quick.changed(&WatchEvent::Rescan(crate::RescanReason::Overflow));
        assert_eq!(
            built.recv_timeout(Duration::from_secs(10)).unwrap().files,
            1_002,
            "a rescan walks the project again"
        );
        let last = sender.clone();
        quick.query("never answered", 1, move |_| last.send("late").unwrap());
        drop(quick);
    }
}
