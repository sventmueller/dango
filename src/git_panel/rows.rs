//! The rows of the Git list: section headers with their "all" buttons, files with status letters and buttons that show
//! on hover, commits; and the list itself, which takes keyboard commands.

use std::cell::OnceCell;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityCustomAction, NSButton, NSColor, NSEvent, NSEventModifierFlags, NSFont,
    NSLayoutConstraint, NSLineBreakMode, NSOutlineView, NSStackView, NSTableCellView, NSTrackingArea,
    NSTrackingAreaOptions, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::model::{FileRow, Mark, RowKey, Section};
use super::panel::{GitPanel, with_panel};
use crate::util::{label, ns, symbol_button};

const SECTION_HEIGHT: f64 = 26.0;
const FILE_HEIGHT: f64 = 24.0;
const COMMIT_HEIGHT: f64 = 36.0;
const ACTION_POINTS: f64 = 12.0;

/// Return and the Return of the number block.
const RETURN_KEYS: [u16; 2] = [36, 76];
const SPACE_KEY: u16 = 49;
const DELETE_KEY: u16 = 51;

/// The height of the row for `key`.
pub fn height(key: Option<&RowKey>) -> f64 {
    match key {
        Some(RowKey::Section(_)) => SECTION_HEIGHT,
        Some(RowKey::Commit(_) | RowKey::Worktree(_)) => COMMIT_HEIGHT,
        _ => FILE_HEIGHT,
    }
}

/// How far AppKit's source list sets its disclosure triangles in past the panel's title.
const OUTDENT: f64 = 5.5;

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `GitOutlineView` does not implement Drop.
    /// Return shows the diff, Space stages or unstages, ⌘⌫ discards.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitOutlineView"]
    pub struct GitOutlineView;

    unsafe impl NSObjectProtocol for GitOutlineView {}

    impl GitOutlineView {
        /// The disclosure triangle where the panel's title begins, not in the source list's margin.
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn outline_cell_frame(&self, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfOutlineCellAtRow: row] };
            crate::window::file_outline::outdent(self, frame, OUTDENT, false)
        }

        /// The rows moved along with the triangle.
        #[unsafe(method(frameOfCellAtColumn:row:))]
        fn cell_frame(&self, column: isize, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfCellAtColumn: column, row: row] };
            if column != 0 {
                return frame;
            }
            crate::window::file_outline::outdent(self, frame, OUTDENT, true)
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let handled = guarded_key(self, event);
            if !handled {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

fn guarded_key(view: &GitOutlineView, event: &NSEvent) -> bool {
    crate::util::guarded(false, || {
        let Some(panel) = view.panel() else { return false };
        let code = event.keyCode();
        let command = event.modifierFlags().contains(NSEventModifierFlags::Command);
        if RETURN_KEYS.contains(&code) {
            panel.activate_row(view.selectedRow());
        } else if code == SPACE_KEY {
            panel.toggle_stage(panel.selected_keys());
        } else if code == DELETE_KEY && command {
            panel.discard(panel.selected_keys());
        } else {
            return false;
        }
        true
    })
}

impl GitOutlineView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<GitOutlineView> {
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![GitOutlineView::alloc(mtm), initWithFrame: frame] }
    }

    fn panel(&self) -> Option<Retained<GitPanel>> {
        // SAFETY: Only `ui::outline` makes this view, and it sets the panel as the data source.
        let source = unsafe { self.dataSource() }?;
        let object: Retained<AnyObject> = source.into();
        object.downcast::<GitPanel>().ok()
    }
}

#[derive(Default)]
pub struct HoverIvars {
    actions: OnceCell<Retained<NSView>>,
    /// Keeps the text clear of the buttons while they show.
    beside_actions: OnceCell<Retained<NSLayoutConstraint>>,
    to_edge: OnceCell<Retained<NSLayoutConstraint>>,
}

