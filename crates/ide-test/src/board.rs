//! The Tests panel's model (E111), without AppKit: the tests of a folder by tool, package and file with their state,
//! the rows a tree of them shows, with failed ones only when asked, and where each test and failure is in the source
//! for the gutter. A tool that lists tests by building, such as `cargo test`, lists on request only; until then its
//! tests come from the sources, and a whole run that named any test drops those it did not name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Location, Report, Scope, State, Test, TestId, Tool};

/// One test the panel knows, with what its last run said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The tool that runs it.
    pub tool: Tool,
    /// Who and where it is.
    pub test: Test,
    /// What became of it.
    pub state: State,
    /// The failure's or skip's message.
    pub message: Option<String>,
    /// Where the failing assert is.
    pub failure: Option<Location>,
    /// It was found in the sources, and no listing or run named it yet.
    pub guessed: bool,
}

/// What a row of the tree is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    /// A package with its tool.
    Package(Tool, String),
    /// A file of a package.
    File(Tool, String, PathBuf),
    /// A test, by its index in [`Board::entries`].
    Test(usize),
}

/// One row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// How deep it is, 0 for packages.
    pub level: usize,
    /// What it says.
    pub label: String,
    /// Its state, for a package or file the most telling of its tests'.
    pub state: State,
    /// What it stands for.
    pub node: Node,
}

/// The tests of one folder.
#[derive(Debug, Clone, Default)]
pub struct Board {
    entries: Vec<Entry>,
    /// Only failed tests show.
    pub failed_only: bool,
    /// The packages and files shown open; all others are closed.
    pub open: std::collections::HashSet<Node>,
    /// The last run, for running it again.
    pub last_run: Option<(Tool, Scope)>,
    /// The tools running every test, with whether they named one yet.
    sweeping: HashMap<Tool, bool>,
}

impl Board {
    /// Every test known, in the order the tools listed them.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// `tool` listed `tests`: they replace its earlier list, keeping what runs said of the same tests.
    pub fn listed(&mut self, tool: Tool, tests: Vec<Test>) {
        let old: Vec<Entry> = self.entries.iter().filter(|e| e.tool == tool).cloned().collect();
        self.entries.retain(|e| e.tool != tool);
        for test in tests {
            let known = old.iter().find(|e| same(&e.test.id, &test.id));
            self.entries.push(Entry {
                tool,
                state: known.map_or(State::NotRun, |e| e.state),
                message: known.and_then(|e| e.message.clone()),
                failure: known.and_then(|e| e.failure.clone()),
                test: Test {
                    location: test.location.or_else(|| known.and_then(|e| e.test.location.clone())),
                    id: test.id,
                },
                guessed: false,
            });
        }
    }

    /// The sources show `tests` of `tool`: they stand in for its list until a listing or a run comes, and only while
    /// it has none.
    pub fn guessed(&mut self, tool: Tool, tests: Vec<Test>) {
        if self.entries.iter().any(|e| e.tool == tool) {
            return;
        }
        self.entries.extend(tests.into_iter().map(|test| Entry {
            tool,
            test,
            state: State::NotRun,
            message: None,
            failure: None,
            guessed: true,
        }));
    }

    /// A run of `scope` with `tool` started: its tests show as running where the tool reports no starts.
    pub fn started(&mut self, tool: Tool, scope: &Scope) {
        self.last_run = Some((tool, scope.clone()));
        if *scope == Scope::All {
            self.sweeping.insert(tool, false);
        }
        if tool.reports_starts() {
            return;
        }
        for entry in self
            .entries
            .iter_mut()
            .filter(|e| e.tool == tool && in_scope(&e.test.id, scope))
        {
            entry.state = State::Running;
            entry.message = None;
            entry.failure = None;
        }
    }

