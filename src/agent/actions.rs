//! What the user does in an AI tab: send, queue, attach, answer requests, pick model and mode, rewind.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ide_claude::{PermissionAnswer, PlanAnswer};
use ide_git::{Restore, RestoreOptions};
use ide_thread::{
    Answer, Attachment, Entry, EntryKind, Lane, PermissionOption, RequestState, Status, Thread, ToolView, UserMessage,
};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{ClassType, MainThreadMarker, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSBitmapImageFileType, NSBitmapImageRep, NSMenu,
    NSModalResponseOK, NSOpenPanel, NSPasteboard, NSView,
};
use objc2_foundation::{NSArray, NSDictionary, NSPoint, NSURL};
use splat_i18n::{t, tf};

use super::acp::Agent;
use super::cards::pack;
use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE, DENY, PLAN_APPROVE, PLAN_REJECT, edited_input, scope_name};
use super::composer::image_types;
use super::tab::AgentTab;
use super::{format, style, ui};
use crate::util::{item, main_soon, ns, set_target};

/// What the model is told when the user refuses a tool; for the model, so not translated.
const DENIED: &str = "The user denied this tool call.";

/// The modes of the CLI's permission prompt, with how the composer names them.
pub fn modes() -> [(&'static str, &'static str, &'static str); 5] {
    [
        ("default", t("Manuell"), "hand.raised"),
        ("acceptEdits", t("Änderungen annehmen"), "pencil.and.outline"),
        ("plan", t("Plan"), "list.bullet.clipboard"),
        ("auto", t("Auto"), "wand.and.stars"),
        ("bypassPermissions", t("Umgehen"), "exclamationmark.shield"),
    ]
}

/// The name of an effort level as the composer shows it.
pub fn effort_name(level: &str) -> String {
    match level {
        "minimal" => t("Minimal").to_string(),
        "low" => t("Niedrig").to_string(),
        "medium" => t("Mittel").to_string(),
        "high" => t("Hoch").to_string(),
        "xhigh" => t("Extra hoch").to_string(),
        "max" => t("Maximal").to_string(),
        other => other.to_string(),
    }
}

/// A model's id as people say it: `claude-opus-5-5` as `Opus 5.5`.
pub fn model_name(id: &str) -> String {
    let id = id.split('[').next().unwrap_or(id);
    let rest = id.strip_prefix("claude-").unwrap_or(id);
    let mut parts = rest.split('-').filter(|p| p.len() < 8);
    let Some(family) = parts.next() else {
        return id.to_string();
    };
    let mut name: String = family
        .chars()
        .enumerate()
        .map(|(n, c)| if n == 0 { c.to_ascii_uppercase() } else { c })
        .collect();
    let version: Vec<&str> = parts.collect();
    if !version.is_empty() {
        name.push(' ');
        name.push_str(&version.join("."));
    }
    name
}

/// Every entry of `thread` with its number, in all lanes.
fn find_entry(thread: &Thread, id: u64) -> Option<Arc<Entry>> {
    let lanes = std::iter::once(Lane::Main).chain(thread.subagents().iter().map(|s| Lane::Subagent(s.key.clone())));
    for lane in lanes {
        if let Some(entry) = thread.entries(&lane).iter().find(|e| e.id.get() == id) {
            return Some(entry.clone());
        }
    }
    None
}

pub(super) fn pop_up(menu: &NSMenu, view: &NSView) {
    let below = NSPoint::new(0.0, view.frame().size.height + 4.0);
    menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(view));
}

impl AgentTab {
    pub(super) fn entry(&self, id: u64) -> Option<Arc<Entry>> {
        find_entry(&self.state.borrow().thread, id)
    }

    pub(super) fn menu_item(
        &self,
        menu: &NSMenu,
        title: &str,
        action: Sel,
        tag: isize,
    ) -> Retained<objc2_app_kit::NSMenuItem> {
        let entry = item(self.mtm, title, Some(action), "");
        entry.setTag(tag);
        set_target(&entry, &self.target);
        menu.addItem(&entry);
        entry
    }

    /// The send button: sends, or stops the turn when there is nothing to send.
    pub fn send_or_stop(&self) {
        let has_text = !self.composer.text().trim().is_empty() || !self.state.borrow().attachments.is_empty();
        if has_text {
            self.submit();
        } else if self.status() != Status::Idle {
            self.stop_turn();
        }
    }

    /// Sends the first queued message once the session rests.
    pub(super) fn send_queued(&self) {
        let next = {
            let mut state = self.state.borrow_mut();
            if state.status != Status::Idle || state.sending || !state.source.is_live() || state.queue.is_empty() {
                return;
            }
            state.queue.remove(0)
        };
        self.refresh_queue();
        self.send_message(next);
    }

