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
    compile_icon_in(&mut System, &root(), &target_dir().join("icon"))
}

/// `compile_icon` for the workspace at `root`, compiling into `out`.
fn compile_icon_in(sh: &mut dyn Shell, root: &Path, out: &Path) -> Result<(), String> {
    if out.exists() {
        files::remove_dir_all(out)?;
    }
    files::create_dir_all(out)?;
    sh.run(
        Command::new("xcrun")
            .args(["actool", "--compile"])
            .arg(out)
            .args(["--platform", "macosx", "--minimum-deployment-target", MINIMUM_SYSTEM])
            .args(["--app-icon", ICON_NAME, "--include-all-app-icons"])
            .arg("--output-partial-info-plist")
            .arg(out.join("partial.plist"))
            .args(["--errors", "--warnings", "--output-format", "human-readable-text"])
            .arg(root.join(ICON_SOURCE)),
    )?;
    for file in ICON_FILES {
        files::copy(out.join(file), root.join("assets").join(file))?;
    }
    Ok(())
}

/// Copies the compiled icon from `assets/` into a bundle's resources.
pub(crate) fn copy_icon(resources: &Path) -> Result<(), String> {
    copy_icon_from(&root().join("assets"), resources)
}

/// `copy_icon` from the folder `assets`.
fn copy_icon_from(assets: &Path, resources: &Path) -> Result<(), String> {
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

    use crate::shell::fake::{Fake, arg, scratch};

    #[test]
    fn actool_compiles_the_document_into_the_assets_folder() {
        let root = scratch("icon-compile");
        let out = root.join("target/icon");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("left-over"), "").unwrap();
        let mut sh = Fake::new(|cmd: &Command, _: &str| {
            let compiled = arg(cmd, Some("--compile"));
            for file in ICON_FILES {
                std::fs::write(compiled.join(file), file).unwrap();
            }
            (0, String::new())
        });
        compile_icon_in(&mut sh, &root, &out).unwrap();
        assert!(!out.join("left-over").exists());
        for file in ICON_FILES {
            assert_eq!(std::fs::read_to_string(root.join("assets").join(file)).unwrap(), file);
        }
        let line = &sh.commands[0];
        assert!(
            line.starts_with(&format!("xcrun actool --compile {}", out.display())),
            "{line}"
        );
        assert!(line.contains(&format!(
            "--minimum-deployment-target {MINIMUM_SYSTEM} --app-icon AppIcon"
        )));
        assert!(line.ends_with(&root.join(ICON_SOURCE).display().to_string()));

        let mut failing = Fake::new(|_: &Command, _: &str| (1, String::new()));
        assert!(compile_icon_in(&mut failing, &root, &out).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_bundle_gets_the_compiled_icon_or_a_hint_how_to_compile_it() {
        let dir = scratch("icon-copy");
        let resources = dir.join("Resources");
        std::fs::create_dir_all(&resources).unwrap();
        copy_icon_from(&root().join("assets"), &resources).unwrap();
        for file in ICON_FILES {
            assert!(resources.join(file).is_file(), "{file}");
        }
        let missing = copy_icon_from(&dir.join("assets"), &resources).unwrap_err();
        assert!(missing.ends_with("is missing: `cargo xtask icon` compiles it from assets/AppIcon.icon (needs Xcode)"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_repository_carries_the_icon_and_its_compiled_files() {
        let root = root();
        assert!(root.join(ICON_SOURCE).join("icon.json").exists());
        for file in ICON_FILES {
            assert!(root.join("assets").join(file).exists(), "{file}");
        }
    }
}
