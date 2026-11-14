//! Blame (E86): who changed a line last, at the end of the cursor's line after a short rest (an inset, E81) and, on
//! request, in a column before the line numbers with one entry per block of lines. The data come from
//! `git blame --porcelain --incremental` of the last commit, in the background, kept per file and commit; the lines of
//! the text map to the committed ones through the hunks the change marks found, so unsaved and uncommitted lines read
//! "Nicht committet". git runs only in folders the user trusts (E11, E22).

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_core::settings::{Settings, keys};
use ide_git::{Blame, BlameCommit};
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSButton, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSLayoutAttribute, NSMenuItem, NSPasteboard,
    NSPasteboardTypeString, NSPopover, NSPopoverBehavior, NSStackView, NSTextField, NSUserInterfaceLayoutOrientation,
    NSView, NSViewController,
};
use objc2_foundation::{NSRect, NSRectEdge, NSSize};
use splat_i18n::{t, tf};

use super::insets::{self, Inset, Layer, Place};
use super::view::EditorTextView;
use crate::util::{constant, item, label, main_soon, ns, set_action};

/// How long the cursor rests on a line before its blame shows at its end.
const REST: f64 = 0.6;
/// How many files' blame stay in memory.
const CACHED: usize = 24;
/// The widest the popover's message grows before it wraps.
const MESSAGE_WIDTH: f64 = 420.0;

/// One file's blame at one commit, with where it is in its repository.
pub struct Loaded {
    repo_root: PathBuf,
    /// The file relative to the repository's root.
    relative: PathBuf,
    blame: Blame,
}

/// What blame says about one line of the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// Added or changed since the last commit, saved or not.
    Uncommitted,
    Commit(BlameCommit),
}

/// What an editor knows about its blame.
#[derive(Default)]
pub struct State {
    /// The blame of the file at the commit the change marks compare with, once read.
    loaded: Option<(String, Rc<Loaded>)>,
    /// The commit being read, so a second request waits for the first.
    reading: Option<String>,
    /// The last mapping of lines to committed lines, with the revision it belongs to, which the column keeps using
    /// while the marks catch up with typing; built once per answer of the marks, which takes the revision away.
    mapping: Option<(Option<u64>, Mapping)>,
    /// Whether the column before the line numbers shows.
    pub(super) column: bool,
    popover: Option<Retained<NSPopover>>,
    /// Where the mouse rests over the blame at a line's end, which opens its popover after a pause.
    hover: Option<objc2_foundation::NSPoint>,
    /// The commit the popover is about, for its buttons.
    shown: Option<(Rc<Loaded>, BlameCommit)>,
    /// The column view, which `column` shows and hides.
    pub(super) view: Option<Weak<super::blame_column::BlameColumn>>,
}

/// A file and the commit its blame belongs to.
type Key = (PathBuf, String);

/// For each line of a text, counted from 0, its line in the committed file; `None` for one added or changed since.
type Mapping = Rc<[Option<usize>]>;

thread_local! {
    static AT_LINE_END: Cell<Option<bool>> = const { Cell::new(None) };
    /// Blame by file and commit, the most recent last.
    static CACHE: RefCell<Vec<(Key, Rc<Loaded>)>> = const { RefCell::new(Vec::new()) };
}

/// Whether the blame of the cursor's line shows at its end, as the user last chose.
pub fn at_line_end() -> bool {
    AT_LINE_END.with(|cell| {
        cell.get().unwrap_or_else(|| {
            let value = Settings::load_user().get(&keys::BLAME_AT_LINE_END);
            cell.set(Some(value));
            value
        })
    })
}

/// Reads the setting again on next use, after it changed elsewhere.
pub fn forget() {
    AT_LINE_END.with(|cell| cell.set(None));
}

/// Turns the blame at the end of the cursor's line on or off in every editor and remembers the choice.
pub fn toggle_line_end() {
    set_line_end(!at_line_end());
}

/// Sets whether the blame of the cursor's line shows at its end, as the settings window does.
pub fn set_line_end(on: bool) {
    AT_LINE_END.with(|cell| cell.set(Some(on)));
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::BLAME_AT_LINE_END, &on) {
        crate::diag::report("saving the blame setting", error);
    }
    for view in super::open_editors() {
        if on {
            rest(&view);
        } else {
            insets::clear(&view, Layer::Blame);
        }
    }
}

