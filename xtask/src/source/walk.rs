//! A walk over a parsed file that knows at each node what it is built into, the module, function and macro it is in
//! and how a path there resolves, and that reads the bodies of macros that hold Rust code, such as `define_class!`,
//! `thread_local!` and `format!`.

use std::collections::BTreeMap;

use proc_macro2::Span;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ImplItem, Item, Macro, Stmt, Token, TraitItem};

use super::index::{BlockScope, Imports};
use super::{Build, Index, Source, cfg};

/// What a check sees of a walk; each method runs before the walk goes into the node.
pub(crate) trait Hooks {
    fn item(&mut self, _cx: &Cx, _item: &Item) {}
    /// A function with a body, entered: `Cx::function` already names it.
    fn function(&mut self, _cx: &Cx, _signature: &syn::Signature, _body: &syn::Block) {}
    fn expr(&mut self, _cx: &Cx, _expr: &Expr) {}
    fn local(&mut self, _cx: &Cx, _local: &syn::Local) {}
    /// Every path but those of attributes: of types, expressions, patterns and macros.
    fn path(&mut self, _cx: &Cx, _path: &syn::Path) {}
    fn field(&mut self, _cx: &Cx, _field: &syn::Field) {}
    fn mac(&mut self, _cx: &Cx, _mac: &Macro) {}
}

/// Where the walk is.
pub(crate) struct Cx<'w> {
    pub(crate) source: &'w Source,
    index: Option<&'w Index>,
    build: Build,
    module: Vec<String>,
    /// The functions the walk is in, innermost last, each by its path from `crate`.
    functions: Vec<Vec<String>>,
    /// The types of the `impl` blocks and the traits the walk is in, innermost last.
    owners: Vec<Vec<String>>,
    macros: Vec<String>,
    blocks: Vec<BlockScope>,
}

impl Cx<'_> {
    pub(crate) fn build(&self) -> Build {
        self.build
    }

    pub(crate) fn index(&self) -> Option<&Index> {
        self.index
    }

    pub(crate) fn module(&self) -> &[String] {
        &self.module
    }

    /// The innermost function, by its path from `crate`: `crate::module::f`, `crate::module::Type::f` in an `impl`.
    pub(crate) fn function(&self) -> Option<&[String]> {
        self.functions.last().map(Vec::as_slice)
    }

    pub(crate) fn in_macro(&self, name: &str) -> bool {
        self.macros.iter().any(|m| m == name)
    }

    /// The crate root of the file, if a crate reaches it.
    pub(crate) fn krate(&self) -> Option<&str> {
        self.source.place.as_ref().map(|place| place.krate.as_str())
    }

    /// What `path` names here: from `crate` for the crate's own items, through `use`, `self`, `super` and `Self`;
    /// otherwise as written, as for another crate's item.
    pub(crate) fn resolve(&self, path: &syn::Path) -> Vec<String> {
        let names: Vec<String> = path.segments.iter().map(|segment| name(&segment.ident)).collect();
        self.resolve_names(&names)
    }

    pub(crate) fn resolve_names(&self, names: &[String]) -> Vec<String> {
        if names.first().is_some_and(|first| first == "Self")
            && let Some(owner) = self.owners.last()
        {
            return owner.iter().chain(&names[1..]).cloned().collect();
        }
        match (self.krate(), self.index) {
            (Some(krate), Some(index)) => index.resolve(krate, &self.module, &self.blocks, names),
            _ => names.to_vec(),
        }
    }

    fn item_path(&self, name: String) -> Vec<String> {
        let mut path = match (self.functions.last(), self.owners.last()) {
            (Some(function), _) => function.clone(),
            (None, Some(owner)) => owner.clone(),
            (None, None) => std::iter::once("crate".to_string())
                .chain(self.module.iter().cloned())
                .collect(),
        };
        path.push(name);
        path
    }
}

/// The line, from 1, where `span` starts.
pub(crate) fn line(span: Span) -> usize {
    span.start().line
}

/// An identifier without the `r#` of a raw one.
pub(crate) fn name(ident: &syn::Ident) -> String {
    let text = ident.to_string();
    text.strip_prefix("r#").map_or(text.clone(), str::to_string)
}

/// Walks `source` with `hooks`, resolving paths through `index` if there is one.
pub(crate) fn walk<H: Hooks>(source: &Source, index: Option<&Index>, hooks: &mut H) {
    let mut walker = Walker {
        cx: Cx {
            source,
            index,
            build: source.build,
            module: source
                .place
                .as_ref()
                .map(|place| place.module.clone())
                .unwrap_or_default(),
            functions: Vec::new(),
            owners: Vec::new(),
            macros: Vec::new(),
            blocks: Vec::new(),
        },
        hooks,
    };
    walker.visit_file(&source.syntax);
}

