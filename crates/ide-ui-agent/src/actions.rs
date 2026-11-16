//! What the user does in an AI tab: send, queue, attach, answer requests, pick model and mode, rewind.

use ide_ui_common::window::Files;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ide_claude::{RemoteControl, Session};
use ide_thread::{Attachment, Entry, EntryKind, Status, ToolView, UserMessage};
use ide_ui_common::alert::{Alert, Role, first};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{ClassType, MainThreadMarker, sel};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSMenu, NSModalResponseOK, NSOpenPanel, NSPasteboard, NSView, NSWorkspace,
};
use objc2_foundation::{NSArray, NSDictionary, NSPoint, NSURL};
use splat_i18n::{t, tf};

use super::acp::Agent;
use super::composer::image_types;
use super::model::{effort_name, find_entry, model_name, modes};
use super::tab::AgentTab;
use super::{format, style, ui};
use crate::work::Part;
use ide_ui_common::util::{item, main_soon, ns, set_target};

/// Whether `agent` gives prompts no id of its own, as ACP agents and the own agent do, so the app's own id keys the
/// checkpoint taken before a prompt (I6).
pub(super) fn app_ids(agent: &Agent) -> bool {
    agent.is_acp() || agent.is_own()
}

pub(super) fn pop_up(menu: &NSMenu, view: &NSView) {
    let below = NSPoint::new(0.0, view.frame().size.height + 4.0);
    menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(view));
}

impl AgentTab {
    pub(super) fn entry(&self, id: u64) -> Option<Arc<Entry>> {
        find_entry(self.state.borrow().thread(), id)
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
        let has_text = !self.composer.text().trim().is_empty() || !self.state.borrow().attachments().is_empty();
        if has_text {
            self.submit();
        } else if self.status() != Status::Idle
            && !super::stop::too_soon_after_send(self.state.borrow().sent_at(), std::time::Instant::now())
        {
            self.stop_turn(super::Stop::Button);
        }
    }

    /// Sends the first queued message once the session rests.
    pub(super) fn send_queued(&self) {
        let Some(next) = self.state.borrow_mut().next_queued() else {
            return;
        };
        self.refresh_queue();
        self.send_message(next);
    }

    /// Takes a checkpoint of the folder (I6), then sends `message`.
    pub(super) fn send_message(&self, message: UserMessage) {
        if self.is_driven() {
            return self.deliver(message, None);
        }
        let own = self.state.borrow().source().tab_session().cloned();
        if let Some(driven) = own {
            return self.hand_over(&driven, message);
        }
        self.state.borrow_mut().begin_send();
        self.refresh_composer();
        let key = self.key;
        let label = format::first_line(&message.text, 80);
        let buffers = crate::host::host().unsaved_buffers(&self.cwd);
        self.store
            .create(self.thread_id.to_string(), label, buffers, move |checkpoint| {
                main_soon(move || {
                    if let Some(tab) = super::get(key) {
                        tab.deliver(message, checkpoint);
                    }
                });
            });
    }

    pub(super) fn deliver(&self, mut message: UserMessage, checkpoint: Option<ide_git::Checkpoint>) {
        let app_ids = app_ids(&self.agent);
        if app_ids && message.message_id.is_none() {
            message.message_id = Some(format!("app-{}", ide_thread::ThreadId::new()));
        }
        let own_id = message.message_id.clone().filter(|_| app_ids);
        self.state.borrow_mut().delivering(&message);
        let text = message.text.clone();
        let sent = self.state.borrow().source().send(message);
        self.delivered(&text, sent.map(|uuid| uuid.or(own_id)), checkpoint);
    }

    /// Hands `message` to the tab's own session in the registry, which takes the checkpoint before it and tells
    /// with `Signal::Delivered` that it went out.
    fn hand_over(&self, driven: &crate::driven::Driven, mut message: UserMessage) {
        if app_ids(&self.agent) && message.message_id.is_none() {
            message.message_id = Some(format!("app-{}", ide_thread::ThreadId::new()));
        }
        self.state.borrow_mut().begin_send();
        self.refresh_composer();
        let prompt = ide_session::Prompt {
            message: message.clone(),
            buffers: crate::host::host().unsaved_buffers(&self.cwd),
            ..ide_session::Prompt::default()
        };
        match driven.prompt(prompt) {
            Ok(turn) => self.state.borrow_mut().handed_over(turn, message),
            Err(error) => {
                self.state.borrow_mut().delivering(&message);
                self.delivered(&message.text, Err(error), None);
            }
        }
    }

