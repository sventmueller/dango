//! The ratchets of E143 on the app's lookups of global state: the app context through `context::app()`, `try_app()`
//! and `AppContext::get`/`of_delegate`, and a UI crate's state through its host's `state()`/`try_state()`. A lookup
//! counts however it is named (an import, a rename, `Self::`), and so does every call of a funnel: a function of the
//! same crate that hands a lookup out, returning it or passing it to its closure, itself or through another funnel,
//! such as a wrapper `fn state()`, `open_editors()` or `with_tabs(f)`.

mod flow;

use std::collections::{BTreeMap, BTreeSet};

use syn::Expr;

use crate::source::{Build, Cx, Hooks, Workspace, walk};
use flow::bound;

/// The app crate, whose context module holds the app context.
const APP_CRATE: &str = "src/main.rs";

/// The functions of the app crate that look the app context up, by their path from `crate`.
const APP_PRIMITIVES: [&[&str]; 4] = [
    &["crate", "context", "app"],
    &["crate", "context", "try_app"],
    &["crate", "context", "AppContext", "get"],
    &["crate", "context", "AppContext", "of_delegate"],
];

/// The methods of a UI crate's `Host` that hand out the crate's state.
const STATE_METHODS: [&str; 2] = ["state", "try_state"];

/// The static that holds a UI crate's installed host.
const HOOK: &str = "HOST";

/// What a site reaches.
#[derive(Clone, Debug, PartialEq)]
enum Reach {
    /// The app context directly, through the function `APP_PRIMITIVES` names by its last segments.
    App(String),
    /// A UI crate's state directly, through its host.
    State,
    /// A function of the same crate, by its path from `crate`, which is a lookup if it is a funnel.
    Call(Vec<String>),
}

#[derive(Debug)]
struct Site {
    krate: String,
    reach: Reach,
    /// Whether the ratchet counts the site: production code of the app or a UI crate.
    counted: bool,
}

/// The lookups the ratchets count, by kind.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Lookups {
    /// The app context's: the primitives by name and `funnels`, the calls of functions that reach them.
    pub(super) app: BTreeMap<String, usize>,
    /// A UI crate's state: `host` for the direct ones and `funnels`.
    pub(super) state: BTreeMap<String, usize>,
}

impl Lookups {
    pub(super) fn app_total(&self) -> usize {
        self.app.values().sum()
    }

    pub(super) fn state_total(&self) -> usize {
        self.state.values().sum()
    }
}

/// Whether the ratchets count the code of the file at `relative`: the app's and the UI crates'.
fn in_scope(relative: &str) -> bool {
    relative.starts_with("src/") || relative.starts_with("crates/ide-ui-")
}

/// The lookups of the app context and the UI crates' state in `ws`.
pub(super) fn count(ws: &Workspace) -> Lookups {
    let mut getters = Getters::default();
    for source in &ws.files {
        walk(source, Some(&ws.index), &mut getters);
    }
    let mut collect = Collect {
        getters,
        sites: Vec::new(),
        handouts: BTreeMap::new(),
        host_names: BTreeSet::new(),
    };
    for source in ws.files.iter().filter(|source| source.place.is_some()) {
        walk(source, Some(&ws.index), &mut collect);
    }
    tally(&collect.sites, &collect.handouts)
}

/// What each function hands out, by crate and path.
type Handouts = BTreeMap<(String, Vec<String>), Vec<Reach>>;

fn tally(sites: &[Site], handouts: &Handouts) -> Lookups {
    let app = funnels(handouts, |reach| matches!(reach, Reach::App(_)));
    let state = funnels(handouts, |reach| *reach == Reach::State);
    let mut lookups = Lookups::default();
    for site in sites.iter().filter(|site| site.counted) {
        let is_funnel = |funnels: &BTreeSet<(String, Vec<String>)>| match &site.reach {
            Reach::Call(path) => funnels.contains(&(site.krate.clone(), path.clone())),
            _ => false,
        };
        match &site.reach {
            Reach::App(name) => *lookups.app.entry(name.clone()).or_default() += 1,
            Reach::State => *lookups.state.entry("host".to_string()).or_default() += 1,
            Reach::Call(_) => {}
        }
        if is_funnel(&app) {
            *lookups.app.entry("funnels".to_string()).or_default() += 1;
        }
        if is_funnel(&state) {
            *lookups.state.entry("funnels".to_string()).or_default() += 1;
        }
    }
    lookups
}

