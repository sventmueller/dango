//! The tab of one order (E175): its timeline from the log, the request that waits for the user as a card, the
//! evidence package with the deviations first, the sections of its diff for a partial acceptance (C55), and which of
//! the order's commands fit where it stands; built from `ide_factory`'s order, the view only shows it.

use std::collections::BTreeSet;

use ide_factory::evidence::{Evidence, GateRecord};
use ide_factory::intake::{DiffLine, line_diff};
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

    /// "Bau – erledigt · 12 min · 1,20 $ · claude · opus".
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
    /// G2 that keeps the branch and then publishes it as a merge request (E177).
    AcceptAsChange,
    /// G2 without the take-over: the order's branch stays, its worktree goes (E177 without O2).
    KeepBranch,
    /// Shows the diff, which G2 at class high needs first (R70).
    OpenDiff,
    /// Opens the session of the current role to read.
    OpenSession,
    /// Pushes the kept branch, opens a merge request and comments the order's issue, or goes on where that stopped
    /// (E177).
    Publish,
    /// Opens the merge request publishing opened or found, in the browser tab (E77).
    ShowChange,
    /// Opens the forge's page for a new merge request of the branch the order's work is on, in the browser tab (E77,
    /// E177 without O2).
    MergeRequest,
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
    pub const ALL: [Command; 16] = [
        Command::Approve,
        Command::Accept,
        Command::AcceptAsChange,
        Command::KeepBranch,
        Command::OpenDiff,
        Command::OpenSession,
        Command::Publish,
        Command::ShowChange,
        Command::MergeRequest,
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
            Command::AcceptAsChange => t("Abnehmen und Merge Request"),
            Command::KeepBranch => t("Abnehmen und Branch behalten"),
            Command::Publish => t("Als Merge Request"),
            Command::ShowChange => t("Merge Request öffnen"),
            Command::MergeRequest => t("Merge Request anlegen"),
            Command::OpenDiff => t("Diff öffnen"),
            Command::OpenSession => t("Sitzung öffnen"),
            Command::BackToBuilder => t("Zurück an den Builder …"),
            Command::ChangeCriteria => t("Kriterien ändern …"),
            Command::ByHand => t("Von Hand weiter"),
            Command::BackToLeitstand => t("Zurück an die Software Factory"),
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

/// Whether the folder can publish to GitHub or GitLab, as the app last found it (E177).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ForgeAccess {
    /// Not looked at yet.
    #[default]
    Unknown,
    /// No remote on a service the app knows.
    NoRemote,
    /// The service is known, but no token for its host.
    NoToken {
        /// The host.
        host: String,
    },
    /// A remote on a known service with a token for its host.
    Ready,
}

