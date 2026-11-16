//! The sessions of orders (E165, E173): one `ide_session::Registry` for all of them, started in each order's bounds,
//! and what their drivers report mapped into the factory's own terms: the typed end of each turn with the prompt it
//! answers, requests left to the user, the session's activity for the stall rule (E168), its usage for the budgets and
//! its quota windows (C64, C65), gaps in its stream (C62) and the end of its process.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ide_factory::budget::UsageReport;
use ide_factory::event::TurnOutcome;
use ide_factory::input::RequestKind;
use ide_factory::quota::{QuotaKey, QuotaObservation, QuotaStatus};
use ide_factory::routing::{Agent, RankEntry, Readiness};
use ide_factory::{InputKind, OrderId, PromptId, RequestId, SessionEvent, SessionKey, Stamp};
use ide_process::Supervisor;
use ide_session::{
    AgentPath, Contract, Driver, Launch, LaunchError, Outcome, Prompt, Registry, SendError, SessionId, Signal,
    SubscriberId, TurnId,
};
use ide_thread::{BackgroundKind, EntryKind, Lane, LimitStatus, Status, Thread, ToolStatus};
use splat_i18n::{t, tf};

use super::{Back, Delivery, Leitstand};
use crate::context::AppContext;

/// How long the hop to the main thread waits, so a burst of events from a session is taken in at once.
const COALESCE: Duration = Duration::from_millis(30);

/// How long the git of a session's checkpoints may take altogether; a session lives that long at most.
const CHECKPOINT_DEADLINE: Duration = Duration::from_secs(24 * 60 * 60);

/// The program a session starts, as looked for on the login shell's PATH.
pub(super) enum Program {
    /// Claude Code.
    Claude(PathBuf),
    /// Codex with an app server of its own (E193).
    Codex(PathBuf),
    /// The scripted session of the self-test (F61).
    #[cfg(feature = "selftest")]
    Script(ide_session::Script),
    /// The path is not there, or the Leitstand does not start it.
    Missing,
}

/// A session ready to launch on the main thread, after its program and environment were looked for.
pub(super) struct Ready {
    pub(super) root: PathBuf,
    pub(super) order: OrderId,
    pub(super) title: String,
    pub(super) role: ide_factory::Role,
    pub(super) entry: RankEntry,
    pub(super) resume: Option<String>,
    pub(super) worktree: PathBuf,
    pub(super) program: Program,
    /// The login shell's environment.
    pub(super) login: Vec<(String, String)>,
    /// What the order's sessions get of it (E186).
    pub(super) env: Vec<(String, String)>,
    pub(super) network: Vec<String>,
}

/// What links a session of the registry to its order.
pub(super) struct Link {
    pub(super) root: PathBuf,
    pub(super) order: OrderId,
    pub(super) title: String,
    pub(super) key: SessionKey,
    pub(super) role: ide_factory::Role,
    agent: Agent,
    subscriber: SubscriberId,
    /// The factory's prompt of each turn the driver took.
    prompts: BTreeMap<TurnId, PromptId>,
    /// The requests left to the user, by the agent's id.
    requests: BTreeMap<String, ide_session::RequestKind>,
    started: bool,
    resume: Option<String>,
    activity: Option<Activity>,
    usage: Option<UsageReport>,
    limits: Vec<ide_thread::RateLimit>,
}

/// What a session shows for the stall rule, compared so only a change goes to the factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Activity {
    running: bool,
    tool_since: Option<u64>,
    subagents: u32,
    background: u32,
    queued: u32,
}

/// The registry and what links its sessions to orders.
#[derive(Default)]
pub(super) struct Sessions {
    registry: Option<Registry>,
    links: BTreeMap<SessionId, Link>,
}

/// What a flush of one session leads to.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Mapped {
    pub(super) inputs: Vec<InputKind>,
    pub(super) quotas: Vec<QuotaObservation>,
    /// The session ended, or never started: the registry lets it go.
    pub(super) ended: bool,
}

/// What the mapping reads of a driver.
pub(super) struct View<'a> {
    pub(super) thread: &'a Thread,
    pub(super) queued: usize,
    pub(super) session_id: Option<&'a str>,
    pub(super) now: Stamp,
}

impl Sessions {
    fn registry(&mut self, back: &Back) -> &mut Registry {
        self.registry.get_or_insert_with(|| {
            let back = back.clone();
            let wake: Arc<dyn ide_session::Wake> =
                Arc::new(move |session: SessionId| back.deliver_after(COALESCE, Delivery::Flush(session)));
            Registry::new(Supervisor::global().clone(), wake)
        })
    }

