//! The rows of the debug panel: two sections, the sessions with their threads and call stacks, and the breakpoints with
//! the exception filters the adapters offered; each row by a key that survives a reload of the list.

use std::path::PathBuf;

use super::Model;

/// A section of the panel, by the name the kept state folds it under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// The sessions with their threads and frames.
    Sessions,
    /// The breakpoints and exception filters.
    Breakpoints,
}

impl Section {
    /// Every section, in the panel's order.
    pub(crate) const ALL: [Section; 2] = [Section::Sessions, Section::Breakpoints];

    /// The name it is kept under.
    pub fn name(self) -> &'static str {
        match self {
            Section::Sessions => "sessions",
            Section::Breakpoints => "breakpoints",
        }
    }
}

/// A row of the panel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A section's title row.
    Section(Section),
    /// The line a section shows while it has nothing.
    Empty(Section),
    /// A session.
    Session(u64),
    /// A thread of a session.
    Thread(u64, i64),
    /// A frame of a thread's call stack, by its place from the innermost.
    Frame(u64, i64, usize),
    /// An exception filter, by the adapter's id.
    Filter(String),
    /// A breakpoint, by file and line.
    Breakpoint(PathBuf, usize),
}

impl Model {
    /// The rows below `key`, the sections for `None`.
    pub fn rows(&self, key: Option<&Key>) -> Vec<Key> {
        let Some(key) = key else {
            return Section::ALL.into_iter().map(Key::Section).collect();
        };
        let rows = match key {
            Key::Section(Section::Sessions) => self.sessions.iter().map(|s| Key::Session(s.id)).collect(),
            Key::Section(Section::Breakpoints) => self
                .filters
                .iter()
                .map(|f| Key::Filter(f.id.clone()))
                .chain(
                    self.breakpoints
                        .all()
                        .iter()
                        .map(|b| Key::Breakpoint(b.path.clone(), b.line)),
                )
                .collect(),
            Key::Session(id) => self
                .session(*id)
                .map(|s| s.threads.iter().map(|t| Key::Thread(*id, t.id)).collect())
                .unwrap_or_default(),
            Key::Thread(id, thread) => self
                .session(*id)
                .and_then(|s| s.thread(*thread))
                .and_then(|t| t.frames.as_ref())
                .map(|frames| (0..frames.len()).map(|i| Key::Frame(*id, *thread, i)).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        match key {
            Key::Section(section) if rows.is_empty() => vec![Key::Empty(*section)],
            _ => rows,
        }
    }

    /// Whether the row of `key` can open.
    pub fn opens(&self, key: &Key) -> bool {
        match key {
            Key::Section(_) => true,
            Key::Session(id) => self.session(*id).is_some_and(|s| !s.threads.is_empty()),
            Key::Thread(id, thread) => self
                .session(*id)
                .and_then(|s| s.thread(*thread))
                .and_then(|t| t.frames.as_ref())
                .is_some_and(|frames| !frames.is_empty()),
            _ => false,
        }
    }

    /// The rows that should stand open: the paused session, its stopped thread, and the sections not folded.
    pub fn open_rows(&self) -> Vec<Key> {
        let mut open: Vec<Key> = Section::ALL
            .into_iter()
            .filter(|s| !self.is_folded(s.name()))
            .map(Key::Section)
            .collect();
        for session in &self.sessions {
            open.push(Key::Session(session.id));
            if let Some(thread) = session.paused_thread() {
                open.push(Key::Thread(session.id, thread));
            }
        }
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kept::{Breakpoint, Debugging, ExceptionFilter};
    use crate::model::session::{Frame, Origin, Session};

    #[test]
    fn sections_list_sessions_threads_frames_filters_and_breakpoints() {
        let mut model = Model::new(Debugging {
            breakpoints: vec![Breakpoint::new("/p/a.rs", 3)],
            exception_filters: vec![ExceptionFilter {
                id: "panic".into(),
                label: "Panic".into(),
                enabled: true,
            }],
            ..Debugging::default()
        });
        assert_eq!(
            model.rows(Some(&Key::Section(Section::Sessions))),
            [Key::Empty(Section::Sessions)]
        );
        assert_eq!(
            model.rows(Some(&Key::Section(Section::Breakpoints))),
            [Key::Filter("panic".into()), Key::Breakpoint("/p/a.rs".into(), 3)]
        );
        model.started(Session::new(1, "shop", Origin::Launched));
        model.threads(1, vec![(7, "main".into())]);
        model.stopped(1, 7, "breakpoint");
        let frame = Frame {
            id: 100,
            name: "main".into(),
            path: None,
            line: 2,
            column: 0,
        };
        model.stack(1, 7, vec![frame.clone(), frame]);
        assert_eq!(model.rows(Some(&Key::Session(1))), [Key::Thread(1, 7)]);
        assert_eq!(
            model.rows(Some(&Key::Thread(1, 7))),
            [Key::Frame(1, 7, 0), Key::Frame(1, 7, 1)]
        );
        assert!(model.opens(&Key::Thread(1, 7)));
        assert!(!model.opens(&Key::Frame(1, 7, 0)));
        model.fold("breakpoints", true);
        assert_eq!(
            model.open_rows(),
            [Key::Section(Section::Sessions), Key::Session(1), Key::Thread(1, 7)]
        );
    }
}
