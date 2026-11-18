//! Notarize locally, stage every asset in a draft, atomically push the version and tag (with a green seal if any), then
//! publish the draft once GitHub holds exactly the notarized files.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::seal::{self, Git};
use crate::*;

const REPOSITORY: &str = "sventmueller/dango";
const MANIFEST: &str = "deployment.json";

#[derive(Deserialize, Serialize)]
struct Deployment {
    version: String,
    source_commit: String,
    commit: String,
    fingerprint: String,
    files: BTreeMap<String, String>,
}

impl Deployment {
    fn save(&self, dist: &Path) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        files::write(dist.join(MANIFEST), json)
    }

    fn verify(&self, sh: &mut dyn Shell, dist: &Path) -> Result<(), String> {
        let expected = assets(&self.version);
        if self.files.len() != expected.len() || expected.iter().any(|file| !self.files.contains_key(file)) {
            return Err("the deployment manifest does not list exactly the release assets".into());
        }
        for (file, digest) in &self.files {
            if updates::sha256(sh, &dist.join(file))? != *digest {
                return Err(format!("{file} changed after notarization; refusing to upload or push"));
            }
        }
        Ok(())
    }

    /// The update feed offers exactly this release's image: its version, its download address, a real signature and
    /// the image's length.
    fn check_feed(&self, dist: &Path) -> Result<(), String> {
        let feed = files::read_to_string(dist.join(updates::FEED))?;
        let image = dist.join("Dango.dmg");
        let length = std::fs::metadata(&image)
            .map_err(|e| format!("{}: {e}", image.display()))?
            .len();
        let version = &self.version;
        let offered = [
            format!("<sparkle:version>{version}</sparkle:version>"),
            format!("url=\"{}\"", updates::image_url(updates::RELEASES, version)),
            format!("length=\"{length}\""),
        ];
        if feed.contains("DRY-RUN") || offered.iter().any(|part| !feed.contains(part.as_str())) {
            return Err(format!(
                "the update feed does not offer the notarized image of {version}"
            ));
        }
        Ok(())
    }
}

fn assets(version: &str) -> Vec<String> {
    vec![
        updates::image_name(version),
        "Dango.dmg".into(),
        format!("Dango-{version}.md"),
        updates::FEED.into(),
        cask::file_name(),
    ]
}

/// `cargo xtask deploy`. Normal builds and a standalone notarization never change VERSION.
pub(crate) fn deploy(args: &[String]) -> Result<(), String> {
    flags(args, [])?;
    let sh: &mut dyn Shell = &mut System;
    let git = Git::at(&root());
    let fp = seal::ready(sh, &git)?;
    let origin = git.output(sh, &["remote", "get-url", "origin"])?;
    if !matches!(
        origin.trim(),
        "https://github.com/sventmueller/dango.git" | "git@github.com:sventmueller/dango.git"
    ) {
        return Err(format!("origin is {}, expected {REPOSITORY}", origin.trim()));
    }
    let access = sh.stdout(&mut gh(&[
        "api",
        &format!("repos/{REPOSITORY}"),
        "--jq",
        ".permissions.push",
    ]))?;
    if access.trim() != "true" {
        return Err(format!("gh has no write access to {REPOSITORY}"));
    }
    git.output(sh, &["fetch", "origin", "main"])?;
    git.output(sh, &["merge-base", "--is-ancestor", "FETCH_HEAD", "HEAD"])?;
    let dist = root().join("dist");
    let head = git.output(sh, &["rev-parse", "HEAD"])?;
    let current = version()?;
    let pending = pending(&dist, &fp, head.trim(), &current)?;
    let mut deployment = if let Some(pending) = pending {
        println!("resuming deploy {}", pending.version);
        pending.verify(sh, &dist)?;
        pending
    } else {
        let next = following(&current)?;
        let tag = format!("v{next}");
        let tags = git.output(sh, &["tag", "--list", &tag])?;
        let remote = git.output(sh, &["ls-remote", "origin", &format!("refs/tags/{tag}")])?;
        if !tags.trim().is_empty() || !remote.trim().is_empty() {
            return Err(format!("{tag} already exists; refusing to replace a release"));
        }
        release::release_version(
            release::Options {
                notarize: true,
                layout: true,
            },
            &next,
        )?;
        // A build must not sneak edits into the notarized content, nor may another process move HEAD or VERSION.
        if seal::ready(sh, &git)? != fp || git.output(sh, &["rev-parse", "HEAD"])? != head || version()? != current {
            return Err("the source changed while notarizing; nothing was pushed".into());
        }
        files::copy(dist.join(updates::image_name(&next)), dist.join("Dango.dmg"))?;
        let mut hashes = BTreeMap::new();
        for file in assets(&next) {
            hashes.insert(file.clone(), updates::sha256(sh, &dist.join(&file))?);
        }
        let prepared = Deployment {
            version: next,
            source_commit: head.trim().into(),
            commit: String::new(),
            fingerprint: fp,
            files: hashes,
        };
        // Written before the version commit, so an interrupted upload/push can resume without a second bump.
        prepared.save(&dist)?;
        prepared
    };
    commit_version(sh, &git, &mut deployment)?;
    deployment.save(&dist)?;
    let tag = format!("v{}", deployment.version);
    ensure_tag(sh, &git, &tag, &deployment.commit)?;
    deployment.check_feed(&dist)?;
    stage(sh, &dist, &deployment)?;
    // Do not push if files or source changed during the upload.
    deployment.verify(sh, &dist)?;
    if git.output(sh, &["rev-parse", "HEAD"])?.trim() != deployment.commit {
        return Err("HEAD changed during the upload; nothing was pushed".into());
    }
    seal::push(sh, &git, &tag)?;
    publish(sh, &git, &dist, &deployment)?;
    files::remove_file(dist.join(MANIFEST))?;
    println!("published https://github.com/{REPOSITORY}/releases/tag/{tag}");
    Ok(())
}

