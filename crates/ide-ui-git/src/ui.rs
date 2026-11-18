//! The views of the Git panel: branch and remote controls, progress, problems, the commit message and the list.

use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSBezelStyle, NSBorderType, NSBox, NSBoxType, NSButton, NSColor, NSControlSize, NSFont,
    NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation, NSLineBreakMode, NSMenu, NSPopUpButton,
    NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSStackView, NSStackViewDistribution,
    NSStackViewGravity, NSTextAlignment, NSTextField, NSTitlePosition, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::commit_field::CommitTextView;
use super::model::Phase;
use super::panel::GitPanel;
use super::rows::GitOutlineView;
use super::text::Problem;
use ide_ui_common::panel::{header_button, header_row};
use ide_ui_common::util::{
    activate, borderless_button, label, ns, pin_edges, pin_sides, set_action, symbol, symbol_button,
};

/// How far the panel's content keeps from the sidebar's edges: as far as the panel's title.
pub(super) const MARGIN: f64 = 8.0;
const MESSAGE_HEIGHT: f64 = 62.0;
/// How far the button that asks a model for the message keeps from the message box's corner.
const AI_INSET: f64 = 4.0;

pub(crate) struct Ui {
    pub(crate) page: Retained<NSView>,
    /// Ahead and behind with the remote buttons, which the panel's title row shows.
    pub(crate) header: Retained<NSStackView>,
    empty: Retained<NSView>,
    content: Retained<NSStackView>,
    pub(crate) branch: Retained<NSPopUpButton>,
    pub(crate) distance: Retained<NSTextField>,
    pub(crate) fetch: Retained<NSButton>,
    pub(crate) pull: Retained<NSButton>,
    pub(crate) push: Retained<NSButton>,
    pub(crate) progress_row: Retained<NSStackView>,
    pub(crate) progress: Retained<NSProgressIndicator>,
    pub(crate) progress_label: Retained<NSTextField>,
    pub(crate) cancel: Retained<NSButton>,
    pub(crate) banner: Banner,
    problem_row: Retained<NSStackView>,
    problem_label: Retained<NSTextField>,
    problem_output: Retained<NSButton>,
    pub(crate) message: Retained<CommitTextView>,
    pub(crate) placeholder: Retained<NSTextField>,
    /// Asks a model for the commit message (E88).
    pub(crate) ai: Retained<NSButton>,
    pub(crate) ai_box: AiBox,
    pub(crate) amend: Retained<NSButton>,
    pub(crate) commit: Retained<NSButton>,
    pub(crate) outline: Retained<GitOutlineView>,
}

impl Ui {
    pub(crate) fn new(mtm: MainThreadMarker, panel: &GitPanel) -> Ui {
        let page: Retained<NSView> = super::panel::GitPage::new(mtm, panel.id()).into_super();
        let empty = NSView::new(mtm);
        let content = NSStackView::new(mtm);
        content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        content.setAlignment(NSLayoutAttribute::Leading);
        content.setDistribution(NSStackViewDistribution::Fill);
        content.setSpacing(ide_ui_common::style::space::M);
        for view in [&*empty, &content as &NSView] {
            page.addSubview(view);
            pin_edges(view, &page);
        }
        let (branch_row, branch) = branch_row(mtm);
        let (header, distance, [fetch, pull, push]) = header(mtm, panel);
        let (progress_row, progress, progress_label, cancel) = progress_row(mtm, panel);
        let banner = Banner::new(mtm, panel);
        let (problem_row, problem_label, problem_output) = problem_row(mtm, panel);
        let (message_box, message, placeholder, ai) = message_box(mtm, panel);
        let ai_box = AiBox::new(mtm, panel);
        let (commit_row, amend, commit) = commit_row(mtm, panel);
        let (list, outline) = outline(mtm, panel);
        for (view, inset) in [
            (&*branch_row as &NSView, MARGIN),
            (&progress_row, MARGIN),
            (&banner.view, MARGIN),
            (&problem_row, MARGIN),
            (&message_box, MARGIN),
            (&ai_box.view, MARGIN),
            (&commit_row, MARGIN),
            (&list, 0.0),
        ] {
            content.addArrangedSubview(view);
            pin_sides(view, &content, inset);
            view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
        }
        list.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        progress_row.setHidden(true);
        banner.view.setHidden(true);
        problem_row.setHidden(true);
        ai_box.view.setHidden(true);
        content.setHidden(true);
        header.setHidden(true);
        Ui {
            page,
            header,
            empty,
            content,
            branch,
            distance,
            fetch,
            pull,
            push,
            progress_row,
            progress,
            progress_label,
            cancel,
            banner,
            problem_row,
            problem_label,
            problem_output,
            message,
            placeholder,
            ai,
            ai_box,
            amend,
            commit,
            outline,
        }
    }

