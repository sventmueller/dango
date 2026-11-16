//! The Leitstand's words in the user's language (E141): stations, conditions and their reasons, deviations, gates,
//! refusals and the sheet's problems, all of them typed values of `ide_factory` until here.

use ide_factory::budget::{BudgetScope, Spend, Unit};
use ide_factory::evidence::{Deviation, Invalidation};
use ide_factory::gates::{GateFailure, GateStep};
use ide_factory::options::{ChangeRefusal, OptionProblem, SheetNote};
use ide_factory::panel::Hold;
use ide_factory::station::{Condition, FailReason, PauseReason, RiskClass, Role, Station, TakeOverBlocker, WaitReason};
use ide_factory::workflow::recurring::Interval;
use ide_factory::workflow::rules::Loosening;
use ide_factory::workflow::{Expected, WorkflowError, WorkflowErrorKind};
use ide_factory::{Refusal, Refused, RestoreProblem};
use splat_i18n::{t, tf};

/// The name of `station`.
pub fn station(station: Station) -> &'static str {
    match station {
        Station::Intake => t("Eingang"),
        Station::Spec => t("Spezifikation"),
        Station::Build => t("Bau"),
        Station::Review => t("Prüfung"),
        Station::Acceptance => t("Abnahme"),
        Station::TakeOver => t("Übernahme"),
        Station::Done => t("Erledigt"),
    }
}

/// The name of `role`.
pub fn role(role: Role) -> &'static str {
    match role {
        Role::Planner => t("Planer"),
        Role::Builder => t("Builder"),
        Role::Reviewer => t("Prüfer"),
        Role::Scout => t("Späher"),
    }
}

/// The name of `class`.
pub fn risk(class: RiskClass) -> &'static str {
    match class {
        RiskClass::Low => t("niedrig"),
        RiskClass::Medium => t("mittel"),
        RiskClass::High => t("hoch"),
    }
}

/// How a condition looks: its shape and words, never its colour alone (E101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    /// The factory works on it.
    Running,
    /// It waits for the user: the red dot (E71).
    Waiting,
    /// It paused by itself.
    Paused,
    /// Its station failed.
    Failed,
    /// The user aborted it.
    Aborted,
    /// It is taken over.
    Done,
}

impl Tone {
    /// The tone of an order in `station` under `condition`.
    pub fn of(station: Station, condition: &Condition) -> Tone {
        match condition {
            _ if station == Station::Done => Tone::Done,
            Condition::Running => Tone::Running,
            Condition::Waiting(_) => Tone::Waiting,
            Condition::Paused(_) => Tone::Paused,
            Condition::Failed(_) => Tone::Failed,
            Condition::Aborted => Tone::Aborted,
        }
    }

    /// The SF Symbol that shows it, a different shape for each tone.
    pub fn symbol(self) -> &'static str {
        match self {
            Tone::Running => "play.circle",
            Tone::Waiting => "exclamationmark.circle.fill",
            Tone::Paused => "pause.circle",
            Tone::Failed => "xmark.octagon",
            Tone::Aborted => "slash.circle",
            Tone::Done => "checkmark.circle",
        }
    }

    /// Its word, which VoiceOver reads with the symbol.
    pub fn word(self) -> &'static str {
        match self {
            Tone::Running => t("läuft"),
            Tone::Waiting => t("wartet"),
            Tone::Paused => t("pausiert"),
            Tone::Failed => t("gescheitert"),
            Tone::Aborted => t("abgebrochen"),
            Tone::Done => t("erledigt"),
        }
    }
}

/// What a condition says, with its reason: "wartet: Nachweis abnehmen".
pub fn condition(station: Station, condition: &Condition) -> String {
    let tone = Tone::of(station, condition);
    let reason = match condition {
        _ if tone == Tone::Done => None,
        Condition::Running | Condition::Aborted => None,
        Condition::Waiting(why) => Some(wait(why)),
        Condition::Paused(why) => Some(pause(why)),
        Condition::Failed(why) => Some(fail(why)),
    };
    match reason {
        Some(reason) => tf("{}: {}", &[tone.word(), &reason]),
        None => tone.word().to_string(),
    }
}

