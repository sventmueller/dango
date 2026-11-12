//! Finding a debug adapter with the login shell's environment (E93): lldb-dap through `xcrun`, `dlv` on the PATH or
//! in Go's bin folder, js-debug in the app's tools folder, debugpy in the project's Python.
//!
//! Only for trusted folders: the project's Python and the adapters run code the project controls.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_process::Supervisor;

use crate::adapter::{Adapter, AdapterKind, free_port};
use crate::helper::{self, Helper};
use crate::install::{self, JS_DEBUG};

/// How long a helper such as `xcrun --find` or `go env` may take.
const HELPER_TIMEOUT: Duration = Duration::from_secs(20);

/// What the app can install itself when an adapter is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// vscode-js-debug in its pinned version, checked against its digest, into the tools folder.
    JsDebug,
}

/// No adapter was found; what the status bar says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// For which adapter.
    pub kind: AdapterKind,
    /// The adapter's program as the user knows it, such as `dlv`.
    pub program: &'static str,
    /// A program it needs that the login shell's PATH lacks, such as `node`; install that first.
    pub needs: Option<&'static str>,
    /// What the app can install itself, after the user agreed.
    pub install: Option<Install>,
    /// The command the user installs it with, when the app cannot.
    pub command: Option<String>,
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not installed", self.program)?;
        if let Some(needs) = self.needs {
            write!(f, "; it needs {needs}")?;
        }
        if let Some(command) = &self.command {
            write!(f, "; install it with: {command}")?;
        }
        Ok(())
    }
}

/// Finds and installs adapters with the login shell's environment, never the app's own.
pub struct Adapters {
    supervisor: Supervisor,
    env: Vec<(String, String)>,
    tools: PathBuf,
}

impl Adapters {
    /// Adapters found with `env`, the login shell's environment, and the app's tools folder.
    pub fn new(supervisor: &Supervisor, env: Vec<(String, String)>) -> Adapters {
        Adapters::with_tools(supervisor, env, ide_lsp::default_tools())
    }

    /// Adapters with another tools folder, for tests.
    pub fn with_tools(supervisor: &Supervisor, env: Vec<(String, String)>, tools: PathBuf) -> Adapters {
        Adapters {
            supervisor: supervisor.clone(),
            env,
            tools,
        }
    }

    /// The login shell's environment the adapters get.
    pub fn env(&self) -> &[(String, String)] {
        &self.env
    }

    /// The supervisor that starts them.
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    /// The tools folder.
    pub fn tools(&self) -> &Path {
        &self.tools
    }

    /// The adapter of `kind` for the project at `root`, ready to start; blocks while helpers run, so call it off
    /// the main thread. The caller has decided that `root` is trusted.
    pub fn find(&self, kind: AdapterKind, root: &Path) -> Result<Adapter, Missing> {
        match kind {
            AdapterKind::Lldb => self.lldb(root),
            AdapterKind::Go => self.delve(root),
            AdapterKind::JsDebug => self.js_debug(root),
            AdapterKind::Python => self.debugpy(root),
            AdapterKind::Fake => Ok(Adapter::fake(crate::fake::Script::default())),
        }
    }

    /// Installs `what` into the tools folder and returns the adapter's main file; blocks for a while, so call it
    /// off the main thread.
    pub fn install(
        &self,
        what: Install,
        net: &dyn ide_lsp::Net,
        cancel: &ide_lsp::Cancel,
        progress: &mut dyn FnMut(ide_lsp::Progress),
    ) -> Result<PathBuf, ide_lsp::InstallError> {
        match what {
            Install::JsDebug => install::js_debug(self, net, cancel, progress),
        }
    }

