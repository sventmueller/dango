//! The tab of one order (E175): its timeline from the log, the request that waits for the user as a card, the
//! evidence package with the deviations first, the sections of its diff for a partial acceptance (C55), and which of
//! the order's commands fit where it stands; built from `ide_factory`'s order, the view only shows it.

use std::collections::BTreeSet;

use ide_factory::evidence::{Evidence, GateRecord};
use ide_factory::order::Order;
use ide_factory::station::{Condition, RiskClass, Role, Station, WaitReason};
use ide_factory::verdict::{Judgement, RoundOutcome};
use ide_factory::{HunkId, InputKind, OrderId, RequestId};
use splat_i18n::{t, tf};

use super::list::agent;
use super::words::{self, Tone};

/// A section of the order's diff, which G2 can reject (C55).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    id: HunkId,
    file: String,
    header: String,
}

impl Section {
    /// The section `id` of `file`, headed `header` as git heads a hunk.
    pub fn new(id: HunkId, file: &str, header: &str) -> Section {
        Section {
            id,
            file: file.to_string(),
            header: header.to_string(),
        }
    }

    /// Its id.
    pub fn id(&self) -> &HunkId {
        &self.id
    }

    /// "src/a.rs @@ -3,4 +3,6 @@".
    pub fn title(&self) -> String {
        format!("{} {}", self.file, self.header)
    }
}

/// Where a station stands on the order's timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passage {
    /// The order went through it.
    Passed,
    /// The order stands there.
    Current,
    /// The log names it skipped, as the specification in stage 1.
    Skipped,
    /// Still ahead.
    Ahead,
}

/// A station on the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    station: Station,
    passage: Passage,
    words: String,
}

impl Stage {
    /// The station.
    pub fn station(&self) -> Station {
        self.station
    }

    /// Where it stands.
    pub fn passage(&self) -> Passage {
        self.passage
    }

    /// The SF Symbol of its passage, a shape of its own for each.
    pub fn symbol(&self) -> &'static str {
        match self.passage {
            Passage::Passed => "checkmark.circle.fill",
            Passage::Current => "circle.inset.filled",
            Passage::Skipped => "arrow.uturn.forward.circle",
            Passage::Ahead => "circle",
        }
    }

    /// "Bau – erledigt · 12 min · 1.20 $ · claude · opus".
    pub fn words(&self) -> &str {
        &self.words
    }
}

/// A command of an order, from its tab, the panel's and the table's menus and "Ablage ▸ Auftrag".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// G1.
    Approve,
    /// G2, with the sections the user rejected.
    Accept,
    /// Shows the diff, which G2 at class high needs first (R70).
    OpenDiff,
    /// Opens the session of the current role to read.
    OpenSession,
    /// Back to the builder with a comment.
    BackToBuilder,
    /// New criteria, one per line (E184).
    ChangeCriteria,
    /// The user takes the station over (E184).
    ByHand,
    /// The station goes back to the Leitstand (E184).
    BackToLeitstand,
    /// Goes on after a pause (E167).
    Resume,
    /// Lets a wait or a failure pass.
    Continue,
    /// Removes the worktree and ends the order, after a sheet.
    Abort,
}

impl Command {
    /// Every command, in the order the tab and the menus list them.
    pub const ALL: [Command; 11] = [
        Command::Approve,
        Command::Accept,
        Command::OpenDiff,
        Command::OpenSession,
        Command::BackToBuilder,
        Command::ChangeCriteria,
        Command::ByHand,
        Command::BackToLeitstand,
        Command::Resume,
        Command::Continue,
        Command::Abort,
    ];

    /// Its title on a button and in a menu; "…" where a dialog asks for more.
    pub fn title(self) -> &'static str {
        match self {
            Command::Approve => t("Freigeben"),
            Command::Accept => t("Abnehmen"),
            Command::OpenDiff => t("Diff öffnen"),
            Command::OpenSession => t("Sitzung öffnen"),
            Command::BackToBuilder => t("Zurück an den Builder …"),
            Command::ChangeCriteria => t("Kriterien ändern …"),
            Command::ByHand => t("Von Hand weiter"),
            Command::BackToLeitstand => t("Zurück an den Leitstand"),
            Command::Resume => t("Auftrag fortsetzen"),
            Command::Continue => t("Weiter"),
            Command::Abort => t("Auftrag abbrechen …"),
        }
    }

    /// Its place in [`Command::ALL`], the tag of its controls.
    pub fn tag(self) -> isize {
        let index = Command::ALL.iter().position(|c| *c == self).unwrap_or(0);
        isize::try_from(index).unwrap_or(0)
    }

    /// The command of `tag`.
    pub fn of_tag(tag: isize) -> Option<Command> {
        Command::ALL.get(usize::try_from(tag).ok()?).copied()
    }
}