    /// `tool` reported `reports`: known tests take them, tests seen first in a run join the list and take the place
    /// of a test the sources showed by the same name.
    pub fn reported(&mut self, tool: Tool, reports: Vec<Report>) {
        if !reports.is_empty()
            && let Some(heard) = self.sweeping.get_mut(&tool)
        {
            *heard = true;
        }
        let mut by_name: HashMap<(String, String), Vec<usize>> = HashMap::new();
        for (i, e) in self.entries.iter().enumerate().filter(|(_, e)| e.tool == tool) {
            by_name
                .entry((e.test.id.package.clone(), e.test.id.name.clone()))
                .or_default()
                .push(i);
        }
        let mut dropped = vec![false; self.entries.len()];
        let mut placeholders: Vec<Option<PathBuf>> = Vec::new();
        for report in reports {
            if report.id.name.is_empty() {
                continue;
            }
            let key = (report.id.package.clone(), report.id.name.clone());
            let found = by_name
                .get(&key)
                .and_then(|at| at.iter().copied().find(|i| same(&self.entries[*i].test.id, &report.id)));
            match found {
                Some(i) => {
                    let entry = &mut self.entries[i];
                    entry.state = report.state;
                    entry.message = report.message;
                    entry.failure = report.failure;
                    entry.guessed = false;
                    if report.location.is_some() {
                        entry.test.location = report.location;
                    }
                }
                None => {
                    placeholders.push(report.id.file.clone());
                    let stand_in = self.stand_in(tool, &report.id, &dropped);
                    let location = report
                        .location
                        .or_else(|| stand_in.and_then(|i| self.entries[i].test.location.clone()));
                    if let Some(i) = stand_in {
                        dropped[i] = true;
                    }
                    by_name.entry(key).or_default().push(self.entries.len());
                    dropped.push(false);
                    self.entries.push(Entry {
                        tool,
                        test: Test {
                            id: report.id,
                            location,
                        },
                        state: report.state,
                        message: report.message,
                        failure: report.failure,
                        guessed: false,
                    });
                }
            }
        }
        // A file Jest listed stands for its tests until they are known.
        let mut index = 0;
        self.entries.retain(|e| {
            let keep = !dropped[index]
                && !(e.tool == tool && e.test.id.name.is_empty() && placeholders.contains(&e.test.id.file));
            index += 1;
            keep
        });
    }

    /// The test the sources showed that `id` is, named otherwise by the run: one of `tool` with the same function
    /// name, of the same package where there is one.
    fn stand_in(&self, tool: Tool, id: &TestId, dropped: &[bool]) -> Option<usize> {
        let last = |name: &str| name.rsplit("::").next().unwrap_or(name).to_string();
        let wanted = last(&id.name);
        let candidates: Vec<usize> = (0..self.entries.len())
            .filter(|i| {
                let e = &self.entries[*i];
                e.tool == tool && e.guessed && !dropped[*i] && last(&e.test.id.name) == wanted
            })
            .collect();
        candidates
            .iter()
            .copied()
            .find(|i| self.entries[*i].test.id.package == id.package)
            .or_else(|| candidates.first().copied())
    }

    /// The run of `tool` ended, `complete` where it was not stopped: tests it did not report are no longer running,
    /// and after a whole run that named any test, the tests only the sources showed go.
    pub fn finished(&mut self, tool: Tool, complete: bool) {
        let heard = self.sweeping.remove(&tool).unwrap_or(false);
        if complete && heard {
            self.entries.retain(|e| !(e.tool == tool && e.guessed));
        }
        for entry in self
            .entries
            .iter_mut()
            .filter(|e| e.tool == tool && e.state == State::Running)
        {
            entry.state = State::NotRun;
        }
    }

