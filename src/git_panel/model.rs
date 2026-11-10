//! What the Git panel lists: staged files, other changes, the worktrees of threads and recent commits, each row named
//! by a key that survives a refresh.

use std::path::{Path, PathBuf};

use ide_git::{Commit, EntryKind, FileState, Status, StatusEntry};
use splat_i18n::t;

use crate::worktrees::TreeState;

/// A group of rows in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// Files with merge conflicts.
    Conflicts,
    /// What the next commit takes.
    Staged,
    /// Changes not staged and untracked files.
    Changes,
    /// Threads that work in worktrees of their own (I1).
    Worktrees,
    /// The latest commits of HEAD.
    Commits,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Conflicts,
        Section::Staged,
        Section::Changes,
        Section::Worktrees,
        Section::Commits,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::Conflicts => t("Konflikte"),
            Section::Staged => t("Vorgemerkt"),
            Section::Changes => t("Änderungen"),
            Section::Worktrees => t("Worktree-Threads"),
            Section::Commits => t("Letzte Commits"),
        }
    }

    fn word(self) -> &'static str {
        match self {
            Section::Conflicts => "conflicts",
            Section::Staged => "staged",
            Section::Changes => "changes",
            Section::Worktrees => "worktrees",
            Section::Commits => "commits",
        }
    }

    fn from_word(word: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|s| s.word() == word)
    }
}

/// What happened to a file, shown as a letter and a colour and said in words to VoiceOver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    Conflict,
    Untracked,
}

impl Mark {
    fn of(state: FileState) -> Option<Mark> {
        Some(match state {
            FileState::Unmodified => return None,
            FileState::Modified => Mark::Modified,
            FileState::TypeChanged => Mark::TypeChanged,
            FileState::Added => Mark::Added,
            FileState::Deleted => Mark::Deleted,
            FileState::Renamed => Mark::Renamed,
            FileState::Copied => Mark::Copied,
            FileState::Unmerged => Mark::Conflict,
        })
    }

    pub fn letter(self) -> &'static str {
        match self {
            Mark::Modified => "M",
            Mark::TypeChanged => "T",
            Mark::Added => "A",
            Mark::Deleted => "D",
            Mark::Renamed => "R",
            Mark::Copied => "C",
            Mark::Conflict => "U",
            Mark::Untracked => "?",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Mark::Modified => t("geändert"),
            Mark::TypeChanged => t("Typ geändert"),
            Mark::Added => t("neu"),
            Mark::Deleted => t("gelöscht"),
            Mark::Renamed => t("umbenannt"),
            Mark::Copied => t("kopiert"),
            Mark::Conflict => t("Konflikt"),
            Mark::Untracked => t("nicht versioniert"),
        }
    }
}

/// A file in one section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub entry: StatusEntry,
    pub mark: Mark,
}

impl FileRow {
    pub fn path(&self) -> &Path {
        &self.entry.path
    }

    /// The path it had before a rename or copy in the index.
    pub fn from(&self) -> Option<&Path> {
        match &self.entry.kind {
            EntryKind::Renamed { from, .. } | EntryKind::Copied { from, .. } => Some(from),
            _ => None,
        }
    }

    pub fn untracked(&self) -> bool {
        self.mark == Mark::Untracked
    }
}

/// The rows of the list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Model {
    pub conflicts: Vec<FileRow>,
    pub staged: Vec<FileRow>,
    pub changes: Vec<FileRow>,
    pub worktrees: Vec<TreeState>,
    pub commits: Vec<Commit>,
}

