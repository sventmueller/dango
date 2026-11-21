//! The secrets check before code leaves the Mac (R15): a request for a provider outside the Mac goes out only after
//! gitleaks found nothing in its text, checked on a worker thread right before sending and once per revision of a
//! file. A file with a finding stays paused until a later revision checks clean; without gitleaks nothing goes out
//! until the user chose to send unchecked (B104).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Whether the text may go out, `unchecked` when the user chose to send it without gitleaks.
    pub(crate) fn sends(&self, unchecked: bool) -> bool {
        match self {
            Verdict::Clean => true,
            Verdict::NotInstalled => unchecked,
            Verdict::Found | Verdict::Failed(_) => false,
        }
    }
}

/// Where a request goes, which decides whether its text is checked first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// A provider on this Mac, which gets the text unchecked.
    Here,
    /// A provider outside the Mac.
    Outside {
        /// Whether the user chose to send the text there while gitleaks is missing.
        unchecked: bool,
    },
}

/// The secrets check of a text, in a folder of its own that nothing of the project's configures; gitleaks in the
/// app, a fake in tests.
pub type Scan = Arc<dyn Fn(&str) -> Verdict + Send + Sync>;

/// The last verdict of each file with the revision it belongs to, and whether the last check found no gitleaks, shared
/// with the worker threads.
#[derive(Default, Clone)]
pub struct Scans(Arc<Mutex<HashMap<PathBuf, (u64, Verdict)>>>, Arc<AtomicBool>);

impl Scans {
    fn cached(&self, path: &Path, revision: u64) -> Option<Verdict> {
        let all = self.0.lock().ok()?;
        all.get(path).filter(|(at, _)| *at == revision).map(|(_, v)| v.clone())
    }

    fn keep(&self, path: &Path, revision: u64, verdict: Verdict) {
        self.1.store(verdict == Verdict::NotInstalled, Ordering::Relaxed);
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

    /// Whether the last check found no gitleaks, which holds every request for a provider outside the Mac back until
    /// the user chooses to send unchecked.
    pub fn missing(&self) -> bool {
        self.1.load(Ordering::Relaxed)
    }
}

/// Hands `request` to `provider`, checking its text first when the provider is outside the Mac; what the check said,
/// if one was made. Blocks for the check, so it runs on a worker thread. A request that may not go out is answered
/// [`Error::Cancelled`], so the question ends.
pub fn send(
    request: Request,
    reach: Reach,
    scans: &Scans,
    scan: &Scan,
    provider: &dyn Provider,
    cancel: Cancel,
    reply: Reply,
) -> Option<Verdict> {
    let Reach::Outside { unchecked } = reach else {
        provider.predict(request, cancel, reply);
        return None;
    };
    let verdict = match scans.cached(&request.path, request.revision) {
        Some(verdict) => verdict,
        None => {
            let verdict = scan(&request.text);
            // A failed check is tried again at the next question.
            if !matches!(verdict, Verdict::Failed(_)) {
                scans.keep(&request.path, request.revision, verdict.clone());
            }
            verdict
        }
    };
    if verdict.sends(unchecked) && !cancel.is_cancelled() {
        provider.predict(request, cancel, reply);
    } else {
        reply(Err(Error::Cancelled));
    }
    Some(verdict)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
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
        let scan: Scan = Arc::new(move |_: &str| {
            counted.fetch_add(1, Ordering::SeqCst);
            verdict.clone()
        });
        (scan, runs)
    }

    const OUTSIDE: Reach = Reach::Outside { unchecked: false };

    fn ask(stub: &Stub, reach: Reach, scans: &Scans, scan: &Scan, revision: u64) -> Option<Verdict> {
        let (tx, rx) = channel();
        let verdict = send(
            request(revision),
            reach,
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
        assert_eq!(ask(&stub, OUTSIDE, &scans, &found, 1), Some(Verdict::Found));
        assert!(stub.asked().is_empty(), "nothing went out");
        assert!(scans.paused(Path::new("/p/src/main.rs")));
        let (clean, _) = fake(Verdict::Clean);
        assert_eq!(
            ask(&stub, OUTSIDE, &scans, &clean, 1),
            Some(Verdict::Found),
            "the same revision stays found"
        );
        assert!(stub.asked().is_empty());
        assert_eq!(ask(&stub, OUTSIDE, &scans, &clean, 2), Some(Verdict::Clean));
        assert_eq!(stub.asked().len(), 1, "a clean later revision goes out");
        assert!(!scans.paused(Path::new("/p/src/main.rs")));
    }

    #[test]
    fn a_clean_text_goes_out_and_is_checked_once_per_revision() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let scans = Scans::default();
        let (clean, runs) = fake(Verdict::Clean);
        ask(&stub, OUTSIDE, &scans, &clean, 4);
        ask(&stub, OUTSIDE, &scans, &clean, 4);
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (2, 1));
    }

    #[test]
    fn a_provider_on_this_mac_gets_the_text_unchecked() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let (found, runs) = fake(Verdict::Found);
        assert_eq!(ask(&stub, Reach::Here, &Scans::default(), &found, 1), None);
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (1, 0));
    }

    #[test]
    fn without_gitleaks_nothing_goes_out_until_the_user_sends_unchecked_and_a_failed_check_tries_again() {
        let stub = Stub::new(vec![], Duration::ZERO);
        let scans = Scans::default();
        let (missing, runs) = fake(Verdict::NotInstalled);
        assert_eq!(ask(&stub, OUTSIDE, &scans, &missing, 1), Some(Verdict::NotInstalled));
        assert!(stub.asked().is_empty(), "nothing went out unchecked");
        assert!(scans.missing());
        let unchecked = Reach::Outside { unchecked: true };
        assert_eq!(ask(&stub, unchecked, &scans, &missing, 1), Some(Verdict::NotInstalled));
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (1, 1));
        let (failed, runs) = fake(Verdict::Failed("broken".into()));
        ask(&stub, unchecked, &scans, &failed, 2);
        ask(&stub, unchecked, &scans, &failed, 2);
        assert_eq!((stub.asked().len(), runs.load(Ordering::SeqCst)), (1, 2));
        assert!(scans.missing(), "a failed check does not say gitleaks is back");
        let (clean, _) = fake(Verdict::Clean);
        ask(&stub, OUTSIDE, &scans, &clean, 3);
        assert!(
            !scans.missing(),
            "once gitleaks is installed, requests go out checked again"
        );
        assert_eq!(stub.asked().len(), 2);
    }
}