/// What a condition says, or why an order that runs is held back for now (E168, E174, R68).
pub fn status(station: Station, condition: &Condition, held: Option<&Hold>) -> String {
    match held {
        Some(Hold::Slot { limit }) => tf("eingereiht: höchstens {} Aufträge gleichzeitig", &[&limit.to_string()]),
        Some(Hold::Overlap { title }) => tf("eingereiht: Pfadbereich überschneidet sich mit „{}“", &[title]),
        Some(Hold::Queue { branch }) => tf("eingereiht: Übernahme in {} nach dem Auftrag davor", &[branch]),
        None => self::condition(station, condition),
    }
}

/// Why an order waits for the user.
pub fn wait(reason: &WaitReason) -> String {
    match reason {
        WaitReason::SpecApproval => t("Spezifikation freigeben").into(),
        WaitReason::Acceptance => t("Nachweis abnehmen").into(),
        WaitReason::Question { .. } => t("Rückfrage des Agenten").into(),
        WaitReason::CancelledTurn => t("Zug abgebrochen").into(),
        WaitReason::AgentConfigChanged { paths } => tf("Agenten-Konfiguration geändert: {}", &[&paths.join(", ")]),
        WaitReason::LateSpecApproval => t("Spezifikation nachträglich freigeben").into(),
        WaitReason::RefsChanged { refs } => tf("Refs des Repositorys geändert: {}", &[&refs.join(", ")]),
        WaitReason::GateCommandsChanged => t("Prüfbefehle geändert, bitte bestätigen").into(),
        WaitReason::NoGatePlan => t("Kein Prüfplan").into(),
        WaitReason::NoReviewer => t("Kein Prüfer eines anderen Herstellers bereit").into(),
        WaitReason::NoAgent => t("Kein Agent der Rolle bereit").into(),
        WaitReason::Unclear => t("Prüfer unsicher").into(),
        WaitReason::ReviewRoundsSpent => t("Prüfrunden aufgebraucht").into(),
        WaitReason::ByHand => t("Von Hand").into(),
        WaitReason::TakeOverBlocked {
            blocker: TakeOverBlocker::OtherBranch(branch),
        } => tf("Arbeitskopie auf Branch {}", &[branch]),
        WaitReason::TakeOverBlocked {
            blocker: TakeOverBlocker::Dirty,
        } => t("Arbeitskopie hat uncommittete Änderungen").into(),
    }
}

/// Why an order paused.
pub fn pause(reason: &PauseReason) -> String {
    match reason {
        PauseReason::Budget { scope } => tf("Budget aufgebraucht ({})", &[&budget_scope(scope)]),
        PauseReason::Quota => t("Kontingent erschöpft").into(),
        PauseReason::Sleep => t("Mac schläft").into(),
        PauseReason::AppEnded => t("App beendet").into(),
        PauseReason::LogUnwritable => t("Protokoll nicht schreibbar").into(),
        PauseReason::InvalidWorkflow => t("Workflow-Datei ungültig").into(),
        PauseReason::NoWindow => t("Kein Fenster des Ordners").into(),
        PauseReason::StreamGap => t("Lücke im Ereignisstrom").into(),
    }
}

fn budget_scope(scope: &BudgetScope) -> String {
    match scope {
        BudgetScope::Order => t("Auftrag").into(),
        BudgetScope::Day => t("Tag").into(),
        BudgetScope::Provider(service) => service.clone(),
    }
}

/// Why a station failed.
pub fn fail(reason: &FailReason) -> String {
    match reason {
        FailReason::AttemptsSpent => t("Alle Versuche gescheitert").into(),
        FailReason::TurnsSpent => t("Züge aufgebraucht").into(),
        FailReason::Effect { effect, detail } => tf("{}: {}", &[effect, detail]),
    }
}

