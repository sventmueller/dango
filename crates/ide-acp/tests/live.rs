//! The agents of the plan (E5) against a tiny prompt, as the app starts them: their own command on the login shell's
//! PATH, or the version the preset pins, installed into a scratch folder the way the app installs it (E35). Each
//! session's messages are recorded with `acp-record` for replay tests, into `IDE_ACP_RECORDINGS` or the temporary
//! folder. They need the network and a sign-in with the agent's service and cost a few tokens, so they run only on
//! request, normally through `make test-integration`:
//! `cargo test -p ide-acp --test live -- --ignored --exact <name> --nocapture`. A test that finds no program, runtime
//! or sign-in prints `integration skip: <why>` and passes, which the runner reports as skipped.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

use ide_core::testing::scratch;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_acp::launch::Origin;
use ide_acp::register::PRESETS;
use ide_acp::{Event, Installer, Installs, Launch, Plan, Preset, Session, SessionError};
use ide_net::Http;
use ide_process::Supervisor;
use ide_process::shell_env::{capture, user_shell};
use ide_thread::{EntryKind, Lane, Status, Thread, ThreadId, UserMessage};

const PROMPT: &str = "Reply with the single word pong.";
/// How long an agent may take to open a session and to answer.
const PATIENCE: Duration = Duration::from_secs(180);

#[derive(Clone, Copy)]
enum From {
    Path,
    Pinned,
}

#[test]
#[ignore = "starts the user's gemini with their sign-in; costs a few tokens"]
fn gemini_on_the_path_answers_a_tiny_prompt() {
    answers("gemini", From::Path);
}

#[test]
#[ignore = "installs the pinned Gemini CLI and starts it with the user's sign-in; costs a few tokens"]
fn gemini_as_pinned_answers_a_tiny_prompt() {
    answers("gemini", From::Pinned);
}

#[test]
#[ignore = "starts the user's agy with their sign-in; costs a few tokens"]
fn antigravity_on_the_path_answers_a_tiny_prompt() {
    answers("antigravity-acp", From::Path);
}

#[test]
#[ignore = "installs the pinned Antigravity server and starts it with the user's sign-in; costs a few tokens"]
fn antigravity_as_pinned_answers_a_tiny_prompt() {
    answers("antigravity-acp", From::Pinned);
}

#[test]
#[ignore = "starts the user's codex-acp with their sign-in; costs a few tokens"]
fn codex_on_the_path_answers_a_tiny_prompt() {
    answers("codex-acp", From::Path);
}

#[test]
#[ignore = "installs the pinned codex-acp and starts it with the user's sign-in; costs a few tokens"]
fn codex_as_pinned_answers_a_tiny_prompt() {
    answers("codex-acp", From::Pinned);
}

fn skip(why: &str) {
    eprintln!("integration skip: {why}");
}

fn answers(id: &str, from: From) {
    let preset = PRESETS.iter().find(|p| p.id == id).expect("a preset");
    let supervisor = Supervisor::new();
    let project = scratch(&format!("{id}-project"));
    let env =
        capture(&supervisor, &user_shell(), &project, Duration::from_secs(30)).expect("the login shell's environment");
    let Some(launch) = launch(preset, from, &supervisor, &env, &project) else {
        return;
    };
    eprintln!(
        "{} {:?}: {} {:?}",
        preset.id,
        launch.origin,
        launch.program.display(),
        launch.args
    );
    let recording = recording(id, from);
    let launch = recorded(launch, &recording);
    let events: Arc<Mutex<Vec<Event>>> = Arc::default();
    let heard = events.clone();
    let thread = Thread::new(ThreadId::from("live"), launch.agent.clone(), launch.cwd.clone());
    let session = Session::start(&supervisor, launch, thread, move |e| heard.lock().unwrap().push(e)).unwrap();
    let opened = wait(&session, &events, || session.is_ready());
    if let Err(why) = opened {
        session.stop(Duration::from_secs(2));
        return skip(&format!("{}: {why}", preset.name));
    }
    session
        .send(UserMessage {
            text: PROMPT.into(),
            ..UserMessage::default()
        })
        .unwrap();
    let answered = wait(&session, &events, || {
        let thread = session.snapshot();
        thread.status() == Status::Idle && answer(&thread).is_some_and(|text| text.to_lowercase().contains("pong"))
    });
    let thread = session.snapshot();
    session.stop(Duration::from_secs(2));
    if let Err(why) = answered {
        return skip(&format!("{}: {why}", preset.name));
    }
    eprintln!(
        "{} answered {:?}; recorded {}",
        preset.name,
        answer(&thread).unwrap_or_default(),
        recording.display()
    );
}

