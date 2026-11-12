//! The providers against the real Copilot server and real local models, found the way the app finds them: with the
//! login shell's environment. They need a sign-in, installed models or the network, so they run only on request
//! (`make test-integration`): `cargo test -p ide-predict --test real_providers -- --ignored --test-threads 1`.
//!
//! They send only the tiny made-up file below, never code of the user's.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use ide_lsp::{Cancel as InstallCancel, Http, Installer, default_tools};
use ide_predict::copilot::{self, Account, Copilot};
use ide_predict::fim::{LMSTUDIO_BASE, LmStudio, OLLAMA_BASE, Ollama};
use ide_predict::{Cancel, Error, Indent, Prediction, Provider, Request};
use ide_process::Supervisor;
use ide_process::shell_env::{capture, user_shell};

/// How long a model may take to load and answer, the first time.
const PATIENCE: Duration = Duration::from_secs(180);

const TEXT: &str = "/// Adds two numbers.\nfn add(a: i32, b: i32) -> i32 {\n    \n}\n";

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-predict-real-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn request(root: &Path) -> Request {
    let caret = TEXT.find("    \n").unwrap() + 4;
    Request {
        path: root.join("src/lib.rs"),
        language: "rust".into(),
        text: TEXT.into(),
        revision: 1,
        selection: caret..caret,
        indent: Indent::Spaces(4),
    }
}

fn predict(provider: &dyn Provider, request: Request, cancel: Cancel) -> Result<Vec<Prediction>, Error> {
    let (tx, rx) = channel();
    provider.predict(
        request,
        cancel,
        Box::new(move |answer| {
            let _ = tx.send(answer);
        }),
    );
    rx.recv_timeout(PATIENCE).expect("an answer in time")
}

/// A proposal for the body of `add`, and a request cancelled at once ends at once.
fn fills_the_gap(provider: &dyn Provider, root: &Path) {
    let found = predict(provider, request(root), Cancel::new()).expect("proposals");
    eprintln!("{}: {found:?}", provider.name());
    assert!(found.iter().any(|p| !p.text.trim().is_empty()), "{found:?}");
    let cancel = Cancel::new();
    let (tx, rx) = channel();
    provider.predict(
        request(root),
        cancel.clone(),
        Box::new(move |answer| {
            let _ = tx.send(answer);
        }),
    );
    std::thread::sleep(Duration::from_millis(30));
    let cancelled = Instant::now();
    cancel.cancel();
    let answer = rx.recv_timeout(PATIENCE).unwrap();
    assert!(
        answer == Err(Error::Cancelled) || cancelled.elapsed() < Duration::from_millis(500),
        "{answer:?} after {:?}",
        cancelled.elapsed()
    );
}

#[test]
#[ignore = "needs Ollama with a fill-in-the-middle model, see make test-integration"]
fn ollama_fills_the_gap_with_a_real_model() {
    let model = std::env::var("IDE_INTEGRATION_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:1.5b".into());
    let Ok(installed) = Ollama::models(OLLAMA_BASE) else {
        return println!("integration skip: Ollama does not answer; start it with `ollama serve`");
    };
    if !installed.contains(&model) {
        return println!("integration skip: {model} is missing; ollama pull {model}");
    }
    fills_the_gap(&Ollama::new(OLLAMA_BASE, &model), &folder("ollama"));
}

#[test]
#[ignore = "needs LM Studio's server with a fill-in-the-middle model, see make test-integration"]
fn lm_studio_fills_the_gap_with_a_real_model() {
    let model =
        std::env::var("IDE_INTEGRATION_LMSTUDIO_MODEL").unwrap_or_else(|_| "qwen2.5-coder-1.5b-instruct".into());
    let Ok(offered) = LmStudio::models(LMSTUDIO_BASE) else {
        return println!("integration skip: LM Studio's server does not answer; start it with `lms server start`");
    };
    if !offered.contains(&model) {
        return println!("integration skip: LM Studio does not offer {model}; lms get {model}");
    }
    fills_the_gap(&LmStudio::new(LMSTUDIO_BASE, &model), &folder("lmstudio"));
}

#[test]
#[ignore = "needs a Copilot sign-in, node and the network, see make test-integration"]
fn copilot_proposes_a_completion_for_the_signed_in_user() {
    let supervisor = Supervisor::new();
    let root = folder("copilot");
    let env =
        capture(&supervisor, &user_shell(), &root, Duration::from_secs(30)).expect("the login shell's environment");
    let program = copilot::find(&default_tools(), &env).unwrap_or_else(|| {
        let tools = root.join("tools");
        let installer = Installer::new(&supervisor, env.clone(), tools, Arc::new(Http::new()));
        installer
            .install(copilot::INSTALL, &InstallCancel::new(), &mut |_| {})
            .expect("copilot-language-server installs with npm")
    });
    let copilot = Copilot::start(&supervisor, &program, &root, env, |_| {}).unwrap();
    let deadline = Instant::now() + PATIENCE;
    while copilot.account() == Account::Starting && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    let account = copilot.account();
    if !matches!(account, Account::SignedIn { .. }) {
        return println!("integration skip: Copilot reports {account:?}; sign in to Copilot in the app first");
    }
    let found = predict(&copilot, request(&root), Cancel::new()).expect("proposals");
    eprintln!("Copilot: {found:?}");
    assert!(!found.is_empty());
    if let Some(taken) = found.iter().find(|p| p.feedback.is_some()) {
        copilot.accepted(taken);
    }
    let cancel = Cancel::new();
    cancel.cancel();
    assert_eq!(predict(&copilot, request(&root), cancel), Err(Error::Cancelled));
}
