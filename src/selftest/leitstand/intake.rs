//! The `leitstand-intake` scenario (E176, E177, E190): a folder whose origin is a stub of GitHub's API and whose workflow
//! file asks for intake from issues labelled `factory` by `octo`. The views show the intake waiting for confirmation;
//! once confirmed, a read takes the one issue by `octo` in as an order and keeps it as taken, a second read takes
//! nothing twice, a read of the order's issue notes its changed text and a read of the text as frozen drops it again,
//! and a rate limit holds every read back. Then publishing's steps reach the stub (`publish`) and the views' host
//! answers as the runtime does (`views`). The workflow's setup command hangs, so no order gets past its worktree.

mod publish;
mod views;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ide_core::settings::Settings;
use ide_factory::OrderId;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::workflow::rules::Loosening;
use ide_session::{End, Script, ScriptTurn, Step};
use ide_ui_factory::Host;

use super::super::ci::stub::{Service, Stub};
use super::super::helpers::wait_within;
use super::super::{check, give_up, give_up_saying};

const SCENARIO: &str = "leitstand-intake";
const PROJECT: &str = "leitstand-intake-project";
const WORKFLOW: &str = "[worktree]\nsetup = \"sleep 600\"\n\n[intake]\nlabel = \"factory\"\nauthors = [\"octo\"]\n\n\
                        [[gates.command]]\nname = \"check\"\nrun = \"true\"\n";
/// The repository's API on the stub.
const REPO: &str = "/api/v3/repos/acme/widget";
/// The one issue of the fixture by an allowed author with the label.
const ISSUE: &str = "https://github.com/acme/widget/issues/28";
const TITLE: &str = "Crash on an empty file";
/// The issue's text as its own page reads now, changed since the list was read.
const CHANGED: &str = "Opening an empty file crashes the parser.\n\n- [ ] an empty file opens\n- [x] a file with one line \
                       opens\n- [ ] the parser reports no error\n";
/// A read of the service through the CI lane, which also asks `gh` for a token, under the load of parallel runs.
const READ: Duration = Duration::from_secs(60);
const SHORT: Duration = Duration::from_secs(20);

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../../crates/ide-ci/tests/fixtures/github/", $name))
    };
}

/// The folder and the stub its origin points at, handed from step to step.
#[derive(Clone)]
struct Flow {
    root: PathBuf,
    stub: Arc<Stub>,
}

fn labelled() -> String {
    format!("{REPO}/issues?state=open&labels=factory&sort=created&direction=asc&per_page=50")
}

fn issue_page() -> String {
    format!("{REPO}/issues/28")
}

fn host() -> crate::leitstand_ui::Orders {
    crate::leitstand_ui::views_host(crate::context::app().leitstand().back())
}

/// The orders of `root` with their title and the page of the issue they came from.
fn orders(root: &Path) -> Vec<(OrderId, String, Option<String>)> {
    let cx = crate::context::app();
    let leitstand = cx.leitstand();
    leitstand
        .order_ids(root)
        .into_iter()
        .filter_map(|id| {
            leitstand.with_order(root, &id, |o| {
                let issue = o.options().issue.as_ref().map(|i| i.url.clone());
                (id.clone(), o.title().to_string(), issue)
            })
        })
        .collect()
}

fn issue_update(root: &Path, id: &OrderId) -> Option<String> {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| o.issue_update().map(str::to_string))
        .flatten()
}

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_script(Some(Script::new(vec![ScriptTurn::new(
        vec![Step::Say("Done.".into())],
        End::Succeed,
    )])));
    cx.leitstand().use_agents(Some(BTreeMap::from([(
        Agent::Claude,
        AgentStatus::ready(&Agent::Claude),
    )])));
    let stub = match Stub::start(Service::GitHub, None) {
        Ok(stub) => stub,
        Err(error) => return give_up_saying(SCENARIO, "the stub of GitHub's API starts", error.to_string()),
    };
    let rules = vec![format!("127.0.0.1:{}=github", stub.port)];
    if let Err(error) = Settings::load_user().set(&crate::ci_panel::CI_HOSTS, &rules) {
        return give_up_saying(SCENARIO, "the stub's host is known as GitHub", error.to_string());
    }
    stub.answer("GET", &labelled(), 200, fixture!("labelled.json"), &[]);
    stub.answer("GET", &issue_page(), 200, fixture!("issue-28.json"), &[]);
    let files = [("README.md", "# Intake\n"), (".dango/factory.toml", WORKFLOW)];
    super::folder::ready(SCENARIO, PROJECT, &files, move |root| {
        let origin = format!(
            "[remote \"origin\"]\n\turl = http://127.0.0.1:{}/acme/widget.git\n",
            stub.port
        );
        if let Err(error) = super::super::git::append(&root.join(".git/config"), &origin) {
            return give_up_saying(SCENARIO, "the folder's origin is the stub", error);
        }
        confirm(Flow { root, stub });
    });
}

