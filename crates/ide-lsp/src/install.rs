//! Installing a missing server into the app's tools folder, after the user agreed: gopls with `go install`,
//! rust-analyzer from its GitHub releases with a checked digest, vtsls and the other npm servers with `npm install`.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ide_process::Supervisor;
use serde::Deserialize;

use crate::registry::{Install, node_bin, tools_bin};
use crate::tool::{self, Exits, Job, RunError};
pub use ide_net::install::{InstallError, Progress, Step};
use ide_net::{Cancel, Net};

/// The latest rust-analyzer release, as GitHub's API describes it.
const RUST_ANALYZER_RELEASE: &str = "https://api.github.com/repos/rust-lang/rust-analyzer/releases/latest";

/// How long `go install` or `npm install` may take.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);

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
            Install::Npm { package, program } => self.npm(package, program, cancel, progress)?,
            Install::Eslint => self.eslint(cancel, progress)?,
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

    fn npm(
        &self,
        package: &str,
        program: &str,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, InstallError> {
        let path = tool::var(&self.env, "PATH");
        tool::which("node", path).ok_or(InstallError::Missing("node"))?;
        let npm = tool::which("npm", path).ok_or(InstallError::Missing("npm"))?;
        fs::create_dir_all(&self.tools)?;
        let prefix = self.tools.display().to_string();
        progress(Progress::Step(Step::Running));
        let args = ["install", "--prefix", &prefix, "--no-fund", "--no-audit", package];
        self.command(&self.env, &npm, &args, cancel, progress)?;
        installed(node_bin(&self.tools, program), "npm install")
    }

    /// Downloads the archive of `ESLINT_TAG`, checks its digest, unpacks it and builds the server with npm.
    fn eslint(&self, cancel: &Cancel, progress: &mut dyn FnMut(Progress)) -> Result<PathBuf, InstallError> {
        use crate::registry::{ESLINT_DIGEST, ESLINT_TAG, eslint_server};
        let path = tool::var(&self.env, "PATH");
        tool::which("node", path).ok_or(InstallError::Missing("node"))?;
        let npm = tool::which("npm", path).ok_or(InstallError::Missing("npm"))?;
        let folder = self.tools.join("vscode-eslint");
        fs::create_dir_all(&folder)?;
        let packed = Scratch(folder.join(format!(".eslint-{}.tar.gz", std::process::id())));
        progress(Progress::Step(Step::Downloading));
        let url = format!("https://github.com/microsoft/vscode-eslint/archive/refs/tags/{ESLINT_TAG}.tar.gz");
        let actual = self.download(&url, &packed.0, cancel, progress)?;
        progress(Progress::Step(Step::Verifying));
        if actual != ESLINT_DIGEST {
            return Err(InstallError::Checksum {
                expected: ESLINT_DIGEST.into(),
                actual,
            });
        }
        progress(Progress::Step(Step::Unpacking));
        let unpacked = folder.join(ESLINT_TAG.replace('/', "-"));
        let _ = fs::remove_dir_all(&unpacked);
        fs::create_dir_all(&unpacked)?;
        let archive = packed.0.display().to_string();
        let target = unpacked.display().to_string();
        let tar = PathBuf::from("/usr/bin/tar");
        self.command(
            &self.env,
            &tar,
            &["-xzf", &archive, "-C", &target, "--strip-components", "1"],
            cancel,
            progress,
        )?;
        progress(Progress::Step(Step::Running));
        self.command_in(
            &unpacked,
            &npm,
            &["install", "--no-fund", "--no-audit"],
            cancel,
            progress,
        )?;
        self.command_in(&unpacked, &npm, &["run", "compile:server"], cancel, progress)?;
        installed(eslint_server(&self.tools), "npm run compile:server")
    }

    fn command_in(
        &self,
        dir: &Path,
        program: &Path,
        args: &[&str],
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<(), InstallError> {
        self.run_in(dir, &self.env, program, args, cancel, progress)
    }

    fn command(
        &self,
        env: &[(String, String)],
        program: &Path,
        args: &[&str],
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<(), InstallError> {
        self.run_in(&self.tools, env, program, args, cancel, progress)
    }

    fn run_in(
        &self,
        dir: &Path,
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
            .run(program, args, dir, &mut |line| {
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
        Ok(ide_net::download(
            &*self.net,
            url,
            path,
            cancel,
            None,
            &mut |done, total| {
                progress(Progress::Bytes { done, total });
            },
        )?)
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
    use std::io;

    use ide_net::Download;
    use ide_process::Ending;
    use sha2::{Digest, Sha256};

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

    fn tools(name: &str) -> ide_core::testing::Scratch {
        ide_core::testing::scratch(&format!("lsp-install-{name}"))
    }

    fn env() -> Vec<(String, String)> {
        vec![("PATH".into(), "/usr/bin:/bin".into())]
    }

    #[test]
    fn rust_analyzer_is_downloaded_checked_unpacked_and_made_runnable() {
        let dir = tools("ok");
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(release(None)));
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
        let installer = Installer::new(
            &Supervisor::new(),
            env(),
            dir.to_path_buf(),
            Arc::new(release(Some(&wrong))),
        );
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
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let result = installer.install(Install::RustAnalyzer, &Cancel::new(), &mut |_| {});
        assert!(matches!(result, Err(InstallError::Unverifiable)));
    }

    #[test]
    fn a_cancelled_installation_stops_and_leaves_nothing() {
        let dir = tools("cancel");
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(release(None)));
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
            tools("missing").to_path_buf(),
            Arc::new(Canned::default()),
        );
        let gopls = installer.install(Install::Gopls, &Cancel::new(), &mut |_| {});
        assert!(matches!(gopls, Err(InstallError::Missing("go"))));
        let vtsls = installer.install(crate::VTSLS, &Cancel::new(), &mut |_| {});
        assert!(matches!(vtsls, Err(InstallError::Missing("node"))));
    }

    /// A folder of executable shell scripts, by name, for the PATH of an installer.
    fn scripts(dir: &Path, scripts: &[(&str, &str)]) -> String {
        let bin = dir.join("path");
        fs::create_dir_all(&bin).unwrap();
        for (name, body) in scripts {
            let path = bin.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
        format!("{}:/usr/bin:/bin", bin.display())
    }

    fn outputs(installer: &Installer, what: Install) -> (Result<PathBuf, InstallError>, Vec<Progress>) {
        let mut seen = vec![];
        let result = installer.install(what, &Cancel::new(), &mut |p| seen.push(p));
        (result, seen)
    }

    #[test]
    fn gopls_is_installed_by_go_install_into_the_tools_folder() {
        let dir = tools("gopls");
        let go = "echo \"installing $2\" >&2\nprintf '#!/bin/sh\\n' > \"$GOBIN/gopls\"\nchmod 755 \"$GOBIN/gopls\"";
        let path = scripts(&dir, &[("go", go)]);
        let installer = Installer::new(
            &Supervisor::new(),
            vec![("PATH".into(), path)],
            dir.join("tools"),
            Arc::new(Canned::default()),
        );
        let (result, seen) = outputs(&installer, Install::Gopls);
        assert_eq!(result.unwrap(), dir.join("tools/bin/gopls"));
        assert_eq!(
            seen,
            [
                Progress::Step(Step::Running),
                Progress::Output("installing golang.org/x/tools/gopls@latest".into()),
                Progress::Step(Step::Done),
            ]
        );
    }

    #[test]
    fn an_npm_server_is_installed_under_the_tools_folder_as_its_prefix() {
        let dir = tools("npm");
        let npm = "mkdir -p \"$3/node_modules/.bin\"\nprintf '#!/bin/sh\\n' > \"$3/node_modules/.bin/vtsls\"\nchmod 755 \"$3/node_modules/.bin/vtsls\"\necho \"added $6\" >&2";
        let path = scripts(&dir, &[("node", ""), ("npm", npm)]);
        let installer = Installer::new(
            &Supervisor::new(),
            vec![("PATH".into(), path)],
            dir.join("tools"),
            Arc::new(Canned::default()),
        );
        let (result, seen) = outputs(&installer, crate::VTSLS);
        assert_eq!(result.unwrap(), dir.join("tools/node_modules/.bin/vtsls"));
        assert!(seen.contains(&Progress::Output("added @vtsls/language-server".into())));
    }

    #[test]
    fn a_failing_or_empty_install_command_says_what_went_wrong() {
        let dir = tools("npmfail");
        let path = scripts(
            &dir,
            &[
                ("node", ""),
                ("npm", "echo 'E404 not found' >&2\nexit 1"),
                ("go", "exit 0"),
            ],
        );
        let installer = Installer::new(
            &Supervisor::new(),
            vec![("PATH".into(), path)],
            dir.join("tools"),
            Arc::new(Canned::default()),
        );
        let (failed, _) = outputs(&installer, crate::VTSLS);
        let Err(InstallError::Failed {
            program,
            ending,
            output,
        }) = failed
        else {
            panic!("npm failed: {failed:?}");
        };
        assert!(program.ends_with("/npm"));
        assert_eq!((ending, output.trim()), (Some(Ending::Code(1)), "E404 not found"));
        let (empty, _) = outputs(&installer, Install::Gopls);
        let Err(InstallError::Unusable(why)) = empty else {
            panic!("go install left nothing: {empty:?}");
        };
        assert!(why.starts_with("go install left no "), "{why}");
    }

    #[test]
    fn node_without_npm_is_reported_as_missing_npm() {
        let dir = tools("nonpm");
        let path = scripts(&dir, &[("node", "")]);
        let installer = Installer::new(
            &Supervisor::new(),
            vec![("PATH".into(), path)],
            dir.join("tools"),
            Arc::new(Canned::default()),
        );
        assert!(matches!(
            outputs(&installer, crate::VTSLS).0,
            Err(InstallError::Missing("npm"))
        ));
    }

    #[test]
    fn a_release_without_a_download_for_this_mac_or_no_network_is_a_network_error() {
        let dir = tools("noasset");
        let mut canned = release(None);
        let json = serde_json::json!({"tag_name": "2099-01-01", "assets": [{"name": "rust-analyzer-sparc.gz", "browser_download_url": "x"}]});
        canned
            .files
            .insert(RUST_ANALYZER_RELEASE.to_string(), serde_json::to_vec(&json).unwrap());
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let Err(InstallError::Network(why)) = outputs(&installer, Install::RustAnalyzer).0 else {
            panic!("no asset is a network error");
        };
        assert_eq!(why, format!("release 2099-01-01 has no {}", asset_name()));
        let offline = Installer::new(
            &Supervisor::new(),
            env(),
            dir.to_path_buf(),
            Arc::new(Canned::default()),
        );
        let Err(InstallError::Network(why)) = outputs(&offline, Install::RustAnalyzer).0 else {
            panic!("an unreachable release is a network error");
        };
        assert_eq!(why, "404");
    }

    #[test]
    fn a_downloaded_server_that_does_not_start_is_not_installed() {
        let dir = tools("unusable");
        let program = gzip(b"#!/bin/sh\necho 'dyld: missing symbol' >&2\nexit 3\n");
        let url = "https://example.invalid/broken.gz";
        let json = serde_json::json!({
            "tag_name": "t",
            "assets": [{"name": asset_name(), "browser_download_url": url, "digest": format!("sha256:{}", sha256(&program))}],
        });
        let canned = Canned {
            files: HashMap::from([
                (RUST_ANALYZER_RELEASE.to_string(), serde_json::to_vec(&json).unwrap()),
                (url.to_string(), program.clone()),
            ]),
            asked: Mutex::default(),
        };
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let (result, seen) = outputs(&installer, Install::RustAnalyzer);
        let Err(InstallError::Unusable(why)) = result else {
            panic!("a broken download is unusable: {result:?}");
        };
        assert_eq!(why.trim(), "dyld: missing symbol");
        let total = Some(program.len() as u64);
        assert!(seen.contains(&Progress::Bytes {
            done: program.len() as u64,
            total
        }));
        assert_eq!(
            fs::read_dir(dir.join("bin")).unwrap().count(),
            0,
            "nothing stays behind"
        );
    }

    #[test]
    fn every_failure_reads_as_a_sentence() {
        let cases = [
            (InstallError::Cancelled, "the installation was cancelled"),
            (InstallError::Missing("go"), "go is not installed"),
            (
                InstallError::Failed {
                    program: "npm".into(),
                    ending: None,
                    output: " E404 \n".into(),
                },
                "npm failed (None): E404",
            ),
            (InstallError::Network("offline".into()), "the download failed: offline"),
            (
                InstallError::Checksum {
                    expected: "aa".into(),
                    actual: "bb".into(),
                },
                "the download is damaged: expected aa, got bb",
            ),
            (
                InstallError::Unverifiable,
                "the release publishes no digest to check the download against",
            ),
            (
                InstallError::Unusable("exit 3".into()),
                "the installed program does not start: exit 3",
            ),
            (
                InstallError::from(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
                "the tools folder could not be written: denied",
            ),
        ];
        for (error, text) in cases {
            assert_eq!(error.to_string(), text);
        }
    }
}
