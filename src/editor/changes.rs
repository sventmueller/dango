//! Change marks in the line number column: where the text differs from its file in the last commit (E11), read with
//! splat-git, which starts no process and so works in folders the user does not trust (E22). A click on a mark shows
//! the committed lines in a popover that can put them back.

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use ide_document::Snapshot;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSColor, NSFont, NSLineBreakMode, NSPopover, NSPopoverBehavior, NSScrollView, NSStackView, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSRectEdge, NSSize};
use splat_git::Oid;
use splat_git::diff::{ChangeKind, Hunk};
use splat_i18n::t;

use super::view::EditorTextView;
use crate::util::{label, main_soon, ns, set_action};

/// How long typing pauses before the marks follow it.
const DELAY: f64 = 0.3;
/// How long the repository settles after a change in `.git` before its commit is read again.
const HEAD_DELAY: f64 = 0.4;
/// The widest and tallest the committed lines grow in the popover before they scroll.
const MAX_WIDTH: f64 = 560.0;
const MAX_HEIGHT: f64 = 260.0;
const PADDING: NSEdgeInsets = NSEdgeInsets {
    top: 10.0,
    left: 12.0,
    bottom: 10.0,
    right: 12.0,
};

/// The file in the last commit, as the marks compare against it.
struct Head {
    commit: Oid,
    text: Arc<str>,
}

/// What an editor knows about its file's last commit and where its text differs.
#[derive(Default)]
pub struct Marks {
    head: Option<Head>,
    hunks: Vec<Hunk>,
    /// The lines of the text the hunks were found in, to place a removal behind the last line.
    lines: usize,
    /// The document's revision the hunks were found in.
    revision: Option<u64>,
    /// Counts questions, so that the answer to an older one is dropped.
    asked: u64,
    popover: Option<Retained<NSPopover>>,
    /// The hunk the popover shows, with the document revision it was found in.
    shown: Option<(Hunk, u64)>,
}

/// How a line is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Added,
    Modified,
    /// Lines were removed in front of this one.
    RemovedAbove,
    /// Lines were removed behind this one, the last.
    RemovedBelow,
}

impl Mark {
    pub fn color(self) -> Retained<NSColor> {
        match self {
            Mark::Added => NSColor::systemGreenColor(),
            Mark::Modified => NSColor::systemBlueColor(),
            Mark::RemovedAbove | Mark::RemovedBelow => NSColor::systemRedColor(),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Mark::Added => t("Seit dem letzten Commit hinzugefügt"),
            Mark::Modified => t("Seit dem letzten Commit geändert"),
            Mark::RemovedAbove | Mark::RemovedBelow => t("Seit dem letzten Commit gelöscht"),
        }
    }
}

/// The text in the commit, as the document would read it, or `None` for a file that is not UTF-8 or UTF-16 text.
fn decode(bytes: Vec<u8>) -> Option<String> {
    let (_, decoded) = ide_document::Encoding::read(bytes)?;
    Some(ide_document::FileFormat::of(&decoded).1.to_string())
}

/// The file's repository, commit and committed text; `None` outside a repository, before the first commit, for a file
/// the commit does not hold, and for a repository this reader cannot read.
fn read_head(path: &Path, known: Option<Oid>) -> Option<Result<Head, Oid>> {
    let repository = splat_git::find(path)?;
    let commit = repository.head_commit().ok()??;
    if known == Some(commit) {
        return Some(Err(commit));
    }
    let bytes = repository.head_version(path).ok()??;
    let text: Arc<str> = decode(bytes)?.into();
    Some(Ok(Head { commit, text }))
}

fn hunks_of(head: &str, snapshot: &Snapshot) -> (Vec<Hunk>, usize, u64) {
    let text = snapshot.text().to_string();
    let lines = text.split_inclusive('\n').count();
    (splat_git::diff::hunks(head, &text), lines, snapshot.revision())
}

/// Reads the file's last commit again and compares; `force` reads it even when the commit is the one already known.
pub fn reload(view: &EditorTextView, force: bool) {
    let Some(path) = view.document().path().map(Path::to_path_buf) else {
        clear(view);
        return;
    };
    let snapshot = view.document().snapshot();
    let known = if force {
        None
    } else {
        view.ivars().marks.borrow().head.as_ref().map(|h| h.commit)
    };
    let asked = next_question(view);
    let serial = view.ivars().lsp.borrow().serial;
    spawn(move || {
        let answer = match read_head(&path, known) {
            None => Answer::Gone,
            Some(Err(_same)) => Answer::Same,
            Some(Ok(head)) => {
                let (hunks, lines, revision) = hunks_of(&head.text, &snapshot);
                Answer::Head(head, hunks, lines, revision)
            }
        };
        main_soon(move || deliver(serial, asked, answer));
    });
}

