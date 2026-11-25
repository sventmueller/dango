//! A plan as a document in a tab of its own: the plan on the left, where the user marks passages, and the comments
//! in a column on the right. A specification without a session (`spec.rs`) is laid out the same way.

use std::cell::RefCell;
use std::rc::Rc;

use ide_thread::{Decision, PlanApproval, RequestState};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSButton, NSColor, NSScrollView, NSStackView, NSTextField, NSTextStorage, NSTextView, NSView};
use objc2_foundation::{NSAttributedString, NSEdgeInsets, NSObjectProtocol, NSRange, NSSize};
use splat_i18n::{t, tf};

use super::super::cards::request::PLAN_APPROVE;
use super::super::markdown::{self, Tone};
use super::super::tab::AgentTab;
use super::super::{style, ui};
use super::{PLAN_IN_DOC, PLAN_REJECT, PLAN_REVISE, PlanNote, locate};
use crate::AgentKey;
use ide_ui_common::util::{activate, guarded, main_soon, ns, pin_inset};
use ide_ui_common::window::{TabDecor, TabKind, Tabs, Window};

/// The width of the comments' column.
const COLUMN: f64 = 320.0;

pub(crate) struct RootIvars {
    agent: AgentKey,
    entry: u64,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `DocRoot` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEPlanDocRoot"]
    #[ivars = RootIvars]
    pub(crate) struct DocRoot;

    unsafe impl NSObjectProtocol for DocRoot {}

    impl DocRoot {
        /// A closed tab takes its document with it.
        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            guarded((), || {
                if self.window().is_some() {
                    return;
                }
                let (agent, entry) = (self.ivars().agent, self.ivars().entry);
                main_soon(move || {
                    (crate::host::state().plan.docs.docs.borrow_mut())
                        .retain(|d| !(d.agent == agent && d.entry == entry && d.root.window().is_none()));
                });
            });
        }
    }
);

/// An open plan document.
struct PlanDoc {
    agent: AgentKey,
    entry: u64,
    root: Retained<DocRoot>,
    text: Retained<NSTextView>,
    /// The plan as rendered, before its passages are marked.
    rendered: Retained<NSAttributedString>,
    notes: Retained<NSStackView>,
    state: Retained<NSTextField>,
    buttons: [Retained<NSButton>; 4],
}

/// The open plan documents.
#[derive(Default)]
pub(super) struct PlanDocs {
    docs: RefCell<Vec<Rc<PlanDoc>>>,
}

/// The document of plan `entry` of the AI tab `agent` among the open `docs`.
fn doc(docs: &PlanDocs, agent: AgentKey, entry: u64) -> Option<Rc<PlanDoc>> {
    (docs.docs.borrow().iter())
        .find(|d| d.agent == agent && d.entry == entry)
        .cloned()
}

/// The text of `view`.
fn storage(view: &NSTextView) -> Option<Retained<NSTextStorage>> {
    // SAFETY: The text view made by `scrollableTextView` owns its text system; asking for the storage keeps TextKit 2.
    unsafe { view.textStorage() }
}

/// Pins each of `views` into `root` with `constraints`, made once all of them are in it.
fn place(
    root: &NSView,
    views: &[&NSView],
    constraints: impl FnOnce() -> Vec<Retained<objc2_app_kit::NSLayoutConstraint>>,
) {
    for view in views {
        root.addSubview(view);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    activate(constraints());
}

/// The comments' column: the list, scrolling, above `hint`, how comments reach the agent.
fn side(mtm: MainThreadMarker, notes: &NSStackView, hint: &str) -> Retained<NSView> {
    let side = NSView::new(mtm);
    let scroll = NSScrollView::new(mtm);
    let document = crate::agent_panel::flipped(mtm);
    scroll.setDocumentView(Some(&document));
    scroll.setHasVerticalScroller(true);
    scroll.setDrawsBackground(false);
    document.addSubview(notes);
    let inset = NSEdgeInsets {
        top: 14.0,
        left: 14.0,
        bottom: 14.0,
        right: 14.0,
    };
    pin_inset(notes, &document, inset);
    document.setTranslatesAutoresizingMaskIntoConstraints(false);
    document
        .widthAnchor()
        .constraintEqualToAnchor(&scroll.contentView().widthAnchor())
        .setActive(true);
    let hint = ui::wrapping(
        mtm,
        hint,
        &style::small(),
        &NSColor::tertiaryLabelColor(),
        COLUMN - 28.0,
    );
    place(&side, &[&scroll, &hint], || {
        vec![
            scroll.topAnchor().constraintEqualToAnchor(&side.topAnchor()),
            scroll.leadingAnchor().constraintEqualToAnchor(&side.leadingAnchor()),
            scroll.trailingAnchor().constraintEqualToAnchor(&side.trailingAnchor()),
            scroll
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&hint.topAnchor(), -10.0),
            hint.leadingAnchor()
                .constraintEqualToAnchor_constant(&side.leadingAnchor(), 14.0),
            hint.trailingAnchor()
                .constraintEqualToAnchor_constant(&side.trailingAnchor(), -14.0),
            hint.bottomAnchor()
                .constraintEqualToAnchor_constant(&side.bottomAnchor(), -14.0),
        ]
    });
    side
}