    /// The session of `key`.
    fn find(&self, key: &SessionKey) -> Option<SessionId> {
        self.links.iter().find(|(_, link)| link.key == *key).map(|(id, _)| *id)
    }

    /// The links of the sessions that work now, oldest first (C58).
    pub(super) fn working(&self) -> Vec<&Link> {
        let Some(registry) = &self.registry else {
            return Vec::new();
        };
        registry.working().iter().filter_map(|id| self.links.get(id)).collect()
    }

    /// Whether a session of an order works in `folder` or below it.
    pub(super) fn works_in(&self, folder: &Path) -> bool {
        self.registry.as_ref().is_some_and(|r| !r.working_in(folder).is_empty())
    }

    /// The session of `key` with its driver, to stop its turn from the activity view.
    pub(super) fn driver(&mut self, key: &SessionKey) -> Option<&mut Driver> {
        let id = self.find(key)?;
        self.registry.as_mut()?.get_mut(id)
    }

    /// Every session's process ends, as the app quits.
    pub(super) fn stop_all(&mut self) {
        if let Some(registry) = self.registry.as_mut() {
            registry.stop_all();
        }
    }
}

/// The session's role in `ide_session`'s terms.
fn role_of(role: ide_factory::Role) -> ide_session::Role {
    match role {
        ide_factory::Role::Planner => ide_session::Role::Planner,
        ide_factory::Role::Builder => ide_session::Role::Builder,
        ide_factory::Role::Reviewer => ide_session::Role::Reviewer,
        ide_factory::Role::Scout => ide_session::Role::Scout,
    }
}

/// Why a session did not start, in the user's words, as routing records it (E170, E141).
fn readiness_of(error: &LaunchError) -> Readiness {
    match error {
        LaunchError::NotFound(_) => Readiness::NotInstalled,
        LaunchError::NoRole(_) => Readiness::StartFailed(t("Dieser Agent übernimmt in Aufträgen keine Rolle.").into()),
        LaunchError::Unsupported { .. } => {
            Readiness::StartFailed(t("Dieser Agent kann die Grenzen des Auftrags nicht einhalten.").into())
        }
        LaunchError::Folder { .. } => {
            Readiness::StartFailed(t("Die Sitzung eines Auftrags arbeitet nur in seinem Worktree.").into())
        }
        LaunchError::Start(why) => Readiness::StartFailed(tf("Die Sitzung ist nicht gestartet: {}", &[why])),
    }
}

impl Leitstand {
    /// Starts the session `ready` describes in the order's bounds (E173); without O1, what reaches beyond them goes
    /// to the user rather than being refused (C72).
    pub(super) fn launch(&self, cx: &Rc<AppContext>, ready: Ready) {
        let failed = |readiness| InputKind::SessionFailed {
            order: ready.order.clone(),
            role: ready.role,
            readiness,
        };
        let path = match ready.program {
            Program::Claude(program) => AgentPath::Claude { program },
            Program::Codex(program) => AgentPath::Codex { program, version: None },
            #[cfg(feature = "selftest")]
            Program::Script(script) => AgentPath::Scripted(script),
            Program::Missing => return self.input(cx, &ready.root, failed(Readiness::NotInstalled)),
        };
        let contract = Contract::Order(ide_session::Order {
            order: ready.order.0.clone(),
            role: role_of(ready.role),
            worktree: ready.worktree.clone(),
            build_dir: None,
            read_roots: Vec::new(),
            allowed_domains: ready.network.clone(),
            refuse_outside: false,
        });
        let git = ide_git::Git::new(Supervisor::global(), ready.login.clone())
            .unattended(ready.env.clone(), Instant::now() + CHECKPOINT_DEADLINE);
        let mut launch = Launch::new(path, &ready.worktree, ready.env.clone(), contract.clone());
        launch.model.clone_from(&ready.entry.model);
        launch.effort.clone_from(&ready.entry.effort);
        launch.resume.clone_from(&ready.resume);
        launch.git = Some(git);
        let started = {
            let mut sessions = self.sessions.borrow_mut();
            let registry = sessions.registry(&self.back());
            registry
                .start(launch, ide_session::rule::for_contract(&contract))
                .map(|id| (id, registry.get_mut(id).map(Driver::subscribe)))
        };
        match started {
            Ok((id, Some(subscriber))) => {
                let link = Link {
                    root: ready.root,
                    order: ready.order,
                    title: ready.title,
                    key: SessionKey(format!("{}-{id}", self.clock.run())),
                    role: ready.role,
                    agent: ready.entry.agent,
                    subscriber,
                    prompts: BTreeMap::new(),
                    requests: BTreeMap::new(),
                    started: false,
                    resume: ready.resume,
                    activity: None,
                    usage: None,
                    limits: Vec::new(),
                };
                self.sessions.borrow_mut().links.insert(id, link);
            }
            Ok((_, None)) => self.input(cx, &ready.root, failed(Readiness::StartFailed(String::new()))),
            Err(error) => self.input(cx, &ready.root, failed(readiness_of(&error))),
        }
    }