    /// Takes in that a message of `text` went out with `checkpoint` taken before it and the id `sent` gives, or
    /// did not.
    pub(super) fn delivered(
        &self,
        text: &str,
        sent: Result<Option<String>, ide_core::explain::Explained>,
        checkpoint: Option<ide_git::Checkpoint>,
    ) {
        match sent {
            Ok(id) => {
                self.state.borrow_mut().sent(id, checkpoint);
            }
            Err(error) => {
                ide_ui_common::diag::report("sending to the agent", &error);
                if self.composer.text().is_empty() {
                    self.composer.set_text(text);
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
            self.composer.set_placeholder(state.placeholder());
            drop(state);
            return self.refresh_send(&self.composer.text());
        }
        let state = self.state.borrow();
        self.composer.model.setTitle(&ns(&state.model_pill()));
        let (mode_title, mode_symbol) = state.mode_pill();
        self.composer.mode.setTitle(&ns(&mode_title));
        self.composer
            .mode
            .setImage(ide_ui_common::util::symbol(mode_symbol, &mode_title).as_deref());
        self.composer
            .mode
            .setImagePosition(objc2_app_kit::NSCellImagePosition::ImageLeading);
        let locked = state.source().driven().is_some();
        self.composer.mode.setEnabled(!locked);
        self.composer.sandbox.setEnabled(!locked);
        let look = state.sandbox_look();
        let tint = if look.on {
            objc2_app_kit::NSColor::systemGreenColor()
        } else {
            objc2_app_kit::NSColor::secondaryLabelColor()
        };
        self.composer
            .sandbox
            .setImage(ide_ui_common::util::symbol(look.symbol, look.tip).as_deref());
        self.composer.sandbox.setContentTintColor(Some(&tint));
        self.composer.sandbox.setToolTip(Some(&ns(look.tip)));
        if locked {
            let tip = t("Der Auftrag legt Modus und Sandbox fest");
            self.composer.mode.setToolTip(Some(&ns(tip)));
            self.composer.sandbox.setToolTip(Some(&ns(tip)));
        }
        self.composer.set_placeholder(state.placeholder());
        drop(state);
        self.refresh_send(&self.composer.text());
    }

    /// The send button for `text` in the composer.
    pub(super) fn refresh_send(&self, text: &str) {
        let state = self.state.borrow();
        self.composer.show_running(state.running(), state.can_send(text));
    }

    /// The queued messages above the composer, each with a button that drops it.
    pub(super) fn refresh_queue(&self) {
        for view in self.queue_view.arrangedSubviews().iter() {
            self.queue_view.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
        let queue: Vec<String> = self.state.borrow().queue().iter().map(|m| m.text.clone()).collect();
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
        self.state.borrow_mut().remove_queued(index);
        self.refresh_queue();
    }

    pub(crate) fn remove_attachment(&self, index: usize) {
        self.state.borrow_mut().remove_attachment(index);
        self.refresh_chips();
    }

    /// Takes pictures, files and text from a web page from a paste or a drop as attachments; whether there were any.
    pub fn take_pasteboard(&self, pasteboard: &NSPasteboard) -> bool {
        let classes = NSArray::from_slice(&[NSURL::class()]);
        // SAFETY: Reading NSURL objects with no options returns NSURLs, as the class list asks.
        let urls = unsafe { pasteboard.readObjectsForClasses_options(&classes, Some(&NSDictionary::new())) };
        let paths: Vec<PathBuf> = urls
            .iter()
            .flat_map(|a| a.iter())
            .filter_map(|object| {
                let url = object.downcast_ref::<NSURL>().filter(|u| u.isFileURL())?;
                Some(PathBuf::from(url.path()?.to_string()))
            })
            .collect();
        let pictures = paths.iter().any(|p| picture_type(p).is_some());
        let mut taken = self.take_files(paths);
        if taken.is_empty() && !pictures {
            taken.extend(super::outside::web_paste(pasteboard));
        }
        if taken.is_empty() && !pictures {
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
            return pictures;
        }
        self.state.borrow_mut().attach(taken);
        self.refresh_chips();
        true
    }

    /// The files at `paths` as attachments, but for the pictures, which are read on the disk lane and come later.
    fn take_files(&self, paths: Vec<PathBuf>) -> Vec<Attachment> {
        let (pictures, files): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|path| picture_type(path).is_some());
        if !pictures.is_empty() {
            let (key, cwd) = (self.key, self.cwd.clone());
            let read = move || {
                let attach = |path: &PathBuf| super::outside::classify(file_attachment(path), &cwd);
                pictures.iter().map(attach).collect::<Vec<_>>()
            };
            let queued = ide_core::lane::DISK.ask(read, move |read| {
                main_soon(move || {
                    if let (Some(tab), Some(read)) = (super::get(key), read) {
                        tab.state.borrow_mut().attach(read);
                        tab.refresh_chips();
                    }
                });
            });
            if queued.is_err() {
                ide_ui_common::diag::report("reading pasted pictures", "the disk lane is full");
            }
        }
        let attach = |path: PathBuf| super::outside::classify(Attachment::File { path, lines: None }, &self.cwd);
        files.into_iter().map(attach).collect()
    }

    /// `+`: files and pictures from an open panel.
    pub(crate) fn attach(&self) {
        let panel = NSOpenPanel::openPanel(self.mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(true);
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&ns(&self.cwd.to_string_lossy()))));
        if panel.runModal() != NSModalResponseOK {
            return;
        }
        let paths = panel
            .URLs()
            .iter()
            .filter_map(|u| u.path())
            .map(|p| PathBuf::from(p.to_string()))
            .collect();
        let picked = self.take_files(paths);
        self.state.borrow_mut().attach(picked);
        self.refresh_chips();
    }