/// Compares the text with the commit once typing pauses.
pub fn schedule(view: &EditorTextView) {
    if view.ivars().marks.borrow().head.is_some() {
        view.later(sel!(ideGitMarks), DELAY);
    }
}

/// Something in a `.git` folder changed: the commit may have moved.
pub fn schedule_head(view: &EditorTextView) {
    view.later(sel!(ideGitHead), HEAD_DELAY);
}

/// Compares the text as it is now with the commit known.
pub fn compare(view: &EditorTextView) {
    let Some(text) = view.ivars().marks.borrow().head.as_ref().map(|h| h.text.clone()) else {
        return;
    };
    let snapshot = view.document().snapshot();
    let asked = next_question(view);
    let serial = view.ivars().lsp.borrow().serial;
    spawn(move || {
        let (hunks, lines, revision) = hunks_of(&text, &snapshot);
        main_soon(move || deliver(serial, asked, Answer::Hunks(hunks, lines, revision)));
    });
}

fn next_question(view: &EditorTextView) -> u64 {
    let mut marks = view.ivars().marks.borrow_mut();
    marks.asked += 1;
    marks.asked
}

fn spawn(job: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new().name("ide-git-marks".into()).spawn(job) {
        crate::diag::report("comparing with the last commit", error);
    }
}

enum Answer {
    Gone,
    /// The commit is the one known, so only the text is compared again.
    Same,
    Head(Head, Vec<Hunk>, usize, u64),
    Hunks(Vec<Hunk>, usize, u64),
}

fn deliver(serial: u64, asked: u64, answer: Answer) {
    let Some(view) = super::find_editor(serial) else { return };
    {
        let mut marks = view.ivars().marks.borrow_mut();
        if marks.asked != asked {
            return;
        }
        match answer {
            Answer::Same => {
                drop(marks);
                compare(&view);
                return;
            }
            Answer::Gone => {
                marks.head = None;
                marks.hunks.clear();
            }
            Answer::Head(head, hunks, lines, revision) => {
                marks.head = Some(head);
                marks.hunks = hunks;
                marks.lines = lines;
                marks.revision = Some(revision);
            }
            Answer::Hunks(hunks, lines, revision) => {
                marks.hunks = hunks;
                marks.lines = lines;
                marks.revision = Some(revision);
            }
        }
    }
    super::redraw_gutter(&view);
    super::blame::marks_changed(&view);
}

fn clear(view: &EditorTextView) {
    {
        let mut marks = view.ivars().marks.borrow_mut();
        marks.asked += 1;
        marks.head = None;
        marks.hunks.clear();
    }
    super::redraw_gutter(view);
}

/// The hunk that `line`, counted from 0, belongs to, with how it is marked there.
fn hunk_at(hunks: &[Hunk], lines: usize, line: usize) -> Option<(&Hunk, Mark)> {
    let after = hunks.partition_point(|h| h.change.lines.start <= line);
    let covering = hunks[..after].iter().rev().find(|h| h.change.lines.contains(&line));
    if let Some(hunk) = covering {
        let mark = match hunk.change.kind {
            ChangeKind::Added => Mark::Added,
            _ => Mark::Modified,
        };
        return Some((hunk, mark));
    }
    let removed = |at: usize| {
        hunks
            .iter()
            .find(|h| h.change.kind == ChangeKind::Removed && h.change.lines.start == at)
    };
    if let Some(hunk) = removed(line) {
        return Some((hunk, Mark::RemovedAbove));
    }
    removed(line + 1)
        .filter(|_| line + 1 >= lines)
        .map(|hunk| (hunk, Mark::RemovedBelow))
}

/// The commit the marks compare against, as its full hash.
pub fn head_commit(view: &EditorTextView) -> Option<String> {
    view.ivars().marks.borrow().head.as_ref().map(|h| h.commit.to_string())
}

/// Where each line of the text, counted from 0, stands in the file of the last commit: `None` for a line added or
/// changed since; `None` as a whole while the marks have not caught up with the text.
pub fn head_lines(view: &EditorTextView) -> Option<Vec<Option<usize>>> {
    let marks = view.ivars().marks.borrow();
    marks.head.as_ref()?;
    if marks.revision != Some(view.document().revision()) {
        return None;
    }
    Some(map_to_head(&marks.hunks, view.document().text().line_count()))
}