    pub(crate) fn helper(&self) -> Helper<'_> {
        Helper {
            supervisor: &self.supervisor,
            env: &self.env,
            timeout: HELPER_TIMEOUT,
            cancel: None,
        }
    }

    pub(crate) fn path(&self) -> Option<&str> {
        helper::var(&self.env, "PATH")
    }

    fn home(&self) -> PathBuf {
        helper::var(&self.env, "HOME").map_or_else(ide_core::paths::home, PathBuf::from)
    }

    /// lldb-dap of the selected developer tools; `/usr/bin` only holds stand-ins that offer to install them.
    fn lldb(&self, root: &Path) -> Result<Adapter, Missing> {
        let home = self.home();
        let found = self
            .helper()
            .run(Path::new("/usr/bin/xcode-select"), &["-p"], &home, &mut |_| {})
            .ok()
            .filter(helper::Output::succeeded)
            .and_then(|_| {
                self.helper()
                    .run(Path::new("/usr/bin/xcrun"), &["--find", "lldb-dap"], &home, &mut |_| {})
                    .ok()
            })
            .filter(helper::Output::succeeded)
            .map(|output| PathBuf::from(output.stdout.trim()))
            .filter(|program| !program.starts_with("/usr/bin") && helper::is_executable(program));
        match found {
            Some(program) => Ok(Adapter::command(
                AdapterKind::Lldb,
                &program,
                &[],
                self.env.clone(),
                root,
            )),
            None => Err(Missing {
                kind: AdapterKind::Lldb,
                program: "lldb-dap",
                needs: None,
                install: None,
                command: Some("xcode-select --install".into()),
            }),
        }
    }

    /// `dlv` on the PATH or in Go's bin folder, listening for the session on a port of its own.
    fn delve(&self, root: &Path) -> Result<Adapter, Missing> {
        let program = helper::which("dlv", self.path()).or_else(|| {
            let go = helper::which("go", self.path())?;
            let output = self
                .helper()
                .run(&go, &["env", "GOPATH"], &self.home(), &mut |_| {})
                .ok()?;
            let first = output.stdout.trim().split(':').next()?.to_string();
            let dlv = Path::new(&first).join("bin/dlv");
            helper::is_executable(&dlv).then_some(dlv)
        });
        let missing = || Missing {
            kind: AdapterKind::Go,
            program: "dlv",
            needs: helper::which("go", self.path()).is_none().then_some("go"),
            install: None,
            command: Some("go install github.com/go-delve/delve/cmd/dlv@latest".into()),
        };
        let program = program.ok_or_else(missing)?;
        let port = free_port().map_err(|_| missing())?;
        let args = vec!["dap".to_string(), format!("--listen=127.0.0.1:{port}")];
        Ok(Adapter::listening(
            AdapterKind::Go,
            &program,
            &args,
            self.env.clone(),
            root,
            port,
        ))
    }

    /// js-debug's DAP server from the tools folder, run by Node, listening on a port of its own.
    fn js_debug(&self, root: &Path) -> Result<Adapter, Missing> {
        let node = helper::which("node", self.path());
        let server = install::js_debug_server(&self.tools);
        let missing = |needs| Missing {
            kind: AdapterKind::JsDebug,
            program: "js-debug",
            needs,
            install: Some(Install::JsDebug),
            command: None,
        };
        let Some(node) = node else {
            return Err(missing(Some("node")));
        };
        if !server.is_file() {
            return Err(missing(None));
        }
        let port = free_port().map_err(|_| missing(None))?;
        let args = vec![server.display().to_string(), port.to_string(), "127.0.0.1".to_string()];
        let mut adapter = Adapter::listening(AdapterKind::JsDebug, &node, &args, self.env.clone(), root, port);
        adapter.label = format!("js-debug {}", JS_DEBUG.version);
        Ok(adapter)
    }

    /// debugpy's adapter in the project's Python.
    fn debugpy(&self, root: &Path) -> Result<Adapter, Missing> {
        let python = self.python(root);
        let Some(python) = python else {
            return Err(Missing {
                kind: AdapterKind::Python,
                program: "debugpy",
                needs: Some("python3"),
                install: None,
                command: None,
            });
        };
        let has = self
            .helper()
            .run(&python, &["-c", "import debugpy"], root, &mut |_| {})
            .is_ok_and(|o| o.succeeded());
        if !has {
            return Err(Missing {
                kind: AdapterKind::Python,
                program: "debugpy",
                needs: None,
                install: None,
                command: Some(format!("{} -m pip install debugpy", python.display())),
            });
        }
        let args = vec!["-m".to_string(), "debugpy.adapter".to_string()];
        Ok(Adapter::command(
            AdapterKind::Python,
            &python,
            &args,
            self.env.clone(),
            root,
        ))
    }

    /// The project's Python: its virtual environment, the one the login shell activated, or `python3` on the PATH.
    pub fn python(&self, root: &Path) -> Option<PathBuf> {
        let venvs = [".venv", "venv", "env"].map(|name| root.join(name).join("bin/python"));
        let active = helper::var(&self.env, "VIRTUAL_ENV").map(|v| Path::new(v).join("bin/python"));
        venvs
            .into_iter()
            .chain(active)
            .find(|p| helper::is_executable(p))
            .or_else(|| helper::which("python3", self.path()))
            .filter(|p| !is_stub(p) || self.has_developer_tools())
    }

    fn has_developer_tools(&self) -> bool {
        self.helper()
            .run(Path::new("/usr/bin/xcode-select"), &["-p"], &self.home(), &mut |_| {})
            .is_ok_and(|o| o.succeeded())
    }
}

