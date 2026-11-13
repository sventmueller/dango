//! The client every request goes through, and the `Net` seam installers and registers fetch with.

use std::io::{self, Read};
use std::time::Duration;

/// How a client is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client<'a> {
    /// What the requests are for, which the user agent names, such as `CI`.
    pub purpose: &'a str,
    /// How long connecting may take.
    pub connect: Duration,
    /// How long one read may wait for bytes.
    pub read: Duration,
    /// Whether an error status ends the call as an error, rather than a response the caller reads.
    pub status_as_error: bool,
}

/// An agent for `client`: HTTPS checked against the system's certificate store, named after the app.
pub fn client(client: Client<'_>) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .http_status_as_error(client.status_as_error)
        .user_agent(format!("{} ({})", ide_product::NAME, client.purpose))
        .timeout_connect(Some(client.connect))
        .timeout_per_read(Some(client.read))
        .build()
        .new_agent()
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
    }
}