/// The parts of a document's tab that change with what it shows.
pub(super) struct Frame {
    /// The document's text, where the user marks passages.
    pub(super) text: Retained<NSTextView>,
    /// The comments' column.
    pub(super) notes: Retained<NSStackView>,
    /// Where the document stands, in its head.
    pub(super) state: Retained<NSTextField>,
}

/// What a document's tab names in its head.
pub(super) struct Heading<'a> {
    /// The symbol before the title, and what VoiceOver calls it.
    pub(super) icon: (&'a str, &'a str),
    pub(super) title: &'a str,
    /// How comments reach whoever reads them, below the comments.
    pub(super) hint: &'a str,
}

/// Lays a document out in `root`: the head with `heading`, the state and `buttons`; the text on the left and the
/// comments' column on the right.
pub(super) fn frame(mtm: MainThreadMarker, root: &NSView, heading: &Heading<'_>, buttons: &[&NSButton]) -> Frame {
    let scroll = NSTextView::scrollableTextView(mtm);
    let text = scroll
        .documentView()
        .and_then(|v| v.downcast::<NSTextView>().ok())
        .unwrap_or_else(|| NSTextView::new(mtm));
    text.setEditable(false);
    text.setSelectable(true);
    text.setTextContainerInset(NSSize::new(28.0, 22.0));
    let icon = ui::icon(
        mtm,
        heading.icon.0,
        heading.icon.1,
        15.0,
        &NSColor::secondaryLabelColor(),
    );
    let title = ui::label(mtm, heading.title, &style::bold(15.0), &NSColor::labelColor());
    let state = ui::label(mtm, "", &style::small(), &NSColor::secondaryLabelColor());
    let spacer = ui::spacer(mtm);
    let mut parts: Vec<&NSView> = vec![&icon, &title, &spacer, &state];
    parts.extend(buttons.iter().map(|button| -> &NSView { button }));
    let head = ui::row(mtm, 8.0, &parts);
    head.setEdgeInsets(ui::insets(10.0, 16.0, 10.0, 14.0));
    let notes = ui::column(mtm, 8.0, &[]);
    let side = side(mtm, &notes, heading.hint);
    let (line, divider) = (ui::separator(mtm), ui::separator(mtm));
    place(root, &[&head, &line, &scroll, &divider, &side], || {
        vec![
            head.topAnchor().constraintEqualToAnchor(&root.topAnchor()),
            head.leadingAnchor().constraintEqualToAnchor(&root.leadingAnchor()),
            head.trailingAnchor().constraintEqualToAnchor(&root.trailingAnchor()),
            line.topAnchor().constraintEqualToAnchor(&head.bottomAnchor()),
            line.leadingAnchor().constraintEqualToAnchor(&root.leadingAnchor()),
            line.trailingAnchor().constraintEqualToAnchor(&root.trailingAnchor()),
            line.heightAnchor().constraintEqualToConstant(1.0),
            scroll.topAnchor().constraintEqualToAnchor(&line.bottomAnchor()),
            scroll.leadingAnchor().constraintEqualToAnchor(&root.leadingAnchor()),
            scroll.bottomAnchor().constraintEqualToAnchor(&root.bottomAnchor()),
            scroll
                .trailingAnchor()
                .constraintEqualToAnchor(&divider.leadingAnchor()),
            divider.topAnchor().constraintEqualToAnchor(&line.bottomAnchor()),
            divider.bottomAnchor().constraintEqualToAnchor(&root.bottomAnchor()),
            divider.widthAnchor().constraintEqualToConstant(1.0),
            divider.trailingAnchor().constraintEqualToAnchor(&side.leadingAnchor()),
            side.topAnchor().constraintEqualToAnchor(&line.bottomAnchor()),
            side.bottomAnchor().constraintEqualToAnchor(&root.bottomAnchor()),
            side.trailingAnchor().constraintEqualToAnchor(&root.trailingAnchor()),
            side.widthAnchor().constraintEqualToConstant(COLUMN),
        ]
    });
    Frame { text, notes, state }
}

