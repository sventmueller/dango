//! The bounds a Claude session of a work order runs in (docs/phase-13.md, E173, E191): allow and deny rules instead
//! of questions, every shell command in the sandbox, reads and writes kept to the worktree and the build folder, no
//! MCP servers, no web tools and no switch to bypassing the rules.
//!
//! The settings keys are those of Claude Code 2.1.325, read from its settings schema: the `permissions` keys
//! `blockReadsOutsideWorkingDirectories`, `blockWritesOutsideWorkingDirectories`, `blockReadsExemptPaths` (only
//! from `--settings`) and `disableBypassPermissionsMode`, and the `sandbox` keys `enabled`, `failIfUnavailable`,
//! `allowUnsandboxedCommands`, `autoAllowBashIfSandboxed`, `network.allowedDomains` and `filesystem`.
//!
//! The project's configuration comes from the order's frozen copy only (E172, C57): `--setting-sources=user` keeps
//! the worktree's settings, `CLAUDE.md`, `CLAUDE.local.md`, rules, skills and hooks out, and the frozen folder joins
//! through `--add-dir` with `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD`, which loads its `CLAUDE.md`,
//! `.claude/CLAUDE.md` and `.claude/rules/` but not its `CLAUDE.local.md`, a file of the excluded source `local`.

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

/// Git commands that discard only the worktree's own work, which the checkpoint before each turn brings back
/// (E173, C47): allowed with the owner's decision O1, asked otherwise.
pub const OWN_WORKTREE_DISCARDS: [&str; 2] = ["git reset --hard", "git clean"];

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
    /// The agent configuration frozen from the user's working copy (E172): its `CLAUDE.md` files are the only ones
    /// the session loads, and nothing of the session writes there.
    pub frozen: Option<PathBuf>,
    /// The frozen project settings, `.claude/settings.json` with `.claude/settings.local.json` over it, as read from
    /// `frozen`; only their deny and ask rules carry over, since everything else could loosen the order's bounds.
    pub frozen_settings: Option<Value>,
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

    /// The folders the session may use besides its working folder: the build folder, and the frozen configuration
    /// for its `CLAUDE.md` files.
    pub(crate) fn extra_dirs(&self) -> Vec<PathBuf> {
        self.build_dir.iter().chain(&self.frozen).cloned().collect()
    }

    /// The setting sources the CLI loads from files: the user's only, so no settings, `CLAUDE.md`, rules, skills or
    /// hooks come from the worktree, where the builder writes (E172, C57). The frozen ones come through `--add-dir`.
    pub(crate) fn setting_sources(&self) -> &'static str {
        "user"
    }

    fn folders(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&self.worktree).chain(&self.build_dir)
    }

    /// The tools that run without asking: reading in the worktree and the build folder, for a builder editing there
    /// too, and shell commands, which all run in the sandbox.
    pub(crate) fn allowed_tools(&self) -> Vec<String> {
        let mut rules: Vec<String> = self.folders().map(|f| format!("Read({})", rule_path(f))).collect();
        rules.extend(self.frozen.iter().map(|f| format!("Read({})", rule_path(f))));
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
        rules.extend(self.frozen.iter().map(|f| format!("Edit({})", rule_path(f))));
        rules
    }

    /// The rules that ask instead of allowing: without O1, git that discards the worktree's own work goes to the
    /// user (E173, C47); with it, those commands run, the checkpoint covering them.
    fn ask_rules(&self) -> Vec<String> {
        if !self.asks {
            return Vec::new();
        }
        OWN_WORKTREE_DISCARDS.iter().map(|c| format!("Bash({c}:*)")).collect()
    }

    /// The settings handed over with `--settings`. Both read blocks are set: the file tools' block names
    /// `blockReadsOutsideWorkingDirectories`, and the sandbox's `denyRead` of the home folder holds for commands
    /// whatever that block leaves open (A21).
    pub(crate) fn settings(&self) -> Value {
        let mut readable: Vec<String> = self.folders().map(|f| shown(f)).collect();
        readable.extend(self.read_roots.iter().chain(&self.frozen).map(|f| shown(f)));
        let mut unreadable = vec!["~".to_string()];
        unreadable.extend(credentials());
        let mut filesystem = json!({"denyRead": unreadable, "allowRead": readable});
        let frozen = self.frozen.iter().map(|f| shown(f));
        if self.builds {
            filesystem["allowWrite"] = json!(self.build_dir.iter().map(|f| shown(f)).collect::<Vec<_>>());
            if self.frozen.is_some() {
                filesystem["denyWrite"] = json!(frozen.collect::<Vec<_>>());
            }
        } else {
            filesystem["denyWrite"] = json!(self.folders().map(|f| shown(f)).chain(frozen).collect::<Vec<_>>());
        }
        let (mut deny, mut ask) = carried(self.frozen_settings.as_ref());
        deny.extend(credentials().map(|file| format!("Read({file})")));
        ask.extend(self.ask_rules());
        json!({
            "permissions": {
                "blockReadsOutsideWorkingDirectories": true,
                "blockWritesOutsideWorkingDirectories": true,
                "blockReadsExemptPaths": self.read_roots.iter().map(|f| shown(f)).collect::<Vec<_>>(),
                "disableBypassPermissionsMode": "disable",
                "deny": deny,
                "ask": ask,
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

/// The credential files in the toolchains' folders, which stay unreadable when those folders are read roots; a deny
/// holds inside a wider allow in both the sandbox and the permission rules.
fn credentials() -> impl Iterator<Item = String> {
    ide_process::sandbox::CREDENTIAL_FILES
        .iter()
        .map(|file| format!("~/{file}"))
}

/// The deny and ask rules of the frozen project settings, which only tighten.
fn carried(settings: Option<&Value>) -> (Vec<String>, Vec<String>) {
    let rules = |key: &str| -> Vec<String> {
        settings
            .and_then(|s| s["permissions"][key].as_array())
            .map(|list| list.iter().filter_map(|r| r.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    };
    (rules("deny"), rules("ask"))
}