impl Model {
    pub fn new(status: &Status, commits: Vec<Commit>, worktrees: Vec<TreeState>) -> Model {
        let mut model = Model {
            commits,
            worktrees,
            ..Model::default()
        };
        for entry in &status.entries {
            match &entry.kind {
                EntryKind::Ignored => {}
                EntryKind::Untracked => model.changes.push(FileRow {
                    entry: entry.clone(),
                    mark: Mark::Untracked,
                }),
                EntryKind::Unmerged(_) => model.conflicts.push(FileRow {
                    entry: entry.clone(),
                    mark: Mark::Conflict,
                }),
                _ => {
                    if let Some(mark) = Mark::of(entry.index) {
                        model.staged.push(FileRow {
                            entry: entry.clone(),
                            mark,
                        });
                    }
                    if let Some(mark) = Mark::of(entry.worktree) {
                        model.changes.push(FileRow {
                            entry: entry.clone(),
                            mark,
                        });
                    }
                }
            }
        }
        model.conflicts.sort_by(|a, b| a.path().cmp(b.path()));
        model.staged.sort_by(|a, b| a.path().cmp(b.path()));
        model.changes.sort_by(|a, b| a.path().cmp(b.path()));
        model
    }

    /// The sections that show: conflicts and staged files only when there are some, commits only after the first.
    pub fn sections(&self) -> Vec<Section> {
        Section::ALL
            .into_iter()
            .filter(|s| match s {
                Section::Conflicts => !self.conflicts.is_empty(),
                Section::Staged => !self.staged.is_empty(),
                Section::Changes => true,
                Section::Worktrees => !self.worktrees.is_empty(),
                Section::Commits => !self.commits.is_empty(),
            })
            .collect()
    }

    pub fn files(&self, section: Section) -> &[FileRow] {
        match section {
            Section::Conflicts => &self.conflicts,
            Section::Staged => &self.staged,
            Section::Changes => &self.changes,
            Section::Worktrees | Section::Commits => &[],
        }
    }

    pub fn len(&self, section: Section) -> usize {
        match section {
            Section::Commits => self.commits.len(),
            Section::Worktrees => self.worktrees.len(),
            other => self.files(other).len(),
        }
    }

    /// The keys of the rows in `section`.
    pub fn keys(&self, section: Section) -> Vec<RowKey> {
        match section {
            Section::Commits => self.commits.iter().map(|c| RowKey::Commit(c.hash.clone())).collect(),
            Section::Worktrees => self
                .worktrees
                .iter()
                .map(|w| RowKey::Worktree(w.tree.path.clone()))
                .collect(),
            other => self
                .files(other)
                .iter()
                .map(|f| RowKey::File(other, f.path().to_path_buf()))
                .collect(),
        }
    }

    pub fn file(&self, section: Section, path: &Path) -> Option<&FileRow> {
        self.files(section).iter().find(|f| f.path() == path)
    }

    pub fn commit(&self, hash: &str) -> Option<&Commit> {
        self.commits.iter().find(|c| c.hash == hash)
    }

    /// The thread worktree at `path`.
    pub fn tree(&self, path: &Path) -> Option<&TreeState> {
        self.worktrees.iter().find(|w| w.tree.path == path)
    }

    /// The files the worktree at `path` changed, as rows below it.
    pub fn tree_keys(&self, path: &Path) -> Vec<RowKey> {
        self.tree(path).map_or_else(Vec::new, |w| {
            w.changed
                .iter()
                .map(|f| RowKey::WorktreeFile(path.to_path_buf(), f.path.clone()))
                .collect()
        })
    }

    /// How many files differ from HEAD in the index or the working tree, for the badge.
    pub fn changed_files(&self) -> usize {
        let mut paths: Vec<&Path> = self
            .conflicts
            .iter()
            .chain(&self.staged)
            .chain(&self.changes)
            .map(FileRow::path)
            .collect();
        paths.sort();
        paths.dedup();
        paths.len()
    }
}

/// Names a row across refreshes, so the list keeps what was open and selected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowKey {
    Section(Section),
    File(Section, PathBuf),
    Commit(String),
    /// A thread's worktree, by its folder.
    Worktree(PathBuf),
    /// A file a thread's worktree changed: the worktree's folder, the file relative to it.
    WorktreeFile(PathBuf, PathBuf),
}

