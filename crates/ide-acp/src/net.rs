//! The register's and the installer's only way onto the network, so that tests can stand in for it.

use std::io::{self, Read};
use std::time::Duration;

/// Fetches URLs.
pub trait Net: Send + Sync {
    /// Starts fetching `url`; an error for anything but success.
    fn get(&self, url: &str) -> io::Result<Box<dyn Read + Send>>;

    /// Fetches `url` as text of at most `limit` bytes.
    fn get_text(&self, url: &str, limit: u64) -> io::Result<String> {
        let mut text = String::new();
        let read = self.get(url)?.take(limit + 1).read_to_string(&mut text)?;
        if read as u64 > limit {
            return Err(io::Error::other(format!("{url} is larger than {limit} bytes")));
        }
        Ok(text)
    }
}

/// HTTPS checked against the system's certificate store.
pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    /// A client with time limits for connecting and for each read.
    pub fn new() -> Http {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .user_agent(format!("{} (agent register)", ide_product::NAME))
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_per_read(Some(Duration::from_secs(60)))
            .build()
            .new_agent();
        Http { agent }
    }
}

impl Default for Http {
    fn default() -> Http {
        Http::new()
    }
}

impl Net for Http {
    fn get(&self, url: &str) -> io::Result<Box<dyn Read + Send>> {
        if !url.starts_with("https://") {
            return Err(io::Error::other(format!("{url} is not an HTTPS address")));
        }
        let response = self.agent.get(url).call().map_err(io::Error::other)?;
        Ok(Box::new(response.into_body().into_reader()))
    }
}