/// A deviation of the evidence (E163).
pub fn deviation(deviation: &Deviation) -> String {
    match deviation {
        Deviation::SecretsNotChecked => t("Secrets nicht geprüft (gitleaks fehlt)").into(),
        Deviation::AgentConfigChanged { paths } => tf("Agenten-Konfiguration geändert: {}", &[&paths.join(", ")]),
        Deviation::ChecksChanged { paths } => tf("Prüfrelevante Dateien geändert: {}", &[&paths.join(", ")]),
        Deviation::SameVendorReview => t("Prüfung ohne Herstellerwechsel").into(),
        Deviation::AgentSwitched { role: r, from, to } => tf(
            "Agentenwechsel als {}: {} → {}",
            &[role(*r), &from.to_string(), &to.to_string()],
        ),
        Deviation::ModelSwitched { role: r, model } => tf("Modellwechsel als {}: {}", &[role(*r), model]),
        Deviation::Interventions { count } => tf("Eingriffe: {}", &[&count.to_string()]),
        Deviation::ByHand => t("Von Hand bearbeitet").into(),
        Deviation::Unmetered => t("Verbrauch ohne verlässliche Zahl").into(),
        Deviation::BudgetOvershoot => t("Budget um einen Zug überschritten").into(),
        Deviation::QuotaUnknown => t("Kontingent unbekannt").into(),
        Deviation::CredentialsCopied => t("Dateien mit Zugangsdaten mitkopiert").into(),
        Deviation::BaseUpdated => t("Basis aktualisiert").into(),
        Deviation::TieBreak {
            role: r,
            chosen,
            over,
            percent,
            over_percent,
        } => tf(
            "Gleichstand als {} nach Erstdurchlaufquote entschieden: {} ({} %) vor {} ({} %)",
            &[
                role(*r),
                &super::list::agent(Some(&chosen.agent.to_string()), chosen.model.as_deref()),
                &percent.to_string(),
                &super::list::agent(Some(&over.agent.to_string()), over.model.as_deref()),
                &over_percent.to_string(),
            ],
        ),
    }
}

/// Why evidence is no longer valid.
pub fn invalidation(why: Invalidation) -> &'static str {
    match why {
        Invalidation::Tree => t("Stand geändert"),
        Invalidation::Rules => t("Regelsatz geändert"),
        Invalidation::Base => t("Basis geändert"),
        Invalidation::Manifest => t("Eingaben geändert"),
        Invalidation::Criteria => t("Kriterien geändert"),
        Invalidation::ByHand => t("Von Hand bearbeitet"),
        Invalidation::PartialAcceptance => t("Abschnitte verworfen"),
    }
}

/// The name of a step of the review row; a gate command's is its own.
pub fn gate_step(step: GateStep, name: &str) -> String {
    match step {
        GateStep::PathScope => t("Pfadbereich").into(),
        GateStep::Secrets => t("Secrets-Prüfung").into(),
        GateStep::Command(_) => name.to_string(),
        GateStep::Reviewer => t("Prüfer").into(),
    }
}

/// Why a gate failed.
pub fn gate_failure(failure: &GateFailure) -> String {
    match failure {
        GateFailure::OutsideScope { paths } => tf("Außerhalb des Pfadbereichs: {}", &[&paths.join(", ")]),
        GateFailure::Secrets { count } => tf("Secrets gefunden: {}", &[&count.to_string()]),
        GateFailure::Exit { code } => tf("Exit-Status {}", &[&code.to_string()]),
        GateFailure::TimedOut => t("Zeit überschritten").into(),
        GateFailure::OutputCut => t("Ausgabe zu lang").into(),
        GateFailure::Ended => t("Ohne Status beendet").into(),
        GateFailure::NoTests => t("Keine Tests ausgeführt").into(),
        GateFailure::Reviewer { not_met } => tf("Nicht erfüllt: {}", &[&not_met.join(", ")]),
        GateFailure::LeftBehind { paths } => tf("Hinterlassene Dateien: {}", &[&paths.join(", ")]),
    }
}