    /// The rows of the tree: packages, their files where tests name one, and tests; with `failed_only`, only what
    /// failed and the rows above it.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let shown: Vec<usize> = (0..self.entries.len())
            .filter(|i| !self.failed_only || self.entries[*i].state == State::Failed)
            .collect();
        let mut packages: Vec<(Tool, String)> = Vec::new();
        for i in &shown {
            let e = &self.entries[*i];
            if !packages.contains(&(e.tool, e.test.id.package.clone())) {
                packages.push((e.tool, e.test.id.package.clone()));
            }
        }
        for (tool, package) in packages {
            let members: Vec<usize> = shown
                .iter()
                .copied()
                .filter(|i| self.entries[*i].tool == tool && self.entries[*i].test.id.package == package)
                .collect();
            let label = if package.is_empty() {
                tool.name().to_string()
            } else {
                package.clone()
            };
            rows.push(Row {
                level: 0,
                label,
                state: summary(members.iter().map(|i| self.entries[*i].state)),
                node: Node::Package(tool, package.clone()),
            });
            let mut files: Vec<Option<PathBuf>> = Vec::new();
            for i in &members {
                let file = self.entries[*i].test.id.file.clone();
                if !files.contains(&file) {
                    files.push(file);
                }
            }
            for file in files {
                let in_file: Vec<usize> = members
                    .iter()
                    .copied()
                    .filter(|i| self.entries[*i].test.id.file == file)
                    .collect();
                let level = match &file {
                    Some(path) => {
                        rows.push(Row {
                            level: 1,
                            label: path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into()),
                            state: summary(in_file.iter().map(|i| self.entries[*i].state)),
                            node: Node::File(tool, package.clone(), path.clone()),
                        });
                        2
                    }
                    None => 1,
                };
                for i in in_file
                    .into_iter()
                    .filter(|i| !self.entries[*i].test.id.name.is_empty())
                {
                    let name = &self.entries[i].test.id.name;
                    let label = name.rsplit("::").next().unwrap_or(name).to_string();
                    rows.push(Row {
                        level,
                        label: if label.contains(".py") { name.clone() } else { label },
                        state: self.entries[i].state,
                        node: Node::Test(i),
                    });
                }
            }
        }
        rows
    }

    /// Opens the package or file `node`, or closes it.
    pub fn set_open(&mut self, node: Node, open: bool) {
        if open {
            self.open.insert(node);
        } else {
            self.open.remove(&node);
        }
    }

    /// Whether the package or file `node` shows its rows.
    pub fn is_open(&self, node: &Node) -> bool {
        self.failed_only || self.open.contains(node)
    }

    /// The tests whose source is in `path`, by line from 1.
    pub fn tests_in(&self, path: &Path) -> Vec<(u32, &Entry)> {
        self.entries
            .iter()
            .filter_map(|e| {
                let at = e.test.location.as_ref()?;
                (at.path == path).then_some((at.line, e))
            })
            .collect()
    }

    /// The failed asserts in `path`, by line from 1, with their message.
    pub fn failures_in(&self, path: &Path) -> Vec<(u32, String)> {
        self.entries
            .iter()
            .filter(|e| e.state == State::Failed)
            .filter_map(|e| {
                let at = e.failure.as_ref().filter(|f| f.path == path)?;
                Some((at.line, e.message.clone().unwrap_or_else(|| e.test.id.name.clone())))
            })
            .collect()
    }

    /// The test at or around `line` of `path`, from 1: the one whose source starts last at or before it.
    pub fn at(&self, path: &Path, line: u32) -> Option<&Entry> {
        self.tests_in(path)
            .into_iter()
            .filter(|(start, _)| *start <= line)
            .max_by_key(|(start, _)| *start)
            .map(|(_, e)| e)
    }

    /// Gives the tests of `path` the places that its `text` shows for `tool`'s tests, where the tool gave none:
    /// a Rust function under `#[test]`, a Go function `Test…`.
    pub fn locate(&mut self, path: &Path, text: &str) {
        let found = in_source(path, text);
        if found.is_empty() {
            return;
        }
        for entry in &mut self.entries {
            if entry.test.location.is_some() {
                continue;
            }
            let name = &entry.test.id.name;
            let last = name.rsplit("::").next().unwrap_or(name);
            let top = name.split('/').next().unwrap_or(name);
            if let Some((_, line)) = found.iter().find(|(f, _)| f == last || f == top) {
                entry.test.location = Some(Location {
                    path: path.to_path_buf(),
                    line: *line,
                });
            }
        }
    }

    /// The scope that runs the entry at `index`.
    pub fn scope_of(&self, index: usize) -> Option<(Tool, Scope)> {
        let entry = self.entries.get(index)?;
        if entry.test.id.name.is_empty() {
            return Some((entry.tool, Scope::File(entry.test.id.file.clone()?)));
        }
        Some((entry.tool, Scope::Test(entry.test.id.clone())))
    }
}

