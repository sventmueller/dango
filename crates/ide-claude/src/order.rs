//! The bounds a Claude session of a work order runs in (docs/phase-13.md, E173, E191): allow and deny rules instead
//! of questions, every shell command in the sandbox, reads and writes kept to the worktree and the build folder, no
//! MCP servers, no web tools and no switch to bypassing the rules.
//!
//! The settings keys are those of Claude Code 2.1.325, read from its settings schema: the `permissions` keys
//! `blockReadsOutsideWorkingDirectories`, `blockWritesOutsideWorkingDirectories`, `blockReadsExemptPaths` (only
//! from `--settings`) and `disableBypassPermissionsMode`, and the `sandbox` keys `enabled`, `failIfUnavailable`,
//! `allowUnsandboxedCommands`, `autoAllowBashIfSandboxed`, `network.allowedDomains` and `filesystem`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Git commands that reach beyond an order's worktree: refs and branches the main repository shares with it, its
/// stash and configuration, other worktrees, other repositories and remotes. Each becomes a `Bash(…:*)` deny rule.
pub const BEYOND_WORKTREE: [&str; 27] = [
    "git push",
    "git fetch",
    "git pull",
    "git update-ref",
    "git symbolic-ref",
    "git branch -d",
    "git branch -D",
    "git branch --delete",
    "git branch -f",
    "git branch --force",
    "git branch -m",
    "git branch -M",
    "git branch --move",
    "git tag",
    "git stash",
    "git worktree",
    "git remote",
    "git config",
    "git gc",
    "git prune",
    "git reflog",
    "git replace",
    "git notes",
    "git filter-branch",
    "git -C",
    "git --git-dir",
    "git --work-tree",
];

/// Tools that reach the network past the sandbox, which no order session gets (E191).
const WEB_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];

/// The file tools that write, which a reader never gets.
const WRITING_TOOLS: [&str; 3] = ["Edit", "Write", "NotebookEdit"];

/// Where an order session may read, write and connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderBounds {
    /// Whether the session builds: it edits files in the worktree and the build folder. A planner, reviewer or scout
    /// only reads, and its commands write nowhere in either.
    pub builds: bool,
    /// Whether a call outside the rules asks through the app's permission prompt instead of being refused by the
    /// CLI itself: without the owner's decision O1, such requests go to the user (C72).
    pub asks: bool,
    /// The order's worktree, which is the session's working folder.
    pub worktree: PathBuf,
    /// The order's build folder, a working folder besides the worktree.
    pub build_dir: Option<PathBuf>,
    /// Folders that may be read besides the worktree and the build folder, such as toolchains.
    pub read_roots: Vec<PathBuf>,
    /// The domains sandboxed commands may reach; none means no network.
    pub allowed_domains: Vec<String>,
}

/// `path` as the absolute path of a permission rule, which starts with `//`, with everything below it.
fn rule_path(path: &Path) -> String {
    format!("/{}/**", path.display())
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl OrderBounds {
    /// The permission mode: refusing what the rules do not allow, or asking the app.
    pub(crate) fn permission_mode(&self) -> &'static str {
        if self.asks { "default" } else { "dontAsk" }
    }

    /// The folders the session may use besides its working folder.
    pub(crate) fn extra_dirs(&self) -> Vec<PathBuf> {
        self.build_dir.iter().cloned().collect()
    }

    fn folders(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&self.worktree).chain(&self.build_dir)
    }

    /// The tools that run without asking: reading in the worktree and the build folder, for a builder editing there
    /// too, and shell commands, which all run in the sandbox.
    pub(crate) fn allowed_tools(&self) -> Vec<String> {
        let mut rules: Vec<String> = self.folders().map(|f| format!("Read({})", rule_path(f))).collect();
        if self.builds {
            rules.extend(self.folders().map(|f| format!("Edit({})", rule_path(f))));
        }
        rules.push("Bash".into());
        rules
    }

    /// The tools that never run: git beyond the worktree, the web tools, and for a reader the writing file tools.
    pub(crate) fn disallowed_tools(&self) -> Vec<String> {
        let mut rules: Vec<String> = BEYOND_WORKTREE.iter().map(|c| format!("Bash({c}:*)")).collect();
        rules.extend(WEB_TOOLS.iter().map(|t| (*t).to_string()));
        if !self.builds {
            rules.extend(WRITING_TOOLS.iter().map(|t| (*t).to_string()));
        }
        rules
    }

    /// The settings handed over with `--settings`. Both read blocks are set: the file tools' block names
    /// `blockReadsOutsideWorkingDirectories`, and the sandbox's `denyRead` of the home folder holds for commands
    /// whatever that block leaves open (A21).
    pub(crate) fn settings(&self) -> Value {
        let mut readable: Vec<String> = self.folders().map(|f| shown(f)).collect();
        readable.extend(self.read_roots.iter().map(|f| shown(f)));
        let mut filesystem = json!({"denyRead": ["~"], "allowRead": readable});
        if self.builds {
            filesystem["allowWrite"] = json!(self.build_dir.iter().map(|f| shown(f)).collect::<Vec<_>>());
        } else {
            filesystem["denyWrite"] = json!(self.folders().map(|f| shown(f)).collect::<Vec<_>>());
        }
        json!({
            "permissions": {
                "blockReadsOutsideWorkingDirectories": true,
                "blockWritesOutsideWorkingDirectories": true,
                "blockReadsExemptPaths": self.read_roots.iter().map(|f| shown(f)).collect::<Vec<_>>(),
                "disableBypassPermissionsMode": "disable",
            },
            "sandbox": {
                "enabled": true,
                "failIfUnavailable": true,
                "allowUnsandboxedCommands": false,
                "autoAllowBashIfSandboxed": true,
                "network": {"allowedDomains": self.allowed_domains},
                "filesystem": filesystem,
            },
        })
    }
}
