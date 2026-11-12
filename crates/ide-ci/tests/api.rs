//! Both services' APIs answered from recordings: runs, jobs with paging, steps, logs, re-running, cancelling, finding a
//! self-managed GitLab under a path, rate limits and tokens.

use std::sync::Arc;

use ide_ci::log::{self, LineKind};
use ide_ci::{Client, Error, Kind, Method, Recorded, State, candidates, locate};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const GITHUB: &str = "https://api.github.com/repos/acme/widget";
const GITLAB: &str = "https://gitlab.example.com/gitlab/api/v4/projects/group%2Fapp";

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .replace("{{SHA}}", SHA)
        .replace("{{BRANCH}}", "main")
}

fn github(recorded: Recorded, token: Option<&str>) -> (Client, Arc<Recorded>) {
    let recorded = Arc::new(recorded);
    let forge = candidates("git@github.com:acme/widget.git", &[]).remove(0);
    (
        Client::new(forge, token.map(str::to_string), recorded.clone()),
        recorded,
    )
}

fn gitlab(recorded: Recorded) -> (Client, Arc<Recorded>) {
    let recorded = Arc::new(recorded);
    let forge = candidates("https://gitlab.example.com/gitlab/group/app.git", &[])
        .into_iter()
        .find(|f| f.project == "group/app")
        .unwrap();
    (Client::new(forge, Some("glpat-x".into()), recorded.clone()), recorded)
}

#[test]
fn github_runs_jobs_over_two_pages_steps_and_the_log_of_a_failed_job() {
    let page2 = format!("{GITHUB}/actions/runs/9001/jobs?filter=latest&per_page=100&page=2");
    let link = format!("<{page2}>; rel=\"next\"");
    let (client, recorded) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/runs?head_sha={SHA}&per_page=30"),
                200,
                &fixture("github/runs.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/runs/9001/jobs?filter=latest&per_page=100"),
                200,
                &fixture("github/jobs-9001-page1.json"),
                &[("Link", &link)],
            )
            .answer(Method::Get, &page2, 200, &fixture("github/jobs-9001-page2.json"), &[])
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/jobs/72/logs"),
                200,
                &fixture("github/log-72.txt"),
                &[],
            ),
        Some("gho_secret"),
    );
    let runs = client.runs(Some("main"), SHA).unwrap();
    let summary: Vec<_> = runs.iter().map(|r| (r.name.as_str(), r.number, r.state)).collect();
    assert_eq!(summary, [("CI", 42, State::Failed), ("Lint", 17, State::Running)]);
    assert_eq!(runs[0].created, ide_ci::seconds("2026-11-09T12:00:00Z"));
    let jobs = client.jobs(&runs[0]).unwrap();
    assert_eq!(
        jobs.iter().map(|j| (j.name.as_str(), j.state)).collect::<Vec<_>>(),
        [("build", State::Success), ("test", State::Failed)]
    );
    let steps: Vec<_> = jobs[1].steps.iter().map(|s| (s.number, s.state)).collect();
    assert_eq!(steps, [(1, State::Success), (2, State::Failed), (3, State::Skipped)]);
    assert_eq!(jobs[1].seconds(0), Some(130));
    let lines = log::lines(&client.log(&jobs[1]).unwrap());
    assert_eq!(lines[0].kind, LineKind::Section);
    assert_eq!(lines.last().map(|l| l.kind), Some(LineKind::Error));
    assert!(log::text(&lines).contains("cargo test --workspace\nrunning 3 tests"));
    assert!(log::excerpt(&lines, 8).contains("panicked at src/parser.rs:42:9"));
    let asked = recorded.asked();
    assert!(asked.iter().all(|r| {
        r.headers
            .contains(&("Authorization".into(), "Bearer gho_secret".into()))
    }));
    assert!(asked.iter().all(|r| {
        r.headers
            .iter()
            .any(|(n, v)| n == "X-GitHub-Api-Version" && v == "2022-11-28")
    }));
}

