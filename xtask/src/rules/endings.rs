//! How a window self-test scenario ends: at its end with `finish`, or before it only as a failed check with
//! `give_up` or on purpose with `skip`, so that the report tells the runner which happened. An early `finish` would
//! pass a scenario that stopped checking halfway.

use syn::visit::Visit;
use syn::{Expr, Stmt};

use crate::source::{Source, Workspace, line};

/// The call that ends a scenario as one that ran to its end.
const FINISH: &str = "finish";

/// The calls that end a scenario before its end as it should: as a failed check, or on purpose.
const EARLY: [&str; 3] = ["give_up", "give_up_saying", "skip"];

/// Whether the file at `relative` holds the self-test's scenarios.
pub(super) fn covers(relative: &str) -> bool {
    relative == "src/selftest.rs" || relative.starts_with("src/selftest/")
}

/// The calls in the scenario files of `ws` that end a scenario as if it ran to its end before it did: after
/// `return`, not as the last thing their function or closure does, or inside a branch. A call of a function that ends
/// with such a call counts as one, in its own file and in those it is visible to, so a loop ends by falling through
/// to its end.
pub(super) fn problems(ws: &Workspace) -> Vec<String> {
    let files: Vec<&Source> = ws.files.iter().filter(|file| covers(&file.relative)).collect();
    let shared = shared_ending(&files);
    let mut problems = Vec::new();
    for file in files {
        let mut ends = Ends {
            relative: &file.relative,
            ending: ending_functions(&file.syntax, &shared),
            problems: Vec::new(),
            position: Position::Inside,
            body: Body::Branch,
        };
        ends.visit_file(&file.syntax);
        problems.extend(ends.problems);
    }
    problems
}

/// A function of a file that has a body.
struct Function {
    name: String,
    /// Whether code outside its file may call it.
    visible: bool,
    /// The function its body calls last, as its own statement and outside any closure.
    tail_call: Option<String>,
}

/// The functions of a file, nested ones and methods included.
#[derive(Default)]
struct Functions(Vec<Function>);

impl Functions {
    fn add(&mut self, ident: &syn::Ident, vis: Option<&syn::Visibility>, body: &syn::Block) {
        let tail_call = match body.stmts.last() {
            Some(Stmt::Expr(Expr::Call(call), _)) => callee(call),
            _ => None,
        };
        self.0.push(Function {
            name: ident.to_string(),
            visible: vis.is_some_and(|vis| !matches!(vis, syn::Visibility::Inherited)),
            tail_call,
        });
    }
}

impl<'ast> Visit<'ast> for Functions {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        self.add(&function.sig.ident, Some(&function.vis), &function.block);
        syn::visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        self.add(&function.sig.ident, Some(&function.vis), &function.block);
        syn::visit::visit_impl_item_fn(self, function);
    }
}

fn functions(syntax: &syn::File) -> Vec<Function> {
    let mut functions = Functions::default();
    functions.visit_file(syntax);
    functions.0
}