struct Walker<'w, 'h, H> {
    cx: Cx<'w>,
    hooks: &'h mut H,
}

impl<H: Hooks> Walker<'_, '_, H> {
    /// Runs `f` with the build narrowed by `attrs`.
    fn built<R>(&mut self, attrs: &[Attribute], f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.cx.build;
        self.cx.build = outer.max(cfg::build_of(attrs));
        let result = f(self);
        self.cx.build = outer;
        result
    }

    fn function<R>(&mut self, signature: &syn::Signature, body: &syn::Block, f: impl FnOnce(&mut Self) -> R) -> R {
        let path = self.cx.item_path(name(&signature.ident));
        self.cx.functions.push(path);
        self.hooks.function(&self.cx, signature, body);
        let result = f(self);
        self.cx.functions.pop();
        result
    }

    fn owned<R>(&mut self, owner: Vec<String>, f: impl FnOnce(&mut Self) -> R) -> R {
        self.cx.owners.push(owner);
        let functions = std::mem::take(&mut self.cx.functions);
        let result = f(self);
        self.cx.functions = functions;
        self.cx.owners.pop();
        result
    }

    /// The Rust code in the body of `mac`, walked as items, else as comma-separated expressions, else as statements.
    fn macro_body(&mut self, mac: &Macro) {
        let tokens = mac.tokens.clone();
        let name = mac.path.segments.last().map(|s| name(&s.ident)).unwrap_or_default();
        let items = syn::parse2::<syn::File>(tokens.clone()).or_else(|error| {
            if name != "thread_local" {
                return Err(error);
            }
            // The last static of a `thread_local!` block may go without its semicolon.
            let mut closed = tokens.clone();
            closed.extend([proc_macro2::TokenTree::Punct(proc_macro2::Punct::new(
                ';',
                proc_macro2::Spacing::Alone,
            ))]);
            syn::parse2::<syn::File>(closed)
        });
        self.cx.macros.push(name);
        if let Ok(file) = items
            && !file.items.is_empty()
        {
            for item in &file.items {
                self.visit_item(item);
            }
        } else if let Ok(exprs) = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(tokens.clone()) {
            for expr in &exprs {
                self.visit_expr(expr);
            }
        } else if let Ok(stmts) = syn::Block::parse_within.parse2(tokens) {
            for stmt in &stmts {
                self.visit_stmt(stmt);
            }
        }
        self.cx.macros.pop();
    }
}

