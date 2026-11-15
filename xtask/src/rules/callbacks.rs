//! The rules on the app's AppKit code that read single calls: no `mem::forget`, the body of every block the system
//! calls is a `guarded(..)` (or `contained(..)`) call, alone or before the constant the block returns, so that a panic
//! cannot unwind through the system's frames, and sizes come from `ide_ui_common::style`, not from literals.

use syn::{Expr, Stmt};

use crate::source::{Cx, Hooks, line};

/// The block constructors of block2, whose closures the system calls.
const BLOCKS: [&str; 2] = ["RcBlock", "StackBlock"];

/// The functions that catch a panic of the closure they are handed.
const GUARDS: [&str; 2] = ["guarded", "contained"];

/// The calls whose first argument is a size, by the end of their name.
const SIZED: [&str; 4] = [
    "FontOfSize",
    "FontOfSize_weight",
    "setSpacing",
    "setCustomSpacing_afterView",
];

/// The problems the hooks find.
#[derive(Default)]
pub(super) struct Callbacks {
    pub(super) problems: Vec<String>,
}

impl Hooks for Callbacks {
    fn expr(&mut self, cx: &Cx, expr: &Expr) {
        let relative = &cx.source.relative;
        match expr {
            Expr::Path(path) => {
                if cx
                    .resolve(&path.path)
                    .ends_with(&["mem".to_string(), "forget".to_string()])
                {
                    self.problems.push(format!(
                        "{relative}:{}: mem::forget leaks; keep the value where its owner drops it",
                        line(path.path.segments[0].ident.span())
                    ));
                }
            }
            Expr::Call(call) => {
                if let Expr::Path(path) = &*call.func {
                    let resolved = cx.resolve(&path.path);
                    if let [.., kind, new] = resolved.as_slice()
                        && BLOCKS.contains(&kind.as_str())
                        && new == "new"
                    {
                        self.block(cx, call);
                    }
                    if let Some(name) = resolved.last() {
                        self.sized(cx, name, call.args.first(), line(path.path.segments[0].ident.span()));
                    }
                }
            }
            Expr::MethodCall(call) => {
                self.sized(
                    cx,
                    &call.method.to_string(),
                    call.args.first(),
                    line(call.method.span()),
                );
            }
            _ => {}
        }
    }
}

impl Callbacks {
    fn block(&mut self, cx: &Cx, call: &syn::ExprCall) {
        let guarded = match call.args.first() {
            Some(Expr::Closure(closure)) => is_guarded(cx, &closure.body),
            _ => false,
        };
        if guarded {
            return;
        }
        let relative = &cx.source.relative;
        let at = match &*call.func {
            Expr::Path(path) => line(path.path.segments[0].ident.span()),
            _ => 0,
        };
        self.problems.push(format!(
            "{relative}:{at}: the body of a block the system calls is not one `guarded(..)` call, so a panic in it \
             would unwind through the system's frames"
        ));
    }

    fn sized(&mut self, cx: &Cx, name: &str, first: Option<&Expr>, at: usize) {
        let relative = &cx.source.relative;
        if relative.ends_with("/style.rs") || !SIZED.iter().any(|sized| name.ends_with(sized)) {
            return;
        }
        if first.is_some_and(is_number) {
            self.problems.push(format!(
                "{relative}:{at}: a literal size in `{name}(..)`; take a token from ide_ui_common::style"
            ));
        }
    }
}

/// Whether the closure body `body` is a call of a guard, or a block of nothing but one, perhaps followed by the
/// literal or constant the block returns, or empty.
fn is_guarded(cx: &Cx, body: &Expr) -> bool {
    let is_guard = |expr: &Expr| match expr {
        Expr::Call(call) => match &*call.func {
            Expr::Path(path) => cx
                .resolve(&path.path)
                .last()
                .is_some_and(|name| GUARDS.contains(&name.as_str())),
            _ => false,
        },
        _ => false,
    };
    match body {
        Expr::Block(block) => match block.block.stmts.as_slice() {
            [] => true,
            [Stmt::Expr(expr, _)] => is_guard(expr),
            [
                Stmt::Expr(expr, Some(_)),
                Stmt::Expr(Expr::Lit(_) | Expr::Path(_), None),
            ] => is_guard(expr),
            _ => false,
        },
        expr => is_guard(expr),
    }
}