/// The views show the intake waiting for the user, who confirms it there.
fn confirm(flow: Flow) {
    let host = host();
    let notices = host.notices(&flow.root);
    let intake = notices
        .loosenings
        .iter()
        .find(|l| matches!(l, Loosening::IssueIntake { .. }))
        .cloned();
    check(
        "the views show the intake from issues waiting for the user's confirmation, and nothing wrong",
        intake.is_some() && notices.errors.is_empty() && notices.problems.is_empty(),
        format!("{:?}", notices.loosenings),
    );
    let Some(intake) = intake else {
        return give_up(SCENARIO, "the workflow file asks for intake from issues");
    };
    host.confirm_loosening(&flow.root, &intake);
    let probe = flow.root.clone();
    wait_within(
        SHORT,
        move || crate::context::app().leitstand().takes_issues(&probe),
        move |takes| {
            if !takes {
                return give_up(SCENARIO, "the confirmed intake applies to the folder");
            }
            first_read(flow);
        },
    );
}

/// A read takes the one issue by an allowed author with the label in as an order and keeps it as taken; the issue of
/// another author and the pull request with the label stay out.
fn first_read(flow: Flow) {
    crate::context::app().leitstand().read_issues_now();
    let probe = flow.root.clone();
    wait_within(
        READ,
        move || !orders(&probe).is_empty(),
        move |came| {
            let seen = orders(&flow.root);
            let taken = crate::context::app().leitstand().issues_taken(&flow.root);
            check(
                "a read of the issues takes the one by an allowed author with the label in as an order, kept as taken",
                came && seen.len() == 1
                    && seen[0].1 == TITLE
                    && seen[0].2.as_deref() == Some(ISSUE)
                    && taken.contains(ISSUE)
                    && taken.len() == 1,
                format!("{seen:?}, taken {taken:?}"),
            );
            let Some((id, _, _)) = seen.into_iter().next() else {
                return give_up(SCENARIO, "the issue becomes an order");
            };
            second_read(flow, id);
        },
    );
}

/// Waits until `flow`'s stub has been asked for the labelled issues more than `before` times and the read is in, then
/// calls `then`.
fn after_read(flow: Flow, before: usize, then: impl FnOnce(Flow, bool) + 'static) {
    let probe = flow.clone();
    wait_within(
        READ,
        move || {
            probe.stub.asked("GET", &labelled()) > before
                && !crate::context::app().leitstand().reading_issues(&probe.root)
        },
        move |read| then(flow, read),
    );
}

/// A second read takes the issue in no second time.
fn second_read(flow: Flow, id: OrderId) {
    let before = flow.stub.asked("GET", &labelled());
    crate::context::app().leitstand().read_issues_now();
    after_read(flow, before, move |flow, read| {
        let seen = orders(&flow.root);
        check(
            "a second read takes the issue in no second time and finds no change of its text",
            read && seen.len() == 1 && issue_update(&flow.root, &id).is_none(),
            format!("{seen:?}"),
        );
        issue_check(flow, id);
    });
}

/// Reading the order's issue again, as its tab does, notes the changed text without changing the order.
fn issue_check(flow: Flow, id: OrderId) {
    host().check_issue(&flow.root, &id);
    let probe = (flow.root.clone(), id.clone());
    wait_within(
        READ,
        move || issue_update(&probe.0, &probe.1).is_some(),
        move |_| {
            let noted = issue_update(&flow.root, &id);
            check(
                "reading the order's issue again notes its changed text for the user to take over",
                noted.as_deref() == Some(CHANGED) && flow.stub.asked("GET", &issue_page()) >= 1,
                format!("{noted:?}"),
            );
            frozen_again(flow, id);
        },
    );
}

/// The list reads the issue as the order froze it again, which drops the noted change.
fn frozen_again(flow: Flow, id: OrderId) {
    let before = flow.stub.asked("GET", &labelled());
    crate::context::app().leitstand().read_issues_now();
    let probe = (flow.root.clone(), id.clone());
    wait_within(
        READ,
        move || issue_update(&probe.0, &probe.1).is_none(),
        move |dropped| {
            check(
                "the issue reading as the order froze it again drops the noted change",
                dropped && flow.stub.asked("GET", &labelled()) > before,
                format!("{:?}", issue_update(&flow.root, &id)),
            );
            rate_limit(flow, id);
        },
    );
}

/// A rate limit holds the folder's reads, of the list and of one issue, until the time the service names.
fn rate_limit(flow: Flow, id: OrderId) {
    let reset = (ide_ci::now() + 3600).to_string();
    flow.stub.answer(
        "GET",
        &labelled(),
        403,
        "{\"message\":\"API rate limit exceeded\"}",
        &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", &reset)],
    );
    let before = flow.stub.asked("GET", &labelled());
    crate::context::app().leitstand().read_issues_now();
    after_read(flow, before, move |flow, read| {
        let held = crate::context::app().leitstand().issues_held(&flow.root);
        let asked = (
            flow.stub.asked("GET", &labelled()),
            flow.stub.asked("GET", &issue_page()),
        );
        crate::context::app().leitstand().read_issues_now();
        host().check_issue(&flow.root, &id);
        let again = (
            flow.stub.asked("GET", &labelled()),
            flow.stub.asked("GET", &issue_page()),
        );
        check(
            "a rate limit holds every read of the folder's issues back until the time the service names",
            read && held
                && asked == again
                && !crate::context::app().leitstand().reading_issues(&flow.root)
                && orders(&flow.root).len() == 1,
            format!("held {held}, asked {asked:?} then {again:?}"),
        );
        publish::run(flow, id);
    });
}
