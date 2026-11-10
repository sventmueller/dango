//! The line under an editor: how many problems its file has, what its language server is doing, and the one offer to
//! install a missing server.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, sel};
use objc2_app_kit::{
    NSBox, NSBoxType, NSButton, NSCellImagePosition, NSColor, NSControlSize, NSFont, NSLayoutConstraintOrientation,
    NSLineBreakMode, NSStackView, NSStackViewGravity, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::{t, tf};

use crate::editor::EditorTextView;
use crate::util::{label, ns, set_action, symbol};

/// The height of the bar.
pub const HEIGHT: f64 = 24.0;

/// What the bar's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Install,
    Cancel,
    Retry,
    Restart,
}

impl Action {
    fn title(self) -> &'static str {
        match self {
            Action::None => "",
            Action::Install => t("Installieren"),
            Action::Cancel => t("Abbrechen"),
            Action::Retry => t("Erneut versuchen"),
            Action::Restart => t("Neu starten"),
        }
    }
}

/// The controls of one editor's bar.
pub struct Bar {
    problems: Retained<NSButton>,
    crumbs: Retained<NSButton>,
    server: Retained<NSTextField>,
    button: Retained<NSButton>,
}

/// The bar of `view`, with a line above it.
pub fn bar(mtm: MainThreadMarker, view: &EditorTextView) -> Retained<NSView> {
    // SAFETY: A nil target until `set_action` below, which checks that the view answers the action.
    let problems = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), None, None, mtm) };
    problems.setBordered(false);
    problems.setImagePosition(NSCellImagePosition::ImageLeading);
    problems.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    problems.setToolTip(Some(&ns(t("Zum nächsten Problem"))));
    set_action(&problems, view, sel!(ideNextProblem:));
    // SAFETY: As above.
    let crumbs = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), None, None, mtm) };
    crumbs.setBordered(false);
    crumbs.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    crumbs.setToolTip(Some(&ns(t("Gehe zu Symbol …"))));
    crumbs.setLineBreakMode(NSLineBreakMode::ByTruncatingHead);
    crumbs.setContentCompressionResistancePriority_forOrientation(150.0, NSLayoutConstraintOrientation::Horizontal);
    set_action(&crumbs, view, sel!(ideSymbols:));
    crumbs.setHidden(true);
    let server = label(mtm, "");
    server.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    server.setTextColor(Some(&NSColor::secondaryLabelColor()));
    server.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    server.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    // SAFETY: As above.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), None, None, mtm) };
    button.setControlSize(NSControlSize::Small);
    button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    set_action(&button, view, sel!(ideLspAction:));
    button.setHidden(true);
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(10.0);
    stack.setEdgeInsets(NSEdgeInsets {
        top: 0.0,
        left: 10.0,
        bottom: 0.0,
        right: 10.0,
    });
    stack.addView_inGravity(&problems, NSStackViewGravity::Leading);
    stack.addView_inGravity(&crumbs, NSStackViewGravity::Leading);
    stack.addView_inGravity(&server, NSStackViewGravity::Leading);
    stack.addView_inGravity(&button, NSStackViewGravity::Trailing);
    crate::editor::status_items(mtm, view, &stack);
    let line = NSBox::new(mtm);
    line.setBoxType(NSBoxType::Separator);
    let container = NSView::new(mtm);
    container.addSubview(&line);
    container.addSubview(&stack);
    line.setTranslatesAutoresizingMaskIntoConstraints(false);
    stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        line.leadingAnchor().constraintEqualToAnchor(&container.leadingAnchor()),
        line.trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        line.topAnchor().constraintEqualToAnchor(&container.topAnchor()),
        stack
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor()),
        stack
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        stack
            .topAnchor()
            .constraintEqualToAnchor_constant(&container.topAnchor(), 1.0),
        stack.bottomAnchor().constraintEqualToAnchor(&container.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    view.ivars().lsp.borrow_mut().status = Some(Bar {
        problems,
        crumbs,
        server,
        button,
    });
    refresh(view);
    container
}

/// Shows the current problem counts and the state of the server.
pub fn refresh(view: &EditorTextView) {
    let counts = super::problem_counts(view);
    let (text, action) = super::server_status(view);
    let crumbs = super::outline::breadcrumbs(view).join(" › ");
    let state = view.ivars().lsp.borrow();
    let Some(bar) = state.status.as_ref() else { return };
    match counts {
        None => bar.problems.setHidden(true),
        Some((errors, warnings)) => {
            bar.problems.setHidden(false);
            bar.problems.setTitle(&ns(&problems_text(errors, warnings)));
            let (name, color) = if errors > 0 {
                ("xmark.octagon.fill", NSColor::systemRedColor())
            } else if warnings > 0 {
                ("exclamationmark.triangle.fill", NSColor::systemOrangeColor())
            } else {
                ("checkmark.circle", NSColor::secondaryLabelColor())
            };
            bar.problems.setImage(symbol(name, t("Probleme")).as_deref());
            bar.problems.setContentTintColor(Some(&color));
        }
    }
    bar.crumbs.setHidden(crumbs.is_empty());
    bar.crumbs.setTitle(&ns(&crumbs));
    bar.server.setStringValue(&ns(&text));
    bar.server.setToolTip(Some(&ns(&text)));
    bar.button.setHidden(action == Action::None);
    bar.button.setTitle(&ns(action.title()));
}

fn problems_text(errors: usize, warnings: usize) -> String {
    let errors = match errors {
        0 => None,
        1 => Some(t("1 Fehler").to_string()),
        n => Some(tf("{} Fehler", &[&n.to_string()])),
    };
    let warnings = match warnings {
        0 => None,
        1 => Some(t("1 Warnung").to_string()),
        n => Some(tf("{} Warnungen", &[&n.to_string()])),
    };
    match (errors, warnings) {
        (None, None) => t("Keine Probleme").to_string(),
        (Some(one), None) | (None, Some(one)) => one,
        (Some(errors), Some(warnings)) => format!("{errors}, {warnings}"),
    }
}

/// The breadcrumbs the bar of `view` shows, for the self-test.
#[cfg(feature = "selftest")]
pub fn crumbs(view: &EditorTextView) -> String {
    let state = view.ivars().lsp.borrow();
    state
        .status
        .as_ref()
        .filter(|bar| !bar.crumbs.isHidden())
        .map(|bar| bar.crumbs.title().to_string())
        .unwrap_or_default()
}

/// What the bar of `view` shows, for the self-test: the problems and the server's line.
#[cfg(feature = "selftest")]
pub fn shown(view: &EditorTextView) -> (String, String) {
    let state = view.ivars().lsp.borrow();
    state.status.as_ref().map_or_else(Default::default, |bar| {
        (bar.problems.title().to_string(), bar.server.stringValue().to_string())
    })
}
