//! The terminal command that opens folders and files in the app, like `code .` for VS Code.

use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertStyle};
use objc2_foundation::NSBundle;
use splat_i18n::tf;

use crate::util::ns;

/// Marks a command file as written by the app, so it never replaces one the user wrote.
const MARK: &str = "# Installed by";

/// Where the command goes: a folder in the user's PATH that needs no administrator.
fn target() -> PathBuf {
    ide_core::paths::home().join(".local/bin").join(ide_product::COMMAND)
}

/// The script behind the command; relative paths are resolved against the shell's folder before `open` sees them.
fn script(bundle_id: &str) -> String {
    format!(
        "#!/bin/sh\n{MARK} {name}; opens folders and files in it.\n[ \"$#\" -eq 0 ] && set -- .\nfor target in \"$@\"; do\n  \
         case \"$target\" in /*) ;; *) target=\"$PWD/$target\" ;; esac\n  open -b {bundle_id} \"$target\" || exit $?\ndone\n",
        name = ide_product::NAME
    )
}

/// Why the command was not installed.
#[derive(Debug)]
enum Refusal {
    Foreign(PathBuf),
    Io(std::io::Error),
}

fn write_command(path: &Path, bundle_id: &str) -> Result<(), Refusal> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && !existing.contains(MARK)
    {
        return Err(Refusal::Foreign(path.to_path_buf()));
    }
    let dir = path.parent().unwrap_or(Path::new("/"));
    std::fs::create_dir_all(dir).map_err(Refusal::Io)?;
    std::fs::write(path, script(bundle_id)).map_err(Refusal::Io)?;
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).map_err(Refusal::Io)
}

/// Whether `dir` is on the login shell's PATH.
fn on_path(dir: &Path) -> bool {
    crate::environment::get()
        .and_then(|env| env.iter().find(|(name, _)| name == "PATH"))
        .is_some_and(|(_, path)| path.split(':').any(|entry| Path::new(entry) == dir))
}

/// Installs the command and tells the user how it went.
pub fn install(mtm: MainThreadMarker) {
    let bundle_id = NSBundle::mainBundle()
        .bundleIdentifier()
        .map_or_else(|| ide_product::BUNDLE_ID.to_string(), |id| id.to_string());
    let path = target();
    let command = ide_product::COMMAND;
    let alert = NSAlert::new(mtm);
    match write_command(&path, &bundle_id) {
        Ok(()) => {
            alert.setMessageText(&ns(&tf("Der Befehl „{}“ ist installiert.", &[command])));
            let dir = path.parent().unwrap_or(Path::new("/"));
            let detail = if on_path(dir) {
                tf("„{} .“ öffnet den aktuellen Ordner im Terminal.", &[command])
            } else {
                tf(
                    "Er liegt in {}, das nicht in deinem PATH steht. Ergänze den Ordner in deiner Shell-Konfiguration.",
                    &[&crate::app::display_path(dir)],
                )
            };
            alert.setInformativeText(&ns(&detail));
        }
        Err(Refusal::Foreign(path)) => {
            alert.setAlertStyle(NSAlertStyle::Warning);
            alert.setMessageText(&ns(&tf("Der Befehl „{}“ wurde nicht installiert.", &[command])));
            alert.setInformativeText(&ns(&tf(
                "{} gibt es schon und stammt nicht von hier; es bleibt unverändert.",
                &[&crate::app::display_path(&path)],
            )));
        }
        Err(Refusal::Io(error)) => {
            alert.setAlertStyle(NSAlertStyle::Warning);
            alert.setMessageText(&ns(&tf("Der Befehl „{}“ wurde nicht installiert.", &[command])));
            alert.setInformativeText(&ns(&error.to_string()));
        }
    }
    alert.runModal();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> ide_core::testing::Scratch {
        ide_core::testing::scratch(&format!("command-{name}"))
    }

    #[test]
    fn the_script_is_valid_shell_and_names_the_bundle() {
        let dir = scratch("syntax");
        let path = dir.join("cmd");
        std::fs::write(&path, script("local.example")).unwrap();
        let status = std::process::Command::new("/bin/sh")
            .arg("-n")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(script("local.example").contains("open -b local.example"));
    }

    #[test]
    fn a_command_the_user_wrote_is_never_replaced() {
        let dir = scratch("foreign");
        let path = dir.join("bin/cmd");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "#!/bin/sh\necho mine\n").unwrap();
        assert!(matches!(
            write_command(&path, "local.example"),
            Err(Refusal::Foreign(_))
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "#!/bin/sh\necho mine\n");
    }

    #[test]
    fn our_own_command_is_written_executable_and_replaced_on_reinstall() {
        let dir = scratch("own");
        let path = dir.join("bin/cmd");
        write_command(&path, "local.one").unwrap();
        write_command(&path, "local.two").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("local.two") && !text.contains("local.one"));
        let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&path).unwrap().permissions());
        assert_eq!(mode & 0o777, 0o755);
    }
}
