//! Signing in through the browser against a stub server on the loopback address: GitHub's device flow through
//! pending, slow down and success, its refusals; GitLab's authorization code with PKCE through the browser's return,
//! a wrong state, the code exchange and the refresh that replaces both tokens; and renewing a kept token.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_ci::oauth::{self, LoginError, Pkce};
use ide_ci::{Auth, Commands, HostRule, Memory, Network, Printed, Source, candidates};

/// Answers each path from a queue of bodies, the last one repeating, and keeps every request's path and body.
struct Stub {
    port: u16,
    seen: Arc<Mutex<Vec<(String, String)>>>,
}

impl Stub {
    fn start(script: &[(&str, &[&str])]) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let queues: HashMap<String, Vec<String>> = script
            .iter()
            .map(|(path, bodies)| ((*path).to_string(), bodies.iter().map(|b| (*b).to_string()).collect()))
            .collect();
        let queues = Arc::new(Mutex::new(queues));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &queues, &kept);
            }
        });
        Stub { port, seen }
    }

    fn web(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn bodies(&self, path: &str) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p == path)
            .map(|(_, b)| b.clone())
            .collect()
    }
}

fn serve(stream: TcpStream, queues: &Mutex<HashMap<String, Vec<String>>>, seen: &Mutex<Vec<(String, String)>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap() == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    seen.lock()
        .unwrap()
        .push((path.clone(), String::from_utf8_lossy(&body).into_owned()));
    let answer = {
        let mut queues = queues.lock().unwrap();
        match queues.get_mut(&path) {
            Some(queue) if queue.len() > 1 => queue.remove(0),
            Some(queue) => queue[0].clone(),
            None => "{\"error\":\"not_found\"}".into(),
        }
    };
    let mut out = stream;
    let _ = write!(
        out,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
        answer.len()
    );
}

const DEVICE_CODE: &str = r#"{"device_code":"dc-123","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#;

#[test]
fn the_device_flow_waits_out_pending_and_slow_down_and_then_gets_a_token() {
    let stub = Stub::start(&[
        ("/login/device/code", &[DEVICE_CODE]),
        (
            "/login/oauth/access_token",
            &[
                r#"{"error":"authorization_pending"}"#,
                r#"{"error":"slow_down","interval":10}"#,
                r#"{"access_token":"gho_token","token_type":"bearer","scope":"repo,workflow"}"#,
            ],
        ),
    ]);
    let network = Network::new();
    let code = oauth::device_code(&network, &stub.web(), "client 1", oauth::GITHUB_SCOPES).unwrap();
    assert_eq!((code.user_code.as_str(), code.interval), ("WDJB-MJHT", 5));
    let mut waits = Vec::new();
    let grant = oauth::device_token(&network, &stub.web(), "client 1", &code, &mut |d| {
        waits.push(d.as_secs());
        true
    })
    .unwrap();
    assert_eq!(grant.token, "gho_token");
    assert_eq!(grant.refresh, None);
    assert_eq!(waits, [5, 5, 10]);
    assert_eq!(
        stub.bodies("/login/device/code"),
        ["client_id=client%201&scope=repo%20workflow"]
    );
    let polls = stub.bodies("/login/oauth/access_token");
    assert_eq!(polls.len(), 3);
    assert_eq!(
        polls[0],
        "client_id=client%201&device_code=dc-123&grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"
    );
}

