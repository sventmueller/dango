//! The Agents tab of the settings (E5, E58): every agent a thread can run and whether this Mac has it — Claude
//! through its own program, the own agent, the ACP presets with their adapters, agents installed from the registry —
//! with the ways to install, update or go back; and the ACP registry, fetched only when the user asks, in a window of
//! its own with each entry installable on the user's word.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ide_acp::install::{Plan, Refusal};
use ide_acp::launch::Origin;
use ide_acp::register::{self, Distribution, PRESETS, Preset, Register};
use ide_acp::{Installer, Installs};
use ide_net::Http;
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControlSize, NSLayoutAttribute, NSPanel,
    NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSStackView, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::agent::AcpAgent;
use crate::util::{guarded, main_soon, ns, set_action};

use super::form::{self, MARGIN, WIDTH};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// The tallest the groups get before they scroll.
const MAX_HEIGHT: f64 = 560.0;

/// The room inside a group's line, at its sides and above and below it.
const PADDING: f64 = 12.0;
const INSET: f64 = 9.0;

/// The width of the dot before an agent's name.
const MARK: f64 = 16.0;

/// The agent registry window, what its installations have got to and what it lists.
#[derive(Default)]
pub struct AgentRegistryWindow {
    /// What each installation running now has got to, by the agent's id; the registry's fetch under the empty id.
    progress: RefCell<HashMap<String, String>>,
    /// The registry window, kept while the app runs.
    window: RefCell<Option<Retained<NSPanel>>>,
    /// The registry's ids in the window's order, which its buttons' tags count.
    shown: RefCell<Vec<String>>,
}

fn with_progress<R>(f: impl FnOnce(&RefCell<HashMap<String, String>>) -> R) -> R {
    f(&crate::context::app().settings.agent_registry.progress)
}

fn with_window<R>(f: impl FnOnce(&RefCell<Option<Retained<NSPanel>>>) -> R) -> R {
    f(&crate::context::app().settings.agent_registry.window)
}

fn with_shown<R>(f: impl FnOnce(&RefCell<Vec<String>>) -> R) -> R {
    f(&crate::context::app().settings.agent_registry.shown)
}

/// A button of a line: its title, its action and its tag.
type Action = (String, Sel, isize);

/// How far an agent is from starting, as the dot before its name shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Ready,
    /// Part of it is there, such as an agent's program without its adapter.
    Partial,
    Absent,
    /// Something runs that changes the state: an installation, or the login shell still being read.
    Busy,
    /// No state of its own; the dot's room stays so the names line up.
    Blank,
}

