//! Publishing's service calls answered from recordings (E177): a merge request found again by its marker or opened, a
//! comment found again or made, the checks a restart makes (C66), the secrets scan as the factory takes it, and the
//! failures in the CI panel's words.

use std::sync::Arc;

use ide_ci::{Client, Method, Recorded, candidates};
use ide_factory::event::Action;

use super::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const GITHUB: &str = "https://api.github.com/repos/acme/widget";
const MARKER: &str = "<!-- dango-order: o-1 -->";
const OTHER: &str = "<!-- dango-order: o-9 -->";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../crates/ide-ci/tests/fixtures/github/", $name))
    };
}

fn github(recorded: Recorded) -> (Client, Arc<Recorded>) {
    let recorded = Arc::new(recorded);
    let forge = candidates("git@github.com:acme/widget.git", &[]).remove(0);
    (Client::new(forge, Some("gho_x".into()), recorded.clone()), recorded)
}

/// A service whose pull requests of `order/o-1` and comments of issue 28 are those of the fixtures, and which opens a
/// pull request and takes a comment.
fn service() -> (Client, Arc<Recorded>) {
    github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/pulls?state=all&head=acme%3Aorder%2Fo-1&per_page=100"),
                200,
                fixture!("pulls-of-branch.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITHUB}/pulls"),
                201,
                fixture!("pull-created.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/issues/28/comments?per_page=100"),
                200,
                fixture!("comments.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITHUB}/issues/28/comments"),
                201,
                fixture!("comment-created.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/git/ref/heads/order/o-1"),
                200,
                &fixture!("ref.json").replace("{{SHA}}", SHA),
                &[],
            ),
    )
}

/// A service that refuses every call for want of a token it takes.
fn refusing() -> (Client, Arc<Recorded>) {
    let unauthorized = r#"{"message":"Bad credentials"}"#;
    github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/pulls?state=all&head=acme%3Aorder%2Fo-1&per_page=100"),
                401,
                unauthorized,
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/issues/28/comments?per_page=100"),
                401,
                unauthorized,
                &[],
            ),
    )
}

fn posts(recorded: &Recorded) -> Vec<String> {
    recorded
        .asked()
        .into_iter()
        .filter(|r| r.method == Method::Post)
        .map(|r| r.url)
        .collect()
}

#[test]
fn only_the_steps_that_reach_the_service_are_publishings() {
    let order = OrderId("o-1".into());
    let op = OperationId("op-1".into());
    let verify = |target| Effect::Verify {
        order: order.clone(),
        op: op.clone(),
        action: Action::Push {
            branch: "order/o-1".into(),
            commit: SHA.into(),
        },
        target,
    };
    assert!(handles(&Effect::ScanOutgoing {
        order: order.clone(),
        run_id: RunId(1),
        text: "out".into(),
    }));
    assert!(handles(&Effect::Push {
        order: order.clone(),
        op: op.clone(),
        branch: "order/o-1".into(),
        commit: SHA.into(),
    }));
    assert!(handles(&Effect::CommentIssue {
        order: order.clone(),
        op: op.clone(),
        issue: 28,
        body: "Done".into(),
        marker: MARKER.into(),
    }));
    assert!(handles(&verify(TargetState::RemoteBranchAt {
        branch: "order/o-1".into(),
        commit: SHA.into(),
    })));
    assert!(!handles(&verify(TargetState::WorktreeGone { path: "/w".into() })));
    assert!(!handles(&Effect::StopGate { order }));
}

#[test]
fn a_merge_request_with_the_orders_marker_is_taken_and_otherwise_opened() {
    let (client, recorded) = service();
    assert_eq!(
        open_change(&client, "order/o-1", "main", "Parse nested tables", "Summary", MARKER),
        OpResult::ChangeOpened {
            change: ChangeRef::new(32, "https://github.com/acme/widget/pull/32", true),
        }
    );
    assert!(posts(&recorded).is_empty());
    assert_eq!(
        open_change(&client, "order/o-1", "main", "Parse nested tables", "Summary", OTHER),
        OpResult::ChangeOpened {
            change: ChangeRef::new(32, "https://github.com/acme/widget/pull/32", false),
        }
    );
    assert_eq!(posts(&recorded), [format!("{GITHUB}/pulls")]);
}