/// For `lines` lines of a text whose differences from the committed text are `hunks`, the committed line of each.
fn map_to_head(hunks: &[Hunk], lines: usize) -> Vec<Option<usize>> {
    let mut out = Vec::with_capacity(lines);
    let mut shift: isize = 0;
    let mut hunks = hunks.iter().peekable();
    for line in 0..lines {
        while let Some(hunk) = hunks.peek() {
            let new = &hunk.change.lines;
            let ended = if new.is_empty() {
                new.start <= line
            } else {
                new.end <= line
            };
            if !ended {
                break;
            }
            shift += isize::try_from(hunk.old_lines.len()).unwrap_or(0) - isize::try_from(new.len()).unwrap_or(0);
            hunks.next();
        }
        let changed = hunks.peek().is_some_and(|hunk| hunk.change.lines.contains(&line));
        out.push(if changed {
            None
        } else {
            usize::try_from(isize::try_from(line).unwrap_or(0) + shift).ok()
        });
    }
    out
}

/// How `line`, counted from 0, is marked.
pub fn mark_at(view: &EditorTextView, line: usize) -> Option<Mark> {
    let marks = view.ivars().marks.borrow();
    hunk_at(&marks.hunks, marks.lines, line).map(|(_, mark)| mark)
}

/// Draws `mark` at the right edge of `row` in the line number column, which is `width` wide.
pub fn draw(mark: Mark, row: NSRect, width: f64) {
    mark.color().setFill();
    let path = objc2_app_kit::NSBezierPath::bezierPath();
    match mark {
        Mark::Added | Mark::Modified => {
            let bar = NSRect::new(
                NSPoint::new(width - 4.0, row.origin.y),
                NSSize::new(3.0, row.size.height),
            );
            objc2_app_kit::NSRectFill(bar);
            return;
        }
        Mark::RemovedAbove | Mark::RemovedBelow => {
            let y = if mark == Mark::RemovedAbove {
                row.origin.y
            } else {
                row.origin.y + row.size.height
            };
            path.moveToPoint(NSPoint::new(width - 7.0, y - 4.0));
            path.lineToPoint(NSPoint::new(width - 1.0, y));
            path.lineToPoint(NSPoint::new(width - 7.0, y + 4.0));
            path.closePath();
        }
    }
    path.fill();
}

/// The column was clicked on `line`, counted from 0, at `at` in `column`: shows the committed lines of its hunk.
pub fn clicked(view: &EditorTextView, line: usize, column: &NSView, at: NSRect) -> bool {
    let Some(head) = view.ivars().marks.borrow().head.as_ref().map(|h| h.text.clone()) else {
        return false;
    };
    let (hunks, lines, revision) = {
        let document = view.document();
        let text = document.text().to_string();
        let lines = text.split_inclusive('\n').count();
        (splat_git::diff::hunks(&head, &text), lines, document.revision())
    };
    let Some((hunk, mark)) = hunk_at(&hunks, lines, line).map(|(h, m)| (h.clone(), m)) else {
        return false;
    };
    let committed = committed_lines(&head, &hunk.old_lines);
    show(view, mark, &committed, column, at);
    let mut marks = view.ivars().marks.borrow_mut();
    marks.shown = Some((hunk, revision));
    marks.hunks = hunks;
    marks.lines = lines;
    true
}

fn committed_lines(head: &str, lines: &Range<usize>) -> String {
    head.split_inclusive('\n')
        .skip(lines.start)
        .take(lines.end - lines.start)
        .collect()
}

fn show(view: &EditorTextView, mark: Mark, committed: &str, column: &NSView, at: NSRect) {
    dismiss(view);
    let mtm = view.mtm();
    let content = content(mtm, view, mark, committed);
    let size = content.fittingSize();
    let controller = NSViewController::new(mtm);
    controller.setView(&content);
    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setAnimates(false);
    popover.setContentViewController(Some(&controller));
    popover.setContentSize(size);
    popover.showRelativeToRect_ofView_preferredEdge(at, column, NSRectEdge::MaxX);
    view.ivars().marks.borrow_mut().popover = Some(popover);
}

