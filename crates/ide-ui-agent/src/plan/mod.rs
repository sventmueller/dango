//! A plan the agent asks to approve, with the user's comments on passages of it. The plan's card in the AI tab and
//! its document tab both mark the passages and list the comments; "Mit Kommentaren überarbeiten lassen" sends each
//! comment under the passage it quotes. Approving sends none, as the agent takes no message with an approval.

mod ask;
mod doc;
mod spec;

use std::rc::Rc;

use ide_thread::{EntryKind, PlanApproval};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2_app_kit::{NSColor, NSStackView, NSTextInputClient, NSTextView, NSView};
use objc2_foundation::{NSAttributedString, NSMutableAttributedString, NSRange, NSRect};
use splat_i18n::t;

use super::attrs::{Attribute, add_attribute};
pub use super::cards::request::{PLAN_IN_CARD, PLAN_IN_DOC, PLAN_REJECT, PLAN_REVISE};
use super::tab::AgentTab;
use super::{style, ui};
use ide_ui_common::window::AsWindow;

/// The longest passage a comment quotes back to the agent, in characters.
const MAX_QUOTE: usize = 400;

/// The plans' documents, comments and questions on view.
#[derive(Default)]
pub(crate) struct PlanState {
    notes: PlanNotes,
    docs: doc::PlanDocs,
    ask: ask::PlanAsk,
}

/// The id the last comment on a plan got.
#[derive(Default)]
pub(crate) struct PlanNotes {
    last_note: std::cell::Cell<u64>,
}

fn with_last_note<R>(f: impl FnOnce(&std::cell::Cell<u64>) -> R) -> R {
    f(&crate::host::state().plan.notes.last_note)
}

/// A new comment's id, which its controls carry as their tag.
fn next_note() -> u64 {
    with_last_note(|last| {
        last.set(last.get() + 1);
        last.get()
    })
}

/// A comment on a passage of the plan, or on the whole plan when it quotes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanNote {
    pub id: u64,
    /// Where the passage starts in the plan's rendered text, in UTF-16 units, which tells repeated passages apart.
    pub(crate) location: usize,
    /// The passage.
    pub quote: String,
    /// What the user says about it.
    pub comment: String,
}

impl PlanNote {
    fn is_general(&self) -> bool {
        self.quote.trim().is_empty()
    }
}

/// Puts comments on passages first, in the order of the plan, and comments on the whole plan after them.
fn sort(notes: &mut [PlanNote]) {
    notes.sort_by_key(|n| (n.is_general(), if n.is_general() { 0 } else { n.location }));
}

