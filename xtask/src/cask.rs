//! The Homebrew Cask (E2), which a tap of the owner's serves; Homebrew checks its checksum against the disk image.

use ide_product::{BUNDLE_ID, CASK, NAME};

use crate::*;

/// The product's web page, which the Cask names.
pub(crate) const HOMEPAGE: &str = "https://github.com/sventmueller/dango";

const DRY_RUN_HOMEPAGE: &str = "https://example.invalid/";

/// The homepage a run writes into the Cask.
pub(crate) fn homepage(dry_run: bool) -> Result<&'static str, String> {
    match (HOMEPAGE.is_empty(), dry_run) {
        (false, _) => Ok(HOMEPAGE),
        (true, true) => Ok(DRY_RUN_HOMEPAGE),
        (true, false) => Err("HOMEPAGE in xtask/src/cask.rs is empty: name the product's web page".into()),
    }
}

/// The Cask's file name in a tap's `Casks/` folder.
pub(crate) fn file_name() -> String {
    format!("{CASK}.rb")
}

/// What the Cask of a release says.
pub(crate) struct Release<'a> {
    pub(crate) version: &'a str,
    /// The disk image's SHA-256 checksum.
    pub(crate) sha256: &'a str,
    /// The releases page, see `updates::RELEASES`.
    pub(crate) releases: &'a str,
    pub(crate) homepage: &'a str,
}

/// The Cask. Sparkle updates the app in place, so Homebrew leaves updating to it (`auto_updates`); `zap` names the
/// folders `ide_core::paths` keeps the app's data in.
pub(crate) fn cask(release: &Release) -> String {
    let Release {
        version,
        sha256,
        releases,
        homepage,
    } = release;
    let image = updates::image_name("#{version}");
    let feed = updates::feed_url(releases);
    let description = "Editor for working with coding agents";
    format!(
        r#"cask "{CASK}" do
  version "{version}"
  sha256 "{sha256}"

  url "{releases}/download/v#{{version}}/{image}"
  name "{NAME}"
  desc "{description}"
  homepage "{homepage}"

  livecheck do
    url "{feed}"
    strategy :sparkle
  end

  auto_updates true
  depends_on macos: ">= :sonoma"

  app "{NAME}.app"

  zap trash: [
    "~/Library/Application Support/{BUNDLE_ID}",
    "~/Library/Caches/{BUNDLE_ID}",
    "~/Library/HTTPStorages/{BUNDLE_ID}",
    "~/Library/Logs/{BUNDLE_ID}",
    "~/Library/Preferences/{BUNDLE_ID}.plist",
    "~/Library/Saved Application State/{BUNDLE_ID}.savedState",
    "~/Library/WebKit/{BUNDLE_ID}",
  ]
end
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cask_downloads_the_image_of_its_version_and_lets_sparkle_update() {
        let cask = cask(&Release {
            version: "1.2.3",
            sha256: "ab12",
            releases: "https://h.example/releases",
            homepage: "https://h.example/",
        });
        assert!(cask.starts_with(&format!("cask \"{CASK}\" do\n  version \"1.2.3\"\n  sha256 \"ab12\"\n")));
        assert!(cask.contains(&format!(
            "  url \"https://h.example/releases/download/v#{{version}}/{}\"\n",
            updates::image_name("#{version}")
        )));
        assert!(
            cask.replace("#{version}", "1.2.3")
                .contains(&updates::image_url("https://h.example/releases", "1.2.3"))
        );
        assert!(
            cask.contains(
                "    url \"https://h.example/releases/latest/download/appcast.xml\"\n    strategy :sparkle\n"
            )
        );
        assert!(cask.contains("  auto_updates true\n  depends_on macos: \">= :sonoma\"\n"));
        assert!(cask.contains(&format!("  app \"{NAME}.app\"\n")));
        assert!(cask.contains(&format!("\"~/Library/Application Support/{BUNDLE_ID}\"")));
        assert!(
            !cask.contains("macOS"),
            "Homebrew's audit refuses the platform in the description"
        );
        assert_eq!(file_name(), format!("{CASK}.rb"));
    }

    #[test]
    fn a_release_is_built_for_real_only_once_the_homepage_is_known() {
        assert_eq!(homepage(false).is_err(), HOMEPAGE.is_empty());
        assert!(homepage(true).is_ok());
    }
}
