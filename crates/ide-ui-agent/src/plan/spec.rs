//! A specification as a document without a session (E178, F63), such as a work order's Markdown file at G1: the text
//! beside the comments on its passages, laid out as a plan's document. "Kommentare senden" hands every comment under
//! the passage it quotes to whoever reads the answer, the order's planner; "Freigeben" and "Verwerfen" answer it as
//! G1 does. The answer goes out on the next turn of the main thread, as it may take the view away.

use std::cell::{Cell, OnceCell, RefCell};
use std::path::Path;
use std::rc::{Rc, Weak};

use dispatch2::MainThreadBound;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSButton, NSColor, NSModalResponse, NSView};
use objc2_foundation::{NSAttributedString, NSObjectProtocol, NSRange};
use splat_i18n::{t, tf};

use super::super::cards::request::PLAN_APPROVE;
use super::super::markdown::{self, Tone};
use super::super::target::tag;
use super::super::{style, ui};
use super::ask::{self, Asking, Popover};
use super::doc::{Frame, Heading, frame, show_notes};
use super::{PLAN_IN_DOC, PLAN_REJECT, PLAN_REVISE, PlanNote, entry_tag, feedback, locate, rect_of};
use ide_ui_common::alert::{Alert, Role};
use ide_ui_common::util::{guarded, main_soon, ns};

/// What the user does with a specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecAnswer {
    /// Approve it as it is.
    Approve,
    /// Send it back with these comments, each under the passage it quotes.
    Comment(String),
    /// Throw it away.
    Discard,
}

/// A specification to show, and who hears what the user does with it.
pub struct Spec {
    /// Its Markdown.
    pub markdown: String,
    /// Called with each answer, on the main thread.
    pub answer: Rc<dyn Fn(SpecAnswer)>,
}

impl Spec {
    /// The specification in the Markdown file at `path`; `None` when it cannot be read.
    pub fn read(path: &Path, answer: Rc<dyn Fn(SpecAnswer)>) -> Option<Spec> {
        let markdown = std::fs::read_to_string(path).ok()?;
        Some(Spec { markdown, answer })
    }
}

/// An open specification.
struct SpecDoc {
    mtm: MainThreadMarker,
    target: Retained<SpecTarget>,
    frame: Frame,
    rendered: Retained<NSAttributedString>,
    notes: RefCell<Vec<PlanNote>>,
    /// The number the next comment gets.
    next_note: Cell<u64>,
    /// The comment popover on view, which the document keeps itself.
    asking: RefCell<Option<Popover>>,
    /// Comment, discard, send the comments, approve.
    buttons: [Retained<NSButton>; 4],
    answer: Rc<dyn Fn(SpecAnswer)>,
    /// Whether it was approved or sent back; it takes nothing more then.
    answered: Cell<bool>,
}

pub(crate) struct RootIvars {
    doc: OnceCell<Rc<SpecDoc>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `SpecRoot` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESpecDocRoot"]
    #[ivars = RootIvars]
    pub(crate) struct SpecRoot;

    unsafe impl NSObjectProtocol for SpecRoot {}
);

pub(crate) struct TargetIvars {
    doc: RefCell<Weak<SpecDoc>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `SpecTarget` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESpecDocTarget"]
    #[ivars = TargetIvars]
    pub(crate) struct SpecTarget;

    unsafe impl NSObjectProtocol for SpecTarget {}

    impl SpecTarget {
        #[unsafe(method(ideAgentPlan:))]
        fn plan(&self, sender: Option<&AnyObject>) {
            self.on(sender, |doc, tag| doc.answer_with((tag & 0xff) as u8));
        }

        #[unsafe(method(ideAgentPlanComment:))]
        fn plan_comment(&self, sender: Option<&AnyObject>) {
            self.on(sender, |doc, _| doc.comment());
        }

        #[unsafe(method(ideAgentPlanCommit:))]
        fn plan_commit(&self, sender: Option<&AnyObject>) {
            self.on(sender, |doc, _| doc.commit());
        }

        #[unsafe(method(ideAgentPlanCancel:))]
        fn plan_cancel(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(doc) = self.ivars().doc.borrow().upgrade() {
                    doc.dismiss();
                }
            });
        }

        #[unsafe(method(ideAgentPlanEditNote:))]
        fn plan_edit_note(&self, sender: Option<&AnyObject>) {
            let anchor = sender.and_then(|s| s.downcast_ref::<NSView>()).map(|v| v.retain());
            self.on(sender, |doc, note| {
                if let Some(anchor) = &anchor {
                    doc.edit(note, anchor);
                }
            });
        }

        #[unsafe(method(ideAgentPlanRemoveNote:))]
        fn plan_remove_note(&self, sender: Option<&AnyObject>) {
            self.on(sender, |doc, note| doc.remove(note));
        }

        #[unsafe(method(ideAgentPlanShowNote:))]
        fn plan_show_note(&self, sender: Option<&AnyObject>) {
            self.on(sender, |doc, note| doc.reveal(note));
        }
    }
);