/// How `preset` starts `from` where it is asked for; nothing, after saying why, when it cannot.
fn launch(
    preset: &Preset,
    from: From,
    supervisor: &Supervisor,
    env: &[(String, String)],
    project: &Path,
) -> Option<Launch> {
    match from {
        From::Path => {
            let none = Installs::new(scratch(&format!("{}-none", preset.id)).keep());
            let launch = Launch::of_preset(preset, &none, env, project).filter(|l| l.origin == Origin::OnPath);
            if launch.is_none() {
                let command = preset.on_path.map_or(preset.id, |(name, _)| name);
                skip(&format!("{command} is not on the login shell's PATH"));
            }
            launch
        }
        From::Pinned => {
            let installs = Installs::new(scratch(&format!("{}-agents", preset.id)).keep());
            let installer = Installer::new(
                supervisor,
                env.to_vec(),
                installs.clone(),
                Arc::new(Http::new("agent register")),
            );
            let plan = Plan::of_preset(preset);
            if let Some(problem) = installer.missing(&plan) {
                skip(&format!("{} {}: {problem}", preset.name, plan.version));
                return None;
            }
            let installed = installer
                .install(&plan, true, &|p| eprintln!("{p:?}"))
                .expect("the pinned version installs");
            eprintln!("installed {} {}", preset.id, installed.version);
            Launch::installed(preset.id, preset.name, &installs, env, project)
        }
    }
}

fn recording(id: &str, from: From) -> PathBuf {
    let dir = std::env::var_os("IDE_ACP_RECORDINGS").map_or_else(std::env::temp_dir, PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let source = match from {
        From::Path => "path",
        From::Pinned => "pinned",
    };
    let file = dir.join(format!("{id}-{source}.ndjson"));
    let _ = std::fs::remove_file(&file);
    file
}

/// The same launch with `acp-record` in between.
fn recorded(mut launch: Launch, file: &Path) -> Launch {
    let mut args = vec![
        file.to_string_lossy().into_owned(),
        launch.program.to_string_lossy().into_owned(),
    ];
    args.append(&mut launch.args);
    launch.program = PathBuf::from(env!("CARGO_BIN_EXE_acp-record"));
    launch.args = args;
    launch
}

/// The text of the agent's last answer.
fn answer(thread: &Thread) -> Option<String> {
    thread
        .entries(&Lane::Main)
        .iter()
        .rev()
        .find_map(|entry| match &entry.kind {
            EntryKind::AgentMessage(message) => Some(message.markdown.clone()),
            _ => None,
        })
}

/// Waits until `done`; why not when the agent wants a sign-in, fails, ends or takes too long.
fn wait(session: &Session, events: &Mutex<Vec<Event>>, done: impl Fn() -> bool) -> Result<(), String> {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        for event in events.lock().unwrap().drain(..) {
            match event {
                Event::Failed(SessionError::SignIn) => return Err("asks for a sign-in".into()),
                Event::Failed(error) => panic!("the session failed: {error}"),
                Event::Ended { stderr } => panic!("the agent ended: {stderr}"),
                _ => {}
            }
        }
        assert!(!session.is_ended(), "the session ended");
        assert!(Instant::now() < deadline, "no answer within {} s", PATIENCE.as_secs());
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}