impl<'ast, H: Hooks> Visit<'ast> for Walker<'_, '_, H> {
    fn visit_attribute(&mut self, _: &'ast Attribute) {}

    fn visit_item(&mut self, item: &'ast Item) {
        self.built(item_attrs(item), |walker| {
            walker.hooks.item(&walker.cx, item);
            visit::visit_item(walker, item);
        });
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        let Some((_, items)) = &module.content else {
            return;
        };
        self.cx.module.push(name(&module.ident));
        let blocks = std::mem::take(&mut self.cx.blocks);
        let functions = std::mem::take(&mut self.cx.functions);
        let owners = std::mem::take(&mut self.cx.owners);
        for item in items {
            self.visit_item(item);
        }
        self.cx.blocks = blocks;
        self.cx.functions = functions;
        self.cx.owners = owners;
        self.cx.module.pop();
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        self.function(&function.sig, &function.block, |walker| {
            visit::visit_item_fn(walker, function)
        });
    }

    fn visit_item_impl(&mut self, block: &'ast syn::ItemImpl) {
        let owner = match &*block.self_ty {
            syn::Type::Path(path) => match self.cx.resolve(&path.path) {
                // A type the index does not know, such as one a macro it cannot read declares, is taken as local.
                resolved if resolved.len() == 1 => self.cx.item_path(resolved[0].clone()),
                resolved => resolved,
            },
            _ => vec!["_".to_string()],
        };
        self.owned(owner, |walker| visit::visit_item_impl(walker, block));
    }

    fn visit_item_trait(&mut self, block: &'ast syn::ItemTrait) {
        let owner = self.cx.item_path(name(&block.ident));
        self.owned(owner, |walker| visit::visit_item_trait(walker, block));
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        let attrs: &[Attribute] = match item {
            ImplItem::Const(item) => &item.attrs,
            ImplItem::Fn(item) => &item.attrs,
            ImplItem::Type(item) => &item.attrs,
            ImplItem::Macro(item) => &item.attrs,
            _ => &[],
        };
        self.built(attrs, |walker| visit::visit_impl_item(walker, item));
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        self.function(&function.sig, &function.block, |walker| {
            visit::visit_impl_item_fn(walker, function);
        });
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        let attrs: &[Attribute] = match item {
            TraitItem::Const(item) => &item.attrs,
            TraitItem::Fn(item) => &item.attrs,
            TraitItem::Type(item) => &item.attrs,
            TraitItem::Macro(item) => &item.attrs,
            _ => &[],
        };
        self.built(attrs, |walker| visit::visit_trait_item(walker, item));
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        if let Some(body) = &function.default {
            self.function(&function.sig, body, |walker| {
                visit::visit_trait_item_fn(walker, function)
            });
        } else {
            visit::visit_trait_item_fn(self, function);
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let mut scope = BlockScope {
            items: BTreeMap::new(),
            imports: Imports::default(),
        };
        for stmt in &block.stmts {
            match stmt {
                Stmt::Item(Item::Fn(function)) => {
                    let ident = name(&function.sig.ident);
                    scope.items.insert(ident.clone(), self.cx.item_path(ident));
                }
                Stmt::Item(Item::Use(declaration)) => scope.imports.add(&declaration.tree),
                _ => {}
            }
        }
        self.cx.blocks.push(scope);
        visit::visit_block(self, block);
        self.cx.blocks.pop();
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        let attrs: &[Attribute] = match stmt {
            Stmt::Local(local) => &local.attrs,
            Stmt::Macro(mac) => &mac.attrs,
            Stmt::Expr(expr, _) => expr_attrs(expr),
            Stmt::Item(_) => &[],
        };
        self.built(attrs, |walker| visit::visit_stmt(walker, stmt));
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        self.hooks.expr(&self.cx, expr);
        visit::visit_expr(self, expr);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        self.hooks.local(&self.cx, local);
        visit::visit_local(self, local);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.hooks.path(&self.cx, path);
        visit::visit_path(self, path);
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        self.built(&field.attrs, |walker| {
            walker.hooks.field(&walker.cx, field);
            visit::visit_field(walker, field);
        });
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.hooks.mac(&self.cx, mac);
        visit::visit_macro(self, mac);
        self.macro_body(mac);
    }
}

pub(crate) fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn expr_attrs(expr: &Expr) -> &[Attribute] {
    match expr {
        Expr::Assign(expr) => &expr.attrs,
        Expr::Block(expr) => &expr.attrs,
        Expr::Call(expr) => &expr.attrs,
        Expr::ForLoop(expr) => &expr.attrs,
        Expr::If(expr) => &expr.attrs,
        Expr::Loop(expr) => &expr.attrs,
        Expr::Macro(expr) => &expr.attrs,
        Expr::Match(expr) => &expr.attrs,
        Expr::MethodCall(expr) => &expr.attrs,
        Expr::Path(expr) => &expr.attrs,
        Expr::Unsafe(expr) => &expr.attrs,
        Expr::While(expr) => &expr.attrs,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::super::snippets;
    use super::*;

    /// The functions, builds and macros at each call of `mark`.
    #[derive(Default)]
    struct Marks(Vec<(String, Build, bool)>);

    impl Hooks for Marks {
        fn expr(&mut self, cx: &Cx, expr: &Expr) {
            if let Expr::Call(call) = expr
                && let Expr::Path(path) = &*call.func
                && cx.resolve(&path.path).last().is_some_and(|n| n == "mark")
            {
                let function = cx.function().map(|f| f.join("::")).unwrap_or_default();
                self.0.push((function, cx.build(), cx.in_macro("define_class")));
            }
        }
    }

    #[test]
    fn the_walk_knows_function_build_and_macro_at_each_node() {
        let ws = snippets(&[(
            "src/main.rs",
            "fn a() { mark(); }\nimpl Panel { fn b(&self) { #[cfg(feature = \"selftest\")] mark(); } }\n\
             define_class!(\n    struct View;\n    impl View {\n        #[unsafe(method(c))]\n        fn c(&self) { \
             mark(); }\n    }\n);\nfn d() { let s = format!(\"{}\", mark()); fn e() { mark(); } }\n#[cfg(test)]\nmod \
             tests { fn f() { mark(); } }\n",
        )]);
        let mut marks = Marks::default();
        walk(&ws.files[0], Some(&ws.index), &mut marks);
        assert_eq!(
            marks.0,
            [
                ("crate::a".to_string(), Build::Production, false),
                ("crate::Panel::b".to_string(), Build::SelfTest, false),
                ("crate::View::c".to_string(), Build::Production, true),
                ("crate::d".to_string(), Build::Production, false),
                ("crate::d::e".to_string(), Build::Production, false),
                ("crate::tests::f".to_string(), Build::Test, false),
            ]
        );
    }
}
