//! What the editor's status bar says about a language server, and installing a missing one when the user asks; never
//! silently (E10).

use std::path::Path;
use std::sync::Arc;

use ide_lsp::{Cancel, NotFound, Status};
use ide_process::Supervisor;
use objc2::DefinedClass;
use splat_i18n::{t, tf};

use super::{Slot, State, find, language_of, refresh_slot, registry, slot, status, trusted_root};
use crate::editor::EditorTextView;
use crate::util::main_soon;

/// What the status bar of `view` says about its server, and the action its button offers.
pub(crate) fn server_status(view: &EditorTextView) -> (String, status::Action) {
    let Some(path) = view.document().path().map(Path::to_path_buf) else {
        return (String::new(), status::Action::None);
    };
    if language_of(&path).is_none() {
        return (String::new(), status::Action::None);
    }
    let root = view.controller().and_then(|c| c.project_root());
    let found = root
        .clone()
        .zip(view.ivars().lsp.borrow().slot)
        .and_then(|(root, id)| slot(&root, id));
    if let Some(slot) = found {
        return slot_status(&slot);
    }
    let text = match root {
        None => t("Sprachserver nur in einem Ordner"),
        Some(root) if !crate::project::is_trusted(&root) => t("Sprachserver aus: Ordner nicht vertraut"),
        Some(_) => "",
    };
    (text.to_string(), status::Action::None)
}

fn slot_status(slot: &Slot) -> (String, status::Action) {
    match &*slot.state.borrow() {
        State::Finding => (
            tf("Suche Sprachserver für {} …", &[slot.language.name()]),
            status::Action::None,
        ),
        State::Missing(missing) => missing_text(missing),
        State::Installing(_, progress) => (progress.clone(), status::Action::Cancel),
        State::Failed(message) => (message.clone(), status::Action::Retry),
        State::Running(server) => {
            let name = server.label().to_string();
            match server.status() {
                Status::Starting => (tf("{} startet …", &[&name]), status::Action::None),
                Status::Restarting { .. } => (tf("{} startet neu …", &[&name]), status::Action::None),
                Status::Failed => (tf("{} ist wiederholt abgestürzt", &[&name]), status::Action::Restart),
                Status::Stopping | Status::Stopped => (tf("{} ist beendet", &[&name]), status::Action::Restart),
                Status::Running => {
                    let busy = slot.progress.borrow().values().next().map(|(_, text)| text.clone());
                    let message = slot.message.borrow().clone();
                    let text = match (busy, message) {
                        (Some(busy), _) => format!("{name}: {busy}"),
                        (None, Some(message)) => format!("{name}: {message}"),
                        (None, None) => name,
                    };
                    (text, status::Action::None)
                }
            }
        }
    }
}

pub(super) fn missing_text(missing: &NotFound) -> (String, status::Action) {
    let not_on_path = |program: &str| {
        tf(
            "Kein Sprachserver für {}: {} fehlt auf dem PATH der Login-Shell",
            &[missing.language.name(), program],
        )
    };
    match (missing.missing, missing.install, missing.hint) {
        (Some(tool), ..) => (not_on_path(tool), status::Action::None),
        (None, Some(_), _) => (
            tf("{} fehlt – in den App-Ordner installieren?", &[&missing.program]),
            status::Action::Install,
        ),
        (None, None, Some(hint)) => (
            tf("{} fehlt – installieren mit „{}“", &[&missing.program, hint]),
            status::Action::None,
        ),
        (None, None, None) => (not_on_path(&missing.program), status::Action::None),
    }
}

