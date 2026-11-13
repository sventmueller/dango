//! A debug console: what the program and the adapter wrote, the expressions the user evaluated with their answers, and
//! whether its session is over, after which it only shows its history (Review C18).

use ide_core::debugging::{ConsoleKind, ConsoleLine};

/// How many lines a console keeps; the oldest go first.
pub const LIMIT: usize = 5_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
/// The debug console of a session: its output and the expressions evaluated in it.
pub struct Console {
    lines: Vec<ConsoleLine>,
    /// Whether its session ended, or it came back after a restart; it takes no more input then.
    pub finished: bool,
    /// The expressions evaluated, for ↑ and ↓ in the input field.
    asked: Vec<String>,
}

impl Console {
    /// A console that shows `history` and takes no input, as one that comes back after a restart.
    pub fn finished(history: Vec<ConsoleLine>) -> Console {
        let mut console = Console {
            lines: history,
            finished: true,
            asked: Vec::new(),
        };
        console.trim();
        console
    }

    /// What it shows.
    pub fn lines(&self) -> &[ConsoleLine] {
        &self.lines
    }

    /// Adds `text` of `kind`; output that does not end its line continues on the last line of the same kind.
    pub fn push(&mut self, kind: ConsoleKind, text: &str) {
        if text.is_empty() {
            return;
        }
        match self.lines.last_mut() {
            Some(last) if last.kind == kind && kind == ConsoleKind::Output && !last.text.ends_with('\n') => {
                last.text.push_str(text);
            }
            _ => self.lines.push(ConsoleLine {
                kind,
                text: text.to_string(),
            }),
        }
        self.trim();
    }

    /// Takes the expression the user typed; `None` when there is nothing to evaluate or the console is finished.
    pub fn ask(&mut self, expression: &str) -> Option<String> {
        let expression = expression.trim();
        if expression.is_empty() || self.finished {
            return None;
        }
        self.push(ConsoleKind::Input, expression);
        if self.asked.last().is_none_or(|last| last != expression) {
            self.asked.push(expression.to_string());
        }
        Some(expression.to_string())
    }

    /// The expression asked `back` steps ago, for ↑.
    pub fn asked(&self, back: usize) -> Option<&str> {
        let index = self.asked.len().checked_sub(back.checked_add(1)?)?;
        self.asked.get(index).map(String::as_str)
    }

    /// Ends the console with `note`, such as the program's exit code.
    pub fn finish(&mut self, note: &str) {
        if !self.finished {
            self.push(ConsoleKind::Note, note);
            self.finished = true;
        }
    }

    /// The whole text, each line on its own line.
    #[cfg(test)]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(&line.text);
            if !line.text.ends_with('\n') {
                text.push('\n');
            }
        }
        text
    }

    fn trim(&mut self) {
        if self.lines.len() > LIMIT {
            self.lines.drain(..self.lines.len() - LIMIT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_continues_its_line_and_evaluations_stand_apart() {
        let mut console = Console::default();
        console.push(ConsoleKind::Output, "count: ");
        console.push(ConsoleKind::Output, "3\n");
        console.push(ConsoleKind::Output, "done\n");
        assert_eq!(console.ask(" n + 1 "), Some("n + 1".into()));
        console.push(ConsoleKind::Result, "4");
        assert_eq!(console.lines().len(), 4);
        assert_eq!(console.text(), "count: 3\ndone\nn + 1\n4\n");
        assert_eq!(console.ask("   "), None);
    }

    #[test]
    fn up_goes_back_through_what_was_asked() {
        let mut console = Console::default();
        console.ask("a");
        console.ask("b");
        console.ask("b");
        assert_eq!(console.asked(0), Some("b"));
        assert_eq!(console.asked(1), Some("a"));
        assert_eq!(console.asked(2), None);
    }

    #[test]
    fn a_finished_console_only_shows() {
        let mut console = Console::default();
        console.push(ConsoleKind::Output, "hi\n");
        console.finish("Beendet");
        console.finish("again");
        assert!(console.finished);
        assert_eq!(console.ask("n"), None);
        assert_eq!(console.lines().last().map(|l| l.kind), Some(ConsoleKind::Note));
        let back = Console::finished(console.lines().to_vec());
        assert!(back.finished);
        assert_eq!(back.text(), console.text());
    }

    #[test]
    fn the_oldest_lines_go_beyond_the_limit() {
        let mut console = Console::default();
        for n in 0..LIMIT + 10 {
            console.push(ConsoleKind::Error, &format!("{n}"));
        }
        assert_eq!(console.lines().len(), LIMIT);
        assert_eq!(console.lines()[0].text, "10");
    }
}
