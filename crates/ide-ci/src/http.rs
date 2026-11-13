//! Calls to a service's API: the transport, status codes as typed errors, rate limits and paging.

use ide_core::{IgnoringPoison, LockIgnoringPoison};
use std::sync::Mutex;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// The most of a response body that is read; a log longer than this keeps its end.
pub(crate) const BODY_LIMIT: usize = 16 * 1024 * 1024;

/// How many pages a list follows at most.
pub(crate) const MAX_PAGES: usize = 10;

/// An HTTP method the APIs need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Reading.
    Get,
    /// Re-running or cancelling.
    Post,
}

/// A call to an API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The method.
    pub method: Method,
    /// The whole URL.
    pub url: String,
    /// The headers, the token among them.
    pub headers: Vec<(String, String)>,
    /// What a POST sends; empty for none.
    pub body: Vec<u8>,
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The headers, names in lower case.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Response {
    /// The first header called `name`, in any case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The body as text, with invalid UTF-8 replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// How calls reach the network, so tests can answer them from recordings.
pub trait Transport: Send + Sync {
    /// Sends `request`; an error only when there is no answer at all.
    fn send(&self, request: &Request) -> Result<Response>;
}

/// The network over HTTPS checked against the system's certificates. Redirects keep no `Authorization` header, so a
/// log's download from blob storage gets no token.
pub struct Network {
    agent: ide_net::ureq::Agent,
}

impl Network {
    /// A client with time limits for connecting and reading.
    pub fn new() -> Network {
        let agent = ide_net::client(ide_net::Client {
            purpose: "CI",
            connect: Duration::from_secs(20),
            read: Duration::from_secs(60),
            status_as_error: false,
        });
        Network { agent }
    }
}

impl Default for Network {
    fn default() -> Network {
        Network::new()
    }
}

impl Transport for Network {
    fn send(&self, request: &Request) -> Result<Response> {
        let result = match request.method {
            Method::Get => {
                let mut call = self.agent.get(&request.url);
                for (name, value) in &request.headers {
                    call = call.header(name, value);
                }
                call.call()
            }
            Method::Post => {
                let mut call = self.agent.post(&request.url);
                for (name, value) in &request.headers {
                    call = call.header(name, value);
                }
                if request.body.is_empty() {
                    call.send_empty()
                } else {
                    call.send(&request.body[..])
                }
            }
        };
        let mut response = result.map_err(|e| Error::Network(e.to_string()))?;
        let headers = response
            .headers()
            .iter()
            .map(|(n, v)| (n.as_str().to_ascii_lowercase(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(BODY_LIMIT as u64)
            .read_to_vec()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Response { status, headers, body })
    }
}

/// Answers from a list of recorded responses by method and URL, and keeps what was asked; for tests. Several answers
/// to one URL come one after the other, and the last one stays.
#[derive(Default)]
pub struct Recorded {
    answers: Mutex<Vec<(Method, String, Response)>>,
    asked: Mutex<Vec<Request>>,
}

impl Recorded {
    /// No answers yet.
    pub fn new() -> Recorded {
        Recorded::default()
    }

    /// Answers `method` on `url` with `status` and `body`, plus `headers`.
    pub fn answer(mut self, method: Method, url: &str, status: u16, body: &str, headers: &[(&str, &str)]) -> Recorded {
        let response = Response {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| (n.to_ascii_lowercase(), (*v).to_string()))
                .collect(),
            body: body.as_bytes().to_vec(),
        };
        self.answers
            .get_mut()
            .ignoring_poison()
            .push((method, url.to_string(), response));
        self
    }

    /// The requests so far.
    pub fn asked(&self) -> Vec<Request> {
        self.asked.lock_ignoring_poison().clone()
    }
}

impl Transport for Recorded {
    fn send(&self, request: &Request) -> Result<Response> {
        self.asked.lock_ignoring_poison().push(request.clone());
        let mut answers = self.answers.lock_ignoring_poison();
        let fits = |(m, url, _): &(Method, String, Response)| *m == request.method && *url == request.url;
        let first = answers
            .iter()
            .position(fits)
            .ok_or_else(|| Error::Network(format!("no recording for {}", request.url)))?;
        let more = answers.iter().skip(first + 1).any(fits);
        Ok(if more {
            answers.remove(first).2
        } else {
            answers[first].2.clone()
        })
    }
}

/// `response` as `Ok`, or the error its status means; `what` names the thing asked for.
pub(crate) fn checked(response: Response, host: &str, what: &str) -> Result<Response> {
    let status = response.status;
    if (200..300).contains(&status) {
        return Ok(response);
    }
    let remaining = response
        .header("x-ratelimit-remaining")
        .or(response.header("ratelimit-remaining"));
    let limited = status == 429 || (status == 403 && remaining == Some("0"));
    if limited {
        let reset = response
            .header("x-ratelimit-reset")
            .or(response.header("ratelimit-reset"))
            .and_then(|v| v.trim().parse().ok());
        let after = response
            .header("retry-after")
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|seconds| now() + seconds);
        return Err(Error::RateLimited { until: reset.or(after) });
    }
    let message = message(&response);
    Err(match status {
        401 => Error::Unauthorized { host: host.to_string() },
        403 => Error::Forbidden { message },
        404 => Error::NotFound { what: what.to_string() },
        _ => Error::Http { status, message },
    })
}

/// The `message` both services put in their error bodies, else the start of the body.
fn message(response: &Response) -> String {
    let text = response.text();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| {
            v.get("message")
                .or(v.get("error"))
                .map(|m| m.as_str().map_or_else(|| m.to_string(), str::to_string))
        })
        .unwrap_or_else(|| text.chars().take(200).collect())
}