/// What `spend` used, in the units that counted: "1.20 $ · 34000 Tokens".
pub fn spend(spend: &Spend) -> String {
    let mut parts = Vec::new();
    if spend.dollars() > 0.0 {
        parts.push(dollars(spend.dollars()));
    }
    if spend.tokens() > 0 {
        parts.push(tf("{} Tokens", &[&spend.tokens().to_string()]));
    }
    if spend.window_share() > 0.0 {
        parts.push(tf("{} % Fenster", &[&format!("{:.0}", spend.window_share() * 100.0)]));
    }
    if parts.is_empty() {
        return t("nichts").into();
    }
    parts.join(" · ")
}

/// An amount of dollars.
pub fn dollars(amount: f64) -> String {
    tf("{} $", &[&format!("{amount:.2}")])
}

/// A unit of spending.
pub fn unit(unit: Unit) -> &'static str {
    match unit {
        Unit::Dollars => t("Dollar"),
        Unit::Tokens => t("Tokens"),
        Unit::WindowShare => t("Fensteranteil"),
    }
}

/// A span of time, to the minute once it is longer than one: "45 s", "12 min", "2 h 5 min".
pub fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        return tf("{} s", &[&seconds.to_string()]);
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return tf("{} min", &[&minutes.to_string()]);
    }
    tf(
        "{} h {} min",
        &[&(minutes / 60).to_string(), &(minutes % 60).to_string()],
    )
}

/// An amount of disk space.
pub fn bytes(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    if bytes >= GIB {
        tf("{} GB", &[&format!("{:.1}", ratio(bytes, GIB))])
    } else {
        tf("{} MB", &[&(bytes / MIB).to_string()])
    }
}

#[expect(clippy::cast_precision_loss, reason = "a size shown to one decimal")]
fn ratio(a: u64, b: u64) -> f64 {
    a as f64 / b as f64
}

/// Why the factory refused a command.
pub fn refusal(refusal: &Refusal) -> String {
    match &refusal.why {
        Refused::UnknownOrder => t("Diesen Auftrag gibt es nicht mehr.").into(),
        Refused::Exists => t("Einen Auftrag mit dieser Kennung gibt es schon.").into(),
        Refused::Sheet { check } => check.problems.iter().map(problem).collect::<Vec<_>>().join(" "),
        Refused::NotNow => t("Das geht an dieser Stelle des Auftrags nicht.").into(),
        Refused::StaleEvidence => t("Der Nachweis ist nicht mehr aktuell.").into(),
        Refused::OpenDiffFirst => t("Bei hoher Risikoklasse zuerst den Diff öffnen.").into(),
        Refused::Option { refusal } => change_refusal(refusal),
        Refused::StaleConfirmation => t("Die Prüfbefehle haben sich inzwischen geändert.").into(),
    }
}

/// Why an option cannot change now.
pub fn change_refusal(refusal: &ChangeRefusal) -> String {
    match refusal {
        ChangeRefusal::StationBegun { station: s } => tf("Die Station {} hat schon begonnen.", &[station(*s)]),
        ChangeRefusal::BelowSpent { unit: u } => tf("Weniger als schon verbraucht ({}).", &[unit(*u)]),
        ChangeRefusal::Problem { problem: p } => problem(p),
    }
}

