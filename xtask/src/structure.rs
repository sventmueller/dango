//! The checks that keep the code's shape as E98 wants it: no Rust file over a thousand lines, and models of panels and
//! tabs that know nothing of AppKit and keep their fields to themselves. The models are read as syntax trees
//! (`crate::source`), so comments, strings and unit tests do not count.

use proc_macro2::{TokenStream, TokenTree};

use crate::root;
use crate::source::{Build, Cx, Hooks, Source, Workspace, line, walk};

/// The most lines a Rust file may have.
pub(crate) const MAX_LINES: usize = 1000;

/// The public fields the models hold, a ratchet held exactly: a state whose fields anyone may set keeps its rules only
/// as long as every caller follows them, so models hand out methods for their changes instead.
const MODEL_PUB_FIELDS: usize = 221;

pub(crate) fn check() -> Result<(), String> {
    let ws = Workspace::load(&root(), &[""])?;
    let (summary, problems) = review(&ws);
    if problems.is_empty() {
        println!("{summary}");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// The summary of `ws` and what is wrong with it.
fn review(ws: &Workspace) -> (String, Vec<String>) {
    let mut problems = Vec::new();
    let mut pub_fields = 0;
    for source in &ws.files {
        problems.extend(too_long(source));
        if is_model(&source.relative) {
            let mut model = Model::default();
            walk(source, None, &mut model);
            problems.extend(model.objc2);
            pub_fields += model.pub_fields;
        }
    }
    problems.extend(ratchet(pub_fields, MODEL_PUB_FIELDS));
    let summary = format!(
        "{} Rust files, none over {MAX_LINES} lines; no model that uses objc2, {pub_fields} public fields in the models",
        ws.files.len()
    );
    (summary, problems)
}

/// The ratchet on the models' public fields: more is a regression, fewer must lower the constant to the count.
fn ratchet(pub_fields: usize, limit: usize) -> Option<String> {
    match pub_fields.cmp(&limit) {
        std::cmp::Ordering::Greater => Some(format!(
            "the models have {pub_fields} public fields, more than {limit}; change them through methods"
        )),
        std::cmp::Ordering::Less => Some(format!(
            "the models have {pub_fields} public fields, fewer than {limit}: lower MODEL_PUB_FIELDS in \
             xtask/src/structure.rs to {pub_fields}, so the count cannot rise again"
        )),
        std::cmp::Ordering::Equal => None,
    }
}

fn too_long(source: &Source) -> Option<String> {
    let lines = source.text.lines().count();
    (lines > MAX_LINES).then(|| {
        format!(
            "{}: {lines} lines, more than {MAX_LINES}; split it along its seams",
            source.relative
        )
    })
}

/// What a model file holds that the rules count.
#[derive(Default)]
struct Model {
    objc2: Vec<String>,
    pub_fields: usize,
}

impl Model {
    fn uses_objc2(&mut self, cx: &Cx, at: usize) {
        let problem = format!(
            "{}:{at}: a model uses objc2; AppKit belongs in the panel's view or controller",
            cx.source.relative
        );
        if !self.objc2.contains(&problem) {
            self.objc2.push(problem);
        }
    }
}

impl Hooks for Model {
    fn item(&mut self, cx: &Cx, item: &syn::Item) {
        match item {
            syn::Item::Use(declaration) => {
                if let Some(at) = objc2_in_use(&declaration.tree) {
                    self.uses_objc2(cx, at);
                }
            }
            syn::Item::ExternCrate(declaration) if is_objc2(&declaration.ident) => {
                self.uses_objc2(cx, line(declaration.ident.span()));
            }
            _ => {}
        }
    }

    fn path(&mut self, cx: &Cx, path: &syn::Path) {
        if let Some(first) = path.segments.first()
            && is_objc2(&first.ident)
        {
            self.uses_objc2(cx, line(first.ident.span()));
        }
    }

    fn mac(&mut self, cx: &Cx, mac: &syn::Macro) {
        if let Some(at) = objc2_in_tokens(mac.tokens.clone()) {
            self.uses_objc2(cx, at);
        }
    }

    fn field(&mut self, cx: &Cx, field: &syn::Field) {
        if cx.build() != Build::Test && matches!(field.vis, syn::Visibility::Public(_)) {
            self.pub_fields += 1;
        }
    }
}

/// Whether `ident` names objc2 or one of its framework crates, such as `objc2_app_kit`.
fn is_objc2(ident: &syn::Ident) -> bool {
    let name = ident.to_string();
    name == "objc2" || name.starts_with("objc2_")
}

fn objc2_in_use(tree: &syn::UseTree) -> Option<usize> {
    match tree {
        syn::UseTree::Path(path) => is_objc2(&path.ident).then(|| line(path.ident.span())),
        syn::UseTree::Name(name) => is_objc2(&name.ident).then(|| line(name.ident.span())),
        syn::UseTree::Rename(rename) => is_objc2(&rename.ident).then(|| line(rename.ident.span())),
        syn::UseTree::Group(group) => group.items.iter().find_map(objc2_in_use),
        syn::UseTree::Glob(_) => None,
    }
}

/// The line of the first objc2 path in tokens a macro does not let the walk read as Rust, such as `msg_send!`'s.
fn objc2_in_tokens(tokens: TokenStream) -> Option<usize> {
    tokens.into_iter().find_map(|token| match token {
        TokenTree::Ident(ident) if ident == "objc2" || ident.to_string().starts_with("objc2_") => {
            Some(line(ident.span()))
        }
        TokenTree::Group(group) => objc2_in_tokens(group.stream()),
        _ => None,
    })
}

/// Whether the file at `relative` is a model: a `model.rs` or a file in a `model` folder.
fn is_model(relative: &str) -> bool {
    let mut parts = relative.rsplit('/');
    let name = parts.next().unwrap_or_default();
    name == "model.rs" || parts.any(|folder| folder == "model")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::snippets;

    fn problems(path: &str, code: &str) -> Vec<String> {
        review(&snippets(&[(path, code)]))
            .1
            .into_iter()
            .filter(|p| !p.starts_with("the models have"))
            .collect()
    }

    fn pub_fields(code: &str) -> usize {
        let ws = snippets(&[("src/git_panel/model.rs", code)]);
        let mut model = Model::default();
        walk(&ws.files[0], None, &mut model);
        model.pub_fields
    }

    #[test]
    fn a_long_file_fails() {
        let text = "fn a() {}\n".repeat(MAX_LINES + 1);
        assert_eq!(problems("src/a.rs", &text).len(), 1);
        assert!(problems("src/a.rs", &"\n".repeat(MAX_LINES)).is_empty());
    }

    #[test]
    fn objc2_in_a_model_fails_but_not_in_a_comment_a_text_or_elsewhere() {
        let code = "use objc2::rc::Retained;\n";
        assert_eq!(problems("src/git_panel/model.rs", code).len(), 1);
        assert_eq!(problems("src/agent/model/rows.rs", code).len(), 1);
        assert!(problems("src/agent/view.rs", code).is_empty());
        assert!(
            problems(
                "src/git_panel/model.rs",
                "// no objc2 here\nconst S: &str = \"objc2\";\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn objc2_counts_however_the_model_names_it() {
        for code in [
            "use {std::rc::Rc, objc2_app_kit::NSView};\n",
            "fn f() -> objc2::rc::Retained<V> { todo!() }\n",
            "fn f() { let v = msg_send![objc2::class!(NSView), new]; }\n",
            "use objc2 as appkit;\n",
        ] {
            assert_eq!(problems("src/git_panel/model.rs", code).len(), 1, "{code}");
        }
    }

    #[test]
    fn public_fields_are_counted_but_not_functions_paths_or_tests() {
        let text = "struct A {\n    pub name: String,\n    pub(crate) kept: u8,\n    hidden: u8,\n}\n\
                    pub fn f() {}\npub use a::b;\n#[cfg(test)]\nmod tests {\n    struct T { pub x: u8 }\n}\n\
                    struct After {\n    pub\n    split: u8,\n}\nstruct Pair(pub u8, u8);\n";
        assert_eq!(pub_fields(text), 3);
    }

    #[test]
    fn the_field_ratchet_fails_both_ways_with_the_value_to_put_in() {
        assert_eq!(ratchet(5, 5), None);
        assert!(ratchet(6, 5).unwrap().contains("more than 5"));
        assert!(
            ratchet(4, 5)
                .unwrap()
                .contains("lower MODEL_PUB_FIELDS in xtask/src/structure.rs to 4")
        );
    }

    #[test]
    fn the_code_as_it_is_passes() {
        check().unwrap();
    }
}
