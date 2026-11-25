//! Installing a missing server into the app's tools folder, after the user agreed: gopls with `go install`,
//! rust-analyzer from a pinned GitHub release with a checked digest, vtsls and the other npm servers with
//! `npm install`.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ide_process::Supervisor;

use crate::registry::{Install, node_bin, tools_bin};
pub use ide_net::install::{InstallError, Progress, Step};
use ide_net::{Cancel, Net};
use ide_process::tool::{self, Helper, RunError};

/// The rust-analyzer release the app installs; a new one is pinned by hand after checking its digests.
const RUST_ANALYZER_TAG: &str = "2026-11-16";

/// The SHA-256 of the release's file for each processor, in lowercase hex.
const RUST_ANALYZER_DIGESTS: &[(&str, &str)] = &[
    (
        "rust-analyzer-aarch64-apple-darwin.gz",
        "608b7c7d6bec1fcd6c964578b1f8234888afe6d6c0e7915284f1e2fd36ef1343",
    ),
    (
        "rust-analyzer-x86_64-apple-darwin.gz",
        "6901d956b4f2b091343eaf3570944098488f3ffec018987145285ed8253c07d2",
    ),
];

/// How long `go install` or `npm install` may take.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Installs servers into the tools folder with the login shell's environment.
pub struct Installer {
    supervisor: Supervisor,
    env: Vec<(String, String)>,
    tools: PathBuf,
    net: Arc<dyn Net>,
}

