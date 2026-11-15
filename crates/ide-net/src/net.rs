//! The client every request goes through, and the `Net` seam installers and registers fetch with.

use std::io::{self, Read};
use std::time::Duration;

/// How a client is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Client<'a> {
    /// What the requests are for, which the user agent names, such as `CI`.
    pub(crate) purpose: &'a str,
    /// How long connecting may take.
    pub(crate) connect: Duration,
    /// How long one read may wait for bytes.
    pub(crate) read: Duration,
    /// Whether an error status ends the call as an error, rather than a response the caller reads.
    pub(crate) status_as_error: bool,
    /// Whether every request and every redirect must be HTTPS; a client that also reaches plain addresses, such as
    /// a local model server, has its HTTPS requests kept on HTTPS with [`guarded`] instead.
    pub(crate) https_only: bool,
}

/// An agent for `client`: HTTPS checked against the system's certificate store, named after the app.
pub(crate) fn client(client: Client<'_>) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .https_only(client.https_only)
        .http_status_as_error(client.status_as_error)
        .user_agent(format!("{} ({})", ide_product::NAME, client.purpose))
        .timeout_connect(Some(client.connect))
        .timeout_per_read(Some(client.read))
        .build()
        .new_agent()
}

/// Whether a request to `url` has to stay on HTTPS through its redirects: every request that starts on HTTPS, since
/// ureq follows a redirect to plain HTTP unless told otherwise.
pub(crate) fn stays_https(url: &str) -> bool {
    url.get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// `call` refusing every redirect off HTTPS when `url` is an HTTPS address; a plain one, such as a local server's,
/// is the caller's choice.
pub(crate) fn guarded<B>(call: ureq::RequestBuilder<B>, url: &str) -> ureq::RequestBuilder<B> {
    if stays_https(url) {
        call.config().https_only(true).build()
    } else {
        call
    }
}

/// A download in progress.
pub struct Download {
    /// The size, if the server said.
    pub length: Option<u64>,
    /// The bytes.
    pub body: Box<dyn Read + Send>,
}

/// Fetches URLs; the only way installers and registers reach the network, so tests can stand in for it.
pub trait Net: Send + Sync {
    /// Starts fetching `url`, asking for `accept`; an error for anything but success.
    fn get(&self, url: &str, accept: &str) -> io::Result<Download>;

    /// Fetches `url` as text of at most `limit` bytes.
    fn get_text(&self, url: &str, limit: u64) -> io::Result<String> {
        let mut text = String::new();
        let read = self.get(url, "*/*")?.body.take(limit + 1).read_to_string(&mut text)?;
        if read as u64 > limit {
            return Err(io::Error::other(format!("{url} is larger than {limit} bytes")));
        }
        Ok(text)
    }
}

/// The network over HTTPS only, with time limits for connecting and for each read.
pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    /// A client whose requests are for `purpose`, such as `agent register`.
    pub fn new(purpose: &str) -> Http {
        Http {
            agent: client(Client {
                purpose,
                connect: Duration::from_secs(30),
                read: Duration::from_secs(60),
                status_as_error: true,
                https_only: true,
            }),
        }
    }
}

impl Net for Http {
    fn get(&self, url: &str, accept: &str) -> io::Result<Download> {
        if !url.starts_with("https://") {
            return Err(io::Error::other(format!("{url} is not an HTTPS address")));
        }
        let response = self
            .agent
            .get(url)
            .header("Accept", accept)
            .call()
            .map_err(io::Error::other)?;
        let body = response.into_body();
        Ok(Download {
            length: body.content_length(),
            body: Box::new(body.into_reader()),
        })
    }
}

/// A client for a web service's API, such as GitHub's or GitLab's, whose answers come back whatever their status. A
/// request that starts on HTTPS stays on it through its redirects, and no redirect carries the `Authorization` header.
pub struct Service {
    agent: ureq::Agent,
}

