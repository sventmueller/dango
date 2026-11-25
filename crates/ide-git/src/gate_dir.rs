//! A private git folder for the commands of a work order's gates (E187): its configuration holds only what git needs
//! to read the repository, its objects come from the real repository as an alternate, and its HEAD, refs and index are
//! copies. git inside a gate reads the repository through it, while the real configuration with remotes, credentials,
//! hooks and filters stays out of reach, and whatever git writes stays in the copy.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Repository;

/// The `core` settings taken over from the repository's configuration, which decide how git compares the working tree
/// with the index; values other than plain words are dropped.
const KEPT_CORE: [&str; 6] = [
    "ignorecase",
    "precomposeunicode",
    "filemode",
    "symlinks",
    "autocrlf",
    "eol",
];

/// The extensions taken over, which say how the objects are named.
const KEPT_EXTENSIONS: [&str; 2] = ["objectformat", "compatobjectformat"];

/// A private git folder for one gate run, made by [`GateGitDir::build`] and removed by [`GateGitDir::remove`].
#[derive(Debug)]
pub struct GateGitDir {
    folder: PathBuf,
    work_tree: PathBuf,
}

impl GateGitDir {
    /// Makes the private folder for the working tree of `repo` in `folder`, replacing what was there: the folder `git`
    /// reads as `GIT_DIR` and the file it reads as its global configuration. Refused for a repository whose refs are
    /// not kept as files, such as reftable's, whose HEAD of a linked worktree cannot be copied.
    pub fn build(repo: &Repository, folder: &Path) -> io::Result<GateGitDir> {
        let common = repo.common_dir();
        let objects = common.join("objects");
        let alternate = objects
            .to_str()
            .filter(|path| !path.contains('\n') && !path.starts_with('"'));
        let Some(alternate) = alternate else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "objects folder without a plain path",
            ));
        };
        let settings = settings(&fs::read_to_string(common.join("config"))?);
        let storage = settings.iter().find(|(name, _)| name == "extensions.refstorage");
        if storage.is_some_and(|(_, value)| !value.eq_ignore_ascii_case("files")) {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "refs not kept as files"));
        }
        if folder.exists() {
            fs::remove_dir_all(folder)?;
        }
        let dir = folder.join("git");
        for sub in ["objects/info", "objects/pack", "refs/heads", "refs/tags", "info"] {
            fs::create_dir_all(dir.join(sub))?;
        }
        fs::write(dir.join("config"), configuration(&settings))?;
        fs::write(dir.join("objects/info/alternates"), format!("{alternate}\n"))?;
        fs::write(folder.join("global"), identity())?;
        copy_file(&repo.git_dir().join("HEAD"), &dir.join("HEAD"))?;
        for (from, name) in [
            (common, "packed-refs"),
            (common, "shallow"),
            (common, "info/exclude"),
            (repo.git_dir(), "index"),
        ] {
            if from.join(name).exists() {
                copy_file(&from.join(name), &dir.join(name))?;
            }
        }
        for entry in fs::read_dir(repo.git_dir())?.flatten() {
            if entry.file_name().to_string_lossy().starts_with("sharedindex.") && entry.file_type()?.is_file() {
                fs::copy(entry.path(), dir.join(entry.file_name()))?;
            }
        }
        copy_tree(&common.join("refs"), &dir.join("refs"))?;
        copy_tree(&repo.git_dir().join("refs"), &dir.join("refs"))?;
        Ok(GateGitDir {
            folder: folder.to_path_buf(),
            work_tree: repo.root().to_path_buf(),
        })
    }

    /// The folder holding the private git folder and its global configuration, which the gate writes in.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The variables that point git at the private folder and the working tree, without the system's and the user's
    /// configuration and without asking for credentials.
    pub fn environment(&self) -> Vec<(String, String)> {
        let path = |path: &Path| path.to_string_lossy().into_owned();
        vec![
            ("GIT_DIR".into(), path(&self.folder.join("git"))),
            ("GIT_WORK_TREE".into(), path(&self.work_tree)),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), path(&self.folder.join("global"))),
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ]
    }

    /// Removes the folder with everything a gate's git wrote into it.
    pub fn remove(self) -> io::Result<()> {
        fs::remove_dir_all(&self.folder)
    }

    /// Whether a gate needs the private folder for `repo`: when its configuration may hold credentials, in a remote's
    /// address, a header, a credential section or a file it includes, or when it cannot be read. Otherwise git in the
    /// gate reads the repository itself, and no `GIT_DIR` leads a test suite's own repositories into a copy.
    pub fn needed(repo: &Repository) -> bool {
        fs::read_to_string(repo.common_dir().join("config")).map_or(true, |text| may_hold_credentials(&text))
    }

    /// The variables of a gate without the private folder: a global configuration in `folder` with only a neutral
    /// identity, as the user's is out of reach in the sandbox, without the system's and without asking for
    /// credentials.
    pub fn neutral_environment(folder: &Path) -> io::Result<Vec<(String, String)>> {
        fs::create_dir_all(folder)?;
        let global = folder.join("global");
        fs::write(&global, identity())?;
        Ok(vec![
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), global.to_string_lossy().into_owned()),
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ])
    }
}

