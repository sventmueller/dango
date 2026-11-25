//! The rows of the panel and the inspector, what selecting, opening and Return do with them, and their context menus.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem, NSOutlineView, NSView,
};
use objc2_foundation::NSNotification;
use splat_i18n::{t, tf};

use super::model::breakpoints::Look;
use super::model::rows::{Key, Section};
use super::model::session::{Origin, Phase, Session};
use super::model::variables::{Part, Path, Row};
use super::panel::{DebugPanel, RowKey, key_of, row_of};
use super::ui;
use ide_ui_common::util::{ns, set_target};

/// How a session stands, in words.
pub(crate) fn phase_words(session: &Session) -> String {
    match &session.phase {
        Phase::Starting if session.waiting => t("Wartet auf Freigabe durch macOS …").to_string(),
        Phase::Starting => t("Startet …").to_string(),
        Phase::Running => match session.origin {
            Origin::Launched => t("Läuft").to_string(),
            Origin::Attached => t("Angehängt").to_string(),
        },
        Phase::Paused { reason, .. } => tf("Angehalten: {}", &[&reason_words(reason)]),
        Phase::Ended { code: Some(code), .. } => tf("Beendet mit Code {}", &[&code.to_string()]),
        Phase::Ended {
            problem: Some(problem), ..
        } => tf("Beendet: {}", &[problem]),
        Phase::Ended { .. } => t("Beendet").to_string(),
    }
}

/// Why a session stopped, in words, from the reason the protocol names.
fn reason_words(reason: &str) -> String {
    match reason {
        "breakpoint" | "function breakpoint" | "data breakpoint" | "instruction breakpoint" => t("Haltepunkt").into(),
        "step" => t("Schritt").into(),
        "exception" => t("Ausnahme").into(),
        "pause" => t("Pause").into(),
        "entry" => t("Programmstart").into(),
        "goto" => t("Sprung").into(),
        other => other.to_string(),
    }
}

/// A place in words, such as `main.rs:12`, its line counted from 1.
pub(crate) fn place(path: &std::path::Path, line: usize) -> String {
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    format!("{name}:{}", line + 1)
}

/// A colour a row's symbol is tinted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Tint {
    Accent,
    Green,
    Orange,
    Secondary,
    Tertiary,
}

impl Tint {
    fn color(self) -> Retained<NSColor> {
        match self {
            Tint::Accent => NSColor::controlAccentColor(),
            Tint::Green => NSColor::systemGreenColor(),
            Tint::Orange => NSColor::systemOrangeColor(),
            Tint::Secondary => NSColor::secondaryLabelColor(),
            Tint::Tertiary => NSColor::tertiaryLabelColor(),
        }
    }
}

/// What a row's checkbox switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Switched {
    Filter,
    Breakpoint,
}