    /// Takes a checkpoint of the folder (I6), then sends `message`.
    pub(super) fn send_message(&self, message: UserMessage) {
        self.state.borrow_mut().sending = true;
        self.refresh_composer();
        let key = self.key;
        let label = format::first_line(&message.text, 80);
        self.store.create(self.thread_id.to_string(), label, move |checkpoint| {
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.deliver(message, checkpoint);
                }
            });
        });
    }

    pub(super) fn deliver(&self, mut message: UserMessage, checkpoint: Option<ide_git::Checkpoint>) {
        // ACP and the own agent give prompts no id; the app's own one keys the checkpoint taken before the prompt (I6).
        let app_ids = self.agent.is_acp() || self.agent.is_own();
        if app_ids && message.message_id.is_none() {
            message.message_id = Some(format!("app-{}", ide_thread::ThreadId::new()));
        }
        let own_id = message.message_id.clone().filter(|_| app_ids);
        {
            let mut state = self.state.borrow_mut();
            state.sending = false;
            // A refusal comes with the result of the first prompt on the rewound session, so by the second the
            // rewind holds.
            if state.fork.as_ref().is_some_and(|f| f.prompted) {
                state.fork = None;
            } else if let Some(fork) = state.fork.as_mut() {
                fork.prompted = true;
            }
            state.last_sent = Some(message.clone());
        }
        let text = message.text.clone();
        let sent = self.state.borrow().source.send(message);
        match sent {
            Ok(uuid) => {
                if let Some(checkpoint) = checkpoint {
                    let mut state = self.state.borrow_mut();
                    if let Some(uuid) = uuid.or(own_id) {
                        state.checkpoints.insert(uuid, checkpoint.clone());
                    }
                    state.turn = Some(checkpoint);
                }
            }
            Err(error) => {
                crate::diag::report("sending to the agent", &error);
                if self.composer.text().is_empty() {
                    self.composer.set_text(&text);
                }
            }
        }
        self.refresh_composer();
    }

    /// The pills, the send button, the queue and the warning, as the state is.
    pub(super) fn refresh_composer(&self) {
        if self.agent.is_own() {
            return self.refresh_own_composer();
        }
        if let Some(agent) = self.acp_agent() {
            self.refresh_acp_pills(&agent);
            let state = self.state.borrow();
            self.composer.set_placeholder(
                if state.sending {
                    t("Sicherungspunkt wird angelegt …").to_string()
                } else {
                    tf("⌘Esc zum Fokussieren oder Verlassen von {}", &[&agent.name])
                }
                .as_str(),
            );
            let running = state.status != Status::Idle || state.sending;
            let can_send = !self.composer.text().trim().is_empty() || !state.attachments.is_empty();
            self.composer.show_running(running, can_send);
            return;
        }
        let state = self.state.borrow();
        let model_id = state.thread.model().map(str::to_string).or_else(|| state.model.clone());
        let model = model_id
            .as_deref()
            .map(|id| {
                state
                    .caps
                    .models
                    .iter()
                    .find(|m| m.value == id)
                    .map_or_else(|| model_name(id), |m| m.display_name.clone())
            })
            .unwrap_or_else(|| t("Standardmodell").to_string());
        let effort = state
            .thread
            .effort()
            .map(str::to_string)
            .or_else(|| state.effort.clone());
        let pill = match effort {
            Some(effort) => tf("{} · {}", &[&model, &effort_name(&effort)]),
            None => model,
        };
        self.composer.model.setTitle(&ns(&pill));
        let mode_id = state
            .thread
            .mode()
            .map(str::to_string)
            .unwrap_or_else(|| state.mode.clone());
        let mode_id = if mode_id == "manual" {
            "default".to_string()
        } else {
            mode_id
        };
        let (mode_title, mode_symbol) = modes()
            .iter()
            .find(|(id, _, _)| *id == mode_id)
            .map_or((mode_id.clone(), "hand.raised"), |(_, title, symbol)| {
                ((*title).to_string(), *symbol)
            });
        self.composer.mode.setTitle(&ns(&mode_title));
        self.composer
            .mode
            .setImage(crate::util::symbol(mode_symbol, &mode_title).as_deref());
        self.composer
            .mode
            .setImagePosition(objc2_app_kit::NSCellImagePosition::ImageLeading);
        let (shield, tint, tip) = if state.sandbox {
            (
                "checkmark.shield.fill",
                objc2_app_kit::NSColor::systemGreenColor(),
                t("Sandbox an: Shell-Befehle laufen abgeschirmt; Datei- und Webwerkzeuge nicht"),
            )
        } else {
            (
                "shield.slash",
                objc2_app_kit::NSColor::secondaryLabelColor(),
                t("Sandbox aus: Shell-Befehle laufen ohne Abschirmung"),
            )
        };
        self.composer
            .sandbox
            .setImage(crate::util::symbol(shield, tip).as_deref());
        self.composer.sandbox.setContentTintColor(Some(&tint));
        self.composer.sandbox.setToolTip(Some(&ns(tip)));
        self.composer.set_placeholder(if state.sending {
            t("Sicherungspunkt wird angelegt …")
        } else {
            t("⌘Esc zum Fokussieren oder Verlassen von Claude")
        });
        let running = state.status != Status::Idle || state.sending;
        let can_send = !self.composer.text().trim().is_empty() || !state.attachments.is_empty();
        self.composer.show_running(running, can_send);
    }

    /// The queued messages above the composer, each with a button that drops it.
    pub(super) fn refresh_queue(&self) {
        for view in self.queue_view.arrangedSubviews().iter() {
            self.queue_view.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
        let queue: Vec<String> = self.state.borrow().queue.iter().map(|m| m.text.clone()).collect();
        for (n, text) in queue.iter().enumerate() {
            let mtm = self.mtm;
            let icon = ui::icon(
                mtm,
                "clock",
                t("Wartet"),
                11.0,
                &objc2_app_kit::NSColor::secondaryLabelColor(),
            );
            let label = ui::label(
                mtm,
                &tf("Wartet: {}", &[&format::first_line(text, 120)]),
                &style::small(),
                &objc2_app_kit::NSColor::secondaryLabelColor(),
            );
            let remove = ui::symbol_button(
                mtm,
                "xmark.circle.fill",
                t("Aus der Warteschlange nehmen"),
                11.0,
                &self.target,
                sel!(ideAgentRemoveQueued:),
                isize::try_from(n).unwrap_or(-1),
            );
            let spacer = ui::spacer(mtm);
            let row = ui::row(mtm, 6.0, &[&icon, &label, &spacer, &remove]);
            ui::add_full_width(&self.queue_view, &row);
        }
        self.queue_view.setHidden(queue.is_empty());
    }

    pub fn remove_queued(&self, index: usize) {
        {
            let mut state = self.state.borrow_mut();
            if index < state.queue.len() {
                state.queue.remove(index);
            }
        }
        self.refresh_queue();
    }

    pub fn remove_attachment(&self, index: usize) {
        {
            let mut state = self.state.borrow_mut();
            if index < state.attachments.len() {
                state.attachments.remove(index);
            }
        }
        self.refresh_chips();
    }

    /// Takes pictures, files and text from a web page from a paste or a drop as attachments; whether there were any.
    pub fn take_pasteboard(&self, pasteboard: &NSPasteboard) -> bool {
        let mut taken = Vec::new();
        let classes = NSArray::from_slice(&[NSURL::class()]);
        // SAFETY: Reading NSURL objects with no options returns NSURLs, as the class list asks.
        let urls = unsafe { pasteboard.readObjectsForClasses_options(&classes, Some(&NSDictionary::new())) };
        for object in urls.iter().flat_map(|a| a.iter()) {
            if let Some(path) = object
                .downcast_ref::<NSURL>()
                .filter(|u| u.isFileURL())
                .and_then(|u| u.path())
            {
                taken.push(super::outside::classify(
                    file_attachment(Path::new(&path.to_string())),
                    &self.cwd,
                ));
            }
        }
        if taken.is_empty() {
            taken.extend(super::outside::web_paste(pasteboard));
        }
        if taken.is_empty() {
            for kind in image_types() {
                if let Some(data) = pasteboard.dataForType(kind) {
                    if let Some(png) = as_png(&data.to_vec()) {
                        taken.push(Attachment::Image {
                            name: None,
                            media_type: "image/png".into(),
                            data: png.into(),
                        });
                    }
                    break;
                }
            }
        }
        if taken.is_empty() {
            return false;
        }
        self.state.borrow_mut().attachments.extend(taken);
        self.refresh_chips();
        true
    }

    /// `+`: files and pictures from an open panel.
    pub fn attach(&self) {
        let panel = NSOpenPanel::openPanel(self.mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(true);
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&ns(&self.cwd.to_string_lossy()))));
        if panel.runModal() != NSModalResponseOK {
            return;
        }
        let picked: Vec<Attachment> = panel
            .URLs()
            .iter()
            .filter_map(|u| u.path())
            .map(|p| super::outside::classify(file_attachment(Path::new(&p.to_string())), &self.cwd))
            .collect();
        self.state.borrow_mut().attachments.extend(picked);
        self.refresh_chips();
    }

    /// `/`: the agent's slash commands.
    pub fn slash_menu(&self, view: &NSView) {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let commands: Vec<(String, String)> = if self.agent.is_acp() {
            self.acp_commands()
        } else if self.agent.is_own() {
            vec![(
                "compact".to_string(),
                t("Ältere Züge zu einer Zusammenfassung verdichten").to_string(),
            )]
        } else {
            let state = self.state.borrow();
            if state.caps.commands.is_empty() {
                state
                    .caps
                    .slash_commands
                    .iter()
                    .map(|c| (c.clone(), String::new()))
                    .collect()
            } else {
                state
                    .caps
                    .commands
                    .iter()
                    .map(|c| (c.name.clone(), c.description.clone()))
                    .collect()
            }
        };
        if commands.is_empty() {
            let none = self.menu_item(&menu, t("Noch keine Befehle bekannt"), sel!(ideAgentPickCommand:), -1);
            none.setEnabled(false);
        }
        for (n, (name, description)) in commands.iter().enumerate() {
            let entry = self.menu_item(&menu, &format!("/{name}"), sel!(ideAgentPickCommand:), index_tag(n));
            if !description.is_empty() {
                entry.setToolTip(Some(&ns(description)));
            }
        }
        self.state.borrow_mut().offered.commands = commands.into_iter().map(|(n, _)| n).collect();
        pop_up(&menu, view);
    }

    pub fn pick_command(&self, index: usize) {
        let Some(name) = self.state.borrow().offered.commands.get(index).cloned() else {
            return;
        };
        self.composer.set_text(&format!("/{name} "));
        self.focus_composer();
        self.refresh_composer();
    }

    /// The model and effort pill: the models the CLI offers, then the effort levels of the current one.
    pub fn model_menu(&self, view: &NSView) {
        if self.agent.is_acp() {
            return self.acp_model_menu(view);
        }
        if self.agent.is_own() {
            return self.own_model_menu(view);
        }
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let (models, efforts, current, effort) = {
            let state = self.state.borrow();
            let current = state.thread.model().map(str::to_string).or_else(|| state.model.clone());
            let models: Vec<(String, String, String)> = state
                .caps
                .models
                .iter()
                .map(|m| (m.value.clone(), m.display_name.clone(), m.description.clone()))
                .collect();
            let supported = state
                .caps
                .models
                .iter()
                .find(|m| Some(&m.value) == current.as_ref())
                .map(|m| m.supported_effort_levels.clone())
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| ["low", "medium", "high", "xhigh", "max"].map(str::to_string).to_vec());
            let effort = state
                .thread
                .effort()
                .map(str::to_string)
                .or_else(|| state.effort.clone());
            (models, supported, current, effort)
        };
        let heading = self.menu_item(&menu, t("Modell"), sel!(ideAgentPickModel:), -1);
        heading.setEnabled(false);
        if models.is_empty() {
            let shown = current
                .as_deref()
                .map_or_else(|| t("Standardmodell").to_string(), model_name);
            let entry = self.menu_item(&menu, &shown, sel!(ideAgentPickModel:), -1);
            entry.setEnabled(false);
        }
        for (n, (value, name, description)) in models.iter().enumerate() {
            let entry = self.menu_item(&menu, name, sel!(ideAgentPickModel:), index_tag(n));
            entry.setToolTip(Some(&ns(description)));
            if Some(value) == current.as_ref() {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(self.mtm));
        let heading = self.menu_item(&menu, t("Denktiefe"), sel!(ideAgentPickEffort:), -1);
        heading.setEnabled(false);
        for (n, level) in efforts.iter().enumerate() {
            let entry = self.menu_item(&menu, &effort_name(level), sel!(ideAgentPickEffort:), index_tag(n));
            if Some(level) == effort.as_ref() {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        {
            let mut state = self.state.borrow_mut();
            state.offered.models = models.into_iter().map(|(v, _, _)| v).collect();
            state.offered.efforts = efforts;
        }
        pop_up(&menu, view);
    }

    pub fn pick_model(&self, index: usize) {
        if self.agent.is_acp() {
            return self.acp_pick_model(index);
        }
        if self.agent.is_own() {
            return self.own_pick_model(index);
        }
        let Some(model) = self.state.borrow().offered.models.get(index).cloned() else {
            return;
        };
        self.state.borrow().source.set_model(&model);
        super::choices::remember(self.agent.id(), |choice| choice.model = Some(model.clone()));
        self.state.borrow_mut().model = Some(model);
        self.refresh_composer();
    }

    pub fn pick_effort(&self, index: usize) {
        if self.agent.is_own() {
            return self.own_pick_effort(index);
        }
        let Some(level) = self.state.borrow().offered.efforts.get(index).cloned() else {
            return;
        };
        self.state.borrow().source.set_effort(&level);
        super::choices::remember(self.agent.id(), |choice| choice.effort = Some(level.clone()));
        self.state.borrow_mut().effort = Some(level);
        self.refresh_composer();
    }

    /// The mode pill: how freely Claude may use tools.
    pub fn mode_menu(&self, view: &NSView) {
        if self.agent.is_acp() {
            return self.acp_mode_menu(view);
        }
        if self.agent.is_own() {
            return self.own_mode_menu(view);
        }
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let current = {
            let state = self.state.borrow();
            state
                .thread
                .mode()
                .map(str::to_string)
                .unwrap_or_else(|| state.mode.clone())
        };
        for (n, (id, title, symbol)) in modes().iter().enumerate() {
            let entry = self.menu_item(&menu, title, sel!(ideAgentPickMode:), index_tag(n));
            entry.setImage(crate::util::symbol(symbol, title).as_deref());
            if *id == current || (current == "manual" && *id == "default") {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        self.state.borrow_mut().offered.modes = modes().iter().map(|(id, _, _)| (*id).to_string()).collect();
        pop_up(&menu, view);
    }

    pub fn pick_mode(&self, index: usize) {
        if self.agent.is_acp() {
            return self.acp_pick_mode(index);
        }
        if self.agent.is_own() {
            return self.own_pick_mode(index);
        }
        let Some(mode) = self.state.borrow().offered.modes.get(index).cloned() else {
            return;
        };
        let key = self.key;
        self.state.borrow().source.set_mode(&mode, move |why| {
            crate::util::main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.mode_refused(&why);
                }
            });
        });
        super::choices::remember(self.agent.id(), |choice| choice.mode = Some(mode.clone()));
        self.state.borrow_mut().mode = mode;
        self.refresh_composer();
    }

    /// The CLI kept its mode instead of the one picked: the composer shows the mode it runs in, the next sessions start
    /// with that one, and an alert gives the CLI's reason.
    pub(super) fn mode_refused(&self, why: &str) {
        let now = match self.state.borrow().thread.mode() {
            None | Some("manual") => "default".to_string(),
            Some(mode) => mode.to_string(),
        };
        super::choices::remember(self.agent.id(), |choice| choice.mode = Some(now.clone()));
        self.state.borrow_mut().mode = now;
        self.refresh_composer();
        crate::editor::show_alert(self.mtm, t("Freigabemodus nicht gewechselt"), why);
    }

    /// The sandbox switch (E19): the CLI takes it at start, so a resting session starts again with it.
    pub fn toggle_sandbox(&self) {
        if self.acp_agent().is_some_and(|a| !a.sandbox.is_available()) {
            return;
        }
        if self.agent.is_own() {
            return self.own_toggle_sandbox();
        }
        let restart = {
            let mut state = self.state.borrow_mut();
            state.sandbox = !state.sandbox;
            state.source.is_live() && state.status == Status::Idle
        };
        self.refresh_composer();
        if restart {
            let source = std::mem::replace(&mut self.state.borrow_mut().source, super::source::Source::Idle);
            source.end();
            self.start();
        }
    }

    pub fn toggle_bookmark(&self) {
        let on = {
            let mut state = self.state.borrow_mut();
            state.bookmarked = !state.bookmarked;
            state.bookmarked
        };
        crate::agent_panel::set_bookmarked(&self.thread_id, on);
        self.refresh();
        if let Some(controller) = self.controller() {
            super::changed(&controller);
        }
    }

    /// The clock: sessions of this folder from the index and, experimentally, from the CLI's own transcripts.
    pub fn history_menu(&self, view: &NSView) {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let past = crate::agent_panel::past_sessions(&self.cwd, 20);
        if past.is_empty() {
            let none = self.menu_item(&menu, t("Keine früheren Sitzungen"), sel!(ideAgentOpenPast:), -1);
            none.setEnabled(false);
        }
        for (n, (resume, when)) in past.iter().enumerate() {
            let title = resume.title.clone().unwrap_or_else(|| resume.session.clone());
            let entry = self.menu_item(
                &menu,
                &tf("{} · {}", &[&format::first_line(&title, 60), &format::age(*when)]),
                sel!(ideAgentOpenPast:),
                index_tag(n),
            );
            entry.setToolTip(Some(&ns(&title)));
        }
        self.state.borrow_mut().offered.past = past.into_iter().map(|(r, _)| r).collect();
        pop_up(&menu, view);
    }

    pub fn open_past(&self, index: usize) {
        let Some(resume) = self.state.borrow().offered.past.get(index).cloned() else {
            return;
        };
        if let Some(controller) = self.controller() {
            super::open_tab(&controller, Some(resume));
        }
    }

    pub fn new_session(&self) {
        if let Some(controller) = self.controller()
            && let Some(tab) = super::open_tab(&controller, None)
        {
            tab.focus_composer();
        }
    }

    /// Renames the session, as the title's double click and the session list do.
    pub fn rename(&self) {
        let Some(name) = ask_text(self.mtm, t("Sitzung umbenennen"), &self.title()) else {
            return;
        };
        self.set_title(&name);
    }

    pub fn set_title(&self, name: &str) {
        self.state.borrow_mut().custom_title = Some(name.to_string());
        let mut index = ide_thread::ThreadIndex::open(&ide_thread::ThreadIndex::default_dir());
        let _ = index.rename(&self.thread_id, name);
        self.persist();
        self.refresh();
        if let Some(controller) = self.controller() {
            super::changed(&controller);
        }
    }

    pub fn trust_folder(&self) {
        crate::project::trust(&self.cwd);
        self.start();
    }

    pub fn retry(&self) {
        self.start();
    }

    /// Opens a terminal in the folder, where `claude` signs in by itself.
    pub fn sign_in(&self, index: usize) {
        if self.agent.is_acp() {
            return self.acp_sign_in(index);
        }
        if let Some(controller) = self.controller() {
            crate::terminal::open_tab(&controller, Some(&self.cwd));
        }
    }

    pub fn permission(&self, id: u64, what: u8, view: Option<&NSView>) {
        let several_rules = || {
            self.entry(id).is_some_and(|e| match &e.kind {
                EntryKind::PermissionRequest(r) => {
                    r.options
                        .iter()
                        .filter(|o| matches!(o, PermissionOption::AllowAlways { .. }))
                        .count()
                        > 1
                }
                _ => false,
            })
        };
        if self.agent.is_acp() && !(what == ALLOW_ALWAYS && several_rules()) {
            return self.acp_permission(id, what, None);
        }
        if self.agent.is_own() {
            return self.own_permission(id, what);
        }
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PermissionRequest(request) = &entry.kind else {
            return;
        };
        if request.state != RequestState::Pending {
            return;
        }
        let answer = match what {
            ALLOW_ONCE => {
                let Some(input) = self.edited(id, &request.tool_name, &request.input) else {
                    return;
                };
                PermissionAnswer::Allow {
                    updated_input: Some(input).filter(|i| *i != request.input),
                    always: Vec::new(),
                }
            }
            ALLOW_ALWAYS => {
                let rules: Vec<(usize, String, String)> = request
                    .options
                    .iter()
                    .filter_map(|o| match o {
                        PermissionOption::AllowAlways {
                            suggestion,
                            rule,
                            scope,
                        } => Some((*suggestion, rule.clone(), scope.clone())),
                        _ => None,
                    })
                    .collect();
                if rules.len() > 1 {
                    let menu = NSMenu::new(self.mtm);
                    for (suggestion, rule, scope) in &rules {
                        let tag = pack(entry.id, u8::try_from(*suggestion).unwrap_or(0));
                        self.menu_item(
                            &menu,
                            &tf("{} · {}", &[rule, &scope_name(scope)]),
                            sel!(ideAgentAllowRule:),
                            tag,
                        );
                    }
                    if let Some(view) = view {
                        pop_up(&menu, view);
                    }
                    return;
                }
                let Some((suggestion, _, _)) = rules.first() else {
                    return;
                };
                return self.allow_rule(id, *suggestion);
            }
            DENY => PermissionAnswer::Deny {
                message: DENIED.to_string(),
                interrupt: false,
            },
            _ => return,
        };
        self.settle(id, &request.request_id, answer);
    }

    /// Allows request `id` and stores the rule of suggestion `suggestion`.
    pub fn allow_rule(&self, id: u64, suggestion: usize) {
        if self.agent.is_acp() {
            return self.acp_permission(id, ALLOW_ALWAYS, Some(suggestion));
        }
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PermissionRequest(request) = &entry.kind else {
            return;
        };
        let Some(input) = self.edited(id, &request.tool_name, &request.input) else {
            return;
        };
        let answer = PermissionAnswer::Allow {
            updated_input: Some(input).filter(|i| *i != request.input),
            always: vec![suggestion],
        };
        self.settle(id, &request.request_id, answer);
    }

    fn settle(&self, id: u64, request: &str, answer: PermissionAnswer) {
        let result = self.state.borrow().source.answer(request, answer);
        if let Err(error) = result {
            crate::diag::report("answering a permission request", &error);
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// The input as the user edited it; `None`, after telling them, when the edit is not valid.
    fn edited(&self, id: u64, tool: &str, original: &serde_json::Value) -> Option<serde_json::Value> {
        let text = self.timeline().inputs().borrow().editor_text(id);
        let Some(text) = text else {
            return Some(original.clone());
        };
        let input = edited_input(tool, original, &text);
        if input.is_none() {
            let alert = NSAlert::new(self.mtm);
            alert.setMessageText(&ns(t("Die geänderte Eingabe ist kein gültiges JSON.")));
            alert.setInformativeText(&ns(t("Korrigiere sie oder lehne die Anfrage ab.")));
            alert.runModal();
        }
        input
    }

    /// Option `option` of question `question` of request `id`; sent once every question has an answer.
    pub fn answer(&self, id: u64, question: u8, option: u8) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::Question(asked) = &entry.kind else {
            return;
        };
        let Some(label) = asked
            .items
            .get(usize::from(question))
            .and_then(|i| i.options.get(usize::from(option)))
            .map(|o| o.label.clone())
        else {
            return;
        };
        self.timeline()
            .inputs()
            .borrow_mut()
            .chosen
            .insert((id, question), label);
        self.send_answers(id);
    }

    /// The user's own words, or the checked options, for question `question` of request `id`.
    pub fn answer_other(&self, id: u64, question: u8) {
        let text = {
            let inputs = self.timeline().inputs().borrow();
            let mut parts = inputs
                .checks
                .get(&(id, question))
                .map(|b| super::cards::request::checked(b))
                .unwrap_or_default();
            parts.extend(inputs.field_text(id, question));
            parts.join(", ")
        };
        if text.is_empty() {
            return;
        }
        self.timeline()
            .inputs()
            .borrow_mut()
            .chosen
            .insert((id, question), text);
        self.send_answers(id);
    }

    fn send_answers(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::Question(asked) = &entry.kind else {
            return;
        };
        let Some(request) = asked.request_id.clone() else {
            return;
        };
        let answers: Option<Vec<Answer>> = {
            let inputs = self.timeline().inputs().borrow();
            asked
                .items
                .iter()
                .enumerate()
                .map(|(n, item)| {
                    let chosen = inputs.chosen.get(&(id, u8::try_from(n).ok()?))?;
                    Some(Answer {
                        question: item.question.clone(),
                        answer: chosen.clone(),
                    })
                })
                .collect()
        };
        let Some(answers) = answers else {
            self.timeline().reload_entry(id);
            return;
        };
        let result = self.state.borrow().source.answer_questions(&request, answers);
        if let Err(error) = result {
            crate::diag::report("answering a question", &error);
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// Answers plan `id` as its button `what` says: approve, revise with the comments, or reject.
    pub fn plan(&self, id: u64, what: u8) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PlanApproval(plan) = &entry.kind else {
            return;
        };
        let Some(request) = plan.request_id.clone() else { return };
        let answer = match what {
            PLAN_APPROVE => PlanAnswer::Approve,
            PLAN_REJECT => PlanAnswer::Reject,
            _ => PlanAnswer::Revise(self.plan_feedback(id)),
        };
        super::plan::close_comment();
        let result = self.state.borrow_mut().source.answer_plan(&request, answer);
        match result {
            Ok(changes) if !changes.is_empty() => {
                let snapshot = self.state.borrow().source.snapshot();
                if let Some(snapshot) = snapshot {
                    self.absorb(snapshot, &changes);
                }
            }
            Ok(_) => {}
            Err(error) => crate::diag::report("answering a plan", &error),
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// Stops the subagent the card `id` shows.
    pub fn stop_task_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::ToolCall(call) = &entry.kind else { return };
        let task = self
            .state
            .borrow()
            .thread
            .subagent(&call.call_id)
            .and_then(|s| s.task_id.clone());
        if let Some(task) = task {
            self.stop_task(&task);
        }
    }

    /// Stops a subagent or background task by its task id.
    pub fn stop_task(&self, task: &str) {
        self.state.borrow().source.stop_task(task);
    }

    pub fn move_to_background(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        if let EntryKind::ToolCall(call) = &entry.kind {
            self.state.borrow().source.background(&call.call_id);
        }
    }

    pub fn show_output_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        if let EntryKind::ToolCall(call) = &entry.kind
            && let Some(task) = &call.background_task
        {
            self.show_background_output(task);
        }
    }

    /// Shows the whole output of background task `task`: the file the CLI writes it to, else what it hands over.
    pub fn show_background_output(&self, task: &str) {
        let Some(controller) = self.controller() else { return };
        let path = {
            let state = self.state.borrow();
            state
                .thread
                .background()
                .iter()
                .find(|b| b.id == task)
                .and_then(|b| b.output.path.clone())
        };
        if let Some(path) = path.map(PathBuf::from).filter(|p| p.is_file()) {
            controller.open_file(&path);
            return;
        }
        let Some(session) = self.state.borrow().source.session().cloned() else {
            return;
        };
        let key = self.key;
        let task = task.to_string();
        session.task_output(&task).then(move |output| {
            let Ok(output) = output else { return };
            let file = ide_core::paths::data_dir()
                .join("task-output")
                .join(format!("{task}.txt"));
            let written = file
                .parent()
                .map(std::fs::create_dir_all)
                .transpose()
                .and_then(|_| std::fs::write(&file, output.output));
            if written.is_ok() {
                main_soon(move || {
                    if let Some(controller) = super::get(key).and_then(|t| t.controller()) {
                        controller.open_file(&file);
                    }
                });
            }
        });
    }

    /// Opens the overview of the session's subagents and background tasks below `anchor`, else below the head's
    /// button for it.
    pub fn show_tasks(&self, anchor: Option<&NSView>) {
        let Some(controller) = self.controller() else { return };
        let fallback = ui::view(&self.header.tasks);
        crate::activity::show_tab(&controller, self.key, anchor.unwrap_or(&fallback));
    }

    /// Shows only the conversation, without tools, thinking and status lines, or everything again.
    pub fn toggle_focus_view(&self) {
        let on = !self.timeline().focus_view();
        self.timeline().set_focus_view(on);
        self.header.set_focus_view(on);
    }

    /// Copies the answer `id` as markdown.
    pub fn copy_entry(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::AgentMessage(message) = &entry.kind else {
            return;
        };
        copy_text(&message.markdown);
    }

    /// Opens the file a Read, Edit or Write card names.
    pub fn open_path_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::ToolCall(call) = &entry.kind else { return };
        let path = match &call.view {
            ToolView::Read { path, .. } | ToolView::Edit { path, .. } | ToolView::Write { path, .. } => path.clone(),
            _ => return,
        };
        let path = self.cwd.join(path);
        if let Some(controller) = self.controller() {
            controller.open_file(&path);
        }
    }

    /// The rewind menu of user message `id`: the conversation, the code, or both; only while the session rests.
    pub fn rewind_menu(&self, id: u64, view: Option<&NSView>) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return;
        };
        let Some(uuid) = &message.message_id else { return };
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let resting = self.status() == Status::Idle && !self.state.borrow().sending;
        for (title, what, possible) in self.rewind_choices(&entry, uuid) {
            let item = self.menu_item(&menu, title, sel!(ideAgentRewindPick:), pack(entry.id, what));
            item.setEnabled(possible && resting);
            if !resting {
                item.setToolTip(Some(&ns(&tf("Erst wenn {} fertig ist", &[self.agent.name()]))));
            }
        }
        if let Some(view) = view {
            pop_up(&menu, view);
        }
    }

    /// What the rewind menu of user message `entry` offers: the conversation, the code (I6), both; an agent over ACP
    /// and the own agent can only take the code back, from the app's own checkpoint (E40).
    pub(super) fn rewind_choices(&self, entry: &Entry, uuid: &str) -> Vec<(&'static str, u8, bool)> {
        let state = self.state.borrow();
        if !matches!(self.agent, Agent::Claude) {
            let has_code = state.checkpoints.contains_key(uuid);
            return vec![(t("Code auf den Stand vor dieser Nachricht"), 2, has_code)];
        }
        let has_code = state.checkpoints.contains_key(uuid) || state.source.is_live();
        drop(state);
        let has_conversation = self.conversation_rewindable(entry);
        vec![
            (t("Unterhaltung bis hierher zurückspulen"), 1, has_conversation),
            (t("Code auf den Stand vor dieser Nachricht"), 2, has_code),
            (t("Beides zurückspulen"), 3, has_conversation && has_code),
        ]
    }

    /// Rewinds to before user message `id`: `what` 1 the conversation, 2 the code, 3 both.
    pub fn rewind(&self, id: u64, what: u8) {
        if self.status() != Status::Idle || (!matches!(self.agent, Agent::Claude) && what != 2) {
            return;
        }
        if what == 2 || what == 3 {
            self.rewind_code(id);
        }
        if what == 1 || what == 3 {
            self.rewind_conversation(id);
        }
    }

    /// Brings the files back to how they were before user message `id`: the app's checkpoint, else the CLI's own
    /// file history.
    fn rewind_code(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return;
        };
        let Some(uuid) = message.message_id.clone() else { return };
        let checkpoint = self.state.borrow().checkpoints.get(&uuid).cloned();
        match checkpoint {
            Some(checkpoint) => self.restore(checkpoint, RestoreOptions::default()),
            None if matches!(self.agent, Agent::Claude) => self.rewind_with_cli(&uuid),
            None => {}
        }
    }

    fn restore(&self, checkpoint: ide_git::Checkpoint, options: RestoreOptions) {
        let key = self.key;
        let again = checkpoint.clone();
        self.store.restore(checkpoint, options, move |outcome| {
            main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.restored(again, outcome);
                }
            });
        });
    }

    fn restored(&self, checkpoint: ide_git::Checkpoint, outcome: Result<Restore, String>) {
        let alert = NSAlert::new(self.mtm);
        match outcome {
            Ok(Restore::UserChanges(files)) => {
                alert.setMessageText(&ns(&tf(
                    "Du hast seitdem {} Dateien selbst geändert.",
                    &[&files.len().to_string()],
                )));
                let names: Vec<String> = files.iter().take(12).map(|f| f.display().to_string()).collect();
                alert.setInformativeText(&ns(&names.join("\n")));
                alert.addButtonWithTitle(&ns(t("Deine Änderungen behalten")));
                alert.addButtonWithTitle(&ns(t("Trotzdem überschreiben")));
                alert.addButtonWithTitle(&ns(t("Abbrechen")));
                let response = run_alert(&alert);
                if response == NSAlertFirstButtonReturn {
                    let options = RestoreOptions {
                        keep: files,
                        ..RestoreOptions::default()
                    };
                    self.restore(checkpoint, options);
                } else if response == NSAlertSecondButtonReturn {
                    let options = RestoreOptions {
                        force: true,
                        ..RestoreOptions::default()
                    };
                    self.restore(checkpoint, options);
                }
                return;
            }
            Ok(Restore::Restored(done)) => {
                alert.setMessageText(&ns(t("Code zurückgespult")));
                alert.setInformativeText(&ns(&tf(
                    "{} Dateien wiederhergestellt, {} entfernt.",
                    &[&done.written.len().to_string(), &done.removed.len().to_string()],
                )));
            }
            Err(error) => {
                alert.setMessageText(&ns(t("Zurückspulen fehlgeschlagen")));
                alert.setInformativeText(&ns(&error));
            }
        }
        run_alert(&alert);
    }

    /// Without a checkpoint of the app, as for a resumed session: the CLI's own file history (experimental).
    fn rewind_with_cli(&self, uuid: &str) {
        let Some(session) = self.state.borrow().source.session().cloned() else {
            return;
        };
        let key = self.key;
        session.rewind_files(uuid, false).then(move |result| {
            main_soon(move || {
                let Some(mtm) = MainThreadMarker::new() else { return };
                if super::get(key).is_none() {
                    return;
                }
                let alert = NSAlert::new(mtm);
                match result {
                    Ok(done) if done.can_rewind => {
                        alert.setMessageText(&ns(t("Code zurückgespult")));
                        alert.setInformativeText(&ns(&tf(
                            "{} Dateien geändert.",
                            &[&done.files_changed.len().to_string()],
                        )));
                    }
                    Ok(done) => {
                        alert.setMessageText(&ns(t("Zurückspulen fehlgeschlagen")));
                        alert.setInformativeText(&ns(&done.error.unwrap_or_default()));
                    }
                    Err(error) => {
                        alert.setMessageText(&ns(t("Zurückspulen fehlgeschlagen")));
                        alert.setInformativeText(&ns(&error.to_string()));
                    }
                }
                alert.runModal();
            });
        });
    }
}

