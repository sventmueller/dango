//! The panel's runs from start to end, none of it holding up the main thread: tools are found and runs started on
//! the disk lane, and a run's output is read on a worker, which hands back only the reports that changed.

use ide_core::LockIgnoringPoison;

use super::*;

impl TestsPanel {
    /// Finds the folder's tools on the disk lane, then hands the panel to `then`.
    pub(super) fn find_tools(&self, then: impl FnOnce(&TestsPanel) + Send + 'static) {
        if !crate::project::is_trusted(self.root()) {
            self.say(t("Tests nur in vertrauten Ordnern"));
            return;
        }
        let env = ide_ui_common::window::try_environment(Duration::ZERO).unwrap_or_default();
        let path = ide_process::tool::var(&env, "PATH").map(str::to_string);
        let root = self.root().to_path_buf();
        let id = self.id();
        let then = std::sync::Arc::new(std::sync::Mutex::new(Some(then)));
        let later = then.clone();
        let asked = path.clone();
        let queued = ide_core::lane::DISK.ask(
            move || ide_test::tools(&root, asked.as_deref()),
            move |tools| {
                main_soon(move || {
                    super::super::with_panel(id, |panel| {
                        *panel.ivars().tools.borrow_mut() = tools.unwrap_or_default();
                        if let Some(then) = later.lock_ignoring_poison().take() {
                            then(panel);
                        }
                    });
                });
            },
        );
        if queued.is_err() {
            *self.ivars().tools.borrow_mut() = ide_test::tools(self.root(), path.as_deref());
            if let Some(then) = then.lock_ignoring_poison().take() {
                then(self);
            }
        }
    }

    /// Starts `command` of `tool` on the disk lane, a listing without `scope`, and reads it until it ends.
    pub(super) fn launch(&self, tool: Tool, command: ide_test::Command, scope: Option<Scope>) {
        let env = ide_ui_common::window::try_environment(Duration::ZERO).unwrap_or_default();
        let listing = scope.is_none();
        let label = run_label(tool, listing);
        let key = self.ivars().next_run.get();
        self.ivars().next_run.set(key + 1);
        self.ivars().starting.set(self.ivars().starting.get() + 1);
        let id = self.id();
        let start = move || run::start(key, &command, &env, &label, listing);
        let job = std::sync::Arc::new(std::sync::Mutex::new(Some(start)));
        let queued = {
            let job = job.clone();
            ide_core::lane::DISK.ask(
                move || job.lock_ignoring_poison().take().map(|start| start()),
                move |started| {
                    main_soon(move || {
                        let mut started = started.flatten();
                        super::super::with_panel(id, |panel| panel.started(tool, listing, started.take()));
                        // The window closed while the run started.
                        if let Some(Ok(running)) = started {
                            run::stop(&running);
                        }
                    });
                },
            )
        };
        if queued.is_err()
            && let Some(start) = job.lock_ignoring_poison().take()
        {
            self.started(tool, listing, Some(start()));
        }
    }

    /// A run of `tool` started, or did not.
    fn started(&self, tool: Tool, listing: bool, started: Option<std::io::Result<Running>>) {
        self.ivars().starting.set(self.ivars().starting.get().saturating_sub(1));
        match started {
            Some(Ok(running)) => {
                self.ivars().runs.borrow_mut().push((tool, running));
                self.say(&run_label(tool, listing));
                self.poll_later(listing);
            }
            Some(Err(error)) => {
                self.ivars().board.borrow_mut().finished(tool, false);
                self.say(&tf("{} startet nicht: {}", &[tool.name(), &error.to_string()]));
            }
            None => self.ivars().board.borrow_mut().finished(tool, false),
        }
        self.show();
    }

    pub(super) fn poll_later(&self, listing: bool) {
        if self.ivars().polling.replace(true) {
            return;
        }
        let id = self.id();
        main_after(POLL, move || {
            super::super::with_panel(id, |panel| panel.poll(listing));
        });
    }