#[test]
fn a_declined_expired_cancelled_or_misconfigured_device_flow_says_why() {
    let cases: [(&str, LoginError); 3] = [
        (r#"{"error":"access_denied"}"#, LoginError::Denied),
        (r#"{"error":"expired_token"}"#, LoginError::Expired),
        (
            r#"{"error":"device_flow_disabled","error_description":"Device flow is disabled"}"#,
            LoginError::Refused("Device flow is disabled".into()),
        ),
    ];
    for (answer, expected) in cases {
        let stub = Stub::start(&[
            ("/login/device/code", &[DEVICE_CODE]),
            ("/login/oauth/access_token", &[answer]),
        ]);
        let network = Network::new();
        let code = oauth::device_code(&network, &stub.web(), "c", "repo").unwrap();
        assert_eq!(
            oauth::device_token(&network, &stub.web(), "c", &code, &mut |_| true),
            Err(expected)
        );
    }
    let stub = Stub::start(&[("/login/device/code", &[DEVICE_CODE])]);
    let network = Network::new();
    let code = oauth::device_code(&network, &stub.web(), "c", "repo").unwrap();
    assert_eq!(
        oauth::device_token(&network, &stub.web(), "c", &code, &mut |_| false),
        Err(LoginError::Cancelled)
    );
    let refused = Stub::start(&[("/login/device/code", &[r#"{"error":"unauthorized_client"}"#])]);
    assert_eq!(
        oauth::device_code(&network, &refused.web(), "c", "repo"),
        Err(LoginError::Refused("unauthorized_client".into()))
    );
}

/// What a browser coming back to the listener sends, after a request for a favicon.
fn browser_returns(port: u16, query: &'static str) {
    std::thread::spawn(move || {
        for path in ["/favicon.ico".to_string(), format!("/callback?{query}")] {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
            let mut answer = String::new();
            let _ = stream.read_to_string(&mut answer);
        }
    });
}

fn listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

#[test]
fn the_browser_brings_the_code_back_only_with_this_logins_state() {
    let wait = Duration::from_secs(5);
    let (open, port) = listener();
    browser_returns(port, "code=abc%2B1&state=s1");
    assert_eq!(
        oauth::callback_code(&open, "s1", wait, &|| false, "done").as_deref(),
        Ok("abc+1")
    );
    let (other, port) = listener();
    browser_returns(port, "code=abc&state=forged");
    assert_eq!(
        oauth::callback_code(&other, "s1", wait, &|| false, "done"),
        Err(LoginError::StateMismatch)
    );
    let (denied, port) = listener();
    browser_returns(port, "error=access_denied&state=s1");
    assert_eq!(
        oauth::callback_code(&denied, "s1", wait, &|| false, "done"),
        Err(LoginError::Denied)
    );
    let (quiet, _) = listener();
    assert_eq!(
        oauth::callback_code(&quiet, "s1", Duration::from_millis(200), &|| false, "done"),
        Err(LoginError::Expired)
    );
    assert_eq!(
        oauth::callback_code(&quiet, "s1", wait, &|| true, "done"),
        Err(LoginError::Cancelled)
    );
    let (_taken, port) = listener();
    assert!(matches!(oauth::listen(port), Err(LoginError::PortInUse(p)) if p == port));
}

#[test]
fn the_code_and_verifier_buy_a_token_and_a_refresh_replaces_both() {
    let stub = Stub::start(&[(
        "/gitlab/oauth/token",
        &[
            r#"{"access_token":"a1","token_type":"bearer","expires_in":7200,"refresh_token":"r1","created_at":1}"#,
            r#"{"access_token":"a2","token_type":"bearer","expires_in":7200,"refresh_token":"r2","created_at":2}"#,
        ],
    )]);
    let network = Network::new();
    let web = format!("{}/gitlab", stub.web());
    let pkce = Pkce::with("verifier-1", "state-1");
    let redirect = oauth::redirect_uri(oauth::CALLBACK_PORT);
    let grant = oauth::exchange_code(&network, &web, "app", "code-1", &pkce, &redirect).unwrap();
    assert_eq!((grant.token.as_str(), grant.refresh.as_deref()), ("a1", Some("r1")));
    assert!(grant.expires.is_some_and(|at| at > ide_ci::now() + 7000));
    let renewed = oauth::refresh_grant(&network, &web, "app", "r1", &redirect).unwrap();
    assert_eq!((renewed.token.as_str(), renewed.refresh.as_deref()), ("a2", Some("r2")));
    let bodies = stub.bodies("/gitlab/oauth/token");
    assert_eq!(
        bodies[0],
        "client_id=app&code=code-1&grant_type=authorization_code&redirect_uri=http%3A%2F%2F127.0.0.1%3A47819%2Fcallback&code_verifier=verifier-1"
    );
    assert_eq!(
        bodies[1],
        "client_id=app&refresh_token=r1&grant_type=refresh_token&redirect_uri=http%3A%2F%2F127.0.0.1%3A47819%2Fcallback"
    );
}

struct NoTools;

impl Commands for NoTools {
    fn run(&self, _program: &str, _args: &[&str]) -> Option<Printed> {
        None
    }
}

#[test]
fn a_kept_token_about_to_expire_is_renewed_and_kept_again_per_account() {
    let stub = Stub::start(&[(
        "/gitlab/oauth/token",
        &[r#"{"access_token":"new","expires_in":7200,"refresh_token":"r-new"}"#],
    )]);
    let rules = [HostRule::parse(&format!("127.0.0.1:{}/gitlab=gitlab", stub.port)).unwrap()];
    let forge = candidates(&format!("http://127.0.0.1:{}/gitlab/group/app.git", stub.port), &rules).remove(0);
    let auth = Auth::new(Arc::new(Memory::default()), Arc::new(NoTools), Vec::new());
    let soon = oauth::Grant {
        token: "old".into(),
        refresh: Some("r-old".into()),
        expires: Some(ide_ci::now() + 10),
    };
    auth.save_grant(&forge, "lab-user", &soon, "app").unwrap();
    let fresh = auth.fresh(&forge, &Network::new()).unwrap();
    assert_eq!(
        (fresh.token.as_str(), fresh.account.as_deref()),
        ("new", Some("lab-user"))
    );
    assert_eq!(fresh.source, Source::Keychain);
    assert_eq!(auth.find(&forge).unwrap().token, "new");
    assert!(stub.bodies("/gitlab/oauth/token")[0].contains("refresh_token=r-old"));
    let later = oauth::Grant {
        token: "lasting".into(),
        refresh: Some("r".into()),
        expires: Some(ide_ci::now() + 3600),
    };
    auth.save_grant(&forge, "lab-user", &later, "app").unwrap();
    assert_eq!(auth.fresh(&forge, &Network::new()).unwrap().token, "lasting");
    assert_eq!(stub.bodies("/gitlab/oauth/token").len(), 1);
    assert_eq!(
        auth.renew(&forge, &Network::new(), "lasting").unwrap().token,
        "new",
        "a refused token is renewed at once"
    );
    assert_eq!(stub.bodies("/gitlab/oauth/token").len(), 2);
    assert_eq!(
        auth.renew(&forge, &Network::new(), "lasting").unwrap().token,
        "new",
        "a token another read renewed meanwhile is taken as it is"
    );
    assert_eq!(stub.bodies("/gitlab/oauth/token").len(), 2);
    auth.save(&forge, "lab-user", "typed").unwrap();
    assert_eq!(
        auth.renew(&forge, &Network::new(), "typed"),
        None,
        "a typed token has nothing to renew it with"
    );
    auth.forget(&forge).unwrap();
    assert_eq!(auth.find(&forge), None);
}
