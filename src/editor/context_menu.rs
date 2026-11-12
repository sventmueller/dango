//! The editor's context menu: the text view's own entries, with the Format menu in a Markdown document or the
//! language server's formatting and code actions in code.

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::NSCopying;
use splat_i18n::t;

use super::EditorTextView;
use crate::util::{ns, set_target};

/// Adds the format entry and the code actions to `menu`, the text view's context menu, after its cut, copy and paste.
pub fn extend(view: &EditorTextView, menu: &NSMenu) {
    navigation(view, menu);
    let items: Vec<Retained<NSMenuItem>> = format_item(view)
        .into_iter()
        .chain(action_items(view))
        .chain(super::blame::context_items(view))
        .collect();
    if items.is_empty() {
        return;
    }
    let at = after_paste(menu);
    menu.insertItem_atIndex(&NSMenuItem::separatorItem(view.mtm()), at);
    for (offset, item) in (1..).zip(&items) {
        aim(item, view);
        menu.insertItem_atIndex(item, at + offset);
    }
}

/// Copies of the menu bar's code action items, where the language server offers code actions (E82).
fn action_items(view: &EditorTextView) -> Vec<Retained<NSMenuItem>> {
    let Some(main) = NSApplication::sharedApplication(view.mtm()).mainMenu() else {
        return vec![];
    };
    if crate::lsp::validate(view, Some(sel!(ideQuickFix:))) != Some(true) {
        return vec![];
    }
    [sel!(ideQuickFix:), sel!(ideRefactor:), sel!(ideSourceActions:)]
        .into_iter()
        .filter_map(|action| find(&main, action).map(|item| item.copy()))
        .collect()
}

/// Puts copies of the menu bar's places to go from the symbol at the top of `menu`, where a language server serves the
/// editor (E85); those the server does not offer show greyed out.
fn navigation(view: &EditorTextView, menu: &NSMenu) {
    let Some(main) = NSApplication::sharedApplication(view.mtm()).mainMenu() else {
        return;
    };
    if crate::lsp::validate(view, Some(sel!(ideDefinition:))) != Some(true) {
        return;
    }
    let actions = [
        sel!(ideDefinition:),
        sel!(ideImplementation:),
        sel!(ideTypeDefinition:),
        sel!(ideDeclaration:),
    ];
    let items: Vec<Retained<NSMenuItem>> = actions
        .into_iter()
        .filter_map(|action| find(&main, action).map(|item| item.copy()))
        .collect();
    for (at, item) in (0..).zip(&items) {
        aim(item, view);
        menu.insertItem_atIndex(item, at);
    }
    if !items.is_empty() {
        menu.insertItem_atIndex(
            &NSMenuItem::separatorItem(view.mtm()),
            isize::try_from(items.len()).unwrap_or(0),
        );
    }
}

/// A copy of the menu bar's Format menu for Markdown, or of its "Dokument formatieren" where a language server
/// formats the code; the copy keeps the shortcuts the keymap gave the original.
fn format_item(view: &EditorTextView) -> Option<Retained<NSMenuItem>> {
    let main = NSApplication::sharedApplication(view.mtm()).mainMenu()?;
    if view.document().path().is_some_and(crate::preview::is_markdown) {
        let format = main.itemWithTitle(&ns(t("Format")))?;
        let copy = format.copy();
        copy.setSubmenu(Some(&format.submenu()?.copy()));
        return Some(copy);
    }
    if crate::lsp::validate(view, Some(sel!(ideFormat:))) != Some(true) {
        return None;
    }
    find(&main, sel!(ideFormat:)).map(|item| item.copy())
}

fn find(menu: &NSMenu, action: Sel) -> Option<Retained<NSMenuItem>> {
    menu.itemArray().iter().find_map(|item| {
        if item.action() == Some(action) {
            return Some(item);
        }
        item.submenu().and_then(|submenu| find(&submenu, action))
    })
}

/// Sends the actions of `item` and its submenu to `view`, which need not be the first responder when it is clicked.
fn aim(item: &NSMenuItem, view: &EditorTextView) {
    match item.submenu() {
        Some(submenu) => {
            for child in submenu.itemArray().iter() {
                aim(&child, view);
            }
        }
        None if item.action().is_some() => set_target(item, view),
        None => {}
    }
}

/// Where the group with Paste ends, or the menu's end.
fn after_paste(menu: &NSMenu) -> isize {
    let items = menu.itemArray();
    let paste = items.iter().position(|item| item.action() == Some(sel!(paste:)));
    let end = paste.and_then(|at| {
        items
            .iter()
            .skip(at)
            .position(|item| item.isSeparatorItem())
            .map(|offset| at + offset)
    });
    isize::try_from(end.unwrap_or(items.len())).unwrap_or(menu.numberOfItems())
}