impl ForgeAccess {
    /// Why publishing cannot start; `None` when it can.
    pub fn reason(&self) -> Option<String> {
        match self {
            ForgeAccess::Ready => None,
            ForgeAccess::Unknown => Some(t("Das Remote des Ordners wird noch geprüft …").into()),
            ForgeAccess::NoRemote => Some(t("Der Ordner hat kein Remote bei GitHub oder GitLab.").into()),
            ForgeAccess::NoToken { host } => Some(tf(
                "Für {} ist kein Token da. Im CI-Panel anmelden; GitLab braucht den Bereich „api“.",
                &[host],
            )),
        }
    }
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
    /// The specification while it waits for G1.
    spec: Option<String>,
    /// The branch the order's work is on once it is done: the kept branch, or the base it was taken over into.
    merge_branch: Option<String>,
    /// The builder's objection to the criteria while the order waits on it (E178).
    objection: Option<String>,
    /// The lines of the difference between the issue as the order froze it and as it reads now (E176).
    issue_change: Option<Vec<String>>,
    /// Whether the issue's change can be taken over now.
    issue_takeable: bool,
    /// What goes to GitHub or GitLab when the user publishes (E177).
    outgoing: ide_factory::publish::Outgoing,
    /// How far publishing got, a line each.
    publication: Vec<String>,
    /// The merge request publishing opened or found.
    change_url: Option<String>,
    /// The commands of publishing that fit where the order stands, whether the folder can publish or not.
    publish_fits: Vec<Command>,
    forge: ForgeAccess,
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
            spec: order
                .spec()
                .filter(|_| *order.condition() == Condition::Waiting(WaitReason::SpecApproval))
                .map(str::to_string),
            merge_branch: merge_branch(order),
            objection: match order.condition() {
                Condition::Waiting(WaitReason::CriteriaObjection { text }) => Some(text.clone()),
                _ => None,
            },
            issue_change: issue_change(order),
            issue_takeable: order.issue_update().is_some() && !order.finished() && order.station() != Station::TakeOver,
            outgoing: super::change::outgoing(order),
            publication: super::change::lines(order),
            change_url: order
                .publication()
                .and_then(|p| p.change())
                .map(|c| c.url().to_string()),
            publish_fits: publish_fits(order, accept_block),
            forge: ForgeAccess::Unknown,
        }
    }

    /// The model with what the app found of the folder's service: publishing is offered only with a remote there and
    /// a token for its host (E177).
    pub fn with_forge(mut self, forge: ForgeAccess) -> Detail {
        for command in &self.publish_fits {
            if forge == ForgeAccess::Ready {
                self.enabled.insert(command.tag());
            } else {
                self.enabled.remove(&command.tag());
            }
        }
        self.forge = forge;
        self
    }

    /// Why "Als Merge Request" cannot start although the order would let it.
    pub fn publish_block(&self) -> Option<String> {
        self.publish_fits.first().and_then(|_| self.forge.reason())
    }

    /// How far publishing got, a line each; empty before the user asked.
    pub fn publication(&self) -> &[String] {
        &self.publication
    }

    /// The page of the merge request publishing opened or found.
    pub fn change_url(&self) -> Option<&str> {
        self.change_url.as_deref()
    }

    /// The builder's objection to the criteria, while the order waits on it (E178).
    pub fn objection(&self) -> Option<&str> {
        self.objection.as_deref()
    }

    /// How the order's issue changed since the order came in, a line each with "- " for removed and "+ " for added
    /// text, while the change waits for the user (E176).
    pub fn issue_change(&self) -> Option<&[String]> {
        self.issue_change.as_deref()
    }

    /// The input that takes the issue's change over, while it can be.
    pub fn take_issue_change(&self) -> Option<InputKind> {
        self.issue_takeable.then(|| InputKind::TakeIssueChange(self.id.clone()))
    }

    /// The specification while it waits for G1 (E178), which the tab shows with its comments.
    pub fn spec(&self) -> Option<&str> {
        self.spec.as_deref()
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

    /// The model with the condition saying why the order, which runs, is held back for now (E174, R68).
    pub fn with_hold(mut self, held: Option<&ide_factory::panel::Hold>) -> Detail {
        if held.is_some() {
            self.condition = words::status(self.station, &Condition::Running, held);
        }
        self
    }

    /// The branch "Merge Request anlegen" opens a merge request for, once the order is done.
    pub fn merge_branch(&self) -> Option<&str> {
        self.merge_branch.as_deref()
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

    /// The input of `command` that needs nothing more from the user, G2 without the sections in `rejected`; `None` for
    /// those that ask first.
    pub fn input(&self, command: Command, rejected: &BTreeSet<HunkId>) -> Option<InputKind> {
        let id = self.id.clone();
        let turned_down = || -> Vec<HunkId> {
            self.sections
                .iter()
                .filter(|s| rejected.contains(&s.id))
                .map(|s| s.id.clone())
                .collect()
        };
        match command {
            Command::Approve => Some(InputKind::Approve(id)),
            Command::KeepBranch => Some(InputKind::KeepBranch {
                order: id,
                fingerprint: self.fingerprint.clone()?,
                rejected: turned_down(),
            }),
            Command::AcceptAsChange => Some(InputKind::Publish {
                order: id,
                fingerprint: Some(self.fingerprint.clone()?),
                rejected: turned_down(),
                texts: self.outgoing.clone(),
            }),
            Command::Publish => Some(InputKind::Publish {
                order: id,
                fingerprint: None,
                rejected: Vec::new(),
                texts: self.outgoing.clone(),
            }),
            Command::ByHand => Some(InputKind::ByHand(id)),
            Command::BackToLeitstand => Some(InputKind::BackToLeitstand(id)),
            Command::Resume => Some(InputKind::Resume(Some(id))),
            Command::Continue => Some(InputKind::Continue(id)),
            Command::Accept
            | Command::OpenDiff
            | Command::OpenSession
            | Command::ShowChange
            | Command::MergeRequest
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
        (
            Command::KeepBranch,
            waiting(&|r| *r == WaitReason::Acceptance) && accept_block.is_none(),
        ),
        (Command::MergeRequest, merge_branch(order).is_some()),
        (
            Command::ShowChange,
            order.publication().is_some_and(|p| p.change().is_some()),
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

/// The commands of publishing that fit `order` as the factory takes them: G2 with publishing while G2 can be given,
/// publishing a done order whose branch was kept until everything is out, or again after it stopped (E177).
fn publish_fits(order: &Order, accept_block: Option<AcceptBlock>) -> Vec<Command> {
    let acceptable = *order.condition() == Condition::Waiting(WaitReason::Acceptance)
        && accept_block.is_none()
        && order.evidence().is_some_and(|e| e.invalid().is_none());
    [
        (Command::AcceptAsChange, acceptable),
        (Command::Publish, order.can_publish()),
    ]
    .into_iter()
    .filter(|(_, fits)| *fits)
    .map(|(command, _)| command)
    .collect()
}

/// The branch a merge request of the done `order` brings in: the kept branch, or the base it was taken over into.
fn merge_branch(order: &Order) -> Option<String> {
    if order.station() != Station::Done {
        None
    } else if order.branch_kept() {
        Some(order.id().branch())
    } else if order.taken_over() {
        Some(order.base().branch.clone())
    } else {
        None
    }
}

/// Whether "Weiter" lets the wait for `reason` pass, as the factory's `Continue` does.
fn passes(reason: &WaitReason) -> bool {
    matches!(
        reason,
        WaitReason::AgentConfigChanged { .. }
            | WaitReason::CriteriaObjection { .. }
            | WaitReason::LateSpecApproval
            | WaitReason::RefsChanged { .. }
            | WaitReason::Unclear
            | WaitReason::GateCommandsChanged
            | WaitReason::ReviewRoundsSpent
            | WaitReason::CancelledTurn
            | WaitReason::NoAgent
            | WaitReason::NoReviewer
            | WaitReason::TakeOverBlocked { .. }
    )
}

/// The difference of the issue's text since the order came in, while the change waits for the user.
fn issue_change(order: &Order) -> Option<Vec<String>> {
    let now = order.issue_update()?;
    let before = &order.options().issue.as_ref()?.body;
    let lines = line_diff(before, now)
        .into_iter()
        .map(|line| match line {
            DiffLine::Kept(text) => format!("  {text}"),
            DiffLine::Removed(text) => format!("- {text}"),
            DiffLine::Added(text) => format!("+ {text}"),
        })
        .collect();
    Some(lines)
}

fn timeline(order: &Order, now: u64) -> Vec<Stage> {
    let costs = order.costs();
    let visits: Vec<Station> = order.visits().iter().map(|v| v.station).collect();
    Station::ALL
        .into_iter()
        .map(|station| {
            let passage = passage(station, order.station(), &visits, order.g1_skipped());
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

/// Where `station` stands for an order at `current` that entered the stations `visits` in this order: passed only if
/// the order left it forward and has not gone back before it since, as "Zurück an den Builder" or a partial
/// acceptance does.
fn passage(station: Station, current: Station, visits: &[Station], spec_skipped: bool) -> Passage {
    if station == current {
        return Passage::Current;
    }
    if station == Station::Spec && spec_skipped {
        return Passage::Skipped;
    }
    let Some(last) = visits.iter().rposition(|s| *s == station) else {
        return Passage::Ahead;
    };
    let since = &visits[last + 1..];
    if !since.is_empty() && since.iter().all(|s| *s > station) {
        Passage::Passed
    } else {
        Passage::Ahead
    }
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

    #[test]
    fn a_station_the_order_went_back_from_is_not_passed() {
        use Station::{Acceptance, Build, Intake, Review, Spec, TakeOver};
        let back = [Intake, Build, Review, Build];
        assert_eq!(passage(Intake, Build, &back, true), Passage::Passed);
        assert_eq!(passage(Spec, Build, &back, true), Passage::Skipped);
        assert_eq!(passage(Build, Build, &back, true), Passage::Current);
        assert_eq!(passage(Review, Build, &back, true), Passage::Ahead);
        let partial = [Intake, Spec, Build, Review, Acceptance, Review];
        assert_eq!(passage(Spec, Review, &partial, false), Passage::Passed);
        assert_eq!(passage(Build, Review, &partial, false), Passage::Passed);
        assert_eq!(passage(Review, Review, &partial, false), Passage::Current);
        assert_eq!(passage(Acceptance, Review, &partial, false), Passage::Ahead);
        assert_eq!(passage(TakeOver, Review, &partial, false), Passage::Ahead);
        let again = [Intake, Build, Review, Build, Review, Acceptance];
        assert_eq!(passage(Review, Acceptance, &again, false), Passage::Passed);
    }
}
