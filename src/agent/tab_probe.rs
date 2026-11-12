//! What the self-test reads from and does with an AI tab.

use std::path::PathBuf;
use std::rc::Rc;

use ide_thread::{Thread, ThreadId};
use objc2::rc::Retained;
use objc2_app_kit::{NSStackView, NSView};

use super::acp::Agent;
use super::source;
use super::tab::AgentTab;
use crate::window::WindowController;

/// What the self-test reads from a tab.
pub struct Inspection {
    pub rows: usize,
    pub built: usize,
    pub visible: Vec<String>,
    pub pills: Vec<String>,
    pub composer: bool,
    pub status: Vec<String>,
    pub title: String,
    pub subagents: Vec<String>,
    pub background: Vec<String>,
    /// The tooltip of the head's Remote Control button, while it shows.
    pub remote: Option<String>,
}

impl AgentTab {
    /// A tab that plays recordings, with no process behind it.
    pub fn open_replay(controller: &WindowController) -> Option<Rc<AgentTab>> {
        let cwd = controller.project_root().or_else(|| controller.project())?;
        Some(AgentTab::open_replay_in(controller, cwd))
    }

    /// A tab that plays recordings in `cwd`, for the self-test.
    pub fn open_replay_in(controller: &WindowController, cwd: PathBuf) -> Rc<AgentTab> {
        let thread_id = ThreadId::new();
        let thread = Thread::new(thread_id.clone(), ide_claude::AGENT, &cwd);
        let source = source::replay(thread.clone());
        let tab = Rc::new(AgentTab::build(
            controller,
            Agent::Claude,
            cwd,
            thread_id,
            thread,
            source,
        ));
        tab.add_to_window(controller);
        tab
    }

    /// Plays recorded lines.
    pub fn feed(&self, lines: &str) {
        let (changes, snapshot) = {
            let mut state = self.state.borrow_mut();
            let changes = state.source.feed(lines);
            (changes, state.source.snapshot())
        };
        if let Some(snapshot) = snapshot {
            self.absorb(snapshot, &changes);
        }
    }

    /// The tooltip of the head's button for `part`, whether it can be clicked and whether it shows the accent colour.
    pub fn work_button(&self, part: crate::activity::Part) -> (String, bool, bool) {
        let button = self.header.work_button(part);
        let tip = button.toolTip().map(|t| t.to_string()).unwrap_or_default();
        let accent = button
            .contentTintColor()
            .is_some_and(|c| c == objc2_app_kit::NSColor::controlAccentColor());
        (tip, button.isEnabled(), accent)
    }

    /// Shows `title` in the head for a moment and measures it as [`Header::title_widths`] does.
    pub fn title_widths(&self, title: &str) -> (f64, f64, f64) {
        self.header.set_title(title);
        let widths = self.header.title_widths();
        self.refresh();
        widths
    }

    pub fn inspect(&self) -> Inspection {
        let thread = self.snapshot();
        Inspection {
            rows: self.timeline.count(),
            built: self.timeline.built(),
            visible: self.timeline.visible_summaries(),
            pills: [&self.composer.model, &self.composer.mode]
                .iter()
                .map(|b| b.title().to_string())
                .collect(),
            composer: self.composer.card.window().is_some() && !self.composer.card.isHidden(),
            status: self.status.texts(),
            title: self.title(),
            subagents: thread.subagents().iter().map(|s| s.description.clone()).collect(),
            background: thread.background().iter().map(|b| b.label.clone()).collect(),
            remote: self.header.remote_tip(),
        }
    }

    /// Keeps the thread off Remote Control whatever the user's Claude settings say, so a live self-test registers
    /// nothing on claude.ai.
    pub fn keep_remote_control_off(&self) {
        self.state.borrow_mut().remote = Some(false);
    }

    /// Picks the model for the next start.
    pub fn use_model(&self, model: &str) {
        self.state.borrow_mut().model = Some(model.to_string());
    }