fn is_number(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(lit) => matches!(lit.lit, syn::Lit::Int(_) | syn::Lit::Float(_)),
        Expr::Cast(cast) => is_number(&cast.expr),
        Expr::Paren(paren) => is_number(&paren.expr),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{snippets, walk};

    /// The problems of `code` at `path`, which is `src/a.rs` of the app crate or a file no crate reaches.
    fn problems(path: &str, code: &str) -> Vec<String> {
        let ws = snippets(&[("src/main.rs", "mod a;\n"), (path, code)]);
        let mut callbacks = Callbacks::default();
        walk(ws.file(path).unwrap(), Some(&ws.index), &mut callbacks);
        callbacks.problems
    }

    fn lines(found: &[String]) -> Vec<&str> {
        found.iter().map(|p| p.split(": ").next().unwrap()).collect()
    }

    #[test]
    fn a_leak_is_found_however_it_is_named() {
        let code = "use std::mem;\nuse std::mem::forget as drop_never;\nfn f() {\n    std::mem::forget(a);\n    \
                    mem::forget(b);\n    drop_never(c);\n    v.into_iter().for_each(mem::forget);\n    forget(d);\n    \
                    // mem::forget(e)\n    let s = \"mem::forget(f)\";\n}\nfn forget(_: u8) {}\n";
        assert_eq!(
            lines(&problems("src/a.rs", code)),
            ["src/a.rs:4", "src/a.rs:5", "src/a.rs:6", "src/a.rs:7"]
        );
    }

    #[test]
    fn a_block_body_must_be_one_guarded_call() {
        let code = "fn f() {\n    let a = RcBlock::new(move || {\n        go();\n    });\n    \
                    let b = RcBlock::new(move || guarded((), || go()));\n    let c = RcBlock::new(|_: Bool| {});\n    \
                    let d = block2::RcBlock::new(move |x| {\n        ide_ui_common::util::guarded((), || go(x));\n    });\n    \
                    let e = RcBlock::new(move || {\n        risky();\n        guarded((), || go());\n    });\n    \
                    let f = StackBlock::new(|| helper(|| guarded((), go)));\n    let g = RcBlock::new(handler);\n    \
                    let h = RcBlock::new(|| -> Bool { contained(|| go()).unwrap_or(Bool::NO) });\n    \
                    let i = RcBlock::new(move || -> Bool { guarded((), || go()); Bool::YES });\n    \
                    let j = RcBlock::new(move || -> Bool { guarded((), || go()); answer() });\n}\n";
        assert_eq!(
            lines(&problems("src/a.rs", code)),
            [
                "src/a.rs:2",
                "src/a.rs:10",
                "src/a.rs:14",
                "src/a.rs:15",
                "src/a.rs:16",
                "src/a.rs:18"
            ]
        );
    }

    #[test]
    fn a_renamed_guard_counts_and_a_lookalike_does_not() {
        let code = "use ide_ui_common::util::guarded as safe;\nfn f() {\n    let a = RcBlock::new(|| safe((), go));\n    \
                    let b = RcBlock::new(|| unguarded((), go));\n}\n";
        assert_eq!(lines(&problems("src/a.rs", code)), ["src/a.rs:4"]);
    }

    #[test]
    fn a_block_in_an_appkit_class_is_read_too() {
        let code = "define_class!(\n    struct V;\n    impl V {\n        #[unsafe(method(go))]\n        fn go(&self) {\n            \
                    let b = RcBlock::new(|| run());\n        }\n    }\n);\n";
        assert_eq!(lines(&problems("src/a.rs", code)), ["src/a.rs:6"]);
    }

    #[test]
    fn literal_sizes_are_found_outside_style() {
        let code = "fn f() {\n    let s = NSFont::systemFontOfSize(\n        13.0,\n    );\n    \
                    stack.setSpacing(style::space::M);\n    stack.setSpacing(4.0 as f64);\n    \
                    let m = NSFont::monospacedSystemFontOfSize_weight(12.0, w);\n}\n";
        assert_eq!(
            lines(&problems("src/a.rs", code)),
            ["src/a.rs:2", "src/a.rs:6", "src/a.rs:7"]
        );
        assert!(
            problems(
                "crates/ide-ui-common/src/style.rs",
                "fn f() { NSFont::systemFontOfSize(13.0); }"
            )
            .is_empty()
        );
    }
}
