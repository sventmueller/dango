//! A terminal view's shell: starting it, what it reports, and what the view does about it: the title, the bell, the
//! banners, the end of the program, clearing and selecting.

use std::time::{Duration, Instant};

use ide_process::Supervisor;
use ide_terminal::{Clipboard, Event, Foreground, Options, Session};
use objc2::DefinedClass;
use objc2_app_kit::{NSBeep, NSPasteboard, NSWorkspace};
use splat_i18n::{t, tf};

use super::view::TerminalView;
use crate::model::{self, BELL_FLASH, Bell, Frame};
use ide_ui_common::util::main_after;

/// How long a notice shows above the terminal.
const NOTICE_TIME: Duration = Duration::from_secs(8);

impl TerminalView {
    /// Starts the shell with the login shell's environment `env`.
    pub fn spawn(&self, env: &[(String, String)]) {
        let mut options = Options::new(self.ivars().cwd.borrow().clone());
        options.palette = self.ivars().theme.borrow().palette.clone();
        if let Some(size) = self.grid_size() {
            options.size = size;
            self.ivars().state.size.set(Some(size));
        }
        match Session::spawn(Supervisor::global(), env, options, super::listener(self.id())) {
            Ok(session) => {
                session.focus(self.ivars().state.focused.get());
                *self.ivars().session.borrow_mut() = Some(session);
                self.layout_grid();
                self.refresh(true);
            }
            Err(error) => {
                ide_ui_common::diag::report("starting a terminal", &error);
                self.notify(&tf(
                    "Das Terminal konnte nicht starten: {}",
                    &[&ide_core::explain::Explain::explain(&error)],
                ));
            }
        }
    }

    /// Runs `f` with the session once it has started.
    pub fn with_session<R>(&self, f: impl FnOnce(&Session) -> R) -> Option<R> {
        self.ivars().session.borrow().as_ref().map(f)
    }

    /// Reacts to what the session reports.
    pub(super) fn handle(&self, event: Event) {
        match event {
            Event::Redraw => self.redraw_requested(),
            Event::Title(title) => self.set_title(title),
            Event::Bell => self.bell(),
            Event::ClipboardStore {
                clipboard: Clipboard::Clipboard,
                text,
            } => super::clip::store(&text),
            // A Mac has no primary selection, and copying into it on every selection would overwrite the pasteboard.
            Event::ClipboardStore { .. } => {}
            Event::ClipboardLoad(_) => {
                let text = super::clip::text(&NSPasteboard::generalPasteboard()).unwrap_or_default();
                self.with_session(|session| session.answer_clipboard(&text));
            }
            Event::WorkingDirectory(dir) => {
                if dir.is_local() {
                    *self.ivars().cwd.borrow_mut() = dir.path;
                }
            }
            Event::Prompt => {
                let prompts = &self.ivars().state.prompts;
                prompts.set(prompts.get() + 1);
            }
            Event::CommandStarted | Event::CommandFinished { .. } => {}
            Event::DevServer(url) => self.show_dev_server(url),
            Event::Exited(_) => self.exited(),
        }
    }

    fn set_title(&self, title: Option<String>) {
        *self.ivars().title.borrow_mut() = model::shown_title(title);
        self.show_title();
    }

    /// Gives the tab the title when it changed, at most every `TITLE_GAP`.
    fn show_title(&self) {
        let Some(tab) = self.tab() else { return };
        let state = &self.ivars().state;
        let title = self
            .ivars()
            .title
            .borrow()
            .clone()
            .unwrap_or_else(|| t("Terminal").to_string());
        if state.tab_title.borrow().as_deref() == Some(title.as_str()) {
            return;
        }
        match state.titles.ask(Instant::now(), model::TITLE_GAP) {
            Frame::Now => {
                super::host().set_tab_title(self.window_id(), tab, &title);
                *state.tab_title.borrow_mut() = Some(title);
            }
            Frame::After(wait) => {
                let id = self.id();
                main_after(wait, move || {
                    if let Some(view) = super::find(id) {
                        view.ivars().state.titles.pending.set(false);
                        view.show_title();
                    }
                });
            }
            Frame::Pending | Frame::Stale => {}
        }
    }

    /// The tab's id, once the tab exists; a title that arrived before is shown now.
    pub fn set_tab(&self, id: isize) {
        self.ivars().tab.set(Some(id));
        self.ivars().state.tab_title.borrow_mut().take();
        if self.ivars().title.borrow().is_some() {
            self.show_title();
        }
    }

    pub fn label(&self) -> String {
        model::label(self.ivars().title.borrow().as_deref())
    }

    fn bell(&self) {
        let visible = !self.isHiddenOrHasHiddenAncestor() && self.window().is_some_and(|w| w.isKeyWindow());
        let reduce_motion = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
        match self.ivars().state.bell(Instant::now(), visible, reduce_motion) {
            Bell::Ignore => return,
            Bell::Beep => return NSBeep(),
            Bell::Flash(_) => {}
        }
        self.setNeedsDisplay(true);
        let id = self.id();
        main_after(BELL_FLASH, move || {
            if let Some(view) = super::find(id) {
                view.setNeedsDisplay(true);
            }
        });
    }

    fn show_dev_server(&self, url: String) {
        self.with_banners(|banners| {
            banners.dev_server.show(&tf("Dev-Server läuft: {}", &[&url]));
            *banners.url.borrow_mut() = Some(url);
        });
    }

    pub(super) fn open_dev_server(&self) {
        let mut url = None;
        self.with_banners(|banners| {
            url = banners.url.borrow().clone();
            banners.dev_server.hide();
        });
        if let Some(url) = url {
            super::host().opened(self.window_id(), super::Opened::Url(url));
        }
    }

    /// Shows `text` above the terminal for a few seconds.
    pub fn notify(&self, text: &str) {
        let shown = self.ivars().state.next_notice();
        self.with_banners(|banners| banners.notice.show(text));
        let id = self.id();
        main_after(NOTICE_TIME, move || {
            if let Some(view) = super::find(id)
                && view.ivars().state.is_last_notice(shown)
            {
                view.with_banners(|banners| banners.notice.hide());
            }
        });
    }

    fn exited(&self) {
        self.ivars().state.exited.set(true);
        self.update_blink();
        self.layout_grid();
        self.setNeedsDisplay(true);
    }

    pub(super) fn clear(&self) {
        self.with_session(|session| {
            let shell = matches!(session.foreground(), Foreground::Shell(_));
            session.clear(shell && !session.input_modes().alternate_screen);
        });
        self.refresh(true);
    }

    pub(super) fn select_all(&self) {
        let Some(size) = self.ivars().state.size.get() else {
            return;
        };
        self.with_session(|session| {
            let corner = |line: u16, column: u16| ide_terminal::Point {
                line: usize::from(line),
                column: usize::from(column),
            };
            session.scroll(ide_terminal::Scroll::Top);
            session.select(
                ide_terminal::SelectionKind::Simple,
                corner(0, 0),
                ide_terminal::CellSide::Left,
            );
            session.scroll(ide_terminal::Scroll::Bottom);
            let end = corner(size.lines.saturating_sub(1), size.columns.saturating_sub(1));
            session.extend_selection(end, ide_terminal::CellSide::Right);
        });
        self.refresh(true);
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        self.with_session(Session::selected_text)
            .flatten()
            .filter(|text| !text.is_empty())
    }
}