/// Shows `notes` in `frame`'s column, or `empty` while there are none, and marks their passages in `rendered`;
/// `editable` while the document takes comments, whose links go to `target`.
pub(super) fn show_notes(
    mtm: MainThreadMarker,
    frame: &Frame,
    (rendered, notes): (&NSAttributedString, &[PlanNote]),
    target: &objc2::runtime::AnyObject,
    (empty, editable): (&str, bool),
) {
    if let Some(storage) = storage(&frame.text) {
        let selected = frame.text.selectedRange();
        storage.setAttributedString(&super::marked(rendered, notes));
        if selected.location + selected.length <= storage.length() {
            frame.text.setSelectedRange(selected);
        }
    }
    for view in frame.notes.arrangedSubviews().iter() {
        frame.notes.removeArrangedSubview(&view);
        view.removeFromSuperview();
    }
    if notes.is_empty() {
        let empty = ui::wrapping(
            mtm,
            empty,
            &style::small(),
            &NSColor::secondaryLabelColor(),
            COLUMN - 28.0,
        );
        frame.notes.addArrangedSubview(&empty);
    } else {
        let list = super::note_list(mtm, target, notes, COLUMN - 28.0, (true, editable));
        ui::add_full_width(&frame.notes, &list);
    }
}

impl PlanDoc {
    fn build(tab: &AgentTab, entry: u64, plan: &str) -> Rc<PlanDoc> {
        let mtm = tab.mtm;
        let this = DocRoot::alloc(mtm).set_ivars(RootIvars { agent: tab.key, entry });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let root: Retained<DocRoot> = unsafe { msg_send![super(this), init] };
        let rendered = markdown::render(mtm, plan, style::BODY + 1.0, Tone::Primary);
        let tag = |what: u8| super::entry_tag(entry, what);
        let target = &tab.target;
        let comment = ui::button(
            mtm,
            t("Kommentieren"),
            target,
            sel!(ideAgentPlanComment:),
            tag(PLAN_IN_DOC),
        );
        comment.setToolTip(Some(&ns(t(
            "Kommentiert die markierte Stelle, ohne Markierung den ganzen Plan",
        ))));
        let reject = ui::button(mtm, t("Plan ablehnen"), target, sel!(ideAgentPlan:), tag(PLAN_REJECT));
        reject.setToolTip(Some(&ns(t("Der Plan wird nicht umgesetzt, der Agent hält an"))));
        let revise = ui::button(
            mtm,
            t("Überarbeiten lassen"),
            target,
            sel!(ideAgentPlan:),
            tag(PLAN_REVISE),
        );
        let approve = ui::button(mtm, t("Plan annehmen"), target, sel!(ideAgentPlan:), tag(PLAN_APPROVE));
        approve.setBezelColor(Some(&NSColor::controlAccentColor()));
        let heading = Heading {
            icon: ("list.bullet.clipboard", t("Plan")),
            title: t("Plan zur Freigabe"),
            hint: t(
                "Kommentare gehen nur mit „Mit Kommentaren überarbeiten lassen“ an den Agenten, jeder unter der Stelle, die er meint.",
            ),
        };
        let buttons: [&NSButton; 4] = [&comment, &reject, &revise, &approve];
        let Frame { text, notes, state } = frame(mtm, &root, &heading, &buttons);
        Rc::new(PlanDoc {
            agent: tab.key,
            entry,
            root,
            text,
            rendered,
            notes,
            state,
            buttons: [comment, reject, revise, approve],
        })
    }

    /// Shows `notes`, their marks in the text and where `plan` stands.
    fn show(&self, tab: &AgentTab, plan: &PlanApproval, notes: &[PlanNote]) {
        let pending = plan.state == RequestState::Pending;
        let frame = Frame {
            text: self.text.clone(),
            notes: self.notes.clone(),
            state: self.state.clone(),
        };
        let empty = t("Markiere eine Stelle im Plan und wähle „Kommentieren“.");
        show_notes(tab.mtm, &frame, (&self.rendered, notes), &tab.target, (empty, pending));
        let [comment, reject, revise, approve] = &self.buttons;
        comment.setEnabled(pending);
        for button in [reject, revise, approve] {
            button.setEnabled(pending && plan.request_id.is_some());
        }
        revise.setTitle(&ns(if notes.is_empty() {
            t("Überarbeiten lassen")
        } else {
            t("Mit Kommentaren überarbeiten lassen")
        }));
        self.state.setStringValue(&ns(&state_text(plan)));
    }
}