    /// Shows the repository's controls when it is ready, else an explanation of why not and what helps.
    pub(crate) fn show_phase(&self, panel: &GitPanel, phase: &Phase) {
        let mtm = panel.mtm();
        self.content.setHidden(*phase != Phase::Ready);
        self.header.setHidden(*phase != Phase::Ready);
        self.empty.setHidden(*phase == Phase::Ready);
        for subview in self.empty.subviews().iter() {
            subview.removeFromSuperview();
        }
        let missing = super::text::problem(&ide_git::Error::GitMissing).message;
        let (symbol_name, title, detail, button): (&str, &str, &str, Option<(&str, Sel)>) = match phase {
            Phase::Ready => return,
            Phase::Loading => (
                "arrow.triangle.branch",
                t("Git"),
                t("Das Repository wird gelesen …"),
                None,
            ),
            Phase::Untrusted => (
                "lock.shield",
                t("Ordner nicht vertraut"),
                t("Git startet erst, wenn du diesem Ordner vertraust."),
                Some((t("Ordner vertrauen …"), sel!(ideGitTrust:))),
            ),
            Phase::NoGit => ("exclamationmark.triangle", t("Git fehlt"), &missing, None),
            Phase::NotRepository => (
                "arrow.triangle.branch",
                t("Kein Git-Repository"),
                t("Dieser Ordner wird noch nicht mit Git versioniert."),
                Some((t("Repository anlegen"), sel!(ideGitInit:))),
            ),
            Phase::Failed(message) => (
                "exclamationmark.triangle",
                t("Git meldet einen Fehler"),
                message.as_str(),
                Some((t("Erneut versuchen"), sel!(ideGitRetry:))),
            ),
        };
        let state = empty_state(mtm, panel, symbol_name, title, detail, button);
        self.empty.addSubview(&state);
        state.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            state
                .centerXAnchor()
                .constraintEqualToAnchor(&self.empty.centerXAnchor()),
            state
                .centerYAnchor()
                .constraintEqualToAnchor_constant(&self.empty.centerYAnchor(), -40.0),
            state
                .leadingAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&self.empty.leadingAnchor(), 16.0),
            state
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&self.empty.trailingAnchor(), -16.0),
        ]);
    }

    /// The width a wrapping text in a row of the panel has, inside the margins and the space of `inset`.
    pub(crate) fn text_width(&self, inset: f64) -> f64 {
        (self.page.frame().size.width - 2.0 * MARGIN - inset).max(120.0)
    }

    /// Shows `problem` above the commit message, or hides the row.
    pub(crate) fn show_problem(&self, problem: Option<&Problem>) {
        let Some(problem) = problem else {
            self.problem_row.setHidden(true);
            return;
        };
        self.problem_label.setPreferredMaxLayoutWidth(self.text_width(52.0));
        self.problem_label.setStringValue(&ns(&problem.message));
        self.problem_output.setHidden(problem.output.is_none());
        self.problem_output.setTitle(&ns(if problem.hook {
            t("Ausgabe des Hooks …")
        } else {
            t("Details …")
        }));
        self.problem_row.setHidden(false);
    }
}

/// What follows a message a model wrote (E101): the note that it needs checking, writing it again, taking it back, and
/// the request that went out.
pub(crate) struct AiBox {
    pub(crate) view: Retained<NSStackView>,
    /// The note and the buttons that stay until the user's first own change.
    pub(crate) written: Retained<NSStackView>,
    pub(crate) sent: Retained<NSButton>,
}

