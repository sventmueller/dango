//! An order's merge request in the user's words (E177): the texts that go to GitHub or GitLab, a summary of the
//! evidence with the deviations first, and how far publishing got. No session transcripts, no file contents, no gate
//! logs: only what the evidence package states.

use ide_factory::evidence::{Evidence, GateRecord};
use ide_factory::order::Order;
use ide_factory::publish::{CHANGE_PLACEHOLDER, Outgoing, PublishFailure, PublishStep};
use ide_factory::station::Station;
use ide_factory::verdict::Judgement;
use splat_i18n::{t, tf};

use super::words;

/// What goes out for `order`: its title, the body with the evidence's summary, and the comment for its issue.
pub fn outgoing(order: &Order) -> Outgoing {
    let comment = tf(
        "Der Auftrag „{}“ ist abgenommen und als Merge Request eingereicht: {}",
        &[order.title(), CHANGE_PLACEHOLDER],
    );
    Outgoing::new(order.title(), &body(order), &comment)
}

fn body(order: &Order) -> String {
    let evidence = order.evidence();
    let deviations: Vec<String> = evidence
        .map_or_else(|| order.deviations(), Evidence::deviations)
        .iter()
        .map(words::deviation)
        .collect();
    let mut parts = vec![section(t("Abweichungen"), &deviations)];
    parts.push(format!("### {}\n\n{}", t("Ziel"), order.goal().trim()));
    parts.push(section(t("Kriterien"), &criteria(order)));
    if let Some(evidence) = evidence {
        let gates: Vec<String> = evidence.gates().iter().map(gate).collect();
        parts.push(section(t("Gates"), &gates));
        let costs: Vec<String> = evidence
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
            .collect();
        parts.push(section(t("Kosten und Laufzeit"), &costs));
        let commit = &evidence.commit()[..evidence.commit().len().min(12)];
        let fingerprint = &evidence.fingerprint()[..evidence.fingerprint().len().min(16)];
        parts.push(tf(
            "{} · {}",
            &[&tf("Commit {}", &[commit]), &tf("Fingerabdruck {}", &[fingerprint])],
        ));
    }
    if let Some(issue) = &order.options().issue {
        parts.push(tf("Aus Issue #{}.", &[&issue.number.to_string()]));
    }
    parts.join("\n\n")
}

