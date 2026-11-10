//! Sparkle, the updater only the distributed disk image carries, where releases are downloaded from, and the update
//! feed installed copies read.

use std::path::{Path, PathBuf};
use std::process::Command;

use ide_product::{BUNDLE_ID, NAME, UPDATE_PUBLIC_KEY};

use crate::*;

/// Sparkle's GitHub release, pinned by the checksum of its archive; the same as Splat's and Lowspeak's.
pub(crate) const SPARKLE_VERSION: &str = "2.10.1";

const SPARKLE_SHA256: &str = "1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27";

/// The public key of a dry run, while `UPDATE_PUBLIC_KEY` is empty.
const DRY_RUN_UPDATE_KEY: &str = "DRY-RUN-NO-UPDATE-KEY";

/// Where Sparkle's tools keep the private key in the login keychain: their service, and an account of the app's own,
/// named after the neutral identifier so a new display name keeps the key (E18).
const UPDATE_KEY_SERVICE: &str = "https://sparkle-project.org";
pub(crate) const UPDATE_KEY_ACCOUNT: &str = BUNDLE_ID;

/// Where the distribution's Sparkle looks for updates, and the public key every update must be signed with.
pub(crate) struct Feed<'a> {
    pub(crate) url: &'a str,
    pub(crate) key: &'a str,
}

/// The releases page in GitHub's layout: each release's files under `/download/v<version>/`, the newest one's under
/// `/latest/download/`. Empty until the owner chooses where releases live; a dry run uses `DRY_RUN_RELEASES` instead,
/// and a notarized release refuses to start.
pub(crate) const RELEASES: &str = "";

const DRY_RUN_RELEASES: &str = "https://example.invalid/releases";

/// The feed's file name among a release's files.
pub(crate) const FEED: &str = "appcast.xml";

/// The releases page a run builds for.
pub(crate) fn releases(dry_run: bool) -> Result<&'static str, String> {
    match (RELEASES.is_empty(), dry_run) {
        (false, _) => Ok(RELEASES),
        (true, true) => Ok(DRY_RUN_RELEASES),
        (true, false) => Err("RELEASES in xtask/src/updates.rs is empty: name where releases are downloaded".into()),
    }
}

/// Where installed copies look for updates: the feed is a file of the newest release, so publishing a release
/// publishes the update.
pub(crate) fn feed_url(releases: &str) -> String {
    format!("{releases}/latest/download/{FEED}")
}

/// The disk image's file name, without spaces, which a download address would have to escape.
pub(crate) fn image_name(version: &str) -> String {
    format!("{}-{version}.dmg", NAME.replace(' ', "-"))
}

/// Where the disk image of `version` is downloaded from.
pub(crate) fn image_url(releases: &str, version: &str) -> String {
    format!("{releases}/download/v{version}/{}", image_name(version))
}

/// Sparkle's release archive, downloaded once into target/, checked against its pinned checksum and unpacked: the
/// framework and the `bin/` tools that sign updates.
pub(crate) fn sparkle() -> Result<PathBuf, String> {
    let target = target_dir();
    let dir = target.join(format!("sparkle-{SPARKLE_VERSION}"));
    if dir.join("Sparkle.framework").exists() {
        return Ok(dir);
    }
    files::create_dir_all(&target)?;
    let archive = target.join(format!("Sparkle-{SPARKLE_VERSION}.tar.xz"));
    let url = format!(
        "https://github.com/sparkle-project/Sparkle/releases/download/{SPARKLE_VERSION}/Sparkle-{SPARKLE_VERSION}.tar.xz"
    );
    run(Command::new("curl")
        .args(["-fsSL", "--proto", "=https", "--proto-redir", "=https", "-o"])
        .arg(&archive)
        .arg(&url))?;
    let actual = sha256(&archive)?;
    if actual != SPARKLE_SHA256 {
        files::remove_file(&archive)?;
        return Err(format!("{url} has checksum {actual}, expected {SPARKLE_SHA256}"));
    }
    let unpacking = target.join(format!("sparkle-{SPARKLE_VERSION}.partial"));
    if unpacking.exists() {
        files::remove_dir_all(&unpacking)?;
    }
    files::create_dir_all(&unpacking)?;
    run(Command::new("tar").arg("-xf").arg(&archive).arg("-C").arg(&unpacking))?;
    files::rename(&unpacking, &dir)?;
    files::remove_file(&archive)?;
    Ok(dir)
}

/// The framework inside a bundle.
pub(crate) fn framework(app: &Path) -> PathBuf {
    app.join("Contents/Frameworks/Sparkle.framework")
}

/// Sparkle's programs inside a bundle that carries it, the framework's own code first.
pub(crate) fn sparkle_programs(app: &Path) -> Vec<PathBuf> {
    let code = framework(app).join("Versions/B");
    vec![
        code.join("Sparkle"),
        code.join("Autoupdate"),
        code.join("Updater.app/Contents/MacOS/Updater"),
    ]
}

