//! The ratchets of E197 on work that may wait on the main thread in the app's AppKit code: direct calls into
//! `std::fs` and `.recv()` on a channel, counted in production code however they are named.

use std::collections::BTreeMap;

use syn::Expr;

use crate::source::{Build, Cx, Hooks};

/// The calls of production code that may block, by kind.
#[derive(Default)]
pub(super) struct Blocking {
    pub(super) found: BTreeMap<&'static str, usize>,
}

impl Blocking {
    /// The calls into `std::fs`.
    pub(super) fn fs(&self) -> usize {
        self.found.get(FS).copied().unwrap_or(0)
    }

    /// The `.recv()` calls.
    pub(super) fn recv(&self) -> usize {
        self.found.get(RECV).copied().unwrap_or(0)
    }
}

const FS: &str = "std::fs";
const RECV: &str = "recv";

impl Hooks for Blocking {
    fn expr(&mut self, cx: &Cx, expr: &Expr) {
        if cx.build() != Build::Production {
            return;
        }
        let kind = match expr {
            Expr::Path(path)
                if cx
                    .resolve(&path.path)
                    .starts_with(&["std".to_string(), "fs".to_string()]) =>
            {
                FS
            }
            Expr::MethodCall(call) if call.method == "recv" && call.args.is_empty() => RECV,
            _ => return,
        };
        *self.found.entry(kind).or_default() += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{snippets, walk};

    #[test]
    fn calls_into_std_fs_and_recv_count_however_they_are_named_and_tests_do_not() {
        let ws = snippets(&[
            ("src/main.rs", "mod a;\n"),
            (
                "src/a.rs",
                "use std::fs::{self, File};\nuse std::fs::read as load;\nfn f() {\n    std::fs::write(p, b);\n    \
                 fs::read(p);\n    File::open(p);\n    load(p);\n    rx.recv();\n    rx.recv_timeout(d);\n    \
                 other::fs::read(p);\n}\n#[cfg(test)]\nmod tests {\n    fn t() { std::fs::read(p); rx.recv(); }\n}\n",
            ),
        ]);
        let mut hooks = Blocking::default();
        walk(ws.file("src/a.rs").unwrap(), Some(&ws.index), &mut hooks);
        assert_eq!((hooks.fs(), hooks.recv()), (4, 1));
    }
}
