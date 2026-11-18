//! How errors reach the user in the app's AppKit code (E141): as typed errors that explain themselves
//! (`ide_core::explain::Explain`), never as a bare `String` whose language and origin nobody can tell, and never in
//! their `Debug` form.

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;
use syn::{Expr, GenericArgument, PathArguments, Type};

use crate::source::{Build, Cx, Hooks, line};

/// Calls whose arguments the user reads, or that carry an error on to where the user reads it, by the end of their
/// name (`alert` stands for `show_alert` too), the last two segments where they have two.
const SHOWN: [&str; 16] = [
    "setInformativeText",
    "setMessageText",
    "Alert::new",
    "informative",
    "warn",
    "confirm",
    "setStringValue",
    "setToolTip",
    "alert",
    "notify",
    "set_status",
    "show_note",
    "Problem::plain",
    "Problem::Start",
    "Failed",
    "map_err",
];

/// What breaks the rules on errors in production code; tests may compare errors as they like.
#[derive(Default)]
pub(super) struct Errors {
    pub(super) problems: Vec<String>,
}

impl Hooks for Errors {
    fn path(&mut self, cx: &Cx, path: &syn::Path) {
        if cx.build() != Build::Production {
            return;
        }
        for segment in path.segments.iter().filter(|segment| segment.ident == "Result") {
            let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                continue;
            };
            if arguments.args.len() >= 2
                && let Some(GenericArgument::Type(Type::Path(error))) = arguments.args.last()
                && error.qself.is_none()
                && error
                    .path
                    .segments
                    .last()
                    .is_some_and(|last| last.ident == "String" && last.arguments.is_none())
            {
                self.problems.push(format!(
                    "{}:{}: an error as a bare String; give it a type that implements Explain, or Explained for one \
                     already in the user's words",
                    cx.source.relative,
                    line(segment.ident.span())
                ));
            }
        }
    }

    fn expr(&mut self, cx: &Cx, expr: &Expr) {
        if cx.build() != Build::Production {
            return;
        }
        let (names, args, at): (Vec<String>, _, _) = match expr {
            Expr::Call(call) => match &*call.func {
                Expr::Path(path) => (
                    path.path.segments.iter().map(|s| s.ident.to_string()).collect(),
                    &call.args,
                    line(path.path.segments[0].ident.span()),
                ),
                _ => return,
            },
            Expr::MethodCall(call) => (vec![call.method.to_string()], &call.args, line(call.method.span())),
            _ => return,
        };
        let Some(name) = SHOWN.iter().find(|shown| is_named(&names, shown)) else {
            return;
        };
        let mut debug = DebugFormat(false);
        args.iter().for_each(|arg| debug.visit_expr(arg));
        if debug.0 {
            self.problems.push(format!(
                "{}:{at}: `{name}(..)` formats with {{:?}}; say it in the user's words with Explain",
                cx.source.relative
            ));
        }
    }
}

/// Whether the path `names` ends in `shown`: its last segment ends with the last of `shown`, and a segment before it
/// in the first.
fn is_named(names: &[String], shown: &str) -> bool {
    match shown.split_once("::") {
        Some((owner, function)) => {
            matches!(names, [.., before, last] if before.ends_with(owner) && last == function)
        }
        None => names.last().is_some_and(|last| last.ends_with(shown)),
    }
}

/// Finds a `{:?}` in the string literals of an expression, also those inside a macro such as `format!`.
struct DebugFormat(bool);

impl<'ast> Visit<'ast> for DebugFormat {
    fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
        self.0 |= lit.value().contains(":?}");
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.0 |= debug_in(mac.tokens.clone());
    }
}

fn debug_in(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Literal(literal) => literal.to_string().contains(":?}"),
        TokenTree::Group(group) => debug_in(group.stream()),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{snippets, walk};

    fn lines(files: &[(&str, &str)], path: &str) -> Vec<String> {
        let mut all = vec![("src/main.rs", "mod a;\n")];
        all.extend_from_slice(files);
        let ws = snippets(&all);
        let mut errors = Errors::default();
        walk(ws.file(path).unwrap(), Some(&ws.index), &mut errors);
        errors
            .problems
            .iter()
            .map(|p| p.split(": ").next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_bare_string_error_is_found_and_typed_ones_are_not() {
        let source = "fn a() -> Result<(), String> {}\nfn b() -> Result<Vec<String>, Explained> {}\n\
                      fn c(f: impl FnOnce(Result<u8, String>)) {}\nfn d() -> io::Result<String> {}\n\
                      fn e(f: impl Fn() -> Result<u8, Error>) -> Result<u8, String> {}\n\
                      type Answer = std::result::Result<\n    u8,\n    std::string::String,\n>;\n\
                      // fn f() -> Result<(), String>\nconst S: &str = \"Result<(), String>\";\n";
        assert_eq!(
            lines(&[("src/a.rs", source)], "src/a.rs"),
            ["src/a.rs:1", "src/a.rs:3", "src/a.rs:5", "src/a.rs:6"]
        );
    }

    #[test]
    fn a_debug_form_shown_to_the_user_is_found() {
        let source = "fn f() {\n    alert(mtm, t(\"Fehler\"), &format!(\"{error:?}\"));\n    \
                      alert.setInformativeText(&ns(&format!(\n        \"{:?}\",\n        error\n    )));\n    \
                      let r = x.map_err(|e| format!(\"{e:?}\"));\n    let shown = format!(\"{e:?}\");\n    \
                      // alert(\"{e:?}\")\n    show_alert(mtm, &format!(\"{}\", e));\n    \
                      panel.state().phase = Phase::Failed(format!(\"{e:?}\"));\n}\n";
        assert_eq!(
            lines(&[("src/a.rs", source)], "src/a.rs"),
            ["src/a.rs:2", "src/a.rs:3", "src/a.rs:7", "src/a.rs:11"]
        );
    }

    #[test]
    fn unit_tests_may_compare_errors_as_they_like_and_code_after_them_is_checked() {
        let source = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b() -> Result<(), String> { Ok(()) }\n}\n\
                      fn c() -> Result<(), String> { Ok(()) }\n#[cfg(test)]\n#[path = \"a_tests.rs\"]\nmod split;\n";
        let split = "fn d() -> Result<(), String> { Ok(()) }\n";
        let files = [("src/a.rs", source), ("src/a_tests.rs", split)];
        assert_eq!(lines(&files, "src/a.rs"), ["src/a.rs:6"]);
        assert!(lines(&files, "src/a_tests.rs").is_empty());
    }

    #[test]
    fn self_test_hooks_are_no_production() {
        let source = "#[cfg(feature = \"selftest\")]\nmod probe;\n\
                      #[cfg(feature = \"selftest\")]\nfn hook() -> Result<(), String> { Ok(()) }\n";
        let probe = "pub fn p() -> Result<(), String> { Ok(()) }\n";
        let files = [("src/a.rs", source), ("src/a/probe.rs", probe)];
        assert!(lines(&files, "src/a.rs").is_empty());
        assert!(lines(&files, "src/a/probe.rs").is_empty());
    }
}
