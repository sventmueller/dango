//! The secrets check before code leaves the Mac (R15): a request for a provider outside the Mac goes out only after
//! gitleaks found nothing in its text, checked on a worker thread right before sending and once per revision of a
//! file. A file with a finding stays paused until a later revision checks clean; without gitleaks the text goes out
//! unchecked, as commits do.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::{Cancel, Error, Provider, Reply, Request};

/// What the secrets check said about the text of one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// gitleaks found nothing.
    Clean,
    /// gitleaks is not on the login shell's PATH; nothing was checked.
    NotInstalled,
    /// gitleaks found a secret.
    Found,
    /// The check itself failed, in the user's words.
    Failed(String),
}

impl Verdict {
    /// Whether the text may go out.
    pub(crate) fn sends(&self) -> bool {
        matches!(self, Verdict::Clean | Verdict::NotInstalled)
    }
}

/// The secrets check of a text, run in a folder; gitleaks in the app, a fake in tests.
pub type Scan = Arc<dyn Fn(&Path, &str) -> Verdict + Send + Sync>;

/// The last verdict of each file with the revision it belongs to, shared with the worker threads.
#[derive(Default, Clone)]
pub struct Scans(Arc<Mutex<HashMap<PathBuf, (u64, Verdict)>>>);

impl Scans {
    fn cached(&self, path: &Path, revision: u64) -> Option<Verdict> {
        let all = self.0.lock().ok()?;
        all.get(path).filter(|(at, _)| *at == revision).map(|(_, v)| v.clone())
    }

    fn keep(&self, path: &Path, revision: u64, verdict: Verdict) {
        if let Ok(mut all) = self.0.lock() {
            all.insert(path.to_path_buf(), (revision, verdict));
        }
    }

    /// Whether the last check of `path` found a secret, which keeps its proposals paused.
    pub fn paused(&self, path: &Path) -> bool {
        self.0
            .lock()
            .is_ok_and(|all| all.get(path).is_some_and(|(_, v)| *v == Verdict::Found))
    }
}

/// Hands `request` to `provider`, checking its text first when the provider is `outside` the Mac; what the check said,
/// if one was made. Blocks for the check, so it runs on a worker thread. A request that may not go out is answered
/// [`Error::Cancelled`], so the question ends.
pub fn send(
    request: Request,
    outside: bool,
    scans: &Scans,
    scan: &Scan,
    provider: &dyn Provider,
    cancel: Cancel,
    reply: Reply,
) -> Option<Verdict> {
    if !outside {
        provider.predict(request, cancel, reply);
        return None;
    }
    let verdict = match scans.cached(&request.path, request.revision) {
        Some(verdict) => verdict,
        None => {
            let folder = request.path.parent().unwrap_or(Path::new("/"));
            let verdict = scan(folder, &request.text);
            // A failed check is tried again at the next question.
            if !matches!(verdict, Verdict::Failed(_)) {
                scans.keep(&request.path, request.revision, verdict.clone());
            }
            verdict
        }
    };
    if verdict.sends() && !cancel.is_cancelled() {
        provider.predict(request, cancel, reply);
    } else {
        reply(Err(Error::Cancelled));
    }
    Some(verdict)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use crate::Indent;
    use crate::stub::Stub;

    use super::*;

    fn request(revision: u64) -> Request {
        Request {
            path: PathBuf::from("/p/src/main.rs"),
            language: "rust".into(),
            text: "let token = \"…\";\n".into(),
            revision,
            selection: 0..0,
            indent: Indent::Spaces(4),
        }
    }

    /// A fake check answering `verdict`, and how often it ran.
    fn fake(verdict: Verdict) -> (Scan, Arc<AtomicUsize>) {
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = runs.clone();
        let scan: Scan = Arc::new(move |_: &Path, _: &str| {
            counted.fetch_add(1, Ordering::SeqCst);
            verdict.clone()
        });
        (scan, runs)
    }

    fn ask(stub: &Stub, outside: bool, scans: &Scans, scan: &Scan, revision: u64) -> Option<Verdict> {
        let (tx, rx) = channel();
        let verdict = send(
            request(revision),
            outside,
            scans,
            scan,
            stub,
            Cancel::new(),
            Box::new(move |answer| tx.send(answer).unwrap()),
        );
        let answered = rx.recv_timeout(Duration::from_secs(5));
        assert!(answered.is_ok(), "the question ends with an answer");
        verdict
    }

    #[test]
    fn a_finding_sends_nothing_and_pauses_the_file_until_a_later_revision_is_clean() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let scans = Scans::default();
        let (found, _) = fake(Verdict::Found);
        assert_eq!(ask(&stub, true, &scans, &found, 1), Some(Verdict::Found));
        assert!(stub.asked().is_empty(), "nothing went out");
        assert!(scans.paused(Path::new("/p/src/main.rs")));
        let (clean, _) = fake(Verdict::Clean);
        assert_eq!(
            ask(&stub, true, &scans, &clean, 1),
            Some(Verdict::Found),
            "the same revision stays found"
        );
        assert!(stub.asked().is_empty());
        assert_eq!(ask(&stub, true, &scans, &clean, 2), Some(Verdict::Clean));
        assert_eq!(stub.asked().len(), 1, "a clean later revision goes out");
        assert!(!scans.paused(Path::new("/p/src/main.rs")));
    }

    #[test]
    fn a_clean_text_goes_out_and_is_checked_once_per_revision() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let scans = Scans::default();
        let (clean, runs) = fake(Verdict::Clean);
        ask(&stub, true, &scans, &clean, 4);
        ask(&stub, true, &scans, &clean, 4);
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (2, 1));
    }

    #[test]
    fn a_provider_on_this_mac_gets_the_text_unchecked() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let (found, runs) = fake(Verdict::Found);
        assert_eq!(ask(&stub, false, &Scans::default(), &found, 1), None);
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (1, 0));
    }

    #[test]
    fn without_gitleaks_the_text_goes_out_and_a_failed_check_sends_nothing_and_tries_again() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let scans = Scans::default();
        let (missing, _) = fake(Verdict::NotInstalled);
        assert_eq!(ask(&stub, true, &scans, &missing, 1), Some(Verdict::NotInstalled));
        assert_eq!(stub.asked().len(), 1);
        let (failed, runs) = fake(Verdict::Failed("broken".into()));
        ask(&stub, true, &scans, &failed, 2);
        ask(&stub, true, &scans, &failed, 2);
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (1, 2));
    }
}
