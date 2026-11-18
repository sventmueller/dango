//! The rules on starting threads (E142): no result of spawning that is thrown away, so a thread that failed to start
//! is heard of, and in the app's production code no thread started past `util::spawn` or a lane, which would panic
//! when the system has no thread left.

use syn::{Expr, Pat};

use crate::source::{Build, Cx, Hooks, line};

/// The helper that starts the app's threads for one piece of work, the one place that may start them itself.
pub(super) const SPAWN_HELPER: &str = "crates/ide-ui-common/src/util/thread.rs";

/// The `let _ = ..spawn(..)` and `_ = ..spawn(..)` statements, whose thread may fail to start unheard.
#[derive(Default)]
pub(super) struct Discarded {
    pub(super) problems: Vec<String>,
}

impl Discarded {
    fn report(&mut self, cx: &Cx, at: usize) {
        self.problems.push(format!(
            "{}:{at}: the result of spawning is discarded; hand a failure to the caller or the user",
            cx.source.relative
        ));
    }
}

impl Hooks for Discarded {
    fn local(&mut self, cx: &Cx, local: &syn::Local) {
        if let (Pat::Wild(wild), Some(init)) = (&local.pat, &local.init)
            && spawns(&init.expr)
        {
            self.report(cx, line(wild.underscore_token.span));
        }
    }

    fn expr(&mut self, cx: &Cx, expr: &Expr) {
        if let Expr::Assign(assign) = expr
            && let Expr::Infer(wild) = &*assign.left
            && spawns(&assign.right)
        {
            self.report(cx, line(wild.underscore_token.span));
        }
    }
}

/// Whether the chain of calls `expr` passes through a `.spawn(..)`, whose `io::Result` it then holds or maps.
fn spawns(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(call) => call.method == "spawn" || spawns(&call.receiver),
        Expr::Try(inner) => spawns(&inner.expr),
        Expr::Paren(inner) => spawns(&inner.expr),
        _ => false,
    }
}

/// The threads production code starts itself; its tests and self-test hooks aside.
#[derive(Default)]
pub(super) struct Raw {
    pub(super) problems: Vec<String>,
}

impl Hooks for Raw {
    fn expr(&mut self, cx: &Cx, expr: &Expr) {
        if cx.build() != Build::Production || cx.source.relative == SPAWN_HELPER {
            return;
        }
        let Expr::Path(path) = expr else { return };
        let resolved = cx.resolve(&path.path);
        let names: Vec<&str> = resolved.iter().map(String::as_str).collect();
        if matches!(
            names.as_slice(),
            ["std", "thread", "spawn"] | ["std", "thread", "Builder", "new"]
        ) {
            self.problems.push(format!(
                "{}:{}: a thread started past util::spawn; use it, spawn_joined or a lane",
                cx.source.relative,
                line(path.path.segments[0].ident.span())
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{snippets, walk};

    fn found<H: Hooks + Default>(path: &str, code: &str, problems: impl Fn(H) -> Vec<String>) -> Vec<String> {
        let ws = snippets(&[("src/main.rs", "mod a;\n"), (path, code)]);
        let mut hooks = H::default();
        walk(ws.file(path).unwrap(), Some(&ws.index), &mut hooks);
        problems(hooks)
            .iter()
            .map(|p| p.split(": ").next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_discarded_spawn_is_found_and_a_handled_one_is_not() {
        let source = "fn f() {\n    let _ = std::thread::Builder::new()\n        .name(\"x\".into())\n        \
                      .spawn(move || {\n            a();\n        });\n    let _ = done; spawn(x);\n    \
                      if let Err(e) = Builder::new().spawn(f) { report(e) }\n    _ = Builder::new().spawn(g).map(drop);\n    \
                      let _ = run(|| { let h = b.spawn(x); });\n}\n";
        let lines = found("src/a.rs", source, |d: Discarded| d.problems);
        assert_eq!(lines, ["src/a.rs:2", "src/a.rs:9"]);
    }

    #[test]
    fn a_thread_started_past_the_helper_is_found_however_it_is_named() {
        let source = "use std::thread;\nuse std::thread::spawn as start;\nfn a() {\n    std::thread::spawn(|| {});\n    \
                      let b = thread::Builder::new();\n    start(|| {});\n    crate::util::spawn(|| {});\n}\n\
                      #[cfg(test)]\nmod tests {\n    fn c() { std::thread::spawn(|| {}); }\n}\n\
                      fn after_the_tests() { std::thread::spawn(|| {}); }\n";
        let lines = found("src/a.rs", source, |r: Raw| r.problems);
        assert_eq!(lines, ["src/a.rs:4", "src/a.rs:5", "src/a.rs:6", "src/a.rs:13"]);
        assert!(found(SPAWN_HELPER, source, |r: Raw| r.problems).is_empty());
    }
}
