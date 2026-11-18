//! A stand-in for the app: it hands the factory inputs at a set clock, keeps each order's log as the app would, and
//! answers effects the way a healthy repository and agent would, unless a test scripts otherwise or holds them.

#![allow(dead_code, reason = "each test file uses its own part of the harness")]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ide_factory::budget::UsageReport;
use ide_factory::effect::Effect;
use ide_factory::event::{BaseRef, OpResult, TurnOutcome};
use ide_factory::evidence::Manifest;
use ide_factory::gates::{CommandOutcome, SecretScan, TestCount};
use ide_factory::input::RefChange;
use ide_factory::log::{LogEntry, LogFailure, read};
use ide_factory::options::NewOrder;
use ide_factory::relevance::{Change, ChangeKind};
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::workflow::rules::{Confirmations, Rules};
use ide_factory::workflow::{UserDefaults, parse};
use ide_factory::{
    Condition, Factory, Input, InputKind, Order, OrderId, SessionEvent, SessionKey, Stage, Stamp, Station,
};

/// The workflow file most tests run under: one gate.
pub(crate) const WORKFLOW: &str = "[[gates.command]]\nname = \"check\"\nrun = \"make check\"\n";

/// Rules from `defaults` and the workflow `text`, with the gate command set confirmed.
pub(crate) fn rules_with(defaults: &UserDefaults, text: &str) -> Rules {
    let workflow = parse(text).expect("the test's workflow file parses").workflow;
    let first = Rules::new(defaults, Some(&workflow), &Confirmations::default());
    let confirmed = Confirmations::new([first.gate_hash().to_string()]);
    Rules::new(defaults, Some(&workflow), &confirmed)
}

/// Rules from the default settings and `text`.
pub(crate) fn rules(text: &str) -> Rules {
    rules_with(&UserDefaults::default(), text)
}