/// Whether the git configuration `text` may hold a credential: an address with a password or, over HTTP, a user, which
/// is often a token; a header, password, token or secret; a credential section; or an include, whose file is not read
/// here.
fn may_hold_credentials(text: &str) -> bool {
    text.lines().map(|line| without_comment(line.trim())).any(|line| {
        let lower = line.to_ascii_lowercase();
        let header = lower.strip_prefix('[').unwrap_or("");
        let section = ["include", "credential"].iter().any(|name| {
            header
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with([']', ' ', '"', '.']) || rest.starts_with("if"))
        });
        let key = lower.split('=').next().unwrap_or("").trim();
        let named = ["extraheader", "password", "token", "secret"]
            .iter()
            .any(|word| key.contains(word));
        section || named || address_with_credential(&lower)
    })
}

/// Whether `line` holds an address with a password, or a user over HTTP.
fn address_with_credential(line: &str) -> bool {
    line.match_indices("://").any(|(at, _)| {
        let scheme = line[..at]
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '+' && c != '-')
            .next();
        let rest = &line[at + 3..];
        let authority = rest.split(['/', '"', ' ', '\t']).next().unwrap_or("");
        let Some((user, _host)) = authority.rsplit_once('@') else {
            return false;
        };
        user.contains(':') || scheme.is_some_and(|s| s.starts_with("http") || s.starts_with("ftp"))
    })
}

/// The private folder's configuration: the repository format, no hooks and no file monitor, and of `settings` the
/// [`KEPT_CORE`] and [`KEPT_EXTENSIONS`].
fn configuration(settings: &[(String, String)]) -> String {
    let kept = |section: &str, names: &[&str]| -> String {
        settings
            .iter()
            .filter_map(|(name, value)| {
                let key = name.strip_prefix(section)?.strip_prefix('.')?;
                let plain = !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
                (names.contains(&key) && plain).then(|| format!("\t{key} = {value}\n"))
            })
            .collect()
    };
    format!(
        "[core]\n\trepositoryformatversion = 1\n\tbare = false\n\thooksPath = /dev/null\n\tfsmonitor = false\n{}\
         [extensions]\n{}",
        kept("core", &KEPT_CORE),
        kept("extensions", &KEPT_EXTENSIONS),
    )
}

/// The global configuration of a gate's git: a neutral identity for commits a gate makes.
fn identity() -> String {
    format!("[user]\n\tname = {} gate\n\temail = gate@invalid\n", ide_product::NAME)
}

/// The settings of sections without a subsection in the git configuration `text`, as `section.key` in lower case
/// with their values; a key without a value is `true`. Includes are not followed, as git does not follow them for the
/// repository format either.
fn settings(text: &str) -> Vec<(String, String)> {
    let mut section: Option<String> = None;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut line = line.trim();
        if let Some(rest) = line.strip_prefix('[') {
            let Some((header, after)) = rest.split_once(']') else {
                section = None;
                continue;
            };
            let header = header.trim();
            section = (!header.contains(['"', '.', ' ']) && !header.is_empty()).then(|| header.to_ascii_lowercase());
            line = after.trim();
        }
        let line = without_comment(line);
        let Some(section) = &section else { continue };
        if line.is_empty() {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap_or((line, "true"));
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        out.push((
            format!("{section}.{}", key.trim().to_ascii_lowercase()),
            value.to_string(),
        ));
    }
    out
}

