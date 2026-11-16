//! The environment of a work order's sessions and gates (E186): a short list of the login shell's variables instead of
//! all of them, without the SSH agent and anything named like a credential, and with git's credential helpers off, so
//! a builder can neither push with the user's credentials nor read a token from its environment (C49).
//!
//! The environment alone keeps no credential from the model: files and sockets that hold them are closed by the
//! sandbox's read and socket rules (`sandbox::Profile::gate`, C50).

/// The variables an order keeps, besides every `LC_*`: the path, the user, the language, the terminal and where
/// toolchains live.
pub const KEPT: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "LANGUAGE",
    "TMPDIR",
    "TERM",
    "TZ",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "GOPATH",
    "GOROOT",
    "GOMODCACHE",
    "GOCACHE",
    "GOFLAGS",
    "NVM_DIR",
    "NVM_BIN",
    "PNPM_HOME",
    "BUN_INSTALL",
    "DENO_INSTALL",
    "VOLTA_HOME",
    "JAVA_HOME",
    "PYENV_ROOT",
    "RBENV_ROOT",
    "SDKMAN_DIR",
    "HOMEBREW_PREFIX",
    "HOMEBREW_CELLAR",
    "HOMEBREW_REPOSITORY",
    "DEVELOPER_DIR",
    "SDKROOT",
];

/// The endings of names that hold credentials, which go even when a list names them.
const SECRET_ENDINGS: [&str; 4] = ["_TOKEN", "_KEY", "_SECRET", "_PASSWORD"];

/// The variables that always go: the SSH agent's socket hands out the user's keys.
const DROPPED: [&str; 1] = ["SSH_AUTH_SOCK"];

/// Whether a variable called `name` goes from an order's environment whatever a list says.
pub fn is_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    DROPPED.contains(&upper.as_str()) || SECRET_ENDINGS.iter().any(|ending| upper.ends_with(ending))
}

/// The environment of an order's sessions and gates, from the login shell's `login`: the variables of [`KEPT`], every
/// `LC_*` and those named in `extra`, such as a project's build cache, without the ones [`is_secret`] names; then
/// `GIT_CONFIG_*` entries that give git an empty `credential.helper`, which drops every helper configured before.
///
/// ```
/// use ide_process::order_env::for_orders;
///
/// let login = [
///     ("PATH".to_string(), "/usr/bin".to_string()),
///     ("GITHUB_TOKEN".to_string(), "x".to_string()),
///     ("SSH_AUTH_SOCK".to_string(), "/tmp/agent".to_string()),
/// ];
/// let env = for_orders(&login, &[]);
/// assert!(env.contains(&("PATH".to_string(), "/usr/bin".to_string())));
/// assert!(!env.iter().any(|(name, _)| name == "GITHUB_TOKEN" || name == "SSH_AUTH_SOCK"));
/// ```
pub fn for_orders(login: &[(String, String)], extra: &[&str]) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = login
        .iter()
        .filter(|(name, _)| KEPT.contains(&name.as_str()) || name.starts_with("LC_") || extra.contains(&name.as_str()))
        .filter(|(name, _)| !is_secret(name))
        .cloned()
        .collect();
    for (name, value) in [
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "credential.helper"),
        ("GIT_CONFIG_VALUE_0", ""),
        ("GIT_TERMINAL_PROMPT", "0"),
    ] {
        env.push((name.to_string(), value.to_string()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login() -> Vec<(String, String)> {
        [
            ("PATH", "/opt/homebrew/bin:/usr/bin"),
            ("HOME", "/Users/a"),
            ("LC_CTYPE", "UTF-8"),
            ("CARGO_HOME", "/Users/a/.cargo"),
            ("SSH_AUTH_SOCK", "/private/tmp/agent"),
            ("GITLAB_TOKEN", "glpat-x"),
            ("OPENAI_API_KEY", "sk-x"),
            ("AWS_SECRET", "s"),
            ("PGPASSWORD", "p"),
            ("DB_PASSWORD", "p"),
            ("EDITOR", "vim"),
            ("SCCACHE_DIR", "/cache"),
            ("NPM_TOKEN", "n"),
            ("GIT_CONFIG_COUNT", "3"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    fn names(env: &[(String, String)]) -> Vec<&str> {
        env.iter().map(|(k, _)| k.as_str()).collect()
    }

    #[test]
    fn an_order_keeps_the_list_and_loses_credentials() {
        let env = for_orders(&login(), &["SCCACHE_DIR", "NPM_TOKEN"]);
        let names = names(&env);
        for kept in ["PATH", "HOME", "LC_CTYPE", "CARGO_HOME", "SCCACHE_DIR"] {
            assert!(names.contains(&kept), "{kept}: {names:?}");
        }
        for gone in [
            "SSH_AUTH_SOCK",
            "GITLAB_TOKEN",
            "OPENAI_API_KEY",
            "AWS_SECRET",
            "DB_PASSWORD",
            "EDITOR",
            "NPM_TOKEN",
            "PGPASSWORD",
        ] {
            assert!(!names.contains(&gone), "{gone}: {names:?}");
        }
    }

    #[test]
    fn git_gets_an_empty_credential_helper_and_no_inherited_configuration() {
        let env = for_orders(&login(), &[]);
        let get = |name: &str| {
            env.iter()
                .filter(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            get("GIT_CONFIG_COUNT"),
            ["1"],
            "the login shell's own entries are not kept"
        );
        assert_eq!(get("GIT_CONFIG_KEY_0"), ["credential.helper"]);
        assert_eq!(get("GIT_CONFIG_VALUE_0"), [""]);
    }

    #[test]
    fn secret_names_are_told_by_their_ending_in_any_case() {
        assert!(is_secret("github_token") && is_secret("SSH_AUTH_SOCK") && is_secret("X_KEY"));
        assert!(!is_secret("KEYCHAIN_DIR") && !is_secret("TOKENIZERS_PARALLELISM") && !is_secret("PATH"));
    }
}