    /// `/`: the agent's slash commands.
    pub(crate) fn slash_menu(&self, view: &NSView) {
        pop_up(&self.slash_items(), view);
    }

    /// The slash commands the agent offers and the overlays' commands (E146), with their descriptions.
    pub(super) fn known_commands(&self) -> Vec<(String, String)> {
        let (caps, codex) = {
            let state = self.state.borrow();
            (state.acp_caps().cloned(), state.source().codex().is_some())
        };
        super::dialogs::with_local(self.agent_commands(), &self.agent, caps.as_ref(), codex)
    }

    /// The slash commands the agent offers itself.
    pub(crate) fn agent_commands(&self) -> Vec<(String, String)> {
        if self.agent.is_acp() {
            self.acp_commands()
        } else if self.agent.is_own() {
            vec![(
                "compact".to_string(),
                t("Ältere Züge zu einer Zusammenfassung verdichten").to_string(),
            )]
        } else {
            let state = self.state.borrow();
            let Some(caps) = state.claude_caps() else {
                return Vec::new();
            };
            if caps.commands.is_empty() {
                caps.slash_commands.iter().map(|c| (c.clone(), String::new())).collect()
            } else {
                caps.commands
                    .iter()
                    .map(|c| (c.name.clone(), c.description.clone()))
                    .collect()
            }
        }
    }

