//! A small HTTP server on 127.0.0.1 that answers as GitHub's or GitLab's API would, from the fixtures of `ide-ci`, and
//! keeps what it was asked. A run that runs finishes after it was read twice; a cancelled pipeline stays cancelled.
//! Their OAuth endpoints sign in without a token: GitHub's device flow says "pending" once, GitLab's token endpoint
//! hands out the token the API wants.

use ide_core::LockIgnoringPoison;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// The service a stub plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Service {
    GitHub,
    GitLab,
}

/// A request the stub got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Seen {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) authorization: Option<String>,
    pub(crate) body: String,
}

/// The token GitHub's device flow hands out.
pub(crate) const DEVICE_TOKEN: &str = "gho_selftest";

/// The code GitHub's device flow shows.
pub(crate) const USER_CODE: &str = "SELF-TEST";

#[derive(Default)]
struct Inner {
    sha: String,
    branch: String,
    seen: Vec<Seen>,
    run_reads: usize,
    cancelled: bool,
    device_polls: usize,
}

/// A running stub.
pub(crate) struct Stub {
    pub(crate) port: u16,
    service: Service,
    token: Option<&'static str>,
    inner: Arc<Mutex<Inner>>,
}

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!("../../../crates/ide-ci/tests/fixtures/", $path))
    };
}