#[test]
fn a_commit_without_runs_shows_its_branchs_latest_and_no_token_sends_none() {
    let (client, recorded) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/runs?head_sha={SHA}&per_page=30"),
                200,
                &fixture("github/runs-none.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/runs?branch=feature%2Fx&per_page=10"),
                200,
                &fixture("github/runs.json"),
                &[],
            ),
        None,
    );
    assert_eq!(client.runs(Some("feature/x"), SHA).unwrap().len(), 2);
    assert!(
        recorded
            .asked()
            .iter()
            .all(|r| r.headers.iter().all(|(n, _)| n != "Authorization"))
    );
}

#[test]
fn github_reruns_failed_jobs_and_cancels_and_says_when_its_limit_is_spent() {
    let (client, recorded) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/runs?head_sha={SHA}&per_page=30"),
                200,
                &fixture("github/runs.json"),
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITHUB}/actions/runs/9001/rerun-failed-jobs"),
                201,
                "",
                &[],
            )
            .answer(
                Method::Post,
                &format!("{GITHUB}/actions/runs/9002/cancel"),
                202,
                "{}",
                &[],
            )
            .answer(
                Method::Get,
                "https://api.github.com/user",
                200,
                &fixture("github/user.json"),
                &[],
            ),
        Some("t"),
    );
    let runs = client.runs(None, SHA).unwrap();
    client.rerun_failed(&runs[0]).unwrap();
    client.cancel(&runs[1]).unwrap();
    assert_eq!(client.account().unwrap(), "octo-selftest");
    let posts: Vec<String> = recorded
        .asked()
        .into_iter()
        .filter(|r| r.method == Method::Post)
        .map(|r| r.url)
        .collect();
    assert_eq!(posts.len(), 2);
    let (limited, _) = github(
        Recorded::new().answer(
            Method::Get,
            &format!("{GITHUB}/actions/runs?head_sha={SHA}&per_page=30"),
            403,
            "{\"message\":\"API rate limit exceeded\"}",
            &[("X-RateLimit-Remaining", "0"), ("X-RateLimit-Reset", "1794229200")],
        ),
        None,
    );
    assert_eq!(
        limited.runs(None, SHA),
        Err(Error::RateLimited {
            until: Some(1_794_229_200)
        })
    );
}

#[test]
fn gitlab_pipelines_jobs_over_next_page_headers_trace_retry_and_cancel() {
    let (client, recorded) = gitlab(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITLAB}/pipelines?sha={SHA}&per_page=20"),
                200,
                &fixture("gitlab/pipelines.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/pipelines/501/jobs?per_page=100"),
                200,
                &fixture("gitlab/jobs-page1.json"),
                &[("X-Next-Page", "2")],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/pipelines/501/jobs?per_page=100&page=2"),
                200,
                &fixture("gitlab/jobs-page2.json"),
                &[("X-Next-Page", "")],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/jobs/3002/trace"),
                200,
                &fixture("gitlab/trace-3002.txt"),
                &[],
            )
            .answer(Method::Post, &format!("{GITLAB}/pipelines/501/retry"), 201, "{}", &[])
            .answer(Method::Post, &format!("{GITLAB}/pipelines/501/cancel"), 200, "{}", &[]),
    );
    let runs = client.runs(Some("main"), SHA).unwrap();
    assert_eq!(
        (runs[0].id, runs[0].state, runs[0].name.as_str()),
        (501, State::Running, "Pipeline")
    );
    let jobs = client.jobs(&runs[0]).unwrap();
    let summary: Vec<_> = jobs
        .iter()
        .map(|j| (j.name.as_str(), j.stage.as_deref(), j.state))
        .collect();
    assert_eq!(
        summary,
        [
            ("build", Some("build"), State::Success),
            ("test", Some("test"), State::Failed),
            ("lint", Some("test"), State::Running),
            ("deploy", Some("deploy"), State::Manual),
        ]
    );
    let lines = log::lines(&client.log(&jobs[1]).unwrap());
    let sections: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == LineKind::Section)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(
        sections,
        [
            "Preparing the \"docker\" executor",
            "Executing \"step_script\" stage of the job script"
        ]
    );
    assert!(log::text(&lines).contains("Downloaded 120 crates"));
    assert!(!log::text(&lines).contains('\u{1b}'));
    client.rerun_failed(&runs[0]).unwrap();
    client.cancel(&runs[0]).unwrap();
    assert!(
        recorded
            .asked()
            .iter()
            .all(|r| r.headers.contains(&("Authorization".into(), "Bearer glpat-x".into())))
    );
}

