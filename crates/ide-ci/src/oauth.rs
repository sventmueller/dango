//! Signing in through the browser (E12): GitHub's device flow, and GitLab's authorization code with PKCE for a
//! public client, whose browser comes back to a one-shot listener on the loopback address. Both give a [`Grant`].

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::client::encode;
use crate::error::Error;
use crate::http::{Method, Request, Response, Transport, now};

/// The scopes GitHub's device flow asks for: reading and re-running Actions of private repositories needs both.
pub const GITHUB_SCOPES: &str = "repo workflow";

/// The scope GitLab's login asks for: retrying and cancelling pipelines needs the full API.
pub const GITLAB_SCOPES: &str = "api";

/// The loopback port GitLab sends the browser back to; the redirect URI registered with the application names it.
pub const CALLBACK_PORT: u16 = 47819;

/// How long the browser has to come back.
pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// The most of the browser's request line that is read; the callback's is a few hundred bytes.
const REQUEST_LINE_LIMIT: u64 = 16 * 1024;

/// A token from signing in, with what renews it where the service hands that out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The access token.
    pub token: String,
    /// The refresh token; GitLab replaces it on every refresh.
    pub refresh: Option<String>,
    /// When the access token expires, in seconds since 1970.
    pub expires: Option<i64>,
}

impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant")
            .field("refresh", &self.refresh.as_ref().map(|_| "…"))
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

/// Why signing in did not work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
    /// The user declined in the browser.
    Denied,
    /// The code ran out, or the browser did not come back in time.
    Expired,
    /// The browser came back with another sign-in's state, so the answer is not taken.
    StateMismatch,
    /// Another program uses the loopback port the browser comes back to.
    PortInUse(u16),
    /// The user stopped it.
    Cancelled,
    /// The service refused, with its reason.
    Refused(String),
    /// The service could not be asked.
    Api(Error),
}

impl From<Error> for LoginError {
    fn from(error: Error) -> LoginError {
        LoginError::Api(error)
    }
}

/// What a token endpoint answers, a token or an error.
#[derive(Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
    interval: Option<u64>,
}

impl TokenAnswer {
    fn grant(self) -> Option<Grant> {
        Some(Grant {
            token: self.access_token?,
            refresh: self.refresh_token,
            expires: self.expires_in.map(|seconds| now() + seconds),
        })
    }

    fn refused(&self) -> LoginError {
        LoginError::Refused(
            self.error_description
                .clone()
                .or_else(|| self.error.clone())
                .unwrap_or_else(|| "no token".into()),
        )
    }
}