impl AiBox {
    fn new(mtm: MainThreadMarker, panel: &GitPanel) -> AiBox {
        let icon = NSImageView::new(mtm);
        if let Some(image) = symbol("sparkles", "") {
            icon.setImage(Some(&image));
        }
        icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        ide_ui_common::util::hide_from_voiceover(&icon);
        let note = NSTextField::wrappingLabelWithString(&ns(t("Von AI erstellt, bitte prüfen")), mtm);
        note.setFont(Some(&ide_ui_common::style::small_system()));
        note.setTextColor(Some(&NSColor::secondaryLabelColor()));
        note.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
        let line = NSStackView::new(mtm);
        line.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        line.setSpacing(ide_ui_common::style::space::XS);
        line.addArrangedSubview(&icon);
        line.addArrangedSubview(&note);
        let link = |title: &str, action: Sel| {
            let button = borderless_button(mtm, title, Some(panel), action);
            button.setFont(Some(&ide_ui_common::style::small_system()));
            button.setContentTintColor(Some(&NSColor::controlAccentColor()));
            button.setContentCompressionResistancePriority_forOrientation(
                100.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            button
        };
        let again = link(t("Neu erzeugen"), sel!(ideGitMessage:));
        let undo = link(t("Widerrufen"), sel!(ideGitUndoMessage:));
        let sent = link(t("Gesendeten Inhalt zeigen"), sel!(ideGitShowSent:));
        let buttons = NSStackView::new(mtm);
        buttons.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        buttons.setSpacing(ide_ui_common::style::space::L);
        buttons.addArrangedSubview(&again);
        buttons.addArrangedSubview(&undo);
        let written = NSStackView::new(mtm);
        written.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        written.setAlignment(NSLayoutAttribute::Leading);
        written.setSpacing(ide_ui_common::style::space::XXS);
        written.addArrangedSubview(&line);
        written.addArrangedSubview(&buttons);
        let view = NSStackView::new(mtm);
        view.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        view.setAlignment(NSLayoutAttribute::Leading);
        view.setSpacing(ide_ui_common::style::space::XXS);
        view.addArrangedSubview(&written);
        view.addArrangedSubview(&sent);
        view.setAccessibilityLabel(Some(&ns(t("Commit-Nachricht von AI"))));
        AiBox { view, written, sent }
    }
}

/// The note that a merge or rebase stopped at conflicts, with what carries it on or abandons it.
pub(crate) struct Banner {
    pub(crate) view: Retained<NSBox>,
    pub(crate) title: Retained<NSTextField>,
    pub(crate) detail: Retained<NSTextField>,
    pub(crate) resume: Retained<NSButton>,
    pub(crate) abort: Retained<NSButton>,
}

impl Banner {
    fn new(mtm: MainThreadMarker, panel: &GitPanel) -> Banner {
        let view = NSBox::new(mtm);
        view.setBoxType(NSBoxType::Custom);
        view.setTitlePosition(NSTitlePosition::NoTitle);
        view.setBorderWidth(0.0);
        view.setCornerRadius(6.0);
        view.setFillColor(&NSColor::systemOrangeColor().colorWithAlphaComponent(0.16));
        view.setContentViewMargins(NSSize::new(8.0, 6.0));
        let title = label(mtm, "");
        title.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::smallSystemFontSize())));
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let detail = NSTextField::wrappingLabelWithString(&ns(""), mtm);
        detail.setFont(Some(&ide_ui_common::style::small_system()));
        detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
        // Text in the sidebar never makes it wider; it wraps or is cut instead.
        for text in [&title, &detail] {
            text.setContentCompressionResistancePriority_forOrientation(
                100.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
        }
        let button = |title: &str, action: Sel| {
            // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
            let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
            set_action(&button, panel, action);
            button.setBezelStyle(NSBezelStyle::Push);
            button.setControlSize(NSControlSize::Small);
            button.setFont(Some(&ide_ui_common::style::small_system()));
            button.setContentCompressionResistancePriority_forOrientation(
                100.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            button
        };
        let resume = button(t("Fortsetzen"), sel!(ideGitContinue:));
        let abort = button(t("Vorgang abbrechen …"), sel!(ideGitAbort:));
        let buttons = NSStackView::new(mtm);
        buttons.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        buttons.setSpacing(ide_ui_common::style::space::S);
        buttons.addArrangedSubview(&resume);
        buttons.addArrangedSubview(&abort);
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::Leading);
        stack.setSpacing(ide_ui_common::style::space::XS);
        for row in [&*title as &NSView, &detail, &buttons] {
            stack.addArrangedSubview(row);
        }
        if let Some(content) = view.contentView() {
            content.addSubview(&stack);
            pin_edges(&stack, &content);
        }
        Banner {
            view,
            title,
            detail,
            resume,
            abort,
        }
    }
}

type Header = (Retained<NSStackView>, Retained<NSTextField>, [Retained<NSButton>; 3]);