#[test]
fn a_self_managed_gitlab_under_a_path_is_found_by_asking_for_the_project() {
    let forges = candidates("https://gitlab.example.com/gitlab/group/app.git", &[]);
    assert_eq!(forges.len(), 2);
    let recorded = Arc::new(
        Recorded::new()
            .answer(
                Method::Get,
                "https://gitlab.example.com/api/v4/projects/gitlab%2Fgroup%2Fapp",
                404,
                "{\"message\":\"404 Project Not Found\"}",
                &[],
            )
            .answer(
                Method::Get,
                "https://gitlab.example.com/gitlab/api/v4/projects/group%2Fapp",
                200,
                &fixture("gitlab/project.json"),
                &[],
            ),
    );
    let found = locate(&forges, Some("t"), recorded.clone()).unwrap();
    assert_eq!(
        (found.kind, found.api.as_str(), found.project.as_str()),
        (Kind::GitLab, "https://gitlab.example.com/gitlab/api/v4", "group/app")
    );
    let refused = Arc::new(Recorded::new().answer(
        Method::Get,
        "https://gitlab.example.com/api/v4/projects/gitlab%2Fgroup%2Fapp",
        401,
        "{\"message\":\"401 Unauthorized\"}",
        &[],
    ));
    assert_eq!(
        locate(&forges, None, refused),
        Err(Error::Unauthorized {
            host: "gitlab.example.com".into()
        })
    );
}

fn posted(recorded: &Recorded) -> Vec<(String, String)> {
    recorded
        .asked()
        .into_iter()
        .filter(|r| r.method == Method::Post)
        .map(|r| (r.url, String::from_utf8_lossy(&r.body).into_owned()))
        .collect()
}

fn job(id: u64, run: u64) -> ide_ci::Job {
    ide_ci::Job {
        id,
        run,
        name: "deploy".into(),
        stage: None,
        state: State::Manual,
        url: String::new(),
        started: None,
        finished: None,
        steps: Vec::new(),
    }
}

#[test]
fn github_reruns_a_job_or_a_whole_run_starts_a_workflow_and_approves_only_its_own_deployments() {
    let pending = format!("{GITHUB}/actions/runs/9001/pending_deployments");
    let (client, recorded) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/actions/runs?head_sha={SHA}&per_page=30"),
                200,
                &fixture("github/runs.json"),
                &[],
            )
            .answer(Method::Post, &format!("{GITHUB}/actions/jobs/72/rerun"), 201, "", &[])
            .answer(Method::Post, &format!("{GITHUB}/actions/runs/9001/rerun"), 201, "", &[])
            .answer(
                Method::Post,
                &format!("{GITHUB}/actions/workflows/release.yml/dispatches"),
                204,
                "",
                &[],
            )
            .answer(Method::Get, &pending, 200, &fixture("github/pending.json"), &[])
            .answer(Method::Post, &pending, 200, "[]", &[]),
        Some("t"),
    );
    let runs = client.runs(None, SHA).unwrap();
    client.retry_job(&job(72, 9001)).unwrap();
    client.rerun_all(&runs[0]).unwrap();
    client.start_run("feature/x", Some("release.yml")).unwrap();
    client.start_job(&job(80, 9001)).unwrap();
    let posts = posted(&recorded);
    assert_eq!(
        posts
            .iter()
            .map(|(url, _)| url.trim_start_matches(GITHUB))
            .collect::<Vec<_>>(),
        [
            "/actions/jobs/72/rerun",
            "/actions/runs/9001/rerun",
            "/actions/workflows/release.yml/dispatches",
            "/actions/runs/9001/pending_deployments",
        ]
    );
    let dispatch: serde_json::Value = serde_json::from_str(&posts[2].1).unwrap();
    assert_eq!(dispatch, serde_json::json!({ "ref": "feature/x" }));
    let review: serde_json::Value = serde_json::from_str(&posts[3].1).unwrap();
    assert_eq!(review["environment_ids"], serde_json::json!([161_171_787]));
    assert_eq!(review["state"], "approved");
    assert!(
        recorded
            .asked()
            .iter()
            .filter(|r| !r.body.is_empty())
            .all(|r| { r.headers.contains(&("Content-Type".into(), "application/json".into())) })
    );
    assert_eq!(
        client.start_run("main", None),
        Err(Error::NotFound {
            what: "workflow".into()
        })
    );
}

