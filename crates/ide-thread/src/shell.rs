//! How a command line reads as far as approvals need it: its commands, their words, and the git commands among them
//! that destroy work git cannot bring back (E27). Every agent kind asks before those through the same matcher.

use serde_json::Value;

/// A git command that destroys work git cannot bring back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destructive {
    /// `git push --force` rewrites the remote's history.
    ForcePush,
    /// `git reset --hard` discards uncommitted changes.
    ResetHard,
    /// `git clean -f` deletes untracked files.
    Clean,
    /// `git stash drop` deletes a stash entry.
    StashDrop,
    /// `git stash clear` deletes every stash entry.
    StashClear,
}

/// How deep `sh -c` and `eval` nest before the matcher stops looking further in.
const MAX_NESTING: usize = 8;

/// The commands of a command line, split at `&&`, `||`, `;`, `|`, `&` and line breaks outside quotes; a command
/// substitution counts as a command of its own.
pub fn segments(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                current.push(c);
            }
            (Some('"'), '$') if chars.peek() == Some(&'(') => {
                parts.push(std::mem::take(&mut current));
                chars.next();
            }
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                current.push(c);
            }
            (None, '\\') => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            (None, ';' | '|' | '&' | '\n' | '(' | ')' | '`') => parts.push(std::mem::take(&mut current)),
            (None, '$') if chars.peek() == Some(&'(') => {
                parts.push(std::mem::take(&mut current));
                chars.next();
            }
            (None, c) => current.push(c),
        }
    }
    parts.push(current);
    parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// The words of one command, with quotes removed.
pub fn words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                started = false;
            }
            (None, c) => {
                current.push(c);
                started = true;
            }
        }
    }
    if started || !current.is_empty() {
        words.push(current);
    }
    words
}

/// How `command` destroys work that git cannot bring back, if it does: a forced push, `reset --hard`, `clean` with
/// `-f`, or dropping or clearing the stash; also inside `sh -c '…'`, `bash -lc "…"` and `eval`.
pub fn destructive_git(command: &str) -> Option<Destructive> {
    nested(command, 0)
}