/// The functions, by crate and path, that hand out what `direct` names, themselves or through each other.
fn funnels(handouts: &Handouts, direct: impl Fn(&Reach) -> bool) -> BTreeSet<(String, Vec<String>)> {
    let mut funnels: BTreeSet<(String, Vec<String>)> = handouts
        .iter()
        .filter(|(_, reaches)| reaches.iter().any(&direct))
        .map(|(function, _)| function.clone())
        .collect();
    loop {
        let more: Vec<(String, Vec<String>)> = handouts
            .iter()
            .filter(|(function, _)| !funnels.contains(*function))
            .filter(|((krate, _), reaches)| {
                reaches.iter().any(|reach| match reach {
                    Reach::Call(path) => funnels.contains(&(krate.clone(), path.clone())),
                    _ => false,
                })
            })
            .map(|(function, _)| function.clone())
            .collect();
        if more.is_empty() {
            return funnels;
        }
        funnels.extend(more);
    }
}

/// The functions that hand out a UI crate's installed host, which return a `Host`, by crate and path.
#[derive(Default)]
struct Getters(BTreeSet<(String, Vec<String>)>);

impl Hooks for Getters {
    fn function(&mut self, cx: &Cx, signature: &syn::Signature, _body: &syn::Block) {
        let returns_host = match &signature.output {
            syn::ReturnType::Type(_, ty) => mentions(ty, "Host"),
            syn::ReturnType::Default => false,
        };
        if returns_host && let (Some(krate), Some(function)) = (cx.krate(), cx.function()) {
            self.0.insert((krate.to_string(), function.to_vec()));
        }
    }
}

/// Whether the type `ty` names `ident` anywhere, as `&'static (dyn Host + Sync)` names `Host`.
fn mentions(ty: &syn::Type, ident: &str) -> bool {
    struct Finder<'a>(&'a str, bool);
    impl<'ast> syn::visit::Visit<'ast> for Finder<'_> {
        fn visit_ident(&mut self, found: &'ast syn::Ident) {
            self.1 |= found == self.0;
        }
    }
    let mut finder = Finder(ident, false);
    syn::visit::Visit::visit_type(&mut finder, ty);
    finder.1
}

struct Collect {
    getters: Getters,
    sites: Vec<Site>,
    handouts: Handouts,
    /// The closure parameters and locals of the current function that hold the host.
    host_names: BTreeSet<String>,
}

impl Collect {
    fn push(&mut self, cx: &Cx, reach: Reach) {
        let Some(krate) = cx.krate() else { return };
        if let (Reach::App(_), Some(function)) = (&reach, cx.function())
            && is_app_primitive(krate, function)
        {
            return;
        }
        self.sites.push(Site {
            krate: krate.to_string(),
            reach,
            counted: cx.build() == Build::Production && in_scope(&cx.source.relative),
        });
    }

    /// What the path `path` reaches as a call or a value.
    fn reach(cx: &Cx, path: &syn::Path) -> Option<Reach> {
        let resolved = cx.resolve(path);
        let krate = cx.krate()?;
        if is_app_primitive(krate, &resolved) {
            return resolved.last().map(|name| Reach::App(name.clone()));
        }
        (resolved.first().is_some_and(|first| first == "crate")).then_some(Reach::Call(resolved))
    }

    /// Whether `expr`, stripped of the method calls, fields and `?` it is reached through, starts at the host.
    fn starts_at_host(&self, cx: &Cx, expr: &Expr) -> bool {
        match root(expr) {
            Expr::Path(path) => {
                let local = path
                    .path
                    .get_ident()
                    .is_some_and(|name| self.host_names.contains(&name.to_string()));
                local || cx.resolve(&path.path).last().is_some_and(|name| name == HOOK)
            }
            Expr::Call(call) => match &*call.func {
                Expr::Path(path) => {
                    let krate = cx.krate().unwrap_or_default().to_string();
                    self.getters.0.contains(&(krate, cx.resolve(&path.path)))
                }
                _ => false,
            },
            _ => false,
        }
    }
}

