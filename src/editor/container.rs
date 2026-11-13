//! The editor with what surrounds it: blame column, line numbers, minimap, Sticky Scroll, status bar and the bar
//! above.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker};

use super::view::EditorTextView;
use super::{blame_column, conflict, minimap, ruler, sticky};

/// The scroll view with the line numbers beside it and the status bar of the language server below.
pub(super) fn editor_container(
    mtm: MainThreadMarker,
    scroll: &objc2_app_kit::NSScrollView,
    view: &EditorTextView,
) -> (Retained<objc2_app_kit::NSView>, Retained<ruler::LineNumbers>) {
    let gutter = ruler::LineNumbers::new(mtm, scroll, view);
    let blame = blame_column::BlameColumn::new(mtm, scroll, view);
    let status = crate::lsp::status::bar(mtm, view);
    let bar = conflict::Bar::new(mtm);
    let map = minimap::Minimap::new(mtm, scroll, view);
    let container = objc2_app_kit::NSView::new(mtm);
    container.addSubview(&map);
    map.setTranslatesAutoresizingMaskIntoConstraints(false);
    container.addSubview(&blame);
    container.addSubview(&gutter);
    container.addSubview(scroll);
    container.addSubview(&status);
    container.addSubview(bar.view());
    let sticky = sticky::Sticky::new(mtm, scroll, view);
    container.addSubview(&sticky);
    sticky.setTranslatesAutoresizingMaskIntoConstraints(false);
    bar.view().setTranslatesAutoresizingMaskIntoConstraints(false);
    blame.setTranslatesAutoresizingMaskIntoConstraints(false);
    gutter.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
    status.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        bar.view().topAnchor().constraintEqualToAnchor(&container.topAnchor()),
        bar.view()
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor()),
        bar.view()
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        // Code and its line numbers stay left to right in a right-to-left interface (E3); the bars around follow it.
        blame.leftAnchor().constraintEqualToAnchor(&container.leftAnchor()),
        blame.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        blame.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        gutter.leftAnchor().constraintEqualToAnchor(&blame.rightAnchor()),
        gutter.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        gutter.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        scroll.leftAnchor().constraintEqualToAnchor(&gutter.rightAnchor()),
        scroll.rightAnchor().constraintEqualToAnchor(&map.leftAnchor()),
        map.rightAnchor().constraintEqualToAnchor(&container.rightAnchor()),
        sticky.leftAnchor().constraintEqualToAnchor(&container.leftAnchor()),
        sticky.rightAnchor().constraintEqualToAnchor(&scroll.rightAnchor()),
        sticky.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        map.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        map.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        scroll.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        scroll.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        status
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor()),
        status
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor()),
        status.bottomAnchor().constraintEqualToAnchor(&container.bottomAnchor()),
        status
            .heightAnchor()
            .constraintEqualToConstant(crate::lsp::status::HEIGHT),
    ] {
        constraint.setActive(true);
    }
    let width = gutter.widthAnchor().constraintEqualToConstant(0.0);
    width.setActive(true);
    gutter.set_width_constraint(width);
    let width = blame.widthAnchor().constraintEqualToConstant(0.0);
    width.setActive(true);
    blame.set_width_constraint(width);
    let width = map.widthAnchor().constraintEqualToConstant(0.0);
    width.setActive(true);
    map.set_width_constraint(width);
    let height = sticky.heightAnchor().constraintEqualToConstant(0.0);
    height.setActive(true);
    sticky.set_height_constraint(height);
    *view.ivars().bar.borrow_mut() = Some(bar);
    (container, gutter)
}