/// The tests that `text` of `path` defines, by function name and line from 1: Rust functions under `#[test]` or
/// `#[tokio::test]`, Go functions `Test…`; for the lines of tests a tool listed without them.
pub fn in_source(path: &Path, text: &str) -> Vec<(String, u32)> {
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "rs" => crate::source::in_file(text)
            .into_iter()
            .filter_map(|(mut path, line)| Some((path.pop()?, line)))
            .collect(),
        "go" => text
            .lines()
            .enumerate()
            .filter_map(|(n, line)| {
                let name = function_name(line.trim(), "func ").filter(|n| n.starts_with("Test"))?;
                Some((name, u32::try_from(n + 1).unwrap_or(u32::MAX)))
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn function_name(line: &str, keyword: &str) -> Option<String> {
    let rest = line.split(keyword).nth(1)?;
    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some(name)
}

/// The state a package or file shows: failed over running over not run, passed only when all passed or skipped.
fn summary(states: impl Iterator<Item = State>) -> State {
    let states: Vec<State> = states.collect();
    if states.contains(&State::Failed) {
        State::Failed
    } else if states.contains(&State::Running) {
        State::Running
    } else if states.contains(&State::NotRun) || states.is_empty() {
        State::NotRun
    } else if states.iter().all(|s| *s == State::Skipped) {
        State::Skipped
    } else {
        State::Passed
    }
}

/// Whether two identities name one test; a file the run names where the listing did not still matches.
fn same(a: &TestId, b: &TestId) -> bool {
    a.package == b.package && a.name == b.name && (a.file.is_none() || b.file.is_none() || a.file == b.file)
}

fn in_scope(id: &TestId, scope: &Scope) -> bool {
    match scope {
        Scope::All => true,
        Scope::Package(package) => id.package == *package,
        Scope::File(file) => id.file.as_ref() == Some(file),
        Scope::Test(test) => same(id, test),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(package: &str, file: Option<&str>, name: &str) -> Test {
        Test {
            id: TestId {
                package: package.into(),
                file: file.map(PathBuf::from),
                name: name.into(),
            },
            location: None,
        }
    }

    fn report(package: &str, name: &str, state: State) -> Report {
        Report {
            id: TestId {
                package: package.into(),
                file: None,
                name: name.into(),
            },
            state,
            message: None,
            failure: None,
            location: None,
        }
    }

    #[test]
    fn a_run_marks_its_tests_and_the_tree_sums_them_up() {
        let mut board = Board::default();
        board.listed(
            Tool::Cargo,
            vec![
                test("sample", None, "tests::adds"),
                test("sample", None, "tests::fails"),
            ],
        );
        board.started(Tool::Cargo, &Scope::All);
        assert!(board.entries().iter().all(|e| e.state == State::Running));
        board.reported(
            Tool::Cargo,
            vec![
                report("sample", "tests::adds", State::Passed),
                report("sample", "tests::fails", State::Failed),
            ],
        );
        let rows = board.rows();
        assert_eq!(rows[0].label, "sample");
        assert_eq!(rows[0].state, State::Failed);
        assert_eq!(
            rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            ["sample", "adds", "fails"]
        );
        board.failed_only = true;
        assert_eq!(board.rows().len(), 2);
        board.listed(Tool::Cargo, vec![test("sample", None, "tests::adds")]);
        assert_eq!(
            board.entries()[0].state,
            State::Passed,
            "a new listing keeps what runs said"
        );
    }

    #[test]
    fn tests_of_open_files_get_their_line_and_the_cursor_finds_its_test() {
        let mut board = Board::default();
        board.listed(
            Tool::Cargo,
            vec![
                test("sample", None, "tests::adds"),
                test("sample", None, "tests::nested::inner"),
            ],
        );
        let source = "fn add() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn adds() {\n        let x = 1;\n    }\n    mod nested {\n        #[test]\n        fn inner() {}\n    }\n}\n";
        let path = Path::new("/p/src/lib.rs");
        board.locate(path, source);
        let lines: Vec<u32> = board.tests_in(path).iter().map(|(l, _)| *l).collect();
        assert_eq!(lines, [5, 10]);
        assert_eq!(board.at(path, 6).map(|e| e.test.id.name.as_str()), Some("tests::adds"));
        assert!(board.at(path, 2).is_none());
        let go = in_source(
            Path::new("/p/a_test.go"),
            "package a\n\nfunc TestOne(t *testing.T) {}\nfunc helper() {}\n",
        );
        assert_eq!(go, [("TestOne".to_string(), 3)]);
    }

    fn located(package: &str, name: &str, line: u32) -> Test {
        let mut found = test(package, None, name);
        found.location = Some(Location {
            path: "/p/src/lib.rs".into(),
            line,
        });
        found
    }

    #[test]
    fn tests_from_the_sources_show_until_a_run_names_the_real_ones() {
        let mut board = Board::default();
        board.guessed(
            Tool::Cargo,
            vec![
                located("sample", "tests::adds", 5),
                located("sample", "tests::wrong_guess", 9),
                located("sample", "helpers::works", 20),
            ],
        );
        assert_eq!(board.rows().len(), 4, "the guessed tests show before any run");
        assert!(
            board.at(Path::new("/p/src/lib.rs"), 6).is_some(),
            "and give the gutter its lines"
        );
        board.started(Tool::Cargo, &Scope::All);
        board.reported(
            Tool::Cargo,
            vec![
                report("sample", "tests::adds", State::Passed),
                report("sample", "inner::helpers::works", State::Failed),
            ],
        );
        let works = board
            .entries()
            .iter()
            .find(|e| e.test.id.name.ends_with("works"))
            .unwrap();
        assert_eq!(
            works.test.id.name, "inner::helpers::works",
            "the run's name replaces the guess"
        );
        assert_eq!(works.test.location.as_ref().map(|l| l.line), Some(20));
        assert_eq!(board.entries().len(), 3);
        board.finished(Tool::Cargo, true);
        let names: Vec<&str> = board.entries().iter().map(|e| e.test.id.name.as_str()).collect();
        assert_eq!(
            names,
            ["tests::adds", "inner::helpers::works"],
            "a whole run drops what it did not name"
        );
        board.guessed(Tool::Cargo, vec![located("sample", "tests::later", 30)]);
        assert_eq!(
            board.entries().len(),
            2,
            "the sources no longer stand in once a run named tests"
        );
        let mut stopped = Board::default();
        stopped.guessed(Tool::Nextest, vec![located("sample", "tests::adds", 5)]);
        stopped.started(Tool::Nextest, &Scope::All);
        stopped.reported(Tool::Nextest, vec![report("sample", "tests::other", State::Running)]);
        stopped.finished(Tool::Nextest, false);
        assert_eq!(
            stopped.entries().len(),
            2,
            "a stopped run keeps the tests it did not reach"
        );
    }

    #[test]
    fn subtests_and_jest_files_come_from_the_run() {
        let mut board = Board::default();
        board.listed(Tool::Jest, vec![test("", Some("/p/a.test.js"), "")]);
        let mut found = report("", "math > adds", State::Passed);
        found.id.file = Some(PathBuf::from("/p/a.test.js"));
        board.reported(Tool::Jest, vec![found]);
        assert_eq!(board.entries().len(), 1);
        assert_eq!(board.entries()[0].test.id.name, "math > adds");
        board.reported(Tool::Go, vec![report("m", "TestTable/case_one", State::Failed)]);
        let mut failed = board.entries()[1].clone();
        failed.failure = Some(Location {
            path: "/p/x_test.go".into(),
            line: 9,
        });
        assert_eq!(board.scope_of(1), Some((Tool::Go, Scope::Test(failed.test.id.clone()))));
    }
}