/// `/usr/bin/python3` only stands in for the developer tools and asks to install them when they are missing.
fn is_stub(program: &Path) -> bool {
    program.starts_with("/usr/bin")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(files: &[&str]) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ide-dap-tools-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for file in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
        dir
    }

    #[test]
    fn js_debug_needs_node_and_is_offered_for_installation() {
        let empty = tools(&[]);
        let adapters = Adapters::with_tools(&Supervisor::new(), vec![("PATH".into(), String::new())], empty.clone());
        let missing = adapters.find(AdapterKind::JsDebug, &empty).unwrap_err();
        assert_eq!((missing.needs, missing.install), (Some("node"), Some(Install::JsDebug)));
        let with_node = tools(&["bin/node"]);
        let path = with_node.join("bin").display().to_string();
        let adapters = Adapters::with_tools(&Supervisor::new(), vec![("PATH".into(), path)], with_node.clone());
        let missing = adapters.find(AdapterKind::JsDebug, &with_node).unwrap_err();
        assert_eq!((missing.needs, missing.install), (None, Some(Install::JsDebug)));
    }

    #[test]
    fn dlv_on_the_path_listens_on_a_port_and_a_missing_one_names_its_install_command() {
        let dir = tools(&["bin/dlv", "bin/go"]);
        let path = dir.join("bin").display().to_string();
        let adapters = Adapters::with_tools(&Supervisor::new(), vec![("PATH".into(), path)], dir.clone());
        let adapter = adapters.find(AdapterKind::Go, &dir).unwrap();
        assert_eq!(adapter.program(), Some(dir.join("bin/dlv").as_path()));
        assert!(adapter.args()[1].starts_with("--listen=127.0.0.1:"));
        let none = Adapters::with_tools(&Supervisor::new(), vec![("PATH".into(), String::new())], dir.clone());
        let missing = none.find(AdapterKind::Go, &dir).unwrap_err();
        assert_eq!(missing.needs, Some("go"));
        assert!(missing.command.unwrap().contains("go-delve/delve/cmd/dlv"));
    }

    #[test]
    fn the_projects_virtual_environment_comes_before_the_path() {
        let dir = tools(&[".venv/bin/python", "bin/python3"]);
        let path = dir.join("bin").display().to_string();
        let adapters = Adapters::with_tools(&Supervisor::new(), vec![("PATH".into(), path)], dir.clone());
        assert_eq!(adapters.python(&dir), Some(dir.join(".venv/bin/python")));
    }
}