/// Where `plan` stands, as the document's head says; nothing while it waits.
fn state_text(plan: &PlanApproval) -> String {
    match &plan.state {
        RequestState::Pending => String::new(),
        RequestState::Decided(Decision::Approved) => t("Plan angenommen").to_string(),
        RequestState::Decided(Decision::Rejected { stop: true, .. }) => t("Plan abgelehnt").to_string(),
        RequestState::Decided(Decision::Rejected { .. }) => t("Plan zurückgegeben").to_string(),
        _ => t("Plan ohne Entscheidung").to_string(),
    }
}

/// What names the document tab of plan `id` in the AI tab `agent`.
fn tab_key(agent: AgentKey, id: u64) -> String {
    format!("plan:{}:{id}", agent.get())
}

/// Closes the document tabs of the AI tab `agent` among those `state` holds, which closed, as their answers would go
/// nowhere.
pub(crate) fn close_all(state: &crate::State, controller: &Window, agent: AgentKey) {
    for id in open_entries(&state.plan.docs, agent) {
        if let Some(tab) = controller.tab_with_key(&tab_key(agent, id)) {
            controller.close_tab(tab);
        }
    }
}

/// Opens plan `id` of `tab` as a document in a tab of its own among the open `docs`, or selects the one already open.
pub(super) fn open(docs: &PlanDocs, tab: &AgentTab, id: u64) {
    let (Some(controller), Some(plan)) = (tab.controller(), tab.plan_of(id)) else {
        return;
    };
    let key = tab_key(tab.key, id);
    if controller.select_tab(&key) {
        return focus(docs, &controller, tab.key, id);
    }
    let doc = PlanDoc::build(tab, id, &plan.plan);
    docs.docs.borrow_mut().push(doc.clone());
    doc.show(tab, &plan, &tab.plan_notes(id));
    let view: Retained<NSView> = Retained::into_super(doc.root.clone());
    let title = tf("Plan · {}", &[&super::super::format::first_line(&tab.title(), 32)]);
    let Some(tab_id) = controller.add_tab(TabKind::Plan, Some(key), &title, view) else {
        return;
    };
    controller.set_tab_tint(tab_id, &tab.agent.colour());
    if let Some(colors) = tab.agent.four_colours() {
        controller.set_tab_four_colours(tab_id, colors);
    }
    focus(docs, &controller, tab.key, id);
}

/// Puts the keyboard focus on the plan among the open `docs`, where the keyboard marks passages to comment on.
fn focus(docs: &PlanDocs, controller: &Window, agent: AgentKey, id: u64) {
    if let (Some(window), Some(text)) = (controller.window(), text_in(docs, agent, id)) {
        window.makeFirstResponder(Some(&text));
    }
}

/// Shows plan `id`'s document, if it is among the open `docs`, as `plan` and `notes` now are.
pub(super) fn show(docs: &PlanDocs, tab: &AgentTab, id: u64, plan: &PlanApproval, notes: &[PlanNote]) {
    if let Some(doc) = doc(docs, tab.key, id) {
        doc.show(tab, plan, notes);
    }
}

/// The plans of the AI tab `agent` that are open as documents among `docs`.
pub(super) fn open_entries(docs: &PlanDocs, agent: AgentKey) -> Vec<u64> {
    (docs.docs.borrow().iter())
        .filter(|d| d.agent == agent)
        .map(|d| d.entry)
        .collect()
}

/// The text view of plan `id`'s document among the open `docs`.
pub(super) fn text_in(docs: &PlanDocs, agent: AgentKey, id: u64) -> Option<Retained<NSTextView>> {
    doc(docs, agent, id).map(|d| d.text.clone())
}

/// Selects the passage of `note` in plan `id`'s document among the open `docs` and points at it.
pub(super) fn reveal(docs: &PlanDocs, agent: AgentKey, id: u64, note: &PlanNote) {
    let Some(doc) = doc(docs, agent, id) else { return };
    let units: Vec<u16> = doc.text.string().to_string().encode_utf16().collect();
    if let Some((at, length)) = locate(&units, note) {
        let range = NSRange::new(at, length);
        doc.text.setSelectedRange(range);
        doc.text.scrollRangeToVisible(range);
        doc.text.showFindIndicatorForRange(range);
    }
}

/// What an open plan document shows, for the self-test.
#[cfg(feature = "selftest")]
#[derive(Debug)]
pub struct DocSeen {
    /// The comments in its column.
    pub notes: usize,
    /// The passages marked in its text.
    pub marked: usize,
    /// Whether it still takes an answer.
    pub waiting: bool,
    /// What its head says about the plan.
    pub state: String,
    /// The title of the button that sends the comments.
    pub revise: String,
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