/// A heading and a Markdown list of `items`, "none" for none.
fn section(heading: &str, items: &[String]) -> String {
    let list = if items.is_empty() {
        format!("- {}", t("keine"))
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item.replace('\n', " ")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!("### {heading}\n\n{list}")
}

/// Each criterion with the reviewer's verdict from the evidence's last round, where there is one.
fn criteria(order: &Order) -> Vec<String> {
    let round = order.evidence().and_then(|e| e.rounds().last());
    order
        .criteria()
        .iter()
        .map(|criterion| {
            let line = format!("{} {}", criterion.id(), criterion.text());
            let verdict = round.and_then(|r| r.verdicts.iter().find(|v| v.id == criterion.id()));
            match verdict {
                Some(verdict) => {
                    let judged = match verdict.verdict {
                        Judgement::Met => t("erfüllt"),
                        Judgement::NotMet => t("nicht erfüllt"),
                        Judgement::Unclear => t("unklar"),
                    };
                    tf("{}: {} – {}", &[&line, judged, &verdict.reason])
                }
                None => line,
            }
        })
        .collect()
}

fn gate(record: &GateRecord) -> String {
    let name = words::gate_step(record.step, &record.name);
    let result = match (&record.failure, record.skipped) {
        (_, true) => t("übersprungen").to_string(),
        (Some(failure), false) => tf("gescheitert: {}", &[&words::gate_failure(failure)]),
        (None, false) => t("bestanden").to_string(),
    };
    let line = tf("{}: {}", &[&name, &result]);
    match &record.command {
        Some(command) => tf("{} · {}", &[&line, &words::duration(command.duration_ms)]),
        None => line,
    }
}

/// How far publishing `order` got, a line each, for its tab; empty before the user asked.
pub fn lines(order: &Order) -> Vec<String> {
    let Some(publication) = order.publication() else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if publication.pushed() {
        lines.push(t("Branch gepusht").to_string());
    }
    if let Some(change) = publication.change() {
        lines.push(if change.reused() {
            tf("Vorhandener Merge Request übernommen: {}", &[change.url()])
        } else {
            tf("Merge Request angelegt: {}", &[change.url()])
        });
    }
    if publication.comment().is_some() {
        lines.push(t("Issue kommentiert").to_string());
    }
    if let Some(failure) = publication.failure() {
        lines.push(self::failure(failure));
    } else if order.publishing() || order.station() != Station::Done {
        lines.push(t("Wird gesendet …").to_string());
    } else if !order.published() && order.open_operations() == 0 {
        lines.push(t("Unterbrochen; „Als Merge Request“ setzt fort.").to_string());
    }
    lines
}

/// Why nothing more went out.
pub fn failure(failure: &PublishFailure) -> String {
    match failure {
        PublishFailure::ScanUnavailable => {
            t("gitleaks fehlt, darum wurde der ausgehende Text nicht geprüft und nichts gesendet.").into()
        }
        PublishFailure::SecretsFound { count } => tf(
            "Der ausgehende Text enthält {} mögliche Secrets; nichts wurde gesendet.",
            &[&count.to_string()],
        ),
        PublishFailure::NoAcceptedCommit => {
            t("Der Auftrag hat keinen abgenommenen Commit, den der Push senden könnte.").into()
        }
        PublishFailure::Step { step, detail } => match step {
            PublishStep::Push => tf("Pushen gescheitert: {}", &[detail]),
            PublishStep::Change => tf("Merge Request gescheitert: {}", &[detail]),
            PublishStep::Comment => tf("Kommentar am Issue gescheitert: {}", &[detail]),
        },
    }
}

#[cfg(test)]
mod tests {
    use ide_factory::event::BaseRef;
    use ide_factory::options::NewOrder;
    use ide_factory::workflow::rules::Rules;
    use ide_factory::{Factory, Input, InputKind, OrderId, Stage, Stamp};

    use super::*;

    fn factory() -> Factory {
        let mut factory = Factory::new(Stage::One, Rules::default());
        let order = NewOrder {
            title: "CSV export".into(),
            goal: "Export the table as CSV.".into(),
            criteria: "- exports CSV\n".into(),
            ..NewOrder::default()
        };
        let base = BaseRef {
            branch: "main".into(),
            commit: "c0".into(),
        };
        let id = OrderId("o1".into());
        factory.handle(Input::new(
            Stamp::new(1, 1),
            InputKind::NewOrder {
                id,
                order,
                base,
                dirty: false,
            },
        ));
        factory
    }

    #[test]
    fn the_body_puts_the_deviations_first_and_states_goal_and_criteria() {
        let factory = factory();
        let order = factory.order(&OrderId("o1".into())).expect("the order came in");
        let texts = outgoing(order);
        assert_eq!(texts.title(), "CSV export");
        let body = texts.body();
        let deviations = body.find(t("Abweichungen")).expect("deviations");
        let goal = body.find("Export the table as CSV.").expect("goal");
        assert!(deviations < goal, "{body}");
        assert!(body.contains("exports CSV"), "{body}");
        assert!(texts.comment().contains(CHANGE_PLACEHOLDER));
        assert!(lines(order).is_empty(), "nothing to show before the user asked");
    }

    #[test]
    fn each_failure_has_words_of_its_own() {
        let detail = "refused".to_string();
        let words: std::collections::BTreeSet<String> = [
            PublishFailure::ScanUnavailable,
            PublishFailure::SecretsFound { count: 1 },
            PublishFailure::NoAcceptedCommit,
            PublishFailure::Step {
                step: PublishStep::Push,
                detail: detail.clone(),
            },
            PublishFailure::Step {
                step: PublishStep::Change,
                detail: detail.clone(),
            },
            PublishFailure::Step {
                step: PublishStep::Comment,
                detail,
            },
        ]
        .iter()
        .map(failure)
        .collect();
        assert_eq!(words.len(), 6);
    }
}