fn branch_row(mtm: MainThreadMarker) -> (Retained<NSStackView>, Retained<NSPopUpButton>) {
    let popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(120.0, 24.0)),
        false,
    );
    popup.setControlSize(NSControlSize::Small);
    popup.setFont(Some(&ide_ui_common::style::small_system()));
    ide_ui_common::util::set_label(&popup, Some(t("Branch")));
    popup.setToolTip(Some(&ns(t("Branch wechseln oder anlegen"))));
    popup.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    popup.setContentHuggingPriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.addView_inGravity(&popup, NSStackViewGravity::Leading);
    (row, popup)
}

fn header(mtm: MainThreadMarker, panel: &GitPanel) -> Header {
    let distance = label(mtm, "");
    distance.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
        NSFont::smallSystemFontSize(),
        ide_ui_common::constant!(objc2_app_kit::NSFontWeightRegular),
    )));
    distance.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let button = |name: &str, title: &str, action: Sel| header_button(mtm, name, title, Some(panel), action);
    let fetch = button("arrow.triangle.2.circlepath", t("Fetch"), sel!(ideGitFetch:));
    let pull = button("arrow.down.circle", t("Pull"), sel!(ideGitPull:));
    let push = button("arrow.up.circle", t("Push"), sel!(ideGitPush:));
    let row = header_row(mtm);
    for view in [&*distance as &NSView, &fetch, &pull, &push] {
        row.addArrangedSubview(view);
    }
    row.setCustomSpacing_afterView(ide_ui_common::style::space::S, &distance);
    (row, distance, [fetch, pull, push])
}

fn progress_row(
    mtm: MainThreadMarker,
    panel: &GitPanel,
) -> (
    Retained<NSStackView>,
    Retained<NSProgressIndicator>,
    Retained<NSTextField>,
    Retained<NSButton>,
) {
    let progress = NSProgressIndicator::new(mtm);
    progress.setStyle(NSProgressIndicatorStyle::Bar);
    progress.setControlSize(NSControlSize::Small);
    progress.setIndeterminate(true);
    progress.setMinValue(0.0);
    progress.setMaxValue(100.0);
    progress.setTranslatesAutoresizingMaskIntoConstraints(false);
    progress.widthAnchor().constraintEqualToConstant(56.0).setActive(true);
    let text = label(mtm, "");
    text.setFont(Some(&ide_ui_common::style::small_system()));
    text.setTextColor(Some(&NSColor::secondaryLabelColor()));
    text.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    text.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    let cancel = symbol_button(
        mtm,
        "xmark.circle.fill",
        t("Abbrechen"),
        ide_ui_common::style::symbol::SMALL,
        Some(panel),
        sel!(ideGitCancel:),
    );
    cancel.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(ide_ui_common::style::space::S);
    row.addView_inGravity(&progress, NSStackViewGravity::Leading);
    row.addView_inGravity(&text, NSStackViewGravity::Leading);
    row.addView_inGravity(&cancel, NSStackViewGravity::Trailing);
    (row, progress, text, cancel)
}

fn problem_row(
    mtm: MainThreadMarker,
    panel: &GitPanel,
) -> (Retained<NSStackView>, Retained<NSTextField>, Retained<NSButton>) {
    let icon = NSImageView::new(mtm);
    if let Some(image) = symbol("exclamationmark.triangle.fill", t("Fehler")) {
        icon.setImage(Some(&image));
    }
    icon.setContentTintColor(Some(&NSColor::systemOrangeColor()));
    icon.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    let text = NSTextField::wrappingLabelWithString(&ns(""), mtm);
    text.setFont(Some(&ide_ui_common::style::small_system()));
    text.setSelectable(true);
    let output = borderless_button(mtm, t("Details …"), Some(panel), sel!(ideGitShowOutput:));
    output.setFont(Some(&ide_ui_common::style::small_system()));
    output.setContentTintColor(Some(&NSColor::controlAccentColor()));
    let dismiss = symbol_button(
        mtm,
        "xmark",
        t("Meldung schließen"),
        10.0,
        Some(panel),
        sel!(ideGitDismissProblem:),
    );
    dismiss.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    let texts = NSStackView::new(mtm);
    texts.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    texts.setAlignment(NSLayoutAttribute::Leading);
    texts.setSpacing(ide_ui_common::style::space::XXS);
    texts.addArrangedSubview(&text);
    texts.addArrangedSubview(&output);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setAlignment(NSLayoutAttribute::Top);
    row.setSpacing(ide_ui_common::style::space::S);
    row.addView_inGravity(&icon, NSStackViewGravity::Leading);
    row.addView_inGravity(&texts, NSStackViewGravity::Leading);
    row.addView_inGravity(&dismiss, NSStackViewGravity::Trailing);
    row.setAccessibilityLabel(Some(&ns(t("Git-Meldung"))));
    (row, text, output)
}

