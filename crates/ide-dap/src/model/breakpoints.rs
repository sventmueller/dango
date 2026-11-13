//! The breakpoints of a project: set and cleared by line, changed through the context menu, following the edits of
//! their file, and what an adapter is told about them.

use std::path::{Path, PathBuf};

use ide_core::debugging::Breakpoint;
use ide_document::Change;

/// How a breakpoint shows in the gutter (E101: never by colour alone).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Filled: sessions stop there.
    Enabled,
    /// Outlined: kept, but sessions pass, because it or all breakpoints are switched off.
    Disabled,
    /// A log point, which writes instead of stopping: a filled diamond.
    Log,
    /// Has a condition or a hit count: filled, with a dot.
    Conditional,
    /// On, but the running session's adapter could not set it, as on a line without code: grey.
    Unverified,
}

/// What a session's adapter made of a breakpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// The file.
    pub path: PathBuf,
    /// The line, from 0.
    pub line: usize,
    /// The adapter's number for it, which its later changes refer to.
    pub id: Option<i64>,
    /// Whether the program stops there.
    pub verified: bool,
    /// Why not, in the adapter's words.
    pub message: Option<String>,
}

/// What the context menu of a breakpoint changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Turn it on or off.
    Enable(bool),
    /// Set or clear the condition it stops on.
    Condition(Option<String>),
    /// Set or clear how many hits it waits for.
    HitCondition(Option<String>),
    /// Set or clear the message that makes it a logpoint.
    LogMessage(Option<String>),
}

/// The breakpoints of every file of a project, kept in the order of path and line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Breakpoints {
    list: Vec<Breakpoint>,
    /// Whether they are on at all (⌘Y), which keeps each one's own state.
    pub active: bool,
    /// What the adapters of running sessions made of them; empty without a session.
    checks: Vec<Check>,
}

impl Breakpoints {
    /// Breakpoints from what the project kept.
    pub fn new(list: Vec<Breakpoint>, active: bool) -> Breakpoints {
        let mut breakpoints = Breakpoints {
            list,
            active,
            checks: Vec::new(),
        };
        breakpoints.tidy();
        breakpoints
    }

    /// Every breakpoint, by path and line.
    pub fn all(&self) -> &[Breakpoint] {
        &self.list
    }

    /// Whether there is none.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The breakpoint at `line` of `path`.
    pub fn at(&self, path: &Path, line: usize) -> Option<&Breakpoint> {
        self.list.iter().find(|b| b.path == path && b.line == line)
    }

    /// The breakpoints of `path`, by line.
    pub fn of(&self, path: &Path) -> impl Iterator<Item = &Breakpoint> {
        self.list.iter().filter(move |b| b.path == path)
    }

