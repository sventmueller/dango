//! The review row (E163): classification of the diff (E172), path scope, secrets scan, gate commands, reviewer
//! (E179), and the evidence sealed at its end. The first failure stops the row and goes back to the builder.

use super::{Cx, Factory};
use crate::effect::{Effect, SandboxRun, ScanRange};
use crate::event::{Event, Phase, PromptKind};
use crate::evidence::{Bound, Deviation, Evidence, GateRecord, Manifest, ManifestEntry};
use crate::gates::{
    CommandOutcome, GateFailure, GateStep, PathScope, SecretScan, judge_command, judge_scope, judge_secrets,
};
use crate::ids::OrderId;
use crate::order::Order;
use crate::prompt::untrusted;
use crate::relevance::{Change, Classification};
use crate::station::{Role, Stage, Station, WaitReason};
use crate::verdict::{RoundOutcome, read_verdict};
use crate::workflow::rules::holds_credentials;

impl Factory {
    /// The steps of the review row for `order`, in their order.
    fn steps(&self, order: &Order) -> Vec<GateStep> {
        let mut steps = vec![GateStep::PathScope, GateStep::Secrets];
        let count = self.rules.gates_for(order.risk()).len();
        steps.extend((0..count).map(GateStep::Command));
        steps.push(GateStep::Reviewer);
        steps
    }

    fn head(order: &Order) -> (String, String) {
        order.head().map_or_else(
            || (order.base().commit.clone(), String::new()),
            |(c, t)| (c.to_string(), t.to_string()),
        )
    }

    /// The next step of the review.
    pub(super) fn review_next(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if !self.rules.has_gates() {
            self.wait(cx, id, WaitReason::NoGatePlan);
            return;
        }
        if !self.rules.gates_confirmed() {
            self.wait(cx, id, WaitReason::GateCommandsChanged);
            return;
        }
        let (head, _) = Factory::head(order);
        let base = order.base().commit.clone();
        let diffed = self
            .scratch
            .get(id)
            .and_then(|s| s.diff.as_ref())
            .is_some_and(|(h, _)| *h == head);
        if !diffed {
            self.set_phase(cx, id, Phase::Diffing);
            let run_id = self.start_run(id);
            cx.effect(Effect::DiffNames {
                order: id.clone(),
                run_id,
                base,
                head,
            });
            return;
        }
        let done: Vec<GateStep> = order.gates().iter().map(|g| g.step).collect();
        let next = self.steps(order).into_iter().find(|step| !done.contains(step));
        match next {
            None => self.seal_or_read_manifest(cx, id),
            Some(GateStep::PathScope) => self.path_scope(cx, id),
            Some(GateStep::Secrets) => {
                self.set_phase(cx, id, Phase::Scanning);
                let run_id = self.start_run(id);
                cx.effect(Effect::ScanSecrets {
                    order: id.clone(),
                    run_id,
                    range: ScanRange::Diff { base, head },
                });
            }
            Some(GateStep::Command(index)) => self.run_gate(cx, id, index),
            Some(GateStep::Reviewer) => self.reviewer(cx, id),
        }
    }

    /// The diff's paths arrived: classify them (E172) and wait for the user where E172 and E163 say so.
    pub(super) fn diffed(&mut self, cx: &mut Cx, id: &OrderId, changes: Vec<Change>) {
        let Some(order) = self.orders.get(id) else { return };
        if *order.phase() != Phase::Diffing {
            return;
        }
        let (head, _) = Factory::head(order);
        let classification = match self.rules.relevance() {
            Ok(relevance) => relevance.classify(&changes),
            Err(_) => Classification::default(),
        };
        let outside = PathScope::new(&order.options().scope)
            .map(|scope| {
                if order.options().scope.is_empty() {
                    Vec::new()
                } else {
                    scope.outside(&changes)
                }
            })
            .unwrap_or_default();
        let same = order.acknowledged() && order.classification() == Some(&classification);
        let late = order.g1_skipped() && (classification.measured > order.expected_risk() || !outside.is_empty());
        let agent_config = classification.agent_config.clone();
        self.scratch(id).diff = Some((head, changes));
        self.emit(cx, id, Event::Classified { classification });
        if same {
            self.emit(cx, id, Event::Acknowledged);
        }
        self.set_phase(cx, id, Phase::Idle);
        if !same && !agent_config.is_empty() {
            self.wait(cx, id, WaitReason::AgentConfigChanged { paths: agent_config });
        } else if !same && late {
            self.wait(cx, id, WaitReason::LateSpecApproval);
        } else {
            self.review_next(cx, id);
        }
    }

