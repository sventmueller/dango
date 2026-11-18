//! The test agent of `ide_acp::test_agent` on stdin and stdout; the first argument names the scenario.

fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "basic".into());
    ide_acp::test_agent::run(std::io::stdin().lock(), std::io::stdout().lock(), &scenario);
}