/// What the popover holds: what happened, the committed lines and the button that puts them back.
fn content(mtm: MainThreadMarker, view: &EditorTextView, mark: Mark, committed: &str) -> Retained<NSView> {
    let title = label(mtm, mark.description());
    title.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    stack.setSpacing(8.0);
    stack.addArrangedSubview(&title);
    if mark != Mark::Added {
        stack.addArrangedSubview(&committed_view(mtm, view, committed));
    }
    // SAFETY: A nil target until `set_action` below, which checks that the view answers the action.
    let button =
        unsafe { objc2_app_kit::NSButton::buttonWithTitle_target_action(&ns(t("Zurücksetzen")), None, None, mtm) };
    button.setToolTip(Some(&ns(t(
        "Diesen Abschnitt auf den Stand des letzten Commits zurücksetzen",
    ))));
    set_action(&button, view, sel!(ideRevertHunk:));
    stack.addArrangedSubview(&button);
    let container = NSView::new(mtm);
    container.addSubview(&stack);
    stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        stack
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&container.leadingAnchor(), PADDING.left),
        stack
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&container.trailingAnchor(), -PADDING.right),
        stack
            .topAnchor()
            .constraintEqualToAnchor_constant(&container.topAnchor(), PADDING.top),
        stack
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&container.bottomAnchor(), -PADDING.bottom),
    ] {
        constraint.setActive(true);
    }
    container
}

/// The committed lines in the code font, scrolling when there are many.
fn committed_view(mtm: MainThreadMarker, view: &EditorTextView, committed: &str) -> Retained<NSView> {
    let shown = committed.strip_suffix('\n').unwrap_or(committed);
    let shown = shown.strip_suffix('\r').unwrap_or(shown);
    let field = NSTextField::wrappingLabelWithString(&ns(shown), mtm);
    field.setFont(Some(&super::code_font(view.ivars().font_size.get() * 0.92)));
    field.setTextColor(Some(&NSColor::secondaryLabelColor()));
    field.setLineBreakMode(NSLineBreakMode::ByClipping);
    field.setSelectable(true);
    crate::util::set_label(&field, Some(t("Stand im letzten Commit")));
    let unbounded = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0e5, 1.0e5));
    let fitted = field.cell().map_or(NSSize::new(MAX_WIDTH, 40.0), |cell| {
        let size = cell.cellSizeForBounds(unbounded);
        NSSize::new(size.width.ceil(), size.height.ceil())
    });
    let size = NSSize::new(fitted.width.min(MAX_WIDTH).ceil(), fitted.height.min(MAX_HEIGHT).ceil());
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), fitted));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), size));
    scroll.setHasVerticalScroller(fitted.height > MAX_HEIGHT);
    scroll.setHasHorizontalScroller(fitted.width > MAX_WIDTH);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(&field));
    scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll
        .widthAnchor()
        .constraintEqualToConstant(size.width)
        .setActive(true);
    scroll
        .heightAnchor()
        .constraintEqualToConstant(size.height)
        .setActive(true);
    Retained::into_super(scroll)
}

/// Closes the popover of `view`, if one is shown.
pub fn dismiss(view: &EditorTextView) {
    let popover = view.ivars().marks.borrow_mut().popover.take();
    if let Some(popover) = popover {
        popover.close();
    }
}

/// Puts the committed lines of the hunk the popover shows back, as one undoable step.
pub fn revert(view: &EditorTextView) {
    let shown = view.ivars().marks.borrow_mut().shown.take();
    dismiss(view);
    let Some((hunk, revision)) = shown else { return };
    let Some(head) = view.ivars().marks.borrow().head.as_ref().map(|h| h.text.clone()) else {
        return;
    };
    let edit = {
        let document = view.document();
        if document.revision() != revision {
            return;
        }
        let text = document.text();
        let (Ok(start), Ok(end)) = (
            text.byte_to_utf16(hunk.change.bytes.start),
            text.byte_to_utf16(hunk.change.bytes.end),
        ) else {
            return;
        };
        let ending = document.line_endings().dominant.as_str();
        (
            start..end,
            with_breaks(&committed_lines(&head, &hunk.old_lines), ending),
        )
    };
    crate::lsp::apply_text_edits(view, vec![edit]);
    compare(view);
}

/// `text` with every line break made `ending`, so that a hunk from a commit with other breaks fits the file.
fn with_breaks(text: &str, ending: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| match line.strip_suffix('\n') {
            Some(content) => format!("{}{ending}", content.strip_suffix('\r').unwrap_or(content)),
            None => line.to_string(),
        })
        .collect()
}

/// The editors whose file lies below `root` look at their commit again.
pub fn git_changed(root: &Path) {
    for view in super::open_editors() {
        if view.document().path().is_some_and(|p| p.starts_with(root)) {
            schedule_head(&view);
        }
    }
}

