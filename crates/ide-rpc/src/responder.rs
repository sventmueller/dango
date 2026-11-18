//! A request from the other side, which is answered exactly once even when its handler forgets it.

type Answer<E> = Box<dyn FnOnce(Result<serde_json::Value, E>) + Send>;

/// Answers one request from the other side; dropped unanswered, it answers with the refusal it was made with.
pub struct Responder<E> {
    answer: Option<Answer<E>>,
    unanswered: Option<E>,
}

impl<E> Responder<E> {
    /// A responder that sends its answer through `answer`, and `unanswered` when it is dropped without one.
    pub fn new(answer: impl FnOnce(Result<serde_json::Value, E>) + Send + 'static, unanswered: E) -> Responder<E> {
        Responder {
            answer: Some(Box::new(answer)),
            unanswered: Some(unanswered),
        }
    }

    /// Answers with `result`.
    pub fn ok(mut self, result: serde_json::Value) {
        self.answer(Ok(result));
    }

    /// Refuses with `refusal`.
    pub fn fail(mut self, refusal: E) {
        self.answer(Err(refusal));
    }

    fn answer(&mut self, answer: Result<serde_json::Value, E>) {
        if let Some(send) = self.answer.take() {
            send(answer);
        }
    }
}

impl<E> Drop for Responder<E> {
    fn drop(&mut self) {
        if let Some(unanswered) = self.unanswered.take() {
            self.answer(Err(unanswered));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use serde_json::json;

    use super::*;

    #[test]
    fn an_answer_goes_out_once_and_a_forgotten_request_is_refused() {
        let (tx, rx) = channel();
        let sent = tx.clone();
        Responder::new(move |answer| sent.send(answer).unwrap(), "unanswered").ok(json!(1));
        assert_eq!(rx.try_recv().unwrap(), Ok(json!(1)));
        assert!(rx.try_recv().is_err());
        drop(Responder::new(move |answer| tx.send(answer).unwrap(), "unanswered"));
        assert_eq!(rx.try_recv().unwrap(), Err("unanswered"));
    }
}
