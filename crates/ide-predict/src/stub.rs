//! A provider that answers from a script, for tests and the self-test.

use std::sync::Mutex;
use std::time::Duration;

use crate::{Cancel, Error, Prediction, Provider, Reply, Request};

/// Answers each request with the next of its scripted answers, after `delay`, on a thread of its own; once the script
/// is used up it answers with no proposals.
pub struct Stub {
    answers: Mutex<Vec<Result<Vec<Prediction>, Error>>>,
    delay: Duration,
    asked: Mutex<Vec<Request>>,
    accepted: Mutex<Vec<Prediction>>,
}

impl Stub {
    /// A provider that gives `answers` in order, each after `delay`.
    pub fn new(answers: Vec<Result<Vec<Prediction>, Error>>, delay: Duration) -> Stub {
        Stub {
            answers: Mutex::new(answers.into_iter().rev().collect()),
            delay,
            asked: Mutex::new(Vec::new()),
            accepted: Mutex::new(Vec::new()),
        }
    }

    /// The requests it was asked, in order.
    pub fn asked(&self) -> Vec<Request> {
        self.asked.lock().map(|a| a.clone()).unwrap_or_default()
    }

    /// The predictions reported as taken, in order.
    pub fn accepted_predictions(&self) -> Vec<Prediction> {
        self.accepted.lock().map(|a| a.clone()).unwrap_or_default()
    }
}

impl Provider for Stub {
    fn name(&self) -> &str {
        "Stub"
    }

    fn predict(&self, request: Request, cancel: Cancel, reply: Reply) {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(request);
        }
        let answer = self
            .answers
            .lock()
            .ok()
            .and_then(|mut a| a.pop())
            .unwrap_or(Ok(Vec::new()));
        let delay = self.delay;
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            reply(if cancel.is_cancelled() {
                Err(Error::Cancelled)
            } else {
                answer
            });
        });
    }

    fn accepted(&self, prediction: &Prediction) {
        if let Ok(mut accepted) = self.accepted.lock() {
            accepted.push(prediction.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::mpsc::channel;

    use super::*;
    use crate::Indent;

    fn request() -> Request {
        Request {
            path: PathBuf::from("/p/main.rs"),
            language: "rust".into(),
            text: "fn main() {\n    \n}\n".into(),
            revision: 3,
            selection: 16..16,
            indent: Indent::Spaces(4),
        }
    }

    #[test]
    fn the_stub_answers_in_order_off_the_caller_and_drops_a_cancelled_request() {
        let first = Prediction {
            range: 16..16,
            text: "println!();".into(),
            feedback: None,
        };
        let stub = Stub::new(vec![Ok(vec![first.clone()]), Ok(vec![first.clone()])], Duration::ZERO);
        let (tx, rx) = channel();
        let caller = std::thread::current().id();
        let sent = tx.clone();
        stub.predict(
            request(),
            Cancel::new(),
            Box::new(move |answer| {
                sent.send((answer, std::thread::current().id())).unwrap();
            }),
        );
        let (answer, on) = rx.recv().unwrap();
        assert_eq!(answer, Ok(vec![first.clone()]));
        assert_ne!(on, caller);
        let cancel = Cancel::new();
        cancel.cancel();
        stub.predict(
            request(),
            cancel,
            Box::new(move |answer| tx.send((answer, std::thread::current().id())).unwrap()),
        );
        assert_eq!(rx.recv().unwrap().0, Err(Error::Cancelled));
        stub.accepted(&first);
        assert_eq!((stub.asked().len(), stub.accepted_predictions()), (2, vec![first]));
    }
}