/// What G2 waits for at class high before it can be given (R70).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptBlock {
    /// The evidence is missing or no longer valid.
    NoValidEvidence,
    /// The diff was not opened yet.
    OpenDiffFirst,
}

/// An order as its tab shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    id: OrderId,
    title: String,
    goal: String,
    criteria: Vec<String>,
    criteria_text: String,
    station: Station,
    tone: Tone,
    condition: String,
    risk: String,
    timeline: Vec<Stage>,
    request: Option<RequestId>,
    deviations: Vec<String>,
    evidence: Vec<String>,
    gates: Vec<String>,
    rounds: Vec<String>,
    costs: Vec<String>,
    provenance: Vec<String>,
    fingerprint: Option<String>,
    sections: Vec<Section>,
    enabled: BTreeSet<CommandKey>,
    accept_block: Option<AcceptBlock>,
}

/// [`Command`] as a set's key.
type CommandKey = isize;

impl Detail {
    /// The tab's model of `order` at `now` (wall clock) with the sections of its diff.
    pub fn new(order: &Order, now: u64, sections: Vec<Section>) -> Detail {
        let evidence = order.evidence();
        let deviations = evidence.map_or_else(|| order.deviations(), Evidence::deviations);
        let (enabled, accept_block) = commands(order);
        Detail {
            id: order.id().clone(),
            title: order.title().to_string(),
            goal: order.goal().to_string(),
            criteria: order
                .criteria()
                .iter()
                .map(|c| format!("{} {}", c.id(), c.text()))
                .collect(),
            criteria_text: order
                .criteria()
                .iter()
                .map(|c| c.text().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            station: order.station(),
            tone: Tone::of(order.station(), order.condition()),
            condition: words::condition(order.station(), order.condition()),
            risk: risk_words(order.expected_risk(), order.risk()),
            timeline: timeline(order, now),
            request: match order.condition() {
                Condition::Waiting(WaitReason::Question { request }) => Some(request.clone()),
                _ => None,
            },
            deviations: deviations.iter().map(words::deviation).collect(),
            evidence: evidence.map(evidence_words).unwrap_or_default(),
            gates: order.gates().iter().map(gate_words).collect(),
            rounds: rounds(order),
            costs: order
                .costs()
                .iter()
                .filter_map(|cost| {
                    let station = cost.station?;
                    Some(tf(
                        "{}: {} · {}",
                        &[
                            words::station(station),
                            &words::duration(cost.runtime_ms),
                            &words::spend(&cost.spend),
                        ],
                    ))
                })
                .collect(),
            provenance: order
                .provenance()
                .iter()
                .map(|p| {
                    let who = agent(Some(&p.agent.to_string()), p.model.as_deref());
                    let commit = p.commit.as_deref().map_or("", |c| &c[..c.len().min(8)]);
                    tf("{}: {} {}", &[words::role(p.role), &who, commit])
                        .trim_end()
                        .to_string()
                })
                .collect(),
            fingerprint: evidence.map(|e| e.fingerprint().to_string()),
            sections,
            enabled,
            accept_block,
        }
    }

    /// The order.
    pub fn id(&self) -> &OrderId {
        &self.id
    }

    /// Its title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Its goal.
    pub fn goal(&self) -> &str {
        &self.goal
    }

    /// Its criteria with their ids, one per line.
    pub fn criteria(&self) -> &[String] {
        &self.criteria
    }

    /// The criteria's texts, one per line, as "Kriterien ändern" starts with them.
    pub fn criteria_text(&self) -> &str {
        &self.criteria_text
    }

    /// Its station.
    pub fn station(&self) -> Station {
        self.station
    }

    /// Its condition's shape and word.
    pub fn tone(&self) -> Tone {
        self.tone
    }

    /// Its condition with the reason.
    pub fn condition(&self) -> &str {
        &self.condition
    }

    /// The expected and measured risk class.
    pub fn risk(&self) -> &str {
        &self.risk
    }

    /// Every station from the intake to done.
    pub fn timeline(&self) -> &[Stage] {
        &self.timeline
    }

    /// The request a session waits on, which the tab answers without the AI tab (F72).
    pub fn request(&self) -> Option<&RequestId> {
        self.request.as_ref()
    }

    /// The deviations, first in the evidence (R70).
    pub fn deviations(&self) -> &[String] {
        &self.deviations
    }

    /// Validity and fingerprint of the evidence, empty before there is one.
    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }

    /// Each gate with result, duration and log excerpt.
    pub fn gates(&self) -> &[String] {
        &self.gates
    }

    /// The reviewer's rounds with verdicts and findings.
    pub fn rounds(&self) -> &[String] {
        &self.rounds
    }