/// Puts the framework into the bundle at `app`.
pub(crate) fn embed_sparkle(app: &Path, sparkle: &Path) -> Result<(), String> {
    let framework = framework(app);
    files::create_dir_all(framework.parent().ok_or("no Frameworks folder")?)?;
    run(Command::new("ditto")
        .arg(sparkle.join("Sparkle.framework"))
        .arg(&framework))?;
    // Only a sandboxed app needs Sparkle's XPC services (E2: the app has no sandbox).
    files::remove_dir_all(framework.join("Versions/B/XPCServices"))?;
    files::remove_file(framework.join("XPCServices"))
}

/// Whether the login keychain holds an update signing key; asks only for the item's attributes, which needs no
/// permission, so no dialog comes up.
pub(crate) fn update_key_present() -> bool {
    Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            UPDATE_KEY_SERVICE,
            "-a",
            UPDATE_KEY_ACCOUNT,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Sparkle's tool that creates and shows the update signing key, where `sparkle` unpacks it.
pub(crate) fn generate_keys() -> PathBuf {
    target_dir().join(format!("sparkle-{SPARKLE_VERSION}/bin/generate_keys"))
}

/// What to do about a missing key: create it in the keychain, if `present` says it is not there, then copy its public
/// half into `UPDATE_PUBLIC_KEY`.
pub(crate) fn missing_key(present: bool) -> String {
    let tool = generate_keys().display().to_string();
    let copy = format!(
        "put the public key it prints into UPDATE_PUBLIC_KEY in crates/ide-product/src/lib.rs (`{tool} -p --account {UPDATE_KEY_ACCOUNT}` \
         prints it again)"
    );
    if present {
        format!("UPDATE_PUBLIC_KEY is empty: {copy}")
    } else {
        format!(
            "no update signing key under the keychain account {UPDATE_KEY_ACCOUNT}: create it with `{tool} --account \
             {UPDATE_KEY_ACCOUNT}`, keep a backup (`-x <file>`), then {copy}"
        )
    }
}

/// The public key a run builds with: in a dry run `UPDATE_PUBLIC_KEY` or a placeholder; otherwise `UPDATE_PUBLIC_KEY`, once the
/// login keychain holds its private half. macOS may ask once whether this copy of `generate_keys` may read it.
pub(crate) fn update_key(dry_run: bool) -> Result<&'static str, String> {
    if dry_run {
        return Ok(if UPDATE_PUBLIC_KEY.is_empty() {
            DRY_RUN_UPDATE_KEY
        } else {
            UPDATE_PUBLIC_KEY
        });
    }
    let present = update_key_present();
    if !present || UPDATE_PUBLIC_KEY.is_empty() {
        return Err(missing_key(present));
    }
    sparkle()?;
    let key = stdout(Command::new(generate_keys()).args(["-p", "--account", UPDATE_KEY_ACCOUNT]))?;
    if key.trim() != UPDATE_PUBLIC_KEY {
        return Err(format!(
            "the keychain's update signing key {} under {UPDATE_KEY_ACCOUNT} is not UPDATE_PUBLIC_KEY {UPDATE_PUBLIC_KEY}",
            key.trim()
        ));
    }
    Ok(UPDATE_PUBLIC_KEY)
}

/// The EdDSA signature and length of `image` for the feed, with the app's own key.
pub(crate) fn sign_update(image: &Path) -> Result<String, String> {
    let tool = sparkle()?.join("bin/sign_update");
    stdout(Command::new(tool).args(["--account", UPDATE_KEY_ACCOUNT]).arg(image))
}

/// What a release's feed entry says.
pub(crate) struct Entry<'a> {
    pub(crate) version: &'a str,
    /// The disk image's download address.
    pub(crate) url: &'a str,
    /// The `sparkle:edSignature` and `length` attributes `sign_update` printed for the image.
    pub(crate) signature: &'a str,
    /// The release date as RFC 822 wants it.
    pub(crate) date: &'a str,
    /// The notes in each language, English first, as Markdown.
    pub(crate) notes: &'a [(String, String)],
}