/// The name of the function a call calls, by the last segment of its path; none for a call of a closure's result.
fn callee(call: &syn::ExprCall) -> Option<String> {
    match &*call.func {
        Expr::Path(path) => path.path.segments.last().map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

/// The functions of `files` that end a scenario and are visible beyond their file.
fn shared_ending(files: &[&Source]) -> Vec<String> {
    let mut shared: Vec<String> = Vec::new();
    loop {
        let mut found: Vec<String> = files
            .iter()
            .flat_map(|file| {
                let ending = ending_functions(&file.syntax, &shared);
                functions(&file.syntax)
                    .into_iter()
                    .filter(move |f| f.visible && ending.contains(&f.name))
                    .map(|f| f.name)
            })
            .filter(|name| !shared.contains(name))
            .collect();
        found.sort();
        found.dedup();
        if found.is_empty() {
            return shared;
        }
        shared.extend(found);
    }
}

/// `finish`, the functions of other files in `shared` that `syntax` does not define itself, and the functions of
/// `syntax` whose body ends with a call of one of them.
fn ending_functions(syntax: &syn::File, shared: &[String]) -> Vec<String> {
    let defined = functions(syntax);
    let mut ending = vec![FINISH.to_string()];
    ending.extend(
        shared
            .iter()
            .filter(|name| !defined.iter().any(|f| &f.name == *name))
            .cloned(),
    );
    loop {
        let more: Vec<String> = defined
            .iter()
            .filter(|f| !ending.contains(&f.name) && !EARLY.contains(&f.name.as_str()))
            .filter(|f| f.tail_call.as_ref().is_some_and(|called| ending.contains(called)))
            .map(|f| f.name.clone())
            .collect();
        if more.is_empty() {
            return ending;
        }
        ending.extend(more);
    }
}

/// Where an expression stands in what holds it.
#[derive(Clone, Copy)]
enum Position {
    /// Part of a larger expression, such as an argument or an initializer.
    Inside,
    /// What a `return` returns.
    Returned,
    /// A statement of a block; `last` if nothing follows it there.
    Statement { last: bool, body: Body },
    /// The body of a `match` arm.
    Arm,
    /// The whole body of a closure written without braces.
    ClosureBody,
}

/// What a block is the body of.
#[derive(Clone, Copy, PartialEq)]
enum Body {
    Function,
    Closure,
    /// Anything else: a branch of `if`, the `else` of `let`, an arm, a plain block.
    Branch,
}

/// Finds the calls of `ending` in a file that end its scenario early.
struct Ends<'a> {
    relative: &'a str,
    ending: Vec<String>,
    problems: Vec<String>,
    /// Where the next expression the walk enters stands.
    position: Position,
    /// What the next block the walk enters is the body of.
    body: Body,
}

impl Ends<'_> {
    fn check(&mut self, call: &syn::ExprCall, position: Position) {
        let Expr::Path(path) = &*call.func else { return };
        let Some(name) = path.path.segments.last().map(|segment| &segment.ident) else {
            return;
        };
        if !self.ending.iter().any(|ending| name == ending) {
            return;
        }
        let why = match position {
            Position::Returned => "after return",
            Position::Statement { last: false, .. } => "with more to do after it",
            Position::Statement { last: true, body } if body != Body::Branch => return,
            Position::Statement { .. } | Position::Arm => "inside a branch",
            Position::ClosureBody => return,
            Position::Inside => "inside an expression",
        };
        self.problems.push(format!(
            "{}:{}: {name} {why} ends the scenario early; a scenario that cannot go on ends with give_up(..), one \
             that has nothing to do here with skip(..)",
            self.relative,
            line(name.span())
        ));
    }
}

