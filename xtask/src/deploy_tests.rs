//! Recovery and publication boundaries without Apple or GitHub.

use super::*;
use crate::shell::fake::{Fake, scratch};

fn sealed_repository(name: &str) -> (std::path::PathBuf, Git, Deployment) {
    let (dir, git) = seal::tests::repository(name);
    let fp = seal::fingerprint(&mut System, &git, seal::Content::Head).unwrap();
    seal::mint(&mut System, &git, seal::Seal::Green, &fp).unwrap();
    let prepared = Deployment {
        version: "0.1.1".into(),
        source_commit: git.output(&mut System, &["rev-parse", "HEAD"]).unwrap().trim().into(),
        commit: String::new(),
        fingerprint: fp,
        files: BTreeMap::new(),
    };
    (dir, git, prepared)
}

#[test]
fn the_version_commit_inherits_the_seal_and_a_retry_never_bumps_it_again() {
    let (dir, git, mut prepared) = sealed_repository("deploy-version-commit");
    commit_version(&mut System, &git, &mut prepared).unwrap();
    let committed = prepared.commit.clone();
    assert_ne!(committed, prepared.source_commit);
    assert_eq!(files::read_to_string(git.dir().join("VERSION")).unwrap(), "0.1.1\n");
    assert_eq!(
        git.output(&mut System, &["diff", "--name-only", "HEAD^", "HEAD"])
            .unwrap()
            .trim(),
        "VERSION"
    );
    assert_eq!(seal::ready(&mut System, &git).unwrap(), prepared.fingerprint);
    commit_version(&mut System, &git, &mut prepared).unwrap();
    assert_eq!(prepared.commit, committed);
    assert!(
        git.output(&mut System, &["ls-remote", "origin"]).unwrap().is_empty(),
        "committing the version pushes nothing"
    );
    files::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_version_commit_restores_the_clean_sealed_source_for_a_retry() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, git, mut prepared) = sealed_repository("deploy-failed-commit");
    let hook = git.dir().join(".git/hooks/pre-commit");
    files::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(commit_version(&mut System, &git, &mut prepared).is_err());
    assert_eq!(
        git.output(&mut System, &["rev-parse", "HEAD"]).unwrap().trim(),
        prepared.source_commit
    );
    assert_eq!(files::read_to_string(git.dir().join("VERSION")).unwrap(), "0.1.0\n");
    assert_eq!(seal::ready(&mut System, &git).unwrap(), prepared.fingerprint);
    files::remove_file(&hook).unwrap();
    commit_version(&mut System, &git, &mut prepared).unwrap();
    files::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_version_tag_is_reused_only_for_the_same_commit() {
    let (dir, git, mut prepared) = sealed_repository("deploy-tags");
    commit_version(&mut System, &git, &mut prepared).unwrap();
    ensure_tag(&mut System, &git, "v0.1.1", &prepared.commit).unwrap();
    ensure_tag(&mut System, &git, "v0.1.1", &prepared.commit).unwrap();
    assert!(ensure_tag(&mut System, &git, "v0.1.1", &prepared.source_commit).is_err());
    files::remove_dir_all(&dir).unwrap();
}

fn deployment() -> Deployment {
    Deployment {
        version: "0.1.3".into(),
        source_commit: "source".into(),
        commit: "release".into(),
        fingerprint: "green".into(),
        files: assets("0.1.3")
            .into_iter()
            .map(|name| (name, "digest".into()))
            .collect(),
    }
}

#[test]
fn interrupted_uploads_resume_the_same_version_only_for_the_same_content() {
    let dir = scratch("deploy-resume");
    deployment().save(&dir).unwrap();
    assert!(pending(&dir, "green", "source", "0.1.2").unwrap().is_some());
    assert!(pending(&dir, "green", "release", "0.1.3").unwrap().is_some());
    assert!(pending(&dir, "different", "release", "0.1.3").is_err());
    assert!(pending(&dir, "green", "another-commit", "0.1.3").is_err());
    assert!(pending(&dir, "green", "source", "0.1.1").is_err());
    files::remove_dir_all(&dir).unwrap();
}

#[test]
fn changed_or_missing_assets_stop_a_resumed_deploy() {
    let mut sh = Fake::new(|_: &Command, _: &str| (0, "digest  file\n".into()));
    deployment().verify(&mut sh, Path::new("/dist")).unwrap();
    let mut changed = Fake::new(|_: &Command, _: &str| (0, "changed  file\n".into()));
    assert!(
        deployment()
            .verify(&mut changed, Path::new("/dist"))
            .unwrap_err()
            .contains("changed after notarization")
    );
    let mut incomplete = deployment();
    incomplete.files.remove("Dango.dmg");
    assert!(incomplete.verify(&mut sh, Path::new("/dist")).is_err());
}

#[test]
fn staging_uploads_every_file_into_a_draft_and_never_publishes_it() {
    let mut sh = Fake::new(|_: &Command, _: &str| (0, String::new()));
    stage(&mut sh, Path::new("/dist"), &deployment()).unwrap();
    assert!(sh.commands[1].contains("--draft --target main"));
    let upload = sh.commands.last().unwrap();
    for file in assets("0.1.3").into_iter().chain([MANIFEST.into()]) {
        assert!(upload.contains(&format!("/dist/{file}")), "{upload}");
    }
    assert!(
        !sh.commands
            .iter()
            .any(|c| c.contains("--draft=false") || c.contains("git push"))
    );
}

#[test]
fn a_published_release_is_never_overwritten_and_a_failed_upload_stops() {
    let mut published = Fake::new(|_: &Command, _: &str| (0, "false\n".into()));
    assert!(
        stage(&mut published, Path::new("/dist"), &deployment())
            .unwrap_err()
            .contains("already published")
    );
    assert_eq!(published.commands.len(), 1);
    let mut failed = Fake::new(|_: &Command, line: &str| {
        if line.contains("release upload") {
            (1, String::new())
        } else {
            (0, "true\n".into())
        }
    });
    assert!(stage(&mut failed, Path::new("/dist"), &deployment()).is_err());
    assert_eq!(failed.commands.len(), 2, "the existing draft is reused");
}
