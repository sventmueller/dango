//! Installing vscode-js-debug into the tools folder (E93, R16): one pinned release, its archive checked against the
//! digest written here before anything of it is unpacked.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_net::install::{InstallError, Progress, Step};
use ide_net::{Cancel, Net};

use crate::discover::Adapters;
use ide_process::tool::{self, Helper, RunError};

/// A pinned release of an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pinned {
    pub(crate) version: &'static str,
    /// Where its archive is downloaded from.
    pub(crate) url: &'static str,
    /// The SHA-256 of the archive, in lowercase hex.
    pub(crate) sha256: &'static str,
}

/// The js-debug release the app installs; a new one is pinned by hand after checking its digest.
pub(crate) const JS_DEBUG: Pinned = Pinned {
    version: "1.140.1",
    url: "https://github.com/microsoft/vscode-js-debug/releases/download/v1.140.1/js-debug-dap-v1.140.1.tar.gz",
    sha256: "c72fab31bcbfdce6ad1f42adfd28eff8d4681dd3b03b5e2e6f9e72e6f9927faf",
};

/// Where the pinned js-debug's DAP server lives in the tools folder.
pub(crate) fn js_debug_server(tools: &Path) -> PathBuf {
    server_in(&js_debug_dir(tools, &JS_DEBUG))
}

fn js_debug_dir(tools: &Path, release: &Pinned) -> PathBuf {
    tools.join(format!("js-debug-{}", release.version))
}

fn server_in(dir: &Path) -> PathBuf {
    dir.join("js-debug/src/dapDebugServer.js")
}

/// Removes a half-written file or folder when an installation stops early.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.0.is_dir() {
            let _ = fs::remove_dir_all(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
    }
}

