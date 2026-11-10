//! Installing a missing server into the app's tools folder, after the user agreed: gopls with `go install`,
//! rust-analyzer from its GitHub releases with a checked digest, vtsls with `npm install`.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ide_process::{Ending, Supervisor};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::registry::{Install, tools_bin, vtsls_bin};
use crate::tool::{self, Cancel, Exits, Job, RunError};

/// The latest rust-analyzer release, as GitHub's API describes it.
const RUST_ANALYZER_RELEASE: &str = "https://api.github.com/repos/rust-lang/rust-analyzer/releases/latest";

/// How long `go install` or `npm install` may take.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A download in progress.
pub struct Download {
    /// The size, if the server said.
    pub length: Option<u64>,
    /// The bytes.
    pub body: Box<dyn Read + Send>,
}

/// Fetches URLs; the installer's only way onto the network, so tests can stand in for it.
pub trait Net: Send + Sync {
    /// Starts fetching `url`, asking for `accept`; an error for anything but success.
    fn get(&self, url: &str, accept: &str) -> io::Result<Download>;
}

/// The network, through HTTPS checked against the system's certificate store.
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
            .user_agent(format!("{} (language server installer)", ide_product::NAME))
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
    fn get(&self, url: &str, accept: &str) -> io::Result<Download> {
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

/// A stage of an installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Finding out what to download.
    Resolving,
    /// Downloading.
    Downloading,
    /// Checking what was downloaded.
    Verifying,
    /// Unpacking it.
    Unpacking,
    /// Running the installing program, such as `go install`.
    Running,
    /// Installed.
    Done,
}

/// What an installation reports while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// A new stage began.
    Step(Step),
    /// Bytes downloaded so far.
    Bytes {
        /// So far.
        done: u64,
        /// Of this many, if known.
        total: Option<u64>,
    },
    /// A line the installing program printed.
    Output(String),
}

/// Why an installation failed.
#[derive(Debug)]
pub enum InstallError {
    /// The user cancelled it.
    Cancelled,
    /// A program it needs is not on the login shell's PATH, such as `go`, `node` or `npm`.
    Missing(&'static str),
    /// The installing program failed.
    Failed {
        /// Which.
        program: String,
        /// How it ended; `None` if it took too long.
        ending: Option<Ending>,
        /// What it printed to standard error.
        output: String,
    },
    /// The network failed or the release has no download for this Mac.
    Network(String),
    /// The download does not match the digest the release publishes.
    Checksum {
        /// Published.
        expected: String,
        /// Downloaded.
        actual: String,
    },
    /// The release publishes no digest, so the download cannot be checked.
    Unverifiable,
    /// The installed program does not start.
    Unusable(String),
    /// Writing the tools folder failed.
    Io(io::Error),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::Cancelled => f.write_str("the installation was cancelled"),
            InstallError::Missing(program) => write!(f, "{program} is not installed"),
            InstallError::Failed {
                program,
                ending,
                output,
            } => {
                write!(f, "{program} failed ({ending:?}): {}", output.trim())
            }
            InstallError::Network(why) => write!(f, "the download failed: {why}"),
            InstallError::Checksum { expected, actual } => {
                write!(f, "the download is damaged: expected {expected}, got {actual}")
            }
            InstallError::Unverifiable => f.write_str("the release publishes no digest to check the download against"),
            InstallError::Unusable(why) => write!(f, "the installed server does not start: {why}"),
            InstallError::Io(error) => write!(f, "the tools folder could not be written: {error}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> InstallError {
        InstallError::Io(error)
    }
}

/// Installs servers into the tools folder with the login shell's environment.
pub struct Installer {
    supervisor: Supervisor,
    exits: Exits,
    env: Vec<(String, String)>,
    tools: PathBuf,
    net: Arc<dyn Net>,
}

impl Installer {
    /// An installer for `tools`, using `env`, the login shell's environment, and `net` for downloads.
    pub fn new(supervisor: &Supervisor, env: Vec<(String, String)>, tools: PathBuf, net: Arc<dyn Net>) -> Installer {
        Installer {
            supervisor: supervisor.clone(),
            exits: Exits::watch(supervisor),
            env,
            tools,
            net,
        }
    }