fn pending(dist: &Path, fp: &str, head: &str, current: &str) -> Result<Option<Deployment>, String> {
    let file = dist.join(MANIFEST);
    if !file.exists() {
        return Ok(None);
    }
    let deployment: Deployment = serde_json::from_str(&files::read_to_string(&file)?).map_err(|e| e.to_string())?;
    if deployment.fingerprint != fp {
        return Err("a deploy is pending for different content; finish it or remove dist/deployment.json before starting a new deploy".into());
    }
    if (head == deployment.source_commit && deployment.version == following(current)?)
        || (head == deployment.commit && current == deployment.version)
        // Recovery if committing VERSION succeeded but saving the resulting SHA was interrupted.
        || (deployment.commit.is_empty() && current == deployment.version)
    {
        Ok(Some(deployment))
    } else {
        Err("dist/deployment.json belongs to a different commit; inspect the pending deploy before continuing".into())
    }
}

fn commit_version(sh: &mut dyn Shell, git: &Git, deployment: &mut Deployment) -> Result<(), String> {
    let head = git.output(sh, &["rev-parse", "HEAD"])?;
    if head.trim() == deployment.source_commit {
        files::write(git.dir().join("VERSION"), format!("{}\n", deployment.version))?;
        git.output(sh, &["add", "--", "VERSION"])?;
        let committed = git.output(
            sh,
            &[
                "commit",
                "-m",
                &format!("Release Dango {}", deployment.version),
                "--",
                "VERSION",
            ],
        );
        if let Err(error) = committed {
            // Undo only our own VERSION edit after, for example, a failed commit hook or signing request.
            if git.output(sh, &["rev-parse", "HEAD"])? == head
                && files::read_to_string(git.dir().join("VERSION"))?.trim() == deployment.version
            {
                git.output(
                    sh,
                    &["restore", "--source=HEAD", "--staged", "--worktree", "--", "VERSION"],
                )?;
            }
            return Err(error);
        }
    }
    let parent = git.output(sh, &["rev-parse", "HEAD^"])?;
    let changed = git.output(sh, &["diff", "--name-only", "HEAD^", "HEAD"])?;
    let committed_version = git.output(sh, &["show", "HEAD:VERSION"])?;
    if parent.trim() != deployment.source_commit
        || changed.trim() != "VERSION"
        || committed_version.trim() != deployment.version
    {
        return Err(
            "the release commit must change only VERSION to the notarized version above its original source".into(),
        );
    }
    deployment.commit = git.output(sh, &["rev-parse", "HEAD"])?.trim().into();
    if seal::ready(sh, git)? != deployment.fingerprint {
        return Err("the release commit changed the notarized content; nothing was pushed".into());
    }
    Ok(())
}

