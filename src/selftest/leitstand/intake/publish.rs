//! Publishing's steps for the order taken in from the issue, asked of the folder's service as its factory would ask
//! for them (E177, C66): a merge request with the order's marker is taken as it is and another opened once, the issue
//! gets its comment once, a restart's checks read the branch's head, the merge request and the comment, and the push
//! goes to the folder's remote as the user's own.

use ide_factory::event::{Action, TargetState};
use ide_factory::{Effect, OperationId, OrderId};

use super::super::super::check;
use super::super::super::ci::stub::Stub;
use super::super::super::helpers::wait_within;
use super::{Flow, READ, REPO};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../../../crates/ide-ci/tests/fixtures/github/", $name))
    };
}

const BRANCH: &str = "order/o-1";
const MARKER: &str = "<!-- dango-order: o-1 -->";
const OTHER: &str = "<!-- dango-order: o-9 -->";
const TITLE: &str = "Parse nested tables";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn pulls_of_branch() -> String {
    format!("{REPO}/pulls?state=all&head=acme%3Aorder%2Fo-1&per_page=100")
}

fn comments() -> String {
    format!("{REPO}/issues/28/comments")
}

fn listed_comments() -> String {
    format!("{REPO}/issues/28/comments?per_page=100")
}

fn branch_ref() -> String {
    format!("{REPO}/git/ref/heads/{BRANCH}")
}

fn answer(stub: &Stub) {
    stub.answer("GET", &pulls_of_branch(), 200, fixture!("pulls-of-branch.json"), &[]);
    stub.answer(
        "POST",
        &format!("{REPO}/pulls"),
        201,
        fixture!("pull-created.json"),
        &[],
    );
    stub.answer("GET", &listed_comments(), 200, fixture!("comments.json"), &[]);
    stub.answer("POST", &comments(), 201, fixture!("comment-created.json"), &[]);
    let head = format!("{{\"ref\":\"refs/heads/{BRANCH}\",\"object\":{{\"sha\":\"{SHA}\",\"type\":\"commit\"}}}}");
    stub.answer("GET", &branch_ref(), 200, &head, &[]);
}

/// Publishing's effects for `order`, each with an operation its factory does not wait for, so the order goes on as it
/// was.
fn effects(order: &OrderId) -> Vec<Effect> {
    let op = |name: &str| OperationId(format!("selftest-{name}"));
    let verify = |name: &str, action, target| Effect::Verify {
        order: order.clone(),
        op: op(name),
        action,
        target,
    };
    let open = |name: &str, marker: &str| Effect::OpenChange {
        order: order.clone(),
        op: op(name),
        branch: BRANCH.into(),
        base: "main".into(),
        title: TITLE.into(),
        body: format!("Summary\n\n{marker}"),
        marker: marker.into(),
    };
    vec![
        open("taken", MARKER),
        open("opened", OTHER),
        Effect::CommentIssue {
            order: order.clone(),
            op: op("comment"),
            issue: 28,
            body: format!("Done\n\n{OTHER}"),
            marker: OTHER.into(),
        },
        verify(
            "pushed",
            Action::Push {
                branch: BRANCH.into(),
                commit: SHA.into(),
            },
            TargetState::RemoteBranchAt {
                branch: BRANCH.into(),
                commit: SHA.into(),
            },
        ),
        verify(
            "change",
            Action::OpenChange {
                branch: BRANCH.into(),
                base: "main".into(),
                marker: MARKER.into(),
            },
            TargetState::ChangeFor {
                branch: BRANCH.into(),
                marker: MARKER.into(),
            },
        ),
        verify(
            "commented",
            Action::CommentIssue {
                issue: 28,
                marker: OTHER.into(),
            },
            TargetState::IssueComment {
                issue: 28,
                marker: OTHER.into(),
            },
        ),
        Effect::Push {
            order: order.clone(),
            op: op("push"),
            branch: BRANCH.into(),
            commit: "HEAD".into(),
        },
    ]
}

/// Whether the push reached the stub, which speaks no git and refuses it.
fn push_tried(stub: &Stub) -> bool {
    stub.seen()
        .iter()
        .any(|s| s.target.starts_with("/acme/widget.git/info/refs"))
}

fn all_asked(stub: &Stub) -> bool {
    stub.asked("GET", &pulls_of_branch()) >= 3
        && stub.asked("POST", &format!("{REPO}/pulls")) >= 1
        && stub.asked("GET", &listed_comments()) >= 2
        && stub.asked("POST", &comments()) >= 1
        && stub.asked("GET", &branch_ref()) >= 1
        && push_tried(stub)
}

pub(super) fn run(flow: Flow, id: OrderId) {
    answer(&flow.stub);
    let cx = crate::context::app();
    for effect in effects(&id) {
        cx.leitstand().publish_now(&flow.root, effect);
    }
    let probe = flow.stub.clone();
    wait_within(
        READ,
        move || all_asked(&probe),
        move |asked| {
            checks(&flow.stub, asked);
            super::views::run(flow, id);
        },
    );
}

fn checks(stub: &Stub, asked: bool) {
    let posted = |target: String| -> Vec<String> {
        stub.seen()
            .into_iter()
            .filter(|s| s.method == "POST" && s.target == target)
            .map(|s| s.body)
            .collect()
    };
    let pulls = posted(format!("{REPO}/pulls"));
    check(
        "a merge request with the order's marker is taken as it is; another is opened once, with its title and branch",
        asked
            && pulls.len() == 1
            && pulls[0].contains(&format!("\"title\":\"{TITLE}\""))
            && pulls[0].contains(&format!("\"head\":\"{BRANCH}\"")),
        format!("{pulls:?}"),
    );
    let comments = posted(comments());
    check(
        "the issue gets the comment once, after a look for its marker",
        asked && comments.len() == 1 && comments[0].contains("Done"),
        format!("{comments:?}"),
    );
    check(
        "after a restart, the service tells whether the branch, the merge request and the comment are there",
        asked && stub.asked("GET", &branch_ref()) >= 1,
        format!("{} reads of the branch", stub.asked("GET", &branch_ref())),
    );
    check(
        "the push goes to the folder's remote on the service, as the user's own push",
        push_tried(stub),
        stub.seen()
            .iter()
            .map(|s| format!("{} {}", s.method, s.target))
            .collect::<Vec<_>>()
            .join(", "),
    );
}
