//! Publishing an order on the user's click only (E177, E164): the secrets scan of everything that goes out first, then
//! the push, the merge request and the issue's comment, each an operation with a target state on the service (C66).
//! Steps already done stay done, so trying again goes on where it stopped. After a restart the factory only verifies
//! what was begun; nothing more goes out until the user clicks again.

use super::{Cx, Factory, Refused};
use crate::effect::Effect;
use crate::event::{Action, Event, OpResult, Phase};
use crate::gates::SecretScan;
use crate::ids::OrderId;
use crate::order::Order;
use crate::publish::{CHANGE_PLACEHOLDER, Outgoing, PublishFailure, PublishStep, marked, marker};
use crate::station::Station;

impl Factory {
    /// The user's click: with `fingerprint` G2 that keeps the branch and publishes once the order is done, without it
    /// publishing a done order whose branch was kept, or trying again.
    pub(super) fn publish(&mut self, cx: &mut Cx, id: &OrderId, fingerprint: Option<String>, texts: Outgoing) {
        if let Some(fingerprint) = fingerprint {
            if !self.acceptable(cx, id, &fingerprint) {
                return;
            }
            let rejected = Vec::new();
            self.emit(cx, id, Event::Accepted { fingerprint, rejected });
            self.emit(cx, id, Event::BranchKept);
            self.emit(cx, id, Event::PublishRequested { texts });
            self.run(cx, id);
            self.enter(cx, id, Station::TakeOver);
            self.advance(cx, id);
            return;
        }
        if !self.known(cx, id) {
            return;
        }
        if !self.orders.get(id).is_some_and(Order::can_publish) {
            cx.refuse(Some(id), Refused::NotNow);
            return;
        }
        self.emit(cx, id, Event::PublishRequested { texts });
        self.publish_start(cx, id);
    }

    /// Has everything that is to go out scanned for secrets, the first step of every attempt.
    pub(super) fn publish_start(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(text) = self.orders.get(id).and_then(outgoing_text) else {
            return;
        };
        self.set_phase(cx, id, Phase::PublishScan);
        let run_id = self.start_run(id);
        cx.effect(Effect::ScanOutgoing {
            order: id.clone(),
            run_id,
            text,
        });
    }

    /// The scan of the outgoing text: only a clean one lets anything go out; without gitleaks nothing does (F75).
    pub(super) fn publish_scanned(&mut self, cx: &mut Cx, id: &OrderId, scan: SecretScan) {
        match scan {
            SecretScan::Clean => self.publish_next(cx, id),
            SecretScan::NotInstalled => self.publish_failed(cx, id, PublishFailure::ScanUnavailable),
            SecretScan::Found { count, .. } => self.publish_failed(cx, id, PublishFailure::SecretsFound { count }),
        }
    }

    /// Begins the first step not done yet: push, merge request, comment; with all done, publishing rests.
    fn publish_next(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let Some(publication) = order.publication() else {
            return;
        };
        let (op, branch, marker) = (order.next_op(), id.branch(), marker(id));
        let (action, effect) = if !publication.pushed() {
            let Some(commit) = accepted_commit(order) else {
                return self.publish_failed(cx, id, PublishFailure::NoAcceptedCommit);
            };
            let action = Action::Push {
                branch: branch.clone(),
                commit: commit.clone(),
            };
            let order = id.clone();
            (
                action,
                Effect::Push {
                    order,
                    op: op.clone(),
                    branch,
                    commit,
                },
            )
        } else if publication.change().is_none() {
            let base = order.base().branch.clone();
            let action = Action::OpenChange {
                branch: branch.clone(),
                base: base.clone(),
                marker: marker.clone(),
            };
            let effect = Effect::OpenChange {
                order: id.clone(),
                op: op.clone(),
                branch,
                base,
                title: publication.texts().title().to_string(),
                body: marked(publication.texts().body(), id),
                marker,
            };
            (action, effect)
        } else if let (Some(issue), None, Some(change)) = (
            order.options().issue.as_ref(),
            publication.comment(),
            publication.change(),
        ) {
            let issue = issue.number;
            let text = publication.texts().comment().replace(CHANGE_PLACEHOLDER, change.url());
            let action = Action::CommentIssue {
                issue,
                marker: marker.clone(),
            };
            let body = marked(&text, id);
            let order = id.clone();
            (
                action,
                Effect::CommentIssue {
                    order,
                    op: op.clone(),
                    issue,
                    body,
                    marker,
                },
            )
        } else {
            self.set_phase(cx, id, Phase::Idle);
            return;
        };
        self.begin(cx, id, action, Phase::Publishing { op });
        cx.effect(effect);
    }

    /// A step's result: a failure stops publishing with the app's words, success goes on to the next step.
    pub(super) fn published_step(&mut self, cx: &mut Cx, id: &OrderId, action: &Action, result: OpResult) {
        if let OpResult::Failed { detail } = result {
            let step = match action {
                Action::Push { .. } => PublishStep::Push,
                Action::OpenChange { .. } => PublishStep::Change,
                _ => PublishStep::Comment,
            };
            self.publish_failed(cx, id, PublishFailure::Step { step, detail });
            return;
        }
        self.set_phase(cx, id, Phase::Idle);
        self.publish_next(cx, id);
    }

    fn publish_failed(&mut self, cx: &mut Cx, id: &OrderId, failure: PublishFailure) {
        self.emit(cx, id, Event::PublishFailed { failure });
        self.set_phase(cx, id, Phase::Idle);
    }

    /// After a restart: a step's target state is recorded as it was found, and publishing rests until the next click.
    pub(super) fn publish_verified(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        op: crate::ids::OperationId,
        reached: Option<OpResult>,
    ) {
        match reached {
            Some(result) => self.emit(cx, id, Event::Finished { op, result }),
            None => self.emit(cx, id, Event::Dropped { op }),
        }
        self.set_phase(cx, id, Phase::Idle);
    }

    /// Once a G2 that asked for publishing has left the order done, publishing begins.
    pub(super) fn publish_when_done(&mut self, cx: &mut Cx, id: &OrderId) {
        let due = self.orders.get(id).is_some_and(|o| {
            o.station() == Station::Done
                && o.publication().is_some_and(|p| p.failure().is_none())
                && !o.published()
                && !o.publishing()
        });
        if due {
            self.publish_start(cx, id);
        }
    }
}

/// The commit G2 accepted: the evidence's, which a kept branch still points at.
fn accepted_commit(order: &Order) -> Option<String> {
    order
        .evidence()
        .map(|e| e.commit().to_string())
        .or_else(|| order.head().map(|(commit, _)| commit.to_string()))
        .filter(|c| !c.is_empty())
}

/// Everything that goes out, as one text for the scan: title, body and comment, markers included.
fn outgoing_text(order: &Order) -> Option<String> {
    let texts = order.publication()?.texts();
    let mut parts = vec![texts.title().to_string(), marked(texts.body(), order.id())];
    if order.options().issue.is_some() {
        parts.push(marked(texts.comment(), order.id()));
    }
    Some(parts.join("\n\n"))
}
