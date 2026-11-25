//! The CI seal (docs/development.md, "CI and deploy"): a git ref `refs/ci-seals/<green|red>/<fingerprint>` saying that
//! a complete `cargo xtask ci` run of one content state passed or failed. `deploy` requires none and pushes a green
//! one along when HEAD's content has it. The fingerprint is the git tree of the content without the version, which only
//! deploy raises, so a commit of the sealed working tree and its release commit inherit the seal, and with the commits
//! and changes of
//! `../splat` and `../lowspeak`, so a change there needs a new seal. Red beats green for the same fingerprint.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::*;

/// The folder of refs the seals live in, here and on origin.
const SEALS: &str = "refs/ci-seals";

/// The branch `deploy` pushes.
const BRANCH: &str = "main";

/// The most differing paths `explain` lists.
const LISTED: usize = 20;

/// The checkouts beside the repository that the workspace builds crates from by path (E14).
pub(crate) const SIBLINGS: [&str; 2] = ["splat", "lowspeak"];

/// The folder of the fingerprint's tree that records the siblings as gitlinks, which a tree holds without the
/// objects they name.
const SIBLINGS_DIR: &str = ".siblings";

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

    /// The checkout `name` beside this one, with the same environment.
    pub(crate) fn sibling(&self, name: &str) -> Git {
        Git {
            dir: self.dir.join("..").join(name),
            env: self.env.clone(),
        }
    }

    /// The object `args` name, such as `HEAD`.
    fn object(&self, sh: &mut dyn Shell, args: &[&str]) -> Result<String, String> {
        let name = self.output(sh, args)?;
        let name = name.trim();
        if is_object_name(name) {
            Ok(name.to_string())
        } else {
            Err(format!("git {} printed {name:?}, not an object name", args.join(" ")))
        }
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

/// The fingerprint of `content`: the name of its git tree with the version neutral and with the state of the
/// siblings, written to the repository's objects.
pub(crate) fn fingerprint(sh: &mut dyn Shell, git: &Git, content: Content) -> Result<String, String> {
    let index = scratch_index();
    let tree = sibling_entries(sh, git).and_then(|siblings| tree_without_version(sh, git, content, &siblings, &index));
    let _ = std::fs::remove_file(&index);
    tree
}

/// Puts each file of `versions::FILES` in `index` with its version neutral, written to the repository's objects.
fn neutral_versions(sh: &mut dyn Shell, git: &Git, index: &Path) -> Result<(), String> {
    let members = versions::members(sh, git.dir())?;
    for path in versions::FILES {
        let staged = indexed(sh, git, index, &["ls-files", "--stage", "--", path])?;
        let mut fields = staged.split_whitespace();
        let (Some(mode), Some(object)) = (fields.next(), fields.next()) else {
            continue;
        };
        let text = git.output(sh, &["cat-file", "blob", object])?;
        let neutral = versions::neutral(path, &text, &members);
        if neutral == text {
            continue;
        }
        let file = scratch_index();
        let written = files::write(&file, neutral).and_then(|()| {
            let file = file.display().to_string();
            git.output(sh, &["hash-object", "-w", "--no-filters", "--", &file])
        });
        let _ = std::fs::remove_file(&file);
        let entry = format!("{mode},{},{path}", written?.trim());
        indexed(sh, git, index, &["update-index", "--cacheinfo", &entry])?;
    }
    Ok(())
}

/// A path for an index file of its own, which git creates on first use.
fn scratch_index() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "xtask-seal-{}-{}.index",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// `git` with the index file `index` in place of the repository's own.
fn indexed(sh: &mut dyn Shell, git: &Git, index: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = git.command(args);
    cmd.env("GIT_INDEX_FILE", index);
    sh.stdout(&mut cmd)
}

fn tree_without_version(
    sh: &mut dyn Shell,
    git: &Git,
    content: Content,
    siblings: &[(String, String)],
    index: &Path,
) -> Result<String, String> {
    indexed(sh, git, index, &["read-tree", "HEAD"])?;
    if content == Content::WorkingTree {
        indexed(sh, git, index, &["add", "--all"])?;
    }
    neutral_versions(sh, git, index)?;
    for (path, object) in siblings {
        let entry = format!("160000,{object},{path}");
        indexed(sh, git, index, &["update-index", "--add", "--cacheinfo", &entry])?;
    }
    let tree = indexed(sh, git, index, &["write-tree"])?;
    let tree = tree.trim();
    if is_object_name(tree) {
        Ok(tree.to_string())
    } else {
        Err(format!("git write-tree printed {tree:?}, not an object name"))
    }
}

/// What the fingerprint records of each sibling there is, as a path below `SIBLINGS_DIR` and the object it names: its
/// HEAD commit, and the tree of its working tree when that holds changes. Writing that tree stores the changed files
/// in the sibling's objects, as `git add` would, and leaves its index alone.
fn sibling_entries(sh: &mut dyn Shell, git: &Git) -> Result<Vec<(String, String)>, String> {
    let mut entries = Vec::new();
    for name in SIBLINGS {
        let sibling = git.sibling(name);
        if !sibling.dir.exists() {
            continue;
        }
        let head = sibling.object(sh, &["rev-parse", "HEAD"])?;
        let committed = sibling.object(sh, &["rev-parse", "HEAD^{tree}"])?;
        let index = scratch_index();
        let working = indexed(sh, &sibling, &index, &["read-tree", "HEAD"])
            .and_then(|_| indexed(sh, &sibling, &index, &["add", "--all"]))
            .and_then(|_| indexed(sh, &sibling, &index, &["write-tree"]));
        let _ = std::fs::remove_file(&index);
        let working = working?.trim().to_string();
        if !is_object_name(&working) {
            return Err(format!(
                "git write-tree printed {working:?} in ../{name}, not an object name"
            ));
        }
        entries.push((format!("{SIBLINGS_DIR}/{name}"), head));
        if working != committed {
            entries.push((format!("{SIBLINGS_DIR}/{name}-changes"), working));
        }
    }
    Ok(entries)
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
        Some((what, rest)) if what == "explain" => {
            flags(rest, [])?;
            print!("{}", explain_in(&mut System, &git)?);
            Ok(())
        }
        _ => Err(USAGE.into()),
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
        Ok(refs) => verdict(refs.iter().map(String::as_str), &head).name().to_string(),
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

/// Refuses on another branch or with changes not committed, and returns HEAD's fingerprint; seals do not matter.
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
    fingerprint(sh, git, Content::Head)
}

/// Refuses unless each sibling is there, without changes and with its HEAD on its upstream, so the release can be built
/// again from what others can fetch; returns each sibling's HEAD commit.
pub(crate) fn siblings_ready(sh: &mut dyn Shell, git: &Git) -> Result<BTreeMap<String, String>, String> {
    let mut commits = BTreeMap::new();
    for name in SIBLINGS {
        let sibling = git.sibling(name);
        if !sibling.dir.exists() {
            return Err(format!("../{name} is missing, which the release builds crates from"));
        }
        let status = sibling.output(sh, &["status", "--porcelain"])?;
        if !status.trim().is_empty() {
            return Err(format!(
                "../{name} has changes that are not committed, which no one could build the release from again:\n{status}"
            ));
        }
        sibling.output(sh, &["fetch", "--quiet"])?;
        let upstream = sibling
            .output(
                sh,
                &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
            )
            .map_err(|_| format!("../{name}'s branch has no upstream to fetch the release's sources from"))?;
        if sibling
            .output(sh, &["merge-base", "--is-ancestor", "HEAD", "@{upstream}"])
            .is_err()
        {
            return Err(format!(
                "../{name} has commits that {} lacks; push them first",
                upstream.trim()
            ));
        }
        commits.insert(name.to_string(), sibling.object(sh, &["rev-parse", "HEAD"])?);
    }
    Ok(commits)
}

/// Pushes the release commit and version tag atomically, after all draft assets have been uploaded, with the green
/// seal of HEAD's content when it has one here, so origin records which released content a complete `make ci` passed.
pub(crate) fn push(sh: &mut dyn Shell, git: &Git, tag: &str) -> Result<(), String> {
    let fp = ready(sh, git)?;
    let local = local_seals(sh, git)?;
    let tag = format!("refs/tags/{tag}");
    let mut refs = vec![BRANCH.to_string(), tag];
    if verdict(local.iter().map(String::as_str), &fp) == Verdict::Green {
        refs.push(Seal::Green.reference(&fp));
    }
    let mut args = vec!["push", "--atomic", "origin"];
    args.extend(refs.iter().map(String::as_str));
    sh.run(&mut git.command(&args))?;
    println!("pushed {}", refs.join(" "));
    Ok(())
}

#[cfg(test)]
#[path = "seal_tests.rs"]
pub(crate) mod tests;
