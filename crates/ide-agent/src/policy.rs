//! Which calls run on their own, which ask the user first and which are refused (E27, E19): by the thread's profile
//! and approval mode, the rules the user stored for the project, and the sandbox. Destructive git commands always
//! ask, whatever the mode or the rules.

pub use ide_thread::shell::{Destructive, destructive_git};
use ide_thread::shell::{segments, words};
use serde::{Deserialize, Serialize};

/// Which tools the model is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Tools that read: files, search, diagnostics, the web. "Nur fragen".
    Ask,
    /// Also editing files and running commands. "Schreiben".
    Write,
}

/// How much runs without asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    /// Everything but reading in the project asks; new threads start here.
    Manual,
    /// Edits in the project run on their own; commands and web requests ask.
    Edits,
    /// Everything runs on their own, except destructive git commands.
    All,
}

/// A permission the user stored for a project.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Rule {
    /// Commands that start with these words, each part of a chained command alike.
    Command {
        /// The words, such as `cargo test`.
        prefix: String,
    },
    /// Edits and writes in the project.
    Edits,
    /// Web requests to this host.
    Fetch {
        /// The host, such as `docs.rs`.
        host: String,
    },
}

/// What a call does, as far as the decision needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    /// Reads at a path; `inside` when it lies in the project or an allowed folder.
    Read {
        /// The path as the user reads it.
        path: String,
        /// Whether it lies in the project or an allowed folder.
        inside: bool,
    },
    /// Writes at a path.
    Write {
        /// The path as the user reads it.
        path: String,
        /// Whether it lies in the project or an allowed folder.
        inside: bool,
    },
    /// Runs a command line.
    Command(String),
    /// Runs a command line whose only destructive git discards the work of a checkpointed workspace, which the
    /// checkpoint brings back (E173, C47).
    Discard(String),
    /// Requests a web page.
    Fetch {
        /// The page's host.
        host: String,
    },
    /// Changes nothing outside the thread, such as the to-do list.
    Internal,
}

/// What happens to a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// It runs.
    Allow,
    /// The user decides first.
    Ask {
        /// The rule the user may store instead of answering each time; none for calls that always ask.
        rule: Option<Rule>,
        /// Why it asks, beyond the call itself.
        reason: Option<Reason>,
        /// The path outside the project that made it ask.
        blocked_path: Option<String>,
        /// Whether no answer may be preselected as allowing.
        risky: bool,
    },
    /// It is refused, for this reason.
    Deny(Refusal),
}

/// Why a call is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// The thread may only read.
    ReadOnly,
    /// The thread's sandbox has no network.
    NoNetwork,
    /// The path lies outside the project and the folders the thread's sandbox allows.
    OutsideSandbox(String),
}

impl Refusal {
    /// What the model reads.
    pub(crate) fn for_model(&self) -> String {
        match self {
            Refusal::ReadOnly => "this thread may only read".into(),
            Refusal::NoNetwork => "the thread's sandbox has no network".into(),
            Refusal::OutsideSandbox(path) => {
                format!("{path} lies outside the project and the folders the thread's sandbox allows")
            }
        }
    }
}

/// Why a call asks, beyond the call itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The path lies outside the project.
    Outside,
    /// The command destroys work that git cannot bring back.
    Destructive(Destructive),
}

/// The thread's settings that decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// The tools offered.
    pub profile: Profile,
    /// How much runs without asking.
    pub approval: Approval,
    /// Whether the thread's sandbox is on: no paths beyond the project and the allowed folders, no network.
    pub sandbox: bool,
    /// The rules stored for the project.
    pub rules: Vec<Rule>,
}

impl Policy {
    /// The policy of a work order's session (docs/phase-13.md, E173): the sandbox on, so what lies outside the
    /// project and its allowed folders and the network are refused, and everything else runs without asking but
    /// destructive git commands; a builder writes, a reader gets the tools of "Nur fragen". No stored rule applies.
    pub fn order(builds: bool) -> Policy {
        Policy {
            profile: if builds { Profile::Write } else { Profile::Ask },
            approval: Approval::All,
            sandbox: true,
            rules: Vec::new(),
        }
    }