fn is_app_primitive(krate: &str, path: &[String]) -> bool {
    krate == APP_CRATE && APP_PRIMITIVES.iter().any(|primitive| primitive.iter().eq(path.iter()))
}

/// The expression a chain of method calls, field accesses, `?`, parentheses and references starts from.
fn root(expr: &Expr) -> &Expr {
    match expr {
        Expr::MethodCall(call) => root(&call.receiver),
        Expr::Field(field) => root(&field.base),
        Expr::Try(inner) => root(&inner.expr),
        Expr::Paren(inner) => root(&inner.expr),
        Expr::Reference(inner) => root(&inner.expr),
        Expr::Unary(inner) => root(&inner.expr),
        other => other,
    }
}

impl Hooks for Collect {
    fn function(&mut self, cx: &Cx, signature: &syn::Signature, body: &syn::Block) {
        self.host_names.clear();
        if let (Some(krate), Some(function)) = (cx.krate(), cx.function()) {
            let reaches = flow::handed_out(cx, signature, body, &self.getters);
            self.handouts.insert((krate.to_string(), function.to_vec()), reaches);
        }
    }

    fn expr(&mut self, cx: &Cx, expr: &Expr) {
        match expr {
            Expr::Path(path) if path.path.segments.len() > 1 => {
                if let Some(reach) = Self::reach(cx, &path.path) {
                    self.push(cx, reach);
                }
            }
            Expr::Call(call) => {
                if let Expr::Path(path) = &*call.func
                    && path.path.segments.len() == 1
                    && let Some(reach) = Self::reach(cx, &path.path)
                {
                    self.push(cx, reach);
                }
            }
            Expr::MethodCall(call) => {
                let method = call.method.to_string();
                if STATE_METHODS.contains(&method.as_str())
                    && call.args.is_empty()
                    && self.starts_at_host(cx, &call.receiver)
                {
                    self.push(cx, Reach::State);
                } else if self.starts_at_host(cx, &call.receiver) {
                    for arg in &call.args {
                        if let Expr::Closure(closure) = arg {
                            let mut names = std::mem::take(&mut self.host_names);
                            closure.inputs.iter().for_each(|pat| bound(pat, &mut names));
                            self.host_names = names;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn local(&mut self, cx: &Cx, local: &syn::Local) {
        if let Some(init) = &local.init
            && self.starts_at_host(cx, &init.expr)
            && !matches!(&*init.expr, Expr::MethodCall(call) if STATE_METHODS.contains(&call.method.to_string().as_str()))
        {
            let mut names = std::mem::take(&mut self.host_names);
            bound(&local.pat, &mut names);
            self.host_names = names;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::snippets;

    const CONTEXT: (&str, &str) = (
        "src/context.rs",
        "pub struct AppContext;\nimpl AppContext {\n    pub fn get(mtm: M) -> Rc<AppContext> { \
         Self::of_delegate(mtm).unwrap() }\n    fn of_delegate(mtm: M) -> Option<Rc<AppContext>> { None }\n}\n\
         pub fn app() -> Rc<AppContext> { AppContext::get(m()) }\npub fn try_app() -> Option<Rc<AppContext>> { \
         AppContext::of_delegate(m()) }\n",
    );

    fn app_lookups(files: &[(&str, &str)]) -> BTreeMap<String, usize> {
        let mut all = vec![("src/main.rs", "mod context;\nmod a;\nmod b;\n"), CONTEXT];
        all.extend_from_slice(files);
        count(&snippets(&all)).app
    }

    fn counts(pairs: &[(&str, usize)]) -> BTreeMap<String, usize> {
        pairs.iter().map(|(name, n)| ((*name).to_string(), *n)).collect()
    }

    #[test]
    fn the_definitions_of_the_lookups_are_no_lookups() {
        assert_eq!(app_lookups(&[]), BTreeMap::new());
    }

    #[test]
    fn a_lookup_counts_through_a_path_an_import_and_a_rename() {
        let a = "use crate::context::app;\nuse crate::context::try_app as maybe;\nuse crate::context::AppContext;\n\
                 fn f() { let a = app(); let b = crate::context::app(); let c = maybe(); }\n\
                 fn g(mtm: M) { let d = AppContext::get(mtm); let e = MainThreadMarker::new().map(AppContext::get); }\n\
                 fn h() { let my_app = 1; let x = my_app; }\n";
        assert_eq!(
            app_lookups(&[("src/a.rs", a)]),
            counts(&[("app", 2), ("try_app", 1), ("get", 2)])
        );
    }

    #[test]
    fn calls_of_funnels_count_transitively_but_tests_do_not() {
        let a = "fn open_editors() -> Vec<E> { crate::context::app().editors() }\n\
                 pub fn all_editors() -> Vec<E> { open_editors() }\n\
                 pub(crate) fn find(serial: u64) -> Option<E> { all_editors().into_iter().find(|e| e.serial == serial) }\n\
                 fn unrelated() -> u8 { 1 }\nfn caller() { let _ = (unrelated(), self::find(1)); }\n\
                 #[cfg(test)]\nmod tests { fn t() { super::all_editors(); } }\n";
        let b = "use crate::a::*;\nfn elsewhere() { all_editors(); }\n";
        assert_eq!(
            app_lookups(&[("src/a.rs", a), ("src/b.rs", b)]),
            counts(&[("app", 1), ("funnels", 4)])
        );
    }

    #[test]
    fn the_state_of_a_ui_crate_counts_through_its_host_and_its_wrappers() {
        let lib = "mod host;\nmod hub;\nmod tabs;\n";
        let host = "pub trait Host { fn state(&self) -> Rc<State>; fn try_state(&self) -> Option<Rc<State>>; }\n\
                    static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();\n\
                    pub(crate) fn host() -> &'static (dyn Host + Sync) { *HOST.get().unwrap() }\n\
                    pub(crate) fn state() -> Rc<State> { host().state() }\n\
                    fn forget(id: u64) { if let Some(s) = HOST.get().and_then(|h| h.try_state()) { s.drop(id) } }\n\
                    fn find(id: u64) { let h = host(); h.state().views.get(&id); }\n";
        let hub = "fn state() -> Rc<State> { super::host::HOST.get().expect(\"x\").state() }\n\
                   fn with_hub<R>(f: impl FnOnce(&Hub) -> R) -> R { f(&state().hub) }\n\
                   fn open() { with_hub(|h| h.open()); }\n";
        let tabs = "use super::host::state;\nfn a(panel: &Panel) { state(); panel.state(); button.state(); }\n";
        let ws = snippets(&[
            ("crates/ide-ui-x/src/lib.rs", lib),
            ("crates/ide-ui-x/src/host.rs", host),
            ("crates/ide-ui-x/src/hub.rs", hub),
            ("crates/ide-ui-x/src/tabs.rs", tabs),
        ]);
        assert_eq!(count(&ws).state, counts(&[("host", 4), ("funnels", 3)]));
    }

    #[test]
    fn an_operation_that_only_uses_the_context_inside_is_no_funnel() {
        let a = "fn save() { let cx = crate::context::app(); cx.save_all(); }\n\
                 fn quit() -> bool { save(); true }\nfn menu() { save(); quit(); }\n\
                 pub fn windows() -> Vec<W> { let cx = crate::context::app(); let all = cx.windows().all(); all }\n\
                 fn front() -> Option<W> { windows().into_iter().next() }\n\
                 fn with_window<R>(f: impl FnOnce(&W) -> R) -> Option<R> { let w = front()?; Some(f(&w)) }\n\
                 fn key() { with_window(|w| w.key()); }\n";
        assert_eq!(app_lookups(&[("src/a.rs", a)]), counts(&[("app", 2), ("funnels", 3)]));
    }

    #[test]
    fn a_method_that_shares_a_funnel_name_is_no_call_of_it() {
        let a = "fn state() -> u8 { crate::context::app().n }\nimpl Panel { fn state(&self) -> u8 { self.n } }\n\
                 fn f(p: &Panel) { p.state(); Panel::state(p); }\n";
        assert_eq!(app_lookups(&[("src/a.rs", a)]), counts(&[("app", 1)]));
    }
}