    /// Runtime and spending per station.
    pub fn costs(&self) -> &[String] {
        &self.costs
    }

    /// Where each turn's work came from.
    pub fn provenance(&self) -> &[String] {
        &self.provenance
    }

    /// The sections of the diff.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Whether `command` fits where the order stands; the others show greyed out (E101).
    pub fn can(&self, command: Command) -> bool {
        self.enabled.contains(&command.tag())
    }

    /// Why "Abnehmen" waits, while it does.
    pub fn accept_block(&self) -> Option<AcceptBlock> {
        self.accept_block
    }

    /// The input of G2 with the sections in `rejected` turned down; `None` without evidence.
    pub fn accept(&self, rejected: &BTreeSet<HunkId>) -> Option<InputKind> {
        Some(InputKind::Accept {
            order: self.id.clone(),
            fingerprint: self.fingerprint.clone()?,
            rejected: self
                .sections
                .iter()
                .filter(|s| rejected.contains(&s.id))
                .map(|s| s.id.clone())
                .collect(),
        })
    }

    /// The input of `command` that needs nothing more from the user; `None` for those that ask first.
    pub fn input(&self, command: Command) -> Option<InputKind> {
        let id = self.id.clone();
        match command {
            Command::Approve => Some(InputKind::Approve(id)),
            Command::ByHand => Some(InputKind::ByHand(id)),
            Command::BackToLeitstand => Some(InputKind::BackToLeitstand(id)),
            Command::Resume => Some(InputKind::Resume(Some(id))),
            Command::Continue => Some(InputKind::Continue(id)),
            Command::Accept
            | Command::OpenDiff
            | Command::OpenSession
            | Command::BackToBuilder
            | Command::ChangeCriteria
            | Command::Abort => None,
        }
    }
}

fn risk_words(expected: RiskClass, measured: RiskClass) -> String {
    if expected == measured {
        tf("Risikoklasse {}", &[words::risk(expected)])
    } else {
        tf(
            "Risikoklasse {}, gemessen {}",
            &[words::risk(expected), words::risk(measured)],
        )
    }
}

/// Which commands fit `order` now, as `ide_factory`'s factory takes them, and what keeps G2 back.
fn commands(order: &Order) -> (BTreeSet<CommandKey>, Option<AcceptBlock>) {
    let condition = order.condition();
    let open = !order.finished();
    let waiting = |reason: &dyn Fn(&WaitReason) -> bool| matches!(condition, Condition::Waiting(r) if reason(r));
    let valid = order.evidence().is_some_and(|e| e.invalid().is_none());
    let accept_block = if !waiting(&|r| *r == WaitReason::Acceptance) {
        None
    } else if !valid {
        Some(AcceptBlock::NoValidEvidence)
    } else if order.risk() == RiskClass::High && !order.diff_opened() {
        Some(AcceptBlock::OpenDiffFirst)
    } else {
        None
    };
    let fits = [
        (
            Command::Approve,
            waiting(&|r| matches!(r, WaitReason::SpecApproval | WaitReason::LateSpecApproval)),
        ),
        (
            Command::Accept,
            waiting(&|r| *r == WaitReason::Acceptance) && accept_block.is_none(),
        ),
        (Command::OpenDiff, order.head().is_some()),
        (Command::OpenSession, order.current().is_some()),
        (
            Command::BackToBuilder,
            open && order.reached() >= Station::Build && order.station() != Station::TakeOver,
        ),
        (Command::ChangeCriteria, open && order.station() != Station::TakeOver),
        (
            Command::ByHand,
            open && matches!(order.station(), Station::Build | Station::Review)
                && !waiting(&|r| *r == WaitReason::ByHand),
        ),
        (Command::BackToLeitstand, waiting(&|r| *r == WaitReason::ByHand)),
        (Command::Resume, matches!(condition, Condition::Paused(_))),
        (
            Command::Continue,
            matches!(condition, Condition::Failed(_)) || waiting(&passes),
        ),
        (Command::Abort, open),
    ];
    let enabled = fits
        .into_iter()
        .filter(|(_, fits)| *fits)
        .map(|(command, _)| command.tag())
        .collect();
    (enabled, accept_block)
}

/// Whether "Weiter" lets the wait for `reason` pass, as the factory's `Continue` does.
fn passes(reason: &WaitReason) -> bool {
    matches!(
        reason,
        WaitReason::AgentConfigChanged { .. }
            | WaitReason::LateSpecApproval
            | WaitReason::RefsChanged { .. }
            | WaitReason::Unclear
            | WaitReason::ReviewRoundsSpent
            | WaitReason::CancelledTurn
            | WaitReason::NoAgent
            | WaitReason::NoReviewer
            | WaitReason::TakeOverBlocked { .. }
    )
}