/// What a service answered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Answer {
    /// The status code.
    pub status: u16,
    /// The headers, names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Service {
    /// A client whose requests are for `purpose`, such as `CI`, with time limits for connecting and for each read.
    pub fn new(purpose: &str, connect: Duration, read: Duration) -> Service {
        Service {
            agent: client(Client {
                purpose,
                connect,
                read,
                status_as_error: false,
                https_only: false,
            }),
        }
    }

    /// Sends a GET to `url`, or a POST of `post` when there is one, with `headers`; an answer longer than `limit`
    /// bytes is an error, as is no answer at all.
    pub fn send(&self, url: &str, headers: &[(String, String)], post: Option<&[u8]>, limit: u64) -> io::Result<Answer> {
        let result = match post {
            None => {
                let mut call = guarded(self.agent.get(url), url);
                for (name, value) in headers {
                    call = call.header(name, value);
                }
                call.call()
            }
            Some(body) => {
                let mut call = guarded(self.agent.post(url), url);
                for (name, value) in headers {
                    call = call.header(name, value);
                }
                if body.is_empty() {
                    call.send_empty()
                } else {
                    call.send(body)
                }
            }
        };
        let mut response = result.map_err(io::Error::other)?;
        let headers = response
            .headers()
            .iter()
            .map(|(n, v)| (n.as_str().to_ascii_lowercase(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(io::Error::other)?;
        Ok(Answer { status, headers, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Text(&'static str);

    impl Net for Text {
        fn get(&self, _: &str, _: &str) -> io::Result<Download> {
            Ok(Download {
                length: None,
                body: Box::new(io::Cursor::new(self.0.as_bytes())),
            })
        }
    }

    #[test]
    fn text_up_to_the_limit_is_read_and_more_is_refused() {
        assert_eq!(Text("four").get_text("u", 4).unwrap(), "four");
        assert!(Text("fives").get_text("u", 4).is_err());
    }

    #[test]
    fn only_https_leaves_the_mac() {
        let refused = Http::new("test").get("http://example.com/", "*/*").err().unwrap();
        assert!(refused.to_string().contains("not an HTTPS address"));
        assert!(
            Http::new("test").agent.config().https_only(),
            "a redirect cannot leave HTTPS either"
        );
    }

    #[test]
    fn a_request_that_starts_on_https_stays_on_it() {
        assert!(stays_https("https://api.github.com/x"));
        assert!(stays_https("HTTPS://Example.com"));
        assert!(!stays_https("http://localhost:11434/api/generate"));
        assert!(!stays_https("https:/"));
    }

    /// A plain HTTP server on the loopback that answers `/start` with a redirect to `/end`, and `/end` with `done`.
    fn redirecting() -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(4).flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                let _ = reader.read_line(&mut first);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let answer = if first.contains("/start") {
                    format!(
                        "HTTP/1.1 302 Found\r\nlocation: http://{address}/end\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    )
                } else {
                    "HTTP/1.1 200 OK\r\ncontent-length: 4\r\nconnection: close\r\n\r\ndone".to_string()
                };
                let _ = (&stream).write_all(answer.as_bytes());
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn a_request_kept_on_https_refuses_plain_http_and_a_plain_one_still_follows_redirects() {
        let base = redirecting();
        let agent = client(Client {
            purpose: "test",
            connect: Duration::from_secs(5),
            read: Duration::from_secs(5),
            status_as_error: false,
            https_only: false,
        });
        let start = format!("{base}/start");
        let mut followed = guarded(agent.get(&start), &start).call().unwrap();
        assert_eq!(followed.body_mut().read_to_string().unwrap(), "done");
        let marked = agent.get(&start).config().https_only(true).build().call();
        assert!(matches!(marked, Err(ureq::Error::RequireHttpsOnly(_))), "{marked:?}");
        let answer = Service::new("test", Duration::from_secs(5), Duration::from_secs(5))
            .send(&format!("{base}/end"), &[], None, 1024)
            .unwrap();
        assert_eq!((answer.status, answer.body.as_slice()), (200, &b"done"[..]));
    }
}