    /// Installs `what` and returns the program's path; blocks for minutes, so call it off the main thread.
    pub fn install(
        &self,
        what: Install,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, InstallError> {
        let path = match what {
            Install::Gopls => self.gopls(cancel, progress)?,
            Install::RustAnalyzer => self.rust_analyzer(cancel, progress)?,
            Install::Vtsls => self.vtsls(cancel, progress)?,
        };
        progress(Progress::Step(Step::Done));
        Ok(path)
    }

    fn gopls(&self, cancel: &Cancel, progress: &mut dyn FnMut(Progress)) -> Result<PathBuf, InstallError> {
        let go = tool::which("go", tool::var(&self.env, "PATH")).ok_or(InstallError::Missing("go"))?;
        let bin = self.tools.join("bin");
        fs::create_dir_all(&bin)?;
        let mut env = self.env.clone();
        env.push(("GOBIN".to_string(), bin.display().to_string()));
        progress(Progress::Step(Step::Running));
        self.command(
            &env,
            &go,
            &["install", "golang.org/x/tools/gopls@latest"],
            cancel,
            progress,
        )?;
        installed(tools_bin(&self.tools, "gopls"), "go install")
    }

    fn vtsls(&self, cancel: &Cancel, progress: &mut dyn FnMut(Progress)) -> Result<PathBuf, InstallError> {
        let path = tool::var(&self.env, "PATH");
        tool::which("node", path).ok_or(InstallError::Missing("node"))?;
        let npm = tool::which("npm", path).ok_or(InstallError::Missing("npm"))?;
        fs::create_dir_all(&self.tools)?;
        let prefix = self.tools.display().to_string();
        progress(Progress::Step(Step::Running));
        let args = [
            "install",
            "--prefix",
            &prefix,
            "--no-fund",
            "--no-audit",
            "@vtsls/language-server",
        ];
        self.command(&self.env, &npm, &args, cancel, progress)?;
        installed(vtsls_bin(&self.tools), "npm install")
    }

    fn command(
        &self,
        env: &[(String, String)],
        program: &Path,
        args: &[&str],
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<(), InstallError> {
        let job = Job {
            supervisor: &self.supervisor,
            exits: &self.exits,
            env,
            timeout: COMMAND_TIMEOUT,
            cancel: Some(cancel),
        };
        let name = program.display().to_string();
        let output = job
            .run(program, args, &self.tools, &mut |line| {
                progress(Progress::Output(line.to_string()))
            })
            .map_err(|error| match error {
                RunError::Cancelled => InstallError::Cancelled,
                RunError::Timeout => InstallError::Failed {
                    program: name.clone(),
                    ending: None,
                    output: String::new(),
                },
                RunError::Start(error) => InstallError::Io(error),
            })?;
        if output.succeeded() {
            Ok(())
        } else {
            Err(InstallError::Failed {
                program: name,
                ending: output.ending,
                output: output.stderr,
            })
        }
    }

    fn rust_analyzer(&self, cancel: &Cancel, progress: &mut dyn FnMut(Progress)) -> Result<PathBuf, InstallError> {
        progress(Progress::Step(Step::Resolving));
        let asset = self.release_asset()?;
        let expected = asset
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .ok_or(InstallError::Unverifiable)?
            .to_ascii_lowercase();
        let bin = self.tools.join("bin");
        fs::create_dir_all(&bin)?;
        let id = std::process::id();
        let packed = Scratch(bin.join(format!(".rust-analyzer-{id}.gz")));
        let unpacked = Scratch(bin.join(format!(".rust-analyzer-{id}")));
        progress(Progress::Step(Step::Downloading));
        let actual = self.download(&asset.browser_download_url, &packed.0, cancel, progress)?;
        progress(Progress::Step(Step::Verifying));
        if actual != expected {
            return Err(InstallError::Checksum { expected, actual });
        }
        progress(Progress::Step(Step::Unpacking));
        gunzip(&packed.0, &unpacked.0, cancel)?;
        fs::set_permissions(&unpacked.0, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;
        self.check_runs(&unpacked.0)?;
        let target = tools_bin(&self.tools, "rust-analyzer");
        fs::rename(&unpacked.0, &target)?;
        Ok(target)
    }

    fn release_asset(&self) -> Result<Asset, InstallError> {
        let mut download = self
            .net
            .get(RUST_ANALYZER_RELEASE, "application/vnd.github+json")
            .map_err(|e| InstallError::Network(e.to_string()))?;
        let mut json = Vec::new();
        download
            .body
            .read_to_end(&mut json)
            .map_err(|e| InstallError::Network(e.to_string()))?;
        let release: Release = serde_json::from_slice(&json).map_err(|e| InstallError::Network(e.to_string()))?;
        let name = asset_name();
        release
            .assets
            .into_iter()
            .find(|asset| asset.name == name)
            .ok_or_else(|| InstallError::Network(format!("release {} has no {name}", release.tag_name)))
    }

    /// Downloads `url` into `path` and returns the SHA-256 of what arrived, in lowercase hex.
    fn download(
        &self,
        url: &str,
        path: &Path,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<String, InstallError> {
        let mut download = self
            .net
            .get(url, "application/octet-stream")
            .map_err(|e| InstallError::Network(e.to_string()))?;
        let mut file = File::create(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err(InstallError::Cancelled);
            }
            let n = download
                .body
                .read(&mut buffer)
                .map_err(|e| InstallError::Network(e.to_string()))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            file.write_all(&buffer[..n])?;
            done += n as u64;
            progress(Progress::Bytes {
                done,
                total: download.length,
            });
        }
        file.sync_all()?;
        Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }

    fn check_runs(&self, program: &Path) -> Result<(), InstallError> {
        let job = Job {
            supervisor: &self.supervisor,
            exits: &self.exits,
            env: &self.env,
            timeout: Duration::from_secs(30),
            cancel: None,
        };
        match job.run(program, &["--version"], &self.tools, &mut |_| {}) {
            Ok(output) if output.succeeded() => Ok(()),
            Ok(output) => Err(InstallError::Unusable(output.stderr)),
            Err(error) => Err(InstallError::Unusable(format!("{error:?}"))),
        }
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

/// The release file for this machine, such as `rust-analyzer-aarch64-apple-darwin.gz`.
fn asset_name() -> String {
    let system = if cfg!(target_os = "macos") {
        "apple-darwin"
    } else {
        "unknown-linux-gnu"
    };
    format!("rust-analyzer-{}-{system}.gz", std::env::consts::ARCH)
}

fn gunzip(from: &Path, to: &Path, cancel: &Cancel) -> Result<(), InstallError> {
    let mut decoder = flate2::read::GzDecoder::new(File::open(from)?);
    let mut out = File::create(to)?;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
        let n = decoder.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        out.write_all(&buffer[..n])?;
    }
    out.sync_all()?;
    Ok(())
}

fn installed(path: PathBuf, by: &str) -> Result<PathBuf, InstallError> {
    if tool::is_executable(&path) {
        Ok(path)
    } else {
        Err(InstallError::Unusable(format!("{by} left no {}", path.display())))
    }
}

/// A file that exists only during an installation; removed when it ends, whichever way.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    /// Serves fixed bytes for fixed URLs and records what was asked.
    #[derive(Default)]
    struct Canned {
        files: HashMap<String, Vec<u8>>,
        asked: Mutex<Vec<String>>,
    }

    impl Net for Canned {
        fn get(&self, url: &str, _: &str) -> io::Result<Download> {
            self.asked.lock().unwrap().push(url.to_string());
            let bytes = self
                .files
                .get(url)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "404"))?;
            Ok(Download {
                length: Some(bytes.len() as u64),
                body: Box::new(io::Cursor::new(bytes)),
            })
        }
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn sha256(bytes: &[u8]) -> String {
        Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn release(digest: Option<&str>) -> Canned {
        let program = gzip(b"#!/bin/sh\necho rust-analyzer 0.0.0-test\n");
        let url = "https://example.invalid/ra.gz";
        let digest = digest.map_or_else(|| format!("sha256:{}", sha256(&program)), str::to_string);
        let json = serde_json::json!({
            "tag_name": "test",
            "assets": [{"name": asset_name(), "browser_download_url": url, "digest": digest}],
        });
        Canned {
            files: HashMap::from([
                (RUST_ANALYZER_RELEASE.to_string(), serde_json::to_vec(&json).unwrap()),
                (url.to_string(), program),
            ]),
            asked: Mutex::default(),
        }
    }

    fn tools(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ide-lsp-install-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn env() -> Vec<(String, String)> {
        vec![("PATH".into(), "/usr/bin:/bin".into())]
    }

    #[test]
    fn rust_analyzer_is_downloaded_checked_unpacked_and_made_runnable() {
        let dir = tools("ok");
        let installer = Installer::new(&Supervisor::new(), env(), dir.clone(), Arc::new(release(None)));
        let mut steps = vec![];
        let path = installer
            .install(Install::RustAnalyzer, &Cancel::new(), &mut |p| {
                if let Progress::Step(step) = p {
                    steps.push(step);
                }
            })
            .unwrap();
        assert_eq!(path, dir.join("bin/rust-analyzer"));
        assert!(tool::is_executable(&path));
        assert_eq!(
            steps,
            [
                Step::Resolving,
                Step::Downloading,
                Step::Verifying,
                Step::Unpacking,
                Step::Done
            ]
        );
        let left: Vec<_> = fs::read_dir(dir.join("bin"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["rust-analyzer"], "no scratch files stay behind");
    }

    #[test]
    fn a_download_that_does_not_match_its_digest_is_thrown_away() {
        let dir = tools("bad");
        let wrong = format!("sha256:{}", "0".repeat(64));
        let installer = Installer::new(&Supervisor::new(), env(), dir.clone(), Arc::new(release(Some(&wrong))));
        let result = installer.install(Install::RustAnalyzer, &Cancel::new(), &mut |_| {});
        assert!(matches!(result, Err(InstallError::Checksum { .. })));
        assert!(!dir.join("bin/rust-analyzer").exists());
        assert_eq!(fs::read_dir(dir.join("bin")).unwrap().count(), 0);
    }

    #[test]
    fn a_release_without_a_digest_is_not_installed() {
        let dir = tools("nodigest");
        let mut canned = release(None);
        let json =
            serde_json::json!({"tag_name": "t", "assets": [{"name": asset_name(), "browser_download_url": "x"}]});
        canned
            .files
            .insert(RUST_ANALYZER_RELEASE.to_string(), serde_json::to_vec(&json).unwrap());
        let installer = Installer::new(&Supervisor::new(), env(), dir, Arc::new(canned));
        let result = installer.install(Install::RustAnalyzer, &Cancel::new(), &mut |_| {});
        assert!(matches!(result, Err(InstallError::Unverifiable)));
    }

    #[test]
    fn a_cancelled_installation_stops_and_leaves_nothing() {
        let dir = tools("cancel");
        let installer = Installer::new(&Supervisor::new(), env(), dir.clone(), Arc::new(release(None)));
        let cancel = Cancel::new();
        let result = installer.install(Install::RustAnalyzer, &cancel, &mut |p| {
            if p == Progress::Step(Step::Downloading) {
                cancel.cancel();
            }
        });
        assert!(matches!(result, Err(InstallError::Cancelled)));
        assert!(!dir.join("bin/rust-analyzer").exists());
    }

    #[test]
    fn missing_go_or_node_is_reported_by_name() {
        let installer = Installer::new(
            &Supervisor::new(),
            vec![("PATH".into(), "/nonexistent".into())],
            tools("missing"),
            Arc::new(Canned::default()),
        );
        let gopls = installer.install(Install::Gopls, &Cancel::new(), &mut |_| {});
        assert!(matches!(gopls, Err(InstallError::Missing("go"))));
        let vtsls = installer.install(Install::Vtsls, &Cancel::new(), &mut |_| {});
        assert!(matches!(vtsls, Err(InstallError::Missing("node"))));
    }
}