/// The file of `view` and the commit its marks compare with, where blame may run: a saved file inside a folder the
/// user trusts, in a repository with a commit.
fn source(view: &EditorTextView) -> Option<(PathBuf, String)> {
    let path = view.document().path()?.to_path_buf();
    let root = view.controller()?.project_root()?;
    if !crate::project::is_trusted(&root) || !path.starts_with(&root) {
        return None;
    }
    Some((path, super::changes::head_commit(view)?))
}

/// Whether blame can show for `view`.
pub fn available(view: &EditorTextView) -> bool {
    source(view).is_some()
}

fn cached(key: &(PathBuf, String)) -> Option<Rc<Loaded>> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache.iter().position(|(k, _)| k == key)?;
        let entry = cache.remove(index);
        let loaded = entry.1.clone();
        cache.push(entry);
        Some(loaded)
    })
}

fn remember(key: (PathBuf, String), loaded: Rc<Loaded>) {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.retain(|(k, _)| *k != key);
        cache.push((key, loaded));
        let over = cache.len().saturating_sub(CACHED);
        cache.drain(..over);
    });
}

/// The blame of `view` for the commit its marks compare with, or `None` while it is read, which starts here.
fn data(view: &EditorTextView) -> Option<Rc<Loaded>> {
    let (path, head) = source(view)?;
    {
        let state = view.ivars().blame.borrow();
        if let Some((commit, loaded)) = &state.loaded
            && *commit == head
        {
            return Some(loaded.clone());
        }
    }
    if let Some(loaded) = cached(&(path.clone(), head.clone())) {
        view.ivars().blame.borrow_mut().loaded = Some((head, loaded.clone()));
        return Some(loaded);
    }
    if view.ivars().blame.borrow().reading.as_deref() == Some(head.as_str()) {
        return None;
    }
    view.ivars().blame.borrow_mut().reading = Some(head.clone());
    read(view.ivars().serial, path, head);
    None
}

/// Reads the blame of `path` at `head` on a thread of its own and hands it to the editor `serial`.
fn read(serial: ide_ui_lsp::EditorSerial, path: PathBuf, head: String) {
    let job = move || {
        ide_process::priority::set_qos(ide_process::priority::Qos::Utility);
        let loaded = blame_of(&path, &head);
        main_soon(move || arrived(serial, path, head, loaded));
    };
    if let Err(error) = crate::util::spawn("ide-git-blame", job) {
        crate::diag::report("reading blame", error);
    }
}

/// Why the blame of a file could not be read; it goes to the system log only.
#[derive(Debug)]
enum BlameError {
    GitNotReady,
    Git(ide_git::Error),
    Io(std::io::Error),
    OutsideRepository,
}

impl std::fmt::Display for BlameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlameError::GitNotReady => f.write_str("git is not ready"),
            BlameError::Git(error) => write!(f, "{error}"),
            BlameError::Io(error) => write!(f, "{error}"),
            BlameError::OutsideRepository => f.write_str("the file is outside its repository"),
        }
    }
}

fn blame_of(path: &Path, head: &str) -> Result<Loaded, BlameError> {
    let git = ide_ui_git::git().ok_or(BlameError::GitNotReady)?;
    let repository = git.discover(path).map_err(BlameError::Git)?;
    let canonical = path.canonicalize().map_err(BlameError::Io)?;
    let relative = canonical
        .strip_prefix(repository.root())
        .map_err(|_| BlameError::OutsideRepository)?
        .to_path_buf();
    let blame = repository.blame(head, &relative).map_err(BlameError::Git)?;
    Ok(Loaded {
        repo_root: repository.root().to_path_buf(),
        relative,
        blame,
    })
}

fn arrived(serial: ide_ui_lsp::EditorSerial, path: PathBuf, head: String, loaded: Result<Loaded, BlameError>) {
    let loaded = match loaded {
        Ok(loaded) => Rc::new(loaded),
        Err(error) => {
            crate::diag::report("reading blame", error);
            if let Some(view) = super::find_editor(serial) {
                view.ivars().blame.borrow_mut().reading = None;
            }
            return;
        }
    };
    remember((path.clone(), head.clone()), loaded.clone());
    for view in super::open_editors() {
        if view.document().path() != Some(path.as_path()) {
            continue;
        }
        {
            let mut state = view.ivars().blame.borrow_mut();
            if state.reading.as_deref() == Some(head.as_str()) {
                state.reading = None;
            }
        }
        if source(&view).is_some_and(|(_, h)| h == head) {
            view.ivars().blame.borrow_mut().loaded = Some((head.clone(), loaded.clone()));
            refresh(&view);
        }
    }
}