/// The message field with its placeholder on top, in a view of their own: a scroll view draws its clip view over
/// subviews added to it.
fn message_box(
    mtm: MainThreadMarker,
    panel: &GitPanel,
) -> (
    Retained<NSView>,
    Retained<CommitTextView>,
    Retained<NSTextField>,
    Retained<NSButton>,
) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, MESSAGE_HEIGHT));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setBorderType(NSBorderType::BezelBorder);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    let text = CommitTextView::new(mtm, frame);
    text.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSize())));
    text.setTextContainerInset(NSSize::new(2.0, 4.0));
    text.setDelegate(Some(ProtocolObject::from_ref(panel)));
    text.setAccessibilityLabel(Some(&ns(t("Commit-Nachricht"))));
    scroll.setDocumentView(Some(&text));
    let container = NSView::new(mtm);
    container.addSubview(&scroll);
    pin_edges(&scroll, &container);
    container
        .heightAnchor()
        .constraintEqualToConstant(MESSAGE_HEIGHT)
        .setActive(true);
    let placeholder = label(mtm, "");
    placeholder.setTextColor(Some(&NSColor::tertiaryLabelColor()));
    placeholder.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSize())));
    placeholder.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    placeholder
        .setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    ide_ui_common::util::hide_from_voiceover(&placeholder);
    container.addSubview(&placeholder);
    placeholder.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        placeholder
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&container.leadingAnchor(), 9.0),
        placeholder
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&container.trailingAnchor(), -6.0),
        placeholder
            .topAnchor()
            .constraintEqualToAnchor_constant(&container.topAnchor(), 5.0),
    ]);
    let ai = symbol_button(
        mtm,
        "sparkles",
        t("Commit-Nachricht erzeugen"),
        ide_ui_common::style::symbol::SMALL,
        Some(panel),
        sel!(ideGitMessage:),
    );
    ai.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    container.addSubview(&ai);
    ai.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        ai.trailingAnchor()
            .constraintEqualToAnchor_constant(&container.trailingAnchor(), -AI_INSET),
        ai.topAnchor()
            .constraintEqualToAnchor_constant(&container.topAnchor(), AI_INSET),
    ]);
    (container, text, placeholder, ai)
}

fn commit_row(
    mtm: MainThreadMarker,
    panel: &GitPanel,
) -> (Retained<NSStackView>, Retained<NSButton>, Retained<NSButton>) {
    // SAFETY: The panel answers the action with a method that takes the sender; `set_action` checks it again.
    let amend = unsafe { NSButton::checkboxWithTitle_target_action(&ns(t("Letzten Commit ändern")), None, None, mtm) };
    set_action(&amend, panel, sel!(ideGitAmend:));
    amend.setControlSize(NSControlSize::Small);
    amend.setFont(Some(&ide_ui_common::style::small_system()));
    amend.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
    let commit = unsafe { NSButton::buttonWithTitle_target_action(&ns(t("Commit")), None, None, mtm) };
    set_action(&commit, panel, sel!(ideGitCommit:));
    commit.setBezelStyle(NSBezelStyle::Push);
    commit.setControlSize(NSControlSize::Small);
    commit.setBezelColor(Some(&NSColor::controlAccentColor()));
    commit.setToolTip(Some(&ns(t("Committen (⌘↩)"))));
    let more = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(28.0, 22.0)),
        true,
    );
    more.setControlSize(NSControlSize::Small);
    more.setTranslatesAutoresizingMaskIntoConstraints(false);
    more.widthAnchor().constraintEqualToConstant(34.0).setActive(true);
    ide_ui_common::util::set_label(&more, Some(t("Weitere Commit-Aktionen")));
    more.setToolTip(Some(&ns(t("Weitere Commit-Aktionen"))));
    if let Some(menu) = more.menu() {
        menu.addItem(&ide_ui_common::util::item(mtm, "", None, ""));
        for (title, action) in [
            (t("Committen und pushen"), Some(sel!(ideGitCommitPush:))),
            (t("Committen und synchronisieren"), Some(sel!(ideGitCommitSync:))),
            ("", None),
            (t("Stash …"), Some(sel!(ideGitStash:))),
            (t("Nur Vorgemerktes stashen"), Some(sel!(ideGitStashStaged:))),
            ("", None),
            (t("Commit-Nachricht erzeugen"), Some(sel!(ideGitMessage:))),
            (t("Gesendeten Inhalt zeigen"), Some(sel!(ideGitShowSent:))),
        ] {
            let Some(action) = action else {
                menu.addItem(&ide_ui_common::util::separator(mtm));
                continue;
            };
            let entry = ide_ui_common::util::item(mtm, title, Some(action), "");
            ide_ui_common::util::set_target(&entry, panel);
            menu.addItem(&entry);
        }
    }
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(ide_ui_common::style::space::XXS);
    row.addView_inGravity(&amend, NSStackViewGravity::Leading);
    row.addView_inGravity(&commit, NSStackViewGravity::Trailing);
    row.addView_inGravity(&more, NSStackViewGravity::Trailing);
    (row, amend, commit)
}

