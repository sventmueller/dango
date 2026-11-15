//! What the app's CI panel holds and decides without a window (E98): the phase it shows, the runs, merge requests and
//! issues it read, when it reads again, and which rows lie below which.

use std::collections::HashMap;
use std::time::Duration;

use crate::{Client, Error, Forge, Item, Job, Run, Source, Workflow};

/// The first wait before reading a running run again; it doubles up to `MAX_WAIT`.
pub const FIRST_WAIT: Duration = Duration::from_secs(5);
const MAX_WAIT: Duration = Duration::from_secs(60);
/// The longest wait for a spent rate limit before trying again.
const MAX_LIMIT_WAIT: Duration = Duration::from_secs(15 * 60);

/// What the panel can show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    /// Reading for the first time.
    Loading,
    /// The folder is not trusted, so nothing is read.
    Untrusted,
    /// Git is not installed.
    NoGit,
    /// The folder is not in a Git repository.
    NoRepository,
    /// The repository has no commit yet.
    NoCommit,
    /// The repository has no remote.
    NoRemote,
    /// The remote's host, which does not say which service it is.
    Unknown(String),
    /// The service wants a token, or refused the one it got.
    SignIn {
        /// The service.
        forge: Forge,
        /// Whether it refused a token.
        refused: bool,
    },
    /// The runs are read.
    Ready,
    /// Reading failed, with why.
    Failed(String),
}

/// What the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    /// The runs of HEAD.
    Runs,
    /// Open merge requests, pull requests on GitHub.
    Changes,
    /// Open issues.
    Issues,
}

impl Tab {
    /// Every tab, in the order the panel shows them.
    pub const ALL: [Tab; 3] = [Tab::Runs, Tab::Changes, Tab::Issues];
}

/// Names a row across reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A run.
    Run(u64),
    /// A run's stage by its place among the run's stages.
    Stage(u64, u32),
    /// A job of a run.
    Job(u64, u64),
    /// A step of a job.
    Step(u64, u64, u32),
    /// An open merge request or issue of the list's tab, by its number.
    Item(u64),
}

/// The panel's state; `L` is a sign-in through the browser while it waits.
#[derive(Debug)]
pub struct State<L> {
    /// What the panel shows.
    pub phase: Phase,
    /// The service of the remote, once known.
    pub forge: Option<Forge>,
    /// The client that read the runs.
    pub client: Option<Client>,
    /// The commit the runs belong to.
    pub head: Head,
    /// Where the token came from.
    pub source: Option<Source>,
    /// The account the token belongs to.
    pub account: Option<String>,
    /// The runs of HEAD.
    pub runs: Vec<RunRow>,
    /// The tab the list shows.
    pub tab: Tab,
    /// The open merge requests or issues, when the list shows them.
    pub items: Vec<Item>,
    /// Why they could not be read.
    pub items_problem: Option<String>,
    /// The GitHub workflows of the checkout that may be started by hand.
    pub workflows: Vec<Workflow>,
    /// A problem shown above the list.
    pub problem: Option<String>,
    /// The problem came from reading, so the next good read clears it.
    problem_from_read: bool,
    /// A re-run, start or cancel is on its way.
    pub busy: bool,
    /// A login waits for the browser.
    pub login: Option<L>,
    /// The rows the open context menu is for.
    pub menu_keys: Vec<Key>,
    /// The rows the user opened or closed.
    pub chosen: HashMap<Key, bool>,
    loading: bool,
    again: bool,
    /// Which planned read is the current one; an older one does nothing.
    planned: u64,
    wait: Duration,
    head_check: bool,
}

/// A run with its jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct RunRow {
    pub run: Run,
    /// Its jobs, if they were read.
    pub jobs: Vec<Job>,
    /// Why its jobs could not be read.
    pub problem: Option<String>,
}

/// The commit the runs belong to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Head {
    /// The commit.
    pub sha: String,
    /// The branch checked out, if any.
    pub branch: Option<String>,
}

/// What a read found.
#[derive(Debug)]
pub enum Loaded {
    /// Git is not installed.
    NoGit,
    /// The folder is not in a Git repository.
    NoRepository,
    /// The repository has no commit yet.
    NoCommit,
    /// The repository has no remote.
    NoRemote,
    /// The remote's host does not say which service it is.
    Unknown(String),
    /// The service wants a token, or refused the one it got.
    SignIn {
        /// The service.
        forge: Forge,
        /// Whether it refused a token.
        refused: bool,
    },
    /// Reading failed, on the service if it was found.
    Failed(Error, Option<Forge>),
    /// The runs were read.
    Ready(Box<Snapshot>),
}