    fn path_scope(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let changes = self
            .scratch
            .get(id)
            .and_then(|s| s.diff.as_ref())
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        if order.options().scope.is_empty() {
            self.emit(
                cx,
                id,
                Event::Skipped {
                    station: Station::Review,
                    why: crate::event::SkipWhy::NoPathScope,
                },
            );
            self.record(cx, id, GateStep::PathScope, "path scope", None, None, None, true);
            self.review_next(cx, id);
            return;
        }
        let failure = PathScope::new(&order.options().scope)
            .ok()
            .and_then(|scope| judge_scope(&scope, &changes).err());
        self.record(
            cx,
            id,
            GateStep::PathScope,
            "path scope",
            failure.clone(),
            None,
            None,
            false,
        );
        match failure {
            Some(failure) => self.rework(cx, id, &format!("The diff leaves the order's path scope: {failure:?}")),
            None => self.review_next(cx, id),
        }
    }

    #[expect(clippy::too_many_arguments, reason = "one record of the row, field by field")]
    fn record(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        step: GateStep,
        name: &str,
        failure: Option<GateFailure>,
        command: Option<CommandOutcome>,
        scan: Option<SecretScan>,
        skipped: bool,
    ) {
        let record = GateRecord {
            step,
            name: name.to_string(),
            failure,
            command,
            scan,
            skipped,
        };
        self.emit(cx, id, Event::Gate { record });
    }

    /// A secrets scan's result: before the commit after a turn, as a step of the review, or of the text that goes to
    /// GitHub or GitLab.
    pub(super) fn scanned(&mut self, cx: &mut Cx, id: &OrderId, scan: SecretScan) {
        let Some(order) = self.orders.get(id) else { return };
        match order.phase() {
            Phase::TurnScan => match judge_secrets(&scan) {
                Err(failure) => {
                    let text = format!("The changes of your turn contain secrets: {failure:?}. Remove them.");
                    self.emit(cx, id, Event::Findings { text, round: false });
                    self.set_phase(cx, id, Phase::Idle);
                    self.advance(cx, id);
                }
                Ok(()) => {
                    let message = turn_message(order);
                    self.commit(cx, id, message);
                }
            },
            Phase::Scanning => {
                let failure = judge_secrets(&scan).err();
                self.record(
                    cx,
                    id,
                    GateStep::Secrets,
                    "secrets",
                    failure.clone(),
                    None,
                    Some(scan),
                    false,
                );
                self.set_phase(cx, id, Phase::Idle);
                match failure {
                    Some(failure) => self.rework(cx, id, &format!("The secrets scan found secrets: {failure:?}")),
                    None => self.review_next(cx, id),
                }
            }
            Phase::PublishScan => self.publish_scanned(cx, id, scan),
            _ => {}
        }
    }

    fn run_gate(&mut self, cx: &mut Cx, id: &OrderId, index: usize) {
        let Some(order) = self.orders.get(id) else { return };
        let gates = self.rules.gates_for(order.risk());
        let Some(gate) = gates.get(index) else { return };
        let images = self
            .rules
            .images()
            .map(|dir| format!("{dir}/{id}/{}", order.updated().wall));
        let name = gate.name.clone();
        let run = SandboxRun {
            command: gate.run.clone(),
            env: self.rules.gate_env().clone(),
            network: self.rules.network().to_vec(),
            sockets: self.rules.sockets().to_vec(),
            deadline_ms: self.rules.limits().gate_ms(),
            output_limit: self.rules.limits().gate_output_bytes(),
        };
        self.set_phase(cx, id, Phase::Gate { index });
        let effect = Effect::RunGate {
            order: id.clone(),
            run_id: self.start_run(id),
            index,
            name,
            run,
            images,
        };
        let scratch = self.scratch(id);
        scratch.gate_started = Some(cx.now.mono);
        scratch.gate_stopping = false;
        cx.effect(effect);
    }