pub struct AgentsIvars {
    /// The agents' lines, and the registry's line in a group of its own.
    agents: OnceCell<Retained<NSStackView>>,
    register: OnceCell<Retained<NSStackView>>,
    /// Whether a thread waits for the login shell's environment, which the states need.
    waiting: Cell<bool>,
    rules: Retained<super::stored_rules::StoredRules>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `AgentsSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentsSettings"]
    #[ivars = AgentsIvars]
    pub struct AgentsSettings;

    unsafe impl NSObjectProtocol for AgentsSettings {}

    impl AgentsSettings {
        #[unsafe(method(ideAgentsInstall:))]
        fn install_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(preset) = usize::try_from(tag(sender)).ok().and_then(|n| PRESETS.get(n)) {
                    install(self.mtm(), preset.id, preset.name, Plan::of_preset(preset));
                }
            });
        }

        #[unsafe(method(ideAgentsRollback:))]
        fn rollback_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(preset) = usize::try_from(tag(sender)).ok().and_then(|n| PRESETS.get(n)) {
                    if let Err(error) = installs().rollback(preset.id) {
                        alert(self.mtm(), &tf("{} konnte nicht zurückgesetzt werden", &[preset.name]), &error.to_string());
                    }
                    super::agents_changed();
                }
            });
        }

        #[unsafe(method(ideAgentsFetch:))]
        fn fetch_action(&self, _sender: Option<&AnyObject>) {
            guarded((), fetch);
        }

        #[unsafe(method(ideAgentsShowRegister:))]
        fn show_register_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || show_register(self));
        }

        #[unsafe(method(ideAgentsInstallEntry:))]
        fn install_entry_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let id = with_shown(|s| usize::try_from(tag(sender)).ok().and_then(|n| s.borrow().get(n).cloned()));
                let entry = id.and_then(|id| crate::agent::cached_register().and_then(|(r, _)| r.entry(&id).cloned()));
                if let Some(entry) = entry
                    && let Ok(plan) = Plan::of_entry(&entry)
                {
                    install(self.mtm(), &entry.id, &entry.name, plan);
                }
            });
        }

        #[unsafe(method(ideAgentsOwnSettings:))]
        fn own_settings_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::show_agent(self.mtm()));
        }
    }
);

fn tag(sender: Option<&AnyObject>) -> isize {
    // SAFETY: Every sender of these actions is a control, which answers `tag`.
    sender.map_or(-1, |s| unsafe { msg_send![s, tag] })
}

fn installs() -> Installs {
    Installs::new(Installs::default_root())
}

fn alert(mtm: MainThreadMarker, title: &str, detail: &str) {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(title));
    alert.setInformativeText(&ns(detail));
    alert.runModal();
}

fn link(mtm: MainThreadMarker, title: &str, target: &AgentsSettings, action: Sel, tag: isize) -> Retained<NSButton> {
    let button = crate::util::borderless_button(mtm, title, None, action);
    set_action(&button, target, action);
    button.setTag(tag);
    button.setFont(Some(&crate::agent::style::small()));
    button.setContentTintColor(Some(&NSColor::linkColor()));
    button
}

/// A push button of a line.
fn push(mtm: MainThreadMarker, title: &str, target: &AgentsSettings, action: Sel, tag: isize) -> Retained<NSButton> {
    // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, action);
    button.setTag(tag);
    button.setBezelStyle(NSBezelStyle::Push);
    button
}

/// The dot before an agent's name, or the spinner while something runs.
fn mark_view(mtm: MainThreadMarker, mark: Mark) -> Retained<NSView> {
    let view: Retained<NSView> = if mark == Mark::Busy {
        let spinner = NSProgressIndicator::new(mtm);
        spinner.setStyle(NSProgressIndicatorStyle::Spinning);
        spinner.setControlSize(NSControlSize::Small);
        spinner.setDisplayedWhenStopped(false);
        // SAFETY: A nil sender is what AppKit's own callers pass.
        unsafe { spinner.startAnimation(None) };
        Retained::into_super(spinner)
    } else {
        let color = match mark {
            Mark::Ready => NSColor::systemGreenColor(),
            Mark::Partial => NSColor::systemOrangeColor(),
            Mark::Absent => NSColor::tertiaryLabelColor(),
            Mark::Busy | Mark::Blank => NSColor::clearColor(),
        };
        let dot = crate::agent::icon(mtm, "circle.fill", "", 8.0, &color);
        // The state beside it says in words what the dot shows.
        crate::util::hide_from_voiceover(&dot);
        Retained::into_super(Retained::into_super(dot))
    };
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.widthAnchor().constraintEqualToConstant(MARK).setActive(true);
    view.heightAnchor().constraintEqualToConstant(MARK).setActive(true);
    view
}

/// Installs `plan` after the user agreed, showing how far it got in the tab.
fn install(mtm: MainThreadMarker, id: &str, name: &str, plan: Plan) {
    if with_progress(|p| p.borrow().contains_key(id)) || !crate::agent::confirm_install(mtm, name, &plan) {
        return;
    }
    let (id, name) = (id.to_string(), name.to_string());
    with_progress(|p| p.borrow_mut().insert(id.clone(), tf("{} wird installiert …", &[&name])));
    super::agents_changed();
    let failed = (id.clone(), name.clone());
    if let Err(why) = crate::util::spawn("acp-install", move || {
        let env = crate::environment::wait(Duration::from_secs(15))
            .map(<[(String, String)]>::to_vec)
            .unwrap_or_default();
        let installer = Installer::new(
            Supervisor::global(),
            env,
            installs(),
            Arc::new(Http::new("agent register")),
        );
        let result = installer.install(&plan, true, &|_| {});
        main_soon(move || {
            with_progress(|p| p.borrow_mut().remove(&id));
            if let Err(error) = result
                && let Some(mtm) = MainThreadMarker::new()
            {
                alert(mtm, &tf("{} wurde nicht installiert", &[&name]), &error.to_string());
            }
            super::agents_changed();
            refresh_register();
        });
    }) {
        let (id, name) = failed;
        with_progress(|p| p.borrow_mut().remove(&id));
        alert(mtm, &tf("{} wurde nicht installiert", &[&name]), &why);
        super::agents_changed();
    }
}

/// Fetches the registry on the user's request and keeps it.
fn fetch() {
    with_progress(|p| {
        p.borrow_mut()
            .insert(String::new(), t("Register wird geladen …").to_string())
    });
    super::agents_changed();
    if let Err(why) = crate::util::spawn("acp-register", || {
        let result = Register::fetch(&Http::new("agent register")).map(|r| r.entries.len());
        main_soon(move || {
            with_progress(|p| p.borrow_mut().remove(""));
            if let Err(error) = result
                && let Some(mtm) = MainThreadMarker::new()
            {
                alert(mtm, t("Das Register wurde nicht geladen"), &error.to_string());
            }
            super::agents_changed();
            refresh_register();
        });
    }) {
        with_progress(|p| p.borrow_mut().remove(""));
        if let Some(mtm) = MainThreadMarker::new() {
            alert(mtm, t("Das Register wurde nicht geladen"), &why);
        }
        super::agents_changed();
    }
}

/// What the tab says about `preset`, at `index` of the presets, and what it offers. An adapter whose agent's own
/// program is there says so, and Claude over ACP, only a fallback for Claude's own connection, never reads as Claude
/// missing.
fn preset_state(preset: &Preset, index: isize, env: &[(String, String)]) -> (Mark, String, Vec<Action>) {
    if let Some(progress) = with_progress(|p| p.borrow().get(preset.id).cloned()) {
        return (Mark::Busy, progress, Vec::new());
    }
    let launch = crate::agent::launch_of(&AcpAgent::of_preset(preset), env, std::path::Path::new("/"));
    let installs = installs();
    let mut actions = Vec::new();
    let mut mark = Mark::Ready;
    let state = match launch.map(|l| l.origin) {
        Some(Origin::OnPath) => tf("Bereit · {} im PATH", &[preset.on_path.map_or("", |(name, _)| name)]),
        Some(Origin::Installed { version, .. }) => {
            let pinned = preset.pinned.version();
            if version != pinned {
                actions.push((tf("Auf {} aktualisieren", &[pinned]), sel!(ideAgentsInstall:), index));
            }
            if let Some(previous) = installs.previous(preset.id) {
                actions.push((
                    tf("Zurück zu {}", &[&previous.version]),
                    sel!(ideAgentsRollback:),
                    index,
                ));
            }
            tf("Bereit · Version {}", &[&version])
        }
        Some(_) => t("Bereit").to_string(),
        None => {
            let claude = preset.id == "claude-acp" && crate::agent::claude_program(env).is_some();
            let cli = preset.cli_on_path(env).is_some();
            let (state, button) = missing(preset, cli, claude);
            mark = if cli && !claude && preset.cli.is_some() {
                Mark::Partial
            } else {
                Mark::Absent
            };
            actions.push((button.to_string(), sel!(ideAgentsInstall:), index));
            state
        }
    };
    (mark, state, actions)
}

/// What the tab says about `preset` when it does not start, and the title of its install button: Claude over ACP
/// as the fallback Claude's own program makes unnecessary (`claude`), an adapter whose agent's program is there
/// (`cli`), or nothing of it.
fn missing(preset: &Preset, cli: bool, claude: bool) -> (String, &'static str) {
    if claude {
        return (
            t("Nicht nötig, Claude läuft über sein eigenes Programm").to_string(),
            t("Installieren …"),
        );
    }
    match preset.cli.filter(|_| cli) {
        Some(program) => (
            tf("{} vorhanden, Adapter fehlt", &[program]),
            t("Adapter installieren …"),
        ),
        None => (t("Nicht vorhanden").to_string(), t("Installieren …")),
    }
}

/// What the tab says about Claude's own connection (E21).
fn claude_state(env: &[(String, String)]) -> (Mark, String) {
    match crate::agent::claude_program(env) {
        Some(path) => (Mark::Ready, tf("Bereit · {}", &[&crate::app::display_path(&path)])),
        None => (
            Mark::Absent,
            t("Nicht gefunden: claude im Terminal installieren und dort anmelden").to_string(),
        ),
    }
}

/// What the tab says about the registry, and what it offers.
fn register_state() -> (String, Vec<Action>) {
    match (
        with_progress(|p| p.borrow().get("").cloned()),
        crate::agent::cached_register(),
    ) {
        (Some(progress), _) => (progress, Vec::new()),
        (None, Some((register, at))) => (
            tf(
                "{} Agenten, Stand {}",
                &[&register.entries.len().to_string(), &crate::agent::age(at)],
            ),
            vec![
                (t("Anzeigen …").to_string(), sel!(ideAgentsShowRegister:), 0),
                (t("Neu laden").to_string(), sel!(ideAgentsFetch:), 0),
            ],
        ),
        (None, None) => (
            t("Noch nicht geladen").to_string(),
            vec![(t("Register laden").to_string(), sel!(ideAgentsFetch:), 0)],
        ),
    }
}

/// The agents installed from the registry beyond the presets, with their versions.
fn registry_installed() -> Vec<(String, String)> {
    let Some((register, _)) = crate::agent::cached_register() else {
        return Vec::new();
    };
    let installs = installs();
    register
        .entries
        .iter()
        .filter(|e| register::preset(&e.id).is_none())
        .filter_map(|e| {
            let (installed, _) = installs.current(&e.id)?;
            Some((e.name.clone(), tf("Bereit · Version {}", &[&installed.version])))
        })
        .collect()
}

impl AgentsSettings {
    pub fn new(mtm: MainThreadMarker) -> Retained<AgentsSettings> {
        let this = Self::alloc(mtm).set_ivars(AgentsIvars {
            agents: OnceCell::new(),
            register: OnceCell::new(),
            waiting: Cell::new(false),
            rules: super::stored_rules::StoredRules::new(mtm),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The tab: what it is about, the agents in a group, the registry in another; as tall as they are, scrolling only
    /// past `MAX_HEIGHT`.
    pub fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let agents = self.ivars().agents.get_or_init(|| group_lines(mtm));
        let register = self.ivars().register.get_or_init(|| group_lines(mtm));
        self.reload();
        let mut views = vec![
            form::heading(mtm, t("Agenten")),
            form::note(
                mtm,
                t(
                    "Die Agenten, mit denen du AI-Threads startest. Bereit ist ein Agent, dessen Programm im PATH der Login-Shell liegt oder den diese App installiert hat.",
                ),
            ),
        ];
        let groups: Vec<Retained<NSView>> = [agents, register]
            .into_iter()
            .map(|lines| {
                let group = crate::agent::framed(
                    mtm,
                    lines,
                    &NSColor::separatorColor(),
                    Some(&NSColor::quaternarySystemFillColor()),
                    8.0,
                    crate::agent::insets(0.0, 0.0, 0.0, 0.0),
                );
                group.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
                group.into_super()
            })
            .collect();
        views.extend(groups.iter().cloned());
        views.extend(self.ivars().rules.views());
        let content = form::stack(mtm, &views);
        content.setCustomSpacing_afterView(ide_ui_common::style::space::XL, &groups[0]);
        content.setEdgeInsets(crate::agent::insets(MARGIN, MARGIN, MARGIN, MARGIN));
        scrolling(mtm, &content)
    }

    /// Shows every agent's state again, as it is now.
    pub fn reload(&self) {
        self.ivars().rules.reload();
        let (Some(agents), Some(register)) = (self.ivars().agents.get(), self.ivars().register.get()) else {
            return;
        };
        let env = crate::environment::get();
        if env.is_none() {
            self.wait_for_environment();
        }
        let env = env.unwrap_or_default();
        let waiting = || (Mark::Busy, t("Umgebung der Login-Shell wird gelesen …").to_string());
        let (mark, claude) = if env.is_empty() { waiting() } else { claude_state(env) };
        let mut lines = vec![self.line(Some(mark), "Claude", &claude, &[])];
        for (n, preset) in PRESETS.iter().enumerate().filter(|(_, preset)| preset.offered) {
            let (mark, state, actions) = if env.is_empty() {
                let (mark, state) = waiting();
                (mark, state, Vec::new())
            } else {
                preset_state(preset, isize::try_from(n).unwrap_or(-1), env)
            };
            let name = if preset.id == "claude-acp" {
                t("Claude über ACP (Ausweichweg)")
            } else {
                preset.name
            };
            lines.push(self.line(Some(mark), name, &state, &actions));
        }
        for (name, state) in registry_installed() {
            lines.push(self.line(Some(Mark::Ready), &name, &state, &[]));
        }
        lines.push(self.line(
            Some(Mark::Blank),
            t("Eigener Agent"),
            t("Anbieter, Modelle und Schlüssel im Tab „Eigener Agent“"),
            &[(t("Öffnen").to_string(), sel!(ideAgentsOwnSettings:), 0)],
        ));
        fill(agents, &lines);
        let (state, actions) = register_state();
        fill(register, &[self.line(None, t("ACP-Register"), &state, &actions)]);
    }

    /// Shows the states again once the login shell's environment is there, which a fresh start still reads.
    fn wait_for_environment(&self) {
        if self.ivars().waiting.replace(true) {
            return;
        }
        if let Err(why) = crate::util::spawn("agents-environment", || {
            let _ = crate::environment::wait(Duration::from_secs(30));
            main_soon(super::agents_changed);
        }) {
            self.ivars().waiting.set(false);
            crate::diag::report("waiting for the login shell's environment", why);
        }
    }

    /// A line of a group: the dot of `mark` if it has one, the name with its state below it, and its buttons.
    fn line(&self, mark: Option<Mark>, name: &str, state: &str, actions: &[Action]) -> Retained<NSView> {
        let mtm = self.mtm();
        let mut buttons = Vec::new();
        let mut room = WIDTH - 2.0 * PADDING;
        for (title, action, tag) in actions {
            let button = push(mtm, title, self, *action, *tag);
            // The agents' buttons have the same titles; the agent's name tells them apart for VoiceOver.
            crate::util::set_label(&button, Some(&format!("{title}: {name}")));
            room -= button.fittingSize().width + SPACING;
            buttons.push(Retained::into_super(Retained::into_super(button)));
        }
        let mut views = Vec::new();
        if let Some(mark) = mark {
            views.push(mark_view(mtm, mark));
            room -= MARK + SPACING;
        }
        let title = NSTextField::labelWithString(&ns(name), mtm);
        title.setFont(Some(&crate::agent::style::body()));
        let detail = NSTextField::wrappingLabelWithString(&ns(state), mtm);
        detail.setFont(Some(&crate::agent::style::small()));
        detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
        detail.setPreferredMaxLayoutWidth(room - SPACING);
        let text = NSStackView::new(mtm);
        text.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        text.setAlignment(NSLayoutAttribute::Leading);
        text.setSpacing(ide_ui_common::style::space::XXS);
        text.addArrangedSubview(&title);
        text.addArrangedSubview(&detail);
        let text: Retained<NSView> = Retained::into_super(text);
        views.push(text.clone());
        views.push(crate::agent_panel::spacer(mtm));
        views.extend(buttons);
        let line = NSStackView::new(mtm);
        line.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        line.setAlignment(NSLayoutAttribute::CenterY);
        line.setSpacing(SPACING);
        line.setEdgeInsets(crate::agent::insets(INSET, PADDING, INSET, PADDING));
        for view in &views {
            line.addArrangedSubview(view);
        }
        line.setTranslatesAutoresizingMaskIntoConstraints(false);
        // A stack centring its views keeps its insets across them only as loosely as it hugs them, which measuring
        // the tab for the window's height overrides.
        for constraint in [
            line.widthAnchor().constraintEqualToConstant(WIDTH),
            line.heightAnchor()
                .constraintGreaterThanOrEqualToAnchor_constant(&text.heightAnchor(), 2.0 * INSET),
        ] {
            constraint.setActive(true);
        }
        Retained::into_super(line)
    }
}

/// The room between the parts of a line.
const SPACING: f64 = 8.0;

/// The column a group's lines stand in.
fn group_lines(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let lines = NSStackView::new(mtm);
    lines.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    lines.setAlignment(NSLayoutAttribute::CenterX);
    lines.setSpacing(ide_ui_common::style::space::NONE);
    lines
}

/// Puts `lines` into `group` in place of what it showed, with a line between each two.
fn fill(group: &NSStackView, lines: &[Retained<NSView>]) {
    for view in group.arrangedSubviews().iter() {
        group.removeArrangedSubview(&view);
        view.removeFromSuperview();
    }
    for (n, line) in lines.iter().enumerate() {
        if n > 0 {
            let rule = crate::agent::separator(group.mtm());
            group.addArrangedSubview(&rule);
            rule.widthAnchor()
                .constraintEqualToConstant(WIDTH - 2.0 * PADDING)
                .setActive(true);
        }
        group.addArrangedSubview(line);
    }
}

/// `content` in a scroll view that is as tall as it, up to `MAX_HEIGHT`.
fn scrolling(mtm: MainThreadMarker, content: &NSView) -> Retained<NSView> {
    let document = crate::agent_panel::flipped(mtm);
    document.addSubview(content);
    content.setTranslatesAutoresizingMaskIntoConstraints(false);
    let scroll = NSScrollView::new(mtm);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(&document));
    document.setTranslatesAutoresizingMaskIntoConstraints(false);
    let root = NSView::new(mtm);
    root.addSubview(&scroll);
    scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
    let fit = scroll.heightAnchor().constraintEqualToAnchor(&document.heightAnchor());
    // Below the labels' compression resistance (750), so a tab taller than `MAX_HEIGHT` scrolls instead of squeezing.
    fit.setPriority(700.0);
    let mut pins = vec![
        scroll.topAnchor().constraintEqualToAnchor(&root.topAnchor()),
        scroll.leadingAnchor().constraintEqualToAnchor(&root.leadingAnchor()),
        scroll.trailingAnchor().constraintEqualToAnchor(&root.trailingAnchor()),
        scroll.bottomAnchor().constraintEqualToAnchor(&root.bottomAnchor()),
        scroll.widthAnchor().constraintEqualToConstant(WIDTH + 2.0 * MARGIN),
    ];
    // The tab view sets the root's frame, which these give way to instead of clashing with it.
    for pin in &mut pins {
        pin.setPriority(999.0);
    }
    for constraint in pins.iter().chain([
        &fit,
        &scroll.heightAnchor().constraintLessThanOrEqualToConstant(MAX_HEIGHT),
        &content.topAnchor().constraintEqualToAnchor(&document.topAnchor()),
        &content
            .leadingAnchor()
            .constraintEqualToAnchor(&document.leadingAnchor()),
        &content.bottomAnchor().constraintEqualToAnchor(&document.bottomAnchor()),
        &document
            .widthAnchor()
            .constraintEqualToAnchor(&scroll.contentView().widthAnchor()),
    ]) {
        constraint.setActive(true);
    }
    root
}

/// What a registry entry offers this Mac, as the registry window says it.
fn offer_text(entry: &register::Entry) -> (String, bool) {
    let installed = installs()
        .current(&entry.id)
        .map(|(installed, _)| installed.version.clone());
    offer(entry, installed.as_deref())
}

/// What `entry` offers, given the version installed of it if any, and whether it can be installed.
fn offer(entry: &register::Entry, installed: Option<&str>) -> (String, bool) {
    if let Some(version) = installed {
        return (tf("Installiert: Version {}", &[version]), false);
    }
    match Plan::of_entry(entry) {
        Ok(plan) => {
            let how = match entry.distributions.first() {
                Some(Distribution::Binary { sha256: Some(_), .. }) => t("Archiv mit Prüfsumme"),
                Some(Distribution::Binary { sha256: None, .. }) => t("Archiv ohne Prüfsumme"),
                Some(Distribution::Npm(_)) => t("npm-Paket"),
                Some(Distribution::Uvx(_)) => t("Python-Paket über uv"),
                None => "",
            };
            (tf("Version {} · {}", &[&plan.version, how]), true)
        }
        Err(Refusal::Unversioned(package)) => (tf("{} nennt keine feste Version", &[&package]), false),
        Err(Refusal::Nothing { unknown }) => (tf("Nicht für diesen Mac ({})", &[&unknown.join(", ")]), false),
    }
}

fn register_rows(target: &AgentsSettings, register: &Register) -> Retained<NSStackView> {
    let mtm = target.mtm();
    let column = NSStackView::new(mtm);
    column.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    column.setAlignment(NSLayoutAttribute::Leading);
    column.setSpacing(ide_ui_common::style::space::L);
    let mut shown = Vec::new();
    for (n, entry) in register.entries.iter().enumerate() {
        let name = NSTextField::labelWithString(&ns(&entry.name), mtm);
        name.setFont(Some(&crate::agent::style::bold(13.0)));
        let description = NSTextField::wrappingLabelWithString(&ns(&entry.description), mtm);
        description.setFont(Some(&crate::agent::style::small()));
        description.setPreferredMaxLayoutWidth(460.0);
        let (offer, installable) = offer_text(entry);
        let state = crate::agent_panel::small_label(mtm, &offer);
        let install = link(
            mtm,
            t("Installieren …"),
            target,
            sel!(ideAgentsInstallEntry:),
            isize::try_from(n).unwrap_or(-1),
        );
        crate::util::set_label(&install, Some(&format!("{}: {}", t("Installieren …"), entry.name)));
        install.setHidden(!installable);
        install.setEnabled(with_progress(|p| !p.borrow().contains_key(&entry.id)));
        let head = NSStackView::stackViewWithViews(
            &objc2_foundation::NSArray::from_retained_slice(&[
                Retained::into_super(Retained::into_super(name)),
                crate::agent_panel::spacer(mtm),
                Retained::into_super(Retained::into_super(install)),
            ]),
            mtm,
        );
        let block = NSStackView::stackViewWithViews(
            &objc2_foundation::NSArray::from_retained_slice(&[
                Retained::into_super(head),
                Retained::into_super(Retained::into_super(description)),
                Retained::into_super(Retained::into_super(state)),
            ]),
            mtm,
        );
        block.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        block.setAlignment(NSLayoutAttribute::Leading);
        block.setSpacing(ide_ui_common::style::space::XXS);
        column.addArrangedSubview(&block);
        block
            .widthAnchor()
            .constraintEqualToAnchor(&column.widthAnchor())
            .setActive(true);
        if let Some(head) = block.arrangedSubviews().firstObject() {
            head.widthAnchor()
                .constraintEqualToAnchor(&block.widthAnchor())
                .setActive(true);
        }
        shown.push(entry.id.clone());
    }
    with_shown(|s| *s.borrow_mut() = shown);
    column
}

/// Opens the window with the registry's entries.
fn show_register(target: &AgentsSettings) {
    let mtm = target.mtm();
    let Some((register, _)) = crate::agent::cached_register() else {
        return;
    };
    let panel = with_window(|w| {
        w.borrow_mut()
            .get_or_insert_with(|| {
                let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
                    NSPanel::alloc(mtm),
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(520.0, 560.0)),
                    NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable,
                    NSBackingStoreType::Buffered,
                    false,
                );
                // SAFETY: Switching it off keeps AppKit from releasing the panel behind Rust's back; the thread-local
                // owns it.
                unsafe { panel.setReleasedWhenClosed(false) };
                panel.setTitle(&ns(t("ACP-Register")));
                panel.center();
                panel
            })
            .clone()
    });
    let rows = register_rows(target, &register);
    let document = crate::agent_panel::flipped(mtm);
    document.addSubview(&rows);
    rows.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        rows.topAnchor()
            .constraintEqualToAnchor_constant(&document.topAnchor(), 14.0),
        rows.leadingAnchor()
            .constraintEqualToAnchor_constant(&document.leadingAnchor(), 16.0),
        rows.trailingAnchor()
            .constraintEqualToAnchor_constant(&document.trailingAnchor(), -16.0),
        rows.bottomAnchor()
            .constraintEqualToAnchor_constant(&document.bottomAnchor(), -14.0),
    ] {
        constraint.setActive(true);
    }
    let scroll = NSScrollView::new(mtm);
    scroll.setHasVerticalScroller(true);
    scroll.setDocumentView(Some(&document));
    document.setTranslatesAutoresizingMaskIntoConstraints(false);
    document
        .widthAnchor()
        .constraintEqualToAnchor(&scroll.contentView().widthAnchor())
        .setActive(true);
    panel.setContentView(Some(&scroll));
    panel.makeKeyAndOrderFront(None);
}

/// Redraws the registry window, if it is open, after an installation or a new fetch.
fn refresh_register() {
    let open = with_window(|w| w.borrow().as_ref().is_some_and(|p| p.isVisible()));
    if open && let Some(target) = super::agents_tab_now() {
        show_register(&target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_adapter_and_the_claude_fallback_do_not_read_as_a_missing_agent() {
        let codex = register::preset("codex-acp").unwrap();
        let (state, button) = missing(codex, true, false);
        assert_eq!(state, "codex vorhanden, Adapter fehlt");
        assert_eq!(button, "Adapter installieren …");
        assert_eq!(missing(codex, false, false).0, "Nicht vorhanden");
        let fallback = register::preset("claude-acp").unwrap();
        assert!(missing(fallback, true, true).0.starts_with("Nicht nötig"));
        assert_eq!(
            missing(register::preset("gemini").unwrap(), true, false).0,
            "Nicht vorhanden"
        );
    }

    fn binary(sha256: Option<&str>) -> Distribution {
        Distribution::Binary {
            archive: "https://example.invalid/a.tar.gz".into(),
            cmd: "./agent".into(),
            args: Vec::new(),
            env: Vec::new(),
            sha256: sha256.map(str::to_string),
        }
    }

    fn entry(distributions: Vec<Distribution>, unknown: &[&str]) -> register::Entry {
        register::Entry {
            id: "agent".into(),
            name: "Agent".into(),
            version: "1.2.0".into(),
            distributions,
            unknown: unknown.iter().map(|u| u.to_string()).collect(),
            ..register::Entry::default()
        }
    }

    #[test]
    fn an_installed_agent_is_not_offered_again() {
        let offered = offer(&entry(vec![binary(Some("ab"))], &[]), Some("1.1.0"));
        assert_eq!(offered, ("Installiert: Version 1.1.0".to_string(), false));
    }

    #[test]
    fn the_offer_says_whether_the_archive_is_checked() {
        assert_eq!(
            offer(&entry(vec![binary(Some("ab"))], &[]), None),
            ("Version 1.2.0 · Archiv mit Prüfsumme".to_string(), true)
        );
        assert_eq!(
            offer(&entry(vec![binary(None)], &[]), None).0,
            "Version 1.2.0 · Archiv ohne Prüfsumme"
        );
    }

    #[test]
    fn an_agent_without_a_way_onto_this_mac_names_the_ways_it_has() {
        let (text, installable) = offer(&entry(Vec::new(), &["windows"]), None);
        assert!(!installable);
        assert_eq!(text, "Nicht für diesen Mac (windows)");
    }
}
