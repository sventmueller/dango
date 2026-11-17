//! The CI seal (docs/development.md, "CI and deploy"): a git ref `refs/ci-seals/<green|red>/<fingerprint>` saying that
//! a complete `cargo xtask ci` run of one content state passed or failed, and `deploy`, which pushes main only with
//! its green seal. The fingerprint is the git tree of the content without `VERSION`, which only deploy raises,
//! so a commit of the sealed working tree inherits the seal. Red beats green for the same fingerprint.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::*;

/// The file the fingerprint leaves out.
const VERSION_FILE: &str = "VERSION";

/// The folder of refs the seals live in, here and on origin.
const SEALS: &str = "refs/ci-seals";

/// The branch `deploy` pushes.
const BRANCH: &str = "main";

/// The most differing paths `explain` lists.
const LISTED: usize = 20;

/// A repository and the environment its git commands get.
pub(crate) struct Git {
    dir: PathBuf,
    env: Vec<(String, String)>,
}

impl Git {
    pub(crate) fn at(dir: &Path) -> Git {
        Git {
            dir: dir.to_path_buf(),
            env: Vec::new(),
        }
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.dir)
            .args(args)
            .envs(self.env.iter().map(|(key, value)| (key, value)));
        cmd
    }

    pub(crate) fn output(&self, sh: &mut dyn Shell, args: &[&str]) -> Result<String, String> {
        sh.stdout(&mut self.command(args))
    }
}

/// Which content a fingerprint describes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Content {
    /// HEAD with every change of the working tree, untracked files that are not ignored included.
    WorkingTree,
    /// HEAD's commit, which is also what a CI checkout of it has.
    Head,
}

/// The fingerprint of `content`: the name of its git tree without `VERSION`, written to the repository's objects.
pub(crate) fn fingerprint(sh: &mut dyn Shell, git: &Git, content: Content) -> Result<String, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let index = std::env::temp_dir().join(format!(
        "xtask-seal-{}-{}.index",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let tree = tree_without_version(sh, git, content, &index);
    let _ = std::fs::remove_file(&index);
    tree
}

fn tree_without_version(sh: &mut dyn Shell, git: &Git, content: Content, index: &Path) -> Result<String, String> {
    let mut indexed = |args: &[&str]| {
        let mut cmd = git.command(args);
        cmd.env("GIT_INDEX_FILE", index);
        sh.stdout(&mut cmd)
    };
    indexed(&["read-tree", "HEAD"])?;
    if content == Content::WorkingTree {
        indexed(&["add", "--all"])?;
    }
    indexed(&["rm", "--cached", "--quiet", "--ignore-unmatch", "--", VERSION_FILE])?;
    let tree = indexed(&["write-tree"])?;
    let tree = tree.trim();
    if is_object_name(tree) {
        Ok(tree.to_string())
    } else {
        Err(format!("git write-tree printed {tree:?}, not an object name"))
    }
}

/// A full SHA-1 or SHA-256 object name.
fn is_object_name(name: &str) -> bool {
    matches!(name.len(), 40 | 64) && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// What a complete run found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Seal {
    Green,
    Red,
}

impl Seal {
    pub(crate) fn reference(self, fingerprint: &str) -> String {
        let colour = match self {
            Seal::Green => "green",
            Seal::Red => "red",
        };
        format!("{SEALS}/{colour}/{fingerprint}")
    }
}

/// What the seals say about one fingerprint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Verdict {
    Green,
    Red,
    Unsealed,
}

impl Verdict {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Verdict::Green => "green",
            Verdict::Red => "red",
            Verdict::Unsealed => "none",
        }
    }
}

/// The verdict of the seal refs `refs` for `fingerprint`; a red seal outweighs a green one.
pub(crate) fn verdict<'a>(refs: impl IntoIterator<Item = &'a str>, fingerprint: &str) -> Verdict {
    let (green, red) = (Seal::Green.reference(fingerprint), Seal::Red.reference(fingerprint));
    let mut found = Verdict::Unsealed;
    for name in refs {
        if name == red {
            return Verdict::Red;
        }
        if name == green {
            found = Verdict::Green;
        }
    }
    found
}