    /// What happens to a call that does `action`.
    pub(crate) fn decide(&self, action: &Action) -> Verdict {
        match action {
            Action::Internal => Verdict::Allow,
            Action::Read { inside: true, .. } => Verdict::Allow,
            Action::Read { path, inside: false } => self.outside(path, None),
            Action::Write { path, inside } => {
                if self.profile == Profile::Ask {
                    return Verdict::Deny(Refusal::ReadOnly);
                }
                if !inside {
                    return self.outside(path, None);
                }
                if self.approval != Approval::Manual || self.rules.contains(&Rule::Edits) {
                    Verdict::Allow
                } else {
                    ask(Some(Rule::Edits), None, None, false)
                }
            }
            Action::Command(command) => self.command(command),
            Action::Discard(command) => self.discard(command),
            Action::Fetch { host } => {
                if self.sandbox {
                    return Verdict::Deny(Refusal::NoNetwork);
                }
                let stored = self.rules.contains(&Rule::Fetch { host: host.clone() });
                if self.approval == Approval::All || stored {
                    Verdict::Allow
                } else {
                    ask(Some(Rule::Fetch { host: host.clone() }), None, None, false)
                }
            }
        }
    }

    fn outside(&self, path: &str, rule: Option<Rule>) -> Verdict {
        if self.sandbox {
            return Verdict::Deny(Refusal::OutsideSandbox(path.to_string()));
        }
        if self.approval == Approval::All {
            return Verdict::Allow;
        }
        ask(rule, Some(Reason::Outside), Some(path.to_string()), false)
    }

    fn command(&self, command: &str) -> Verdict {
        if self.profile == Profile::Ask {
            return Verdict::Deny(Refusal::ReadOnly);
        }
        if let Some(why) = destructive_git(command) {
            return ask(None, Some(Reason::Destructive(why)), None, true);
        }
        self.harmless(command)
    }

    /// A command that discards only the checkpointed workspace's own work: it runs where everything runs on its own,
    /// as in a work order; elsewhere it asks as any destructive command does.
    fn discard(&self, command: &str) -> Verdict {
        if self.profile == Profile::Ask {
            return Verdict::Deny(Refusal::ReadOnly);
        }
        if self.approval == Approval::All {
            return Verdict::Allow;
        }
        self.command(command)
    }

    /// A command that destroys nothing git cannot bring back.
    fn harmless(&self, command: &str) -> Verdict {
        if self.approval == Approval::All {
            return Verdict::Allow;
        }
        let prefixes: Vec<&str> = self
            .rules
            .iter()
            .filter_map(|rule| match rule {
                Rule::Command { prefix } => Some(prefix.as_str()),
                _ => None,
            })
            .collect();
        let parts = segments(command);
        let covered = !parts.is_empty()
            && parts.iter().all(|part| {
                let words = words(part);
                prefixes.iter().any(|prefix| starts_with_words(&words, prefix))
            });
        if covered {
            Verdict::Allow
        } else {
            ask(suggested_rule(command), None, None, false)
        }
    }
}

/// Whether every destructive part of `command` is a `git reset --hard` or `git clean` of the folder it runs in: git
/// named directly, not through a shell or `eval`, pointed at no other repository or tree, and no part changing the
/// folder first. False when nothing in it is destructive.
pub(crate) fn discards_only_own_work(command: &str) -> bool {
    let parts = segments(command);
    let moves = parts.iter().any(|part| {
        words(part)
            .first()
            .is_some_and(|w| matches!(w.as_str(), "cd" | "pushd"))
    });
    let destructive: Vec<&String> = parts.iter().filter(|part| destructive_git(part).is_some()).collect();
    !moves
        && !destructive.is_empty()
        && destructive.iter().all(|part| {
            let words = words(part);
            matches!(destructive_git(part), Some(Destructive::ResetHard | Destructive::Clean))
                && !words.iter().any(|word| elsewhere(word))
        })
}

/// Whether `word` points git at another repository or tree, or hands the command to a shell.
fn elsewhere(word: &str) -> bool {
    let name = word.rsplit('/').next().unwrap_or(word);
    matches!(
        name,
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" | "eval" | "env" | "xargs"
    ) || word == "-C"
        || [
            "--git-dir",
            "--work-tree",
            "GIT_DIR=",
            "GIT_WORK_TREE=",
            "GIT_COMMON_DIR=",
        ]
        .iter()
        .any(|prefix| word.starts_with(prefix))
}

fn ask(rule: Option<Rule>, reason: Option<Reason>, blocked_path: Option<String>, risky: bool) -> Verdict {
    Verdict::Ask {
        rule,
        reason,
        blocked_path,
        risky,
    }
}

/// The rule to offer for `command`: its program and, where the next word is a subcommand, that word too.
fn suggested_rule(command: &str) -> Option<Rule> {
    let parts = segments(command);
    let [part] = parts.as_slice() else {
        return None;
    };
    let words = words(part);
    let program = words.iter().find(|w| !w.contains('='))?;
    let at = words.iter().position(|w| w == program)?;
    let prefix = match words.get(at + 1) {
        Some(sub) if sub.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && !sub.contains('/') => {
            format!("{program} {sub}")
        }
        _ => program.clone(),
    };
    Some(Rule::Command { prefix })
}