    /// Reads the runs again on a worker: what each said since the last poll, a listing once it ended.
    fn poll(&self, listing: bool) {
        let root = self.root().to_path_buf();
        let looks: Vec<run::Look> = self
            .ivars()
            .runs
            .borrow_mut()
            .iter_mut()
            .filter_map(|(tool, running)| running.look(*tool, &root))
            .collect();
        let id = self.id();
        let queued = ide_core::lane::WORK.ask(
            move || looks.into_iter().map(run::look).collect::<Vec<_>>(),
            move |polled| {
                main_soon(move || {
                    super::super::with_panel(id, |panel| panel.polled(listing, polled.unwrap_or_default()));
                });
            },
        );
        if queued.is_err() {
            self.polled(listing, Vec::new());
        }
    }

    /// Takes what the worker read of the runs.
    fn polled(&self, listing: bool, answers: Vec<run::Polled>) {
        self.ivars().polling.set(false);
        let still = {
            let mut runs = self.ivars().runs.borrow_mut();
            let mut board = self.ivars().board.borrow_mut();
            let mut ended = Vec::new();
            for answer in answers {
                let Some((tool, running)) = runs.iter_mut().find(|(_, r)| r.key == answer.key) else {
                    continue;
                };
                running.progress = Some(answer.progress);
                if answer.restarted {
                    running.reports.clear();
                    running.applied = None;
                }
                if let Some(tests) = answer.tests {
                    board.listed(*tool, tests);
                } else if !running.listing {
                    apply(&mut board, *tool, running, answer.reports);
                }
                if let Some(ending) = answer.ending {
                    *self.ivars().output.borrow_mut() = answer.output.unwrap_or_default();
                    ended.push((answer.key, matches!(ending, Ending::Code(_))));
                }
            }
            // A look that was lost reads its run again from the start.
            for (_, running) in runs.iter_mut().filter(|(_, r)| r.progress.is_none()) {
                running.progress = Some(run::Progress::default());
                running.reports.clear();
                running.applied = None;
            }
            for (key, complete) in ended {
                if let Some(index) = runs.iter().position(|(_, r)| r.key == key) {
                    let (tool, running) = runs.remove(index);
                    if !running.listing {
                        board.finished(tool, complete);
                    }
                }
            }
            !runs.is_empty() || self.ivars().starting.get() > 0
        };
        self.show();
        let revision = self.ivars().board.borrow().revision();
        if self.ivars().marked.replace(Some(revision)) != Some(revision) {
            super::super::marks_changed(self);
        }
        let summary = summary(&self.ivars().board.borrow());
        if still {
            if !listing {
                self.say(&tf("Läuft … {}", &[&summary]));
            }
            self.poll_later(listing);
        } else {
            self.say(&tf("Fertig: {}", &[&summary]));
        }
    }
}

/// What the activity view and the status line call a run of `tool`.
fn run_label(tool: Tool, listing: bool) -> String {
    if listing {
        tf("{} auflisten", &[tool.name()])
    } else {
        tf("{} ausführen", &[tool.name()])
    }
}

/// Gives the board the reports of `running` that `changed`; all of them where the board's entries of `tool` changed
/// otherwise since, as another run, a listing or a start may have overwritten what the run said.
fn apply(board: &mut Board, tool: Tool, running: &mut Running, changed: Vec<(usize, ide_test::Report)>) {
    let current = running.applied == Some(board.tool_revision(tool));
    for (index, report) in &changed {
        if running.reports.len() <= *index {
            running.reports.resize(index + 1, None);
        }
        running.reports[*index] = Some(report.clone());
    }
    let reports = if current {
        changed.into_iter().map(|(_, report)| report).collect()
    } else {
        running.reports.iter().flatten().cloned().collect()
    };
    board.reported(tool, reports);
    running.applied = Some(board.tool_revision(tool));
}