pub(crate) fn js_debug(
    adapters: &Adapters,
    net: &dyn Net,
    cancel: &Cancel,
    progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf, InstallError> {
    js_debug_release(adapters, &JS_DEBUG, net, cancel, progress)
}

/// Installs `release` of js-debug and returns its server.
fn js_debug_release(
    adapters: &Adapters,
    release: &Pinned,
    net: &dyn Net,
    cancel: &Cancel,
    progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf, InstallError> {
    let target = js_debug_dir(adapters.tools(), release);
    let server = server_in(&target);
    if server.is_file() {
        progress(Progress::Step(Step::Done));
        return Ok(server);
    }
    if tool::which("node", adapters.path()).is_none() {
        return Err(InstallError::Missing("node"));
    }
    fs::create_dir_all(adapters.tools())?;
    let id = std::process::id();
    let archive = Scratch(adapters.tools().join(format!(".js-debug-{id}.tar.gz")));
    progress(Progress::Step(Step::Downloading));
    let actual = download(net, release.url, &archive.0, cancel, progress)?;
    progress(Progress::Step(Step::Verifying));
    if actual != release.sha256 {
        return Err(InstallError::Checksum {
            expected: release.sha256.to_string(),
            actual,
        });
    }
    progress(Progress::Step(Step::Unpacking));
    let unpacked = Scratch(adapters.tools().join(format!(".js-debug-{id}")));
    fs::create_dir_all(&unpacked.0)?;
    let helper = Helper {
        supervisor: adapters.supervisor(),
        env: adapters.env(),
        timeout: Duration::from_secs(120),
        cancel: Some(cancel),
    };
    let archive_path = archive.0.display().to_string();
    let output = helper
        .run(
            Path::new("/usr/bin/tar"),
            &["-xzf", &archive_path],
            &unpacked.0,
            &mut |_| {},
        )
        .map_err(|error| match error {
            RunError::Cancelled => InstallError::Cancelled,
            RunError::Timeout => InstallError::Failed {
                program: "tar".into(),
                ending: None,
                output: String::new(),
            },
            RunError::Start(error) => InstallError::Io(error),
        })?;
    if cancel.is_cancelled() {
        return Err(InstallError::Cancelled);
    }
    if !output.succeeded() {
        return Err(InstallError::Failed {
            program: "tar".into(),
            ending: Some(output.ending),
            output: output.stderr,
        });
    }
    if !server_in(&unpacked.0).is_file() {
        return Err(InstallError::Unusable(
            "the archive has no js-debug/src/dapDebugServer.js".into(),
        ));
    }
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&unpacked.0, &target)?;
    progress(Progress::Step(Step::Done));
    Ok(server)
}

/// Downloads `url` into `path` and returns the SHA-256 of what arrived, in lowercase hex.
fn download(
    net: &dyn Net,
    url: &str,
    path: &Path,
    cancel: &Cancel,
    progress: &mut dyn FnMut(Progress),
) -> Result<String, InstallError> {
    Ok(ide_net::download(net, url, path, cancel, None, &mut |done, total| {
        progress(Progress::Bytes { done, total });
    })?)
}

#[cfg(test)]
mod tests {
    use std::io;

    use ide_net::Download;
    use ide_process::Supervisor;
    use sha2::{Digest, Sha256};

    use super::*;

    struct Served(Vec<u8>);

    impl Net for Served {
        fn get(&self, _: &str, _: &str) -> io::Result<Download> {
            Ok(Download {
                length: Some(self.0.len() as u64),
                body: Box::new(io::Cursor::new(self.0.clone())),
            })
        }
    }

    struct Offline;

    impl Net for Offline {
        fn get(&self, url: &str, _: &str) -> io::Result<Download> {
            Err(io::Error::other(format!("{url} is out of reach")))
        }
    }

    /// A tools folder with a stand-in for `node` on the PATH it gives, and the adapters installed into it.
    fn tools_with_node() -> (ide_core::testing::Scratch, Adapters) {
        let tools = crate::stub_tests::folder("install");
        crate::stub_tests::script(&tools.join("bin/node"), "");
        let env = crate::stub_tests::env(&tools.join("bin"), &[]);
        let adapters = Adapters::with_tools(&Supervisor::new(), env, tools.to_path_buf());
        (tools, adapters)
    }

    /// A gzipped tar archive of `files`, made by the system's tar as js-debug's release is.
    fn archive(files: &[&str]) -> Vec<u8> {
        let dir = crate::stub_tests::folder("archive");
        for file in files {
            let path = dir.join("content").join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "// js-debug").unwrap();
        }
        fs::create_dir_all(dir.join("content")).unwrap();
        let made = std::process::Command::new("/usr/bin/tar")
            .args(["-czf", "../release.tar.gz", "."])
            .current_dir(dir.join("content"))
            .status()
            .unwrap();
        assert!(made.success());
        fs::read(dir.join("release.tar.gz")).unwrap()
    }

    /// The release that `bytes` are, under the pinned version.
    fn release_of(bytes: &[u8]) -> Pinned {
        let digest: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
        Pinned {
            version: JS_DEBUG.version,
            url: "https://example.invalid/js-debug.tar.gz",
            sha256: Box::leak(digest.into_boxed_str()),
        }
    }

    fn scratch_left(tools: &Path) -> Vec<std::ffi::OsString> {
        fs::read_dir(tools)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().starts_with(".js-debug"))
            .collect()
    }

    #[test]
    fn a_release_that_matches_its_digest_is_unpacked_in_place_of_an_older_folder() {
        let (_tools, adapters) = tools_with_node();
        let bytes = archive(&["js-debug/src/dapDebugServer.js"]);
        let release = release_of(&bytes);
        let old = js_debug_dir(adapters.tools(), &release);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("left-over"), "").unwrap();
        let mut steps = Vec::new();
        let server = js_debug_release(&adapters, &release, &Served(bytes.clone()), &Cancel::new(), &mut |p| {
            steps.push(p);
        })
        .unwrap();
        assert_eq!(server, js_debug_server(adapters.tools()));
        assert!(server.is_file());
        assert!(!old.join("left-over").exists());
        let size = bytes.len() as u64;
        assert_eq!(
            steps,
            [
                Progress::Step(Step::Downloading),
                Progress::Bytes {
                    done: size,
                    total: Some(size)
                },
                Progress::Step(Step::Verifying),
                Progress::Step(Step::Unpacking),
                Progress::Step(Step::Done),
            ]
        );
        assert!(scratch_left(adapters.tools()).is_empty());

        let mut again = Vec::new();
        let installed = js_debug(&adapters, &Offline, &Cancel::new(), &mut |p| again.push(p)).unwrap();
        assert_eq!((installed, again), (server, vec![Progress::Step(Step::Done)]));
    }

    #[test]
    fn an_archive_without_the_server_or_that_tar_cannot_read_is_refused_and_removed() {
        let (_tools, adapters) = tools_with_node();
        let bytes = archive(&["README.md"]);
        let result = js_debug_release(
            &adapters,
            &release_of(&bytes),
            &Served(bytes),
            &Cancel::new(),
            &mut |_| {},
        );
        assert!(
            matches!(result, Err(InstallError::Unusable(ref why)) if why.contains("dapDebugServer.js")),
            "{result:?}"
        );

        let garbage = b"not a tar archive".to_vec();
        let result = js_debug_release(
            &adapters,
            &release_of(&garbage),
            &Served(garbage),
            &Cancel::new(),
            &mut |_| {},
        );
        assert!(
            matches!(result, Err(InstallError::Failed { ref program, .. }) if program == "tar"),
            "{result:?}"
        );
        assert!(scratch_left(adapters.tools()).is_empty());
        assert!(!js_debug_server(adapters.tools()).exists());
    }

    #[test]
    fn without_node_the_network_or_with_a_cancel_nothing_is_installed() {
        let tools = crate::stub_tests::folder("install");
        let bare = Adapters::with_tools(&Supervisor::new(), Vec::new(), tools.to_path_buf());
        let result = js_debug(&bare, &Offline, &Cancel::new(), &mut |_| {});
        assert!(matches!(result, Err(InstallError::Missing("node"))), "{result:?}");

        let (_tools, adapters) = tools_with_node();
        let result = adapters.install(crate::discover::Install::JsDebug, &Offline, &Cancel::new(), &mut |_| {});
        assert!(
            matches!(result, Err(InstallError::Network(ref why)) if why.contains("out of reach")),
            "{result:?}"
        );

        let cancel = Cancel::new();
        cancel.cancel();
        let result = js_debug(&adapters, &Served(b"bytes".to_vec()), &cancel, &mut |_| {});
        assert!(matches!(result, Err(InstallError::Cancelled)), "{result:?}");
        assert!(scratch_left(adapters.tools()).is_empty());
    }

    #[test]
    fn a_download_that_does_not_match_the_pinned_digest_is_refused_and_removed() {
        let tools = ide_core::testing::scratch("dap-install");
        let node = tools.join("bin/node");
        fs::create_dir_all(node.parent().unwrap()).unwrap();
        fs::write(&node, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&node, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let env = vec![("PATH".to_string(), tools.join("bin").display().to_string())];
        let adapters = Adapters::with_tools(&Supervisor::new(), env, tools.to_path_buf());
        let mut steps = Vec::new();
        let result = js_debug(
            &adapters,
            &Served(b"not the archive".to_vec()),
            &Cancel::new(),
            &mut |p| {
                steps.push(p);
            },
        );
        assert!(matches!(result, Err(InstallError::Checksum { .. })), "{result:?}");
        assert!(steps.contains(&Progress::Step(Step::Verifying)));
        let left: Vec<_> = fs::read_dir(&tools)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().starts_with(".js-debug"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        let _ = fs::remove_dir_all(&tools);
    }
}