define_class!(
    // SAFETY: NSTableCellView has no subclassing requirements and `HoverCell` does not implement Drop.
    /// A file row whose buttons show while the mouse is over it.
    #[unsafe(super(NSTableCellView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitHoverCell"]
    #[ivars = HoverIvars]
    pub struct HoverCell;

    unsafe impl NSObjectProtocol for HoverCell {}

    impl HoverCell {
        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            crate::util::guarded((), || self.show_actions(true));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            crate::util::guarded((), || self.show_actions(false));
        }
    }
);

impl HoverCell {
    fn new(mtm: MainThreadMarker) -> Retained<HoverCell> {
        let this = HoverCell::alloc(mtm).set_ivars(HoverIvars::default());
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, FILE_HEIGHT));
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let cell: Retained<HoverCell> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY: The cell owns the area it adds to itself; AppKit keeps the owner unretained, and the area goes with
        // the cell.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                options,
                Some(&cell),
                None,
            )
        };
        cell.addTrackingArea(&area);
        cell
    }

    fn show_actions(&self, visible: bool) {
        let ivars = self.ivars();
        if let (Some(actions), Some(beside), Some(edge)) =
            (ivars.actions.get(), ivars.beside_actions.get(), ivars.to_edge.get())
        {
            actions.setHidden(!visible);
            edge.setActive(!visible);
            beside.setActive(visible);
        }
    }
}

impl GitPanel {
    pub fn row_view(&self, key: &RowKey) -> Option<Retained<NSView>> {
        let state = self.read();
        let mtm = self.mtm();
        match key {
            RowKey::Section(section) => Some(section_cell(mtm, self, *section, state.model.len(*section))),
            RowKey::File(section, path) => {
                let row = state.model.file(*section, path)?.clone();
                drop(state);
                Some(self.file_cell(mtm, *section, &row))
            }
            RowKey::Commit(hash) => {
                let commit = state.model.commit(hash)?;
                Some(commit_cell(mtm, commit))
            }
            RowKey::Worktree(path) => {
                let tree = state.model.tree(path)?.clone();
                drop(state);
                Some(self.tree_cell(mtm, &tree))
            }
            RowKey::WorktreeFile(tree, file) => {
                let change = state.model.tree(tree)?.changed.iter().find(|f| f.path == *file)?.change;
                drop(state);
                Some(self.tree_file_cell(mtm, tree, file, change))
            }
        }
    }

