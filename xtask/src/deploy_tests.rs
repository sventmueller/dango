//! Recovery and publication boundaries without Apple or GitHub.

use super::*;
use crate::shell::fake::{Fake, scratch};

fn prepared_repository(name: &str) -> (std::path::PathBuf, Git, Deployment) {
    let (dir, git) = seal::tests::repository(name);
    let fp = seal::fingerprint(&mut System, &git, seal::Content::Head).unwrap();
    let prepared = Deployment {
        version: "0.1.1".into(),
        source_commit: git.output(&mut System, &["rev-parse", "HEAD"]).unwrap().trim().into(),
        commit: String::new(),
        fingerprint: fp,
        files: BTreeMap::new(),
        siblings: BTreeMap::new(),
    };
    (dir, git, prepared)
}

fn app() -> BTreeSet<String> {
    ["app".to_string()].into()
}

#[test]
fn an_interrupted_bump_is_undone_and_nothing_else() {
    let (dir, git, _) = prepared_repository("deploy-undo-bump");
    assert!(
        !undo_bump(&mut System, &git).unwrap(),
        "a clean tree has nothing to undo"
    );
    versions::bump(git.dir(), &app(), "0.1.1").unwrap();
    assert!(undo_bump(&mut System, &git).unwrap());
    assert_eq!(versions::read(git.dir()).unwrap(), "0.1.0");
    assert!(git.output(&mut System, &["status", "--porcelain"]).unwrap().is_empty());
    versions::bump(git.dir(), &app(), "0.1.1").unwrap();
    files::write(
        git.dir().join("Cargo.toml"),
        format!("{}# edited\n", seal::tests::MANIFEST.replace("0.1.0", "0.1.1")),
    )
    .unwrap();
    assert!(
        !undo_bump(&mut System, &git).unwrap(),
        "an edit beside the version stays"
    );
    files::write(git.dir().join("Cargo.toml"), seal::tests::MANIFEST).unwrap();
    files::write(git.dir().join("a.txt"), "changed\n").unwrap();
    assert!(
        !undo_bump(&mut System, &git).unwrap(),
        "a change elsewhere keeps the bump too"
    );
    files::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_version_commit_keeps_the_fingerprint_and_a_retry_never_bumps_it_again() {
    let (dir, git, mut prepared) = prepared_repository("deploy-version-commit");
    commit_version(&mut System, &git, &mut prepared, &app()).unwrap();
    let committed = prepared.commit.clone();
    assert_ne!(committed, prepared.source_commit);
    assert_eq!(versions::read(git.dir()).unwrap(), "0.1.1");
    assert!(
        files::read_to_string(git.dir().join("Cargo.lock"))
            .unwrap()
            .contains("version = \"0.1.1\"")
    );
    assert_eq!(
        git.output(&mut System, &["diff", "--name-only", "HEAD^", "HEAD"])
            .unwrap(),
        "Cargo.lock\nCargo.toml\n"
    );
    assert_eq!(seal::ready(&mut System, &git).unwrap(), prepared.fingerprint);
    commit_version(&mut System, &git, &mut prepared, &app()).unwrap();
    assert_eq!(prepared.commit, committed);
    assert!(
        git.output(&mut System, &["ls-remote", "origin"]).unwrap().is_empty(),
        "committing the version pushes nothing"
    );
    files::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_version_commit_restores_the_clean_source_for_a_retry() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, git, mut prepared) = prepared_repository("deploy-failed-commit");
    let hook = git.dir().join(".git/hooks/pre-commit");
    files::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(commit_version(&mut System, &git, &mut prepared, &app()).is_err());
    assert_eq!(
        git.output(&mut System, &["rev-parse", "HEAD"]).unwrap().trim(),
        prepared.source_commit
    );
    assert_eq!(versions::read(git.dir()).unwrap(), "0.1.0");
    assert_eq!(seal::ready(&mut System, &git).unwrap(), prepared.fingerprint);
    files::remove_file(&hook).unwrap();
    commit_version(&mut System, &git, &mut prepared, &app()).unwrap();
    files::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_version_tag_is_reused_only_for_the_same_commit() {
    let (dir, git, mut prepared) = prepared_repository("deploy-tags");
    commit_version(&mut System, &git, &mut prepared, &app()).unwrap();
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
        siblings: BTreeMap::new(),
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

fn github(
    main: &'static str,
    draft: &'static str,
    changed: Option<&'static str>,
) -> impl FnMut(&Command, &str) -> (i32, String) {
    move |_: &Command, line: &str| {
        if line.contains("ls-remote origin refs/heads/main") {
            (0, format!("{main}\trefs/heads/main\n"))
        } else if line.starts_with("shasum") {
            (0, "manifest  /dist/deployment.json\n".into())
        } else if line.contains("release view") {
            let files = assets("0.1.3").into_iter().map(|file| {
                let digest = if Some(file.as_str()) == changed {
                    "other"
                } else {
                    "digest"
                };
                format!("sha256:{digest} {file}\n")
            });
            (
                0,
                format!("{draft}\n{}sha256:manifest {MANIFEST}\n", files.collect::<String>()),
            )
        } else {
            (0, String::new())
        }
    }
}

#[test]
fn publishing_needs_the_release_commit_on_origin_and_every_notarized_file_in_the_draft() {
    let git = Git::at(Path::new("/repo"));
    let mut sh = Fake::new(github("release", "true", None));
    publish(&mut sh, &git, Path::new("/dist"), &deployment()).unwrap();
    let edit = sh.commands.last().unwrap();
    assert!(
        edit.contains("release edit v0.1.3") && edit.contains("--target release --draft=false --latest"),
        "{edit}"
    );
    for (main, draft, changed, refusal) in [
        ("newer", "true", None, "not the release commit"),
        ("release", "true", Some("appcast.xml"), "differ from the notarized ones"),
        ("release", "false", None, "no longer a draft"),
    ] {
        let mut sh = Fake::new(github(main, draft, changed));
        let error = publish(&mut sh, &git, Path::new("/dist"), &deployment()).unwrap_err();
        assert!(error.contains(refusal), "{error}");
        assert!(!sh.commands.iter().any(|c| c.contains("release edit")));
    }
}

#[test]
fn only_a_feed_that_offers_the_signed_image_of_the_version_passes() {
    let dir = scratch("deploy-feed");
    files::write(dir.join("Dango.dmg"), [0u8; 42]).unwrap();
    let url = updates::image_url(updates::RELEASES, "0.1.3");
    let feed = |version: &str, signature: &str, length: u64| {
        format!(
            "<sparkle:version>{version}</sparkle:version>\n<enclosure url=\"{url}\" sparkle:edSignature=\"{signature}\" \
             length=\"{length}\" type=\"application/octet-stream\"/>"
        )
    };
    files::write(dir.join(updates::FEED), feed("0.1.3", "c2ln", 42)).unwrap();
    deployment().check_feed(&dir).unwrap();
    for wrong in [
        feed("0.1.2", "c2ln", 42),
        feed("0.1.3", "DRY-RUN-NOT-SIGNED", 42),
        feed("0.1.3", "c2ln", 41),
    ] {
        files::write(dir.join(updates::FEED), wrong).unwrap();
        assert!(deployment().check_feed(&dir).is_err());
    }
    files::remove_dir_all(&dir).unwrap();
}