impl<'ast> Visit<'ast> for Ends<'_> {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        self.body = Body::Function;
        self.visit_block(&function.block);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        self.body = Body::Function;
        self.visit_block(&function.block);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let body = std::mem::replace(&mut self.body, Body::Branch);
        let count = block.stmts.len();
        for (at, stmt) in block.stmts.iter().enumerate() {
            match stmt {
                Stmt::Expr(expr, _) => {
                    self.position = Position::Statement {
                        last: at + 1 == count,
                        body,
                    };
                    self.visit_expr(expr);
                }
                stmt => {
                    self.position = Position::Inside;
                    self.visit_stmt(stmt);
                }
            }
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        let position = std::mem::replace(&mut self.position, Position::Inside);
        match expr {
            Expr::Call(call) => {
                self.check(call, position);
                syn::visit::visit_expr_call(self, call);
            }
            Expr::Return(returned) => {
                if let Some(value) = &returned.expr {
                    self.position = Position::Returned;
                    self.visit_expr(value);
                }
            }
            Expr::Closure(closure) => {
                if matches!(&*closure.body, Expr::Block(_)) {
                    self.body = Body::Closure;
                } else {
                    self.position = Position::ClosureBody;
                }
                self.visit_expr(&closure.body);
            }
            Expr::Match(matched) => {
                self.visit_expr(&matched.expr);
                for arm in &matched.arms {
                    if let syn::Pat::Guard(guarded) = &arm.pat {
                        self.visit_expr(&guarded.guard);
                    }
                    self.position = Position::Arm;
                    self.visit_expr(&arm.body);
                }
            }
            expr => syn::visit::visit_expr(self, expr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::snippets;

    fn found(source: &str) -> Vec<String> {
        problems(&snippets(&[("src/selftest/x.rs", source)]))
    }

    #[test]
    fn the_end_of_a_function_or_closure_finishes() {
        let source = "fn a() {\n    check(\"x\", true, \"\");\n    finish(NAME);\n}\n\
                      fn b() {\n    wait_until(ready, move |ok| {\n        check(\"y\", ok, \"\");\n        \
                      super::finish(NAME)\n    });\n}\n\
                      fn c() {\n    pictures(root, \"p\", |_| finish(NAME));\n}\n\
                      pub(super) fn d(view: &View) -> () {\n    go(view);\n    finish(NAME);\n}\n\
                      fn e() {\n    let ok = restore.finish();\n    check(\"z\", ok, \"\");\n    finish(NAME)\n}\n\
                      // fn f() { return finish(NAME); }\nconst S: &str = \"return finish(NAME);\";\n";
        assert!(found(source).is_empty(), "{:?}", found(source));
    }

    #[test]
    fn an_early_return_with_finish_is_found() {
        let source = "fn a() {\n    let Some(view) = open(\"pairs.rs\") else {\n        return finish(NAME);\n    };\n    \
                      go(view);\n}\nfn b() {\n    let Some(tab) = tab_of(key) else { return super::finish(NAME) };\n    \
                      go(tab);\n    finish(NAME);\n}\n";
        let problems = found(source);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].starts_with("src/selftest/x.rs:3: finish after return"));
        assert!(problems[1].starts_with("src/selftest/x.rs:8: finish after return"));
    }

    #[test]
    fn finish_in_a_branch_or_before_more_code_is_found() {
        let source = "fn a() {\n    if !ok {\n        finish(NAME);\n        return;\n    }\n    go();\n}\n\
                      fn b() {\n    match view {\n        Some(view) => go(view),\n        None => finish(NAME),\n    }\n}\n\
                      fn c() {\n    if running { then() } else { finish(NAME) }\n}\n\
                      fn d() {\n    let Some(x) = y else {\n        finish(NAME);\n        return;\n    };\n}\n\
                      fn e() {\n    finish(NAME);\n    go();\n}\n";
        let reasons: Vec<String> = found(source)
            .iter()
            .filter_map(|p| p.split(';').next().map(str::to_string))
            .collect();
        assert_eq!(
            reasons,
            [
                "src/selftest/x.rs:3: finish with more to do after it ends the scenario early",
                "src/selftest/x.rs:11: finish inside a branch ends the scenario early",
                "src/selftest/x.rs:15: finish inside a branch ends the scenario early",
                "src/selftest/x.rs:19: finish with more to do after it ends the scenario early",
                "src/selftest/x.rs:24: finish with more to do after it ends the scenario early",
            ]
        );
        assert!(found("fn a() {\n    go(finish(NAME));\n}\n")[0].contains("finish inside an expression"));
    }

    #[test]
    fn a_function_that_ends_with_finish_counts_as_finish() {
        let source = "fn done() {\n    save();\n    finish(NAME);\n}\nfn end() {\n    done()\n}\nfn a() {\n    if x {\n        \
                      return end();\n    }\n    go();\n    done();\n}\nfn b() {\n    wait_until(ready, |_| end());\n}\n";
        let problems = found(source);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("src/selftest/x.rs:10: end after return ends the scenario early"));
        assert_eq!(
            ending_functions(&syn::parse_file(source).unwrap(), &[]),
            ["finish", "done", "end", "a"]
        );
    }

    #[test]
    fn a_function_of_another_file_that_ends_counts_where_it_is_visible() {
        let lsp = "pub(super) fn done() {\n    save();\n    finish(NAME);\n}\nfn quiet() {\n    finish(NAME)\n}\n";
        let commands = "use super::lsp::done;\nfn a() {\n    let Some(view) = main_view() else { return done() };\n    \
                        go(view);\n    done();\n}\nfn b() {\n    if x {\n        return quiet();\n    }\n}\n";
        let own = "fn done() {\n    go();\n}\nfn a() {\n    if x {\n        return done();\n    }\n}\n";
        let problems = problems(&snippets(&[
            ("src/selftest/lsp.rs", lsp),
            ("src/selftest/lsp_commands.rs", commands),
            ("src/selftest/own.rs", own),
            ("src/editor/elsewhere.rs", "fn a() { return finish(NAME); }\n"),
        ]));
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("src/selftest/lsp_commands.rs:3: done after return"));
    }

    #[test]
    fn giving_up_and_skipping_may_end_a_scenario_anywhere() {
        let source = "fn a() {\n    let Some(view) = open(\"a.rs\") else {\n        return give_up(NAME, \"a.rs opens\");\n    \
                      };\n    if missing {\n        return skip(NAME, \"a server\", \"missing\");\n    }\n    \
                      match x { None => give_up_saying(NAME, \"x\", \"none\"), Some(x) => go(x) }\n}\n\
                      fn finish(self) -> bool { true }\n";
        assert!(found(source).is_empty(), "{:?}", found(source));
    }

    #[test]
    fn only_the_scenario_files_are_covered() {
        assert!(covers("src/selftest.rs") && covers("src/selftest/git/flows.rs"));
        assert!(!covers("src/editor/saving.rs") && !covers("src/editor/selftest_probe.rs"));
    }
}