impl SpecTarget {
    fn new(mtm: MainThreadMarker) -> Retained<SpecTarget> {
        let this = SpecTarget::alloc(mtm).set_ivars(TargetIvars {
            doc: RefCell::new(Weak::new()),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Runs `f` with the document and the sender's tag, while the document is open.
    fn on(&self, sender: Option<&AnyObject>, f: impl FnOnce(&SpecDoc, u64)) {
        guarded((), || {
            let doc = self.ivars().doc.borrow().upgrade();
            if let (Some(doc), Ok(tag)) = (doc, u64::try_from(tag(sender))) {
                f(&doc, tag);
            }
        });
    }
}

/// The specification `spec` as a view to put into a window, the comments' column beside its text.
pub fn spec_view(mtm: MainThreadMarker, spec: Spec) -> Retained<NSView> {
    let this = SpecRoot::alloc(mtm).set_ivars(RootIvars { doc: OnceCell::new() });
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let root: Retained<SpecRoot> = unsafe { msg_send![super(this), init] };
    let target = SpecTarget::new(mtm);
    let rendered = markdown::render(mtm, &spec.markdown, style::BODY + 1.0, Tone::Primary);
    let button = |title: &str, action, what: u8| ui::button(mtm, title, &target, action, entry_tag(0, what));
    let comment = button(t("Kommentieren"), sel!(ideAgentPlanComment:), PLAN_IN_DOC);
    comment.setToolTip(Some(&ns(t(
        "Kommentiert die markierte Stelle, ohne Markierung die ganze Spezifikation",
    ))));
    let discard = button(t("Verwerfen"), sel!(ideAgentPlan:), PLAN_REJECT);
    let send = button(t("Kommentare senden"), sel!(ideAgentPlan:), PLAN_REVISE);
    let approve = button(t("Freigeben"), sel!(ideAgentPlan:), PLAN_APPROVE);
    approve.setBezelColor(Some(&NSColor::controlAccentColor()));
    let heading = Heading {
        icon: ("doc.text.magnifyingglass", t("Spezifikation")),
        title: t("Spezifikation zur Freigabe"),
        hint: t("Kommentare gehen nur mit „Kommentare senden“ an den Planer, jeder unter der Stelle, die er meint."),
    };
    let frame = frame(mtm, &root, &heading, &[&comment, &discard, &send, &approve]);
    let doc = Rc::new(SpecDoc {
        mtm,
        target,
        frame,
        rendered,
        notes: RefCell::new(Vec::new()),
        next_note: Cell::new(0),
        asking: RefCell::new(None),
        buttons: [comment, discard, send, approve],
        answer: spec.answer,
        answered: Cell::new(false),
    });
    *doc.target.ivars().doc.borrow_mut() = Rc::downgrade(&doc);
    doc.show();
    let _ = root.ivars().doc.set(doc);
    Retained::into_super(root)
}

impl Drop for SpecDoc {
    /// A popover outlives its view otherwise, with nothing left to take its comment.
    fn drop(&mut self) {
        if let Some(popover) = self.asking.get_mut().take() {
            popover.dismiss();
        }
    }
}

impl SpecDoc {
    /// Shows the comments, their marks in the text and what can be done now.
    fn show(&self) {
        let notes = self.notes.borrow().clone();
        let open = !self.answered.get();
        let empty = t("Markiere eine Stelle in der Spezifikation und wähle „Kommentieren“.");
        show_notes(
            self.mtm,
            &self.frame,
            (&self.rendered, &notes),
            &self.target,
            (empty, open),
        );
        let [comment, discard, send, approve] = &self.buttons;
        for button in [comment, discard, approve] {
            button.setEnabled(open);
        }
        send.setEnabled(open && !notes.is_empty());
    }

    /// Asks for a comment on what is marked, or on the whole specification when nothing is.
    fn comment(&self) {
        if self.answered.get() {
            return;
        }
        let text = &self.frame.text;
        let selected = text.selectedRange();
        let string = text.string();
        let quote = (selected.length > 0 && selected.location + selected.length <= string.length())
            .then(|| string.substringWithRange(selected).to_string())
            .filter(|quote| !quote.trim().is_empty());
        let (asking, rect) = match quote {
            Some(quote) => (self.asking(None, selected.location, quote, ""), rect_of(text, selected)),
            None => (self.asking(None, 0, String::new(), ""), text.visibleRect()),
        };
        self.ask(asking, text, rect);
    }

    /// Shows the comment popover for `asking` at `rect` of `anchor`, in place of the one open.
    fn ask(&self, asking: Asking<'_>, anchor: &NSView, rect: objc2_foundation::NSRect) {
        self.dismiss();
        let popover = ask::show(self.mtm, asking, anchor, rect);
        *self.asking.borrow_mut() = Some(popover);
    }

    /// Closes the comment popover, dropping what was typed.
    fn dismiss(&self) {
        let open = self.asking.borrow_mut().take();
        if let Some(popover) = open {
            popover.dismiss();
        }
    }

    fn asking<'a>(&'a self, note: Option<u64>, location: usize, quote: String, current: &'a str) -> Asking<'a> {
        Asking {
            target: &self.target,
            entry: 0,
            note,
            location,
            quote,
            current,
            whole: t("Kommentar zur ganzen Spezifikation"),
        }
    }

    /// Takes what was typed into the comment popover.
    fn commit(&self) {
        // Closing the popover commits once more, which must find nothing left to take.
        let open = self.asking.borrow_mut().take();
        let Some(asked) = open.map(Popover::finish) else {
            return;
        };
        let comment = asked.comment.trim().to_string();
        {
            let mut notes = self.notes.borrow_mut();
            match asked.note {
                Some(id) if comment.is_empty() => notes.retain(|n| n.id != id),
                Some(id) => {
                    if let Some(found) = notes.iter_mut().find(|n| n.id == id) {
                        found.comment = comment;
                    }
                }
                None if comment.is_empty() => {}
                None => notes.push(PlanNote {
                    id: self.next_note.replace(self.next_note.get() + 1),
                    location: asked.location,
                    quote: asked.quote,
                    comment,
                }),
            }
            super::sort(&mut notes);
        }
        self.show();
    }

    /// Asks again for the comment `note`, from the control `anchor` that edits it.
    fn edit(&self, note: u64, anchor: &NSView) {
        let Some(found) = self.notes.borrow().iter().find(|n| n.id == note).cloned() else {
            return;
        };
        let asking = self.asking(Some(found.id), found.location, found.quote, &found.comment);
        self.ask(asking, anchor, anchor.bounds());
    }

    fn remove(&self, note: u64) {
        self.notes.borrow_mut().retain(|n| n.id != note);
        self.show();
    }

    /// Selects and shows the passage of the comment `note`.
    fn reveal(&self, note: u64) {
        let Some(found) = self.notes.borrow().iter().find(|n| n.id == note).cloned() else {
            return;
        };
        let text = &self.frame.text;
        let units: Vec<u16> = text.string().to_string().encode_utf16().collect();
        if let Some((at, length)) = locate(&units, &found) {
            let range = NSRange::new(at, length);
            text.setSelectedRange(range);
            text.scrollRangeToVisible(range);
            text.showFindIndicatorForRange(range);
        }
    }

    /// Answers as the head's button `what` says: approve, send the comments, or discard; approving with comments not
    /// sent asks first, as approving would drop them.
    fn answer_with(&self, what: u8) {
        if self.answered.get() {
            return;
        }
        if what == PLAN_APPROVE && !self.notes.borrow().is_empty() {
            self.ask_unsent();
            return;
        }
        self.give(what);
    }

    /// Asks what becomes of the comments not sent yet: send them to the planner, approve without them, or neither.
    fn ask_unsent(&self) {
        let message = match self.notes.borrow().len() {
            1 => t("1 Kommentar ist noch nicht gesendet.").to_string(),
            n => tf("{} Kommentare sind noch nicht gesendet.", &[&n.to_string()]),
        };
        let alert = Alert::new(self.mtm, &message)
            .warning()
            .informative(t(
                "Beim Freigeben gehen sie verloren. „Kommentare senden“ gibt sie dem Planer, der die Spezifikation dann überarbeitet.",
            ))
            .button(t("Kommentare senden"), Role::Plain)
            .button(t("Ohne Kommentare freigeben"), Role::Destructive)
            .button(t("Abbrechen"), Role::CancelOnEscape);
        let doc = self.target.ivars().doc.borrow().clone();
        let then = move |response: NSModalResponse| {
            let Some(doc) = doc.upgrade() else { return };
            if response == NSAlertFirstButtonReturn {
                doc.answer_with(PLAN_REVISE);
            } else if response == NSAlertSecondButtonReturn {
                doc.give(PLAN_APPROVE);
            }
        };
        match self.frame.text.window() {
            Some(window) => alert.sheet(&window, then),
            None => then(alert.run_modal()),
        }
    }

    fn give(&self, what: u8) {
        if self.answered.get() {
            return;
        }
        let answer = match what {
            PLAN_APPROVE => SpecAnswer::Approve,
            PLAN_REJECT => SpecAnswer::Discard,
            _ => match feedback(&self.notes.borrow(), None) {
                Some(text) => SpecAnswer::Comment(text),
                None => return,
            },
        };
        self.dismiss();
        if answer != SpecAnswer::Discard {
            self.answered.set(true);
            self.frame.state.setStringValue(&ns(match answer {
                SpecAnswer::Approve => t("Freigegeben"),
                _ => t("Kommentare gesendet"),
            }));
            self.show();
        }
        let bound = MainThreadBound::new((self.answer.clone(), answer), self.mtm);
        main_soon(move || {
            let Some(mtm) = MainThreadMarker::new() else { return };
            let (then, answer) = bound.into_inner(mtm);
            then(answer);
        });
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
