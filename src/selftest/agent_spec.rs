//! The specification step of the `agent` scenario (E178, F63): a specification without a session, as a work order's
//! at G1, read from its Markdown file into a window of its own. Comments on a passage and on the whole specification
//! are added, edited, shown and removed; approving with a comment not sent asks first, and sending hands the comment
//! under its passage to whoever reads the answer. A second specification is discarded, which leaves it open, and then
//! approved without its comment.

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn, NSBackingStoreType, NSModalResponse,
    NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::{check, give_up, helpers};
use ide_ui_agent::plan::{self, Spec, SpecAnswer, SpecButton, SpecNoteAction};

const NAME: &str = "agent";
const SPEC: &str = "# Greeting\n\nWrite a greeting into the file of the order.\n\n## Criteria\n\n- the file says hello\n\
                    - nothing else changes\n";
const PASSAGE: &str = "nothing else changes";
const EDITED: &str = "Name every file that may change";

type Answers = Rc<RefCell<Vec<SpecAnswer>>>;
type Then = Box<dyn FnOnce()>;

/// A specification on view and what it answered.
struct Open {
    window: Retained<NSWindow>,
    view: Retained<NSView>,
    answers: Answers,
}

impl Open {
    fn seen(&self) -> plan::SpecSeen {
        plan::spec_seen(&self.view).unwrap_or_else(|| plan::SpecSeen {
            notes: Vec::new(),
            boxes: 0,
            marked: 0,
            enabled: [false; 4],
            state: "no specification".into(),
            asking: None,
            selected: String::new(),
        })
    }

    fn answers(&self) -> Vec<SpecAnswer> {
        self.answers.borrow().clone()
    }

    /// Asks for a comment on `passage`, or on the whole specification for an empty one, and adds `comment`.
    fn comment(&self, passage: &str, comment: &str) -> bool {
        plan::spec_select(&self.view, passage)
            && plan::spec_press(&self.view, SpecButton::Comment)
            && plan::spec_type(&self.view, comment)
            && plan::spec_answer_popover(&self.view, true)
    }

    /// Asks again for comment `note` and saves it as `comment`; what the popover offered to change.
    fn edit(&self, note: u64, comment: &str) -> Option<String> {
        if !plan::spec_note(&self.view, note, SpecNoteAction::Edit) {
            return None;
        }
        let offered = self.seen().asking;
        let saved = plan::spec_type(&self.view, comment) && plan::spec_answer_popover(&self.view, true);
        offered.filter(|_| saved)
    }

    /// Answers the sheet on its window with `response` once it shows, then runs `then` with whether it showed and
    /// went, so that the next sheet is not taken for this one.
    fn answer_sheet(self: Rc<Self>, response: NSModalResponse, then: impl FnOnce(Rc<Open>, bool) + 'static) {
        let window = self.window.clone();
        helpers::wait_until(
            move || window.attachedSheet().is_some(),
            move |shown| {
                if let Some(sheet) = self.window.attachedSheet() {
                    self.window.endSheet_returnCode(&sheet, response);
                }
                let window = self.window.clone();
                helpers::wait_until(
                    move || window.attachedSheet().is_none(),
                    move |gone| then(self, shown && gone),
                );
            },
        );
    }
}

fn window(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(120.0, 120.0), NSSize::new(900.0, 560.0));
    // SAFETY: NSWindow's designated initializer on a newly allocated window.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back.
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

/// `spec` on view in `window`, its answers kept in `answers`.
fn show(mtm: MainThreadMarker, window: Retained<NSWindow>, spec: Spec, answers: Answers) -> Rc<Open> {
    let view = plan::spec_view(mtm, spec);
    window.setContentView(Some(&view));
    window.makeKeyAndOrderFront(None);
    Rc::new(Open { window, view, answers })
}

/// Who hears a specification's answers, and where they are kept.
fn listener() -> (Answers, Rc<dyn Fn(SpecAnswer)>) {
    let answers = Answers::default();
    let sink = Rc::clone(&answers);
    (answers, Rc::new(move |answer| sink.borrow_mut().push(answer)))
}

/// Runs the step, then `then`.
pub(super) fn run(then: impl FnOnce() + 'static) {
    let Some(mtm) = MainThreadMarker::new() else {
        return give_up(NAME, "the step runs on the main thread");
    };
    let path = ide_core::paths::home().join("specification.md");
    if std::fs::write(&path, SPEC).is_err() {
        return give_up(NAME, "the specification's file is written");
    }
    let (answers, answer) = listener();
    let missing = Spec::read(&path.with_file_name("no-specification.md"), Rc::clone(&answer)).is_none();
    let Some(spec) = Spec::read(&path, answer) else {
        return give_up(NAME, "the specification's file is read");
    };
    check(
        "a specification is read from its Markdown file, and a missing file gives none",
        missing && spec.markdown == SPEC,
        format!("missing: {missing}"),
    );
    let open = show(mtm, window(mtm), spec, answers);
    let seen = open.seen();
    check(
        "a new specification has no comments and offers everything but sending comments",
        seen.notes.is_empty() && seen.boxes == 0 && seen.enabled == [true, true, false, true] && seen.state.is_empty(),
        format!("{seen:?}"),
    );
    comments(&open);
    unsent(open, Box::new(then));
}