fn starts_with_words(words: &[String], prefix: &str) -> bool {
    let prefix: Vec<&str> = prefix.split_whitespace().collect();
    !prefix.is_empty() && words.len() >= prefix.len() && words.iter().zip(&prefix).all(|(w, p)| w == p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(approval: Approval, rules: Vec<Rule>) -> Policy {
        Policy {
            profile: Profile::Write,
            approval,
            sandbox: false,
            rules,
        }
    }

    #[test]
    fn destructive_git_asks_even_when_everything_is_allowed_and_offers_no_rule() {
        let all = policy(
            Approval::All,
            vec![Rule::Command {
                prefix: "git push".into(),
            }],
        );
        let Verdict::Ask { rule, risky, .. } = all.decide(&Action::Command("git push -f".into())) else {
            panic!()
        };
        assert_eq!((rule, risky), (None, true));
        assert_eq!(all.decide(&Action::Command("git push".into())), Verdict::Allow);
    }

    #[test]
    fn a_stored_rule_for_the_shell_does_not_let_a_destructive_command_in_it_through() {
        let stored = vec![
            Rule::Command { prefix: "sh".into() },
            Rule::Command { prefix: "git".into() },
        ];
        for approval in [Approval::Manual, Approval::Edits, Approval::All] {
            let policy = policy(approval, stored.clone());
            for command in ["sh -c 'git push -f'", "git -C x reset --hard", "cd a && git clean -fdx"] {
                assert!(
                    matches!(
                        policy.decide(&Action::Command(command.into())),
                        Verdict::Ask {
                            rule: None,
                            reason: Some(Reason::Destructive(_)),
                            risky: true,
                            ..
                        }
                    ),
                    "{command} under {approval:?}"
                );
            }
        }
    }

    #[test]
    fn dropping_or_clearing_the_stash_asks_every_time_and_can_never_be_stored() {
        let stored = vec![
            Rule::Command {
                prefix: "git stash".into(),
            },
            Rule::Command { prefix: "git".into() },
        ];
        for approval in [Approval::Manual, Approval::Edits, Approval::All] {
            let policy = policy(approval, stored.clone());
            for (command, why) in [
                ("git stash drop stash@{0}", Destructive::StashDrop),
                ("git stash clear", Destructive::StashClear),
            ] {
                assert_eq!(
                    policy.decide(&Action::Command(command.into())),
                    Verdict::Ask {
                        rule: None,
                        reason: Some(Reason::Destructive(why)),
                        blocked_path: None,
                        risky: true,
                    },
                    "{command} under {approval:?}"
                );
            }
        }
        let all = policy(Approval::All, vec![]);
        assert_eq!(all.decide(&Action::Command("git stash push".into())), Verdict::Allow);
    }

    #[test]
    fn a_stored_prefix_allows_only_commands_whose_every_part_it_covers() {
        let manual = policy(
            Approval::Manual,
            vec![Rule::Command {
                prefix: "cargo test".into(),
            }],
        );
        assert_eq!(
            manual.decide(&Action::Command("cargo test -p x".into())),
            Verdict::Allow
        );
        assert_eq!(
            manual.decide(&Action::Command("cargo test && cargo test --doc".into())),
            Verdict::Allow
        );
        for command in [
            "cargo test && rm -rf /",
            "cargo test $(rm -rf /)",
            "cargo testing",
            "cargo",
        ] {
            assert!(
                matches!(manual.decide(&Action::Command(command.into())), Verdict::Ask { .. }),
                "{command}"
            );
        }
    }

    #[test]
    fn the_offered_rule_is_the_program_and_its_subcommand() {
        let manual = policy(Approval::Manual, vec![]);
        let offered = |command: &str| match manual.decide(&Action::Command(command.into())) {
            Verdict::Ask { rule, .. } => rule,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            offered("cargo test -p ide-agent"),
            Some(Rule::Command {
                prefix: "cargo test".into()
            })
        );
        assert_eq!(
            offered("RUST_LOG=1 ls -la"),
            Some(Rule::Command { prefix: "ls".into() })
        );
        assert_eq!(offered("ls && pwd"), None);
    }

    #[test]
    fn modes_profiles_and_the_sandbox_decide_edits_reads_and_fetches() {
        let write = |inside| Action::Write {
            path: "a".into(),
            inside,
        };
        let read_outside = Action::Read {
            path: "/etc/hosts".into(),
            inside: false,
        };
        let fetch = Action::Fetch { host: "docs.rs".into() };
        assert!(matches!(
            policy(Approval::Manual, vec![]).decide(&write(true)),
            Verdict::Ask { .. }
        ));
        assert_eq!(
            policy(Approval::Manual, vec![Rule::Edits]).decide(&write(true)),
            Verdict::Allow
        );
        assert_eq!(policy(Approval::Edits, vec![]).decide(&write(true)), Verdict::Allow);
        assert!(matches!(
            policy(Approval::Edits, vec![]).decide(&write(false)),
            Verdict::Ask { .. }
        ));
        assert!(matches!(
            policy(Approval::Manual, vec![]).decide(&read_outside),
            Verdict::Ask {
                blocked_path: Some(_),
                ..
            }
        ));
        assert!(matches!(
            policy(Approval::Edits, vec![]).decide(&fetch),
            Verdict::Ask { .. }
        ));
        assert_eq!(
            policy(Approval::Manual, vec![Rule::Fetch { host: "docs.rs".into() }]).decide(&fetch),
            Verdict::Allow
        );
        let sandboxed = Policy {
            sandbox: true,
            ..policy(Approval::All, vec![])
        };
        assert!(matches!(sandboxed.decide(&read_outside), Verdict::Deny(_)));
        assert!(matches!(sandboxed.decide(&fetch), Verdict::Deny(_)));
        assert!(matches!(sandboxed.decide(&write(false)), Verdict::Deny(_)));
        let reader = Policy {
            profile: Profile::Ask,
            ..policy(Approval::All, vec![])
        };
        assert!(matches!(reader.decide(&write(true)), Verdict::Deny(_)));
        assert!(matches!(reader.decide(&Action::Command("ls".into())), Verdict::Deny(_)));
    }

    #[test]
    fn an_order_builder_works_in_the_project_without_asking_and_is_refused_outside_it() {
        let builder = Policy::order(true);
        let inside = Action::Write {
            path: "src/a.rs".into(),
            inside: true,
        };
        let outside = Action::Read {
            path: "~/.ssh/id_ed25519".into(),
            inside: false,
        };
        assert_eq!(builder.decide(&inside), Verdict::Allow);
        assert_eq!(builder.decide(&Action::Command("cargo build".into())), Verdict::Allow);
        assert!(matches!(
            builder.decide(&outside),
            Verdict::Deny(Refusal::OutsideSandbox(_))
        ));
        let fetch = Action::Fetch { host: "docs.rs".into() };
        assert_eq!(builder.decide(&fetch), Verdict::Deny(Refusal::NoNetwork));
        let reader = Policy::order(false);
        assert_eq!(reader.decide(&inside), Verdict::Deny(Refusal::ReadOnly));
        assert_eq!(
            reader.decide(&Action::Command("ls".into())),
            Verdict::Deny(Refusal::ReadOnly)
        );
        assert_eq!(
            reader.decide(&Action::Read {
                path: "src/a.rs".into(),
                inside: true
            }),
            Verdict::Allow
        );
    }

    #[test]
    fn only_git_that_discards_the_folders_own_work_counts_as_such() {
        for command in [
            "git reset --hard",
            "git clean -fd",
            "git reset --hard HEAD~1 && git clean -fdx",
            "cargo test; git clean -fd",
        ] {
            assert!(discards_only_own_work(command), "{command}");
        }
        for command in [
            "git status",
            "git push -f",
            "git reset --hard && git push --force",
            "git stash drop",
            "git -C ../main reset --hard",
            "git --git-dir=../main/.git reset --hard",
            "GIT_DIR=../main/.git git reset --hard",
            "sh -c 'git reset --hard'",
            "cd ../main && git reset --hard",
        ] {
            assert!(!discards_only_own_work(command), "{command}");
        }
    }

    #[test]
    fn an_orders_builder_discards_its_own_work_without_asking_and_nothing_else_does() {
        let builder = Policy::order(true);
        let discard = Action::Discard("git reset --hard".into());
        assert_eq!(builder.decide(&discard), Verdict::Allow);
        assert!(matches!(
            builder.decide(&Action::Command("git reset --hard".into())),
            Verdict::Ask {
                reason: Some(Reason::Destructive(_)),
                ..
            }
        ));
        assert_eq!(Policy::order(false).decide(&discard), Verdict::Deny(Refusal::ReadOnly));
        assert!(matches!(
            policy(Approval::Manual, Vec::new()).decide(&discard),
            Verdict::Ask {
                reason: Some(Reason::Destructive(_)),
                ..
            }
        ));
    }
}