/// The runs of HEAD on its service.
#[derive(Debug)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Snapshot {
    /// The commit the runs belong to.
    pub head: Head,
    /// The client that read them.
    pub client: Client,
    /// Where the token came from.
    pub source: Option<Source>,
    /// The account the token belongs to.
    pub account: Option<String>,
    pub runs: Vec<RunRow>,
    /// The tab the open merge requests or issues were read for.
    pub tab: Tab,
    /// The open merge requests or issues.
    pub items: Vec<Item>,
    /// Why they could not be read.
    pub items_problem: Option<String>,
    /// The GitHub workflows that may be started by hand.
    pub workflows: Vec<Workflow>,
}

impl Loaded {
    /// A missing or refused token asks to sign in; a repository that is not found without a token may be private.
    pub fn refused(error: Error, forge: Forge, had_token: bool) -> Loaded {
        match error {
            Error::Unauthorized { .. } => Loaded::SignIn {
                forge,
                refused: had_token,
            },
            Error::NotFound { .. } if !had_token => Loaded::SignIn { forge, refused: false },
            other => Loaded::Failed(other, Some(forge)),
        }
    }
}

/// What a read the user or a timer asked for leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refresh {
    /// The folder is not trusted: show that, read nothing.
    Untrusted,
    /// A read runs; another follows it.
    Queued,
    /// Read the service for `tab`, with the account found last time.
    Read {
        /// The account found last time: host, token source and name.
        known: Option<(String, Source, String)>,
        /// The tab to read for.
        tab: Tab,
    },
}

impl<L> Default for State<L> {
    fn default() -> Self {
        State {
            phase: Phase::default(),
            forge: None,
            client: None,
            head: Head::default(),
            source: None,
            account: None,
            runs: Vec::new(),
            tab: Tab::default(),
            items: Vec::new(),
            items_problem: None,
            workflows: Vec::new(),
            problem: None,
            problem_from_read: false,
            busy: false,
            login: None,
            menu_keys: Vec::new(),
            chosen: HashMap::new(),
            loading: false,
            again: false,
            planned: 0,
            wait: FIRST_WAIT,
            head_check: false,
        }
    }
}

impl<L> State<L> {
    /// A panel that has read nothing yet.
    pub fn new() -> State<L> {
        State::default()
    }

    /// Starts a read now, or right after the one that runs; `fresh` starts the wait for running runs over.
    pub fn refresh(&mut self, trusted: bool, fresh: bool) -> Refresh {
        self.planned += 1;
        if fresh {
            self.wait = FIRST_WAIT;
        }
        if !trusted {
            self.phase = Phase::Untrusted;
            self.client = None;
            self.runs.clear();
            return Refresh::Untrusted;
        }
        if self.loading {
            self.again = true;
            return Refresh::Queued;
        }
        self.loading = true;
        Refresh::Read {
            known: self.known(),
            tab: self.tab,
        }
    }

    /// The read could not start.
    pub fn refresh_not_started(&mut self) {
        self.loading = false;
    }

    /// The read is over; whether another one was asked for while it ran.
    pub fn refresh_finished(&mut self) -> bool {
        self.loading = false;
        std::mem::take(&mut self.again)
    }

    /// The account name found last time with its host and token source, so a read need not ask again.
    fn known(&self) -> Option<(String, Source, String)> {
        Some((
            self.forge.as_ref()?.host.clone(),
            self.source.clone()?,
            self.account.clone()?,
        ))
    }