/// `line` up to a `#` or `;` outside quotes.
fn without_comment(line: &str) -> &str {
    let mut quoted = false;
    for (at, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' | ';' if !quoted => return line[..at].trim(),
            _ => {}
        }
    }
    line.trim()
}

/// Copies the regular file `from` to `to`; anything else, such as a symbolic link to a file outside the repository,
/// is refused.
fn copy_file(from: &Path, to: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(from)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is no file", from.display()),
        ));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from, to).map(|_| ())
}

/// Copies the folders and regular files below `from` into `to`, leaving out symbolic links; nothing when `from` is
/// missing.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    fs::create_dir_all(to)?;
    for entry in entries {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        } else if kind.is_file() {
            fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_configuration_with_a_credential_needs_the_private_folder() {
        let plain = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = https://github.com/a/b.git\n\
                     [remote \"ssh\"]\n\turl = git@github.com:a/b.git\n[remote \"s2\"]\n\turl = ssh://git@host/a.git\n\
                     [branch \"main\"]\n\tremote = origin\n";
        assert!(!super::may_hold_credentials(plain));
        for secret in [
            "[remote \"origin\"]\n\turl = https://ghp_abc@github.com/a/b.git\n",
            "[remote \"origin\"]\n\turl = https://user:pw@example.com/a.git\n",
            "[remote \"origin\"]\n\turl = ssh://git:pw@host/a.git\n",
            "[http \"https://github.com/\"]\n\textraHeader = Authorization: Bearer x\n",
            "[credential]\n\thelper = store\n",
            "[credential \"https://github.com\"]\n\tusername = me\n",
            "[include]\n\tpath = ~/.secrets\n",
            "[includeIf \"gitdir:~/work/\"]\n\tpath = ~/.work\n",
            "[lfs]\n\taccesstoken = x\n",
        ] {
            assert!(super::may_hold_credentials(secret), "{secret}");
        }
    }

    use super::*;

    #[test]
    fn only_plain_sections_are_read_and_comments_dropped() {
        let text = "[core]\n\trepositoryformatversion = 1\n\tbare = false ; c\n\tIgnoreCase = true\n\
                    [remote \"origin\"]\n\turl = https://token@example.org/x.git\n\
                    [Extensions]\n\tobjectFormat = \"sha256\" # c\n\tworktreeConfig\n[core] filemode = false\n";
        let found = settings(text);
        assert!(found.contains(&("core.ignorecase".into(), "true".into())), "{found:?}");
        assert!(found.contains(&("core.bare".into(), "false".into())), "{found:?}");
        assert!(
            found.contains(&("extensions.objectformat".into(), "sha256".into())),
            "{found:?}"
        );
        assert!(
            found.contains(&("extensions.worktreeconfig".into(), "true".into())),
            "{found:?}"
        );
        assert!(found.contains(&("core.filemode".into(), "false".into())), "{found:?}");
        assert!(!found.iter().any(|(_, value)| value.contains("token")), "{found:?}");
    }

    #[test]
    fn the_configuration_keeps_the_format_and_the_working_tree_rules_only() {
        let settings = settings(
            "[core]\n\tignorecase = true\n\tsshCommand = ssh -i key\n\teditor = vim\n\tfsmonitor = true\n\
             \thooksPath = .hooks\n\tautocrlf = \"input\\nx\"\n\
             [extensions]\n\tobjectformat = sha256\n\tpartialclone = origin\n\tworktreeconfig = true\n\
             [include]\n\tpath = ~/.secret\n[filter]\n\tclean = x\n",
        );
        assert_eq!(
            configuration(&settings),
            "[core]\n\trepositoryformatversion = 1\n\tbare = false\n\thooksPath = /dev/null\n\tfsmonitor = false\n\
             \tignorecase = true\n[extensions]\n\tobjectformat = sha256\n"
        );
    }
}
