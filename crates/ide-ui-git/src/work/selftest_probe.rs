//! The self-test's hooks of `work.rs`.

/// Keeps the user's own git configuration, such as signing or hooks paths, out of a self-test.
pub(crate) fn for_selftest(mut environment: Vec<(String, String)>) -> Vec<(String, String)> {
    if std::env::var_os("IDE_SELFTEST").is_some() {
        environment.push(("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()));
        environment.push(("GIT_CONFIG_NOSYSTEM".into(), "1".into()));
    }
    environment
}