impl Stub {
    /// Listens on a free port; answers only requests that carry `token`, when there is one.
    pub(crate) fn start(service: Service, token: Option<&'static str>) -> std::io::Result<Arc<Stub>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let stub = Arc::new(Stub {
            port: listener.local_addr()?.port(),
            service,
            token,
            inner: Arc::new(Mutex::new(Inner::default())),
        });
        let serving = stub.clone();
        std::thread::Builder::new()
            .name("selftest-ci-stub".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let _ = serving.serve(stream);
                }
            })?;
        Ok(stub)
    }

    /// The commit and branch its runs belong to.
    pub(crate) fn set_head(&self, sha: &str, branch: &str) {
        let mut inner = self.lock();
        inner.sha = sha.to_string();
        inner.branch = branch.to_string();
    }

    /// What it was asked so far.
    pub(crate) fn seen(&self) -> Vec<Seen> {
        self.lock().seen.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock_ignoring_poison()
    }

    fn serve(&self, stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let mut parts = line.split_whitespace();
        let (method, target) = (
            parts.next().unwrap_or("").to_string(),
            parts.next().unwrap_or("").to_string(),
        );
        let mut authorization = None;
        let mut length = 0;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                if name.eq_ignore_ascii_case("authorization") {
                    authorization = Some(value.trim().to_string());
                } else if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
        }
        let mut sent = vec![0; length];
        reader.read_exact(&mut sent)?;
        let sent = String::from_utf8_lossy(&sent).into_owned();
        let signing_in = target.starts_with("/login/") || target.starts_with("/gitlab/oauth/");
        let allowed = signing_in
            || self
                .token
                .is_none_or(|t| authorization.as_deref() == Some(&format!("Bearer {t}")));
        self.lock().seen.push(Seen {
            method: method.clone(),
            target: target.clone(),
            authorization,
            body: sent,
        });
        let (status, body, headers) = if allowed {
            match self.service {
                Service::GitHub => self.github(&method, &target),
                Service::GitLab => self.gitlab(&method, &target),
            }
        } else {
            (401, "{\"message\":\"401 Unauthorized\"}".to_string(), Vec::new())
        };
        let mut out = stream;
        let mut head = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        out.write_all(head.as_bytes())?;
        out.write_all(body.as_bytes())?;
        out.flush()
    }

    fn fill(&self, text: &str) -> String {
        let inner = self.lock();
        text.replace("{{SHA}}", &inner.sha).replace("{{BRANCH}}", &inner.branch)
    }

    fn github(&self, method: &str, target: &str) -> (u16, String, Vec<(String, String)>) {
        let base = "/api/v3/repos/acme/widget/actions";
        let sha = self.lock().sha.clone();
        let done = self.lock().run_reads >= 2;
        let ok = |body: &str| (200, self.fill(body), Vec::new());
        match (method, target) {
            ("POST", "/login/device/code") => ok(&format!(
                "{{\"device_code\":\"device-selftest\",\"user_code\":\"{USER_CODE}\",\"verification_uri\":\"http://127.0.0.1:{}/login/device\",\"expires_in\":120,\"interval\":1}}",
                self.port
            )),
            ("POST", "/login/oauth/access_token") => {
                self.lock().device_polls += 1;
                if self.lock().device_polls == 1 {
                    ok("{\"error\":\"authorization_pending\"}")
                } else {
                    ok(&format!(
                        "{{\"access_token\":\"{DEVICE_TOKEN}\",\"token_type\":\"bearer\",\"scope\":\"repo,workflow\"}}"
                    ))
                }
            }
            ("GET", "/api/v3/user") => ok(fixture!("github/user.json")),
            ("GET", t) if t == format!("{base}/runs?head_sha={sha}&per_page=30") => {
                self.lock().run_reads += 1;
                let runs = self.fill(fixture!("github/runs.json"));
                let runs = if self.lock().run_reads >= 2 {
                    runs.replace(
                        "\"status\": \"in_progress\",\n      \"conclusion\": null",
                        "\"status\": \"completed\",\n      \"conclusion\": \"success\"",
                    )
                } else {
                    runs
                };
                (200, runs, Vec::new())
            }
            ("GET", t) if t == format!("{base}/runs/9001/jobs?filter=latest&per_page=100") => {
                let next = format!("<http://127.0.0.1:{}{t}&page=2>; rel=\"next\"", self.port);
                (
                    200,
                    self.fill(fixture!("github/jobs-9001-page1.json")),
                    vec![("Link".into(), next)],
                )
            }
            ("GET", t) if t == format!("{base}/runs/9001/jobs?filter=latest&per_page=100&page=2") => {
                ok(fixture!("github/jobs-9001-page2.json"))
            }
            ("GET", t) if t == format!("{base}/runs/9002/jobs?filter=latest&per_page=100") => {
                if done {
                    ok(fixture!("github/jobs-9002-done.json"))
                } else {
                    ok(fixture!("github/jobs-9002.json"))
                }
            }
            ("GET", t) if t == format!("{base}/jobs/72/logs") => ok(fixture!("github/log-72.txt")),
            ("POST", t) if t == format!("{base}/runs/9001/rerun-failed-jobs") => (201, String::new(), Vec::new()),
            ("POST", t) if t == format!("{base}/runs/9001/rerun") || t == format!("{base}/jobs/72/rerun") => {
                (201, String::new(), Vec::new())
            }
            ("POST", t) if t == format!("{base}/workflows/release.yml/dispatches") => (204, String::new(), Vec::new()),
            ("GET", "/api/v3/repos/acme/widget/pulls?state=open&sort=updated&direction=desc&per_page=50") => {
                ok(fixture!("github/pulls.json"))
            }
            ("GET", "/api/v3/repos/acme/widget/issues?state=open&sort=updated&direction=desc&per_page=50") => {
                ok(fixture!("github/issues.json"))
            }
            ("POST", t) if t == format!("{base}/runs/9002/cancel") => (202, "{}".into(), Vec::new()),
            _ => (404, "{\"message\":\"Not Found\"}".into(), Vec::new()),
        }
    }

    fn gitlab(&self, method: &str, target: &str) -> (u16, String, Vec<(String, String)>) {
        let base = "/gitlab/api/v4/projects/group%2Fapp";
        let sha = self.lock().sha.clone();
        let cancelled = self.lock().cancelled;
        let cancel = |text: String| {
            if cancelled {
                text.replace("\"running\"", "\"canceled\"")
            } else {
                text
            }
        };
        match (method, target) {
            ("GET", "/gitlab/api/v4/user") => (200, fixture!("gitlab/user.json").into(), Vec::new()),
            ("POST", "/gitlab/oauth/token") => (
                200,
                format!(
                    "{{\"access_token\":\"{}\",\"token_type\":\"bearer\",\"expires_in\":7200,\"refresh_token\":\"refresh-selftest\",\"created_at\":0}}",
                    self.token.unwrap_or_default()
                ),
                Vec::new(),
            ),
            ("GET", t) if t == format!("{base}/pipelines?sha={sha}&per_page=20") => {
                (200, cancel(self.fill(fixture!("gitlab/pipelines.json"))), Vec::new())
            }
            ("GET", t) if t == format!("{base}/pipelines/501/jobs?per_page=100") => (
                200,
                self.fill(fixture!("gitlab/jobs-page1.json")),
                vec![("X-Next-Page".into(), "2".into())],
            ),
            ("GET", t) if t == format!("{base}/pipelines/501/jobs?per_page=100&page=2") => (
                200,
                cancel(self.fill(fixture!("gitlab/jobs-page2.json"))),
                vec![("X-Next-Page".into(), String::new())],
            ),
            ("GET", t) if t == format!("{base}/jobs/3002/trace") => {
                (200, fixture!("gitlab/trace-3002.txt").into(), Vec::new())
            }
            ("POST", t) if t == format!("{base}/pipelines/501/retry") => (201, "{}".into(), Vec::new()),
            ("POST", t)
                if t == format!("{base}/jobs/3002/retry")
                    || t == format!("{base}/jobs/3004/play")
                    || t.starts_with(&format!("{base}/pipeline?ref=")) =>
            {
                (201, "{}".into(), Vec::new())
            }
            ("GET", t)
                if t == format!("{base}/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50") =>
            {
                (200, fixture!("gitlab/merge_requests.json").into(), Vec::new())
            }
            ("GET", t) if t == format!("{base}/issues?state=opened&order_by=updated_at&sort=desc&per_page=50") => {
                (200, fixture!("gitlab/issues.json").into(), Vec::new())
            }
            ("POST", t) if t == format!("{base}/pipelines/501/cancel") => {
                self.lock().cancelled = true;
                (200, "{}".into(), Vec::new())
            }
            _ => (404, "{\"message\":\"404 Not Found\"}".into(), Vec::new()),
        }
    }
}
