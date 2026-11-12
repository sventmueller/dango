//! The fake adapter as a process, for the tests of who owns which process: it serves the script in the file its
//! first argument names, or the default script, on standard input and output, and may start a program to own.

fn main() {
    let script = std::env::args()
        .nth(1)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    ide_dap::fake::serve(script, std::io::stdin(), std::io::stdout().lock(), true);
}
