//! Copilot against a stand-in for its language server: sign-in, document sync, completions, next edits, feedback
//! and cancelling.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ide_predict::copilot::{Account, Copilot, SignIn};
use ide_predict::{Cancel, Error, Indent, Prediction, Provider, Request};
use ide_process::Supervisor;

const WAIT: Duration = Duration::from_secs(20);

struct Setup {
    copilot: Copilot,
    accounts: Receiver<Account>,
    log: PathBuf,
    root: PathBuf,
    _dir: Temp,
}

/// A folder of its own, removed after the test.
struct Temp(PathBuf);

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn start(name: &str, vars: &[(&str, &str)]) -> Setup {
    let dir = std::env::temp_dir().join(format!("ide-predict-copilot-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log");
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    env.push(("FAKE_LOG".into(), log.display().to_string()));
    env.extend(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let program = Path::new(env!("CARGO_BIN_EXE_ide-predict-fake-copilot"));
    let (tx, accounts) = channel();
    let copilot = Copilot::start(&Supervisor::new(), program, &dir, env, move |account| {
        let _ = tx.send(account);
    })
    .unwrap();
    Setup {
        copilot,
        accounts,
        log,
        root: dir.clone(),
        _dir: Temp(dir),
    }
}

fn until(accounts: &Receiver<Account>, wanted: impl Fn(&Account) -> bool) -> Account {
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let account = accounts.recv_timeout(left).expect("the account changed in time");
        if wanted(&account) {
            return account;
        }
    }
}

fn request(root: &Path, text: &str, revision: u64) -> Request {
    let caret = text.find('|').expect("a caret");
    Request {
        path: root.join("src/lib.rs"),
        language: "rust".into(),
        text: text.replacen('|', "", 1),
        revision,
        selection: caret..caret,
        indent: Indent::Spaces(4),
    }
}

fn predict(provider: &dyn Provider, request: Request) -> Result<Vec<Prediction>, Error> {
    let (tx, rx) = channel();
    provider.predict(
        request,
        Cancel::new(),
        Box::new(move |answer| {
            let _ = tx.send(answer);
        }),
    );
    rx.recv_timeout(WAIT).expect("an answer")
}

fn log(setup: &Setup) -> String {
    std::fs::read_to_string(&setup.log).unwrap_or_default()
}

fn wait_for_log(setup: &Setup, needle: &str) -> String {
    wait_for_count(setup, needle, 1)
}

fn wait_for_count(setup: &Setup, needle: &str, count: usize) -> String {
    let deadline = Instant::now() + WAIT;
    loop {
        let log = log(setup);
        if log.matches(needle).count() >= count || Instant::now() > deadline {
            return log;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_signed_in_user_gets_completions_and_next_edits_of_the_current_text_and_feedback_goes_back() {
    let setup = start("signed-in", &[("FAKE_SIGNED_IN", "1")]);
    let account = until(&setup.accounts, |a| *a != Account::Starting);
    assert_eq!(account, Account::SignedIn { user: "octocat".into() });
    let first = request(&setup.root, "fn main() {\n    let old_name = |\n}\n", 1);
    let caret = first.selection.start;
    let found = predict(&setup.copilot, first).unwrap();
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(
        (found[0].range.clone(), found[0].text.as_str()),
        (caret..caret, "completion()")
    );
    let edit_at = "fn main() {\n    let ".len();
    assert_eq!(
        (found[1].range.clone(), found[1].text.as_str()),
        (edit_at..edit_at + 3, "new")
    );

    let second = request(&setup.root, "fn main() {\n    let renamed = 1;\n    ä|\n}\n", 2);
    let caret = second.selection.start;
    let found = predict(&setup.copilot, second).unwrap();
    assert_eq!(
        found,
        vec![Prediction {
            range: caret..caret,
            text: "completion()".into(),
            feedback: found[0].feedback.clone(),
        }]
    );
    setup.copilot.accepted(&found[0]);
    let log = wait_for_log(&setup, "didAcceptCompletionItem");
    assert!(
        log.contains(r#""github.copilot.didAcceptCompletionItem" ["c1"]"#),
        "{log}"
    );
    assert_eq!(log.matches("textDocument/didOpen").count(), 1, "{log}");
    assert!(log.contains("textDocument/didChange"), "{log}");
}

#[test]
fn without_a_sign_in_the_device_flow_signs_the_user_in() {
    let setup = start("sign-in", &[]);
    assert_eq!(
        until(&setup.accounts, |a| *a != Account::Starting),
        Account::NeedsSignIn
    );
    let asked = predict(&setup.copilot, request(&setup.root, "x|", 1));
    assert_eq!(asked, Err(Error::SignIn));
    let (tx, rx) = channel();
    setup.copilot.sign_in(move |answer| {
        let _ = tx.send(answer);
    });
    let Ok(SignIn::Device(code)) = rx.recv_timeout(WAIT).unwrap() else {
        panic!("a device code")
    };
    assert_eq!(
        (code.user_code.as_str(), code.verification_uri.as_str()),
        ("ABCD-EFGH", "https://github.com/login/device")
    );
    setup.copilot.finish_sign_in(Cancel::new());
    let account = until(&setup.accounts, |a| matches!(a, Account::SignedIn { .. }));
    assert_eq!(account, Account::SignedIn { user: "octocat".into() });
    assert!(predict(&setup.copilot, request(&setup.root, "x|", 2)).is_ok());
    setup.copilot.sign_out();
    assert_eq!(
        until(&setup.accounts, |a| *a == Account::NeedsSignIn),
        Account::NeedsSignIn
    );
}

#[test]
fn an_account_without_a_subscription_is_told_apart() {
    let setup = start(
        "no-subscription",
        &[("FAKE_SIGNED_IN", "1"), ("FAKE_NO_SUBSCRIPTION", "1")],
    );
    let account = until(&setup.accounts, |a| *a != Account::Starting);
    assert_eq!(account, Account::NoSubscription { user: "octocat".into() });
    let asked = predict(&setup.copilot, request(&setup.root, "x|", 1));
    assert!(matches!(asked, Err(Error::Unavailable(_))), "{asked:?}");
}

#[test]
fn cancelling_ends_both_requests_at_the_server_and_replies_off_the_callers_thread() {
    let setup = start("cancel", &[("FAKE_SIGNED_IN", "1")]);
    until(&setup.accounts, |a| matches!(a, Account::SignedIn { .. }));
    let cancel = Cancel::new();
    let (tx, rx) = channel();
    let caller = std::thread::current().id();
    setup.copilot.predict(
        request(&setup.root, "SLOW|", 1),
        cancel.clone(),
        Box::new(move |answer| {
            let _ = tx.send((answer, std::thread::current().id()));
        }),
    );
    wait_for_log(&setup, "copilotInlineEdit");
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    cancel.cancel();
    let (answer, on) = rx.recv_timeout(WAIT).unwrap();
    assert_eq!(answer, Err(Error::Cancelled));
    assert_ne!(on, caller);
    let log = wait_for_count(&setup, "$/cancelRequest", 2);
    assert_eq!(log.matches("$/cancelRequest").count(), 2, "{log}");
}

#[test]
fn a_signed_in_user_is_told_so_by_sign_in_and_the_oldest_of_many_documents_is_closed() {
    let setup = start("many", &[("FAKE_SIGNED_IN", "1")]);
    until(&setup.accounts, |a| matches!(a, Account::SignedIn { .. }));
    assert_eq!(setup.copilot.account(), Account::SignedIn { user: "octocat".into() });
    let (tx, rx) = channel();
    setup.copilot.sign_in(move |answer| {
        let _ = tx.send(answer);
    });
    assert_eq!(
        rx.recv_timeout(WAIT).unwrap(),
        Ok(SignIn::Already { user: "octocat".into() })
    );
    setup.copilot.check();
    wait_for_log(&setup, "checkStatus");
    for n in 0..11 {
        let mut asked = request(&setup.root, "x|", 1);
        asked.path = setup.root.join(format!("src/file{n}.rs"));
        assert!(predict(&setup.copilot, asked).is_ok());
    }
    let log = wait_for_log(&setup, "textDocument/didClose");
    assert_eq!(log.matches("textDocument/didOpen").count(), 11, "{log}");
    assert_eq!(
        log.matches("textDocument/didClose").count(),
        1,
        "only the eleventh pushes one out: {log}"
    );
}