    /// The slash menu, without showing it.
    pub(super) fn slash_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let commands = self.known_commands();
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
        self.state.borrow_mut().offered_mut().commands = commands.into_iter().map(|(n, _)| n).collect();
        menu
    }

    pub(crate) fn pick_command(&self, index: usize) {
        let Some(name) = self.state.borrow().offered().commands.get(index).cloned() else {
            return;
        };
        self.composer.set_text(&format!("/{name} "));
        self.focus_composer();
        self.refresh_composer();
    }

    /// The model and effort pill: the models the CLI offers, then the effort levels of the current one.
    pub(crate) fn model_menu(&self, view: &NSView) {
        pop_up(&self.model_items(), view);
    }

    /// The model pill's menu, without showing it.
    pub(super) fn model_items(&self) -> Retained<NSMenu> {
        if self.agent.is_acp() {
            return self.acp_model_items();
        }
        if self.agent.is_own() {
            return self.own_model_items();
        }
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let (models, efforts, current, effort) = {
            let state = self.state.borrow();
            let current = state
                .thread()
                .model()
                .map(str::to_string)
                .or_else(|| state.picks().model().map(str::to_string));
            let known = state.claude_caps().map_or(&[][..], |caps| &caps.models[..]);
            let models: Vec<(String, String, String)> = known
                .iter()
                .map(|m| (m.value.clone(), m.display_name.clone(), m.description.clone()))
                .collect();
            let supported = known
                .iter()
                .find(|m| Some(&m.value) == current.as_ref())
                .map(|m| m.supported_effort_levels.clone())
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| ["low", "medium", "high", "xhigh", "max"].map(str::to_string).to_vec());
            let effort = state
                .thread()
                .effort()
                .map(str::to_string)
                .or_else(|| state.picks().effort().map(str::to_string));
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
        let driven = self.is_driven();
        for (n, level) in efforts.iter().enumerate() {
            let entry = self.menu_item(&menu, &effort_name(level), sel!(ideAgentPickEffort:), index_tag(n));
            entry.setEnabled(!driven);
            if Some(level) == effort.as_ref() {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        {
            let mut state = self.state.borrow_mut();
            state.offered_mut().models = models.into_iter().map(|(v, _, _)| v).collect();
            state.offered_mut().efforts = efforts;
        }
        menu
    }

    pub(crate) fn pick_model(&self, index: usize) {
        if self.agent.is_acp() {
            return self.acp_pick_model(index);
        }
        if self.agent.is_own() {
            return self.own_pick_model(index);
        }
        let Some(model) = self.state.borrow().offered().models.get(index).cloned() else {
            return;
        };
        self.state.borrow().source().set_model(&model);
        if !self.is_driven() {
            super::choices::remember(self.agent.id(), |choice| choice.model = Some(model.clone()));
        }
        self.state.borrow_mut().pick_model(model);
        self.refresh_composer();
    }

    pub(crate) fn pick_effort(&self, index: usize) {
        if self.is_driven() {
            return;
        }
        if self.agent.is_own() {
            return self.own_pick_effort(index);
        }
        let Some(level) = self.state.borrow().offered().efforts.get(index).cloned() else {
            return;
        };
        self.state.borrow().source().set_effort(&level);
        super::choices::remember(self.agent.id(), |choice| choice.effort = Some(level.clone()));
        self.state.borrow_mut().pick_effort(level);
        self.refresh_composer();
    }

    /// The mode pill: how freely Claude may use tools.
    pub(crate) fn mode_menu(&self, view: &NSView) {
        pop_up(&self.mode_items(), view);
    }

    /// The mode pill's menu, without showing it.
    pub(super) fn mode_items(&self) -> Retained<NSMenu> {
        if self.agent.is_acp() {
            return self.acp_mode_items();
        }
        if self.agent.is_own() {
            return self.own_mode_items();
        }
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let current = {
            let state = self.state.borrow();
            state
                .thread()
                .mode()
                .map(str::to_string)
                .unwrap_or_else(|| state.picks().mode().to_string())
        };
        for (n, (id, title, symbol)) in modes().iter().enumerate() {
            let entry = self.menu_item(&menu, title, sel!(ideAgentPickMode:), index_tag(n));
            entry.setImage(ide_ui_common::util::symbol(symbol, title).as_deref());
            if *id == current || (current == "manual" && *id == "default") {
                entry.setState(objc2_app_kit::NSControlStateValueOn);
            }
        }
        self.state.borrow_mut().offered_mut().modes = modes().iter().map(|(id, _, _)| (*id).to_string()).collect();
        menu
    }

    pub(crate) fn pick_mode(&self, index: usize) {
        if self.agent.is_acp() {
            return self.acp_pick_mode(index);
        }
        if self.agent.is_own() {
            return self.own_pick_mode(index);
        }
        if self.is_driven() {
            return;
        }
        let Some(mode) = self.state.borrow().offered().modes.get(index).cloned() else {
            return;
        };
        let key = self.key;
        self.state.borrow().source().set_mode(&mode, move |why| {
            ide_ui_common::util::main_soon(move || {
                if let Some(tab) = super::get(key) {
                    tab.mode_refused(&why);
                }
            });
        });
        super::choices::remember(self.agent.id(), |choice| choice.mode = Some(mode.clone()));
        self.state.borrow_mut().pick_mode(mode);
        self.refresh_composer();
    }

    /// The CLI kept its mode instead of the one picked: the composer shows the mode it runs in, the next sessions start
    /// with that one, and an alert gives the CLI's reason.
    pub(super) fn mode_refused(&self, why: &str) {
        let now = match self.state.borrow().thread().mode() {
            None | Some("manual") => "default".to_string(),
            Some(mode) => mode.to_string(),
        };
        super::choices::remember(self.agent.id(), |choice| choice.mode = Some(now.clone()));
        self.state.borrow_mut().pick_mode(now);
        self.refresh_composer();
        ide_ui_common::alert::warn(self.mtm, t("Freigabemodus nicht gewechselt"), why);
    }

    /// The sandbox switch (E19): the CLI takes it at start, so a resting session starts again with it.
    pub fn toggle_sandbox(&self) {
        if self.is_driven() {
            return;
        }
        if self.acp_agent().is_some_and(|a| !a.sandbox.is_available()) {
            return;
        }
        if self.agent.is_own() {
            return self.own_toggle_sandbox();
        }
        let restart = {
            let mut state = self.state.borrow_mut();
            state.toggle_sandbox();
            state.source().is_live() && state.status() == Status::Idle
        };
        self.refresh_composer();
        if restart {
            let source = self.state.borrow_mut().end();
            source.end();
            self.start();
        }
    }

    pub fn toggle_bookmark(&self) {
        let on = self.state.borrow_mut().toggle_bookmark();
        crate::agent_panel::set_bookmarked(&self.thread_id, on);
        self.refresh();
        if let Some(controller) = self.controller() {
            super::changed(&controller);
        }
    }

    /// The clock: sessions of this folder from the index and, experimentally, from the CLI's own transcripts.
    pub(crate) fn history_menu(&self, view: &NSView) {
        pop_up(&self.history_items(), view);
    }

    /// The clock's menu, without showing it.
    pub(super) fn history_items(&self) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let past = crate::agent_panel::past_sessions_cached(&self.cwd, 20);
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
        self.state.borrow_mut().offered_mut().past = past.into_iter().map(|(r, _)| r).collect();
        menu
    }

    pub(crate) fn open_past(&self, index: usize) {
        let Some(resume) = self.state.borrow().offered().past.get(index).cloned() else {
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

    pub(crate) fn set_title(&self, name: &str) {
        self.state.borrow_mut().set_custom_title(name.to_string());
        crate::agent_panel::rename(&self.thread_id, name);
        self.persist();
        self.refresh();
        if let Some(controller) = self.controller() {
            super::changed(&controller);
        }
    }

    pub(crate) fn trust_folder(&self) {
        crate::host::host().trust(&self.cwd);
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
            crate::host::host().open_terminal(controller.id(), &self.cwd);
        }
    }

    /// Opens the list of `part` of the session below `anchor`, else below the head's button for it.
    pub fn show_work(&self, part: Part, anchor: Option<&NSView>) {
        let Some(controller) = self.controller() else { return };
        let fallback = ui::view(self.header.work_button(part));
        crate::host::host().show_work(controller.id(), self.key, part, anchor.unwrap_or(&fallback));
    }

    /// Whether the list of `part` has anything to show, as its button in the head says.
    pub fn has_work(&self, part: Part) -> bool {
        matches!(self.agent, Agent::Claude) && crate::work::tally(&self.view_snapshot(), part).listed > 0
    }

    /// Brings the head's buttons for the session's subagents and background processes up to date.
    pub(super) fn refresh_work(&self) {
        let thread = self.view_snapshot();
        let silent = (!matches!(self.agent, Agent::Claude)).then(|| self.agent.name());
        for part in [Part::Subagents, Part::Background] {
            self.header.set_work(part, crate::work::tally(&thread, part), silent);
        }
    }

    /// Shows only the conversation, without tools, thinking and status lines, or everything again.
    pub fn toggle_focus_view(&self) {
        let on = !self.timeline().focus_view();
        self.timeline().set_focus_view(on);
        self.header.set_focus_view(on);
    }

    /// The phone in the head: connects the session to Remote Control, or offers its page and its end.
    pub fn remote_control(&self, view: &NSView) {
        if let Some(session) = self.codex_session() {
            self.codex_remote_click(&session, view);
            return;
        }
        let Some(session) = self.session() else { return };
        match session.remote_control() {
            RemoteControl::Off | RemoteControl::Failed(_) => self.set_remote_control(&session, true),
            RemoteControl::Connected(_) => pop_up(&self.remote_items(), view),
            RemoteControl::Connecting | RemoteControl::Unavailable => {}
        }
    }

    /// What the phone offers while the session is connected: its page and its end, or for Codex pairing and the end.
    pub(super) fn remote_items(&self) -> Retained<NSMenu> {
        if self.codex_session().is_some() {
            return self.codex_remote_items();
        }
        let menu = NSMenu::new(self.mtm);
        menu.setAutoenablesItems(false);
        let action = sel!(ideAgentRemoteControlAction:);
        self.menu_item(&menu, t("Im Browser öffnen"), action, REMOTE_OPEN);
        self.menu_item(&menu, t("Link kopieren"), action, REMOTE_COPY);
        menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(self.mtm));
        self.menu_item(&menu, t("Remote Control beenden"), action, REMOTE_END);
        menu
    }

    /// An item of the Remote Control menu.
    pub(crate) fn remote_control_action(&self, action: isize) {
        if let Some(session) = self.codex_session() {
            self.codex_remote_action(&session, action);
            return;
        }
        let Some(session) = self.session() else { return };
        let RemoteControl::Connected(url) = session.remote_control() else {
            return;
        };
        match action {
            REMOTE_OPEN => {
                if let Some(url) = NSURL::URLWithString(&ns(&url)) {
                    NSWorkspace::sharedWorkspace().openURL(&url);
                }
            }
            REMOTE_COPY => copy_text(&url),
            REMOTE_END => self.set_remote_control(&session, false),
            _ => {}
        }
    }

    /// Connects or disconnects, and keeps the choice for when the thread's session starts again.
    fn set_remote_control(&self, session: &Session, on: bool) {
        self.state.borrow_mut().set_remote(on);
        session.set_remote_control(on);
    }

    /// Copies the answer `id` as markdown.
    pub(crate) fn copy_entry(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::AgentMessage(message) = &entry.kind else {
            return;
        };
        copy_text(&message.markdown);
    }

    /// Opens the file a Read, Edit or Write card names.
    pub(crate) fn open_path_of(&self, id: u64) {
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
}

/// Puts `text` on the general pasteboard.
const REMOTE_OPEN: isize = 0;
const REMOTE_COPY: isize = 1;
const REMOTE_END: isize = 2;

pub(super) fn copy_text(text: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(
        &ns(text),
        ide_ui_common::constant!(objc2_app_kit::NSPasteboardTypeString),
    );
}

/// The tag of a menu item that stands for the `n`th thing offered.
fn index_tag(n: usize) -> isize {
    isize::try_from(n).unwrap_or(-1)
}

/// The media type of the picture at `path`, by its extension; `None` for any other file.
fn picture_type(path: &Path) -> Option<&'static str> {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// A file as an attachment: pictures as images, read from disk, anything else as a mention of its path.
pub(super) fn file_attachment(path: &Path) -> Attachment {
    if let Some(media_type) = picture_type(path)
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
pub(crate) fn ask_text(mtm: MainThreadMarker, title: &str, current: &str) -> Option<String> {
    let field = objc2_app_kit::NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(objc2_foundation::NSRect::new(
        NSPoint::new(0.0, 0.0),
        objc2_foundation::NSSize::new(300.0, 24.0),
    ));
    let answer = Alert::new(mtm, title)
        .button(t("OK"), Role::Plain)
        .button(t("Abbrechen"), Role::Plain)
        .accessory(&field)
        .focus(&field)
        .run_modal();
    if !first(answer) {
        return None;
    }
    let text = field.stringValue().to_string().trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_pictures_of_other_formats_become_png_and_other_data_is_refused() {
        let gif = [
            0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff,
            0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
            0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
        ];
        let png = as_png(&gif).expect("a GIF converts");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        assert_eq!(as_png(&png), Some(png.clone()), "PNG data stays as it is");
        assert_eq!(as_png(b"not a picture"), None);
    }

    #[test]
    fn pictures_attach_as_images_and_other_files_as_paths() {
        let dir = ide_core::testing::scratch("attach");
        let picture = dir.join("shot.png");
        std::fs::write(&picture, [0x89, b'P', b'N', b'G']).unwrap();
        assert!(matches!(file_attachment(&picture), Attachment::Image { .. }));
        assert!(matches!(file_attachment(&dir.join("main.rs")), Attachment::File { .. }));
    }
}