/// The update feed: the release's disk image with its EdDSA signature and length, and its notes in every language
/// there are notes in; Sparkle shows them in the language macOS picks for the app. Macs older than `MINIMUM_SYSTEM`
/// are not offered it.
pub(crate) fn appcast(entry: &Entry) -> Result<String, String> {
    let signature = entry.signature;
    if !signature.starts_with("sparkle:edSignature=\"") || !signature.contains(" length=\"") {
        return Err(format!("sign_update printed {signature:?}"));
    }
    let mut descriptions = String::new();
    for (language, text) in entry.notes {
        if text.contains("]]>") {
            return Err(format!(
                "the {language} notes contain ]]>, which would end the feed's text early"
            ));
        }
        descriptions.push_str(&format!(
            "      <description sparkle:format=\"markdown\" xml:lang=\"{language}\"><![CDATA[{}]]></description>\n",
            text.trim_end()
        ));
    }
    let (version, date, url) = (entry.version, entry.date, entry.url);
    Ok(format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>{NAME}</title>
    <item>
      <title>{NAME} {version}</title>
      <pubDate>{date}</pubDate>
      <sparkle:version>{version}</sparkle:version>
      <sparkle:shortVersionString>{version}</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>{MINIMUM_SYSTEM}</sparkle:minimumSystemVersion>
{descriptions}      <enclosure url="{url}" {signature} type="application/octet-stream"/>
    </item>
  </channel>
</rss>
"#
    ))
}

/// The SHA-256 checksum of a file, in hex.
pub(crate) fn sha256(path: &Path) -> Result<String, String> {
    let listed = stdout(Command::new("shasum").args(["-a", "256"]).arg(path))?;
    listed
        .split_whitespace()
        .next()
        .map(str::to_string)
        .ok_or_else(|| format!("no checksum for {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes() -> Vec<(String, String)> {
        vec![("en".into(), "- New.\n".into()), ("de".into(), "- Neu.\n".into())]
    }

    #[test]
    fn the_feed_offers_the_signed_image_with_its_notes_in_each_language() {
        let signature = "sparkle:edSignature=\"c2ln\" length=\"42\"";
        let url = image_url("https://h.example/releases", "1.0.4");
        let notes = notes();
        let entry = Entry {
            version: "1.0.4",
            url: &url,
            signature,
            date: "Sat, 07 Nov 2026 12:00:00 +0000",
            notes: &notes,
        };
        let feed = appcast(&entry).unwrap();
        assert!(feed.contains(&format!(
            "<enclosure url=\"https://h.example/releases/download/v1.0.4/{}\" \
             sparkle:edSignature=\"c2ln\" length=\"42\" type=\"application/octet-stream\"/>",
            image_name("1.0.4")
        )));
        assert!(feed.contains("<sparkle:version>1.0.4</sparkle:version>"));
        assert!(feed.contains("<sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion>"));
        assert!(feed.contains(
            "      <description sparkle:format=\"markdown\" xml:lang=\"en\"><![CDATA[- New.]]></description>\n\
             \x20     <description sparkle:format=\"markdown\" xml:lang=\"de\"><![CDATA[- Neu.]]></description>\n"
        ));
        assert!(
            appcast(&Entry {
                signature: "ERROR: no key",
                ..entry
            })
            .is_err()
        );
        let broken = vec![("en".to_string(), "a ]]> b".to_string())];
        assert!(
            appcast(&Entry {
                notes: &broken,
                ..entry
            })
            .is_err()
        );
    }

    #[test]
    fn the_app_signs_its_updates_with_a_key_of_its_own() {
        assert_eq!(UPDATE_KEY_ACCOUNT, BUNDLE_ID);
        assert_ne!(
            UPDATE_KEY_ACCOUNT, "ed25519",
            "the shared default account is Splat's and Lowspeak's"
        );
        assert!(!UPDATE_KEY_ACCOUNT.to_lowercase().contains(&NAME.to_lowercase()));
        assert_ne!(
            UPDATE_PUBLIC_KEY, "21sUy80rKTKlgQLPk+T7UrAkoqVQ+VV+6rfHKVAL4EU=",
            "Splat's and Lowspeak's key"
        );
        let expected = if UPDATE_PUBLIC_KEY.is_empty() {
            DRY_RUN_UPDATE_KEY
        } else {
            UPDATE_PUBLIC_KEY
        };
        assert_eq!(update_key(true).unwrap(), expected);
        let create = missing_key(false);
        assert!(
            create.contains(&format!("generate_keys --account {BUNDLE_ID}`")),
            "{create}"
        );
        assert!(create.contains("UPDATE_PUBLIC_KEY in crates/ide-product/src/lib.rs"));
        assert!(!missing_key(true).contains("create it"));
    }

    #[test]
    fn a_release_is_built_for_real_only_once_its_address_is_known() {
        assert_eq!(
            releases(true).unwrap(),
            if RELEASES.is_empty() {
                DRY_RUN_RELEASES
            } else {
                RELEASES
            }
        );
        assert_eq!(releases(false).is_err(), RELEASES.is_empty());
        assert_eq!(
            feed_url("https://h.example/r"),
            "https://h.example/r/latest/download/appcast.xml"
        );
        assert!(image_name("1.2.3").ends_with("-1.2.3.dmg") && !image_name("1.2.3").contains(' '));
    }
}