    /// A gate command ended.
    pub(super) fn gate_done(&mut self, cx: &mut Cx, id: &OrderId, index: usize, outcome: CommandOutcome) {
        let Some(order) = self.orders.get(id) else { return };
        if *order.phase() != (Phase::Gate { index }) {
            return;
        }
        let name = self
            .rules
            .gates_for(order.risk())
            .get(index)
            .map_or_else(|| format!("gate {index}"), |g| g.name.clone());
        let failure = judge_command(&outcome).err();
        let excerpt = outcome.excerpt.clone();
        self.record(
            cx,
            id,
            GateStep::Command(index),
            &name,
            failure.clone(),
            Some(outcome),
            None,
            false,
        );
        self.scratch(id).gate_started = None;
        self.set_phase(cx, id, Phase::Idle);
        match failure {
            Some(failure) => {
                let log = untrusted("the gate's log", &excerpt);
                self.rework(cx, id, &format!("Gate `{name}` failed: {failure:?}\n\n{log}"));
            }
            None => self.review_next(cx, id),
        }
    }

    fn reviewer(&mut self, cx: &mut Cx, id: &OrderId) {
        if self.orders.get(id).is_none_or(|o| o.stage() == Stage::One) {
            self.emit(
                cx,
                id,
                Event::Skipped {
                    station: Station::Review,
                    why: crate::event::SkipWhy::NoReviewerInStageOne,
                },
            );
            self.record(cx, id, GateStep::Reviewer, "reviewer", None, None, None, true);
            self.review_next(cx, id);
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        if self.scratch.get(id).and_then(|s| s.review_input.as_ref()).is_none() {
            let (head, _) = Factory::head(order);
            let base = order.base().commit.clone();
            self.set_phase(cx, id, Phase::ReviewInput);
            cx.effect(Effect::CollectReviewInput {
                order: id.clone(),
                base,
                head,
            });
            return;
        }
        if let Some(session) = self.ensure_session(cx, id, Role::Reviewer) {
            self.prompt(cx, id, Role::Reviewer, &session, PromptKind::Review);
        }
    }

    /// The reviewer's input arrived.
    pub(super) fn review_input(&mut self, cx: &mut Cx, id: &OrderId, diff: String, commits: String) {
        if self.orders.get(id).map(Order::phase) != Some(&Phase::ReviewInput) {
            return;
        }
        self.scratch(id).review_input = Some((diff, commits));
        self.set_phase(cx, id, Phase::Idle);
        self.review_next(cx, id);
    }

    /// The reviewer answered (E179): passed goes on, not met or a high finding goes back to the builder, unclear to
    /// the user.
    pub(super) fn reviewed(&mut self, cx: &mut Cx, id: &OrderId, answer: &str) {
        let Some(order) = self.orders.get(id) else { return };
        let round = read_verdict(answer, order.criteria());
        let outcome = round.outcome.clone();
        let findings: Vec<String> = round
            .findings
            .iter()
            .map(|f| format!("- {:?} {}:{} {}", f.severity, f.file, f.line.unwrap_or(0), f.text))
            .collect();
        let reasons: Vec<String> = round
            .verdicts
            .iter()
            .filter(|v| v.verdict != crate::verdict::Judgement::Met)
            .map(|v| format!("- [{}] {:?}: {}", v.id, v.verdict, v.reason))
            .collect();
        self.emit(cx, id, Event::Reviewed { round });
        self.end_session(cx, id, Role::Reviewer);
        self.set_phase(cx, id, Phase::Idle);
        match outcome {
            RoundOutcome::Passed => {
                self.record(cx, id, GateStep::Reviewer, "reviewer", None, None, None, false);
                self.review_next(cx, id);
            }
            RoundOutcome::Rework { not_met } => {
                let failure = GateFailure::Reviewer { not_met };
                self.record(cx, id, GateStep::Reviewer, "reviewer", Some(failure), None, None, false);
                let text = format!(
                    "The reviewer did not accept the change.\n\n{}\n\n{}",
                    reasons.join("\n"),
                    findings.join("\n")
                );
                self.rework(cx, id, &text);
            }
            RoundOutcome::Unclear { .. } | RoundOutcome::Invalid { .. } => self.wait(cx, id, WaitReason::Unclear),
        }
    }

    /// The user let an unclear verdict pass: the reviewer's step counts as passed.
    pub(super) fn unclear_passed(&mut self, cx: &mut Cx, id: &OrderId) {
        self.record(cx, id, GateStep::Reviewer, "reviewer", None, None, None, false);
        self.review_next(cx, id);
    }

    /// The row failed: back to the builder with `text`, or to the user after the last review round (E168).
    pub(super) fn rework(&mut self, cx: &mut Cx, id: &OrderId, text: &str) {
        self.emit(
            cx,
            id,
            Event::Findings {
                text: text.to_string(),
                round: true,
            },
        );
        let Some(order) = self.orders.get(id) else { return };
        if order.review_rounds() >= self.rules.limits().review_rounds() {
            self.wait(cx, id, WaitReason::ReviewRoundsSpent);
            return;
        }
        self.enter(cx, id, Station::Build);
        self.advance(cx, id);
    }

    fn seal_or_read_manifest(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(manifest) = self.scratch(id).manifest.take() else {
            self.read_manifest(cx, id);
            return;
        };
        self.seal(cx, id, manifest);
    }

    /// Asks for the manifest of inputs outside the tree.
    pub(super) fn read_manifest(&mut self, cx: &mut Cx, id: &OrderId) {
        self.set_phase(cx, id, Phase::Manifest);
        cx.effect(Effect::ReadManifest {
            order: id.clone(),
            copied: self.rules.copy().to_vec(),
        });
    }

    /// The manifest arrived: for sealing at the end of the review, or for checking the evidence before the take-over.
    pub(super) fn manifest(&mut self, cx: &mut Cx, id: &OrderId, manifest: Manifest) {
        let Some(order) = self.orders.get(id) else { return };
        if *order.phase() != Phase::Manifest {
            return;
        }
        let station = order.station();
        self.set_phase(cx, id, Phase::Idle);
        if station == Station::TakeOver {
            self.scratch(id).manifest = Some(manifest);
            self.take_over_next(cx, id);
        } else {
            self.seal(cx, id, manifest);
        }
    }

    /// The evidence's binding for `order` with `manifest` (E163, C69).
    pub(super) fn bound(&self, order: &Order, manifest: Manifest) -> Bound {
        let (_, tree) = Factory::head(order);
        Bound {
            tree,
            rules: self.rules.hash(),
            base: order.base().commit.clone(),
            manifest,
        }
    }

    fn seal(&mut self, cx: &mut Cx, id: &OrderId, manifest: Manifest) {
        let Some(order) = self.orders.get(id) else { return };
        let (commit, _) = Factory::head(order);
        let credentials = manifest.entries().iter().any(|e| match e {
            ManifestEntry::Copied { path, .. } => holds_credentials(path),
            ManifestEntry::Dependency { .. } => false,
        });
        let bound = self.bound(order, manifest);
        let mut deviations = order.deviations().to_vec();
        if order.gates().iter().any(|g| g.scan == Some(SecretScan::NotInstalled)) {
            deviations.push(Deviation::SecretsNotChecked);
        }
        if let Some(c) = order.classification() {
            if !c.agent_config.is_empty() {
                deviations.push(Deviation::AgentConfigChanged {
                    paths: c.agent_config.clone(),
                });
            }
            let checks: Vec<String> = c.tests.iter().chain(&c.config).cloned().collect();
            if !checks.is_empty() {
                deviations.push(Deviation::ChecksChanged { paths: checks });
            }
        }
        if credentials {
            deviations.push(Deviation::CredentialsCopied);
        }
        let evidence = Evidence::new(bound, &commit, deviations).with_details(
            order.gates().to_vec(),
            order.review_log().to_vec(),
            order.costs(),
            order.provenance().to_vec(),
            order.metering(),
        );
        let fingerprint = evidence.fingerprint().to_string();
        let closes = order.partial() || order.kept();
        self.emit(
            cx,
            id,
            Event::Sealed {
                evidence: Box::new(evidence),
            },
        );
        if closes {
            self.emit(
                cx,
                id,
                Event::Accepted {
                    fingerprint,
                    rejected: Vec::new(),
                },
            );
            self.enter(cx, id, Station::TakeOver);
        } else {
            self.enter(cx, id, Station::Acceptance);
        }
        self.advance(cx, id);
    }
}

/// The message of the commit after a turn; role, agent and model stand in the log (E163).
fn turn_message(order: &Order) -> String {
    let turns = order.turns_in(Station::Build);
    format!("{} (turn {turns})", order.title())
}