#[cfg(feature = "selftest")]
thread_local! {
    /// The answers the self-test gives the rewind's questions, first first, and the questions it saw.
    static ALERTS: std::cell::RefCell<(std::collections::VecDeque<objc2_app_kit::NSModalResponse>, Vec<String>)> =
        std::cell::RefCell::new(Default::default());
}

/// Has the self-test answer the next questions of a code rewind with `answers` instead of the user.
#[cfg(feature = "selftest")]
pub fn answer_alerts(answers: &[objc2_app_kit::NSModalResponse]) {
    ALERTS.with(|a| a.borrow_mut().0.extend(answers.iter().copied()));
}

/// The questions a code rewind asked since the self-test began answering them.
#[cfg(feature = "selftest")]
pub fn alerts_seen() -> Vec<String> {
    ALERTS.with(|a| a.borrow().1.clone())
}

/// Shows `alert` and waits for the answer; in the self-test, the answer it set up, if any.
fn run_alert(alert: &NSAlert) -> objc2_app_kit::NSModalResponse {
    #[cfg(feature = "selftest")]
    {
        let answer = ALERTS.with(|a| {
            let mut alerts = a.borrow_mut();
            let answer = alerts.0.pop_front();
            if answer.is_some() {
                alerts.1.push(alert.messageText().to_string());
            }
            answer
        });
        if let Some(answer) = answer {
            return answer;
        }
    }
    alert.runModal()
}