#[test]
fn the_issue_is_commented_once_by_the_marker() {
    let (client, recorded) = service();
    let page = "https://github.com/acme/widget/issues/28#issuecomment-555";
    assert_eq!(
        comment(&client, 28, "Done in #32", MARKER),
        OpResult::Commented { url: page.into() }
    );
    assert!(posts(&recorded).is_empty());
    assert_eq!(
        comment(&client, 28, "Done in #32", OTHER),
        OpResult::Commented { url: page.into() }
    );
    assert_eq!(posts(&recorded), [format!("{GITHUB}/issues/28/comments")]);
}

#[test]
fn a_refusal_of_the_service_fails_the_step_in_the_ci_panels_words() {
    let (client, recorded) = refusing();
    let words = crate::ci_panel::problem(&ide_ci::Error::Unauthorized {
        host: "github.com".into(),
    });
    let refused = OpResult::Failed { detail: words };
    assert_eq!(open_change(&client, "order/o-1", "main", "t", "b", MARKER), refused);
    assert_eq!(comment(&client, 28, "b", MARKER), refused);
    assert!(posts(&recorded).is_empty());
    let (client, _) = github(Recorded::new().answer(
        Method::Get,
        &format!("{GITHUB}/pulls?state=all&head=acme%3Aorder%2Fo-1&per_page=100"),
        200,
        "[]",
        &[],
    ));
    assert!(matches!(
        open_change(&client, "order/o-1", "main", "t", "b", MARKER),
        OpResult::Failed { .. }
    ));
}

#[test]
fn after_a_restart_the_service_tells_which_steps_happened() {
    let (client, _) = service();
    let at = |commit: &str| TargetState::RemoteBranchAt {
        branch: "order/o-1".into(),
        commit: commit.into(),
    };
    assert_eq!(reached(&client, &at(SHA)), Ok(Some(OpResult::Pushed)));
    assert_eq!(reached(&client, &at("feedface")), Ok(None));
    assert_eq!(
        reached(
            &client,
            &TargetState::ChangeFor {
                branch: "order/o-1".into(),
                marker: MARKER.into(),
            }
        ),
        Ok(Some(OpResult::ChangeOpened {
            change: ChangeRef::new(32, "https://github.com/acme/widget/pull/32", true),
        }))
    );
    assert_eq!(
        reached(
            &client,
            &TargetState::IssueComment {
                issue: 28,
                marker: OTHER.into(),
            }
        ),
        Ok(None)
    );
    assert_eq!(
        reached(&client, &TargetState::WorktreeGone { path: "/w".into() }),
        Ok(None)
    );
    let (refused, _) = refusing();
    assert!(
        reached(
            &refused,
            &TargetState::ChangeFor {
                branch: "order/o-1".into(),
                marker: MARKER.into(),
            }
        )
        .is_err()
    );
}

#[test]
fn a_scan_that_could_not_run_counts_as_gitleaks_missing() {
    let finding = |line| ide_git::Finding {
        rule: "github-pat".into(),
        description: "GitHub token".into(),
        file: "out.md".into(),
        line,
        end_line: line,
        commit: None,
    };
    assert_eq!(scanned(Ok(ide_git::SecretScan::Clean)), SecretScan::Clean);
    assert_eq!(
        scanned(Ok(ide_git::SecretScan::Found(vec![finding(3), finding(7)]))),
        SecretScan::Found {
            count: 2,
            places: vec!["3".into(), "7".into()],
        }
    );
    assert_eq!(scanned(Ok(ide_git::SecretScan::NotInstalled)), SecretScan::NotInstalled);
    assert_eq!(scanned(Err(ide_git::Error::GitMissing)), SecretScan::NotInstalled);
}

#[test]
fn nothing_is_pushed_from_a_folder_that_is_not_trusted() {
    let folder = ide_core::testing::scratch("leitstand-publish-untrusted");
    assert_eq!(pushed(&folder, "order/o-1", SHA), untrusted());
    assert!(matches!(untrusted(), OpResult::Failed { detail } if !detail.is_empty()));
    assert!(matches!(not_run(), OpResult::Failed { detail } if !detail.is_empty()));
    assert_ne!(untrusted(), not_run());
}
