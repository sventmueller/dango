//! Installing vscode-js-debug into the tools folder (E93, R16): one pinned release, its archive checked against the
//! digest written here before anything of it is unpacked.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_lsp::{Cancel, InstallError, Net, Progress, Step};
use sha2::{Digest, Sha256};

use crate::discover::Adapters;
use crate::helper::Helper;

/// A pinned release of an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pinned {
    /// Its version.
    pub version: &'static str,
    /// Where its archive is downloaded from.
    pub url: &'static str,
    /// The SHA-256 of the archive, in lowercase hex.
    pub sha256: &'static str,
}

/// The js-debug release the app installs; a new one is pinned by hand after checking its digest.
pub const JS_DEBUG: Pinned = Pinned {
    version: "1.140.1",
    url: "https://github.com/microsoft/vscode-js-debug/releases/download/v1.140.1/js-debug-dap-v1.140.1.tar.gz",
    sha256: "c72fab31bcbfdce6ad1f42adfd28eff8d4681dd3b03b5e2e6f9e72e6f9927faf",
};

/// Where the pinned js-debug's DAP server lives in the tools folder.
pub(crate) fn js_debug_server(tools: &Path) -> PathBuf {
    js_debug_dir(tools).join("js-debug/src/dapDebugServer.js")
}

fn js_debug_dir(tools: &Path) -> PathBuf {
    tools.join(format!("js-debug-{}", JS_DEBUG.version))
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
    let server = js_debug_server(adapters.tools());
    if server.is_file() {
        progress(Progress::Step(Step::Done));
        return Ok(server);
    }
    if crate::helper::which("node", adapters.path()).is_none() {
        return Err(InstallError::Missing("node"));
    }
    fs::create_dir_all(adapters.tools())?;
    let id = std::process::id();
    let archive = Scratch(adapters.tools().join(format!(".js-debug-{id}.tar.gz")));
    progress(Progress::Step(Step::Downloading));
    let actual = download(net, JS_DEBUG.url, &archive.0, cancel, progress)?;
    progress(Progress::Step(Step::Verifying));
    if actual != JS_DEBUG.sha256 {
        return Err(InstallError::Checksum {
            expected: JS_DEBUG.sha256.to_string(),
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
    let output = helper.run(
        Path::new("/usr/bin/tar"),
        &["-xzf", &archive_path],
        &unpacked.0,
        &mut |_| {},
    )?;
    if cancel.is_cancelled() {
        return Err(InstallError::Cancelled);
    }
    if !output.succeeded() {
        return Err(InstallError::Failed {
            program: "tar".into(),
            ending: output.ending,
            output: output.stderr,
        });
    }
    if !unpacked.0.join("js-debug/src/dapDebugServer.js").is_file() {
        return Err(InstallError::Unusable(
            "the archive has no js-debug/src/dapDebugServer.js".into(),
        ));
    }
    let target = js_debug_dir(adapters.tools());
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
    let mut download = net
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

#[cfg(test)]
mod tests {
    use std::io;

    use ide_lsp::Download;
    use ide_process::Supervisor;

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

    #[test]
    fn a_download_that_does_not_match_the_pinned_digest_is_refused_and_removed() {
        let tools = std::env::temp_dir().join(format!("ide-dap-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tools);
        let node = tools.join("bin/node");
        fs::create_dir_all(node.parent().unwrap()).unwrap();
        fs::write(&node, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&node, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let env = vec![("PATH".to_string(), tools.join("bin").display().to_string())];
        let adapters = Adapters::with_tools(&Supervisor::new(), env, tools.clone());
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
