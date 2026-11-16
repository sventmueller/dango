//! The stand-in of `ide_codex::test_server` as a `codex` executable: `--version` prints the release, `app-server`
//! speaks the protocol on stdin and stdout, `app-server proxy` as the shared server, and the CLI's Remote Control
//! commands answer as Codex's do.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(ide_codex::test_server::main(&args));
}
