//! Which exception filters a project's sessions offer: an adapter's filters for languages the project does not use
//! are left out, and a Rust project gets "Rust Panic" from lldb-dap, which has no filter for it, through a function
//! breakpoint.

use std::path::Path;

use crate::adapter::AdapterKind;
use crate::types::{Capabilities, ExceptionFilter};

/// The filter that stops where a Rust program panics; CodeLLDB offers it under this id.
pub(crate) const RUST_PANIC: &str = "rust_panic";

/// The function of the Rust standard library every panic passes once its message is written.
const PANIC_FUNCTION: &str = "rust_panic";

/// The languages a project is written in, as far as the files at its root tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Languages {
    /// It has a `Cargo.toml`.
    pub rust: bool,
    /// It is built with Xcode or Swift Package Manager.
    pub apple: bool,
    /// It is built with CMake, Meson or autotools.
    pub c_family: bool,
}

impl Languages {
    /// The languages of the project at `root`.
    pub fn of(root: &Path) -> Languages {
        let has = |name: &str| root.join(name).exists();
        let apple = has("Package.swift")
            || std::fs::read_dir(root).is_ok_and(|entries| {
                entries.flatten().any(|e| {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    name.ends_with(".xcodeproj") || name.ends_with(".xcworkspace")
                })
            });
        Languages {
            rust: has("Cargo.toml"),
            apple,
            c_family: has("CMakeLists.txt") || has("meson.build") || has("configure.ac") || has("configure"),
        }
    }

    /// Whether nothing is known, so every filter stays.
    fn unknown(self) -> bool {
        !self.rust && !self.apple && !self.c_family
    }

    /// Whether the filter `id` is for a language of the project.
    pub fn fits(self, id: &str) -> bool {
        if self.unknown() {
            return true;
        }
        if id == RUST_PANIC {
            return self.rust;
        }
        if id.starts_with("swift_") || id.starts_with("objc_") {
            return self.apple;
        }
        if id.starts_with("cpp_") {
            return self.apple || self.c_family;
        }
        true
    }
}

/// The filters a session of `kind` with `caps` offers in a project of `languages`.
pub fn offered(kind: AdapterKind, caps: &Capabilities, languages: Languages) -> Vec<ExceptionFilter> {
    let mut filters: Vec<ExceptionFilter> = caps
        .exception_filters
        .iter()
        .filter(|f| languages.fits(&f.filter))
        .cloned()
        .collect();
    if languages.rust && kind == AdapterKind::Lldb && panics_by_function(caps) {
        filters.push(rust_panic());
    }
    filters
}

/// The filter a Rust project shows before its first session: lldb-dap is the only debugger for Rust and takes
/// function breakpoints.
pub(crate) fn rust_panic() -> ExceptionFilter {
    ExceptionFilter {
        filter: RUST_PANIC.into(),
        label: "Rust Panic".into(),
        description: None,
        default: true,
        supports_condition: false,
    }
}

/// Whether the adapter lacks a Rust panic filter of its own but can stop at a function instead.
pub(crate) fn panics_by_function(caps: &Capabilities) -> bool {
    caps.function_breakpoints && !caps.exception_filters.iter().any(|f| f.filter == RUST_PANIC)
}

/// `filters` split into what `setExceptionBreakpoints` takes and the functions `setFunctionBreakpoints` takes for an
/// adapter with `caps`.
pub(crate) fn split(filters: &[String], caps: &Capabilities) -> (Vec<String>, Vec<&'static str>) {
    let by_function = panics_by_function(caps);
    let panic = by_function && filters.iter().any(|f| f == RUST_PANIC);
    let filters = filters
        .iter()
        .filter(|f| !(by_function && *f == RUST_PANIC))
        .cloned()
        .collect();
    (filters, if panic { vec![PANIC_FUNCTION] } else { Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(id: &str) -> ExceptionFilter {
        ExceptionFilter {
            filter: id.into(),
            label: id.into(),
            description: None,
            default: false,
            supports_condition: false,
        }
    }

    fn lldb_dap() -> Capabilities {
        Capabilities {
            function_breakpoints: true,
            exception_filters: [
                "cpp_catch",
                "cpp_throw",
                "objc_catch",
                "objc_throw",
                "swift_catch",
                "swift_throw",
            ]
            .map(filter)
            .to_vec(),
            ..Capabilities::default()
        }
    }

    fn ids(filters: &[ExceptionFilter]) -> Vec<&str> {
        filters.iter().map(|f| f.filter.as_str()).collect()
    }

    const RUST: Languages = Languages {
        rust: true,
        apple: false,
        c_family: false,
    };

    #[test]
    fn a_rust_project_gets_rust_panic_instead_of_the_c_family_filters() {
        assert_eq!(ids(&offered(AdapterKind::Lldb, &lldb_dap(), RUST)), [RUST_PANIC]);
        let swift = Languages {
            rust: false,
            apple: true,
            c_family: false,
        };
        assert_eq!(ids(&offered(AdapterKind::Lldb, &lldb_dap(), swift)).len(), 6);
        let cmake = Languages {
            rust: false,
            apple: false,
            c_family: true,
        };
        assert_eq!(
            ids(&offered(AdapterKind::Lldb, &lldb_dap(), cmake)),
            ["cpp_catch", "cpp_throw"]
        );
    }

    #[test]
    fn an_unknown_project_keeps_every_filter_and_codelldbs_own_panic_filter_is_used() {
        let unknown = Languages {
            rust: false,
            apple: false,
            c_family: false,
        };
        assert_eq!(ids(&offered(AdapterKind::Lldb, &lldb_dap(), unknown)).len(), 6);
        let codelldb = Capabilities {
            function_breakpoints: true,
            exception_filters: vec![filter("cpp_throw"), filter(RUST_PANIC)],
            ..Capabilities::default()
        };
        assert_eq!(ids(&offered(AdapterKind::Lldb, &codelldb, RUST)), [RUST_PANIC]);
        let wanted = vec![RUST_PANIC.to_string()];
        assert_eq!(split(&wanted, &codelldb), (wanted.clone(), Vec::new()));
    }

    #[test]
    fn rust_panic_becomes_a_function_breakpoint_where_the_adapter_has_no_filter() {
        let wanted = vec![RUST_PANIC.to_string(), "cpp_throw".to_string()];
        assert_eq!(
            split(&wanted, &lldb_dap()),
            (vec!["cpp_throw".to_string()], vec![PANIC_FUNCTION])
        );
        assert_eq!(split(&["cpp_throw".into()], &lldb_dap()).1, Vec::<&str>::new());
        let no_functions = Capabilities {
            function_breakpoints: false,
            ..lldb_dap()
        };
        assert!(offered(AdapterKind::Lldb, &no_functions, RUST).is_empty());
    }

    #[test]
    fn the_root_tells_the_languages() {
        let dir = ide_core::testing::scratch("dap-filters");
        assert!(Languages::of(dir.path()).unknown());
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(Languages::of(dir.path()), RUST);
        std::fs::create_dir(dir.path().join("App.xcodeproj")).unwrap();
        assert!(Languages::of(dir.path()).apple);
    }
}
