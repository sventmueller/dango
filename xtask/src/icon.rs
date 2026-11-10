//! The app icon: an Icon Composer document in `assets/`, compiled into the files every bundle carries.

use std::path::Path;
use std::process::Command;

use crate::*;

/// The Icon Composer document the icon is drawn in.
pub(crate) const ICON_SOURCE: &str = "assets/AppIcon.icon";

/// The compiled icon in `assets/`, checked in because only Xcode's actool can compile it and a build needs no Xcode:
/// the layered icon macOS 26 and later draw in `Assets.car`, the flat one for earlier versions in `AppIcon.icns`.
pub(crate) const ICON_FILES: [&str; 2] = ["Assets.car", "AppIcon.icns"];

/// The name of the icon in the asset catalog and of its icon file, which the property list points to.
pub(crate) const ICON_NAME: &str = "AppIcon";

/// Compiles `ICON_SOURCE` with actool into `assets/`.
pub(crate) fn compile_icon() -> Result<(), String> {
    let root = root();
    let out = target_dir().join("icon");
    if out.exists() {
        files::remove_dir_all(&out)?;
    }
    files::create_dir_all(&out)?;
    run(Command::new("xcrun")
        .args(["actool", "--compile"])
        .arg(&out)
        .args(["--platform", "macosx", "--minimum-deployment-target", MINIMUM_SYSTEM])
        .args(["--app-icon", ICON_NAME, "--include-all-app-icons"])
        .arg("--output-partial-info-plist")
        .arg(out.join("partial.plist"))
        .args(["--errors", "--warnings", "--output-format", "human-readable-text"])
        .arg(root.join(ICON_SOURCE)))?;
    for file in ICON_FILES {
        files::copy(out.join(file), root.join("assets").join(file))?;
    }
    Ok(())
}

/// Copies the compiled icon from `assets/` into a bundle's resources.
pub(crate) fn copy_icon(resources: &Path) -> Result<(), String> {
    let assets = root().join("assets");
    for file in ICON_FILES {
        let compiled = assets.join(file);
        if !compiled.exists() {
            return Err(format!(
                "{} is missing: `cargo xtask icon` compiles it from {ICON_SOURCE} (needs Xcode)",
                compiled.display()
            ));
        }
        files::copy(&compiled, resources.join(file))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_carries_the_icon_and_its_compiled_files() {
        let root = root();
        assert!(root.join(ICON_SOURCE).join("icon.json").exists());
        for file in ICON_FILES {
            assert!(root.join("assets").join(file).exists(), "{file}");
        }
    }
}