/// `quote` as a markdown blockquote, cut at `MAX_QUOTE` characters.
fn quoted(quote: &str) -> String {
    let quote = quote.trim();
    let mut cut: String = quote.chars().take(MAX_QUOTE).collect();
    if quote.chars().count() > MAX_QUOTE {
        cut.push('…');
    }
    cut.lines()
        .map(|line| {
            if line.trim().is_empty() {
                ">".to_string()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What "Mit Kommentaren überarbeiten lassen" tells the agent: each passage quoted with its comment below, then the
/// comments on the whole plan and `general`, what the user typed into the card.
pub(crate) fn feedback(notes: &[PlanNote], general: Option<&str>) -> Option<String> {
    let mut notes = notes.to_vec();
    sort(&mut notes);
    let mut parts: Vec<String> = notes
        .iter()
        .filter(|n| !n.comment.trim().is_empty())
        .map(|n| {
            if n.is_general() {
                n.comment.trim().to_string()
            } else {
                format!("{}\n\n{}", quoted(&n.quote), n.comment.trim())
            }
        })
        .collect();
    parts.extend(general.map(str::trim).filter(|g| !g.is_empty()).map(str::to_string));
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Where `note`'s passage is in `text`, as UTF-16 start and length: at its place when the text there still reads
/// the same, else where the passage first is.
pub(crate) fn locate(text: &[u16], note: &PlanNote) -> Option<(usize, usize)> {
    let quote: Vec<u16> = note.quote.encode_utf16().collect();
    if note.is_general() || quote.len() > text.len() {
        return None;
    }
    if text.get(note.location..note.location + quote.len()) == Some(&quote[..]) {
        return Some((note.location, quote.len()));
    }
    text.windows(quote.len())
        .position(|w| w == &quote[..])
        .map(|at| (at, quote.len()))
}

/// `text` with the passages of `notes` marked, each showing its comment when the pointer rests on it.
pub(crate) fn marked(text: &NSAttributedString, notes: &[PlanNote]) -> Retained<NSMutableAttributedString> {
    let out = NSMutableAttributedString::from_attributed_nsstring(text);
    let units: Vec<u16> = out.string().to_string().encode_utf16().collect();
    let mark = Attribute::background(&NSColor::systemYellowColor().colorWithAlphaComponent(0.32));
    let line = Attribute::underline();
    for note in notes {
        if let Some((at, length)) = locate(&units, note) {
            let range = NSRange::new(at, length);
            add_attribute(&out, &mark, range);
            add_attribute(&out, &line, range);
            add_attribute(&out, &Attribute::tool_tip(&note.comment), range);
        }
    }
    out
}

/// The tag of a control of plan `entry`'s card or document, with a number for what it does.
fn entry_tag(entry: u64, what: u8) -> isize {
    isize::try_from(entry << 8 | u64::from(what)).unwrap_or(-1)
}

/// `notes` as boxes `width` points wide, each with the passage it quotes, the comment, and what can be done with it:
/// edited and removed while the plan waits (`editable`), its passage shown in the document (`in_doc`).
pub(crate) fn note_list(
    mtm: MainThreadMarker,
    target: &AnyObject,
    notes: &[PlanNote],
    width: f64,
    (in_doc, editable): (bool, bool),
) -> Retained<NSStackView> {
    let list = ui::column(mtm, 6.0, &[]);
    for note in notes {
        let inner = width - 22.0;
        let passage = if note.is_general() {
            t("Zum ganzen Plan").to_string()
        } else {
            let shown: String = note.quote.trim().chars().take(160).collect();
            format!("„{}“", shown.replace('\n', " "))
        };
        let quote = ui::wrapping(mtm, &passage, &style::small(), &NSColor::secondaryLabelColor(), inner);
        quote.setMaximumNumberOfLines(3);
        let comment = ui::wrapping(mtm, &note.comment, &style::body(), &NSColor::labelColor(), inner);
        let tag = isize::try_from(note.id).unwrap_or(-1);
        let mut actions: Vec<Retained<NSView>> = Vec::new();
        if in_doc && !note.is_general() {
            actions.push(ui::view(&ui::link(
                mtm,
                t("Zeigen"),
                target,
                sel!(ideAgentPlanShowNote:),
                tag,
            )));
        }
        if editable {
            actions.push(ui::view(&ui::link(
                mtm,
                t("Bearbeiten"),
                target,
                sel!(ideAgentPlanEditNote:),
                tag,
            )));
            actions.push(ui::view(&ui::link(
                mtm,
                t("Entfernen"),
                target,
                sel!(ideAgentPlanRemoveNote:),
                tag,
            )));
        }
        actions.push(ui::spacer(mtm));
        let actions: Vec<&NSView> = actions.iter().map(|v| &**v).collect();
        let actions = ui::row(mtm, 10.0, &actions);
        let column = ui::column(mtm, 3.0, &[&quote, &comment, &actions]);
        let yellow = NSColor::systemYellowColor();
        let boxed = ui::framed(
            mtm,
            &column,
            &yellow.colorWithAlphaComponent(0.55),
            Some(&yellow.colorWithAlphaComponent(0.10)),
            6.0,
            ui::insets(6.0, 10.0, 6.0, 10.0),
        );
        ui::add_full_width(&list, &boxed);
    }
    list
}

/// Where `range` is in `view`; the visible part of `view` when it is not in a window.
fn rect_of(view: &NSTextView, range: NSRange) -> NSRect {
    let Some(window) = view.window() else {
        return view.visibleRect();
    };
    // SAFETY: The range lies inside the text; no actual range is asked for.
    let screen = unsafe { view.firstRectForCharacterRange_actualRange(range, std::ptr::null_mut()) };
    let rect = view.convertRect_fromView(window.convertRectFromScreen(screen), None);
    if rect.size.width < 1.0 || rect.size.height < 1.0 {
        return view.visibleRect();
    }
    rect
}

impl AgentTab {
    fn plan_of(&self, id: u64) -> Option<PlanApproval> {
        match &self.entry(id)?.kind {
            EntryKind::PlanApproval(plan) => Some(plan.clone()),
            _ => None,
        }
    }

    fn plan_notes(&self, id: u64) -> Vec<PlanNote> {
        self.timeline()
            .inputs()
            .borrow()
            .plan_notes
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    /// Opens plan `id` as a document in a tab of its own, or selects the one already open.
    pub fn open_plan(&self, id: u64) {
        doc::open(self, id);
    }

    /// Asks for a comment on what is marked in plan `id`'s card or document (`place`), or on the whole plan when
    /// nothing is.
    pub fn plan_comment(&self, id: u64, place: u8) {
        let text = match place {
            PLAN_IN_DOC => doc::text_of(self.key, id),
            PLAN_IN_CARD => self.timeline().inputs().borrow().plan_text.get(&id).cloned(),
            _ => None,
        };
        let Some(text) = text else { return };
        let selected = text.selectedRange();
        let string = text.string();
        if selected.length > 0 && selected.location + selected.length <= string.length() {
            let quote = string.substringWithRange(selected).to_string();
            if !quote.trim().is_empty() {
                let rect = rect_of(&text, selected);
                let asking = self.asking(id, None, selected.location, quote, "");
                return ask::open(self.mtm, self.key, asking, &text, rect);
            }
        }
        let asking = self.asking(id, None, 0, String::new(), "");
        ask::open(self.mtm, self.key, asking, &text, text.visibleRect());
    }

    /// What the comment popover on plan `id` asks: comment `note` or a new one on `quote` at `location`.
    fn asking<'a>(
        &'a self,
        id: u64,
        note: Option<u64>,
        location: usize,
        quote: String,
        current: &'a str,
    ) -> ask::Asking<'a> {
        ask::Asking {
            target: &self.target,
            entry: id,
            note,
            location,
            quote,
            current,
            whole: t("Kommentar zum ganzen Plan"),
        }
    }

    /// The plan comment `note` is on, and the comment.
    fn find_note(&self, note: u64) -> Option<(u64, PlanNote)> {
        let inputs = self.timeline().inputs().borrow();
        inputs
            .plan_notes
            .iter()
            .find_map(|(id, notes)| notes.iter().find(|n| n.id == note).map(|n| (*id, n.clone())))
    }

    /// Asks again for the comment `note`, from the control `anchor` that edits it.
    pub fn plan_edit_note(&self, note: u64, anchor: &NSView) {
        let Some((id, note)) = self.find_note(note) else {
            return;
        };
        let asking = self.asking(id, Some(note.id), note.location, note.quote, &note.comment);
        ask::open(self.mtm, self.key, asking, anchor, anchor.bounds());
    }

    /// Takes what was typed into the comment popover of plan `id`.
    pub fn plan_commit(&self, id: u64) {
        let Some(asked) = ask::take(self.key, id) else { return };
        match asked.note {
            Some(note) => self.edit_plan_note(note, &asked.comment),
            None => self.add_plan_note(id, asked.location, &asked.quote, &asked.comment),
        }
    }

    /// Comments `comment` on the passage `quote` at `location` of plan `id`, or on the whole plan when `quote` is
    /// empty.
    pub(crate) fn add_plan_note(&self, id: u64, location: usize, quote: &str, comment: &str) {
        if comment.trim().is_empty() || self.plan_of(id).is_none() {
            return;
        }
        {
            let inputs = self.timeline().inputs();
            let mut inputs = inputs.borrow_mut();
            let notes = inputs.plan_notes.entry(id).or_default();
            notes.push(PlanNote {
                id: next_note(),
                location,
                quote: quote.to_string(),
                comment: comment.trim().to_string(),
            });
            sort(notes);
        }
        self.plan_changed(id);
    }

    /// Gives the comment `note` the text `comment`; an empty one removes it.
    pub(crate) fn edit_plan_note(&self, note: u64, comment: &str) {
        if comment.trim().is_empty() {
            return self.plan_remove_note(note);
        }
        let Some((id, _)) = self.find_note(note) else { return };
        if let Some(found) = self
            .timeline()
            .inputs()
            .borrow_mut()
            .plan_notes
            .get_mut(&id)
            .and_then(|notes| notes.iter_mut().find(|n| n.id == note))
        {
            found.comment = comment.trim().to_string();
        }
        self.plan_changed(id);
    }

    pub fn plan_remove_note(&self, note: u64) {
        let Some((id, _)) = self.find_note(note) else { return };
        if let Some(notes) = self.timeline().inputs().borrow_mut().plan_notes.get_mut(&id) {
            notes.retain(|n| n.id != note);
        }
        self.plan_changed(id);
    }

    /// Selects and shows the passage of the comment `note` in its plan's document.
    pub(crate) fn plan_show_note(&self, note: u64) {
        if let Some((id, note)) = self.find_note(note) {
            doc::reveal(self.key, id, &note);
        }
    }

    /// What answering plan `id` with "Mit Kommentaren überarbeiten lassen" tells the agent.
    pub fn plan_feedback(&self, id: u64) -> Option<String> {
        let general = self.timeline().inputs().borrow().field_text(id, 0);
        feedback(&self.plan_notes(id), general.as_deref())
    }

    /// Shows plan `id`'s comments again in its card and its document.
    fn plan_changed(&self, id: u64) {
        self.timeline().reload_entry(id);
        if let Some(plan) = self.plan_of(id) {
            doc::show(self, id, &plan, &self.plan_notes(id));
        }
    }

    /// Brings the open plan documents of this tab up to date with the thread.
    pub(super) fn refresh_plan_docs(&self) {
        for id in doc::open_entries(self.key) {
            if let Some(plan) = self.plan_of(id) {
                doc::show(self, id, &plan, &self.plan_notes(id));
            }
        }
    }
}

/// The plan a comment from the keyboard goes to, and where: the plan whose text has the keyboard focus, in its card
/// or its document, else the latest plan that waits in the AI tab with the focus.
pub fn keyboard_target(controller: &(impl AsWindow + ?Sized)) -> Option<(Rc<AgentTab>, u64, u8)> {
    let controller = &controller.as_window();
    let focus = controller.window()?.firstResponder()?.downcast::<NSView>().ok()?;
    let is_focus = |text: &NSTextView| {
        let view: &NSView = text;
        std::ptr::eq(view, &*focus)
    };
    let tabs = crate::of_window(controller);
    for tab in &tabs {
        if let Some(id) = doc::open_entries(tab.key)
            .into_iter()
            .find(|id| doc::text_of(tab.key, *id).is_some_and(|text| is_focus(&text)))
        {
            return Some((tab.clone(), id, PLAN_IN_DOC));
        }
        let card = tab
            .timeline()
            .inputs()
            .borrow()
            .plan_text
            .iter()
            .find_map(|(id, text)| is_focus(text).then_some(*id));
        if let Some(id) = card {
            return Some((tab.clone(), id, PLAN_IN_CARD));
        }
    }
    let tab = tabs.into_iter().find(|tab| focus.isDescendantOf(&tab.root()))?;
    let waiting = tab
        .timeline()
        .inputs()
        .borrow()
        .plan_text
        .keys()
        .copied()
        .filter(|id| tab.plan_of(*id).is_some_and(|plan| plan.request_id.is_some()))
        .max()?;
    Some((tab, waiting, PLAN_IN_CARD))
}

/// Asks for a comment on what is marked in the plan the keyboard is at, as the plan's "Kommentieren" does.
pub fn comment_from_keyboard(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some((tab, id, place)) = keyboard_target(controller) {
        tab.plan_comment(id, place);
    }
}

pub(crate) use ask::close as close_comment;
pub(crate) use doc::close_all as close_docs;
pub use spec::{Spec, SpecAnswer, spec_view};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn note(location: usize, quote: &str, comment: &str) -> PlanNote {
        static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        PlanNote {
            id: LAST.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1,
            location,
            quote: quote.into(),
            comment: comment.into(),
        }
    }

    #[test]
    fn each_comment_goes_back_under_the_passage_it_quotes() {
        let notes = vec![
            note(0, "", "Keep it small"),
            note(30, &"x".repeat(500), " Shorter "),
            note(3, "Fix the parser\n\nfirst", "Write the test first"),
            note(9, "ignored", "  "),
        ];
        let text = feedback(&notes, Some("Thanks")).unwrap();
        let parts: Vec<&str> = text.split("\n\n").collect();
        assert_eq!(parts.len(), 6, "{text}");
        assert_eq!(parts[0], "> Fix the parser\n>\n> first");
        assert_eq!(parts[1], "Write the test first");
        assert!(parts[2].starts_with("> xxx") && parts[2].ends_with("x…"));
        assert_eq!(&parts[3..], ["Shorter", "Keep it small", "Thanks"]);
        assert_eq!(feedback(&[note(0, "a", " ")], Some(" ")), None);
    }

    #[test]
    fn a_passage_is_found_at_its_place_or_where_it_first_is() {
        let text: Vec<u16> = "a test, a test".encode_utf16().collect();
        assert_eq!(locate(&text, &note(8, "a test", "")), Some((8, 6)));
        assert_eq!(locate(&text, &note(3, "a test", "")), Some((0, 6)));
        assert_eq!(locate(&text, &note(0, "gone", "")), None);
        assert_eq!(locate(&text, &note(0, " ", "")), None);
    }
}
