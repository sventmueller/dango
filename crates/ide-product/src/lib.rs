//! The product's name and the identifiers its stored data hangs on, in one place for app, crates and build tool.
//!
//! The display name may change at any time. The identifiers below it name settings, keychain items and folders on
//! users' Macs; changing one of them after a release strands that data.

#![forbid(unsafe_code)]

/// What users read: menu titles, the About panel, the app bundle and the disk image.
pub const NAME: &str = "Dango";

/// The version of the release, from `[workspace.package]` of the workspace; the bundle's Info.plist carries the same.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The URL scheme the app answers, for sign-in callbacks.
pub const URL_SCHEME: &str = "dango";

/// The command the app installs for opening folders from a terminal.
pub const COMMAND: &str = "dango";

/// The bundle identifier; also the defaults domain, the keychain service and the name of the data folders.
///
/// ```
/// use ide_product::{BUNDLE_ID, NAME};
///
/// assert!(!BUNDLE_ID.to_lowercase().contains(&NAME.to_lowercase()));
/// ```
pub const BUNDLE_ID: &str = "local.agentide";

/// The product's repository on GitHub, where users report problems, suggest features and read the source.
pub const REPOSITORY: &str = "https://github.com/sventmueller/dango";

/// The token of the Homebrew Cask, which users install with; the disk image is named after `NAME`.
pub const CASK: &str = "dango";

/// The public half of the product's own EdDSA key, which every update Sparkle installs must be signed with (E51); its
/// private half stays in the owner's login keychain under the account `BUNDLE_ID`, created with Sparkle's
/// `generate_keys --account <BUNDLE_ID>`. A notarized release requires that key to match this public half.
pub const UPDATE_PUBLIC_KEY: &str = "7e5k+u8650uRidvnUWbtJTItzpDfrOVAubE+74OCEM8=";

/// The folder in a project that holds its settings for this app.
pub const PROJECT_DIR: &str = ".dango";

/// Earlier names of `PROJECT_DIR`, read when the current one is missing.
pub const PREVIOUS_PROJECT_DIRS: &[&str] = &[".vibeedit"];

/// The client id of the product's OAuth app on github.com, for signing in to CI through the browser (device flow).
/// Client ids are public. Empty until the app is registered, which leaves the browser login off.
pub const GITHUB_OAUTH_CLIENT_ID: &str = "";

/// The application id of the product's OAuth application on gitlab.com, for signing in to CI through the browser
/// (authorization code with PKCE, public client). Empty until it is registered, which leaves the browser login off.
pub const GITLAB_COM_OAUTH_APPLICATION_ID: &str = "";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_do_not_carry_the_display_name() {
        assert!(!BUNDLE_ID.to_lowercase().contains(&NAME.to_lowercase()));
    }

    #[test]
    fn the_cask_token_is_what_homebrew_accepts() {
        assert!(!CASK.is_empty());
        assert!(
            CASK.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
    }

    #[test]
    fn the_project_folder_is_hidden_and_never_reused() {
        assert!(PROJECT_DIR.starts_with('.'));
        assert!(!PREVIOUS_PROJECT_DIRS.contains(&PROJECT_DIR));
    }

    #[test]
    fn the_version_is_the_one_of_the_workspace() {
        let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml")).unwrap();
        let section = manifest.split("[workspace.package]").nth(1).unwrap();
        let line = section.lines().find(|line| line.starts_with("version = ")).unwrap();
        assert_eq!(line, format!("version = \"{VERSION}\""));
    }
}