/// The committed line of each line of `view`'s text, the latest mapping the marks gave when `fresh` is false.
fn mapping(view: &EditorTextView, fresh: bool) -> Option<Mapping> {
    let revision = view.document().revision();
    if let Some((Some(at), lines)) = &view.ivars().blame.borrow().mapping
        && *at == revision
    {
        return Some(lines.clone());
    }
    if let Some(lines) = super::changes::head_lines(view) {
        let lines: Mapping = lines.into();
        view.ivars().blame.borrow_mut().mapping = Some((Some(revision), lines.clone()));
        return Some(lines);
    }
    if fresh {
        return None;
    }
    view.ivars()
        .blame
        .borrow()
        .mapping
        .as_ref()
        .map(|(_, lines)| lines.clone())
}

/// What blame says about each line of a text: the file's blame and where each line stands in the committed file.
pub struct Lines {
    loaded: Rc<Loaded>,
    mapping: Mapping,
}

impl Lines {
    /// What blame says about `line`, counted from 0.
    pub fn at(&self, line: usize) -> Option<Line> {
        match self.mapping.get(line)? {
            None => Some(Line::Uncommitted),
            Some(committed) => self.loaded.blame.commit_of(*committed).cloned().map(Line::Commit),
        }
    }
}

/// What blame says about `line` of `view`, counted from 0; `None` while the data or the marks are not ready.
pub fn line(view: &EditorTextView, line: usize, fresh: bool) -> Option<Line> {
    let loaded = data(view)?;
    let mapping = mapping(view, fresh)?;
    Lines { loaded, mapping }.at(line)
}

/// What blame says about every line of `view`; `None` while not ready.
pub fn all_lines(view: &EditorTextView) -> Option<Lines> {
    let loaded = data(view)?;
    let mapping = mapping(view, false)?;
    Some(Lines { loaded, mapping })
}

/// How the end of a line reads it: "Autor, vor 3 Tagen · Betreff".
pub fn summary(line: &Line) -> String {
    match line {
        Line::Uncommitted => t("Nicht committet").to_string(),
        Line::Commit(commit) => tf(
            "{}, {} · {}",
            &[&commit.author, &age(commit.author_time), &commit.summary],
        ),
    }
}

/// How long ago `time`, in seconds since 1970, was.
pub fn age(time: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    ide_ui_git::ago(now.saturating_sub(time))
}

/// The line the cursor of `view` is on.
fn cursor_line(view: &EditorTextView) -> usize {
    let document = view.document();
    let text = document.text();
    let at = view.selectedRange().location.min(text.len_utf16());
    text.utf16_to_byte(at).map_or(0, |byte| text.line_of(byte))
}

/// The cursor moved: the blame at a line's end goes and comes back for the new line once the cursor rests.
pub fn cursor_moved(view: &EditorTextView) {
    if !at_line_end() {
        return;
    }
    let line = cursor_line(view);
    let start = view.line_start(line);
    let stays = insets::of(view, Layer::Blame)
        .first()
        .is_some_and(|inset| Some(inset.at) == start);
    if !stays {
        insets::clear(view, Layer::Blame);
    }
    view.later(sel!(ideBlameRest), REST);
}

/// The cursor rested: its line's blame shows at the line's end, once the data and the marks are ready.
pub fn rest(view: &EditorTextView) {
    if !at_line_end() || !available(view) {
        insets::clear(view, Layer::Blame);
        return;
    }
    let number = cursor_line(view);
    let Some(found) = line(view, number, true) else {
        if data(view).is_some() {
            view.later(sel!(ideBlameRest), REST / 2.0);
        }
        return;
    };
    let Some(start) = view.line_start(number) else {
        return;
    };
    let mut inset = Inset::new(start, summary(&found), Place::LineEnd);
    inset.action = Some(Rc::new(move |view: &EditorTextView| {
        if let Some(rect) = insets::shown(view)
            .into_iter()
            .find(|s| s.layer == Layer::Blame)
            .map(|s| s.rect)
        {
            show_popover(view, number, view, rect);
        }
    }));
    insets::set(view, Layer::Blame, vec![inset]);
}