    fn file_cell(&self, mtm: MainThreadMarker, section: Section, row: &FileRow) -> Retained<NSView> {
        let cell = HoverCell::new(mtm);
        let letter = label(mtm, row.mark.letter());
        letter.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
            11.0,
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightBold },
        )));
        letter.setTextColor(Some(&mark_color(row.mark)));
        letter.setAlignment(objc2_app_kit::NSTextAlignment::Center);
        let path = row.path();
        let name = label(
            mtm,
            &path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        );
        name.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        if row.mark == Mark::Deleted {
            name.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        let folder = label(
            mtm,
            &path.parent().map_or_else(String::new, |p| p.display().to_string()),
        );
        folder.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
        folder.setTextColor(Some(&NSColor::secondaryLabelColor()));
        folder.setLineBreakMode(NSLineBreakMode::ByTruncatingHead);
        folder.setContentCompressionResistancePriority_forOrientation(
            100.0,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        let text = NSStackView::new(mtm);
        text.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        text.setSpacing(5.0);
        for view in [&*letter as &NSView, &name, &folder] {
            text.addArrangedSubview(view);
        }
        letter.widthAnchor().constraintEqualToConstant(12.0).setActive(true);
        let actions = self.row_actions(mtm, section);
        actions.setHidden(true);
        cell.addSubview(&text);
        cell.addSubview(&actions);
        // SAFETY: The label is a subview of the cell, which keeps it.
        unsafe { cell.setTextField(Some(&name)) };
        for view in [&*text as &NSView, &actions] {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
            view.centerYAnchor()
                .constraintEqualToAnchor(&cell.centerYAnchor())
                .setActive(true);
        }
        text.leadingAnchor()
            .constraintEqualToAnchor(&cell.leadingAnchor())
            .setActive(true);
        actions
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -4.0)
            .setActive(true);
        let to_edge = text
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0);
        let beside = text
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&actions.leadingAnchor(), -4.0);
        to_edge.setActive(true);
        let ivars = cell.ivars();
        let _ = ivars.actions.set(actions.into_super());
        let _ = ivars.beside_actions.set(beside);
        let _ = ivars.to_edge.set(to_edge);
        let described = describe(section, row);
        cell.setToolTip(Some(&ns(&described)));
        cell.setAccessibilityLabel(Some(&ns(&described)));
        let custom = self.accessibility_actions(section, row);
        cell.setAccessibilityCustomActions(Some(&NSArray::from_retained_slice(&custom)));
        cell.into_super().into_super()
    }

    /// The buttons a file row shows on hover.
    fn row_actions(&self, mtm: MainThreadMarker, section: Section) -> Retained<NSStackView> {
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(2.0);
        for (name, title, action) in file_actions(section) {
            let button: Retained<NSButton> = symbol_button(mtm, name, title, ACTION_POINTS, Some(self), action);
            button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
            stack.addArrangedSubview(&button);
        }
        stack
    }

    /// The row's actions for VoiceOver, which reaches no hover buttons.
    fn accessibility_actions(&self, section: Section, row: &FileRow) -> Vec<Retained<NSAccessibilityCustomAction>> {
        let mut actions: Vec<(&str, Sel)> = vec![];
        if section != Section::Conflicts {
            actions.push((t("Diff zeigen"), sel!(ideGitDiff:)));
        }
        actions.extend(
            file_actions(section)
                .into_iter()
                .map(|(_, title, action)| (title, action)),
        );
        let key = RowKey::File(section, row.path().to_path_buf());
        actions
            .into_iter()
            .map(|(title, action)| {
                let id = self.id();
                let key = key.clone();
                let handler = RcBlock::new(move || -> Bool {
                    crate::util::guarded((), || with_panel(id, |p| p.run_row_action(action, vec![key.clone()])));
                    Bool::YES
                });
                NSAccessibilityCustomAction::initWithName_handler(
                    NSAccessibilityCustomAction::alloc(),
                    &ns(title),
                    Some(&handler),
                )
            })
            .collect()
    }

    /// Runs the row action `action` on `keys`, as its button or menu item would.
    pub fn run_row_action(&self, action: Sel, keys: Vec<RowKey>) {
        if action == sel!(ideGitStage:) {
            self.stage(keys);
        } else if action == sel!(ideGitUnstage:) {
            self.unstage(keys);
        } else if action == sel!(ideGitDiscard:) {
            self.discard(keys);
        } else if action == sel!(ideGitOpen:) {
            self.open(keys);
        } else if action == sel!(ideGitResolve:) {
            self.resolve(keys);
        } else if action == sel!(ideGitDiff:)
            && let Some(key) = keys.first()
        {
            self.show_diff(key, false);
        } else if action == sel!(ideGitTakeOver:) {
            self.take_over(&keys);
        } else if action == sel!(ideGitOpenThread:) {
            self.open_thread(&keys);
        } else if action == sel!(ideGitKeepBranch:) {
            self.keep_branch(&keys);
        } else if action == sel!(ideGitRemoveTree:) {
            self.remove_tree(&keys);
        }
    }
}

/// The symbol, title and action of each button of a file row.
fn file_actions(section: Section) -> Vec<(&'static str, &'static str, Sel)> {
    let open = ("doc.text", t("Im Editor öffnen"), sel!(ideGitOpen:));
    match section {
        Section::Conflicts => vec![open, ("checkmark", t("Als gelöst markieren"), sel!(ideGitResolve:))],
        Section::Staged => vec![open, ("minus", t("Zurücknehmen"), sel!(ideGitUnstage:))],
        _ => vec![
            ("arrow.uturn.backward", t("Verwerfen"), sel!(ideGitDiscard:)),
            open,
            ("plus", t("Vormerken"), sel!(ideGitStage:)),
        ],
    }
}