/// The seal refs of this repository.
fn local_seals(sh: &mut dyn Shell, git: &Git) -> Result<Vec<String>, String> {
    let listed = git.output(sh, &["for-each-ref", "--format=%(refname)", &format!("{SEALS}/")])?;
    Ok(listed.lines().map(str::to_string).collect())
}

/// The seal refs origin has for `fingerprint`.
fn remote_seals(sh: &mut dyn Shell, git: &Git, fingerprint: &str) -> Result<Vec<String>, String> {
    let listed = git.output(sh, &["ls-remote", "origin", &format!("{SEALS}/*/{fingerprint}")])?;
    Ok(listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(_, name)| name.to_string())
        .collect())
}

/// Records `seal` for `fingerprint` as a local ref to a commit of the sealed tree, and returns the ref.
pub(crate) fn mint(sh: &mut dyn Shell, git: &Git, seal: Seal, fingerprint: &str) -> Result<String, String> {
    let reference = seal.reference(fingerprint);
    let message = format!("CI seal {reference}");
    let commit = git.output(sh, &["commit-tree", "--no-gpg-sign", "-m", &message, fingerprint])?;
    git.output(sh, &["update-ref", &reference, commit.trim()])?;
    Ok(reference)
}

/// `cargo xtask seal …`.
pub(crate) fn seal(args: &[String]) -> Result<(), String> {
    let git = Git::at(&root());
    match args.split_first() {
        Some((what, rest)) if what == "fingerprint" => {
            let [head] = flags(rest, ["--head"])?;
            let content = if head { Content::Head } else { Content::WorkingTree };
            println!("{}", fingerprint(&mut System, &git, content)?);
            Ok(())
        }
        Some((what, rest)) if what == "check" => {
            flags(rest, [])?;
            let output = std::env::var_os("GITHUB_OUTPUT").map(PathBuf::from);
            check_in(&mut System, &git, output.as_deref()).map(|_| ())
        }
        Some((what, rest)) if what == "explain" => {
            flags(rest, [])?;
            print!("{}", explain_in(&mut System, &git)?);
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

/// The question the GitHub workflow asks: origin's verdict for HEAD's content, printed and appended to the step
/// outputs file `output` as `fingerprint=` and `verdict=`; red is an error.
pub(crate) fn check_in(sh: &mut dyn Shell, git: &Git, output: Option<&Path>) -> Result<Verdict, String> {
    let fp = fingerprint(sh, git, Content::Head)?;
    let refs = remote_seals(sh, git, &fp)?;
    let found = verdict(refs.iter().map(String::as_str), &fp);
    let lines = format!("fingerprint={fp}\nverdict={}\n", found.name());
    print!("{lines}");
    if let Some(path) = output {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(lines.as_bytes())
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if found == Verdict::Red {
        Err(format!(
            "content {fp} is sealed red: a complete local `make ci` run of it failed"
        ))
    } else {
        Ok(found)
    }
}

/// What the seals say about HEAD's content here and on origin, about the working tree when it differs, and which
/// paths separate HEAD from the nearest green seal when it has none here.
pub(crate) fn explain_in(sh: &mut dyn Shell, git: &Git) -> Result<String, String> {
    let head = fingerprint(sh, git, Content::Head)?;
    let working = fingerprint(sh, git, Content::WorkingTree)?;
    let local = local_seals(sh, git)?;
    let here = verdict(local.iter().map(String::as_str), &head);
    let origin = match remote_seals(sh, git, &head) {
        Ok(refs) => match verdict(refs.iter().map(String::as_str), &head) {
            Verdict::Green => "green, so GitHub's CI skips its tests".to_string(),
            Verdict::Red => "red, so GitHub's CI fails".to_string(),
            Verdict::Unsealed => "none, so GitHub's CI runs `make check`".to_string(),
        },
        Err(e) => format!("unreachable ({e})"),
    };
    let mut text = format!(
        "HEAD's content {head}\n  seal here: {}\n  seal on origin: {origin}\n",
        here.name()
    );
    if working == head {
        text.push_str("the working tree has the same content\n");
    } else {
        let seal = verdict(local.iter().map(String::as_str), &working);
        text.push_str(&format!(
            "the working tree differs: content {working}, seal here: {}; it ships only once committed\n",
            seal.name()
        ));
    }
    if here != Verdict::Green {
        text.push_str(&nearest_green(sh, git, &local, &head)?);
    }
    Ok(text)
}

/// The local green seal whose content is closest to `fingerprint`'s, with the paths that differ.
fn nearest_green(sh: &mut dyn Shell, git: &Git, local: &[String], fingerprint: &str) -> Result<String, String> {
    let prefix = format!("{SEALS}/green/");
    let mut nearest: Option<(String, Vec<String>)> = None;
    for sealed in local.iter().filter_map(|name| name.strip_prefix(&prefix)) {
        let differing = git.output(sh, &["diff", "--name-only", sealed, fingerprint])?;
        let differing: Vec<String> = differing.lines().map(str::to_string).collect();
        if nearest.as_ref().is_none_or(|(_, paths)| differing.len() < paths.len()) {
            nearest = Some((sealed.to_string(), differing));
        }
    }
    let Some((sealed, paths)) = nearest else {
        return Ok("no green seal here to compare with\n".to_string());
    };
    let mut text = format!("nearest green seal {sealed}, {} paths differ:\n", paths.len());
    for path in paths.iter().take(LISTED) {
        text.push_str(&format!("  {path}\n"));
    }
    if paths.len() > LISTED {
        text.push_str(&format!("  … and {} more\n", paths.len() - LISTED));
    }
    Ok(text)
}

/// Refuses on another branch, with changes not committed, or without a green seal here for HEAD's content.
pub(crate) fn ready(sh: &mut dyn Shell, git: &Git) -> Result<String, String> {
    let branch = git.output(sh, &["branch", "--show-current"])?;
    if branch.trim() != BRANCH {
        return Err(format!(
            "deploy pushes {BRANCH}, but {:?} is checked out",
            branch.trim()
        ));
    }
    let status = git.output(sh, &["status", "--porcelain"])?;
    if !status.trim().is_empty() {
        return Err(format!(
            "the working tree has changes that are not committed, which a push would leave out:\n{status}"
        ));
    }
    let fp = fingerprint(sh, git, Content::Head)?;
    let local = local_seals(sh, git)?;
    match verdict(local.iter().map(String::as_str), &fp) {
        Verdict::Green => {}
        Verdict::Red => {
            return Err(format!(
                "HEAD's content {fp} is sealed red: its last complete `make ci` run failed"
            ));
        }
        Verdict::Unsealed => {
            return Err(format!(
                "HEAD's content {fp} has no green seal; `make ci` (or /ci) seals it"
            ));
        }
    }
    let remote = remote_seals(sh, git, &fp)?;
    if verdict(remote.iter().map(String::as_str), &fp) == Verdict::Red {
        return Err(format!(
            "origin has a red seal for HEAD's content {fp}, so its CI would fail"
        ));
    }
    Ok(fp)
}

/// Pushes the release commit, green seal and version tag atomically, after all draft assets have been uploaded.
pub(crate) fn push(sh: &mut dyn Shell, git: &Git, tag: &str) -> Result<(), String> {
    let seal = Seal::Green.reference(&ready(sh, git)?);
    sh.run(&mut git.command(&["push", "--atomic", "origin", BRANCH, &seal, &format!("refs/tags/{tag}")]))?;
    println!("pushed {BRANCH} with {seal} and {tag}");
    Ok(())
}

#[cfg(test)]
#[path = "seal_tests.rs"]
pub(crate) mod tests;