impl Installer {
    /// An installer for `tools`, using `env`, the login shell's environment, and `net` for downloads.
    pub fn new(supervisor: &Supervisor, env: Vec<(String, String)>, tools: PathBuf, net: Arc<dyn Net>) -> Installer {
        Installer {
            supervisor: supervisor.clone(),
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
            Install::RustAnalyzer => self.rust_analyzer(&pinned_asset(&asset_name())?, cancel, progress)?,
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
        // A script node runs, not a program of its own, so it only has to be there.
        let server = eslint_server(&self.tools);
        if server.is_file() {
            Ok(server)
        } else {
            Err(InstallError::Unusable(format!(
                "npm run compile:server left no {}",
                server.display()
            )))
        }
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
        let job = Helper {
            supervisor: &self.supervisor,
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
                ending: Some(output.ending),
                output: output.stderr,
            })
        }
    }

    fn rust_analyzer(
        &self,
        asset: &Asset,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, InstallError> {
        let bin = self.tools.join("bin");
        fs::create_dir_all(&bin)?;
        let id = std::process::id();
        let packed = Scratch(bin.join(format!(".rust-analyzer-{id}.gz")));
        let unpacked = Scratch(bin.join(format!(".rust-analyzer-{id}")));
        progress(Progress::Step(Step::Downloading));
        let actual = self.download(&asset.url, &packed.0, cancel, progress)?;
        progress(Progress::Step(Step::Verifying));
        if actual != asset.sha256 {
            return Err(InstallError::Checksum {
                expected: asset.sha256.clone(),
                actual,
            });
        }
        progress(Progress::Step(Step::Unpacking));
        gunzip(&packed.0, &unpacked.0, cancel)?;
        fs::set_permissions(&unpacked.0, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;
        self.check_runs(&unpacked.0)?;
        let target = tools_bin(&self.tools, "rust-analyzer");
        fs::rename(&unpacked.0, &target)?;
        Ok(target)
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
        let job = Helper {
            supervisor: &self.supervisor,
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

/// A release file to download and the SHA-256 it must have, in lowercase hex.
struct Asset {
    url: String,
    sha256: String,
}

/// The pinned release's file `name`.
fn pinned_asset(name: &str) -> Result<Asset, InstallError> {
    let (_, sha256) = RUST_ANALYZER_DIGESTS
        .iter()
        .find(|(asset, _)| *asset == name)
        .ok_or_else(|| InstallError::Network(format!("release {RUST_ANALYZER_TAG} has no {name}")))?;
    Ok(Asset {
        url: format!("https://github.com/rust-lang/rust-analyzer/releases/download/{RUST_ANALYZER_TAG}/{name}"),
        sha256: (*sha256).to_string(),
    })
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

    /// A server answering a test release and its asset, which has the SHA-256 `digest` or the right one.
    fn release(digest: Option<&str>) -> (Canned, Asset) {
        served(b"#!/bin/sh\necho rust-analyzer 0.0.0-test\n", digest)
    }

    fn served(script: &[u8], digest: Option<&str>) -> (Canned, Asset) {
        let program = gzip(script);
        let url = "https://example.invalid/ra.gz";
        let asset = Asset {
            url: url.to_string(),
            sha256: digest.map_or_else(|| sha256(&program), str::to_string),
        };
        let canned = Canned {
            files: HashMap::from([(url.to_string(), program)]),
            asked: Mutex::default(),
        };
        (canned, asset)
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
        let (canned, asset) = release(None);
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let mut steps = vec![];
        let path = installer
            .rust_analyzer(&asset, &Cancel::new(), &mut |p| {
                if let Progress::Step(step) = p {
                    steps.push(step);
                }
            })
            .unwrap();
        assert_eq!(path, dir.join("bin/rust-analyzer"));
        assert!(tool::is_executable(&path));
        assert_eq!(steps, [Step::Downloading, Step::Verifying, Step::Unpacking]);
        let left: Vec<_> = fs::read_dir(dir.join("bin"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["rust-analyzer"], "no scratch files stay behind");
    }

    #[test]
    fn a_download_that_does_not_match_its_digest_is_thrown_away() {
        let dir = tools("bad");
        let wrong = "0".repeat(64);
        let (canned, asset) = release(Some(&wrong));
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let result = installer.rust_analyzer(&asset, &Cancel::new(), &mut |_| {});
        assert!(matches!(result, Err(InstallError::Checksum { .. })));
        assert!(!dir.join("bin/rust-analyzer").exists());
        assert_eq!(fs::read_dir(dir.join("bin")).unwrap().count(), 0);
    }

    #[test]
    fn rust_analyzer_comes_from_the_pinned_tag_with_a_digest_in_the_code() {
        for name in [
            "rust-analyzer-aarch64-apple-darwin.gz",
            "rust-analyzer-x86_64-apple-darwin.gz",
        ] {
            let asset = pinned_asset(name).unwrap();
            assert_eq!(
                asset.url,
                format!("https://github.com/rust-lang/rust-analyzer/releases/download/{RUST_ANALYZER_TAG}/{name}")
            );
            assert_eq!(asset.sha256.len(), 64);
            assert!(asset.sha256.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        }
        let dir = tools("pinned");
        let canned = Arc::new(Canned::default());
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), canned.clone());
        let Err(InstallError::Network(why)) = outputs(&installer, Install::RustAnalyzer).0 else {
            panic!("an unreachable release is a network error");
        };
        assert_eq!(why, "404");
        assert_eq!(
            *canned.asked.lock().unwrap(),
            [pinned_asset(&asset_name()).unwrap().url]
        );
    }

    #[test]
    fn a_cancelled_installation_stops_and_leaves_nothing() {
        let dir = tools("cancel");
        let (canned, asset) = release(None);
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let cancel = Cancel::new();
        let result = installer.rust_analyzer(&asset, &cancel, &mut |p| {
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
    fn a_processor_without_a_pinned_download_is_a_network_error() {
        let Err(InstallError::Network(why)) = pinned_asset("rust-analyzer-sparc.gz") else {
            panic!("no asset is a network error");
        };
        assert_eq!(
            why,
            format!("release {RUST_ANALYZER_TAG} has no rust-analyzer-sparc.gz")
        );
    }

    #[test]
    fn a_downloaded_server_that_does_not_start_is_not_installed() {
        let dir = tools("unusable");
        let (canned, asset) = served(b"#!/bin/sh\necho 'dyld: missing symbol' >&2\nexit 3\n", None);
        let size = canned.files[&asset.url].len() as u64;
        let installer = Installer::new(&Supervisor::new(), env(), dir.to_path_buf(), Arc::new(canned));
        let mut seen = vec![];
        let result = installer.rust_analyzer(&asset, &Cancel::new(), &mut |p| seen.push(p));
        let Err(InstallError::Unusable(why)) = result else {
            panic!("a broken download is unusable: {result:?}");
        };
        assert_eq!(why.trim(), "dyld: missing symbol");
        assert!(seen.contains(&Progress::Bytes {
            done: size,
            total: Some(size)
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
