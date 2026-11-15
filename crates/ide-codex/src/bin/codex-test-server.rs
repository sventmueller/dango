//! The stand-in of `ide_codex::test_server` as a `codex` executable: `--version` prints the release, `app-server`
//! speaks the protocol on stdin and stdout.

fn main() {
    let scenario = std::env::var("CODEX_TEST_SCENARIO").unwrap_or_else(|_| "basic".into());
    if std::env::args().any(|a| a == "--version") {
        let version = if scenario == "old" {
            "codex-cli 0.150.0"
        } else {
            ide_codex::test_server::VERSION
        };
        println!("{version}");
        return;
    }
    let log = std::env::var_os("CODEX_TEST_LOG").map(std::path::PathBuf::from);
    ide_codex::test_server::run(std::io::stdin().lock(), std::io::stdout().lock(), &scenario, log);
}