/// The file's name, folder and state in words, for VoiceOver and the tooltip.
fn describe(section: Section, row: &FileRow) -> String {
    let path = row.path().display().to_string();
    let state = match row.from() {
        Some(from) => tf("{}, von {}", &[row.mark.word(), &from.display().to_string()]),
        None => row.mark.word().to_string(),
    };
    let side = match section {
        Section::Conflicts => t("ungelöst"),
        Section::Staged => t("vorgemerkt"),
        _ => t("nicht vorgemerkt"),
    };
    tf("{}: {}, {}", &[&path, &state, side])
}

/// Colours that keep their meaning in light and dark mode and with more contrast; the letter carries it as well.
pub(super) fn mark_color(mark: Mark) -> Retained<NSColor> {
    match mark {
        Mark::Modified | Mark::TypeChanged => NSColor::systemOrangeColor(),
        Mark::Added | Mark::Untracked => NSColor::systemGreenColor(),
        Mark::Deleted => NSColor::systemRedColor(),
        Mark::Renamed | Mark::Copied => NSColor::systemBlueColor(),
        Mark::Conflict => NSColor::systemPurpleColor(),
    }
}

fn section_cell(mtm: MainThreadMarker, panel: &GitPanel, section: Section, count: usize) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let title = label(mtm, &section.title().to_uppercase());
    title.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::smallSystemFontSize())));
    title.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let number = label(mtm, &count.to_string());
    number.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
        NSFont::smallSystemFontSize(),
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightRegular },
    )));
    number.setTextColor(Some(&NSColor::tertiaryLabelColor()));
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(5.0);
    row.addArrangedSubview(&title);
    row.addArrangedSubview(&number);
    cell.addSubview(&row);
    row.setTranslatesAutoresizingMaskIntoConstraints(false);
    row.leadingAnchor()
        .constraintEqualToAnchor(&cell.leadingAnchor())
        .setActive(true);
    row.centerYAnchor()
        .constraintEqualToAnchor(&cell.centerYAnchor())
        .setActive(true);
    let all = match section {
        Section::Staged => Some(("minus", t("Alle zurücknehmen"), sel!(ideGitUnstageAll:))),
        Section::Changes if count > 0 => Some(("plus", t("Alle vormerken"), sel!(ideGitStageAll:))),
        _ => None,
    };
    if let Some((name, description, action)) = all {
        let button = symbol_button(mtm, name, description, ACTION_POINTS, Some(panel), action);
        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        cell.addSubview(&button);
        button.setTranslatesAutoresizingMaskIntoConstraints(false);
        button
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -4.0)
            .setActive(true);
        button
            .centerYAnchor()
            .constraintEqualToAnchor(&cell.centerYAnchor())
            .setActive(true);
    }
    let files = match (section, count) {
        (Section::Commits, _) => tf("{}, {} Commits", &[section.title(), &count.to_string()]),
        (_, 1) => tf("{}, 1 Datei", &[section.title()]),
        _ => tf("{}, {} Dateien", &[section.title(), &count.to_string()]),
    };
    cell.setAccessibilityLabel(Some(&ns(&files)));
    // SAFETY: The label is a subview of the cell, which keeps it.
    unsafe { cell.setTextField(Some(&title)) };
    cell.into_super()
}

fn commit_cell(mtm: MainThreadMarker, commit: &ide_git::Commit) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let subject = label(mtm, &commit.subject);
    subject.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let age = super::text::ago(now() - commit.author_time);
    let byline = label(mtm, &tf("{} · {}", &[&commit.author_name, &age]));
    byline.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    byline.setTextColor(Some(&NSColor::secondaryLabelColor()));
    byline.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let lines = NSStackView::new(mtm);
    lines.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    lines.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    lines.setSpacing(1.0);
    lines.addArrangedSubview(&subject);
    lines.addArrangedSubview(&byline);
    cell.addSubview(&lines);
    lines.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        lines.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        lines
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        lines.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    let short: String = commit.hash.chars().take(8).collect();
    cell.setToolTip(Some(&ns(&format!(
        "{short}  {}\n{} <{}>",
        commit.subject, commit.author_name, commit.author_email
    ))));
    cell.setAccessibilityLabel(Some(&ns(&tf(
        "Commit {}: {}, von {}, {}",
        &[&short, &commit.subject, &commit.author_name, &age],
    ))));
    // SAFETY: The label is a subview of the cell, which keeps it.
    unsafe { cell.setTextField(Some(&subject)) };
    cell.into_super()
}