/// The button in the status bar of `view` was pressed: install, cancel, or start the server again.
pub fn status_action(view: &EditorTextView) {
    let Some(root) = trusted_root(view) else { return };
    let Some(id) = view.ivars().lsp.borrow().slot else {
        return;
    };
    let Some(slot) = slot(&root, id) else { return };
    let action = slot_status(&slot).1;
    match action {
        status::Action::Install => install(&root, &slot),
        status::Action::Cancel => {
            if let State::Installing(cancel, _) = &*slot.state.borrow() {
                cancel.cancel();
            }
        }
        status::Action::Retry => {
            *slot.state.borrow_mut() = State::Finding;
            find(&root, &slot);
        }
        status::Action::Restart => {
            if let State::Running(server) = &*slot.state.borrow() {
                server.restart();
            }
        }
        status::Action::None => {}
    }
    refresh_slot(&root, id);
}

/// Installs the slot's missing server into the app's tools folder, after the user asked for it in the status bar.
fn install(folder: &Path, slot: &Slot) {
    let what = match &*slot.state.borrow() {
        State::Missing(NotFound {
            install: Some(what), ..
        }) => *what,
        _ => return,
    };
    let cancel = Cancel::new();
    *slot.state.borrow_mut() = State::Installing(cancel.clone(), t("Installiere …").to_string());
    let (folder, id) = (folder.to_path_buf(), slot.id);
    let _ = std::thread::Builder::new().name("lsp-install".into()).spawn(move || {
        let registry = registry();
        let environment = crate::environment::get().map(<[_]>::to_vec).unwrap_or_default();
        let installer = ide_lsp::Installer::new(
            Supervisor::global(),
            environment,
            registry.tools().to_path_buf(),
            Arc::new(ide_lsp::Http::new()),
        );
        let report = {
            let folder = folder.clone();
            move |progress: ide_lsp::Progress| {
                let folder = folder.clone();
                main_soon(move || install_progress(&folder, id, &progress));
            }
        };
        let mut report = report;
        let result = installer.install(what, &cancel, &mut report);
        main_soon(move || installed(&folder, id, result.map(|_| ()).map_err(|e| e.to_string())));
    });
}

fn install_progress(folder: &Path, id: u64, progress: &ide_lsp::Progress) {
    let Some(slot) = slot(folder, id) else { return };
    let Some(text) = progress_text(progress) else { return };
    if let State::Installing(_, shown) = &mut *slot.state.borrow_mut() {
        *shown = text;
    }
    refresh_slot(folder, id);
}

/// What the status bar says while an installation reports `progress`; `None` for a byte count of unknown size.
fn progress_text(progress: &ide_lsp::Progress) -> Option<String> {
    Some(match progress {
        ide_lsp::Progress::Step(step) => match step {
            ide_lsp::Step::Resolving => t("Installation: suche die neueste Version …").to_string(),
            ide_lsp::Step::Downloading => t("Installation: lade herunter …").to_string(),
            ide_lsp::Step::Verifying => t("Installation: prüfe die Prüfsumme …").to_string(),
            ide_lsp::Step::Unpacking => t("Installation: entpacke …").to_string(),
            ide_lsp::Step::Running | ide_lsp::Step::Done => t("Installiere …").to_string(),
        },
        ide_lsp::Progress::Bytes {
            done,
            total: Some(total),
        } if *total > 0 => tf(
            "Installation: lade herunter … {} %",
            &[&(done.saturating_mul(100) / total).to_string()],
        ),
        ide_lsp::Progress::Bytes { .. } => return None,
        ide_lsp::Progress::Output(line) => {
            let line: String = line.chars().take(80).collect();
            tf("Installiere … {}", &[&line])
        }
    })
}

