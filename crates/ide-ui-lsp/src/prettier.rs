//! Prettier as the formatter of a document (E133): run with `--stdin-filepath` through the supervisor with the login
//! shell's environment, only in trusted folders since Prettier loads plugins from the project, and applied only to
//! the revision it was asked for, as one undo step.

use crate::EditorSerial;
use ide_document::Revision;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use ide_core::settings::Settings;
use ide_process::{Ending, Kind, Supervisor};
use ide_project::prettier::{self, Formatter, Outcome};
use splat_i18n::{t, tf};

use super::rename::Then;
use crate::editor::Editor;
use crate::host as editor;
use ide_ui_common::util::main_soon;

/// How long Prettier may take before the document is saved without it.
const WAIT: Duration = Duration::from_secs(5);

/// What a run needs: the program, the folder it runs in and the file it formats.
struct Run {
    program: PathBuf,
    dir: PathBuf,
    file: PathBuf,
}

/// The run of Prettier for `view` when Prettier formats its file, else `None` and the server does.
fn run_for(view: &dyn Editor) -> Option<Run> {
    let file = view.document().path()?.to_path_buf();
    let root = view.project_root()?;
    if !crate::host::is_trusted(&root) {
        return None;
    }
    let language = view
        .document()
        .language()
        .map(|l| l.name().to_string())
        .unwrap_or_default();
    let setting = Settings::load_user()
        .in_project(&root)
        .get(&ide_core::settings::keys::FORMATTER);
    if prettier::choose(&file, &root, &language, &setting) != Formatter::Prettier {
        return None;
    }
    let env = ide_ui_common::window::try_environment(Duration::ZERO).unwrap_or_default();
    let path = env.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.as_str());
    let (program, dir) = prettier::program(&file, &root, path)?;
    Some(Run { program, dir, file })
}

/// Whether Prettier formats the document of `view`.
pub fn formats(view: &dyn Editor) -> bool {
    run_for(view).is_some()
}

/// Formats `view` with Prettier and calls `then` afterwards, also when it failed, took too long or the text changed
/// meanwhile; `false` when Prettier does not format this document, and `then` was not called.
pub fn format(view: &dyn Editor, then: Option<Then>) -> Result<(), Option<Then>> {
    let Some(run) = run_for(view) else { return Err(then) };
    let serial = view.lsp().borrow().serial;
    let revision = view.document().revision();
    let text = view.document().text().to_string();
    let id = super::next_id();
    crate::host::state().formatting.wait(serial, id, then);
    let spawned = ide_ui_common::util::spawn("prettier", move || {
        let env = ide_ui_common::window::environment(Duration::from_secs(10));
        let outcome = format_text(&run, &text, &env);
        main_soon(move || finished(serial, id, revision, outcome));
    });
    if let Err(error) = spawned {
        finished(serial, id, revision, Outcome::Failed(error.to_string()));
    }
    Ok(())
}

/// Runs Prettier on `text` and waits up to `WAIT` for it.
fn format_text(run: &Run, text: &str, env: &[(String, String)]) -> Outcome {
    let mut command = Command::new(&run.program);
    command
        .arg("--stdin-filepath")
        .arg(&run.file)
        .current_dir(&run.dir)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let supervisor = Supervisor::global();
    let mut spawned = match supervisor.spawn(command, Kind::Tool, "prettier") {
        Ok(spawned) => spawned,
        Err(error) => return Outcome::Failed(error.to_string()),
    };
    let input = text.as_bytes().to_vec();
    let stdin = spawned.stdin.take();
    let writer = ide_ui_common::util::spawn_joined("prettier-input", move || {
        if let Some(mut stdin) = stdin {
            let _ = stdin.write_all(&input);
        }
    });
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        ide_ui_common::util::spawn_joined("prettier-output", move || {
            let mut out = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut out);
            }
            out
        })
    };
    let stdout = read(spawned.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = read(spawned.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let Some(ending) = supervisor.wait(spawned.id, WAIT) else {
        supervisor.stop(spawned.id, Duration::from_millis(200));
        return Outcome::Failed(t("Prettier hat nicht rechtzeitig geantwortet").to_string());
    };
    if let Ok(writer) = writer {
        writer.join();
    }
    let output =
        |read: Result<ide_ui_common::util::Joined<Vec<u8>>, _>| read.ok().and_then(|r| r.join()).unwrap_or_default();
    let (out, err) = (output(stdout), output(stderr));
    let code = match ending {
        Ending::Code(code) => Some(code),
        _ => None,
    };
    prettier::outcome(code, out, &err)
}

/// Applies what Prettier answered to the editor `serial`, if its text is still the one of `revision`, then runs what
/// waited for the formatting.
fn finished(serial: EditorSerial, id: u64, revision: Revision, outcome: Outcome) {
    let Some(view) = editor::find_editor(serial) else {
        return;
    };
    let then = crate::host::state().formatting.take(serial, id);
    let Some(then) = then else { return };
    match outcome {
        Outcome::Formatted(text) if view.document().revision() == revision => {
            if text != view.document().text().to_string() {
                let changes = view.document_mut().replace_all(&text);
                match changes {
                    Ok(changes) => view.apply_model_changes(&changes),
                    Err(error) => ide_ui_common::diag::report("applying Prettier's formatting", error),
                }
            }
        }
        Outcome::Formatted(_) => {
            super::status::note(&*view, t("Ohne Formatieren: der Text hat sich inzwischen geändert"))
        }
        Outcome::Failed(why) => super::status::note(&*view, &tf("Prettier: {}", &[&why])),
    }
    if let Some(then) = then {
        then(&*view);
    }
}
