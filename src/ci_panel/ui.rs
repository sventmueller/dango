//! The views of the CI panel: the repository and account, the buttons, the tabs for runs, merge requests and issues,
//! problems, the list and the empty states.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSFont, NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation,
    NSLineBreakMode, NSMenu, NSScrollView, NSSegmentStyle, NSSegmentSwitchTracking, NSSegmentedControl, NSStackView,
    NSStackViewDistribution, NSStackViewGravity, NSTableColumn, NSTableViewStyle, NSTextAlignment, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::panel::{CiPanel, Phase, Tab};
use super::text;
use crate::util::{borderless_button, label, ns, pin_edges, set_action, symbol, symbol_button};

/// How far the panel's content keeps from the sidebar's edges: as far as the panel's title.
const MARGIN: f64 = 8.0;
const SYMBOL_POINTS: f64 = 13.0;

pub struct Ui {
    pub page: Retained<NSView>,
    empty: Retained<NSView>,
    content: Retained<NSStackView>,
    title: Retained<NSTextField>,
    account: Retained<NSTextField>,
    pub sign: Retained<NSButton>,
    pub buttons: Buttons,
    pub tabs: Retained<NSSegmentedControl>,
    problem_row: Retained<NSStackView>,
    problem_label: Retained<NSTextField>,
    pub outline: Retained<objc2_app_kit::NSOutlineView>,
    no_runs: Retained<NSTextField>,
    /// Whether the empty state offers the browser login, and whether it can be pressed.
    browser: Cell<Option<bool>>,
}

/// The header's buttons that depend on the tab and the runs.
pub struct Buttons {
    pub start: Retained<NSButton>,
    pub rerun: Retained<NSButton>,
    pub cancel: Retained<NSButton>,
    pub add: Retained<NSButton>,
}

impl Ui {
    pub fn new(mtm: MainThreadMarker, panel: &CiPanel) -> Ui {
        let page = NSView::new(mtm);
        let empty = NSView::new(mtm);
        let content = NSStackView::new(mtm);
        content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        content.setAlignment(NSLayoutAttribute::Leading);
        content.setDistribution(NSStackViewDistribution::Fill);
        content.setSpacing(6.0);
        for view in [&*empty, &content as &NSView] {
            page.addSubview(view);
            pin_edges(view, &page);
        }
        let (header, title, buttons) = header(mtm, panel);
        let (account_row, account, sign) = account_row(mtm, panel);
        let tabs = tabs(mtm, panel);
        let (problem_row, problem_label) = problem_row(mtm, panel);
        let (list, outline) = outline(mtm, panel);
        let no_runs = NSTextField::wrappingLabelWithString(&ns(t("Für diesen Commit gibt es keine Läufe.")), mtm);
        no_runs.setTextColor(Some(&NSColor::secondaryLabelColor()));
        no_runs.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
        for (view, inset) in [
            (&*header as &NSView, MARGIN),
            (&account_row, MARGIN),
            (&tabs, MARGIN),
            (&problem_row, MARGIN),
            (&no_runs, MARGIN),
            (&list, 0.0),
        ] {
            content.addArrangedSubview(view);
            view.leadingAnchor()
                .constraintEqualToAnchor_constant(&content.leadingAnchor(), inset)
                .setActive(true);
            view.trailingAnchor()
                .constraintEqualToAnchor_constant(&content.trailingAnchor(), -inset)
                .setActive(true);
            view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
        }
        list.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        for hidden in [&*problem_row as &NSView, &no_runs, &content] {
            hidden.setHidden(true);
        }
        Ui {
            page,
            empty,
            content,
            title,
            account,
            sign,
            buttons,
            tabs,
            problem_row,
            problem_label,
            outline,
            no_runs,
            browser: Cell::new(None),
        }
    }

    /// Whether the empty state offers the browser login, and whether it can be pressed.
    #[cfg(feature = "selftest")]
    pub fn browser_button(&self) -> Option<bool> {
        self.browser.get()
    }