/// POSTs `fields` as a form to `url`, asking for JSON.
fn post_form(transport: &dyn Transport, url: &str, fields: &[(&str, &str)]) -> Result<Response, Error> {
    let body = fields
        .iter()
        .map(|(name, value)| format!("{name}={}", encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let request = Request {
        method: Method::Post,
        url: url.to_string(),
        headers: vec![
            ("Accept".into(), "application/json".into()),
            ("Content-Type".into(), "application/x-www-form-urlencoded".into()),
        ],
        body: body.into_bytes(),
    };
    transport.send(&request)
}

fn answer(response: &Response) -> Result<TokenAnswer, LoginError> {
    serde_json::from_slice(&response.body).map_err(|e| LoginError::Api(Error::Decode(e.to_string())))
}

/// What GitHub hands out to start a device flow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceCode {
    /// The code the app polls with.
    pub(crate) device_code: String,
    /// The code the user types in the browser, such as `WDJB-MJHT`.
    pub user_code: String,
    /// Where the user types it.
    pub verification_uri: String,
    /// How many seconds both codes stay valid.
    #[serde(default = "default_expiry")]
    pub(crate) expires_in: u64,
    /// How many seconds to wait between polls at least.
    #[serde(default = "default_interval")]
    pub interval: u64,
}

/// RFC 8628's defaults, and GitHub's.
fn default_expiry() -> u64 {
    900
}

fn default_interval() -> u64 {
    5
}

/// Asks GitHub at `web`, such as `https://github.com`, for a device code of the OAuth app `client_id`.
pub fn device_code(
    transport: &dyn Transport,
    web: &str,
    client_id: &str,
    scopes: &str,
) -> Result<DeviceCode, LoginError> {
    let response = post_form(
        transport,
        &format!("{web}/login/device/code"),
        &[("client_id", client_id), ("scope", scopes)],
    )?;
    if !(200..300).contains(&response.status) {
        return Err(answer(&response).map_or_else(|e| e, |a| a.refused()));
    }
    serde_json::from_slice(&response.body).map_err(|_| answer(&response).map_or_else(|e| e, |a| a.refused()))
}

/// Polls GitHub until the user confirmed `code` in the browser, waiting with `wait` between polls as the server asks;
/// `wait` returns false when the user cancels.
pub fn device_token(
    transport: &dyn Transport,
    web: &str,
    client_id: &str,
    code: &DeviceCode,
    wait: &mut dyn FnMut(Duration) -> bool,
) -> Result<Grant, LoginError> {
    let url = format!("{web}/login/oauth/access_token");
    let mut interval = code.interval.max(1);
    let mut waited = 0;
    loop {
        if waited >= code.expires_in {
            return Err(LoginError::Expired);
        }
        if !wait(Duration::from_secs(interval)) {
            return Err(LoginError::Cancelled);
        }
        waited += interval;
        let response = post_form(
            transport,
            &url,
            &[
                ("client_id", client_id),
                ("device_code", &code.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
        )?;
        let answer = answer(&response)?;
        match answer.error.as_deref() {
            None => return answer.grant().ok_or(LoginError::Refused("no token".into())),
            Some("authorization_pending") => {}
            Some("slow_down") => interval = answer.interval.unwrap_or(interval + 5).max(1),
            Some("expired_token" | "token_expired") => return Err(LoginError::Expired),
            Some("access_denied") => return Err(LoginError::Denied),
            Some(_) => return Err(answer.refused()),
        }
    }
}

/// The secrets of one authorization code login: the verifier, its challenge, and the state that ties the browser's
/// answer to this login.
#[derive(Clone, PartialEq, Eq)]
pub struct Pkce {
    /// The verifier, sent only with the code exchange.
    pub(crate) verifier: String,
    /// Its S256 challenge, sent with the authorization.
    pub(crate) challenge: String,
    /// What the browser must bring back.
    pub state: String,
}

impl Pkce {
    /// A new verifier from 32 random bytes and a new state from 16.
    pub fn new() -> Result<Pkce, LoginError> {
        let random = |count: usize| -> Result<String, LoginError> {
            let mut bytes = vec![0u8; count];
            security_framework::random::SecRandom::default()
                .copy_bytes(&mut bytes)
                .map_err(|e| LoginError::Refused(e.to_string()))?;
            Ok(URL_SAFE_NO_PAD.encode(bytes))
        };
        Ok(Pkce::with(&random(32)?, &random(16)?))
    }

    /// The login with `verifier` and `state`.
    pub fn with(verifier: &str, state: &str) -> Pkce {
        Pkce {
            verifier: verifier.to_string(),
            challenge: challenge(verifier),
            state: state.to_string(),
        }
    }

    /// Where the browser goes to sign in to the GitLab at `web`.
    pub fn authorize_url(&self, web: &str, client_id: &str, redirect: &str, scopes: &str) -> String {
        format!(
            "{web}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope={}&code_challenge={}&code_challenge_method=S256",
            encode(client_id),
            encode(redirect),
            encode(&self.state),
            encode(scopes),
            encode(&self.challenge)
        )
    }
}

/// The S256 challenge of `verifier`: its SHA-256, base64url without padding.
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// The redirect URI of the listener on `port`.
pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}/callback")
}

/// Listens on the loopback `port` for the browser's return.
pub fn listen(port: u16) -> Result<TcpListener, LoginError> {
    TcpListener::bind(("127.0.0.1", port)).map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => LoginError::PortInUse(port),
        _ => LoginError::Refused(e.to_string()),
    })
}

/// Waits on `listener` for the browser to come back to `/callback`, shows it `page`, and gives the code, once its
/// state is `state`; other paths, such as a favicon, get a 404.
pub fn callback_code(
    listener: &TcpListener,
    state: &str,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
    page: &str,
) -> Result<String, LoginError> {
    listener
        .set_nonblocking(true)
        .map_err(|e| LoginError::Refused(e.to_string()))?;
    let deadline = Instant::now() + timeout;
    loop {
        if cancelled() {
            return Err(LoginError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(LoginError::Expired);
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(result) = answer_browser(stream, state, page) {
                    return result;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(LoginError::Refused(e.to_string())),
        }
    }
}

/// Reads one request; `None` when it is not the callback.
fn answer_browser(stream: TcpStream, state: &str, page: &str) -> Option<Result<String, LoginError>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    // Any local program can connect to the port; a request line without an end must not fill the memory.
    reader.by_ref().take(REQUEST_LINE_LIMIT).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1).unwrap_or("");
    let mut out = stream;
    let Some(query) = target.strip_prefix("/callback?") else {
        let _ = out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return None;
    };
    let body = format!("<!doctype html><meta charset=utf-8><p>{page}</p>");
    let _ = out.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
    let get = |name: &str| {
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
            .map(decode)
    };
    Some(if get("state").as_deref() != Some(state) {
        Err(LoginError::StateMismatch)
    } else if let Some(error) = get("error") {
        Err(if error == "access_denied" {
            LoginError::Denied
        } else {
            LoginError::Refused(error)
        })
    } else {
        get("code").ok_or(LoginError::Refused("no code".into()))
    })
}