fn outline(mtm: MainThreadMarker, panel: &GitPanel) -> (Retained<NSScrollView>, Retained<GitOutlineView>) {
    let view = GitOutlineView::new(mtm, ide_ui_common::panel::LIST_FRAME);
    let scroll = ide_ui_common::panel::source_list_outline(mtm, &view, "git", t("Git-Änderungen"));
    // Without a gap between rows the graph's lines run on from one commit to the next.
    view.setIntercellSpacing(NSSize::new(3.0, 0.0));
    view.setAllowsMultipleSelection(true);
    // SAFETY: The outline view holds its data source and delegate weakly; the panel registry keeps the panel alive.
    unsafe {
        view.setDataSource(Some(ProtocolObject::from_ref(panel)));
        view.setDelegate(Some(ProtocolObject::from_ref(panel)));
    }
    set_action(&view, panel, sel!(ideGitRowClicked:));
    assert!(objc2_foundation::NSObjectProtocol::respondsToSelector(
        panel,
        sel!(ideGitRowDoubleClicked:)
    ));
    // SAFETY: The panel answers the double action with a method that takes the sender, checked above.
    unsafe { view.setDoubleAction(Some(sel!(ideGitRowDoubleClicked:))) };
    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(panel)));
    // SAFETY: The view keeps the menu; its delegate, the panel, outlives the view through the registry.
    unsafe { view.setMenu(Some(&menu)) };
    (scroll, view)
}

/// A symbol, a title, a line of explanation and maybe a button that sends `action` to the panel.
pub fn empty_state(
    mtm: MainThreadMarker,
    panel: &objc2::runtime::AnyObject,
    symbol_name: &str,
    title: &str,
    detail: &str,
    button: Option<(&str, Sel)>,
) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::CenterX);
    stack.setSpacing(ide_ui_common::style::space::M);
    stack.setEdgeInsets(NSEdgeInsets {
        top: 0.0,
        left: 0.0,
        bottom: 0.0,
        right: 0.0,
    });
    if let Some(image) = symbol(symbol_name, title) {
        let view = NSImageView::imageViewWithImage(&image, mtm);
        view.setSymbolConfiguration(Some(
            &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
                28.0,
                ide_ui_common::constant!(objc2_app_kit::NSFontWeightLight),
            ),
        ));
        view.setContentTintColor(Some(&NSColor::tertiaryLabelColor()));
        stack.addArrangedSubview(&view);
    }
    let heading = label(mtm, title);
    heading.setFont(Some(&NSFont::boldSystemFontOfSize(ide_ui_common::style::HEADING)));
    heading.setAlignment(NSTextAlignment::Center);
    stack.addArrangedSubview(&heading);
    let explanation = NSTextField::wrappingLabelWithString(&ns(detail), mtm);
    explanation.setTextColor(Some(&NSColor::secondaryLabelColor()));
    explanation.setAlignment(NSTextAlignment::Center);
    explanation.setPreferredMaxLayoutWidth(200.0);
    stack.addArrangedSubview(&explanation);
    if let Some((title, action)) = button {
        // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
        let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
        set_action(&button, panel, action);
        button.setBezelStyle(NSBezelStyle::Push);
        stack.addArrangedSubview(&button);
    }
    stack
}