    /// Shows the runs when the service answered, else why not and what helps.
    pub fn show_phase(&self, panel: &CiPanel, phase: &Phase) {
        let mtm = panel.mtm();
        self.content.setHidden(*phase != Phase::Ready);
        self.empty.setHidden(*phase == Phase::Ready);
        for subview in self.empty.subviews().iter() {
            subview.removeFromSuperview();
        }
        self.browser.set(None);
        let unknown;
        let failed;
        let (symbol_name, title, detail, button): (&str, &str, &str, Option<(&str, Sel)>) = match phase {
            Phase::Ready => return,
            Phase::Loading => ("checkmark.seal", t("CI"), t("Die Läufe werden gelesen …"), None),
            Phase::Untrusted => (
                "lock.shield",
                t("Ordner nicht vertraut"),
                t(
                    "CI fragt erst nach Läufen, wenn du diesem Ordner vertraust: sein Remote bestimmt, wohin die App sich verbindet.",
                ),
                Some((t("Ordner vertrauen …"), sel!(ideCiTrust:))),
            ),
            Phase::NoGit => (
                "exclamationmark.triangle",
                t("Git fehlt"),
                t("Ohne Git lässt sich der Remote nicht lesen."),
                None,
            ),
            Phase::NoRepository => (
                "checkmark.seal",
                t("Kein Git-Repository"),
                t("CI zeigt die Läufe des Remotes eines Repositorys."),
                None,
            ),
            Phase::NoCommit => (
                "checkmark.seal",
                t("Noch kein Commit"),
                t("Läufe gibt es erst für Commits."),
                None,
            ),
            Phase::NoRemote => (
                "checkmark.seal",
                t("Kein Remote"),
                t("Das Repository hat keinen Remote, dessen Läufe sich zeigen ließen."),
                None,
            ),
            Phase::Unknown(host) => {
                unknown = tf(
                    "Welcher Dienst auf {} läuft, ist nicht zu erkennen. Leg ihn in den Einstellungen unter „CI“ fest.",
                    &[host],
                );
                (
                    "questionmark.circle",
                    t("Unbekannter CI-Dienst"),
                    unknown.as_str(),
                    Some((t("CI-Einstellungen öffnen …"), sel!(ideCiOpenSettings:))),
                )
            }
            Phase::SignIn { forge, refused } => {
                return self.show_sign_in(panel, forge, *refused);
            }
            Phase::Failed(message) => {
                failed = message.clone();
                (
                    "exclamationmark.triangle",
                    t("CI meldet einen Fehler"),
                    failed.as_str(),
                    Some((t("Erneut versuchen"), sel!(ideCiRefresh:))),
                )
            }
        };
        let state = crate::git_panel::empty_state(mtm, panel, symbol_name, title, detail, button);
        self.place_empty(&state);
    }

    fn show_sign_in(&self, panel: &CiPanel, forge: &ide_ci::Forge, refused: bool) {
        let mtm = panel.mtm();
        let waiting = panel.read().login.as_ref().map(|login| login.code.clone());
        if let Some(code) = waiting {
            let detail = code.map_or_else(
                || t("Melde dich im Browser an; die App wartet, bis er zurückkehrt.").to_string(),
                |code| {
                    tf(
                        "Gib im Browser den Code {} ein; die App holt den Token danach von selbst.",
                        &[&code],
                    )
                },
            );
            let state = crate::git_panel::empty_state(
                mtm,
                panel,
                "globe",
                t("Warte auf den Browser"),
                &detail,
                Some((t("Abbrechen"), sel!(ideCiCancelLogin:))),
            );
            self.add_problem(mtm, &state, panel.read().problem.as_deref());
            return self.place_empty(&state);
        }
        let detail = if refused {
            tf("{} hat den Token abgelehnt. Melde dich neu an.", &[&forge.host])
        } else {
            tf(
                "Für die Läufe von {} braucht die App eine Anmeldung bei diesem Host.",
                &[&forge.to_string()],
            )
        };
        let available = super::login::availability(forge);
        let (first, second) = if available.is_ok() {
            (
                (t("Im Browser anmelden"), sel!(ideCiBrowserLogin:)),
                (t("Token eingeben …"), sel!(ideCiSignIn:)),
            )
        } else {
            (
                (t("Token eingeben …"), sel!(ideCiSignIn:)),
                (t("Im Browser anmelden"), sel!(ideCiBrowserLogin:)),
            )
        };
        let state = crate::git_panel::empty_state(
            mtm,
            panel,
            "person.badge.key",
            t("Anmeldung nötig"),
            &detail,
            Some(first),
        );
        let other = borderless_button(mtm, second.0, Some(panel), second.1);
        other.setContentTintColor(Some(&NSColor::controlAccentColor()));
        other.setEnabled(available.is_ok() || second.1 != sel!(ideCiBrowserLogin:));
        state.addArrangedSubview(&other);
        if let Err(note) = available {
            state.addArrangedSubview(&small_note(mtm, note));
        }
        self.browser.set(Some(available.is_ok()));
        self.add_problem(mtm, &state, panel.read().problem.as_deref());
        self.place_empty(&state);
    }