/// A verdict that meets every criterion of `order`.
pub(crate) fn passing_verdict(order: &Order) -> String {
    let items: Vec<String> = order
        .criteria()
        .iter()
        .map(|c| format!(r#"{{"id":"{}","verdict":"met","reason":"done"}}"#, c.id()))
        .collect();
    format!(r#"{{"criteria":[{}],"findings":[]}}"#, items.join(","))
}

/// The planner's default answer.
pub(crate) const SPEC: &str = "# Spec\n\n## Criteria\n- exports CSV\n\n## Path scope\n- src/**\n\n## Risk\nlow\n";

/// The app.
pub(crate) struct App {
    pub(crate) factory: Factory,
    pub(crate) logs: BTreeMap<OrderId, String>,
    pub(crate) effects: Vec<Effect>,
    pub(crate) pending: VecDeque<Effect>,
    pub(crate) refusals: Vec<ide_factory::Refusal>,
    pub(crate) mono: u64,
    pub(crate) wall: u64,
    /// Effect kinds the harness leaves for the test to answer, by their serde tag such as `send_prompt`.
    pub(crate) hold: BTreeSet<&'static str>,
    /// How the next turns end, oldest first; success when empty.
    pub(crate) turns: VecDeque<TurnOutcome>,
    /// The reviewer's next answers; a passing verdict when empty.
    pub(crate) verdicts: VecDeque<String>,
    /// The next gate outcomes; a pass with tests when empty.
    pub(crate) gates: VecDeque<CommandOutcome>,
    /// The next diffs; `src/lib.rs` modified when empty.
    pub(crate) diffs: VecDeque<Vec<Change>>,
    /// The diff of an order every time it is read, before `diffs`.
    pub(crate) order_diffs: BTreeMap<OrderId, Vec<Change>>,
    /// The next merge results; an unmoved target when empty.
    pub(crate) merges: VecDeque<OpResult>,
    /// The next take-over results; done when empty.
    pub(crate) takeovers: VecDeque<OpResult>,
    /// The next ref changes; none when empty.
    pub(crate) refs: VecDeque<Vec<RefChange>>,
    /// The next secrets scans; clean when empty.
    pub(crate) scans: VecDeque<SecretScan>,
    /// Usage each turn reports before it ends.
    pub(crate) usage: Option<UsageReport>,
    /// Whether a byte-for-byte comparison after a merge finds the diff unchanged.
    pub(crate) identical: bool,
    /// What verifying an operation after a restart finds.
    pub(crate) verified: VecDeque<Option<OpResult>>,
    /// Orders whose log cannot be written: their entries do not reach the log, their effects do not run, and the
    /// factory hears of it, as from the app.
    pub(crate) unwritable: BTreeSet<OrderId>,
    commits: u64,
    sessions: u64,
}

impl App {
    /// An app with a factory in `stage` under `rules`, whose Claude, Codex and own agent are ready.
    pub(crate) fn new(stage: Stage, rules: Rules) -> App {
        App::with(Factory::new(stage, rules))
    }

    /// An app around `factory`.
    pub(crate) fn with(factory: Factory) -> App {
        let mut app = App {
            factory,
            logs: BTreeMap::new(),
            effects: Vec::new(),
            pending: VecDeque::new(),
            refusals: Vec::new(),
            mono: 1_000,
            wall: 1_700_000_000_000,
            hold: BTreeSet::new(),
            turns: VecDeque::new(),
            verdicts: VecDeque::new(),
            gates: VecDeque::new(),
            diffs: VecDeque::new(),
            order_diffs: BTreeMap::new(),
            merges: VecDeque::new(),
            takeovers: VecDeque::new(),
            refs: VecDeque::new(),
            scans: VecDeque::new(),
            usage: None,
            identical: true,
            verified: VecDeque::new(),
            unwritable: BTreeSet::new(),
            commits: 0,
            sessions: 0,
        };
        let agents: BTreeMap<Agent, AgentStatus> = [Agent::Claude, Agent::Codex, Agent::Own]
            .into_iter()
            .map(|a| (a.clone(), AgentStatus::ready(&a)))
            .collect();
        app.send(InputKind::Agents(agents));
        app
    }

    pub(crate) fn at(&self) -> Stamp {
        Stamp::new(self.mono, self.wall)
    }

    /// Lets `ms` pass on both clocks.
    pub(crate) fn pass(&mut self, ms: u64) {
        self.mono += ms;
        self.wall += ms;
    }

    /// Lets `ms` pass while the Mac sleeps: the app's monotonic clock is `Instant`, which on macOS stands still in
    /// sleep, so only the wall clock moves.
    pub(crate) fn sleep_through(&mut self, ms: u64) {
        self.wall += ms;
    }

    /// Hands the factory one input and keeps what it asks for.
    pub(crate) fn send(&mut self, kind: InputKind) -> ide_factory::Step {
        self.pass(1);
        let step = self.factory.handle(Input::new(self.at(), kind));
        let failed = self.write(&step.log);
        let runs = |e: &&Effect| e.order().is_none_or(|order| !failed.iter().any(|f| f.order == *order));
        self.effects.extend(step.effects.iter().filter(runs).cloned());
        self.pending.extend(step.effects.iter().filter(runs).cloned());
        self.refusals.extend(step.refusals.iter().cloned());
        for failure in failed {
            self.send(InputKind::LogFailed(failure));
        }
        step
    }

    /// Appends `entries` to their logs and tells the factory, except for orders whose log cannot be written.
    pub(crate) fn write(&mut self, entries: &[LogEntry]) -> Vec<LogFailure> {
        let mut failed: Vec<LogFailure> = Vec::new();
        for entry in entries {
            if self.unwritable.contains(&entry.order) {
                if !failed.iter().any(|f| f.order == entry.order) {
                    failed.push(LogFailure {
                        order: entry.order.clone(),
                        seq: entry.seq,
                    });
                }
                continue;
            }
            let line = entry.line().expect("entries serialise");
            self.logs.entry(entry.order.clone()).or_default().push_str(&line);
            self.factory.logged(&entry.order, entry.seq + 1);
        }
        failed
    }

    /// The app after a restart with `rules`: the factory restored from the logs, its first step written.
    pub(crate) fn restart_with(&self, rules: Rules) -> App {
        let logs: Vec<String> = self.logs.values().cloned().collect();
        let restored = Factory::restore(Stage::One, rules, &logs, Stamp::new(5, self.wall + 1000));
        assert!(restored.problems.is_empty(), "{:?}", restored.problems);
        let mut next = App::with(restored.factory);
        next.wall = self.wall + 2000;
        next.logs = self.logs.clone();
        next.write(&restored.step.log);
        next.effects.extend(restored.step.effects.iter().cloned());
        next.pending.extend(restored.step.effects);
        next
    }

    /// Order `id` as its log says, the way a restart finds it before it forgets what only running processes knew.
    pub(crate) fn replayed(&self, id: &OrderId) -> Order {
        let read = read(self.logs.get(id).map_or("", String::as_str));
        assert!(!read.cut && read.stopped_at.is_none(), "the log of {id} reads whole");
        Order::replay(&read.entries).expect("the log begins with the order's creation")
    }

    /// Asserts that every order is what its log says: the factory's central promise (E167).
    pub(crate) fn assert_replays(&self) {
        for order in self.factory.orders() {
            assert_eq!(
                comparable(order),
                comparable(&self.replayed(order.id())),
                "order {} as it runs and as its log says",
                order.id()
            );
        }
    }

    /// Answers pending effects until only held ones are left.
    pub(crate) fn settle(&mut self) {
        let mut held = VecDeque::new();
        for _ in 0..10_000 {
            let Some(effect) = self.pending.pop_front() else { break };
            if self.hold.contains(kind(&effect).as_str()) {
                held.push_back(effect);
                continue;
            }
            if let Some(reply) = self.reply(&effect) {
                self.send(reply);
            }
        }
        self.pending = held;
    }

    /// Takes the first pending effect of `name` out of the queue.
    pub(crate) fn take(&mut self, name: &str) -> Option<Effect> {
        let at = self.pending.iter().position(|e| kind(e) == name)?;
        self.pending.remove(at)
    }

    /// Answers one effect as the healthy app would.
    pub(crate) fn reply(&mut self, effect: &Effect) -> Option<InputKind> {
        match effect {
            Effect::PrepareWorktree { order, op, .. } => Some(InputKind::Done {
                order: order.clone(),
                op: op.clone(),
                result: OpResult::Prepared {
                    path: format!("/worktrees/{order}"),
                    tree: "tree-base".into(),
                    setup: None,
                    copied: Vec::new(),
                },
            }),
            Effect::FreezeConfig { order, .. } => {
                Some(InputKind::Frozen(order.clone(), Ok(format!("/frozen/{order}"))))
            }
            Effect::StartSession { order, role, .. } => {
                self.sessions += 1;
                Some(InputKind::SessionStarted {
                    order: order.clone(),
                    role: *role,
                    session: SessionKey(format!("s{}", self.sessions)),
                    resume: Some(format!("resume-{}", self.sessions)),
                })
            }
            Effect::SendPrompt {
                order, session, prompt, ..
            } => {
                if let Some(report) = self.usage {
                    self.send(InputKind::Session(
                        order.clone(),
                        session.clone(),
                        SessionEvent::Usage { report },
                    ));
                }
                let outcome = self.turns.pop_front().unwrap_or(TurnOutcome::Succeeded);
                let answer = self.answer_for(order, session);
                Some(InputKind::Session(
                    order.clone(),
                    session.clone(),
                    SessionEvent::TurnEnded {
                        prompt: Some(*prompt),
                        outcome,
                        answer,
                    },
                ))
            }
            Effect::Verify { order, op, .. } => Some(InputKind::Verified {
                order: order.clone(),
                op: op.clone(),
                reached: self.verified.pop_front().unwrap_or(None),
            }),
            _ => self.reply_repository(effect),
        }
    }

    /// Answers an effect on the repository or the gates.
    fn reply_repository(&mut self, effect: &Effect) -> Option<InputKind> {
        match effect {
            Effect::CompareRefs { order } => Some(InputKind::Refs(
                order.clone(),
                self.refs.pop_front().unwrap_or_default(),
            )),
            Effect::ScanSecrets { order, run_id, .. } => Some(InputKind::Scanned(
                order.clone(),
                *run_id,
                self.scans.pop_front().unwrap_or(SecretScan::Clean),
            )),
            Effect::Commit { order, op, .. } => {
                self.commits += 1;
                Some(InputKind::Done {
                    order: order.clone(),
                    op: op.clone(),
                    result: OpResult::Committed {
                        commit: format!("c{}", self.commits),
                        tree: format!("t{}", self.commits),
                    },
                })
            }
            Effect::DiffNames { order, run_id, .. } => Some(InputKind::Diff(
                order.clone(),
                *run_id,
                self.order_diffs
                    .get(order)
                    .cloned()
                    .or_else(|| self.diffs.pop_front())
                    .unwrap_or_else(|| vec![Change::new("src/lib.rs", ChangeKind::Modified)]),
            )),
            Effect::RunGate {
                order, run_id, index, ..
            } => Some(InputKind::GateDone {
                order: order.clone(),
                run_id: *run_id,
                index: *index,
                outcome: self.gates.pop_front().unwrap_or_else(passing_gate),
            }),
            Effect::CollectReviewInput { order, .. } => Some(InputKind::ReviewInput {
                order: order.clone(),
                diff: "+fn csv() {}".into(),
                commits: "CSV export".into(),
            }),
            Effect::ReadManifest { order, .. } => Some(InputKind::Manifest(order.clone(), Manifest::default())),
            Effect::RejectHunks { order, op, .. } => Some(InputKind::Done {
                order: order.clone(),
                op: op.clone(),
                result: OpResult::Reversed,
            }),
            Effect::MergeTarget { order, op, .. } => {
                let (head, tree, base) = self.head_of(order);
                let result = self.merges.pop_front().unwrap_or(OpResult::Merged {
                    head,
                    tree,
                    base,
                    moved: false,
                });
                Some(InputKind::Done {
                    order: order.clone(),
                    op: op.clone(),
                    result,
                })
            }
            Effect::CompareDiffs { order, run_id, .. } => {
                Some(InputKind::Compared(order.clone(), *run_id, self.identical))
            }
            Effect::TakeOver { order, op, .. } => Some(InputKind::Done {
                order: order.clone(),
                op: op.clone(),
                result: self.takeovers.pop_front().unwrap_or(OpResult::TakenOver {
                    commit: "merged".into(),
                }),
            }),
            Effect::RemoveCheckpoints { order, op, .. } | Effect::RemoveWorktree { order, op, .. } => {
                Some(InputKind::Done {
                    order: order.clone(),
                    op: op.clone(),
                    result: OpResult::Removed,
                })
            }
            _ => None,
        }
    }

    fn head_of(&self, order: &OrderId) -> (String, String, String) {
        let order = self.factory.order(order).expect("the order exists");
        let (head, tree) = order
            .head()
            .map_or(("".into(), "".into()), |(c, t)| (c.to_string(), t.to_string()));
        (head, tree, order.base().commit.clone())
    }

    fn answer_for(&mut self, order: &OrderId, session: &SessionKey) -> Option<String> {
        let order = self.factory.order(order)?;
        let role = ide_factory::Role::ALL
            .into_iter()
            .find(|r| order.session(*r).is_some_and(|s| s.session() == session))?;
        match role {
            ide_factory::Role::Planner | ide_factory::Role::Scout => Some(SPEC.to_string()),
            ide_factory::Role::Reviewer => Some(self.verdicts.pop_front().unwrap_or_else(|| passing_verdict(order))),
            _ => None,
        }
    }

    /// Creates an order with the sheet's two required fields and one criterion.
    pub(crate) fn order(&mut self, id: &str) -> OrderId {
        self.order_with(id, |_| {})
    }

    /// Creates an order whose sheet `edit` changes first.
    pub(crate) fn order_with(&mut self, id: &str, edit: impl FnOnce(&mut NewOrder)) -> OrderId {
        let mut sheet = NewOrder {
            title: format!("Order {id}"),
            goal: "Export the table as CSV.".into(),
            criteria: "- exports CSV\n".into(),
            ..NewOrder::default()
        };
        edit(&mut sheet);
        let id = OrderId(id.to_string());
        self.send(InputKind::NewOrder {
            id: id.clone(),
            order: sheet,
            base: BaseRef {
                branch: "main".into(),
                commit: "base0".into(),
            },
            dirty: false,
        });
        id
    }

    pub(crate) fn get(&self, id: &OrderId) -> &Order {
        self.factory.order(id).expect("the order exists")
    }

    pub(crate) fn station(&self, id: &OrderId) -> Station {
        self.get(id).station()
    }

    pub(crate) fn condition(&self, id: &OrderId) -> Condition {
        self.get(id).condition().clone()
    }

    /// The effects of `name` asked for so far.
    pub(crate) fn asked(&self, name: &str) -> usize {
        self.effects.iter().filter(|e| kind(e) == name).count()
    }

    /// The current evidence's fingerprint.
    pub(crate) fn fingerprint(&self, id: &OrderId) -> String {
        self.get(id).evidence().expect("sealed").fingerprint().to_string()
    }

    /// Accepts the current evidence whole.
    pub(crate) fn accept(&mut self, id: &OrderId) {
        let fingerprint = self.fingerprint(id);
        self.send(InputKind::Accept {
            order: id.clone(),
            fingerprint,
            rejected: Vec::new(),
        });
    }

    /// The session of the running turn.
    pub(crate) fn turn_session(&self, id: &OrderId) -> SessionKey {
        let order = self.get(id);
        let role = order.turn().expect("a turn runs").role();
        order.session(role).expect("a session").session().clone()
    }
}

/// What of `order` its log holds: everything but what only the running session showed of its turn.
pub(crate) fn comparable(order: &Order) -> serde_json::Value {
    let mut value = serde_json::to_value(order).expect("orders serialise");
    if let Some(turn) = value.get_mut("turn").and_then(serde_json::Value::as_object_mut) {
        for shown in ["watch", "queued", "ended", "answer"] {
            turn.remove(shown);
        }
    }
    value
}

/// The serde tag of `effect`, such as `send_prompt`.
pub(crate) fn kind(effect: &Effect) -> String {
    serde_json::to_value(effect).expect("effects serialise")["effect"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// A gate that passed with tests.
pub(crate) fn passing_gate() -> CommandOutcome {
    CommandOutcome {
        exit: Some(0),
        tests: Some(TestCount {
            ran: 5,
            failed: 0,
            skipped: 0,
        }),
        duration_ms: 1_000,
        excerpt: "5 passed".into(),
        ..CommandOutcome::default()
    }
}
