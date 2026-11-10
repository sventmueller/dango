//! Shell integration: small scripts that make zsh, bash and fish mark prompts and commands (OSC 133) and report
//! their working directory (OSC 7), loaded around the user's own startup files without changing them.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

const ZSHENV: &str = include_str!("shell/zshenv.zsh");
const ZSH: &str = include_str!("shell/integration.zsh");
const BASH: &str = include_str!("shell/integration.bash");
const FISH: &str = include_str!("shell/integration.fish");

/// The scripts and where they go below the integration folder.
const FILES: &[(&str, &str)] = &[
    ("zsh/.zshenv", ZSHENV),
    ("zsh/integration.zsh", ZSH),
    ("bash/integration.bash", BASH),
    ("fish/vendor_conf.d/ide-shell-integration.fish", FISH),
];

/// Where the app keeps the scripts: `shell-integration` in its cache folder.
pub fn default_dir() -> PathBuf {
    ide_core::paths::cache_dir().join("shell-integration")
}

/// Writes the scripts into `dir`, leaving files alone that are already current so running shells never read a
/// half-written one.
pub fn install(dir: &Path) -> io::Result<()> {
    for (name, content) in FILES {
        let path = dir.join(name);
        if std::fs::read(&path).is_ok_and(|current| current == content.as_bytes()) {
            continue;
        }
        let parent = path.parent().unwrap_or(dir);
        std::fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".{}.{}", std::process::id(), unique()));
        let written = std::fs::File::create(&temporary).and_then(|mut file| file.write_all(content.as_bytes()));
        match written.and_then(|()| std::fs::rename(&temporary, &path)) {
            Ok(()) => {}
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(error);
            }
        }
    }
    Ok(())
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// How to start a login shell: its arguments and the variables to set on top of the user's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Launch {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
}

/// Shells known to take `-l` for a login shell.
const LOGIN_FLAG: &[&str] = &[
    "zsh", "bash", "fish", "sh", "dash", "ksh", "mksh", "tcsh", "csh", "nu", "xonsh", "pwsh",
];

/// How to start `shell` as a login shell, with the scripts in `scripts` when given; `env` is the user's environment,
/// whose `ZDOTDIR` and `XDG_DATA_DIRS` the scripts restore.
pub(crate) fn launch(shell: &Path, scripts: Option<&Path>, env: &[(String, String)]) -> Launch {
    let name = shell
        .file_name()
        .map(|name| name.to_string_lossy().trim_start_matches('-').to_string())
        .unwrap_or_default();
    let original = |key: &str| env.iter().find(|(name, _)| name == key).map(|(_, value)| value.clone());
    let path = |relative: &str| scripts.map(|dir| dir.join(relative).to_string_lossy().into_owned());
    let login = || {
        if LOGIN_FLAG.contains(&name.as_str()) {
            vec!["-l".to_string()]
        } else {
            Vec::new()
        }
    };
    match (name.as_str(), scripts) {
        ("zsh", Some(_)) => {
            let mut env = vec![("ZDOTDIR".to_string(), path("zsh").unwrap_or_default())];
            if let Some(zdotdir) = original("ZDOTDIR") {
                env.push(("IDE_ZSH_ZDOTDIR".into(), zdotdir));
            }
            Launch { args: login(), env }
        }
        ("bash", Some(_)) => Launch {
            args: vec![
                "--init-file".into(),
                path("bash/integration.bash").unwrap_or_default(),
                "-i".into(),
            ],
            env: Vec::new(),
        },
        ("fish", Some(dir)) => {
            let previous = original("XDG_DATA_DIRS");
            // Without XDG_DATA_DIRS fish reads the specification's default, which has to stay in the list.
            let rest = previous.clone().unwrap_or_else(|| "/usr/local/share:/usr/share".into());
            let mut env = vec![("XDG_DATA_DIRS".to_string(), format!("{}:{rest}", dir.to_string_lossy()))];
            if let Some(previous) = previous {
                env.push(("IDE_FISH_XDG_DATA_DIRS".into(), previous));
            }
            Launch { args: login(), env }
        }
        _ => Launch {
            args: login(),
            env: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ide-terminal-{name}-{}-{}", std::process::id(), unique()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn zsh_gets_the_scripts_through_zdotdir_and_keeps_the_users_one_for_later() {
        let dir = Path::new("/cache/si");
        let plain = launch(Path::new("/bin/zsh"), Some(dir), &[]);
        assert_eq!(plain.args, ["-l"]);
        assert_eq!(plain.env, [("ZDOTDIR".to_string(), "/cache/si/zsh".to_string())]);
        let own = launch(
            Path::new("/bin/zsh"),
            Some(dir),
            &[("ZDOTDIR".into(), "/u/.config/zsh".into())],
        );
        assert!(own.env.contains(&("IDE_ZSH_ZDOTDIR".into(), "/u/.config/zsh".into())));
    }

    #[test]
    fn bash_reads_the_script_as_its_init_file() {
        let bash = launch(Path::new("/opt/homebrew/bin/bash"), Some(Path::new("/c")), &[]);
        assert_eq!(bash.args, ["--init-file", "/c/bash/integration.bash", "-i"]);
        assert!(bash.env.is_empty());
    }

    #[test]
    fn fish_finds_the_scripts_first_in_xdg_data_dirs() {
        let fish = launch(Path::new("/opt/homebrew/bin/fish"), Some(Path::new("/c")), &[]);
        assert_eq!(fish.args, ["-l"]);
        assert_eq!(
            fish.env,
            [(
                "XDG_DATA_DIRS".to_string(),
                "/c:/usr/local/share:/usr/share".to_string()
            )]
        );
        let own = launch(
            Path::new("/bin/fish"),
            Some(Path::new("/c")),
            &[("XDG_DATA_DIRS".into(), "/x".into())],
        );
        assert_eq!(
            own.env,
            [
                ("XDG_DATA_DIRS".to_string(), "/c:/x".to_string()),
                ("IDE_FISH_XDG_DATA_DIRS".to_string(), "/x".to_string())
            ]
        );
    }

    #[test]
    fn without_integration_or_for_other_shells_only_the_login_flag_is_passed() {
        assert_eq!(launch(Path::new("/bin/zsh"), None, &[]).args, ["-l"]);
        assert!(launch(Path::new("/bin/zsh"), None, &[]).env.is_empty());
        assert_eq!(launch(Path::new("/bin/bash"), None, &[]).args, ["-l"]);
        assert_eq!(
            launch(Path::new("/usr/local/bin/nu"), Some(Path::new("/c")), &[]).args,
            ["-l"]
        );
        assert!(
            launch(Path::new("/usr/local/bin/elvish"), Some(Path::new("/c")), &[])
                .args
                .is_empty()
        );
    }

    #[test]
    fn installing_writes_every_script_once_and_repairs_a_changed_one() {
        let dir = temporary("install");
        install(&dir).unwrap();
        for (name, content) in FILES {
            assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), *content);
        }
        let zshenv = dir.join("zsh/.zshenv");
        std::fs::write(&zshenv, "broken").unwrap();
        install(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&zshenv).unwrap(), ZSHENV);
        let leftovers = std::fs::read_dir(dir.join("zsh")).unwrap().count();
        assert_eq!(leftovers, 2, "no temporary files stay behind");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