/// Puts `text` on the general pasteboard.
pub(super) fn copy_text(text: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(&ns(text), crate::util::constant!(objc2_app_kit::NSPasteboardTypeString));
}

/// The tag of a menu item that stands for the `n`th thing offered.
fn index_tag(n: usize) -> isize {
    isize::try_from(n).unwrap_or(-1)
}

/// A file as an attachment: pictures as images, anything else as a mention of its path.
pub(super) fn file_attachment(path: &Path) -> Attachment {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let media_type = match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    };
    if let Some(media_type) = media_type
        && let Ok(data) = std::fs::read(path)
    {
        return Attachment::Image {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()),
            media_type: media_type.into(),
            data: data.into(),
        };
    }
    Attachment::File {
        path: path.to_path_buf(),
        lines: None,
    }
}

/// Pasted picture data as PNG.
fn as_png(data: &[u8]) -> Option<Vec<u8>> {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some(data.to_vec());
    }
    let bytes = objc2_foundation::NSData::with_bytes(data);
    let rep = NSBitmapImageRep::imageRepWithData(&bytes)?;
    // SAFETY: An empty properties dictionary is valid for PNG.
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
    Some(png.to_vec())
}

/// Asks for a line of text; `None` when the user cancels or leaves it empty.
pub fn ask_text(mtm: MainThreadMarker, title: &str, current: &str) -> Option<String> {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(title));
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = objc2_app_kit::NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(objc2_foundation::NSRect::new(
        NSPoint::new(0.0, 0.0),
        objc2_foundation::NSSize::new(300.0, 24.0),
    ));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    if alert.runModal() != NSAlertFirstButtonReturn {
        return None;
    }
    let text = field.stringValue().to_string().trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_read_as_people_say_them() {
        assert_eq!(model_name("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_name("claude-haiku-5-5[1m]"), "Haiku 5.5");
        assert_eq!(model_name("claude-sonnet-4-5-20250929"), "Sonnet 4.5");
        assert_eq!(model_name("opus"), "Opus");
    }

    #[test]
    fn pictures_attach_as_images_and_other_files_as_paths() {
        let dir = std::env::temp_dir().join(format!("ide-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let picture = dir.join("shot.png");
        std::fs::write(&picture, [0x89, b'P', b'N', b'G']).unwrap();
        assert!(matches!(file_attachment(&picture), Attachment::Image { .. }));
        assert!(matches!(file_attachment(&dir.join("main.rs")), Attachment::File { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