impl RowKey {
    pub fn encode(&self) -> String {
        match self {
            RowKey::Section(section) => format!("section:{}", section.word()),
            RowKey::File(section, path) => format!("{}:{}", section.word(), path.display()),
            RowKey::Commit(hash) => format!("commit:{hash}"),
            RowKey::Worktree(path) => format!("tree:{}", path.display()),
            RowKey::WorktreeFile(tree, file) => format!("treefile:{}\0{}", tree.display(), file.display()),
        }
    }

    pub fn decode(text: &str) -> Option<RowKey> {
        let (kind, rest) = text.split_once(':')?;
        match kind {
            "section" => Section::from_word(rest).map(RowKey::Section),
            "commit" => Some(RowKey::Commit(rest.to_string())),
            "tree" => Some(RowKey::Worktree(PathBuf::from(rest))),
            "treefile" => {
                let (tree, file) = rest.split_once('\0')?;
                Some(RowKey::WorktreeFile(PathBuf::from(tree), PathBuf::from(file)))
            }
            other => Some(RowKey::File(Section::from_word(other)?, PathBuf::from(rest))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_git::{BranchStatus, Conflict};

    fn entry(path: &str, kind: EntryKind, index: FileState, worktree: FileState) -> StatusEntry {
        StatusEntry {
            path: PathBuf::from(path),
            kind,
            index,
            worktree,
            submodule: None,
        }
    }

    fn status(entries: Vec<StatusEntry>) -> Status {
        Status {
            branch: BranchStatus::default(),
            stash: 0,
            entries,
        }
    }

    #[test]
    fn a_file_changed_in_both_places_shows_in_both_sections_and_counts_once() {
        let model = Model::new(
            &status(vec![
                entry("b.rs", EntryKind::Ordinary, FileState::Modified, FileState::Modified),
                entry("a.rs", EntryKind::Ordinary, FileState::Added, FileState::Unmodified),
                entry(
                    "new.txt",
                    EntryKind::Untracked,
                    FileState::Unmodified,
                    FileState::Unmodified,
                ),
                entry(
                    "both.rs",
                    EntryKind::Unmerged(Conflict::BothModified),
                    FileState::Unmerged,
                    FileState::Unmerged,
                ),
                entry(
                    "target",
                    EntryKind::Ignored,
                    FileState::Unmodified,
                    FileState::Unmodified,
                ),
            ]),
            vec![],
            vec![],
        );
        let staged: Vec<_> = model.staged.iter().map(|f| (f.path().to_path_buf(), f.mark)).collect();
        assert_eq!(staged, [("a.rs".into(), Mark::Added), ("b.rs".into(), Mark::Modified)]);
        let changes: Vec<_> = model.changes.iter().map(|f| f.mark.letter()).collect();
        assert_eq!(changes, ["M", "?"]);
        assert_eq!(model.conflicts.len(), 1);
        assert_eq!(model.conflicts[0].mark.letter(), "U");
        assert_eq!(model.changed_files(), 4);
        assert_eq!(
            model.sections(),
            [Section::Conflicts, Section::Staged, Section::Changes]
        );
    }

    #[test]
    fn keys_survive_their_text_form_also_with_colons_in_paths() {
        for key in [
            RowKey::Section(Section::Commits),
            RowKey::File(Section::Staged, PathBuf::from("a:b/c d.rs")),
            RowKey::Commit("abc123".into()),
            RowKey::Worktree(PathBuf::from("/data/worktrees/app-1/fix: it")),
            RowKey::WorktreeFile(PathBuf::from("/data/w/fix"), PathBuf::from("src/a:b.rs")),
        ] {
            assert_eq!(RowKey::decode(&key.encode()), Some(key));
        }
        assert_eq!(RowKey::decode("nonsense"), None);
    }
}