/// A percent-encoded query value as text.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Exchanges the browser's `code` for a token at the GitLab at `web`.
pub fn exchange_code(
    transport: &dyn Transport,
    web: &str,
    client_id: &str,
    code: &str,
    pkce: &Pkce,
    redirect: &str,
) -> Result<Grant, LoginError> {
    let response = post_form(
        transport,
        &format!("{web}/oauth/token"),
        &[
            ("client_id", client_id),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect),
            ("code_verifier", &pkce.verifier),
        ],
    )?;
    let answer = answer(&response)?;
    if answer.error.is_some() {
        return Err(answer.refused());
    }
    answer.grant().ok_or(LoginError::Refused("no token".into()))
}

/// A new token for `refresh` at the GitLab at `web`; the old token and refresh token stop working.
pub fn refresh_grant(
    transport: &dyn Transport,
    web: &str,
    client_id: &str,
    refresh: &str,
    redirect: &str,
) -> Result<Grant, LoginError> {
    let response = post_form(
        transport,
        &format!("{web}/oauth/token"),
        &[
            ("client_id", client_id),
            ("refresh_token", refresh),
            ("grant_type", "refresh_token"),
            ("redirect_uri", redirect),
        ],
    )?;
    let answer = answer(&response)?;
    if answer.error.is_some() {
        return Err(answer.refused());
    }
    answer.grant().ok_or(LoginError::Refused("no token".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_of_rfc_7636s_example_verifier_is_its_example_challenge() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let fresh = Pkce::new().unwrap();
        assert_eq!(fresh.verifier.len(), 43);
        assert!(
            fresh
                .verifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_ne!(fresh.state, Pkce::new().unwrap().state);
    }

    #[test]
    fn a_slow_down_without_a_wait_still_waits_a_second() {
        let url = "https://github.com/login/oauth/access_token";
        let recorded = crate::http::Recorded::new()
            .answer(Method::Post, url, 200, r#"{"error":"slow_down","interval":0}"#, &[])
            .answer(Method::Post, url, 200, r#"{"access_token":"t"}"#, &[]);
        let code = DeviceCode {
            device_code: "d".into(),
            user_code: "WDJB-MJHT".into(),
            verification_uri: "https://github.com/login/device".into(),
            expires_in: 900,
            interval: 5,
        };
        let mut waits = Vec::new();
        let grant = device_token(&recorded, "https://github.com", "id", &code, &mut |wait| {
            waits.push(wait);
            true
        })
        .unwrap();
        assert_eq!(grant.token, "t");
        assert_eq!(waits, [Duration::from_secs(5), Duration::from_secs(1)]);
    }

    #[test]
    fn a_request_line_without_an_end_is_cut_off_and_the_real_callback_still_counts() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = std::thread::spawn(move || {
            let mut flood = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let _ = flood.write_all(&vec![b'x'; 64 * 1024]);
            let mut real = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let _ = real.write_all(b"GET /callback?code=c0de&state=s1 HTTP/1.1\r\n\r\n");
            let mut answer = String::new();
            let _ = real.read_to_string(&mut answer);
            drop(flood);
            answer
        });
        let started = Instant::now();
        let code = callback_code(&listener, "s1", Duration::from_secs(30), &|| false, "done");
        assert_eq!(code, Ok("c0de".to_string()));
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "the endless line held the login for {:?}",
            started.elapsed()
        );
        assert!(browser.join().unwrap().contains("200 OK"));
    }

    #[test]
    fn the_authorize_url_carries_every_parameter_encoded() {
        let pkce = Pkce::with("verifier", "st/ate");
        let url = pkce.authorize_url(
            "https://gitlab.com",
            "app id",
            &redirect_uri(CALLBACK_PORT),
            GITLAB_SCOPES,
        );
        assert!(url.starts_with("https://gitlab.com/oauth/authorize?client_id=app%20id&"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A47819%2Fcallback"));
        assert!(url.contains("state=st%2Fate&scope=api&code_challenge="));
        assert!(url.ends_with("&code_challenge_method=S256"));
        assert_eq!(decode("a%2Fb+c%3"), "a/b c%3");
    }
}
