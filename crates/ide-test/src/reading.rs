//! A run's output read as it grows: each line once, so a poll parses only what came since the last one, and names
//! the reports that changed since then.

use std::path::{Path, PathBuf};

use crate::{Report, Tool, bun, cargo, go, js, nextest, pytest};

/// The places in a parser's reports that changed since they were last taken.
#[derive(Debug, Default)]
pub(crate) struct Changes {
    marked: Vec<bool>,
    list: Vec<usize>,
}

impl Changes {
    pub(crate) fn mark(&mut self, index: usize) {
        if self.marked.len() <= index {
            self.marked.resize(index + 1, false);
        }
        if !self.marked[index] {
            self.marked[index] = true;
            self.list.push(index);
        }
    }

    fn take(&mut self) -> Vec<usize> {
        for index in &self.list {
            self.marked[*index] = false;
        }
        let mut list = std::mem::take(&mut self.list);
        list.sort_unstable();
        list
    }
}

#[derive(Debug)]
enum Parser {
    Cargo(cargo::Lines),
    Nextest(nextest::Lines),
    Go(go::Lines),
    Pytest(pytest::Lines),
    Js(js::Lines),
    Bun(bun::Lines),
}

/// What a run of a tool printed so far, or wrote to its results file, read line by line.
#[derive(Debug)]
pub struct Reading {
    tool: Tool,
    root: PathBuf,
    parser: Parser,
    /// The start of a line whose end has not come yet.
    rest: String,
    out: Vec<Report>,
    changes: Changes,
}

impl Reading {
    /// The reading of a run of `tool` in the project at `root`, before anything came; Go's reads `go.mod`.
    pub fn new(tool: Tool, root: &Path) -> Reading {
        let parser = match tool {
            Tool::Cargo => Parser::Cargo(cargo::Lines::default()),
            Tool::Nextest => Parser::Nextest(nextest::Lines::default()),
            Tool::Go => Parser::Go(go::Lines::new(root)),
            Tool::Pytest => Parser::Pytest(pytest::Lines::default()),
            Tool::Vitest | Tool::Jest => Parser::Js(js::Lines::default()),
            Tool::Bun => Parser::Bun(bun::Lines::default()),
        };
        Reading {
            tool,
            root: root.to_path_buf(),
            parser,
            rest: String::new(),
            out: Vec::new(),
            changes: Changes::default(),
        }
    }

    /// Takes `text`, which follows what came before.
    pub fn feed(&mut self, text: &str) {
        self.rest.push_str(text);
        let Some(end) = self.rest.rfind('\n') else { return };
        let complete: String = self.rest.drain(..=end).collect();
        for line in complete.lines() {
            self.line(line);
        }
    }

    /// The text ended: a last line without a line break counts too.
    pub fn end(&mut self) {
        let rest = std::mem::take(&mut self.rest);
        if !rest.is_empty() {
            self.line(&rest);
        }
    }

    fn line(&mut self, line: &str) {
        let (root, out, changes) = (&self.root, &mut self.out, &mut self.changes);
        match &mut self.parser {
            Parser::Cargo(lines) => lines.line(root, line, out, changes),
            Parser::Nextest(lines) => lines.line(root, line, out, changes),
            Parser::Go(lines) => lines.line(root, line, out, changes),
            Parser::Pytest(lines) => lines.line(root, line, out, changes),
            Parser::Js(lines) => lines.line(line, out, changes),
            Parser::Bun(lines) => lines.line(root, line, out, changes),
        }
    }

    fn shown(&self, report: &Report) -> Report {
        match self.tool {
            Tool::Go => go::shown(report),
            _ => report.clone(),
        }
    }

    /// The reports that changed since the last call, each with its place among [`Reading::reports`].
    pub fn changed(&mut self) -> Vec<(usize, Report)> {
        self.changes
            .take()
            .into_iter()
            .map(|index| (index, self.shown(&self.out[index])))
            .collect()
    }

    /// Every report so far, in the order the tests first appeared.
    pub(crate) fn reports(&self) -> Vec<Report> {
        self.out.iter().map(|r| self.shown(r)).collect()
    }
}