/// What keeps the sheet's order from starting.
pub fn problem(problem: &OptionProblem) -> String {
    match problem {
        OptionProblem::MissingTitle => t("Der Titel fehlt.").into(),
        OptionProblem::MissingGoal => t("Das Ziel fehlt.").into(),
        OptionProblem::MissingCriteria => t("Mindestens ein Kriterium fehlt.").into(),
        OptionProblem::BadScope { pattern, message } => tf("Das Muster „{}“ ist ungültig: {}", &[pattern, message]),
        OptionProblem::CannotTakeRole { role: r } => tf("Der gewählte Agent kann nicht {} sein.", &[role(*r)]),
        OptionProblem::SameVendor => t("Builder und Prüfer stammen vom selben Hersteller.").into(),
        OptionProblem::SkipNeedsLowRisk => t("Ohne Freigabe nur bei Risikoklasse niedrig.").into(),
        OptionProblem::SkipNotAllowed => t("Das Projekt erlaubt keine Spezifikation ohne Freigabe.").into(),
        OptionProblem::SkipNotForIssues => t("Aufträge aus Issues brauchen immer die Freigabe.").into(),
        OptionProblem::InvalidWorkflow => t("Die Workflow-Datei ist ungültig.").into(),
    }
}

/// What the sheet says without keeping the order from starting.
pub fn note(note: &SheetNote) -> &'static str {
    match note {
        SheetNote::StartsAtHead => {
            t("Die Arbeitskopie hat uncommittete Änderungen; der Auftrag beginnt ohne sie bei HEAD.")
        }
        SheetNote::WithoutVendorChange => t("Ohne Herstellerwechsel: Der Prüfer stammt vom Hersteller eines Builders."),
    }
}

/// What the panel and the Leitstand tab say while a folder has no order.
pub fn no_order_yet() -> String {
    tf(
        "Noch kein Auftrag. Ein Auftrag ist ein Ziel mit prüfbaren Kriterien, das {} in einem eigenen Worktree baut und prüft.",
        &[ide_product::NAME],
    )
}

/// What a loosening the workflow file asks for allows, for the user to confirm on this Mac (E190).
pub fn loosening(loosening: &Loosening) -> String {
    match loosening {
        Loosening::SkipSpecApproval => {
            t("Die Workflow-Datei lässt Aufträge niedriger Risikoklasse ohne Freigabe der Spezifikation laufen.").into()
        }
        Loosening::IssueIntake { intake } => tf(
            "Die Workflow-Datei nimmt Issues mit dem Label „{}“ von {} automatisch als Aufträge an.",
            &[&intake.label, &intake.authors.join(", ")],
        ),
        Loosening::ReviewerPreferred => t("Die Workflow-Datei lässt einen Prüfer desselben Herstellers zu.").into(),
        Loosening::CopyCredentials { paths } => tf(
            "Die Workflow-Datei kopiert Dateien mit Zugangsdaten in die Worktrees: {}",
            &[&paths.join(", ")],
        ),
        Loosening::Tools { role, tools } => tf(
            "Die Workflow-Datei schaltet für die Rolle {} Werkzeuge ein: {}",
            &[self::role(*role), &tools.join(", ")],
        ),
        Loosening::RecurringOrders { orders } => {
            let list: Vec<String> = orders
                .iter()
                .map(|o| tf("„{}“ ({})", &[o.title(), &interval(o.interval())]))
                .collect();
            tf(
                "Die Workflow-Datei legt wiederkehrende Aufträge selbst an: {}",
                &[&list.join(", ")],
            )
        }
    }
}

/// How often a recurring order comes: "wöchentlich", "alle 14 Tage".
pub fn interval(interval: Interval) -> String {
    match interval {
        Interval::Daily => t("täglich").into(),
        Interval::Weekly => t("wöchentlich").into(),
        Interval::Days(days) => tf("alle {} Tage", &[&days.to_string()]),
    }
}

/// What went wrong reading an order's log at the start (E167).
pub fn restore_problem(problem: &RestoreProblem) -> String {
    let order = problem
        .order
        .as_ref()
        .map_or_else(|| t("unbekannt").to_string(), |o| o.0.clone());
    if problem.unreadable {
        tf("Das Protokoll des Auftrags {} ließ sich nicht lesen.", &[&order])
    } else if let Some(line) = problem.stopped_at {
        tf(
            "Das Protokoll des Auftrags {} ist ab Zeile {} unlesbar.",
            &[&order, &line.to_string()],
        )
    } else {
        tf(
            "Die letzte Zeile im Protokoll des Auftrags {} war abgeschnitten und fehlt.",
            &[&order],
        )
    }
}