    /// Puts `problem` below an empty state, where the problem row of the list is hidden.
    fn add_problem(&self, mtm: MainThreadMarker, state: &NSStackView, problem: Option<&str>) {
        let Some(problem) = problem else { return };
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setAlignment(NSLayoutAttribute::Top);
        row.setSpacing(4.0);
        let icon = NSImageView::new(mtm);
        if let Some(image) = symbol("exclamationmark.triangle.fill", t("Fehler")) {
            icon.setImage(Some(&image));
        }
        icon.setContentTintColor(Some(&NSColor::systemOrangeColor()));
        let text = NSTextField::wrappingLabelWithString(&ns(problem), mtm);
        text.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
        text.setSelectable(true);
        text.setPreferredMaxLayoutWidth(200.0);
        row.addArrangedSubview(&icon);
        row.addArrangedSubview(&text);
        state.addArrangedSubview(&row);
    }

    fn place_empty(&self, state: &NSStackView) {
        self.empty.addSubview(state);
        state.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
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
        ] {
            constraint.setActive(true);
        }
    }

    /// The repository, the account and which buttons apply.
    pub fn show_ready(&self, panel: &CiPanel) {
        let state = panel.read();
        if let Some(forge) = &state.forge {
            self.title.setStringValue(&ns(&forge.to_string()));
            self.title.setToolTip(Some(&ns(&forge.page())));
        }
        let keychain = state.source == Some(ide_ci::Source::Keychain);
        let account = match (&state.login, &state.account, &state.source) {
            (Some(_), _, _) => t("Warte auf die Anmeldung im Browser …").to_string(),
            (None, Some(name), Some(source)) => tf("Angemeldet als {} ({})", &[name, &text::source(source)]),
            (None, None, Some(source)) => tf("Mit Token aus {}", &[&text::source(source)]),
            _ => t("Ohne Anmeldung, nur öffentliche Daten").to_string(),
        };
        self.account.setStringValue(&ns(&account));
        let browser = state
            .forge
            .as_ref()
            .is_some_and(|forge| super::login::availability(forge).is_ok());
        let (title, action) = match (&state.login, keychain) {
            (Some(_), _) => (t("Abbrechen"), sel!(ideCiCancelLogin:)),
            (None, true) => (t("Abmelden"), sel!(ideCiSignOut:)),
            (None, false) if browser => (t("Anmelden …"), sel!(ideCiSignIn:)),
            (None, false) => (t("Token eingeben …"), sel!(ideCiSignIn:)),
        };
        self.sign.setTitle(&ns(title));
        set_action(&self.sign, panel, action);
        self.sign
            .setHidden(state.login.is_none() && state.source.is_some() && !keychain);
        drop(state);
        self.show_tab(panel);
    }

    /// The tab's titles and the buttons that act on what it lists.
    fn show_tab(&self, panel: &CiPanel) {
        let state = panel.read();
        let github = panel.is_github();
        let changes = if github {
            t("Pull Requests")
        } else {
            t("Merge Requests")
        };
        self.tabs.setLabel_forSegment(&ns(changes), 1);
        let index = Tab::ALL.iter().position(|t| *t == state.tab).unwrap_or(0);
        self.tabs.setSelectedSegment(isize::try_from(index).unwrap_or(0));
        let runs = state.tab == Tab::Runs;
        let ready = !state.busy && state.client.is_some();
        let failed = state.runs.iter().any(|r| r.run.state == ide_ci::State::Failed);
        let active = state.runs.iter().any(|r| r.run.state.is_active());
        let startable = state.head.branch.is_some() && (!github || !state.workflows.is_empty());
        let Buttons {
            start,
            rerun,
            cancel,
            add,
        } = &self.buttons;
        for button in [start, rerun, cancel] {
            button.setHidden(!runs);
        }
        add.setHidden(runs);
        rerun.setEnabled(failed && ready);
        cancel.setEnabled(active && ready);
        start.setEnabled(startable && ready);
        start.setToolTip(Some(&ns(match (github, startable) {
            (true, false) if state.head.branch.is_some() => {
                t("Kein Workflow dieses Stands lässt sich von Hand starten (workflow_dispatch).")
            }
            (_, false) => t("Ohne Branch lässt sich kein Lauf starten."),
            _ => t("Lauf starten …"),
        })));
        let (add_tip, add_ok) = match state.tab {
            Tab::Changes if github => (t("Neuer Pull Request im Browser"), state.head.branch.is_some()),
            Tab::Changes => (t("Neuer Merge Request im Browser"), state.head.branch.is_some()),
            _ => (t("Neues Issue im Browser"), true),
        };
        add.setToolTip(Some(&ns(add_tip)));
        add.setAccessibilityLabel(Some(&ns(add_tip)));
        add.setEnabled(add_ok && state.client.is_some());
    }

    /// The note below the list, when it shows.
    #[cfg(feature = "selftest")]
    pub fn note(&self) -> Option<String> {
        (!self.no_runs.isHidden()).then(|| self.no_runs.stringValue().to_string())
    }

    /// Says why the list is empty, or hides the note when it is not.
    pub fn show_empty_list(&self, panel: &CiPanel) {
        let state = panel.read();
        let github = panel.is_github();
        let note = match state.tab {
            Tab::Runs if state.runs.is_empty() => Some(t("Für diesen Commit gibt es keine Läufe.").to_string()),
            Tab::Runs => None,
            _ if state.items_problem.is_some() => state.items_problem.clone(),
            _ if !state.items.is_empty() => None,
            Tab::Changes if github => Some(t("Keine offenen Pull Requests.").to_string()),
            Tab::Changes => Some(t("Keine offenen Merge Requests.").to_string()),
            Tab::Issues => Some(t("Keine offenen Issues.").to_string()),
        };
        self.no_runs.setStringValue(&ns(note.as_deref().unwrap_or("")));
        self.no_runs.setHidden(note.is_none());
    }

    pub fn show_problem(&self, problem: Option<&str>) {
        let Some(problem) = problem else {
            self.problem_row.setHidden(true);
            return;
        };
        let width = (self.page.frame().size.width - 2.0 * MARGIN - 40.0).max(120.0);
        self.problem_label.setPreferredMaxLayoutWidth(width);
        self.problem_label.setStringValue(&ns(problem));
        self.problem_row.setHidden(false);
    }
}