/// Seconds since 1970.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl GitPanel {
    /// What a click, the context menu or a key acts on: the clicked row, and the whole selection when it includes it.
    pub fn keys_for(&self, sender: Option<&AnyObject>) -> Vec<RowKey> {
        if sender.is_some_and(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>().is_some()) {
            return self.read().menu_keys.clone();
        }
        let outline = &self.ui().outline;
        let row = sender
            .and_then(|s| s.downcast_ref::<NSView>())
            .map_or(-1, |view| outline.rowForView(view));
        self.keys_around(row)
    }

    /// The row `row`, or the selection when `row` is part of it or none.
    fn keys_around(&self, row: isize) -> Vec<RowKey> {
        let outline = &self.ui().outline;
        if row >= 0 && !outline.isRowSelected(row) {
            return outline
                .itemAtRow(row)
                .and_then(|i| super::panel::key_of(&i))
                .into_iter()
                .collect();
        }
        self.selected_keys()
    }

    pub fn selected_keys(&self) -> Vec<RowKey> {
        let outline = &self.ui().outline;
        (0..outline.numberOfRows())
            .filter(|row| outline.isRowSelected(*row))
            .filter_map(|row| outline.itemAtRow(row))
            .filter_map(|item| super::panel::key_of(&item))
            .collect()
    }

    /// A single click: a file shows its diff in the preview tab, unless the click adds to the selection.
    pub fn preview_row(&self, row: isize) {
        let outline = &self.ui().outline;
        let Some(item) = (row >= 0).then(|| outline.itemAtRow(row)).flatten() else {
            return;
        };
        let modifiers = objc2_app_kit::NSApplication::sharedApplication(self.mtm())
            .currentEvent()
            .map(|event| event.modifierFlags());
        let extends = modifiers.is_some_and(|flags| {
            flags.intersects(objc2_app_kit::NSEventModifierFlags::Shift | objc2_app_kit::NSEventModifierFlags::Command)
        });
        if !extends && let Some(key @ (RowKey::File(..) | RowKey::WorktreeFile(..))) = super::panel::key_of(&item) {
            self.show_diff(&key, true);
        }
    }

    /// Return or a double click: a file shows its diff, a section opens or closes.
    pub fn activate_row(&self, row: isize) {
        let outline = &self.ui().outline;
        let Some(item) = (row >= 0).then(|| outline.itemAtRow(row)).flatten() else {
            return;
        };
        match super::panel::key_of(&item) {
            Some(key @ RowKey::File(Section::Conflicts, _)) => self.open(vec![key]),
            Some(key @ RowKey::File(..)) => self.show_diff(&key, false),
            Some(RowKey::WorktreeFile(tree, file)) => self.show_tree_diff(&tree, &file, false),
            Some(RowKey::Section(_) | RowKey::Worktree(_)) => {
                // SAFETY: The item came from this outline view, which knows it.
                unsafe {
                    if outline.isItemExpanded(Some(&item)) {
                        outline.collapseItem(Some(&item));
                    } else {
                        outline.expandItem(Some(&item));
                    }
                }
            }
            _ => {}
        }
    }

    /// Space: stages what is unstaged and unstages what is staged; on a section's row, all of its files, as the
    /// section's button does.
    pub fn toggle_stage(&self, keys: Vec<RowKey>) {
        match keys.as_slice() {
            [RowKey::Section(Section::Changes)] => return self.stage_all(),
            [RowKey::Section(Section::Staged)] => return self.unstage_all(),
            _ => {}
        }
        let (staged, other): (Vec<RowKey>, Vec<RowKey>) = keys
            .into_iter()
            .partition(|k| matches!(k, RowKey::File(Section::Staged, _)));
        if !other.is_empty() {
            self.stage(other);
        }
        if !staged.is_empty() {
            self.unstage(staged);
        }
    }

    /// Reloads the list, keeping which sections are closed and which rows are selected.
    pub fn reload_outline(&self) {
        let outline = &self.ui().outline;
        let selected = self.selected_keys();
        outline.reloadData();
        let (sections, collapsed) = {
            let state = self.read();
            (state.model.sections(), state.collapsed.clone())
        };
        for section in sections.into_iter().filter(|s| !collapsed.contains(s)) {
            let item = self.item_for(&RowKey::Section(section));
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            unsafe { outline.expandItem(Some(&item)) };
        }
        let open: Vec<_> = self.read().open_trees.iter().cloned().collect();
        for path in open {
            let item = self.item_for(&RowKey::Worktree(path));
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            unsafe { outline.expandItem(Some(&item)) };
        }
        let rows = objc2_foundation::NSMutableIndexSet::new();
        for key in &selected {
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            let row = unsafe { outline.rowForItem(Some(&self.item_for(key))) };
            if let Ok(row) = usize::try_from(row) {
                rows.addIndex(row);
            }
        }
        outline.selectRowIndexes_byExtendingSelection(&rows, false);
    }

    /// The context menu for the clicked row, or the selection it belongs to.
    pub fn fill_menu(&self, menu: &objc2_app_kit::NSMenu) {
        menu.removeAllItems();
        menu.setAutoenablesItems(false);
        let keys = self.keys_around(self.ui().outline.clickedRow());
        let entries = menu_entries(&keys, &self.read().model);
        self.state().menu_keys = keys;
        let mtm = self.mtm();
        for (title, action) in entries {
            let Some(action) = action else {
                menu.addItem(&crate::util::separator(mtm));
                continue;
            };
            let entry = crate::util::item(mtm, title, Some(action), "");
            crate::util::set_target(&entry, self);
            menu.addItem(&entry);
        }
    }
}