/// The command line a tool call's input runs, whether one string or a list of words.
pub fn command_of(input: &Value) -> Option<String> {
    match input.get("command")? {
        Value::String(command) => Some(command.clone()),
        Value::Array(parts) => Some(parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")),
        _ => None,
    }
}

/// How the command a tool call's `input` runs destroys work, if it does.
pub fn destructive_input(input: &Value) -> Option<Destructive> {
    destructive_git(&command_of(input)?)
}

fn nested(command: &str, depth: usize) -> Option<Destructive> {
    segments(command).iter().find_map(|part| {
        let words = words(part);
        destructive_part(&words).or_else(|| {
            if depth >= MAX_NESTING {
                return None;
            }
            inner_script(&words).and_then(|script| nested(&script, depth + 1))
        })
    })
}

/// The script a shell runs with `-c`, or the line `eval` runs.
fn inner_script(words: &[String]) -> Option<String> {
    if let Some(at) = words.iter().position(|w| w == "eval") {
        return Some(words[at + 1..].join(" "));
    }
    let shell = words.iter().position(|w| is_shell(w))?;
    let mut rest = words[shell + 1..].iter();
    while let Some(word) = rest.next() {
        if word.starts_with('-') && !word.starts_with("--") && word[1..].contains('c') {
            return rest.next().cloned();
        }
        if !word.starts_with('-') && !word.starts_with('+') {
            return None;
        }
    }
    None
}

fn is_shell(word: &str) -> bool {
    let name = word.rsplit('/').next().unwrap_or(word);
    matches!(name, "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish")
}

fn destructive_part(words: &[String]) -> Option<Destructive> {
    let git = words.iter().position(|w| w == "git" || w.ends_with("/git"))?;
    let mut rest = words[git + 1..].iter();
    let sub = loop {
        let word = rest.next()?;
        match word.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" => {
                rest.next();
            }
            w if w.starts_with('-') => {}
            w => break w,
        }
    };
    let args: Vec<&str> = rest.map(String::as_str).collect();
    let short = |flag: char| {
        args.iter()
            .any(|a| a.starts_with('-') && !a.starts_with("--") && a[1..].contains(flag))
    };
    match sub {
        "push"
            if args
                .iter()
                .any(|a| a.starts_with("--force") || (a.starts_with('+') && a.len() > 1) || *a == "--mirror")
                || short('f') =>
        {
            Some(Destructive::ForcePush)
        }
        "reset" if args.contains(&"--hard") => Some(Destructive::ResetHard),
        "clean" if (short('f') || args.contains(&"--force")) && !short('n') && !args.contains(&"--dry-run") => {
            Some(Destructive::Clean)
        }
        "stash" => match args.iter().find(|a| !a.starts_with('-')).copied() {
            Some("drop") => Some(Destructive::StashDrop),
            Some("clear") => Some(Destructive::StashClear),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn destructive_git_commands_are_found_in_any_spelling() {
        for (command, why) in [
            ("git push --force", Destructive::ForcePush),
            ("git push -f origin main", Destructive::ForcePush),
            ("git push --force-with-lease", Destructive::ForcePush),
            (
                "git push --force-with-lease=main:abc origin main",
                Destructive::ForcePush,
            ),
            ("git -C repo push origin +main", Destructive::ForcePush),
            ("echo ok; git push -uf origin x", Destructive::ForcePush),
            ("git push --mirror backup", Destructive::ForcePush),
            ("cd x && git reset --hard HEAD~1", Destructive::ResetHard),
            ("git -C x reset --hard", Destructive::ResetHard),
            (
                "git --git-dir=.git --work-tree=. reset --hard origin/main",
                Destructive::ResetHard,
            ),
            ("git -c core.pager=cat reset --hard", Destructive::ResetHard),
            ("git clean -fd", Destructive::Clean),
            ("git clean -fdx", Destructive::Clean),
            ("git clean -xdf", Destructive::Clean),
            ("/usr/bin/git clean --force", Destructive::Clean),
            ("git stash drop", Destructive::StashDrop),
            ("git stash drop stash@{1}", Destructive::StashDrop),
            ("git -C repo stash --quiet drop", Destructive::StashDrop),
            ("git stash clear", Destructive::StashClear),
            ("make && git stash clear", Destructive::StashClear),
            ("cargo test || git reset --hard", Destructive::ResetHard),
            ("ls | xargs git clean -f", Destructive::Clean),
            ("echo $(git reset --hard)", Destructive::ResetHard),
            ("sudo git push --force", Destructive::ForcePush),
            ("env GIT_TRACE=1 git push -f", Destructive::ForcePush),
        ] {
            assert_eq!(destructive_git(command), Some(why), "{command}");
        }
    }

    #[test]
    fn commands_inside_a_shell_or_eval_count_too() {
        for (command, why) in [
            ("sh -c 'git push -f'", Destructive::ForcePush),
            ("bash -c \"cd repo && git reset --hard\"", Destructive::ResetHard),
            ("bash -lc 'git clean -fdx'", Destructive::Clean),
            ("/bin/zsh -e -c 'git stash clear'", Destructive::StashClear),
            ("env X=1 sh -c 'sh -c \"git push --force\"'", Destructive::ForcePush),
            ("eval 'git reset --hard'", Destructive::ResetHard),
            ("ssh-agent sh -c 'git push -f' && true", Destructive::ForcePush),
        ] {
            assert_eq!(destructive_git(command), Some(why), "{command}");
        }
    }

    #[test]
    fn harmless_commands_are_not_destructive() {
        for command in [
            "git push origin main",
            "git push -u origin feature",
            "git reset --soft HEAD~1",
            "git reset HEAD file",
            "git clean -n",
            "git clean -fn",
            "git clean -f --dry-run",
            "echo 'git push --force'",
            "git log --format=%f",
            "git stash",
            "git stash push -m drop",
            "git stash list",
            "git stash pop",
            "git stash show clear",
            "sh -c 'git status'",
            "bash script.sh -c",
            "bash -c",
            "grep -c 'git reset --hard' notes.txt",
            "",
        ] {
            assert_eq!(destructive_git(command), None, "{command}");
        }
    }

    #[test]
    fn the_command_is_read_from_a_string_or_a_list_of_words() {
        assert_eq!(
            destructive_input(&json!({"command": "git push -f"})),
            Some(Destructive::ForcePush)
        );
        assert_eq!(
            destructive_input(&json!({"command": ["git", "reset", "--hard"]})),
            Some(Destructive::ResetHard)
        );
        assert_eq!(destructive_input(&json!({"file_path": "git clean -fd"})), None);
        assert_eq!(destructive_input(&json!({"command": 3})), None);
    }

    #[test]
    fn deep_nesting_ends() {
        let mut command = "git status".to_string();
        for _ in 0..40 {
            command = format!("eval {command}");
        }
        assert_eq!(destructive_git(&command), None);
    }
}