/// A small centred note in tertiary colour.
fn small_note(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let note = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    note.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    note.setTextColor(Some(&NSColor::tertiaryLabelColor()));
    note.setAlignment(NSTextAlignment::Center);
    note.setPreferredMaxLayoutWidth(200.0);
    note
}

fn header(mtm: MainThreadMarker, panel: &CiPanel) -> (Retained<NSStackView>, Retained<NSTextField>, Buttons) {
    let title = label(mtm, "");
    title.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::smallSystemFontSize())));
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    title.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    let button = |name: &str, description: &str, action: Sel| {
        let button = symbol_button(mtm, name, description, SYMBOL_POINTS, Some(panel), action);
        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        button
    };
    let refresh = button("arrow.clockwise", t("Neu laden"), sel!(ideCiRefresh:));
    let start = button("play.circle", t("Lauf starten …"), sel!(ideCiStartRun:));
    let add = button("plus", t("Neues Issue im Browser"), sel!(ideCiNewItem:));
    let rerun = button(
        "arrow.counterclockwise.circle",
        t("Fehlgeschlagene Jobs neu starten"),
        sel!(ideCiRerun:),
    );
    let cancel = button("stop.circle", t("Lauf abbrechen …"), sel!(ideCiCancel:));
    let web = button("safari", t("Im Browser öffnen"), sel!(ideCiOpenWeb:));
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(6.0);
    row.addView_inGravity(&title, NSStackViewGravity::Leading);
    for view in [&*refresh as &NSView, &start, &rerun, &cancel, &add, &web] {
        row.addView_inGravity(view, NSStackViewGravity::Trailing);
    }
    let buttons = Buttons {
        start,
        rerun,
        cancel,
        add,
    };
    (row, title, buttons)
}