fn installed(folder: &Path, id: u64, result: Result<(), String>) {
    let Some(slot) = slot(folder, id) else { return };
    match result {
        Ok(()) => {
            *slot.state.borrow_mut() = State::Finding;
            find(folder, &slot);
        }
        Err(error) => {
            crate::diag::report("installing a language server", &error);
            *slot.state.borrow_mut() = State::Failed(tf("Installation fehlgeschlagen: {}", &[&error]));
        }
    }
    refresh_slot(folder, id);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use ide_lsp::{Progress, Server, Step};

    use super::*;

    #[test]
    fn an_installation_tells_its_stage_its_share_and_the_installers_last_line() {
        assert_eq!(
            progress_text(&Progress::Step(Step::Verifying)).as_deref(),
            Some(t("Installation: prüfe die Prüfsumme …"))
        );
        assert_eq!(
            progress_text(&Progress::Step(Step::Done)).as_deref(),
            Some(t("Installiere …"))
        );
        let share = progress_text(&Progress::Bytes {
            done: 512,
            total: Some(2048),
        });
        assert_eq!(share, Some(tf("Installation: lade herunter … {} %", &["25"])));
        for unknown in [None, Some(0)] {
            assert_eq!(
                progress_text(&Progress::Bytes {
                    done: 9,
                    total: unknown
                }),
                None
            );
        }
        let long = "x".repeat(200);
        let line = progress_text(&Progress::Output(long)).unwrap();
        assert_eq!(line, tf("Installiere … {}", &[&"x".repeat(80)]));
    }

    fn slot(state: State) -> Slot {
        Slot {
            id: 1,
            language: ide_lsp::Language::Go,
            root: std::env::temp_dir(),
            state: RefCell::new(state),
            progress: RefCell::default(),
            #[cfg(feature = "selftest")]
            progress_at: std::cell::Cell::new(None),
            message: RefCell::new(None),
            hold: std::cell::Cell::new(super::super::lease::Hold::default()),
            timer: std::cell::Cell::new(false),
        }
    }

    #[test]
    fn the_status_bar_offers_what_fits_where_the_server_stands() {
        let finding = slot_status(&slot(State::Finding));
        assert_eq!(
            finding,
            (tf("Suche Sprachserver für {} …", &["Go"]), status::Action::None)
        );
        let installing = slot_status(&slot(State::Installing(Cancel::new(), "50 %".into())));
        assert_eq!(installing, ("50 %".to_string(), status::Action::Cancel));
        let failed = slot_status(&slot(State::Failed("no disk".into())));
        assert_eq!(failed, ("no disk".to_string(), status::Action::Retry));
    }

    fn server(program: &str, args: &[&str]) -> Server {
        let mut config = ide_lsp::ServerConfig::new("stub", Path::new(program), &std::env::temp_dir(), vec![]);
        config.args = args.iter().map(ToString::to_string).collect();
        config.restart.max_restarts = 0;
        config.stop_grace = std::time::Duration::from_millis(100);
        Server::start(&Supervisor::new(), config, |_| {}).unwrap()
    }

    #[test]
    fn a_server_that_did_not_answer_yet_starts_and_one_that_crashed_can_be_restarted() {
        let wait = std::time::Duration::from_secs(10);
        let silent = server("/bin/sleep", &["30"]);
        let starting = slot(State::Running(silent.clone()));
        assert_eq!(
            slot_status(&starting),
            (tf("{} startet …", &["stub"]), status::Action::None)
        );
        silent.stop();
        assert!(silent.wait_for(wait, |s| s == Status::Stopped));
        assert_eq!(
            slot_status(&starting),
            (tf("{} ist beendet", &["stub"]), status::Action::Restart)
        );
        let crashing = server("/usr/bin/false", &[]);
        assert!(crashing.wait_for(wait, |s| s == Status::Failed));
        let failed = slot_status(&slot(State::Running(crashing)));
        assert_eq!(
            failed,
            (tf("{} ist wiederholt abgestürzt", &["stub"]), status::Action::Restart)
        );
    }

    #[test]
    fn every_stage_has_words_of_its_own() {
        let stages = [
            Step::Resolving,
            Step::Downloading,
            Step::Verifying,
            Step::Unpacking,
            Step::Running,
        ];
        let texts: std::collections::BTreeSet<String> = stages
            .iter()
            .filter_map(|step| progress_text(&Progress::Step(*step)))
            .collect();
        assert_eq!(texts.len(), stages.len());
    }
}