/// What the cell of a row shows, from which it is built.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Shown {
    Section(&'static str),
    Note(&'static str),
    TwoLines((&'static str, Tint), String, String),
    Line(Option<(&'static str, Tint)>, String, String),
    /// A checkbox with its title, whether it is on, its place among its kind, its action and, for a breakpoint the
    /// adapter could not set, why.
    Switch {
        title: String,
        on: bool,
        index: usize,
        action: Switched,
        unset: Option<String>,
    },
}

impl DebugPanel {
    pub(super) fn row_view(&self, view: &NSOutlineView, item: &AnyObject) -> Option<Retained<NSView>> {
        if !self.is_built() {
            return None;
        }
        let key = super::panel::row_key(item)?;
        let inspector = std::ptr::eq(view, &*self.ui().inspector.outline);
        let shown = match key {
            RowKey::Inspector(row) if inspector => self.inspector_row(row)?,
            RowKey::Panel(key) if !inspector => self.panel_row(key)?,
            _ => return None,
        };
        self.ivars().cells.cell(key, &shown, || Some(self.build(&shown)))
    }

    fn build(&self, shown: &Shown) -> Retained<NSView> {
        let mtm = self.mtm();
        match shown {
            Shown::Section(title) => ide_ui_common::panel::section_cell(mtm, title).into_super(),
            Shown::Note(text) => ui::note(mtm, text),
            Shown::TwoLines((symbol, tint), title, detail) => {
                ui::two_lines(mtm, (symbol, &tint.color()), title, detail)
            }
            Shown::Line(symbol, title, detail) => {
                let symbol = symbol.map(|(name, tint)| (name, tint.color()));
                ui::row(
                    mtm,
                    symbol.as_ref().map(|(name, color)| (*name, &**color)),
                    title,
                    detail,
                )
            }
            Shown::Switch {
                title,
                on,
                index,
                action,
                unset,
            } => {
                let action = match action {
                    Switched::Filter => sel!(ideDebugFilter:),
                    Switched::Breakpoint => sel!(ideDebugBreakpointSwitch:),
                };
                let view = switch(self, title, *on, *index, action);
                if let Some(tip) = unset {
                    view.setToolTip(Some(&ns(tip)));
                    if let Some(button) = view.downcast_ref::<NSButton>() {
                        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
                    }
                }
                view
            }
        }
    }

    fn panel_row(&self, key: &Key) -> Option<Shown> {
        let model = self.read();
        Some(match key {
            Key::Section(Section::Sessions) => Shown::Section(t("Sitzungen")),
            Key::Section(Section::Breakpoints) => Shown::Section(t("Haltepunkte")),
            Key::Empty(Section::Sessions) => Shown::Note(t("Keine Debug-Sitzung")),
            Key::Empty(Section::Breakpoints) => {
                Shown::Note(t("Klicke neben eine Zeilennummer, um einen Haltepunkt zu setzen."))
            }
            Key::Session(id) => {
                let session = model.session(*id)?;
                let tint = if session.is_paused() {
                    Tint::Orange
                } else if session.is_over() {
                    Tint::Tertiary
                } else {
                    Tint::Accent
                };
                Shown::TwoLines(("ladybug", tint), session.name.clone(), phase_words(session))
            }
            Key::Thread(id, thread) => {
                let session = model.session(*id)?;
                let known = session.thread(*thread)?;
                let detail = if session.paused_thread() == Some(*thread) {
                    t("Angehalten")
                } else {
                    ""
                };
                Shown::Line(
                    Some(("square.stack.3d.down.right", Tint::Secondary)),
                    known.name.clone(),
                    detail.to_string(),
                )
            }
            Key::Frame(id, thread, index) => {
                let session = model.session(*id)?;
                let frame = session.thread(*thread)?.frames.as_ref()?.get(*index)?;
                let detail = frame.path.as_deref().map(|p| place(p, frame.line)).unwrap_or_default();
                let paused = session.paused_thread() == Some(*thread) && *index == 0;
                // The innermost frame of the stopped thread carries the arrow the editor shows, not only a colour.
                let symbol = if paused {
                    ("arrowtriangle.right.fill", Tint::Green)
                } else {
                    ("function", Tint::Secondary)
                };
                Shown::Line(Some(symbol), frame.name.clone(), detail)
            }
            Key::Filter(id) => {
                let index = model.filters.iter().position(|f| &f.id == id)?;
                let filter = &model.filters[index];
                Shown::Switch {
                    title: filter.label.clone(),
                    on: filter.enabled,
                    index,
                    action: Switched::Filter,
                    unset: None,
                }
            }
            Key::Breakpoint(path, line) => breakpoint_row(&model, path, *line)?,
        })
    }

    fn inspector_row(&self, row: &Row) -> Option<Shown> {
        let model = self.read();
        let variables = &model.variables;
        Some(match row {
            Row::Part(Part::Variables) => Shown::Section(t("Variablen")),
            Row::Part(Part::Watches) => Shown::Section(t("Überwachte Ausdrücke")),
            Row::Empty(Part::Variables) => Shown::Note(if variables.frame.is_some() {
                t("Die Variablen werden gelesen …")
            } else {
                t("Variablen zeigen sich, wenn das Programm anhält.")
            }),
            Row::Empty(Part::Watches) => Shown::Note(t("Keine überwachten Ausdrücke")),
            Row::Node(Path::Watch(index, names)) if names.is_empty() => {
                let watch = variables.watches.get(*index)?;
                let value = match &watch.value {
                    None => "–".to_string(),
                    Some(Ok(node)) => node.value.clone(),
                    Some(Err(error)) => error.clone(),
                };
                Shown::Line(Some(("eye", Tint::Secondary)), watch.expression.clone(), value)
            }
            Row::Node(path) => {
                let node = variables.node(path)?;
                let detail = match &node.kind {
                    Some(kind) if !node.value.is_empty() => format!("{} ({kind})", node.value),
                    _ => node.value.clone(),
                };
                Shown::Line(None, node.name.clone(), detail)
            }
        })
    }

    /// The user chose a row of the panel: a thread or frame shows its place and its variables.
    pub(super) fn selection_changed_in(&self, notification: &NSNotification) {
        let Some(view) = notification.object().and_then(|o| o.downcast::<NSOutlineView>().ok()) else {
            return;
        };
        if !self.is_built() {
            return;
        }
        if !std::ptr::eq(&*view, &*self.ui().outline) || self.ivars().unfolding.get() {
            return;
        }
        let item = view.itemAtRow(view.selectedRow());
        let effects = match item.as_deref().and_then(key_of) {
            Some(Key::Frame(id, thread, index)) => self.model().select(id, thread, index),
            Some(Key::Thread(id, thread)) => self.model().select(id, thread, 0),
            _ => return,
        };
        self.apply(effects);
    }

    /// The user opened or closed a row: sections remember it, the inspector's rows ask for their children.
    pub(super) fn note_expansion(&self, notification: &NSNotification, open: bool) {
        if self.ivars().unfolding.get() || !self.is_built() {
            return;
        }
        let Some(item) = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")))
        else {
            return;
        };
        let effects = if let Some(Key::Section(section)) = key_of(&item) {
            self.model().fold(section.name(), !open)
        } else if let Some(Row::Node(path)) = row_of(&item).filter(|_| open) {
            self.model().open(&path)
        } else {
            return;
        };
        self.apply(effects);
    }

    /// Return or a double click on row `row` of the panel's list.
    pub(super) fn activate_row(&self, row: isize) {
        let outline = self.ui().outline.clone();
        self.activate_in(&outline, row);
    }

    /// Return on row `row` of `view`: a frame shows its place, a breakpoint its line, other rows open or close.
    pub(crate) fn activate_in(&self, view: &NSOutlineView, row: isize) {
        let Some(item) = (row >= 0).then(|| view.itemAtRow(row)).flatten() else {
            return;
        };
        let effects = match key_of(&item) {
            Some(Key::Frame(id, thread, index)) => self.model().select(id, thread, index),
            Some(Key::Breakpoint(path, line)) => vec![super::model::Effect::Reveal { path, line }],
            _ => {
                // SAFETY: The item came from this outline view, which knows it.
                unsafe {
                    if view.isItemExpanded(Some(&item)) {
                        view.collapseItem(Some(&item));
                    } else {
                        view.expandItem(Some(&item));
                    }
                }
                return;
            }
        };
        self.apply(effects);
    }

    /// Delete on row `row` of `view`: removes a breakpoint or a watch expression; whether it did.
    pub(crate) fn delete_in(&self, view: &NSOutlineView, row: isize) -> bool {
        let Some(item) = (row >= 0).then(|| view.itemAtRow(row)).flatten() else {
            return false;
        };
        if let Some(Key::Breakpoint(path, line)) = key_of(&item) {
            self.toggle_breakpoint(&path, line);
            return true;
        }
        if let Some(Row::Node(Path::Watch(index, names))) = row_of(&item)
            && names.is_empty()
        {
            let effects = self.model().remove_watch(index);
            self.apply(effects);
            return true;
        }
        false
    }

    /// Fills the context menu of the row under the click: a breakpoint's condition, hit count, log message and
    /// removal, a watch expression's removal.
    pub(super) fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let mtm = self.mtm();
        let ui = self.ui();
        for view in [&ui.outline, &ui.inspector.outline] {
            let row = view.clickedRow();
            let Some(item) = (row >= 0).then(|| view.itemAtRow(row)).flatten() else {
                continue;
            };
            if !menu_belongs_to(menu, view) {
                continue;
            }
            if let Some(Key::Breakpoint(path, line)) = key_of(&item) {
                for entry in super::gutter::breakpoint_items(self, &path, line) {
                    menu.addItem(&entry);
                }
            }
            if let Some(Row::Node(Path::Watch(index, names))) = row_of(&item)
                && names.is_empty()
            {
                let entry =
                    ide_ui_common::util::item(mtm, t("Ausdruck entfernen"), Some(sel!(ideDebugRemoveWatch:)), "");
                entry.setTag(isize::try_from(index).unwrap_or(-1));
                set_target(&entry, self);
                menu.addItem(&entry);
            }
        }
    }
}

/// Whether `menu` is the context menu of `view`.
fn menu_belongs_to(menu: &NSMenu, view: &NSOutlineView) -> bool {
    view.menu().is_some_and(|own| std::ptr::eq(&*own, menu))
}

/// What the row of the breakpoint at `line` of `path` shows: a checkbox with its place and what it does.
fn breakpoint_row(model: &super::model::Model, path: &std::path::Path, line: usize) -> Option<Shown> {
    let breakpoint = model.breakpoints.at(path, line)?;
    let index = model.breakpoints.all().iter().position(|b| b == breakpoint)?;
    let mut title = place(path, line);
    let detail = breakpoint
        .log_message
        .as_deref()
        .map(|m| tf("Log: {}", &[m]))
        .or_else(|| breakpoint.condition.as_deref().map(|c| tf("Wenn {}", &[c])))
        .or_else(|| breakpoint.hit_condition.as_deref().map(|h| tf("Treffer {}", &[h])));
    if let Some(detail) = detail {
        title = format!("{title} · {detail}");
    }
    let unverified = model.breakpoints.look(path, line) == Some(Look::Unverified);
    let unset = unverified.then(|| {
        let why = model.breakpoints.check(path, line).and_then(|c| c.message.clone());
        why.map_or_else(
            || t("Haltepunkt, nicht gesetzt").to_string(),
            |why| tf("Der Debugger konnte hier keinen Haltepunkt setzen: {}", &[&why]),
        )
    });
    if unverified {
        title = format!("{title} · {}", t("nicht gesetzt"));
    }
    Some(Shown::Switch {
        title,
        on: breakpoint.enabled,
        index,
        action: Switched::Breakpoint,
        unset,
    })
}

/// A checkbox titled `title` for an exception filter or a breakpoint, tagged with its place.
fn switch(panel: &DebugPanel, title: &str, on: bool, index: usize, action: objc2::runtime::Sel) -> Retained<NSView> {
    let mtm = panel.mtm();
    // SAFETY: A nil target sends the action along the responder chain; `set_action` aims it at the panel.
    let button = unsafe { NSButton::checkboxWithTitle_target_action(&ns(title), None, Some(action), mtm) };
    ide_ui_common::util::set_action(&button, panel, action);
    button.setTag(isize::try_from(index).unwrap_or(-1));
    button.setState(if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    ide_ui_common::ui_size::follow_control_size(&button, objc2_app_kit::NSControlSize::Regular);
    ide_ui_common::ui_size::set_font(&button, &ide_ui_common::style::small_system());
    button.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingMiddle);
    button.into_super().into_super()
}

/// How a breakpoint looks, said for VoiceOver.
pub(crate) fn look_words(look: Look) -> &'static str {
    match look {
        Look::Enabled => t("Haltepunkt"),
        Look::Disabled => t("Haltepunkt, ausgeschaltet"),
        Look::Log => t("Logpunkt"),
        Look::Conditional => t("Haltepunkt mit Bedingung"),
        Look::Unverified => t("Haltepunkt, nicht gesetzt"),
    }
}

/// The place among its kind of the row a menu item or control acts on, from its tag.
pub(crate) fn tag_of(sender: Option<&AnyObject>) -> Option<usize> {
    let sender = sender?;
    let tag = if let Some(item) = sender.downcast_ref::<NSMenuItem>() {
        item.tag()
    } else {
        sender.downcast_ref::<objc2_app_kit::NSControl>()?.tag()
    };
    usize::try_from(tag).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(phase: Phase, origin: Origin, waiting: bool) -> Session {
        Session {
            phase,
            origin,
            waiting,
            ..Session::new(1, "app", Origin::Launched)
        }
    }

    #[test]
    fn a_session_says_how_it_stands_and_why_it_stopped() {
        let cases = [
            (
                Phase::Starting,
                Origin::Launched,
                true,
                "Wartet auf Freigabe durch macOS …",
            ),
            (Phase::Starting, Origin::Launched, false, "Startet …"),
            (Phase::Running, Origin::Launched, false, "Läuft"),
            (Phase::Running, Origin::Attached, false, "Angehängt"),
            (
                Phase::Paused {
                    thread: 1,
                    reason: "function breakpoint".into(),
                },
                Origin::Launched,
                false,
                "Angehalten: Haltepunkt",
            ),
            (
                Phase::Paused {
                    thread: 1,
                    reason: "goto".into(),
                },
                Origin::Launched,
                false,
                "Angehalten: Sprung",
            ),
            (
                Phase::Paused {
                    thread: 1,
                    reason: "signal".into(),
                },
                Origin::Launched,
                false,
                "Angehalten: signal",
            ),
            (
                Phase::Ended {
                    code: Some(3),
                    problem: None,
                },
                Origin::Launched,
                false,
                "Beendet mit Code 3",
            ),
            (
                Phase::Ended {
                    code: None,
                    problem: Some("lldb-dap fehlt.".into()),
                },
                Origin::Launched,
                false,
                "Beendet: lldb-dap fehlt.",
            ),
            (
                Phase::Ended {
                    code: None,
                    problem: None,
                },
                Origin::Attached,
                false,
                "Beendet",
            ),
        ];
        for (phase, origin, waiting, words) in cases {
            assert_eq!(phase_words(&session(phase, origin, waiting)), words);
        }
        let reasons: Vec<String> = ["step", "exception", "pause", "entry", "data breakpoint"]
            .into_iter()
            .map(reason_words)
            .collect();
        assert_eq!(reasons, ["Schritt", "Ausnahme", "Pause", "Programmstart", "Haltepunkt"]);
    }

    #[test]
    fn a_place_counts_its_line_from_one_and_a_breakpoint_says_how_it_looks() {
        assert_eq!(place(std::path::Path::new("/p/src/main.rs"), 11), "main.rs:12");
        assert_eq!(place(std::path::Path::new("/"), 0), "/:1");
        let looks = [
            Look::Enabled,
            Look::Disabled,
            Look::Log,
            Look::Conditional,
            Look::Unverified,
        ]
        .map(look_words);
        assert_eq!(
            looks,
            [
                "Haltepunkt",
                "Haltepunkt, ausgeschaltet",
                "Logpunkt",
                "Haltepunkt mit Bedingung",
                "Haltepunkt, nicht gesetzt"
            ]
        );
    }
}
