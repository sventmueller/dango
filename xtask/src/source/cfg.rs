//! What a `#[cfg(..)]` builds an item into: the product, only the self-test (`feature = "selftest"`) or only tests.

use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

/// The builds an item is part of, from the most to the fewest; an item inside another is built into the lesser of
/// both, the larger value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Build {
    /// The app as users run it (and every build beside it).
    Production,
    /// Only the build with the feature `selftest`.
    SelfTest,
    /// Only `cargo test`.
    Test,
}

/// The values a configuration predicate can have when only `test` and the feature `selftest` are known.
#[derive(Clone, Copy, PartialEq)]
enum Truth {
    Yes,
    No,
    Unknown,
}

/// Which of `test` and the feature `selftest` a build has.
#[derive(Clone, Copy)]
struct Flags {
    test: bool,
    selftest: bool,
}

const PRODUCT: Flags = Flags {
    test: false,
    selftest: false,
};

const SELFTEST: Flags = Flags {
    test: false,
    selftest: true,
};

/// What the `cfg` attributes among `attrs` build their item into.
pub(crate) fn build_of(attrs: &[Attribute]) -> Build {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .filter_map(|attr| attr.parse_args::<Meta>().ok())
        .map(|predicate| {
            if eval(&predicate, PRODUCT) != Truth::No {
                Build::Production
            } else if eval(&predicate, SELFTEST) != Truth::No {
                Build::SelfTest
            } else {
                Build::Test
            }
        })
        .max()
        .unwrap_or(Build::Production)
}

fn eval(predicate: &Meta, flags: Flags) -> Truth {
    let known = |value: bool| if value { Truth::Yes } else { Truth::No };
    match predicate {
        Meta::Path(path) if path.is_ident("test") => known(flags.test),
        Meta::NameValue(pair) if pair.path.is_ident("feature") => match &pair.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(name),
                ..
            }) if name.value() == "selftest" => known(flags.selftest),
            _ => Truth::Unknown,
        },
        Meta::List(list) => {
            let Ok(inner) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
                return Truth::Unknown;
            };
            let values: Vec<Truth> = inner.iter().map(|meta| eval(meta, flags)).collect();
            if list.path.is_ident("not") {
                match values.as_slice() {
                    [Truth::Yes] => Truth::No,
                    [Truth::No] => Truth::Yes,
                    _ => Truth::Unknown,
                }
            } else if list.path.is_ident("all") {
                if values.contains(&Truth::No) {
                    Truth::No
                } else if values.iter().all(|v| *v == Truth::Yes) {
                    Truth::Yes
                } else {
                    Truth::Unknown
                }
            } else if list.path.is_ident("any") {
                if values.contains(&Truth::Yes) {
                    Truth::Yes
                } else if values.iter().all(|v| *v == Truth::No) {
                    Truth::No
                } else {
                    Truth::Unknown
                }
            } else {
                Truth::Unknown
            }
        }
        _ => Truth::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(attribute: &str) -> Build {
        let item: syn::ItemFn = syn::parse_str(&format!("{attribute}\nfn a() {{}}")).unwrap();
        build_of(&item.attrs)
    }

    #[test]
    fn tests_and_the_self_test_are_told_from_the_product() {
        assert_eq!(built(""), Build::Production);
        assert_eq!(built("#[cfg(test)]"), Build::Test);
        assert_eq!(built("#[cfg(feature = \"selftest\")]"), Build::SelfTest);
        assert_eq!(built("#[cfg(any(test, feature = \"selftest\"))]"), Build::SelfTest);
        assert_eq!(built("#[cfg(all(test, target_os = \"macos\"))]"), Build::Test);
        assert_eq!(built("#[cfg(not(test))]"), Build::Production);
        assert_eq!(built("#[cfg(feature = \"dictation\")]"), Build::Production);
        assert_eq!(built("#[inline]\n#[cfg(test)]"), Build::Test);
        assert_eq!(built("#[cfg(not(feature = \"selftest\"))]"), Build::Production);
    }
}
