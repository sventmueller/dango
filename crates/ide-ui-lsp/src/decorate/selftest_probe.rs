//! The self-test's hooks of `decorate.rs`.

use super::*;

pub(crate) fn set_diagnostics(view: &dyn Editor, diagnostics: &[Diagnostic]) {
    let source = super::super::main_slot(view);
    set_diagnostics_from(view, source, diagnostics);
}