    /// Takes in what session `id` reported and hands it to its order's factory.
    pub(super) fn flush(&self, cx: &Rc<AppContext>, id: SessionId) {
        let now = self.clock.now();
        let (root, mapped) = {
            let mut sessions = self.sessions.borrow_mut();
            let Sessions { registry, links } = &mut *sessions;
            let Some(registry) = registry.as_mut() else { return };
            registry.flush(id);
            let (Some(link), Some(driver)) = (links.get_mut(&id), registry.get_mut(id)) else {
                return;
            };
            let signals = driver
                .take(link.subscriber)
                .map(|batch| batch.signals)
                .unwrap_or_default();
            let view = View {
                thread: driver.thread(),
                queued: driver.queued(),
                session_id: driver.session_id(),
                now,
            };
            (link.root.clone(), map(link, &signals, &view))
        };
        if mapped.ended {
            let mut sessions = self.sessions.borrow_mut();
            sessions.links.remove(&id);
            if let Some(registry) = sessions.registry.as_mut() {
                registry.remove(id);
            }
        }
        for observation in mapped.quotas {
            self.observe_quota(cx, observation);
        }
        for input in mapped.inputs {
            self.input(cx, &root, input);
        }
        self.refresh_activity(cx, &root);
    }

    /// Sends a prompt; the turn it starts ends with the same prompt id.
    pub(super) fn send_prompt(
        &self,
        cx: &Rc<AppContext>,
        root: &Path,
        key: &SessionKey,
        prompt: PromptId,
        text: String,
        schema: Option<String>,
    ) {
        let order = {
            let mut sessions = self.sessions.borrow_mut();
            let Some(id) = sessions.find(key) else { return };
            let order = sessions.links.get(&id).map(|l| l.order.clone());
            let schema = schema.and_then(|s| serde_json::from_str(&s).ok());
            let Some(driver) = sessions.registry.as_mut().and_then(|r| r.get_mut(id)) else {
                return;
            };
            let mut message = Prompt::new(text);
            message.schema = schema;
            let sent = match driver.send(message.clone()) {
                Err(SendError::NoSchema(_)) => {
                    message.schema = None;
                    driver.send(message)
                }
                other => other,
            };
            match sent {
                Ok(turn) => {
                    if let Some(link) = sessions.links.get_mut(&id) {
                        link.prompts.insert(turn, prompt);
                    }
                    return;
                }
                Err(_) => order,
            }
        };
        if let Some(order) = order {
            let ended = InputKind::Session(order, key.clone(), SessionEvent::Exited);
            self.input(cx, root, ended);
        }
    }

    /// Stops what `stop` says of the session `key`.
    pub(super) fn stop_session(&self, key: &SessionKey, stop: Stop) {
        let mut sessions = self.sessions.borrow_mut();
        let Some(id) = sessions.find(key) else { return };
        match stop {
            Stop::Turn => {
                if let Some(driver) = sessions.registry.as_mut().and_then(|r| r.get_mut(id)) {
                    driver.stop_turn();
                }
            }
            Stop::Background => {
                if let Some(driver) = sessions.registry.as_mut().and_then(|r| r.get_mut(id)) {
                    driver.stop_background();
                }
            }
            Stop::Session => {
                sessions.links.remove(&id);
                if let Some(registry) = sessions.registry.as_mut() {
                    registry.remove(id);
                }
            }
        }
    }

    /// Answers the request `request` of session `key` with `answer`; true when it reached the agent.
    pub(super) fn answer(&self, key: &SessionKey, request: &RequestId, answer: &ide_factory::effect::Answer) -> bool {
        let mut sessions = self.sessions.borrow_mut();
        let Some(id) = sessions.find(key) else { return false };
        let kind = sessions
            .links
            .get(&id)
            .and_then(|l| l.requests.get(&request.0).copied());
        let Some(driver) = sessions.registry.as_mut().and_then(|r| r.get_mut(id)) else {
            return false;
        };
        let (text, refuse) = match answer {
            ide_factory::effect::Answer::Text(text) => (text.clone(), false),
            ide_factory::effect::Answer::Refuse(why) => (why.clone(), true),
        };
        let answered = match kind {
            Some(ide_session::RequestKind::Question) | None => {
                let answers = questions(driver.thread(), &request.0)
                    .into_iter()
                    .map(|question| ide_thread::Answer {
                        question,
                        answer: text.clone(),
                    })
                    .collect();
                driver.answer_questions(&request.0, answers)
            }
            Some(ide_session::RequestKind::Permission) => driver.answer_permission(&request.0, false, Some(&text)),
            Some(ide_session::RequestKind::Plan) if refuse => {
                driver.answer_plan(&request.0, ide_session::PlanReply::Revise(Some(text)))
            }
            Some(ide_session::RequestKind::Plan) => driver.answer_plan(&request.0, ide_session::PlanReply::Approve),
        };
        if let Err(error) = &answered {
            ide_core::diag::report("answering an order's request", error);
        }
        answered.is_ok()
    }