/// The body as `T`.
pub(crate) fn json<T: DeserializeOwned>(response: &Response) -> Result<T> {
    serde_json::from_slice(&response.body).map_err(|e| Error::Decode(e.to_string()))
}

/// The URL of the next page from a `Link` header, which both services send.
pub(crate) fn next_page(response: &Response) -> Option<String> {
    let link = response.header("link")?;
    link.split(',').find_map(|part| {
        let (url, params) = part.split_once(';')?;
        params
            .split(';')
            .any(|p| p.trim().replace(' ', "") == "rel=\"next\"")
            .then(|| url.trim().trim_start_matches('<').trim_end_matches('>').to_string())
    })
}

/// Seconds since 1970.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, headers: &[(&str, &str)], body: &str) -> Response {
        Response {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_string(), (*v).to_string()))
                .collect(),
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn statuses_become_typed_errors_and_spent_limits_say_until_when() {
        assert!(checked(response(200, &[], "{}"), "h", "x").is_ok());
        assert_eq!(
            checked(
                response(401, &[], "{\"message\":\"Bad credentials\"}"),
                "github.com",
                "x"
            ),
            Err(Error::Unauthorized {
                host: "github.com".into()
            })
        );
        assert_eq!(
            checked(
                response(
                    403,
                    &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1800000000")],
                    ""
                ),
                "h",
                "x"
            ),
            Err(Error::RateLimited {
                until: Some(1_800_000_000)
            })
        );
        assert_eq!(
            checked(
                response(
                    403,
                    &[("x-ratelimit-remaining", "12")],
                    "{\"message\":\"Resource not accessible\"}"
                ),
                "h",
                "x"
            ),
            Err(Error::Forbidden {
                message: "Resource not accessible".into()
            })
        );
        assert!(matches!(
            checked(response(429, &[("retry-after", "30")], ""), "h", "x"),
            Err(Error::RateLimited { until: Some(_) })
        ));
        assert_eq!(
            checked(
                response(404, &[], "{\"message\":\"404 Project Not Found\"}"),
                "h",
                "project"
            ),
            Err(Error::NotFound { what: "project".into() })
        );
        assert_eq!(
            checked(response(500, &[], "boom"), "h", "x"),
            Err(Error::Http {
                status: 500,
                message: "boom".into()
            })
        );
    }

    #[test]
    fn the_next_page_comes_from_the_link_header() {
        let page = response(
            200,
            &[(
                "link",
                "<https://api.github.com/x?page=1>; rel=\"prev\", <https://api.github.com/x?page=3>; rel=\"next\"",
            )],
            "",
        );
        assert_eq!(next_page(&page).as_deref(), Some("https://api.github.com/x?page=3"));
        assert_eq!(next_page(&response(200, &[], "")), None);
    }
}