/// The menu for `keys`: file actions that fit every one of them, or a section's "all" action.
fn menu_entries(keys: &[RowKey], model: &super::model::Model) -> Vec<(&'static str, Option<Sel>)> {
    let files: Vec<(Section, &FileRow)> = keys
        .iter()
        .filter_map(|k| match k {
            RowKey::File(section, path) => model.file(*section, path).map(|row| (*section, row)),
            _ => None,
        })
        .collect();
    if let [RowKey::Section(section)] = keys {
        return match section {
            Section::Staged => vec![(t("Alle zurücknehmen"), Some(sel!(ideGitUnstageAll:)))],
            Section::Changes => vec![(t("Alle vormerken"), Some(sel!(ideGitStageAll:)))],
            Section::Conflicts | Section::Worktrees | Section::Commits => vec![],
        };
    }
    if let [RowKey::Worktree(_) | RowKey::WorktreeFile(..)] = keys {
        return super::trees::menu(keys);
    }
    if files.is_empty() {
        return vec![];
    }
    if files.iter().all(|(s, _)| *s == Section::Conflicts) {
        return vec![
            (t("Im Editor öffnen"), Some(sel!(ideGitOpen:))),
            (t("Als gelöst markieren"), Some(sel!(ideGitResolve:))),
        ];
    }
    let mut entries = vec![];
    if files.len() == 1 {
        entries.push((t("Diff zeigen"), Some(sel!(ideGitDiff:))));
    }
    entries.push((t("Im Editor öffnen"), Some(sel!(ideGitOpen:))));
    entries.push(("", None));
    if files.iter().any(|(s, _)| *s == Section::Changes) {
        entries.push((t("Vormerken"), Some(sel!(ideGitStage:))));
    }
    if files.iter().any(|(s, _)| *s == Section::Staged) {
        entries.push((t("Zurücknehmen"), Some(sel!(ideGitUnstage:))));
    }
    if files.iter().any(|(s, _)| *s == Section::Changes) {
        entries.push(("", None));
        entries.push((t("Verwerfen …"), Some(sel!(ideGitDiscard:))));
    }
    entries
}