#[test]
fn github_lists_open_pull_requests_and_issues_without_the_pull_requests_among_them() {
    let (client, _) = github(
        Recorded::new()
            .answer(
                Method::Get,
                &format!("{GITHUB}/pulls?state=open&sort=updated&direction=desc&per_page=50"),
                200,
                &fixture("github/pulls.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITHUB}/issues?state=open&sort=updated&direction=desc&per_page=50"),
                200,
                &fixture("github/issues.json"),
                &[],
            ),
        Some("t"),
    );
    let pulls = client.changes().unwrap();
    let seen: Vec<_> = pulls
        .iter()
        .map(|p| (p.number, p.draft, p.branch.as_deref(), p.author.as_deref()))
        .collect();
    assert_eq!(
        seen,
        [
            (31, false, Some("feature/tables"), Some("octo")),
            (30, true, Some("lexer"), Some("cat"))
        ]
    );
    assert_eq!(pulls[0].updated, ide_ci::seconds("2026-11-09T11:00:00Z"));
    let issues = client.issues().unwrap();
    assert_eq!(
        issues
            .iter()
            .map(|i| (i.number, i.branch.is_none()))
            .collect::<Vec<_>>(),
        [(28, true)]
    );
    assert_eq!(client.new_issue_page(), "https://github.com/acme/widget/issues/new");
    assert_eq!(
        client.new_change_page("feature/x"),
        "https://github.com/acme/widget/compare/feature%2Fx?expand=1"
    );
}

#[test]
fn gitlab_retries_and_plays_a_job_starts_a_pipeline_and_lists_merge_requests_and_issues() {
    let (client, recorded) = gitlab(
        Recorded::new()
            .answer(Method::Post, &format!("{GITLAB}/jobs/3002/retry"), 201, "{}", &[])
            .answer(Method::Post, &format!("{GITLAB}/jobs/3004/play"), 200, "{}", &[])
            .answer(
                Method::Post,
                &format!("{GITLAB}/pipeline?ref=feature%2Fx"),
                201,
                "{}",
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50"),
                200,
                &fixture("gitlab/merge_requests.json"),
                &[],
            )
            .answer(
                Method::Get,
                &format!("{GITLAB}/issues?state=opened&order_by=updated_at&sort=desc&per_page=50"),
                200,
                &fixture("gitlab/issues.json"),
                &[],
            ),
    );
    client.retry_job(&job(3002, 501)).unwrap();
    client.start_job(&job(3004, 501)).unwrap();
    client.start_run("feature/x", None).unwrap();
    assert_eq!(posted(&recorded).len(), 3);
    let requests = client.changes().unwrap();
    let seen: Vec<_> = requests
        .iter()
        .map(|r| (r.number, r.draft, r.branch.as_deref()))
        .collect();
    assert_eq!(seen, [(14, false, Some("export")), (13, true, Some("runner"))]);
    let issues = client.issues().unwrap();
    assert_eq!(
        issues
            .iter()
            .map(|i| (i.number, i.author.as_deref()))
            .collect::<Vec<_>>(),
        [(7, Some("ops"))]
    );
    assert_eq!(
        client.new_change_page("feature/x"),
        "https://gitlab.example.com/gitlab/group/app/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2Fx"
    );
    let run = ide_ci::Run {
        id: 501,
        name: "Pipeline".into(),
        state: State::Failed,
        url: String::new(),
        branch: Some("main".into()),
        sha: SHA.into(),
        event: None,
        number: 501,
        created: None,
        updated: None,
    };
    assert!(client.rerun_all(&run).is_err());
    assert_eq!(posted(&recorded).len(), 3);
}