/// Runs, merge requests or pull requests, and issues.
fn tabs(mtm: MainThreadMarker, panel: &CiPanel) -> Retained<NSSegmentedControl> {
    let control = NSSegmentedControl::new(mtm);
    control.setSegmentCount(3);
    for (index, title) in [t("Läufe"), t("Merge Requests"), t("Issues")].into_iter().enumerate() {
        control.setLabel_forSegment(&ns(title), isize::try_from(index).unwrap_or(0));
    }
    control.setSegmentStyle(NSSegmentStyle::Automatic);
    control.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
    control.setSegmentDistribution(objc2_app_kit::NSSegmentDistribution::FillEqually);
    control.setControlSize(objc2_app_kit::NSControlSize::Small);
    control.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    control.setSelectedSegment(0);
    // Equal segments make the control three times its longest label wide, which would hold the sidebar wider.
    control.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    crate::util::set_label(&control, Some(t("Was die Liste zeigt")));
    set_action(&control, panel, sel!(ideCiTab:));
    control
}

fn account_row(
    mtm: MainThreadMarker,
    panel: &CiPanel,
) -> (Retained<NSStackView>, Retained<NSTextField>, Retained<NSButton>) {
    let account = label(mtm, "");
    account.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    account.setTextColor(Some(&NSColor::secondaryLabelColor()));
    account.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    account.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    let sign = borderless_button(mtm, t("Token eingeben …"), Some(panel), sel!(ideCiSignIn:));
    sign.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    sign.setContentTintColor(Some(&NSColor::controlAccentColor()));
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(6.0);
    row.addView_inGravity(&account, NSStackViewGravity::Leading);
    row.addView_inGravity(&sign, NSStackViewGravity::Trailing);
    (row, account, sign)
}

fn problem_row(mtm: MainThreadMarker, panel: &CiPanel) -> (Retained<NSStackView>, Retained<NSTextField>) {
    let icon = NSImageView::new(mtm);
    if let Some(image) = symbol("exclamationmark.triangle.fill", t("Fehler")) {
        icon.setImage(Some(&image));
    }
    icon.setContentTintColor(Some(&NSColor::systemOrangeColor()));
    let text = NSTextField::wrappingLabelWithString(&ns(""), mtm);
    text.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    text.setSelectable(true);
    text.setContentCompressionResistancePriority_forOrientation(100.0, NSLayoutConstraintOrientation::Horizontal);
    let dismiss = symbol_button(
        mtm,
        "xmark",
        t("Meldung schließen"),
        10.0,
        Some(panel),
        sel!(ideCiDismissProblem:),
    );
    dismiss.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setAlignment(NSLayoutAttribute::Top);
    row.setSpacing(6.0);
    row.addView_inGravity(&icon, NSStackViewGravity::Leading);
    row.addView_inGravity(&text, NSStackViewGravity::Leading);
    row.addView_inGravity(&dismiss, NSStackViewGravity::Trailing);
    (row, text)
}

fn outline(mtm: MainThreadMarker, panel: &CiPanel) -> (Retained<NSScrollView>, Retained<objc2_app_kit::NSOutlineView>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 400.0));
    let view = super::rows::CiOutlineView::new(mtm, frame);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns("ci"));
    column.setResizingMask(objc2_app_kit::NSTableColumnResizingOptions::AutoresizingMask);
    column.setWidth(frame.size.width);
    view.addTableColumn(&column);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { view.setOutlineTableColumn(Some(&column)) };
    view.setHeaderView(None);
    view.setStyle(NSTableViewStyle::SourceList);
    view.setIndentationPerLevel(10.0);
    view.setAutoresizesOutlineColumn(false);
    view.setColumnAutoresizingStyle(
        objc2_app_kit::NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle,
    );
    view.setAccessibilityLabel(Some(&ns(t("CI-Läufe"))));
    // SAFETY: The outline view holds its data source and delegate weakly; the panel registry keeps the panel alive.
    unsafe {
        view.setDataSource(Some(ProtocolObject::from_ref(panel)));
        view.setDelegate(Some(ProtocolObject::from_ref(panel)));
    }
    set_action(&view, panel, sel!(ideCiDoubleClicked:));
    // SAFETY: The panel answers the double action with a method that takes the sender; `set_action` checked it.
    unsafe { view.setDoubleAction(Some(sel!(ideCiDoubleClicked:))) };
    // SAFETY: A plain action does nothing on a single click; the double action above opens rows.
    unsafe { view.setAction(None) };
    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(panel)));
    // SAFETY: The view keeps the menu; its delegate, the panel, outlives the view through the registry.
    unsafe { view.setMenu(Some(&menu)) };
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(&view));
    view.sizeLastColumnToFit();
    (scroll, view.into_super())
}