/// The mouse moved over `view`: resting on the blame at a line's end opens its popover, as pointing at it does.
pub fn mouse_moved(view: &EditorTextView, event: &objc2_app_kit::NSEvent) {
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    let over = insets::shown(view).into_iter().any(|s| {
        s.layer == Layer::Blame
            && point.x >= s.rect.origin.x
            && point.x < s.rect.origin.x + s.rect.size.width
            && point.y >= s.rect.origin.y
            && point.y < s.rect.origin.y + s.rect.size.height
    });
    let Ok(mut state) = view.ivars().blame.try_borrow_mut() else {
        return;
    };
    let shown = state.popover.as_ref().is_some_and(|p| p.isShown());
    state.hover = over.then_some(point);
    drop(state);
    if over && !shown {
        view.later(sel!(ideBlameHover), REST);
    }
}

/// The mouse rested on the blame at a line's end: its popover opens.
pub fn hover_rest(view: &EditorTextView) {
    let Some(point) = view.ivars().blame.borrow().hover else {
        return;
    };
    let target = insets::shown(view).into_iter().find(|s| {
        s.layer == Layer::Blame
            && point.x >= s.rect.origin.x
            && point.x < s.rect.origin.x + s.rect.size.width
            && point.y >= s.rect.origin.y
            && point.y < s.rect.origin.y + s.rect.size.height
    });
    let Some(target) = target else { return };
    let line = insets::of(view, Layer::Blame)
        .get(target.index)
        .map(|inset| inset.at)
        .and_then(|at| {
            let document = view.document();
            let text = document.text();
            text.utf16_to_byte(at).ok().map(|byte| text.line_of(byte))
        });
    if let Some(line) = line {
        show_popover(view, line, view, target.rect);
    }
}

/// The change marks compared again, as after a commit or an edit: what blame shows follows.
pub fn marks_changed(view: &EditorTextView) {
    if let Some((revision, _)) = &mut view.ivars().blame.borrow_mut().mapping {
        *revision = None;
    }
    let (column, end) = (view.ivars().blame.borrow().column, at_line_end());
    if column || end {
        refresh(view);
    }
}

/// Shows again what blame shows in `view`.
fn refresh(view: &EditorTextView) {
    if at_line_end() {
        rest(view);
    }
    super::blame_column::redraw(view);
}

/// Shows or hides the blame column of `view`.
pub fn toggle_column(view: &EditorTextView) {
    let on = !view.ivars().blame.borrow().column && available(view);
    view.ivars().blame.borrow_mut().column = on;
    super::blame_column::show(view, on);
}

/// The menu items of blame: ticked when on, greyed out where blame cannot run; `None` for every other item.
pub fn validate(view: &EditorTextView, item: &NSMenuItem) -> Option<bool> {
    let action = item.action()?;
    let tick = |on: bool| {
        item.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        })
    };
    if action == sel!(ideToggleBlameAtLineEnd:) {
        tick(at_line_end());
        return Some(true);
    }
    if action == sel!(ideToggleBlameColumn:) {
        tick(view.ivars().blame.borrow().column);
        return Some(available(view));
    }
    if action == sel!(ideShowLineBlame:) {
        return Some(available(view));
    }
    None
}

/// `view_menu`, the menu bar's Darstellung, with the switches of inlay hints (E83) and blame after "Leerzeichen
/// anzeigen".
pub fn with_view_items(mtm: MainThreadMarker, view_menu: Retained<NSMenuItem>) -> Retained<NSMenuItem> {
    let Some(menu) = view_menu.submenu() else {
        return view_menu;
    };
    let whitespace = menu
        .itemArray()
        .iter()
        .position(|entry| entry.action() == Some(sel!(ideToggleWhitespace:)));
    let at = whitespace.map_or(menu.numberOfItems(), |i| isize::try_from(i + 1).unwrap_or(0));
    let entries = [
        item(mtm, t("Inlay-Hinweise"), Some(sel!(ideToggleInlayHints:)), ""),
        item(
            mtm,
            &tf("Inlay-Hinweise für {}", &["…"]),
            Some(sel!(ideToggleInlayHintsLanguage:)),
            "",
        ),
        item(
            mtm,
            t("Diagnosen am Zeilenende"),
            Some(sel!(ideToggleDiagnosticsAtLineEnd:)),
            "",
        ),
        item(mtm, t("Blame am Zeilenende"), Some(sel!(ideToggleBlameAtLineEnd:)), ""),
        item(mtm, t("Blame zeigen"), Some(sel!(ideToggleBlameColumn:)), ""),
        item(mtm, t("Blame der Zeile zeigen"), Some(sel!(ideShowLineBlame:)), ""),
    ];
    for (offset, entry) in (0..).zip(&entries) {
        menu.insertItem_atIndex(entry, at + offset);
    }
    view_menu
}

