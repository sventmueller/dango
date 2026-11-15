//! Temporary repositories for the tests, kept apart from the user's git configuration unless a test asks for it.

#![allow(dead_code, reason = "each test file uses a different part")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use ide_git::{Git, Repository};
use ide_process::Supervisor;
use ide_process::shell_env::{capture, user_shell};

/// A folder of its own for one test, deleted at the end.
pub(crate) struct Scratch {
    pub dir: PathBuf,
    _folder: ide_core::testing::Scratch,
    /// The login shell's environment, for a test of the user's own git and configuration.
    users: Option<Vec<(String, String)>>,
    program: PathBuf,
}

impl Scratch {
    pub(crate) fn new(name: &str) -> Scratch {
        let folder = ide_core::testing::scratch(&format!("git-test-{name}"));
        std::fs::create_dir_all(folder.join("home")).unwrap();
        Scratch {
            dir: folder.to_path_buf(),
            _folder: folder,
            users: None,
            program: PathBuf::from("/usr/bin/git"),
        }
    }

    /// A folder whose git is the user's own: the git on the login shell's PATH, with their HOME and global
    /// configuration, as the app runs it.
    pub(crate) fn with_users_git(name: &str) -> Scratch {
        let mut scratch = Scratch::new(name);
        let env = capture(&Supervisor::new(), &user_shell(), &scratch.dir, Duration::from_secs(30))
            .expect("the login shell's environment");
        scratch.program = env
            .iter()
            .filter(|(key, _)| key == "PATH")
            .flat_map(|(_, path)| path.split(':'))
            .map(|dir| Path::new(dir).join("git"))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| PathBuf::from("/usr/bin/git"));
        scratch.users = Some(env);
        scratch
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The environment the app would pass, without the user's or the system's git configuration.
    pub(crate) fn environment(&self) -> Vec<(String, String)> {
        if let Some(env) = &self.users {
            return env.clone();
        }
        vec![
            ("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into()),
            ("HOME".into(), self.path("home").to_string_lossy().into_owned()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ]
    }

    pub(crate) fn git(&self) -> Git {
        Git::new(&Supervisor::new(), self.environment())
    }

    /// A new repository on `main` with a user, and a first commit of `files` unless there are none.
    pub(crate) fn repository(&self, name: &str, files: &[(&str, &str)]) -> Repository {
        let root = self.path(name);
        std::fs::create_dir_all(&root).unwrap();
        self.raw(&root, &["init", "--quiet", "--initial-branch=main"]);
        self.raw(&root, &["config", "user.name", "Test"]);
        self.raw(&root, &["config", "user.email", "test@example.org"]);
        for (path, content) in files {
            write(&root, path, content);
        }
        if !files.is_empty() {
            self.raw(&root, &["add", "--all"]);
            self.raw(&root, &["commit", "--quiet", "-m", "first"]);
        }
        self.git().discover(&root).unwrap()
    }

    /// Runs git directly, for setting up what the test then looks at; panics when git fails.
    pub(crate) fn raw(&self, dir: &Path, args: &[&str]) -> String {
        let output = Command::new(&self.program)
            .args(args)
            .current_dir(dir)
            .env_clear()
            .envs(self.environment())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Like `raw`, but returns whether git succeeded.
    pub(crate) fn try_raw(&self, dir: &Path, args: &[&str]) -> bool {
        Command::new(&self.program)
            .args(args)
            .current_dir(dir)
            .env_clear()
            .envs(self.environment())
            .output()
            .unwrap()
            .status
            .success()
    }
}

pub(crate) fn write(root: &Path, path: &str, content: &str) {
    write_bytes(root, path, content.as_bytes());
}

pub(crate) fn write_bytes(root: &Path, path: &str, content: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

pub(crate) fn read(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path)).unwrap()
}

/// Lines `1` to `n`, one per line.
pub(crate) fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}