    /// Takes what a read found, with `problem` wording a failure for the user; when to read again, if at all.
    pub fn apply(&mut self, loaded: Loaded, problem: impl Fn(&Error) -> String) -> Option<Duration> {
        match loaded {
            Loaded::NoGit => self.keep(Phase::NoGit, None),
            Loaded::NoRepository => self.keep(Phase::NoRepository, None),
            Loaded::NoCommit => self.keep(Phase::NoCommit, None),
            Loaded::NoRemote => self.keep(Phase::NoRemote, None),
            Loaded::Unknown(host) => self.keep(Phase::Unknown(host), None),
            Loaded::SignIn { forge, refused } => {
                let phase = Phase::SignIn {
                    forge: forge.clone(),
                    refused,
                };
                self.keep(phase, Some(forge));
            }
            Loaded::Failed(error, forge) => {
                let message = problem(&error);
                let limited = match error {
                    crate::Error::RateLimited { until } => Some(until),
                    _ => None,
                };
                if self.phase == Phase::Ready {
                    self.problem = Some(message);
                    self.problem_from_read = true;
                } else {
                    self.keep(Phase::Failed(message), forge);
                }
                return limited.map(|until| {
                    let seconds = until.map_or(60, |u| (u - crate::now()).max(5));
                    Duration::from_secs(u64::try_from(seconds).unwrap_or(60)).min(MAX_LIMIT_WAIT)
                });
            }
            Loaded::Ready(snapshot) => {
                if std::mem::take(&mut self.problem_from_read) {
                    self.problem = None;
                }
                self.phase = Phase::Ready;
                self.forge = Some(snapshot.client.forge().clone());
                self.client = Some(snapshot.client);
                self.head = snapshot.head;
                self.source = snapshot.source;
                self.account = snapshot.account;
                self.runs = snapshot.runs;
                if snapshot.tab == self.tab {
                    self.items = snapshot.items;
                    self.items_problem = snapshot.items_problem;
                }
                self.workflows = snapshot.workflows;
                return self.next_wait();
            }
        }
        None
    }

    /// Shows `phase` with nothing read.
    fn keep(&mut self, phase: Phase, forge: Option<Forge>) {
        self.phase = phase;
        self.forge = forge;
        self.client = None;
        self.runs.clear();
        self.items.clear();
        self.items_problem = None;
    }

    /// How long until the runs are read again: never while none runs, else a wait that doubles each time.
    fn next_wait(&mut self) -> Option<Duration> {
        let active = self
            .runs
            .iter()
            .any(|r| r.run.state.is_active() || r.jobs.iter().any(|j| j.state.is_active()));
        if !active {
            self.wait = FIRST_WAIT;
            return None;
        }
        let wait = self.wait;
        self.wait = (wait * 2).min(MAX_WAIT);
        Some(wait)
    }

    /// Plans a read; its number, which an earlier plan does not have.
    pub fn plan(&mut self) -> u64 {
        self.planned += 1;
        self.planned
    }

    /// Whether the read planned as `planned` is still the current one.
    pub fn is_planned(&self, planned: u64) -> bool {
        self.planned == planned
    }

    /// Notes that something in the folder changed; whether HEAD needs reading, which it does not while a read of it
    /// is planned or the folder is not trusted.
    pub fn changed(&mut self) -> bool {
        if self.head_check || self.phase == Phase::Untrusted {
            return false;
        }
        self.head_check = true;
        true
    }

    /// Notes what HEAD is now, if it could be read; whether the runs need reading again.
    pub fn head_read(&mut self, head: Option<Head>) -> bool {
        self.head_check = false;
        head.is_some_and(|h| h != self.head) || self.phase != Phase::Ready
    }

    /// HEAD could not be read.
    pub fn head_check_failed(&mut self) {
        self.head_check = false;
    }

    /// Shows `problem`, one the user caused rather than a read.
    pub fn set_problem(&mut self, problem: Option<String>) {
        self.problem = problem;
        self.problem_from_read = false;
    }

    /// Lists `tab`; whether it was another one, so the panel reads what it lists.
    pub fn choose_tab(&mut self, tab: Tab) -> bool {
        if self.tab == tab {
            return false;
        }
        self.tab = tab;
        self.items.clear();
        self.items_problem = None;
        true
    }

