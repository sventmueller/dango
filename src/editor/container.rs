//! The editor with what surrounds it: blame column, line numbers, minimap, Sticky Scroll, status bar and the bar
//! above.

use std::cell::{Cell, RefCell};

use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker};

use super::view::EditorTextView;
use super::{blame_column, conflict, find_editor, minimap, overlay, ruler, sticky, viewport, whitespace};
use crate::util::{activate, auto_layout, pin_sides};

/// The views an editor has around and over its text.
#[derive(Default)]
pub(crate) struct Around {
    /// The line number column, redrawn when lines change.
    pub(super) gutter: RefCell<Option<Weak<ruler::LineNumbers>>>,
    /// The view that shows its whitespace.
    pub(super) whitespace: RefCell<Option<Weak<whitespace::Overlay>>>,
    /// The minimap beside it (E107).
    pub(super) minimap: RefCell<Option<Weak<minimap::Minimap>>>,
    /// The header lines held at its top (E108).
    pub(super) sticky: RefCell<Option<Weak<sticky::Sticky>>>,
    /// What is drawn over the text: fold placeholders and the further carets.
    pub(super) overlay: RefCell<Option<Weak<overlay::Overlay>>>,
    /// The bar above the editor.
    pub(super) bar: RefCell<Option<conflict::Bar>>,
    /// The line number column, the minimap and the sticky headers draw again on the next turn of the run loop.
    pub(super) due: Cell<bool>,
}

/// The scroll view with the line numbers beside it and the status bar of the language server below.
pub(super) fn editor_container(
    mtm: MainThreadMarker,
    scroll: &objc2_app_kit::NSScrollView,
    view: &EditorTextView,
) -> (Retained<objc2_app_kit::NSView>, Retained<ruler::LineNumbers>) {
    let (column, gutter) = ruler::column(mtm, view);
    let blame = blame_column::BlameColumn::new(mtm, scroll, view);
    let status = ide_ui_lsp::status::bar(mtm, view);
    let bar = conflict::Bar::new(mtm);
    let map = minimap::Minimap::new(mtm, scroll, view);
    let container = objc2_app_kit::NSView::new(mtm);
    container.addSubview(&map);
    container.addSubview(&blame);
    container.addSubview(&column);
    container.addSubview(scroll);
    container.addSubview(&status);
    container.addSubview(bar.view());
    let sticky = sticky::Sticky::new(mtm, scroll, view);
    container.addSubview(&sticky);
    viewport::follow(scroll, view);
    auto_layout(&[&map, &sticky, bar.view(), &blame, &column, scroll, &status]);
    pin_sides(bar.view(), &container, 0.0);
    pin_sides(&status, &container, 0.0);
    activate([
        bar.view().topAnchor().constraintEqualToAnchor(&container.topAnchor()),
        // Code and its line numbers stay left to right in a right-to-left interface (E3); the bars around follow it.
        blame.leftAnchor().constraintEqualToAnchor(&container.leftAnchor()),
        blame.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        blame.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        column.leftAnchor().constraintEqualToAnchor(&blame.rightAnchor()),
        column.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        column.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        scroll.leftAnchor().constraintEqualToAnchor(&column.rightAnchor()),
        scroll.rightAnchor().constraintEqualToAnchor(&map.leftAnchor()),
        map.rightAnchor().constraintEqualToAnchor(&container.rightAnchor()),
        sticky.leftAnchor().constraintEqualToAnchor(&container.leftAnchor()),
        sticky.rightAnchor().constraintEqualToAnchor(&scroll.rightAnchor()),
        sticky.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        map.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        map.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        scroll.topAnchor().constraintEqualToAnchor(&bar.view().bottomAnchor()),
        scroll.bottomAnchor().constraintEqualToAnchor(&status.topAnchor()),
        status.bottomAnchor().constraintEqualToAnchor(&container.bottomAnchor()),
        status
            .heightAnchor()
            .constraintEqualToConstant(ide_ui_lsp::status::HEIGHT),
    ]);
    let width = column.widthAnchor().constraintEqualToConstant(0.0);
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
    *view.ivars().around.bar.borrow_mut() = Some(bar);
    (container, gutter)
}

/// Redraws the line number column of `view`, after its diagnostics changed, once for any number of asks in a turn of
/// the run loop.
pub(crate) fn redraw_gutter(view: &EditorTextView) {
    if view.ivars().around.due.replace(true) {
        return;
    }
    let serial = view.ivars().serial;
    crate::util::main_soon(move || {
        if let Some(view) = find_editor(serial) {
            redraw_gutter_now(&view);
        }
    });
}

fn redraw_gutter_now(view: &EditorTextView) {
    view.ivars().around.due.set(false);
    if let Some(gutter) = view.ivars().around.gutter.borrow().as_ref().and_then(Weak::load) {
        gutter.redraw();
    }
    minimap::redraw(view);
    sticky::refresh(view);
}