    /// The files that have breakpoints.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self.list.iter().map(|b| b.path.clone()).collect();
        files.dedup();
        files
    }

    /// Sets a breakpoint at `line` of `path`, or clears the one there, as a click in the gutter and ⌘\ do; whether one
    /// is there now.
    pub fn toggle(&mut self, path: &Path, line: usize) -> bool {
        self.checks.retain(|c| !(c.path == path && c.line == line));
        if let Some(index) = self.index(path, line) {
            self.list.remove(index);
            return false;
        }
        self.list.push(Breakpoint::new(path, line));
        self.tidy();
        true
    }

    /// Changes the breakpoint at `line` of `path`; whether there was one.
    pub fn change(&mut self, path: &Path, line: usize, edit: Edit) -> bool {
        let Some(index) = self.index(path, line) else {
            return false;
        };
        let breakpoint = &mut self.list[index];
        let words = |text: Option<String>| text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        match edit {
            Edit::Enable(on) => breakpoint.enabled = on,
            Edit::Condition(text) => breakpoint.condition = words(text),
            Edit::HitCondition(text) => breakpoint.hit_condition = words(text),
            Edit::LogMessage(text) => breakpoint.log_message = words(text),
        }
        true
    }

    /// Clears every breakpoint of `path`, or of every file with `None`.
    pub fn clear(&mut self, path: Option<&Path>) {
        self.list.retain(|b| path.is_some_and(|path| b.path != path));
    }

    /// How the breakpoint at `line` of `path` shows, if there is one.
    pub fn look(&self, path: &Path, line: usize) -> Option<Look> {
        let breakpoint = self.at(path, line)?;
        Some(if !breakpoint.enabled || !self.active {
            Look::Disabled
        } else if self.check(path, line).is_some_and(|c| !c.verified) {
            Look::Unverified
        } else if breakpoint.is_log_point() {
            Look::Log
        } else if breakpoint.condition.is_some() || breakpoint.hit_condition.is_some() {
            Look::Conditional
        } else {
            Look::Enabled
        })
    }

    /// The breakpoints an adapter gets for `path`: none while they are switched off, and only the enabled ones.
    pub fn for_adapter(&self, path: &Path) -> Vec<Breakpoint> {
        if !self.active {
            return Vec::new();
        }
        self.of(path).filter(|b| b.enabled).cloned().collect()
    }

    /// What the adapter made of the breakpoint at `line` of `path`, if a session set it.
    pub fn check(&self, path: &Path, line: usize) -> Option<&Check> {
        self.checks.iter().find(|c| c.path == path && c.line == line)
    }

    /// The adapter answered the breakpoints of `path` asked for at `asked`, lines from 1, in that order. One it
    /// verified on another line moves there unless that line has one; whether any moved.
    pub fn answered(&mut self, path: &Path, asked: &[u32], answers: &[crate::Breakpoint]) -> bool {
        self.checks.retain(|c| c.path != path);
        let mut moved = false;
        for (asked, answer) in asked.iter().zip(answers) {
            let Some(mut line) = (*asked as usize).checked_sub(1) else {
                continue;
            };
            if self.index(path, line).is_none() {
                continue;
            }
            let actual = answer.line.and_then(|l| (l as usize).checked_sub(1));
            if let Some(actual) = actual.filter(|a| answer.verified && *a != line)
                && self.index(path, actual).is_none()
                && let Some(index) = self.index(path, line)
            {
                self.list[index].line = actual;
                line = actual;
                moved = true;
            }
            self.checks.push(Check {
                path: path.to_path_buf(),
                line,
                id: answer.id,
                verified: answer.verified,
                message: answer.message.clone(),
            });
        }
        if moved {
            self.tidy();
        }
        moved
    }

    /// The adapter changed the breakpoint it numbers `answer.id` on its own, such as once a library loaded;
    /// whether that changes how it looks.
    pub fn changed(&mut self, answer: &crate::Breakpoint) -> bool {
        let Some(check) = self
            .checks
            .iter_mut()
            .find(|c| answer.id.is_some() && c.id == answer.id)
        else {
            return false;
        };
        let before = (check.verified, check.message.clone());
        check.verified = answer.verified;
        check.message.clone_from(&answer.message);
        before != (check.verified, check.message.clone())
    }

    /// No session runs any more, so no adapter says anything about them.
    pub fn forget_checks(&mut self) {
        self.checks.clear();
    }

    /// Moves the breakpoints of `path` with `change`, an edit of its text: lines inserted above push them down, lines
    /// removed above pull them up, and one on a line the edit removed goes to the line the edit leaves there. Whether
    /// any moved.
    pub fn shift(&mut self, path: &Path, change: &Change) -> bool {
        let (start, old_end, new_end) = (change.start.line, change.old_end.line, change.new_end.line);
        if start == old_end && start == new_end {
            return false;
        }
        // An insertion at the very start of a line pushes that line down with it.
        let pushes_start = change.start.utf16 == 0 && old_end == start && change.text.ends_with('\n');
        self.checks.retain(|c| c.path != path);
        let mut moved = false;
        for breakpoint in self.list.iter_mut().filter(|b| b.path == path) {
            let line = breakpoint.line;
            let new = if line > old_end || (line == start && pushes_start) {
                line - old_end + new_end
            } else if line > start {
                line.min(new_end)
            } else {
                line
            };
            if new != line {
                breakpoint.line = new;
                moved = true;
            }
        }
        if moved {
            self.tidy();
        }
        moved
    }

    fn index(&self, path: &Path, line: usize) -> Option<usize> {
        self.list.iter().position(|b| b.path == path && b.line == line)
    }

    /// Orders by file and line; of two that an edit put on one line, the first stays.
    fn tidy(&mut self) {
        self.list.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        self.list
            .dedup_by(|later, earlier| later.path == earlier.path && later.line == earlier.line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_document::{Document, Edit as TextEdit};

    const FILE: &str = "/p/src/main.rs";

    fn path() -> PathBuf {
        PathBuf::from(FILE)
    }

    /// The change `edit` makes to `text`.
    fn change(text: &str, at: usize, remove: usize, insert: &str) -> Change {
        let mut document = Document::untitled(text);
        document
            .apply(TextEdit {
                range_utf16: at..at + remove,
                text: insert.into(),
            })
            .unwrap()
    }

    fn lines(breakpoints: &Breakpoints) -> Vec<usize> {
        breakpoints.of(&path()).map(|b| b.line).collect()
    }

    fn answer(id: i64, verified: bool, line: Option<u32>) -> crate::Breakpoint {
        crate::Breakpoint {
            id: Some(id),
            verified,
            message: (!verified).then(|| "no code".into()),
            line,
            ..crate::Breakpoint::default()
        }
    }

    #[test]
    fn what_the_adapter_could_not_set_looks_unverified_until_no_session_runs() {
        let mut breakpoints = Breakpoints::new(Vec::new(), true);
        for line in [4, 14, 20] {
            breakpoints.toggle(&path(), line);
        }
        assert_eq!(
            breakpoints.look(&path(), 4),
            Some(Look::Enabled),
            "before a session nothing is known"
        );
        let answers = [
            answer(1, false, None),
            answer(2, false, Some(15)),
            answer(3, true, Some(21)),
        ];
        assert!(!breakpoints.answered(&path(), &[5, 15, 21], &answers));
        assert_eq!(breakpoints.look(&path(), 4), Some(Look::Unverified));
        assert_eq!(breakpoints.look(&path(), 14), Some(Look::Unverified));
        assert_eq!(breakpoints.look(&path(), 20), Some(Look::Enabled));
        assert_eq!(
            breakpoints.check(&path(), 4).and_then(|c| c.message.as_deref()),
            Some("no code")
        );
        assert!(breakpoints.changed(&answer(1, true, Some(5))));
        assert_eq!(breakpoints.look(&path(), 4), Some(Look::Enabled));
        breakpoints.change(&path(), 14, Edit::Enable(false));
        assert_eq!(breakpoints.look(&path(), 14), Some(Look::Disabled));
        breakpoints.forget_checks();
        breakpoints.change(&path(), 14, Edit::Enable(true));
        assert_eq!(breakpoints.look(&path(), 14), Some(Look::Enabled));
    }

    #[test]
    fn a_breakpoint_the_adapter_set_on_another_line_moves_there_unless_one_is_there() {
        let mut breakpoints = Breakpoints::new(Vec::new(), true);
        for line in [4, 6] {
            breakpoints.toggle(&path(), line);
        }
        let answers = [answer(1, true, Some(7)), answer(2, true, Some(9))];
        assert!(breakpoints.answered(&path(), &[5, 7], &answers));
        assert_eq!(lines(&breakpoints), [4, 8], "line 6 had one, so the first stays");
        assert!(breakpoints.check(&path(), 8).is_some_and(|c| c.verified));
    }

    #[test]
    fn a_click_sets_a_breakpoint_and_the_next_clears_it() {
        let mut breakpoints = Breakpoints::new(vec![], true);
        assert!(breakpoints.toggle(&path(), 4));
        assert!(breakpoints.toggle(&path(), 1));
        assert_eq!(lines(&breakpoints), [1, 4]);
        assert_eq!(breakpoints.look(&path(), 4), Some(Look::Enabled));
        assert!(!breakpoints.toggle(&path(), 4));
        assert_eq!(lines(&breakpoints), [1]);
        assert_eq!(breakpoints.look(&path(), 4), None);
    }

    #[test]
    fn lines_typed_above_push_a_breakpoint_down_and_removed_ones_pull_it_up() {
        let text = "a\nb\nc\nd\ne\n";
        let mut breakpoints = Breakpoints::new(vec![Breakpoint::new(FILE, 3), Breakpoint::new(FILE, 0)], true);
        assert!(breakpoints.shift(&path(), &change(text, 2, 0, "x\ny\n")));
        assert_eq!(lines(&breakpoints), [0, 5], "two lines in front of line 1");
        let text = "a\nx\ny\nb\nc\nd\ne\n";
        assert!(breakpoints.shift(&path(), &change(text, 2, 4, "")));
        assert_eq!(lines(&breakpoints), [0, 3], "the two lines removed again");
        assert!(
            !breakpoints.shift(&path(), &change(text, 1, 0, "zz")),
            "typing within a line moves nothing"
        );
    }

    #[test]
    fn a_return_at_the_start_of_its_line_takes_the_breakpoint_along() {
        let text = "fn main() {\n    run();\n}\n";
        let mut breakpoints = Breakpoints::new(vec![Breakpoint::new(FILE, 1)], true);
        breakpoints.shift(&path(), &change(text, 12, 0, "\n"));
        assert_eq!(lines(&breakpoints), [2]);
        let text = "fn main() {\n\n    run();\n}\n";
        breakpoints.shift(&path(), &change(text, 17, 0, "\n"));
        assert_eq!(lines(&breakpoints), [2], "a return in the middle of the line leaves it");
    }

    #[test]
    fn a_breakpoint_on_removed_lines_lands_where_they_were_and_two_become_one() {
        let text = "a\nb\nc\nd\n";
        let mut breakpoints = Breakpoints::new(vec![Breakpoint::new(FILE, 1), Breakpoint::new(FILE, 2)], true);
        breakpoints.shift(&path(), &change(text, 1, 4, ""));
        assert_eq!(lines(&breakpoints), [0], "lines b and c joined into a");
        assert_eq!(breakpoints.all().len(), 1);
    }

    #[test]
    fn other_files_stay_where_they_are() {
        let other = Breakpoint::new("/p/src/lib.rs", 5);
        let mut breakpoints = Breakpoints::new(vec![other.clone()], true);
        breakpoints.shift(&path(), &change("a\nb\n", 0, 0, "x\n"));
        assert_eq!(breakpoints.all(), [other]);
    }

    #[test]
    fn switched_off_breakpoints_stay_but_reach_no_adapter() {
        let mut breakpoints = Breakpoints::new(vec![Breakpoint::new(FILE, 1), Breakpoint::new(FILE, 7)], true);
        breakpoints.change(&path(), 7, Edit::Enable(false));
        assert_eq!(breakpoints.look(&path(), 7), Some(Look::Disabled));
        assert_eq!(breakpoints.for_adapter(&path()).len(), 1);
        breakpoints.active = false;
        assert_eq!(breakpoints.look(&path(), 1), Some(Look::Disabled));
        assert!(breakpoints.for_adapter(&path()).is_empty());
        assert_eq!(breakpoints.all().len(), 2);
    }

    #[test]
    fn conditions_and_messages_change_the_look_and_blank_ones_go() {
        let mut breakpoints = Breakpoints::new(vec![Breakpoint::new(FILE, 2)], true);
        breakpoints.change(&path(), 2, Edit::Condition(Some(" n > 3 ".into())));
        assert_eq!(
            breakpoints.at(&path(), 2).and_then(|b| b.condition.clone()),
            Some("n > 3".into())
        );
        assert_eq!(breakpoints.look(&path(), 2), Some(Look::Conditional));
        breakpoints.change(&path(), 2, Edit::LogMessage(Some("n = {n}".into())));
        assert_eq!(breakpoints.look(&path(), 2), Some(Look::Log));
        breakpoints.change(&path(), 2, Edit::LogMessage(Some("  ".into())));
        breakpoints.change(&path(), 2, Edit::Condition(None));
        assert_eq!(breakpoints.look(&path(), 2), Some(Look::Enabled));
        assert!(!breakpoints.change(&path(), 9, Edit::Enable(false)));
    }

    #[test]
    fn clearing_a_file_or_all() {
        let mut breakpoints = Breakpoints::new(
            vec![Breakpoint::new(FILE, 1), Breakpoint::new("/p/src/lib.rs", 2)],
            true,
        );
        assert_eq!(breakpoints.files(), [PathBuf::from("/p/src/lib.rs"), path()]);
        breakpoints.clear(Some(&path()));
        assert_eq!(breakpoints.all().len(), 1);
        breakpoints.clear(None);
        assert!(breakpoints.is_empty());
    }
}