    /// Picks `model`, `effort` and `mode` as the pills' menus do, which remembers them for the agent's next sessions.
    pub fn pick_all(&self, model: &str, effort: &str, mode: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.offered.models = vec![model.to_string()];
            state.offered.efforts = vec![effort.to_string()];
            state.offered.modes = vec![mode.to_string()];
        }
        self.pick_model(0);
        self.pick_effort(0);
        self.pick_mode(0);
    }

    /// Picks the mode `mode` as the mode pill's menu does, which remembers it for the agent's next sessions.
    pub fn pick_mode_id(&self, mode: &str) {
        self.state.borrow_mut().offered.modes = vec![mode.to_string()];
        self.pick_mode(0);
    }

    /// The model, effort and mode the session runs with or starts with.
    pub fn picks(&self) -> (Option<String>, Option<String>, String) {
        let state = self.state.borrow();
        (state.model.clone(), state.effort.clone(), state.mode.clone())
    }

    /// Switches to the permission mode `mode`, as the mode pill's menu does.
    pub fn use_mode(&self, mode: &str) {
        self.state.borrow().source.set_mode(mode, |_| ());
        self.state.borrow_mut().mode = mode.to_string();
        self.refresh_composer();
    }

    /// The views that show at the foot of the tab besides the composer and its status line, with their heights.
    pub fn foot_extras(&self) -> Option<Vec<(String, f64)>> {
        // SAFETY: Asking for the superview only reads; the foot holds the composer while the tab lives.
        let foot = unsafe { self.composer.card.superview() }.and_then(|v| v.downcast::<NSStackView>().ok())?;
        foot.layoutSubtreeIfNeeded();
        let own: [&NSView; 2] = [&self.composer.card, &self.status.view];
        Some(
            foot.arrangedSubviews()
                .iter()
                .filter(|view| !view.isHidden() && !own.iter().any(|o| std::ptr::eq(*o, &**view)))
                .map(|view| {
                    (
                        view.class().name().to_string_lossy().into_owned(),
                        view.frame().size.height,
                    )
                })
                .collect(),
        )
    }

    /// Types `text` into the composer and sends it, as Return would.
    pub fn type_and_send(&self, text: &str) {
        self.composer.set_text(text);
        self.submit();
    }

    /// Attaches the files at `paths` as the `+` button's panel would.
    pub fn attach_files(&self, paths: &[PathBuf]) {
        let picked: Vec<ide_thread::Attachment> = paths
            .iter()
            .map(|path| super::outside::classify(super::actions::file_attachment(path), &self.cwd))
            .collect();
        self.state.borrow_mut().attachments.extend(picked);
        self.refresh_chips();
    }

    /// What the next message takes along.
    pub fn pending_attachments(&self) -> Vec<ide_thread::Attachment> {
        self.state.borrow().attachments.clone()
    }

    /// The text of the next message's attachments from outside.
    pub fn outside_texts(&self) -> Vec<String> {
        self.state
            .borrow()
            .attachments
            .iter()
            .filter_map(|a| match a {
                ide_thread::Attachment::External { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// What the composer holds.
    pub fn composer_text(&self) -> String {
        self.composer.text()
    }

    /// Types `text` into the composer, the caret at its end, as a user would.
    pub fn type_in_composer(&self, text: &str) {
        self.composer.set_text(text);
        let end = text.encode_utf16().count();
        self.composer
            .text
            .setSelectedRange(objc2_foundation::NSRange::new(end, 0));
        self.composer_changed();
    }

    /// The completions the list shows.
    pub fn suggestion_labels(&self) -> Vec<String> {
        if self.suggestions.is_shown() {
            self.suggestions.labels()
        } else {
            Vec::new()
        }
    }

    /// Whether the editor's selection is offered as context.
    pub fn has_offer(&self) -> bool {
        self.state.borrow().offer.is_some()
    }

    /// The attachments of the next message.
    pub fn attachment_names(&self) -> Vec<String> {
        self.state
            .borrow()
            .attachments
            .iter()
            .map(|a| match a {
                ide_thread::Attachment::Image { .. } => "image".to_string(),
                ide_thread::Attachment::File { path, .. } | ide_thread::Attachment::Selection { path, .. } => {
                    path.display().to_string()
                }
                ide_thread::Attachment::External { label, .. } => format!("{label} (external)"),
            })
            .collect()
    }

    /// The size of each picture tile above the composer's text, with the width of the picture it shows.
    pub fn thumbnails(&self) -> Vec<(objc2_foundation::NSSize, f64)> {
        fn picture(view: &objc2_app_kit::NSView) -> Option<f64> {
            if let Some(image) = view
                .downcast_ref::<objc2_app_kit::NSImageView>()
                .and_then(|v| v.image())
                .filter(|image| !image.isTemplate())
            {
                return Some(image.size().width);
            }
            view.subviews().iter().find_map(|child| picture(&child))
        }
        let chips = &self.composer.chips;
        chips.layoutSubtreeIfNeeded();
        chips
            .arrangedSubviews()
            .iter()
            .filter_map(|chip| picture(&chip).map(|width| (chip.frame().size, width)))
            .collect()
    }

    /// Clicks the first picture tile above the composer's text; the size the picture opens at over the tab.
    pub fn enlarge_thumbnail(&self) -> Option<objc2_foundation::NSSize> {
        fn clear_button(view: &objc2_app_kit::NSView) -> Option<Retained<objc2_app_kit::NSButton>> {
            if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>()
                && button.isTransparent()
            {
                return Some(objc2::Message::retain(button));
            }
            view.subviews().iter().find_map(|child| clear_button(&child))
        }
        let button = clear_button(&self.composer.chips)?;
        // SAFETY: The button's target is the opener, which lives as long as the app.
        unsafe { button.performClick(None) };
        super::enlarge::shown()
    }

    /// The size of the picture open large over the tab, if one is.
    pub fn enlarged(&self) -> Option<objc2_foundation::NSSize> {
        super::enlarge::shown()
    }

    /// Whether a rewind waits for the agent to accept it.
    pub fn rewind_pending(&self) -> bool {
        self.state.borrow().fork.is_some()
    }

    /// How many checkpoints were taken before prompts.
    pub fn checkpoint_count(&self) -> usize {
        self.state.borrow().checkpoints.len()
    }

    /// Opens every card part with `part`, as a user would click them.
    pub fn open_all(&self, part: super::cards::Part) {
        let ids: Vec<u64> = self
            .snapshot()
            .entries(&ide_thread::Lane::Main)
            .iter()
            .map(|e| e.id.get())
            .collect();
        for id in ids {
            self.timeline.toggle(id, part);
        }
    }
}