/// The hunks of `view` as kinds and line ranges, and the text of the popover's hunk, for the self-test.
/// What the self-test reads of an editor's change marks.
#[cfg(feature = "selftest")]
pub struct Seen {
    /// The file's last commit is known.
    pub head: bool,
    /// Each hunk's mark, with removals as `RemovedAbove`, and its lines.
    pub hunks: Vec<(Mark, Range<usize>)>,
    /// The committed lines the popover shows, while it is shown.
    pub popover: Option<String>,
}

#[cfg(feature = "selftest")]
pub fn seen(view: &EditorTextView) -> Seen {
    let marks = view.ivars().marks.borrow();
    let hunks = marks
        .hunks
        .iter()
        .map(|h| {
            let mark = match h.change.kind {
                ChangeKind::Added => Mark::Added,
                ChangeKind::Modified => Mark::Modified,
                ChangeKind::Removed => Mark::RemovedAbove,
            };
            (mark, h.change.lines.clone())
        })
        .collect();
    let popover = marks
        .popover
        .as_ref()
        .filter(|p| p.isShown())
        .and(marks.shown.as_ref())
        .zip(marks.head.as_ref())
        .map(|((hunk, _), head)| committed_lines(&head.text, &hunk.old_lines));
    Seen {
        head: marks.head.is_some(),
        hunks,
        popover,
    }
}

/// The popover's window, for a picture, and its button, for a click.
#[cfg(feature = "selftest")]
pub fn popover_parts(
    view: &EditorTextView,
) -> Option<(Retained<objc2_app_kit::NSWindow>, Retained<objc2_app_kit::NSButton>)> {
    let marks = view.ivars().marks.borrow();
    let content = marks.popover.as_ref()?.contentViewController()?.view();
    let stack = content.subviews().firstObject()?.downcast::<NSStackView>().ok()?;
    let button = stack
        .arrangedSubviews()
        .iter()
        .find_map(|v| v.downcast::<objc2_app_kit::NSButton>().ok())?;
    Some((stack.window()?, button))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunks(old: &str, new: &str) -> (Vec<Hunk>, usize) {
        (splat_git::diff::hunks(old, new), new.split_inclusive('\n').count())
    }

    #[test]
    fn lines_are_marked_by_the_hunk_they_belong_to() {
        let (found, lines) = hunks("a\nb\nc\nd\n", "a\nB\nc\nnew\nd\n");
        assert_eq!(hunk_at(&found, lines, 1).map(|(_, m)| m), Some(Mark::Modified));
        assert_eq!(hunk_at(&found, lines, 3).map(|(_, m)| m), Some(Mark::Added));
        assert_eq!(hunk_at(&found, lines, 0), None);
        let (found, lines) = hunks("a\nb\nc\n", "a\nc\n");
        assert_eq!(hunk_at(&found, lines, 1).map(|(_, m)| m), Some(Mark::RemovedAbove));
        let (found, lines) = hunks("a\nb\n", "a\n");
        assert_eq!(
            hunk_at(&found, lines, 0).map(|(_, m)| m),
            Some(Mark::RemovedBelow),
            "a removal behind the last line marks that line"
        );
    }

    #[test]
    fn committed_lines_take_the_breaks_of_the_file() {
        assert_eq!(committed_lines("a\nb\nc", &(1..3)), "b\nc");
        assert_eq!(with_breaks("b\r\nc\nd", "\r\n"), "b\r\nc\r\nd");
        assert_eq!(with_breaks("b\r\n", "\n"), "b\n");
    }

    #[test]
    fn each_line_finds_its_committed_line_and_changed_ones_none() {
        let (found, _) = hunks("a\nb\nc\nd\ne\n", "a\nB\nc\nnew\nmore\nd\ne\n");
        assert_eq!(
            map_to_head(&found, 7),
            vec![Some(0), None, Some(2), None, None, Some(3), Some(4)]
        );
        let (found, _) = hunks("a\nb\nc\nd\n", "a\nd\n");
        assert_eq!(
            map_to_head(&found, 2),
            vec![Some(0), Some(3)],
            "lines after a removal move up"
        );
        let (found, _) = hunks("a\n", "a\n");
        assert_eq!(map_to_head(&found, 1), vec![Some(0)]);
        let (found, _) = hunks("a\nb\n", "x\na\nb\n");
        assert_eq!(map_to_head(&found, 3), vec![None, Some(0), Some(1)]);
    }

    #[test]
    fn committed_text_loses_its_byte_order_mark() {
        assert_eq!(decode(b"\xEF\xBB\xBFtext".to_vec()).as_deref(), Some("text"));
        assert_eq!(decode(vec![0xFF, 0xFE, b'a', 0]).as_deref(), Some("a"));
        assert_eq!(decode(vec![0xC3, 0x28]), None);
    }
}
