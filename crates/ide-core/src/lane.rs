//! Worker threads with a limit on waiting jobs, and code run so that a panic costs one result and not the app.

use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};

type Job = Box<dyn FnOnce() + Send>;

/// Threads that run jobs one after the other, started by the first job. A job that panics ends only itself.
pub struct Lane {
    name: &'static str,
    workers: usize,
    capacity: usize,
    jobs: OnceLock<SyncSender<Job>>,
}

/// The lane already holds as many waiting jobs as it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full;

impl Lane {
    /// A lane of `workers` threads called `name` that holds up to `capacity` waiting jobs.
    pub const fn new(name: &'static str, workers: usize, capacity: usize) -> Lane {
        Lane {
            name,
            workers,
            capacity,
            jobs: OnceLock::new(),
        }
    }

    /// Queues `job`; `Full` when `capacity` jobs are already waiting, so the caller can drop or merge work instead
    /// of piling it up.
    pub fn try_run(&self, job: impl FnOnce() + Send + 'static) -> Result<(), Full> {
        match self.sender().try_send(Box::new(job)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => Err(Full),
        }
    }

    /// Runs `work` on the lane and hands its result, or `None` after a panic, to `answer` there; `Full` as `try_run`.
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    ///
    /// use ide_core::lane::Lane;
    ///
    /// static LANE: Lane = Lane::new("example", 1, 8);
    /// let (tx, rx) = channel();
    /// let failed = tx.clone();
    /// LANE.ask(|| 6 * 7, move |answer| tx.send(answer).unwrap()).unwrap();
    /// LANE.ask(|| -> i32 { panic!("lost") }, move |answer| failed.send(answer).unwrap()).unwrap();
    /// assert_eq!(rx.recv().unwrap(), Some(42));
    /// assert_eq!(rx.recv().unwrap(), None);
    /// ```
    pub fn ask<T>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        answer: impl FnOnce(Option<T>) + Send + 'static,
    ) -> Result<(), Full> {
        self.try_run(move || answer(contained(work)))
    }

    fn sender(&self) -> &SyncSender<Job> {
        self.jobs.get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(self.capacity);
            let rx = Arc::new(Mutex::new(rx));
            for n in 0..self.workers.max(1) {
                let rx = rx.clone();
                let name = if self.workers <= 1 {
                    self.name.to_string()
                } else {
                    format!("{}-{n}", self.name)
                };
                std::thread::Builder::new()
                    .name(name)
                    .spawn(move || {
                        while let Ok(Ok(job)) = rx.lock().map(|rx| rx.recv()) {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                        }
                    })
                    .expect("lane thread");
            }
            tx
        })
    }
}

/// Runs `f`; `None` if it panicked.
pub fn contained<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

/// Reading and writing the user's files: a network volume that stops answering may hold it for minutes.
pub static DISK: Lane = Lane::new("ide-disk", 1, 256);

/// Computation off the main thread.
pub static WORK: Lane = Lane::new("ide-work", 3, 256);

#[cfg(test)]
mod tests {
    use std::sync::Barrier;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_panicking_job_ends_only_itself() {
        static LANE: Lane = Lane::new("test-panic", 1, 4);
        let (tx, rx) = channel();
        LANE.ask(|| panic!("in the job"), {
            let tx = tx.clone();
            move |r: Option<()>| tx.send(r.is_none()).unwrap()
        })
        .unwrap();
        LANE.ask(|| 7, move |r| tx.send(r == Some(7)).unwrap()).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap());
    }

    #[test]
    fn a_full_lane_refuses_instead_of_growing() {
        static LANE: Lane = Lane::new("test-full", 1, 2);
        let gate = Arc::new(Barrier::new(2));
        let held = gate.clone();
        LANE.try_run(move || {
            held.wait();
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        LANE.try_run(|| ()).unwrap();
        LANE.try_run(|| ()).unwrap();
        assert_eq!(LANE.try_run(|| ()), Err(Full));
        gate.wait();
    }
}
