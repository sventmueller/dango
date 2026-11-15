//! What a function hands out of global state: the lookups and calls its returned values start from, and those of
//! the values it passes to a closure it was given, as `with_tabs(f)` passes the tabs. A function that hands out a
//! lookup is a funnel; one that only uses it inside, such as a whole operation, is not.

use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;
use syn::{Expr, Stmt};

use super::{Getters, HOOK, Reach, STATE_METHODS, is_app_primitive};
use crate::source::Cx;

/// The reaches the values that leave the function with `signature` and `body` start from.
pub(super) fn handed_out(cx: &Cx, signature: &syn::Signature, body: &syn::Block, getters: &Getters) -> Vec<Reach> {
    let mut gather = Gather::default();
    gather.visit_block(body);
    let params = signature
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => match &*typed.pat {
                syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    let flow = Flow {
        cx,
        getters,
        params,
        locals: gather.locals,
    };
    let mut leaving: Vec<&Expr> = gather.returned;
    leaving.extend(tail(body));
    for (callee, args) in gather.calls {
        if flow.params.contains(&callee) {
            leaving.extend(args);
        }
    }
    let mut reaches = Vec::new();
    for expr in leaving {
        flow.roots(expr, &mut BTreeSet::new(), &mut reaches);
    }
    reaches
}

fn tail(block: &syn::Block) -> Option<&Expr> {
    match block.stmts.last() {
        Some(Stmt::Expr(expr, None)) => Some(expr),
        _ => None,
    }
}

/// The `return`s, the `let` bindings and the calls by a plain name of a function body, without nested functions.
#[derive(Default)]
struct Gather<'b> {
    returned: Vec<&'b Expr>,
    locals: BTreeMap<String, Vec<&'b Expr>>,
    calls: Vec<(String, Vec<&'b Expr>)>,
    /// How many closures deep the walk is, whose `return` leaves only the closure.
    closures: usize,
}

impl<'b> Visit<'b> for Gather<'b> {
    fn visit_item(&mut self, _: &'b syn::Item) {}

    fn visit_expr_closure(&mut self, closure: &'b syn::ExprClosure) {
        self.closures += 1;
        syn::visit::visit_expr_closure(self, closure);
        self.closures -= 1;
    }

    fn visit_expr_return(&mut self, ret: &'b syn::ExprReturn) {
        if self.closures == 0
            && let Some(expr) = &ret.expr
        {
            self.returned.push(expr);
        }
        syn::visit::visit_expr_return(self, ret);
    }

    fn visit_expr_call(&mut self, call: &'b syn::ExprCall) {
        if let Expr::Path(path) = &*call.func
            && let Some(name) = path.path.get_ident()
        {
            self.calls.push((name.to_string(), call.args.iter().collect()));
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_local(&mut self, local: &'b syn::Local) {
        if let Some(init) = &local.init {
            let mut names = BTreeSet::new();
            bound(&local.pat, &mut names);
            for name in names {
                self.locals.entry(name).or_default().push(&init.expr);
            }
        }
        syn::visit::visit_local(self, local);
    }
}

/// The names `pat` binds.
pub(super) fn bound(pat: &syn::Pat, names: &mut BTreeSet<String>) {
    match pat {
        syn::Pat::Ident(ident) => {
            names.insert(ident.ident.to_string());
        }
        syn::Pat::Type(typed) => bound(&typed.pat, names),
        syn::Pat::Reference(reference) => bound(&reference.pat, names),
        syn::Pat::TupleStruct(tuple) => tuple.elems.iter().for_each(|pat| bound(pat, names)),
        syn::Pat::Tuple(tuple) => tuple.elems.iter().for_each(|pat| bound(pat, names)),
        _ => {}
    }
}

struct Flow<'c, 'w, 'b> {
    cx: &'c Cx<'w>,
    getters: &'c Getters,
    params: BTreeSet<String>,
    locals: BTreeMap<String, Vec<&'b Expr>>,
}

impl Flow<'_, '_, '_> {
    /// Adds to `reaches` what the value of `expr` starts from; `seen` holds the locals already followed.
    fn roots(&self, expr: &Expr, seen: &mut BTreeSet<String>, reaches: &mut Vec<Reach>) {
        match expr {
            Expr::MethodCall(call) => {
                if STATE_METHODS.contains(&call.method.to_string().as_str())
                    && call.args.is_empty()
                    && self.is_host(&call.receiver, &mut BTreeSet::new())
                {
                    reaches.push(Reach::State);
                } else {
                    self.roots(&call.receiver, seen, reaches);
                }
            }
            Expr::Field(field) => self.roots(&field.base, seen, reaches),
            Expr::Try(inner) => self.roots(&inner.expr, seen, reaches),
            Expr::Paren(inner) => self.roots(&inner.expr, seen, reaches),
            Expr::Reference(inner) => self.roots(&inner.expr, seen, reaches),
            Expr::Unary(inner) => self.roots(&inner.expr, seen, reaches),
            Expr::Cast(inner) => self.roots(&inner.expr, seen, reaches),
            Expr::Group(inner) => self.roots(&inner.expr, seen, reaches),
            Expr::Call(call) => self.call(call, seen, reaches),
            Expr::Path(path) => self.path(&path.path, seen, reaches),
            Expr::Block(block) => self.block(&block.block, seen, reaches),
            Expr::Unsafe(block) => self.block(&block.block, seen, reaches),
            Expr::If(branch) => {
                self.block(&branch.then_branch, seen, reaches);
                if let Some((_, otherwise)) = &branch.else_branch {
                    self.roots(otherwise, seen, reaches);
                }
            }
            Expr::Match(matched) => matched.arms.iter().for_each(|arm| self.roots(&arm.body, seen, reaches)),
            Expr::Tuple(tuple) => tuple.elems.iter().for_each(|elem| self.roots(elem, seen, reaches)),
            Expr::Array(array) => array.elems.iter().for_each(|elem| self.roots(elem, seen, reaches)),
            Expr::Struct(literal) => literal
                .fields
                .iter()
                .for_each(|field| self.roots(&field.expr, seen, reaches)),
            _ => {}
        }
    }

    fn block(&self, block: &syn::Block, seen: &mut BTreeSet<String>, reaches: &mut Vec<Reach>) {
        if let Some(expr) = tail(block) {
            self.roots(expr, seen, reaches);
        }
    }

    /// A call hands out the callee's lookup, or a funnel's; another crate's function or constructor, such as `Some`
    /// or `Rc::clone`, hands on its arguments; a closure parameter's result is its own.
    fn call(&self, call: &syn::ExprCall, seen: &mut BTreeSet<String>, reaches: &mut Vec<Reach>) {
        let Expr::Path(path) = &*call.func else { return };
        if path
            .path
            .get_ident()
            .is_some_and(|name| self.params.contains(&name.to_string()))
        {
            return;
        }
        let resolved = self.cx.resolve(&path.path);
        let krate = self.cx.krate().unwrap_or_default();
        if is_app_primitive(krate, &resolved) {
            reaches.extend(resolved.last().map(|name| Reach::App(name.clone())));
        } else if resolved.first().is_some_and(|first| first == "crate") {
            reaches.push(Reach::Call(resolved));
        } else {
            call.args.iter().for_each(|arg| self.roots(arg, seen, reaches));
        }
    }

    fn path(&self, path: &syn::Path, seen: &mut BTreeSet<String>, reaches: &mut Vec<Reach>) {
        if let Some(name) = path.get_ident().map(ToString::to_string) {
            if let Some(inits) = self.locals.get(&name)
                && seen.insert(name)
            {
                inits.iter().for_each(|init| self.roots(init, seen, reaches));
            }
            return;
        }
        let resolved = self.cx.resolve(path);
        if is_app_primitive(self.cx.krate().unwrap_or_default(), &resolved) {
            reaches.extend(resolved.last().map(|name| Reach::App(name.clone())));
        }
    }

    /// Whether `expr` starts at the installed host: its static, a function that hands it out, or a local of either.
    fn is_host(&self, expr: &Expr, seen: &mut BTreeSet<String>) -> bool {
        match super::root(expr) {
            Expr::Path(path) => {
                if let Some(name) = path.path.get_ident().map(ToString::to_string)
                    && let Some(inits) = self.locals.get(&name)
                {
                    return seen.insert(name) && inits.iter().any(|init| self.is_host(init, seen));
                }
                self.cx.resolve(&path.path).last().is_some_and(|name| name == HOOK)
            }
            Expr::Call(call) => match &*call.func {
                Expr::Path(path) => {
                    let krate = self.cx.krate().unwrap_or_default().to_string();
                    self.getters.0.contains(&(krate, self.cx.resolve(&path.path)))
                }
                _ => false,
            },
            _ => false,
        }
    }
}