/// The context menu's blame entries, where blame can run.
pub fn context_items(view: &EditorTextView) -> Vec<Retained<NSMenuItem>> {
    if !available(view) {
        return Vec::new();
    }
    let mtm = view.mtm();
    vec![
        item(mtm, t("Blame zeigen"), Some(sel!(ideToggleBlameColumn:)), ""),
        item(mtm, t("Blame der Zeile zeigen"), Some(sel!(ideShowLineBlame:)), ""),
    ]
}

/// Opens the popover of the cursor's line, as the menu command does, below the caret.
pub fn show_for_cursor(view: &EditorTextView) {
    let number = cursor_line(view);
    let at = view.selectedRange().location;
    let Some(caret) = ide_ui_lsp::rect_of(view, at..at) else {
        return;
    };
    if line(view, number, false).is_none() {
        objc2_app_kit::NSBeep();
        return;
    }
    show_popover(
        view,
        number,
        view,
        NSRect::new(caret.origin, NSSize::new(1.0, caret.size.height)),
    );
}

/// Shows the whole commit of `line` of `view` in a popover at `rect` of `anchor`.
pub fn show_popover(view: &EditorTextView, line_number: usize, anchor: &NSView, rect: NSRect) {
    dismiss(view);
    let Some(found) = line(view, line_number, false) else {
        return;
    };
    let mtm = view.mtm();
    let content = match &found {
        Line::Uncommitted => uncommitted_content(mtm),
        Line::Commit(commit) => commit_content(mtm, view, commit),
    };
    let controller = NSViewController::new(mtm);
    let size = content.fittingSize();
    controller.setView(&content);
    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setAnimates(false);
    popover.setContentViewController(Some(&controller));
    popover.setContentSize(size);
    popover.showRelativeToRect_ofView_preferredEdge(rect, anchor, NSRectEdge::MaxY);
    let hash = match &found {
        Line::Commit(commit) => Some(commit.hash.clone()),
        Line::Uncommitted => None,
    };
    {
        let mut state = view.ivars().blame.borrow_mut();
        state.popover = Some(popover);
        state.shown = match (found, state.loaded.as_ref()) {
            (Line::Commit(commit), Some((_, loaded))) => Some((loaded.clone(), commit)),
            _ => None,
        };
    }
    if let Some(hash) = hash {
        read_message(view, hash);
    }
}

/// Closes the popover of `view`, if one is open.
pub fn dismiss(view: &EditorTextView) {
    let popover = view.ivars().blame.borrow_mut().popover.take();
    if let Some(popover) = popover {
        popover.close();
    }
}

fn stack(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(ide_ui_common::style::space::M);
    stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
        top: 12.0,
        left: 14.0,
        bottom: 12.0,
        right: 14.0,
    });
    stack
}

fn uncommitted_content(mtm: MainThreadMarker) -> Retained<NSView> {
    let stack = stack(mtm);
    let title = label(mtm, t("Nicht committet"));
    title.setFont(Some(&NSFont::boldSystemFontOfSize(ide_ui_common::style::LABEL)));
    stack.addArrangedSubview(&title);
    stack.addArrangedSubview(&label(mtm, t("Seit dem letzten Commit geändert")));
    stack.into_super()
}

/// Tags of the popover's parts, to find the message when it arrives.
const MESSAGE_TAG: isize = 86;