/// Comments on the passage and on the whole specification; an empty one and a cancelled one are dropped; an edited one
/// keeps its place, showing it selects its passage, and editing one to nothing removes it.
fn comments(open: &Open) {
    let on_passage = open.comment(PASSAGE, "Name the files that may change");
    let seen = open.seen();
    check(
        "a comment on a marked passage is listed and marks the passage, and the comments can be sent",
        on_passage && seen.notes == [0] && seen.boxes == 1 && seen.marked == 1 && seen.enabled[2],
        format!("{seen:?}"),
    );
    let whole = open.comment("", "Keep it short");
    let blank = open.comment("", "   ");
    let cancelled = plan::spec_press(&open.view, SpecButton::Comment)
        && plan::spec_type(&open.view, "dropped")
        && plan::spec_answer_popover(&open.view, false);
    let seen = open.seen();
    check(
        "a comment on the whole specification comes after those on passages; a blank or cancelled one is dropped",
        whole && blank && cancelled && seen.notes == [0, 1] && seen.boxes == 2 && seen.asking.is_none(),
        format!("{seen:?}"),
    );
    let offered = open.edit(0, EDITED);
    plan::spec_select(&open.view, "");
    let shown = plan::spec_note(&open.view, 0, SpecNoteAction::Show);
    let seen = open.seen();
    check(
        "editing a comment offers its text, and showing it selects its passage",
        offered.as_deref() == Some("Name the files that may change") && shown && seen.selected == PASSAGE,
        format!("{offered:?} {seen:?}"),
    );
    let emptied = open.edit(1, "");
    let seen = open.seen();
    check(
        "a comment edited to nothing is removed",
        emptied.as_deref() == Some("Keep it short") && seen.notes == [0] && seen.marked == 1,
        format!("{seen:?}"),
    );
}

/// Approving with a comment not sent asks first: Cancel leaves everything as it was, "Kommentare senden" sends it.
fn unsent(open: Rc<Open>, then: Then) {
    plan::spec_press(&open.view, SpecButton::Approve);
    open.answer_sheet(NSAlertThirdButtonReturn, move |open, asked| {
        let seen = open.seen();
        check(
            "approving with a comment not sent asks first, and cancelling leaves the specification open",
            asked && open.answers().is_empty() && seen.enabled[3] && seen.notes == [0],
            format!("{seen:?}"),
        );
        plan::spec_press(&open.view, SpecButton::Approve);
        open.answer_sheet(NSAlertFirstButtonReturn, move |open, asked| {
            let waiting = Rc::clone(&open);
            helpers::wait_until(move || !waiting.answers().is_empty(), move |_| sent(open, asked, then));
        });
    });
}

fn sent(open: Rc<Open>, asked: bool, then: Then) {
    let seen = open.seen();
    let expected = SpecAnswer::Comment(format!("> {PASSAGE}\n\n{EDITED}"));
    check(
        "sending from the question hands the comment under its passage over and closes the specification",
        asked
            && open.answers() == [expected]
            && seen.state == t("Kommentare gesendet")
            && seen.enabled == [false; 4]
            && !plan::spec_press(&open.view, SpecButton::Comment),
        format!("{:?} {seen:?}", open.answers()),
    );
    let Some(mtm) = MainThreadMarker::new() else {
        return give_up(NAME, "the step runs on the main thread");
    };
    let (answers, answer) = listener();
    let spec = Spec {
        markdown: SPEC.to_string(),
        answer,
    };
    let second = show(mtm, open.window.clone(), spec, answers);
    drop(open);
    plan::spec_press(&second.view, SpecButton::Discard);
    let waiting = Rc::clone(&second);
    helpers::wait_until(move || !waiting.answers().is_empty(), move |_| discarded(second, then));
}

/// Discarding answers and leaves the specification open; a removed comment is gone, and approving without the
/// comment left approves.
fn discarded(open: Rc<Open>, then: Then) {
    let seen = open.seen();
    check(
        "discarding a specification answers so and leaves it open",
        open.answers() == [SpecAnswer::Discard] && seen.enabled == [true, true, false, true],
        format!("{:?} {seen:?}", open.answers()),
    );
    let added = open.comment(PASSAGE, "Gone again");
    let removed = plan::spec_note(&open.view, 0, SpecNoteAction::Remove);
    let after_remove = open.seen().notes;
    let kept = open.comment(PASSAGE, "Not sent");
    check(
        "removing a comment takes it out of the column",
        added && removed && after_remove.is_empty() && kept,
        format!("{after_remove:?}"),
    );
    plan::spec_press(&open.view, SpecButton::Approve);
    open.answer_sheet(NSAlertSecondButtonReturn, move |open, asked| {
        let waiting = Rc::clone(&open);
        helpers::wait_until(
            move || waiting.answers().len() > 1,
            move |_| {
                let seen = open.seen();
                check(
                    "approving without the comments approves the specification and closes it",
                    asked
                        && open.answers() == [SpecAnswer::Discard, SpecAnswer::Approve]
                        && seen.state == t("Freigegeben")
                        && seen.enabled == [false; 4],
                    format!("{:?} {seen:?}", open.answers()),
                );
                open.window.close();
                then();
            },
        );
    });
}