    /// The rows below `parent`: runs at the top, then GitLab's stages, then jobs, then steps; or the open merge
    /// requests or issues.
    pub fn children(&self, parent: Option<Key>) -> Vec<Key> {
        let run_of = |run: u64| self.runs.iter().find(|r| r.run.id == run);
        match parent {
            None if self.tab != Tab::Runs => self.items.iter().map(|i| Key::Item(i.number)).collect(),
            None => self.runs.iter().map(|r| Key::Run(r.run.id)).collect(),
            Some(Key::Run(run)) => run_of(run)
                .map(|r| match crate::stages(&r.jobs).len() {
                    0 => r.jobs.iter().map(|j| Key::Job(run, j.id)).collect(),
                    stages => (0..u32::try_from(stages).unwrap_or(0))
                        .map(|stage| Key::Stage(run, stage))
                        .collect(),
                })
                .unwrap_or_default(),
            Some(Key::Stage(run, stage)) => run_of(run)
                .and_then(|r| {
                    let stages = crate::stages(&r.jobs);
                    let jobs = &stages.get(usize::try_from(stage).ok()?)?.jobs;
                    Some(jobs.iter().map(|j| Key::Job(run, j.id)).collect())
                })
                .unwrap_or_default(),
            Some(Key::Job(run, job)) => run_of(run)
                .and_then(|r| r.jobs.iter().find(|j| j.id == job))
                .map(|j| j.steps.iter().map(|s| Key::Step(run, job, s.number)).collect())
                .unwrap_or_default(),
            Some(_) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_asked_for_while_one_runs_follows_it() {
        let mut state = State::<()>::new();
        assert_eq!(
            state.refresh(true, true),
            Refresh::Read {
                known: None,
                tab: Tab::Runs
            }
        );
        assert_eq!(state.refresh(true, false), Refresh::Queued);
        assert!(state.refresh_finished());
        assert!(!state.refresh_finished());
    }

    #[test]
    fn an_untrusted_folder_reads_nothing_and_checks_no_head() {
        let mut state = State::<()>::default();
        assert_eq!(state.refresh(false, false), Refresh::Untrusted);
        assert_eq!(state.phase, Phase::Untrusted);
        assert!(!state.changed());
    }

    #[test]
    fn only_the_last_plan_reads() {
        let mut state = State::<()>::default();
        let first = state.plan();
        let second = state.plan();
        assert!(!state.is_planned(first));
        assert!(state.is_planned(second));
        state.refresh(false, false);
        assert!(!state.is_planned(second));
    }

    #[test]
    fn head_is_read_once_until_it_was() {
        let mut state = State::<()>::default();
        assert!(state.changed());
        assert!(!state.changed());
        assert!(state.head_read(None));
        state.phase = Phase::Ready;
        assert!(state.changed());
        assert!(!state.head_read(Some(Head::default())));
        assert!(state.changed());
        let moved = Head {
            sha: "abc".into(),
            branch: None,
        };
        assert!(state.head_read(Some(moved)));
    }

    #[test]
    fn a_failed_read_after_a_good_one_keeps_the_runs_and_a_good_read_clears_its_problem() {
        let mut state = State::<()>::default();
        assert_eq!(state.apply(Loaded::NoRemote, |e| e.to_string()), None);
        assert_eq!(state.phase, Phase::NoRemote);
        state.phase = Phase::Ready;
        let wait = state.apply(Loaded::Failed(crate::Error::RateLimited { until: None }, None), |e| {
            e.to_string()
        });
        assert_eq!(wait, Some(Duration::from_secs(60)));
        assert_eq!(state.phase, Phase::Ready);
        assert!(state.problem.is_some());
        state.set_problem(Some("mine".into()));
        state.apply(Loaded::Failed(crate::Error::Decode("x".into()), None), |e| {
            e.to_string()
        });
        assert_ne!(state.problem.as_deref(), Some("mine"));
    }

    #[test]
    fn a_refused_or_missing_token_asks_to_sign_in_and_other_failures_stay_failures() {
        let forge = crate::candidates("https://github.com/o/r.git", &[]).remove(0);
        let unauthorized = || Error::Unauthorized {
            host: "github.com".into(),
        };
        let not_found = || Error::NotFound { what: "r".into() };
        let asks = |loaded: Loaded| match loaded {
            Loaded::SignIn { refused, .. } => Some(refused),
            _ => None,
        };
        assert_eq!(asks(Loaded::refused(unauthorized(), forge.clone(), true)), Some(true));
        assert_eq!(asks(Loaded::refused(unauthorized(), forge.clone(), false)), Some(false));
        assert_eq!(asks(Loaded::refused(not_found(), forge.clone(), false)), Some(false));
        assert!(matches!(
            Loaded::refused(not_found(), forge.clone(), true),
            Loaded::Failed(Error::NotFound { .. }, Some(_))
        ));
        assert!(matches!(
            Loaded::refused(Error::Decode("x".into()), forge, false),
            Loaded::Failed(Error::Decode(_), Some(_))
        ));
    }

    #[test]
    fn another_tab_empties_the_list_until_it_is_read() {
        let mut state = State::<()>::default();
        assert!(!state.choose_tab(Tab::Runs));
        state.items_problem = Some("x".into());
        assert!(state.choose_tab(Tab::Issues));
        assert!(state.items_problem.is_none());
        assert!(state.children(None).is_empty());
        assert!(state.children(Some(Key::Run(1))).is_empty());
    }
}
