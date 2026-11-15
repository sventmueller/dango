//! A plan as a document in a tab of its own: the plan on the left, where the user marks passages, and the comments
//! in a column on the right.

use std::cell::RefCell;
use std::rc::Rc;

use ide_thread::{Decision, PlanApproval, RequestState};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSButton, NSColor, NSScrollView, NSStackView, NSTextField, NSTextStorage, NSTextView, NSView};
use objc2_foundation::{NSAttributedString, NSObjectProtocol, NSRange, NSSize};
use splat_i18n::{t, tf};

use super::super::cards::request::PLAN_APPROVE;
use super::super::markdown::{self, Tone};
use super::super::tab::AgentTab;
use super::super::{style, ui};
use super::{PLAN_IN_DOC, PLAN_REJECT, PLAN_REVISE, PlanNote, locate};
use crate::AgentKey;
use ide_ui_common::util::{guarded, main_soon, ns};
use ide_ui_common::window::{AsWindow, TabDecor, TabKind, Tabs, Window};

/// The width of the comments' column.
const COLUMN: f64 = 320.0;

pub struct RootIvars {
    agent: AgentKey,
    entry: u64,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `DocRoot` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEPlanDocRoot"]
    #[ivars = RootIvars]
    pub struct DocRoot;

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
                    with_docs(|docs| {
                        docs.borrow_mut()
                            .retain(|d| !(d.agent == agent && d.entry == entry && d.root.window().is_none()));
                    });
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
pub struct PlanDocs {
    docs: RefCell<Vec<Rc<PlanDoc>>>,
}

fn with_docs<R>(f: impl FnOnce(&RefCell<Vec<Rc<PlanDoc>>>) -> R) -> R {
    f(&crate::host::state().plan.docs.docs)
}

fn doc(agent: AgentKey, entry: u64) -> Option<Rc<PlanDoc>> {
    with_docs(|docs| {
        docs.borrow()
            .iter()
            .find(|d| d.agent == agent && d.entry == entry)
            .cloned()
    })
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
    for constraint in constraints() {
        constraint.setActive(true);
    }
}

/// The comments' column: the list, scrolling, above a hint how comments reach the agent.
fn side(mtm: MainThreadMarker, notes: &NSStackView) -> Retained<NSView> {
    let side = NSView::new(mtm);
    let scroll = NSScrollView::new(mtm);
    let document = crate::agent_panel::flipped(mtm);
    scroll.setDocumentView(Some(&document));
    scroll.setHasVerticalScroller(true);
    scroll.setDrawsBackground(false);
    place(&document, &[notes], || {
        vec![
            notes
                .topAnchor()
                .constraintEqualToAnchor_constant(&document.topAnchor(), 14.0),
            notes
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&document.leadingAnchor(), 14.0),
            notes
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&document.trailingAnchor(), -14.0),
            notes
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&document.bottomAnchor(), -14.0),
        ]
    });
    document.setTranslatesAutoresizingMaskIntoConstraints(false);
    document
        .widthAnchor()
        .constraintEqualToAnchor(&scroll.contentView().widthAnchor())
        .setActive(true);
    let hint = ui::wrapping(
        mtm,
        t(
            "Kommentare gehen nur mit „Mit Kommentaren überarbeiten lassen“ an den Agenten, jeder unter der Stelle, die er meint.",
        ),
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

impl PlanDoc {
    fn build(tab: &AgentTab, entry: u64, plan: &str) -> Rc<PlanDoc> {
        let mtm = tab.mtm;
        let this = DocRoot::alloc(mtm).set_ivars(RootIvars { agent: tab.key, entry });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let root: Retained<DocRoot> = unsafe { msg_send![super(this), init] };
        let scroll = NSTextView::scrollableTextView(mtm);
        let text = scroll
            .documentView()
            .and_then(|v| v.downcast::<NSTextView>().ok())
            .unwrap_or_else(|| NSTextView::new(mtm));
        text.setEditable(false);
        text.setSelectable(true);
        text.setTextContainerInset(NSSize::new(28.0, 22.0));
        let rendered = markdown::render(mtm, plan, style::BODY + 1.0, Tone::Primary);
        let icon = ui::icon(
            mtm,
            "list.bullet.clipboard",
            t("Plan"),
            15.0,
            &NSColor::secondaryLabelColor(),
        );
        let title = ui::label(mtm, t("Plan zur Freigabe"), &style::bold(15.0), &NSColor::labelColor());
        let state = ui::label(mtm, "", &style::small(), &NSColor::secondaryLabelColor());
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
        let spacer = ui::spacer(mtm);
        let head = ui::row(
            mtm,
            8.0,
            &[&icon, &title, &spacer, &state, &comment, &reject, &revise, &approve],
        );
        head.setEdgeInsets(ui::insets(10.0, 16.0, 10.0, 14.0));
        let notes = ui::column(mtm, 8.0, &[]);
        let side = side(mtm, &notes);
        let (line, divider) = (ui::separator(mtm), ui::separator(mtm));
        place(&root, &[&head, &line, &scroll, &divider, &side], || {
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
        let mtm = tab.mtm;
        let pending = plan.state == RequestState::Pending;
        if let Some(storage) = storage(&self.text) {
            let selected = self.text.selectedRange();
            storage.setAttributedString(&super::marked(&self.rendered, notes));
            if selected.location + selected.length <= storage.length() {
                self.text.setSelectedRange(selected);
            }
        }
        for view in self.notes.arrangedSubviews().iter() {
            self.notes.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
        if notes.is_empty() {
            let empty = ui::wrapping(
                mtm,
                t("Markiere eine Stelle im Plan und wähle „Kommentieren“."),
                &style::small(),
                &NSColor::secondaryLabelColor(),
                COLUMN - 28.0,
            );
            self.notes.addArrangedSubview(&empty);
        } else {
            let list = super::note_list(mtm, &tab.target, notes, COLUMN - 28.0, (true, pending));
            ui::add_full_width(&self.notes, &list);
        }
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

/// Closes the document tabs of the AI tab `agent`, which closed, as their answers would go nowhere.
pub fn close_all(controller: &(impl AsWindow + ?Sized), agent: AgentKey) {
    let controller = &controller.as_window();
    for id in open_entries(agent) {
        if let Some(tab) = controller.tab_with_key(&tab_key(agent, id)) {
            controller.close_tab(tab);
        }
    }
}

/// Opens plan `id` of `tab` as a document in a tab of its own, or selects the one already open.
pub fn open(tab: &AgentTab, id: u64) {
    let (Some(controller), Some(plan)) = (tab.controller(), tab.plan_of(id)) else {
        return;
    };
    let key = tab_key(tab.key, id);
    if controller.select_tab(&key) {
        return focus(&controller, tab.key, id);
    }
    let doc = PlanDoc::build(tab, id, &plan.plan);
    with_docs(|docs| docs.borrow_mut().push(doc.clone()));
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
    focus(&controller, tab.key, id);
}

/// Puts the keyboard focus on the plan, where the keyboard marks passages to comment on.
fn focus(controller: &Window, agent: AgentKey, id: u64) {
    if let (Some(window), Some(text)) = (controller.window(), text_of(agent, id)) {
        window.makeFirstResponder(Some(&text));
    }
}

/// Shows plan `id`'s document, if it is open, as `plan` and `notes` now are.
pub fn show(tab: &AgentTab, id: u64, plan: &PlanApproval, notes: &[PlanNote]) {
    if let Some(doc) = doc(tab.key, id) {
        doc.show(tab, plan, notes);
    }
}

/// The plans of the AI tab `agent` that are open as documents.
pub fn open_entries(agent: AgentKey) -> Vec<u64> {
    with_docs(|docs| {
        docs.borrow()
            .iter()
            .filter(|d| d.agent == agent)
            .map(|d| d.entry)
            .collect()
    })
}

/// The text view of plan `id`'s document.
pub fn text_of(agent: AgentKey, id: u64) -> Option<Retained<NSTextView>> {
    doc(agent, id).map(|d| d.text.clone())
}

/// Selects the passage of `note` in plan `id`'s document and points at it.
pub fn reveal(agent: AgentKey, id: u64, note: &PlanNote) {
    let Some(doc) = doc(agent, id) else { return };
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