    /// Allows or refuses a permission request of session `key` the user decides, which the factory hears of as
    /// answered (C72).
    pub(super) fn decide(&self, key: &SessionKey, request: &str, allow: bool) -> bool {
        let mut sessions = self.sessions.borrow_mut();
        let Some(id) = sessions.find(key) else { return false };
        let Some(driver) = sessions.registry.as_mut().and_then(|r| r.get_mut(id)) else {
            return false;
        };
        driver.answer_permission(request, allow, None).is_ok()
    }
}

/// What to stop of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop {
    Turn,
    Background,
    Session,
}

/// The questions of the request `request` that waits in `thread`.
fn questions(thread: &Thread, request: &str) -> Vec<String> {
    thread
        .pending_requests()
        .into_iter()
        .find_map(|entry| match &entry.kind {
            EntryKind::Question(q) if q.request_id.as_deref() == Some(request) => {
                Some(q.items.iter().map(|item| item.question.clone()).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// What `signals` and the session's state mean for the factory, with what `link` remembers between flushes. Nothing
/// of a session reaches the factory before it reported its start.
pub(super) fn map(link: &mut Link, signals: &[Signal], view: &View<'_>) -> Mapped {
    let mut mapped = Mapped::default();
    let mut events = Vec::new();
    if link.started {
        events.extend(changed_state(link, view));
    }
    for signal in signals {
        match signal {
            Signal::Ready if !link.started => {
                link.started = true;
                let resume = view.session_id.map(str::to_string).or_else(|| link.resume.clone());
                mapped.inputs.push(InputKind::SessionStarted {
                    order: link.order.clone(),
                    role: link.role,
                    session: link.key.clone(),
                    resume,
                });
                events.extend(changed_state(link, view));
            }
            Signal::TurnEnded(end) => events.push(SessionEvent::TurnEnded {
                prompt: link.prompts.remove(&end.turn),
                outcome: outcome_of(&end.outcome),
                answer: last_answer(view.thread),
            }),
            Signal::Waiting { request, kind } => {
                link.requests.insert(request.clone(), *kind);
                events.push(SessionEvent::Request {
                    request: RequestId(request.clone()),
                    kind: match kind {
                        ide_session::RequestKind::Question => RequestKind::Question,
                        ide_session::RequestKind::Permission => RequestKind::OutsideBounds,
                        ide_session::RequestKind::Plan => RequestKind::Plan,
                    },
                });
            }
            Signal::Resynced => events.push(SessionEvent::Gap { reconciled: true }),
            Signal::NeedsPause(_) => events.push(SessionEvent::Gap { reconciled: false }),
            Signal::Failed(why) | Signal::Ended { stderr: why } if !link.started => {
                mapped.ended = true;
                mapped.inputs.push(InputKind::SessionFailed {
                    order: link.order.clone(),
                    role: link.role,
                    readiness: not_started(why),
                });
                return mapped;
            }
            Signal::Ended { .. } => {
                mapped.ended = true;
                events.push(SessionEvent::Exited);
            }
            _ => {}
        }
    }
    mapped.quotas = changed_limits(link, view);
    let (order, key) = (link.order.clone(), link.key.clone());
    let session = |event| InputKind::Session(order.clone(), key.clone(), event);
    mapped.inputs.extend(events.into_iter().map(session));
    mapped
}

/// Why a session ended before it started, in the user's words.
fn not_started(why: &str) -> Readiness {
    if why.trim().is_empty() {
        Readiness::StartFailed(t("Die Sitzung hat sich vor dem Start beendet.").into())
    } else {
        Readiness::StartFailed(tf("Die Sitzung ist nicht gestartet: {}", &[why.trim()]))
    }
}

/// The session's activity and usage where they changed.
fn changed_state(link: &mut Link, view: &View<'_>) -> Vec<SessionEvent> {
    let mut events: Vec<SessionEvent> = changed_activity(link, view).into_iter().collect();
    if let Some(report) = changed_usage(link, view.thread) {
        events.push(SessionEvent::Usage { report });
    }
    events
}

fn outcome_of(outcome: &Outcome) -> TurnOutcome {
    match outcome {
        Outcome::Succeeded => TurnOutcome::Succeeded,
        Outcome::Failed(why) => TurnOutcome::Failed(why.clone()),
        Outcome::Cancelled => TurnOutcome::Cancelled,
        Outcome::ConnectionLost => TurnOutcome::ConnectionLost,
    }
}

/// The agent's last answer on the main timeline, for the planner and the reviewer.
fn last_answer(thread: &Thread) -> Option<String> {
    thread
        .entries(&Lane::Main)
        .iter()
        .rev()
        .find_map(|entry| match &entry.kind {
            EntryKind::AgentMessage(message) => Some(message.markdown.clone()),
            _ => None,
        })
}

fn changed_activity(link: &mut Link, view: &View<'_>) -> Option<SessionEvent> {
    let thread = view.thread;
    let tool_since = thread
        .entries(&Lane::Main)
        .iter()
        .filter_map(|entry| match &entry.kind {
            EntryKind::ToolCall(call) if matches!(call.status, ToolStatus::InProgress | ToolStatus::Pending) => {
                call.started
            }
            _ => None,
        })
        .min()
        .map(|started| {
            let wall = started
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            view.now.mono.0.saturating_sub(view.now.wall.saturating_sub(wall))
        });
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let activity = Activity {
        running: thread.status() == Status::Running,
        tool_since,
        subagents: count(thread.subagents().iter().filter(|s| !s.status.is_final()).count()),
        background: count(
            thread
                .background()
                .iter()
                .filter(|b| !b.status.is_final() && !b.ambient && b.kind != BackgroundKind::Subagent)
                .count(),
        ),
        queued: count(view.queued),
    };
    if link.activity == Some(activity) {
        return None;
    }
    link.activity = Some(activity);
    Some(SessionEvent::Activity {
        running: activity.running,
        tool_since: activity.tool_since.map(ide_factory::Millis),
        subagents: activity.subagents,
        background: activity.background,
        queued: activity.queued,
    })
}

/// The session's usage, when it changed: dollars only where the user pays them, not on a subscription (E57).
fn changed_usage(link: &mut Link, thread: &Thread) -> Option<UsageReport> {
    let usage = thread.usage();
    let tokens = usage.input_tokens + usage.output_tokens + usage.cache_read_tokens;
    let report = UsageReport {
        dollars: usage.cost_usd.filter(|_| !usage.subscription),
        tokens: (tokens > 0).then_some(tokens),
        tokens_reliable: matches!(link.agent, Agent::Claude | Agent::Codex | Agent::Own),
    };
    if report == UsageReport::default() || link.usage == Some(report) {
        return None;
    }
    link.usage = Some(report);
    Some(report)
}

/// The billing service of a session's agent, by whether it runs on a subscription (E171, C65).
pub(super) fn service(agent: &Agent, subscription: bool) -> String {
    format!("{agent}-{}", if subscription { "subscription" } else { "api" })
}

/// The quota windows the session reported anew.
fn changed_limits(link: &mut Link, view: &View<'_>) -> Vec<QuotaObservation> {
    let limits = view.thread.rate_limits();
    if limits == link.limits.as_slice() {
        return Vec::new();
    }
    link.limits = limits.to_vec();
    let service = service(&link.agent, view.thread.usage().subscription);
    limits
        .iter()
        .map(|limit| QuotaObservation {
            key: QuotaKey {
                account: link.agent.to_string(),
                service: service.clone(),
                window: limit.window.clone(),
            },
            utilization: limit.utilization,
            status: match limit.status {
                LimitStatus::Allowed => QuotaStatus::Allowed,
                LimitStatus::Warning => QuotaStatus::Warning,
                LimitStatus::Rejected => QuotaStatus::Rejected,
            },
            window_seconds: window_seconds(&limit.window),
            resets_at: limit.resets_at.and_then(|at| {
                at.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .and_then(|d| u64::try_from(d.as_millis()).ok())
            }),
            observed_at: view.now.wall,
        })
        .collect()
}

/// How long a window of this name lasts; a day for one the name does not tell.
fn window_seconds(window: &str) -> u64 {
    let hour = 60 * 60;
    match window {
        w if w.contains("five_hour") || w.contains("5h") => 5 * hour,
        w if w.contains("seven_day") || w.contains("week") => 7 * 24 * hour,
        w if w.contains("hour") => hour,
        _ => 24 * hour,
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
