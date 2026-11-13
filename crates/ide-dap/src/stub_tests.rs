//! Stand-ins for the programs the crate runs, such as `cargo`, `go` or `python3`: shell scripts in a folder of
//! their own that print what a test wants and log how they were called.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh folder for one test, without symbolic links in its path.
pub(crate) fn folder(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ide-dap-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(&dir).unwrap()
}

/// Writes `body` as the executable shell script `path`.
pub(crate) fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

/// The text `text` prints when a script runs it, without the shell reading anything in it.
pub(crate) fn printed(text: &str) -> String {
    format!("cat <<'STUB_END'\n{text}\nSTUB_END\n")
}

/// The login shell's environment the tests give: the stubs in `bin` first, then `/bin` for `cat` and `sleep`.
pub(crate) fn env(bin: &Path, more: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut env = vec![("PATH".to_string(), format!("{}:/bin", bin.display()))];
    env.extend(more.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())));
    env
}