fn ensure_tag(sh: &mut dyn Shell, git: &Git, tag: &str, commit: &str) -> Result<(), String> {
    let listed = git.output(sh, &["tag", "--list", tag])?;
    if listed.trim().is_empty() {
        git.output(sh, &["tag", tag, commit])?;
    } else if git
        .output(sh, &["rev-parse", &format!("refs/tags/{tag}^{{commit}}")])?
        .trim()
        != commit
    {
        return Err(format!("{tag} points at another commit; refusing to replace it"));
    }
    Ok(())
}

fn gh(args: &[&str]) -> Command {
    let mut command = Command::new("gh");
    command.args(args).current_dir(root());
    command
}

fn stage(sh: &mut dyn Shell, dist: &Path, deployment: &Deployment) -> Result<(), String> {
    let tag = format!("v{}", deployment.version);
    let query = format!(".[] | select(.tagName == \"{tag}\") | .isDraft");
    let draft = sh.stdout(&mut gh(&[
        "release",
        "list",
        "--repo",
        REPOSITORY,
        "--limit",
        "1000",
        "--json",
        "tagName,isDraft",
        "--jq",
        &query,
    ]))?;
    match draft.trim() {
        "false" => return Err(format!("{tag} is already published; refusing to replace its assets")),
        "true" => {}
        "" => {
            sh.run(
                gh(&[
                    "release",
                    "create",
                    &tag,
                    "--repo",
                    REPOSITORY,
                    "--draft",
                    "--target",
                    "main",
                    "--title",
                    &format!("Dango {}", deployment.version),
                    "--notes-file",
                ])
                .arg(dist.join(format!("Dango-{}.md", deployment.version))),
            )?;
        }
        other => return Err(format!("unexpected draft status {other:?}")),
    }
    let mut upload = gh(&["release", "upload", &tag, "--repo", REPOSITORY, "--clobber"]);
    for file in deployment.files.keys() {
        upload.arg(dist.join(file));
    }
    upload.arg(dist.join(MANIFEST));
    sh.run(&mut upload)
}

/// Publishes the draft as latest, once origin's main is the release commit, so latest never moves backwards, and every
/// file of the draft has the checksum of its notarized original.
fn publish(sh: &mut dyn Shell, git: &Git, dist: &Path, deployment: &Deployment) -> Result<(), String> {
    let tag = format!("v{}", deployment.version);
    let main = git.output(sh, &["ls-remote", "origin", "refs/heads/main"])?;
    if main.split_whitespace().next() != Some(deployment.commit.as_str()) {
        return Err(format!("origin's main is not the release commit; {tag} stays a draft"));
    }
    let mut expected: BTreeMap<String, String> = deployment
        .files
        .iter()
        .map(|(file, digest)| (file.clone(), format!("sha256:{digest}")))
        .collect();
    let manifest = updates::sha256(sh, &dist.join(MANIFEST))?;
    expected.insert(MANIFEST.into(), format!("sha256:{manifest}"));
    let listed = sh.stdout(&mut gh(&[
        "release",
        "view",
        &tag,
        "--repo",
        REPOSITORY,
        "--json",
        "isDraft,assets",
        "--jq",
        r#"(.isDraft | tostring), (.assets[] | "\(.digest) \(.name)")"#,
    ]))?;
    let mut lines = listed.lines();
    if lines.next() != Some("true") {
        return Err(format!("{tag} is no longer a draft; refusing to publish it again"));
    }
    let uploaded: BTreeMap<String, String> = lines
        .filter_map(|line| line.split_once(' '))
        .map(|(digest, file)| (file.into(), digest.into()))
        .collect();
    if uploaded != expected {
        return Err(format!(
            "the draft's files differ from the notarized ones; {tag} stays a draft"
        ));
    }
    sh.run(&mut gh(&[
        "release",
        "edit",
        &tag,
        "--repo",
        REPOSITORY,
        "--target",
        &deployment.commit,
        "--draft=false",
        "--latest",
    ]))
}

#[cfg(test)]
#[path = "deploy_tests.rs"]
mod tests;
