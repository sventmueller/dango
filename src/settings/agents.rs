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
use ide_acp::net::Http;
use ide_acp::register::{self, Distribution, PRESETS, Preset, Register};
use ide_acp::{Installer, Installs};
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSBackingStoreType, NSButton, NSColor, NSLayoutAttribute, NSPanel, NSScrollView, NSStackView, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::agent::AcpAgent;
use crate::util::{guarded, main_soon, ns, set_action};

/// The width of the tab's list.
const WIDTH: f64 = 600.0;

/// The height of the tab, which scrolls when the agents are more than fit.
const HEIGHT: f64 = 440.0;

/// The width of the agents' names, so their states line up.
const NAME_WIDTH: f64 = 190.0;

thread_local! {
    /// What each installation running now has got to, by the agent's id; the registry's fetch under the empty id.
    static PROGRESS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// The registry window, kept while the app runs.
    static WINDOW: RefCell<Option<Retained<NSPanel>>> = const { RefCell::new(None) };
    /// The registry's ids in the window's order, which its buttons' tags count.
    static SHOWN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// A button of a line: its title, its action and its tag.
type Action = (String, Sel, isize);

pub struct AgentsIvars {
    list: OnceCell<Retained<NSStackView>>,
    /// Whether a thread waits for the login shell's environment, which the states need.
    waiting: Cell<bool>,
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
                    self.reload();
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
                let id = SHOWN.with(|s| usize::try_from(tag(sender)).ok().and_then(|n| s.borrow().get(n).cloned()));
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

/// Installs `plan` after the user agreed, showing how far it got in the tab.
fn install(mtm: MainThreadMarker, id: &str, name: &str, plan: Plan) {
    if PROGRESS.with(|p| p.borrow().contains_key(id)) || !crate::agent::confirm_install(mtm, name, &plan) {
        return;
    }
    let (id, name) = (id.to_string(), name.to_string());
    PROGRESS.with(|p| p.borrow_mut().insert(id.clone(), tf("{} wird installiert …", &[&name])));
    super::agents_changed();
    let _ = std::thread::Builder::new().name("acp-install".into()).spawn(move || {
        let env = crate::environment::wait(Duration::from_secs(15))
            .map(<[(String, String)]>::to_vec)
            .unwrap_or_default();
        let installer = Installer::new(Supervisor::global(), env, installs(), Arc::new(Http::new()));
        let result = installer.install(&plan, true, &|_| {});
        main_soon(move || {
            PROGRESS.with(|p| p.borrow_mut().remove(&id));
            if let Err(error) = result
                && let Some(mtm) = MainThreadMarker::new()
            {
                alert(mtm, &tf("{} wurde nicht installiert", &[&name]), &error.to_string());
            }
            super::agents_changed();
            refresh_register();
        });
    });
}

/// Fetches the registry on the user's request and keeps it.
fn fetch() {
    PROGRESS.with(|p| {
        p.borrow_mut()
            .insert(String::new(), t("Register wird geladen …").to_string())
    });
    super::agents_changed();
    let _ = std::thread::Builder::new().name("acp-register".into()).spawn(|| {
        let result = Register::fetch(&Http::new()).map(|r| r.entries.len());
        main_soon(move || {
            PROGRESS.with(|p| p.borrow_mut().remove(""));
            if let Err(error) = result
                && let Some(mtm) = MainThreadMarker::new()
            {
                alert(mtm, t("Das Register wurde nicht geladen"), &error.to_string());
            }
            super::agents_changed();
            refresh_register();
        });
    });
}

/// What the tab says about `preset`, at `index` of the presets, and what it offers. An adapter whose agent's own
/// program is there says so, and Claude over ACP, only a fallback for Claude's own connection, never reads as Claude
/// missing.
fn preset_state(preset: &Preset, index: isize, env: &[(String, String)]) -> (String, Vec<Action>) {
    if let Some(progress) = PROGRESS.with(|p| p.borrow().get(preset.id).cloned()) {
        return (progress, Vec::new());
    }
    let launch = crate::agent::launch_of(&AcpAgent::of_preset(preset), env, std::path::Path::new("/"));
    let installs = installs();
    let mut actions = Vec::new();
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
            let (state, button) = missing(preset, preset.cli_on_path(env).is_some(), claude);
            actions.push((button.to_string(), sel!(ideAgentsInstall:), index));
            state
        }
    };
    (state, actions)
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
fn claude_state(env: &[(String, String)]) -> String {
    match crate::agent::claude_program(env) {
        Some(path) => tf("Bereit · {}", &[&crate::app::display_path(&path)]),
        None => t("Nicht gefunden: claude im Terminal installieren und dort anmelden").to_string(),
    }
}

/// What the tab says about the registry, and what it offers.
fn register_state() -> (String, Vec<Action>) {
    match (
        PROGRESS.with(|p| p.borrow().get("").cloned()),
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
            list: OnceCell::new(),
            waiting: Cell::new(false),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The tab: what it is about, then the agents line by line, scrolling when they are more than fit.
    pub fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let note = NSTextField::wrappingLabelWithString(
            &ns(t(
                "Die Agenten, mit denen du AI-Threads startest. Bereit ist ein Agent, dessen Programm im PATH der Login-Shell liegt oder den diese App installiert hat.",
            )),
            mtm,
        );
        note.setFont(Some(&crate::agent::style::small()));
        note.setTextColor(Some(&NSColor::secondaryLabelColor()));
        note.setPreferredMaxLayoutWidth(WIDTH);
        let list = self.ivars().list.get_or_init(|| {
            let list = NSStackView::new(mtm);
            list.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            list.setAlignment(NSLayoutAttribute::Leading);
            list.setSpacing(8.0);
            list
        });
        self.reload();
        let document = crate::agent_panel::flipped(mtm);
        document.addSubview(list);
        list.setTranslatesAutoresizingMaskIntoConstraints(false);
        let scroll = NSScrollView::new(mtm);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&document));
        document.setTranslatesAutoresizingMaskIntoConstraints(false);
        let root = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH + 40.0, HEIGHT)),
        );
        for view in [&*note as &NSView, &scroll] {
            root.addSubview(view);
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        for constraint in [
            list.topAnchor().constraintEqualToAnchor(&document.topAnchor()),
            list.leadingAnchor().constraintEqualToAnchor(&document.leadingAnchor()),
            list.bottomAnchor().constraintEqualToAnchor(&document.bottomAnchor()),
            document
                .widthAnchor()
                .constraintEqualToAnchor(&scroll.contentView().widthAnchor()),
            note.topAnchor()
                .constraintEqualToAnchor_constant(&root.topAnchor(), 20.0),
            note.leadingAnchor()
                .constraintEqualToAnchor_constant(&root.leadingAnchor(), 20.0),
            note.trailingAnchor()
                .constraintEqualToAnchor_constant(&root.trailingAnchor(), -20.0),
            scroll
                .topAnchor()
                .constraintEqualToAnchor_constant(&note.bottomAnchor(), 14.0),
            scroll
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&root.leadingAnchor(), 20.0),
            scroll
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&root.trailingAnchor(), -20.0),
            scroll
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&root.bottomAnchor(), -20.0),
        ] {
            constraint.setActive(true);
        }
        root
    }

    /// Shows every agent's state again, as it is now.
    pub fn reload(&self) {
        let Some(list) = self.ivars().list.get() else { return };
        for view in list.arrangedSubviews().iter() {
            list.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
        let env = crate::environment::get();
        let waiting = t("Umgebung der Login-Shell wird gelesen …");
        if env.is_none() {
            self.wait_for_environment();
        }
        let env = env.unwrap_or_default();
        let claude = if env.is_empty() {
            waiting.to_string()
        } else {
            claude_state(env)
        };
        list.addArrangedSubview(&self.line("Claude", &claude, &[]));
        list.addArrangedSubview(&self.line(
            t("Eigener Agent"),
            t("Anbieter, Modelle und Schlüssel im Tab „Eigener Agent“"),
            &[(t("Öffnen").to_string(), sel!(ideAgentsOwnSettings:), 0)],
        ));
        for (n, preset) in PRESETS.iter().enumerate().filter(|(_, preset)| preset.offered) {
            let (state, actions) = if env.is_empty() {
                (waiting.to_string(), Vec::new())
            } else {
                preset_state(preset, isize::try_from(n).unwrap_or(-1), env)
            };
            let name = if preset.id == "claude-acp" {
                t("Claude über ACP (Ausweichweg)")
            } else {
                preset.name
            };
            list.addArrangedSubview(&self.line(name, &state, &actions));
        }
        for (name, state) in registry_installed() {
            list.addArrangedSubview(&self.line(&name, &state, &[]));
        }
        let (state, actions) = register_state();
        list.addArrangedSubview(&self.line(t("ACP-Register"), &state, &actions));
    }

    /// Shows the states again once the login shell's environment is there, which a fresh start still reads.
    fn wait_for_environment(&self) {
        if self.ivars().waiting.replace(true) {
            return;
        }
        let _ = std::thread::Builder::new().name("agents-environment".into()).spawn(|| {
            let _ = crate::environment::wait(Duration::from_secs(30));
            main_soon(super::agents_changed);
        });
    }

    fn line(&self, name: &str, state: &str, actions: &[Action]) -> Retained<NSStackView> {
        let mtm = self.mtm();
        let title = NSTextField::labelWithString(&ns(name), mtm);
        title.setFont(Some(&crate::agent::style::medium(crate::agent::style::BODY)));
        title.setTranslatesAutoresizingMaskIntoConstraints(false);
        title
            .widthAnchor()
            .constraintEqualToConstant(NAME_WIDTH)
            .setActive(true);
        let detail = NSTextField::wrappingLabelWithString(&ns(state), mtm);
        detail.setFont(Some(&crate::agent::style::small()));
        detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
        detail.setPreferredMaxLayoutWidth(WIDTH - NAME_WIDTH - 140.0);
        let mut views: Vec<Retained<NSView>> = vec![
            Retained::into_super(Retained::into_super(title)),
            Retained::into_super(Retained::into_super(detail)),
            crate::agent_panel::spacer(mtm),
        ];
        for (title, action, tag) in actions {
            let button = link(mtm, title, self, *action, *tag);
            // The agents' buttons have the same titles; the agent's name tells them apart for VoiceOver.
            crate::util::set_label(&button, Some(&format!("{title}: {name}")));
            views.push(Retained::into_super(Retained::into_super(button)));
        }
        let line = NSStackView::new(mtm);
        line.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        line.setAlignment(NSLayoutAttribute::FirstBaseline);
        line.setSpacing(8.0);
        for view in &views {
            line.addArrangedSubview(view);
        }
        line.setTranslatesAutoresizingMaskIntoConstraints(false);
        line.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        line
    }

    /// What the tab shows, line by line, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn lines(&self) -> Vec<String> {
        let Some(list) = self.ivars().list.get() else {
            return Vec::new();
        };
        list.arrangedSubviews()
            .iter()
            .map(|line| {
                let Some(stack) = line.downcast_ref::<NSStackView>() else {
                    return String::new();
                };
                stack
                    .arrangedSubviews()
                    .iter()
                    .filter_map(|v| {
                        v.downcast_ref::<NSTextField>()
                            .map(|l| l.stringValue().to_string())
                            .or_else(|| v.downcast_ref::<NSButton>().map(|b| format!("[{}]", b.title())))
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }
}

/// What a registry entry offers this Mac, as the registry window says it.
fn offer_text(entry: &register::Entry) -> (String, bool) {
    if let Some((installed, _)) = installs().current(&entry.id) {
        return (tf("Installiert: Version {}", &[&installed.version]), false);
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
    column.setSpacing(10.0);
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
        install.setEnabled(PROGRESS.with(|p| !p.borrow().contains_key(&entry.id)));
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
        block.setSpacing(2.0);
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
    SHOWN.with(|s| *s.borrow_mut() = shown);
    column
}

/// Opens the window with the registry's entries.
fn show_register(target: &AgentsSettings) {
    let mtm = target.mtm();
    let Some((register, _)) = crate::agent::cached_register() else {
        return;
    };
    let panel = WINDOW.with(|w| {
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
    let open = WINDOW.with(|w| w.borrow().as_ref().is_some_and(|p| p.isVisible()));
    if open && let Some(target) = super::agents_tab_now() {
        show_register(&target);
    }
}

/// The registry window and the ids it lists, for the self-test.
#[cfg(feature = "selftest")]
pub fn register_window() -> Option<(Retained<NSPanel>, Vec<String>)> {
    let panel = WINDOW.with(|w| w.borrow().clone())?;
    let ids = SHOWN.with(|s| s.borrow().clone());
    Some((panel, ids))
}

/// Opens the registry window as its button does, for the self-test.
#[cfg(feature = "selftest")]
pub fn open_register(target: &AgentsSettings) {
    show_register(target);
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
}