fn timeline(order: &Order, now: u64) -> Vec<Stage> {
    let costs = order.costs();
    Station::ALL
        .into_iter()
        .map(|station| {
            let visited = order.visits().iter().any(|v| v.station == station);
            let passage = if station == order.station() {
                Passage::Current
            } else if station == Station::Spec && order.g1_skipped() {
                Passage::Skipped
            } else if visited {
                Passage::Passed
            } else {
                Passage::Ahead
            };
            let mut parts = vec![words::station(station).to_string()];
            match passage {
                Passage::Current => parts.push(words::condition(order.station(), order.condition())),
                Passage::Skipped => parts.push(t("übersprungen").into()),
                Passage::Passed | Passage::Ahead => {}
            }
            if let Some(cost) = costs.iter().find(|c| c.station == Some(station)) {
                parts.push(words::duration(cost.runtime_ms));
                if !cost.spend.is_zero() {
                    parts.push(words::spend(&cost.spend));
                }
            }
            if let Some(entered) = order.visits().iter().rev().find(|v| v.station == station) {
                parts.push(tf("vor {}", &[&words::duration(now.saturating_sub(entered.at.wall))]));
            }
            if let Some(slot) = role_of(station).and_then(|role| order.session(role)) {
                parts.push(agent(
                    Some(&slot.entry().agent.to_string()),
                    slot.entry().model.as_deref(),
                ));
            }
            Stage {
                station,
                passage,
                words: parts.join(" · "),
            }
        })
        .collect()
}

fn role_of(station: Station) -> Option<Role> {
    match station {
        Station::Spec => Some(Role::Planner),
        Station::Build => Some(Role::Builder),
        Station::Review => Some(Role::Reviewer),
        _ => None,
    }
}

fn evidence_words(evidence: &Evidence) -> Vec<String> {
    let validity = match evidence.invalid() {
        None => t("Nachweis gültig").to_string(),
        Some(why) => tf("Nachweis ungültig: {}", &[words::invalidation(why)]),
    };
    let mut lines = vec![
        validity,
        tf("Commit {}", &[&evidence.commit()[..evidence.commit().len().min(12)]]),
        tf(
            "Fingerabdruck {}",
            &[&evidence.fingerprint()[..evidence.fingerprint().len().min(16)]],
        ),
    ];
    lines.extend(evidence.images().iter().map(|image| tf("Bild: {}", &[image])));
    lines
}

fn gate_words(record: &GateRecord) -> String {
    let name = words::gate_step(record.step, &record.name);
    let result = match (&record.failure, record.skipped) {
        (_, true) => t("übersprungen").to_string(),
        (Some(failure), false) => tf("gescheitert: {}", &[&words::gate_failure(failure)]),
        (None, false) => t("bestanden").to_string(),
    };
    let mut line = tf("{}: {}", &[&name, &result]);
    if let Some(command) = &record.command {
        line = tf("{} · {}", &[&line, &words::duration(command.duration_ms)]);
        let excerpt = command.excerpt.trim();
        if !excerpt.is_empty() {
            line = format!("{line}\n{excerpt}");
        }
    }
    line
}

fn rounds(order: &Order) -> Vec<String> {
    order
        .review_log()
        .iter()
        .enumerate()
        .map(|(index, round)| {
            let outcome = match &round.outcome {
                RoundOutcome::Passed => t("bestanden").to_string(),
                RoundOutcome::Rework { not_met } => tf("zurück an den Builder: {}", &[&not_met.join(", ")]),
                RoundOutcome::Unclear { criteria } => tf("unklar: {}", &[&criteria.join(", ")]),
                RoundOutcome::Invalid { duplicates } => {
                    tf("ungültig, doppelt beurteilt: {}", &[&duplicates.join(", ")])
                }
            };
            let mut lines = vec![tf("Runde {}: {}", &[&(index + 1).to_string(), &outcome])];
            for verdict in &round.verdicts {
                let judged = match verdict.verdict {
                    Judgement::Met => t("erfüllt"),
                    Judgement::NotMet => t("nicht erfüllt"),
                    Judgement::Unclear => t("unklar"),
                };
                lines.push(tf("{}: {} – {}", &[&verdict.id, judged, &verdict.reason]));
            }
            for finding in &round.findings {
                let place = finding
                    .line
                    .map_or_else(|| finding.file.clone(), |line| format!("{}:{line}", finding.file));
                lines.push(tf("Befund {}: {}", &[&place, &finding.text]));
            }
            lines.join("\n")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_comes_back_from_its_tag() {
        for command in Command::ALL {
            assert_eq!(Command::of_tag(command.tag()), Some(command));
        }
        assert_eq!(Command::of_tag(-1), None);
    }
}