fn commit_content(mtm: MainThreadMarker, view: &EditorTextView, commit: &BlameCommit) -> Retained<NSView> {
    let stack = stack(mtm);
    let short: String = commit.hash.chars().take(8).collect();
    let title = label(
        mtm,
        &tf(
            "{} · {}",
            &[&tf("{} · {}", &[&commit.author, &age(commit.author_time)]), &short],
        ),
    );
    title.setFont(Some(&NSFont::boldSystemFontOfSize(ide_ui_common::style::LABEL)));
    title.setToolTip(Some(&ns(&commit.author_email)));
    stack.addArrangedSubview(&title);
    let message = NSTextField::wrappingLabelWithString(&ns(&commit.summary), mtm);
    message.setSelectable(true);
    message.setPreferredMaxLayoutWidth(MESSAGE_WIDTH);
    message.setTag(MESSAGE_TAG);
    stack.addArrangedSubview(&message);
    let buttons = NSStackView::new(mtm);
    buttons.setSpacing(ide_ui_common::style::space::M);
    for (title, action) in [
        (t("Commit-ID kopieren"), sel!(ideBlameCopyHash:)),
        (t("Im Graph zeigen"), sel!(ideBlameShowInGraph:)),
        (t("Diff zeigen"), sel!(ideBlameShowDiff:)),
    ] {
        // SAFETY: A nil target until `set_action` below, which checks that the view answers the action.
        let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
        set_action(&button, view, action);
        buttons.addArrangedSubview(&button);
    }
    stack.addArrangedSubview(&buttons);
    stack.into_super()
}

/// Reads the whole message of `hash` and puts it into the open popover of `view`.
fn read_message(view: &EditorTextView, hash: String) {
    let Some(loaded) = view.ivars().blame.borrow().shown.as_ref().map(|(l, _)| l.clone()) else {
        return;
    };
    let root = loaded.repo_root.clone();
    let serial = view.ivars().serial;
    let job = move || {
        let message = ide_ui_git::git()
            .and_then(|git| git.discover(&root).ok())
            .and_then(|repository| repository.commit_message(&hash).ok());
        main_soon(move || {
            let (Some(message), Some(view)) = (message, super::find_editor(serial)) else {
                return;
            };
            let popover = view.ivars().blame.borrow().popover.clone();
            let field = popover
                .as_ref()
                .and_then(|p| p.contentViewController())
                .and_then(|c| c.view().viewWithTag(MESSAGE_TAG))
                .and_then(|v| v.downcast::<NSTextField>().ok());
            if let (Some(popover), Some(field)) = (popover, field) {
                field.setStringValue(&ns(&message));
                if let Some(content) = popover.contentViewController() {
                    popover.setContentSize(content.view().fittingSize());
                }
            }
        });
    };
    if let Err(error) = crate::util::spawn("ide-git-message", job) {
        crate::diag::report("reading a commit message", error);
    }
}

/// The popover's "Commit-ID kopieren".
pub fn copy_hash(view: &EditorTextView) {
    let Some((_, commit)) = view.ivars().blame.borrow().shown.clone() else {
        return;
    };
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(&ns(&commit.hash), constant!(NSPasteboardTypeString));
    dismiss(view);
}

/// The popover's "Im Graph zeigen": the commit in the Git panel's history (E74).
pub fn show_in_graph(view: &EditorTextView) {
    let shown = view.ivars().blame.borrow().shown.clone();
    if let (Some((_, commit)), Some(controller)) = (shown, view.controller()) {
        dismiss(view);
        ide_ui_git::reveal_commit(&controller, &commit.hash);
    }
}

/// The popover's "Diff zeigen": what the commit changed in this file, in a diff tab.
pub fn show_diff(view: &EditorTextView) {
    let shown = view.ivars().blame.borrow().shown.clone();
    if let (Some((loaded, commit)), Some(controller)) = (shown, view.controller()) {
        dismiss(view);
        ide_ui_git::show_commit_diff(&controller, &loaded.repo_root, &commit.hash, &loaded.relative);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_end_names_author_age_and_subject_or_that_it_is_uncommitted() {
        assert_eq!(summary(&Line::Uncommitted), t("Nicht committet"));
        let commit = BlameCommit {
            hash: "abc".into(),
            author: "Ada".into(),
            author_email: "ada@example.com".into(),
            author_time: 0,
            summary: "Fix the parser".into(),
        };
        let text = summary(&Line::Commit(commit));
        assert!(
            text.starts_with("Ada, ") && text.ends_with(" · Fix the parser"),
            "{text}"
        );
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
