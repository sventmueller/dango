//! What the app writes on the user's click (E177), answered from recordings: merge requests and pull requests opened
//! and found again by their marker, comments on issues, where a branch points, and tokens without write rights.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

use std::sync::Arc;

use ide_ci::{Client, Error, Method, Recorded, candidates};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const GITHUB: &str = "https://api.github.com/repos/acme/widget";
const GITLAB: &str = "https://gitlab.example.com/gitlab/api/v4/projects/group%2Fapp";

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .replace("{{SHA}}", SHA)
}

fn github(recorded: Recorded) -> (Client, Arc<Recorded>) {
    let recorded = Arc::new(recorded);
    let forge = candidates("git@github.com:acme/widget.git", &[]).remove(0);
    (Client::new(forge, Some("gho_x".into()), recorded.clone()), recorded)
}

fn gitlab(recorded: Recorded) -> (Client, Arc<Recorded>) {
    let recorded = Arc::new(recorded);
    let forge = candidates("https://gitlab.example.com/gitlab/group/app.git", &[])
        .into_iter()
        .find(|f| f.project == "group/app")
        .unwrap();
    (Client::new(forge, Some("glpat-x".into()), recorded.clone()), recorded)
}

/// The JSON bodies of the POSTs so far, by URL.
fn posted(recorded: &Recorded) -> Vec<(String, serde_json::Value)> {
    recorded
        .asked()
        .into_iter()
        .filter(|r| r.method == Method::Post)
        .map(|r| (r.url, serde_json::from_slice(&r.body).unwrap()))
        .collect()
}

#[test]
fn github_opens_a_pull_request_finds_it_by_its_marker_and_comments_the_issue() {
    let (client, recorded) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/pulls?state=all&head=acme%3Aorder%2Fo-1&per_page=100"),
                200,
                &fixture("github/pulls-of-branch.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITHUB}/pulls"),
                201,
                &fixture("github/pull-created.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITHUB}/issues/28/comments"),
                201,
                &fixture("github/comment-created.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/issues/28/comments?per_page=100"),
                200,
                &fixture("github/comments.json"),
                &[],
            ),
    );
    let marker = "<!-- dango-order: o-1 -->";
    let found = client.find_change("order/o-1", marker).unwrap().unwrap();
    assert_eq!((found.number, found.branch.as_deref()), (32, Some("order/o-1")));
    assert_eq!(
        client.find_change("order/o-1", "<!-- dango-order: o-9 -->").unwrap(),
        None
    );
    let opened = client
        .open_change(
            "order/o-1",
            "main",
            "Parse nested tables",
            &format!("Summary\n\n{marker}"),
        )
        .unwrap();
    assert_eq!(opened.url, "https://github.com/acme/widget/pull/32");
    assert_eq!(opened.author.as_deref(), Some("octo"));
    let page = client.comment_issue(28, &format!("Done in #32\n\n{marker}")).unwrap();
    assert_eq!(page, "https://github.com/acme/widget/issues/28#issuecomment-555");
    assert_eq!(client.find_comment(28, marker).unwrap().as_deref(), Some(page.as_str()));
    assert_eq!(client.find_comment(28, "<!-- dango-order: o-9 -->").unwrap(), None);
    let posts = posted(&recorded);
    assert_eq!(posts[0].0, format!("{GITHUB}/pulls"));
    assert_eq!(posts[0].1["head"], "order/o-1");
    assert_eq!(posts[0].1["base"], "main");
    assert_eq!(posts[0].1["title"], "Parse nested tables");
    assert!(posts[0].1["body"].as_str().unwrap().ends_with(marker));
    assert_eq!(posts[1].1["body"], format!("Done in #32\n\n{marker}"));
}

#[test]
fn gitlab_opens_a_merge_request_finds_it_by_its_marker_and_notes_the_issue() {
    let (client, recorded) = gitlab(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITLAB}/merge_requests?state=all&source_branch=order%2Fo-2&per_page=100"),
                200,
                &fixture("gitlab/merge-requests-of-branch.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITLAB}/merge_requests"),
                201,
                &fixture("gitlab/merge-request-created.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITLAB}/issues/7/notes"),
                201,
                &fixture("gitlab/note-created.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/issues/7/notes?sort=desc&per_page=100"),
                200,
                &fixture("gitlab/notes.json"),
                &[],
            ),
    );
    let marker = "<!-- dango-order: o-2 -->";
    let found = client.find_change("order/o-2", marker).unwrap().unwrap();
    assert_eq!((found.number, found.author.as_deref()), (15, Some("lab-selftest")));
    let opened = client
        .open_change(
            "order/o-2",
            "main",
            "Add the export job",
            &format!("Summary\n\n{marker}"),
        )
        .unwrap();
    assert_eq!(
        (opened.number, opened.url.as_str(), opened.branch.as_deref()),
        (
            15,
            "https://gitlab.example.com/gitlab/group/app/-/merge_requests/15",
            Some("order/o-2")
        )
    );
    let page = client.comment_issue(7, &format!("Done in !15\n\n{marker}")).unwrap();
    assert_eq!(page, "https://gitlab.example.com/gitlab/group/app/-/issues/7#note_777");
    assert_eq!(client.find_comment(7, marker).unwrap().as_deref(), Some(page.as_str()));
    let posts = posted(&recorded);
    assert_eq!(posts[0].1["source_branch"], "order/o-2");
    assert_eq!(posts[0].1["target_branch"], "main");
    assert!(posts[0].1["description"].as_str().unwrap().ends_with(marker));
    assert_eq!(posts[1].0, format!("{GITLAB}/issues/7/notes"));
}

#[test]
fn a_branch_head_is_read_from_the_service_and_a_missing_branch_is_none() {
    let (client, _) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/git/ref/heads/order/o-1"),
                200,
                &fixture("github/ref.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/git/ref/heads/order/o-9"),
                404,
                r#"{"message":"Not Found"}"#,
                &[],
            ),
    );
    assert_eq!(client.branch_head("order/o-1").unwrap().as_deref(), Some(SHA));
    assert_eq!(client.branch_head("order/o-9").unwrap(), None);
    let (client, _) = gitlab(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITLAB}/repository/branches/order%2Fo-2"),
                200,
                &fixture("gitlab/branch.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/repository/branches/order%2Fo-9"),
                404,
                r#"{"message":"404 Branch Not Found"}"#,
                &[],
            ),
    );
    assert_eq!(client.branch_head("order/o-2").unwrap().as_deref(), Some(SHA));
    assert_eq!(client.branch_head("order/o-9").unwrap(), None);
}

#[test]
fn a_token_without_write_rights_says_which_it_lacks() {
    let (client, _) = github(Recorded::new().answer(
        Method::Post,
        &format!("{GITHUB}/pulls"),
        403,
        r#"{"message":"Resource not accessible by personal access token"}"#,
        &[("X-Accepted-GitHub-Permissions", "pull_requests=write")],
    ));
    assert_eq!(
        client.open_change("order/o-1", "main", "t", "b"),
        Err(Error::MissingScope {
            host: "github.com".into(),
            scope: "pull_requests=write".into()
        })
    );
    let (client, _) = gitlab(Recorded::new().answer(
        Method::Post,
        &format!("{GITLAB}/issues/7/notes"),
        403,
        r#"{"error":"insufficient_scope","error_description":"The request requires higher privileges than provided by the access token.","scope":"api"}"#,
        &[],
    ));
    assert_eq!(
        client.comment_issue(7, "b"),
        Err(Error::MissingScope {
            host: "gitlab.example.com".into(),
            scope: "api".into()
        })
    );
}