/// What is wrong with the workflow file, with its key and place.
pub fn workflow_error(error: &WorkflowError) -> String {
    let what = match &error.kind {
        WorkflowErrorKind::Syntax { message } => tf("Kein gültiges TOML: {}", &[message]),
        WorkflowErrorKind::UnknownKey => t("Unbekannter Schlüssel").into(),
        WorkflowErrorKind::WrongKind { expected } => tf("Erwartet: {}", &[&expected_words(*expected)]),
        WorkflowErrorKind::NewerVersion { found, supported } => tf(
            "Version {} ist neuer als die gelesene Version {}",
            &[&found.to_string(), &supported.to_string()],
        ),
        WorkflowErrorKind::Pattern { pattern, message } => tf("Das Muster „{}“ ist ungültig: {}", &[pattern, message]),
        WorkflowErrorKind::EmptyCommand => t("Prüfbefehl ohne Befehl").into(),
        WorkflowErrorKind::DuplicateGate { name } => tf("Zwei Prüfbefehle heißen „{}“", &[name]),
        WorkflowErrorKind::CopyOutsideRepository { path } => tf("„{}“ liegt außerhalb des Repositorys", &[path]),
        WorkflowErrorKind::ImagesInsideRepository { path } => tf("„{}“ liegt im Repository", &[path]),
        WorkflowErrorKind::BadVariableName { name } => tf("„{}“ ist kein gültiger Variablenname", &[name]),
        WorkflowErrorKind::Missing => t("Fehlt oder ist leer").into(),
        WorkflowErrorKind::BadInterval { value } => tf(
            "„{}“ ist kein Abstand; erlaubt sind daily, weekly oder Tage wie 14d",
            &[value],
        ),
        WorkflowErrorKind::DuplicateRecurring { title } => tf("Zwei wiederkehrende Aufträge heißen „{}“", &[title]),
    };
    match (error.key.is_empty(), error.line) {
        (true, None) => what,
        (true, Some(line)) => tf("Zeile {}: {}", &[&line.to_string(), &what]),
        (false, None) => tf("{}: {}", &[&error.key, &what]),
        (false, Some(line)) => tf("{} (Zeile {}): {}", &[&error.key, &line.to_string(), &what]),
    }
}

fn expected_words(expected: Expected) -> String {
    match expected {
        Expected::Text => t("ein Text").into(),
        Expected::Count => t("eine ganze Zahl").into(),
        Expected::Number => t("eine Zahl").into(),
        Expected::Switch => t("true oder false").into(),
        Expected::Texts => t("eine Liste von Texten").into(),
        Expected::Table => t("eine Tabelle").into(),
        Expected::Tables => t("eine Liste von Tabellen").into(),
        Expected::OneOf(words) => words.join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_of_time_read_to_the_minute() {
        assert_eq!(duration(45_000), "45 s");
        assert_eq!(duration(12 * 60_000 + 5_000), "12 min");
        assert_eq!(duration(125 * 60_000), "2 h 5 min");
    }

    #[test]
    fn every_tone_has_a_shape_of_its_own() {
        let tones = [
            Tone::Running,
            Tone::Waiting,
            Tone::Paused,
            Tone::Failed,
            Tone::Aborted,
            Tone::Done,
        ];
        let shapes: std::collections::HashSet<&str> = tones.iter().map(|t| t.symbol()).collect();
        assert_eq!(shapes.len(), tones.len());
    }

    #[test]
    fn a_waiting_order_names_its_reason() {
        let words = condition(Station::Acceptance, &Condition::Waiting(WaitReason::Acceptance));
        assert!(words.contains(t("Nachweis abnehmen")), "{words}");
        assert_eq!(condition(Station::Done, &Condition::Running), t("erledigt"));
    }
}
