//! The app as the `ide-ui-*` crates see it: each crate's `Host`, answered from the app context (E99).

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ide_ui_common::window::{self, Panel, TabId, TabKind, WindowId};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{NSColor, NSView, NSWindow};

use crate::window::WindowController;

/// The running app, as the crates' hooks reach it.
pub(crate) struct AppHost;

impl ide_ui_export::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_export::State> {
        context().exports().clone()
    }
}

impl ide_ui_terminal::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_terminal::State> {
        context().terminal_views().clone()
    }

    fn try_state(&self) -> Option<Rc<ide_ui_terminal::State>> {
        try_context().map(|cx| cx.terminal_views().clone())
    }

    fn opened(&self, window: WindowId, what: ide_ui_terminal::Opened) {
        use ide_ui_terminal::Opened;
        let Some(controller) = self::window(window) else {
            return;
        };
        match what {
            Opened::Url(url) => crate::browser::open_tab(&controller, Some(url.as_str())),
            Opened::File { path, line, column } => crate::editor::reveal_position(&controller, &path, line, column),
            Opened::TerminalHere(cwd) => crate::terminal::open_tab(&controller, Some(cwd.as_path())),
        }
    }
}

#[cfg(feature = "dictation")]
impl ide_ui_dictation::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_dictation::State> {
        context().dictation().clone()
    }
}

/// The app context the hooks answer from; with `try_context` the one place the crates' calls look it up.
pub(crate) fn context() -> Rc<crate::context::AppContext> {
    crate::context::app()
}

/// `context`, unless the app has none (yet).
fn try_context() -> Option<Rc<crate::context::AppContext>> {
    crate::context::try_app()
}

/// The window controller with `id` until it is dropped from the open windows, which happens once AppKit is done
/// closing its window, as a weak reference to it would load until then.
pub(crate) fn window(id: WindowId) -> Option<Retained<WindowController>> {
    try_context()?.windows().get(id.get())
}

impl window::IdentityHost for AppHost {
    fn is_open(&self, window: WindowId) -> bool {
        self::window(window).is_some()
    }

    fn frontmost(&self) -> Option<WindowId> {
        crate::window::frontmost(MainThreadMarker::new()?).map(|c| c.id())
    }

    fn for_project(&self, root: &Path) -> Option<WindowId> {
        crate::window::for_project(root).map(|c| c.id())
    }

    fn ns_window(&self, window: WindowId) -> Option<Retained<NSWindow>> {
        self::window(window)?.window()
    }

    fn project_root(&self, window: WindowId) -> Option<PathBuf> {
        self::window(window)?.project_root()
    }

    fn project(&self, window: WindowId) -> Option<PathBuf> {
        self::window(window)?.project()
    }

    fn tab_windows(&self, window: WindowId) -> Vec<Retained<NSWindow>> {
        self::window(window).map(|c| c.tab_windows()).unwrap_or_default()
    }
}

impl window::TrustHost for AppHost {
    fn is_trusted(&self, window: WindowId) -> bool {
        self::window(window).is_some_and(|c| c.is_trusted())
    }

    fn ask_trust_again(&self, window: WindowId) {
        if let Some(c) = self::window(window) {
            c.ask_trust_again();
        }
    }
}

impl window::TabsHost for AppHost {
    fn add_tab(
        &self,
        window: WindowId,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Option<TabId> {
        Some(self::window(window)?.add_tab(kind, key, title, view))
    }

    fn add_preview_tab(
        &self,
        window: WindowId,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Option<TabId> {
        Some(self::window(window)?.add_preview_tab(kind, key, title, view))
    }

    fn keep_tab(&self, window: WindowId, tab: TabId) {
        if let Some(c) = self::window(window) {
            c.keep_tab(tab);
        }
    }

    fn select_tab_with_key(&self, window: WindowId, key: &str) -> bool {
        self::window(window).is_some_and(|c| c.select_tab(key))
    }

    fn set_tab_title(&self, window: WindowId, tab: TabId, title: &str) {
        if let Some(c) = self::window(window) {
            c.set_tab_title(tab, title);
        }
    }

    fn select_tab(&self, window: WindowId, tab: TabId) {
        if let Some(c) = self::window(window) {
            c.select_tab_by_id(tab);
        }
    }

    fn close_tab(&self, window: WindowId, tab: TabId) {
        if let Some(c) = self::window(window) {
            c.close_tab(tab);
        }
    }

    fn selected_tab(&self, window: WindowId) -> Option<TabId> {
        self::window(window)?.selected_tab()
    }

    fn tab_with_key(&self, window: WindowId, key: &str) -> Option<TabId> {
        self::window(window)?.tab_with_key(key)
    }
}

impl window::TabDecorHost for AppHost {
    fn set_tab_status(&self, window: WindowId, tab: TabId, color: Option<&NSColor>) {
        if let Some(c) = self::window(window) {
            c.set_tab_status(tab, color);
        }
    }

    fn set_tab_tint(&self, window: WindowId, tab: TabId, color: &NSColor) {
        if let Some(c) = self::window(window) {
            c.set_tab_tint(tab, color);
        }
    }

    fn set_tab_four_colours(&self, window: WindowId, tab: TabId, colors: [Retained<NSColor>; 4]) {
        if let Some(c) = self::window(window) {
            c.set_tab_four_colours(tab, colors);
        }
    }
}

impl window::PanelsHost for AppHost {
    fn activate_panel(&self, window: WindowId, panel: Panel) {
        if let Some(c) = self::window(window) {
            c.activate_panel(panel);
        }
    }

    fn parts_ready(&self, window: WindowId) {
        if let Some(c) = self::window(window) {
            c.attach_waiting_parts();
        }
    }

    fn set_panel_page(&self, window: WindowId, panel: Panel, page: &NSView) {
        if let Some(c) = self::window(window) {
            c.set_panel_page(panel, page);
        }
    }

    fn set_symbol_count(&self, window: WindowId, panel: Panel, count: usize) {
        if let Some(c) = self::window(window) {
            c.set_symbol_count(panel, count);
        }
    }

    fn set_symbol_status(&self, window: WindowId, panel: Panel, words: Option<&str>) {
        if let Some(c) = self::window(window) {
            c.set_symbol_status(panel, words);
        }
    }

    fn set_panel_accessory(&self, window: WindowId, panel: Panel, view: &NSView) {
        if let Some(c) = self::window(window) {
            c.set_panel_accessory(panel, view);
        }
    }
}

impl window::InspectorHost for AppHost {
    fn set_inspector_content(&self, window: WindowId, view: &NSView) {
        if let Some(c) = self::window(window) {
            c.set_inspector_content(view);
        }
    }

    fn show_inspector(&self, window: WindowId, shown: bool) {
        if let Some(c) = self::window(window) {
            c.show_inspector(shown);
        }
    }
}

impl window::NoticesHost for AppHost {
    fn show_notice(&self, window: WindowId, text: &str) {
        if let Some(c) = self::window(window) {
            c.show_notice(text);
        }
    }

    fn refresh_activity(&self, window: WindowId) {
        if let Some(c) = self::window(window) {
            crate::activity::refresh(&c);
        }
    }
}

impl window::FilesHost for AppHost {
    fn open_file(&self, window: WindowId, path: &Path) {
        if let Some(c) = self::window(window) {
            c.open_file(path);
        }
    }
}

impl window::AppEnvHost for AppHost {
    fn keymap(&self) -> ide_core::keymap::Keymap {
        try_context().map(|cx| cx.keymap().borrow().clone()).unwrap_or_default()
    }

    fn is_quitting(&self) -> bool {
        crate::session::is_quitting()
    }

    fn environment(&self, wait: Duration) -> Option<Vec<(String, String)>> {
        crate::environment::wait(wait).map(<[_]>::to_vec)
    }
}

impl ide_ui_common::window::AsWindow for WindowController {
    fn as_window(&self) -> ide_ui_common::window::Window {
        self.handle()
    }
}

impl ide_ui_debug::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_debug::State> {
        context().debug().clone()
    }

    fn refresh_editors(&self, window: WindowId) {
        if let Some(c) = self::window(window) {
            crate::debug::refresh_editors(&c);
        }
    }

    fn forget_values(&self, window: WindowId) {
        if let Some(c) = self::window(window) {
            crate::debug::forget_values(&c);
        }
    }

    fn front_editor(&self, window: WindowId) -> Option<(Option<PathBuf>, Option<String>)> {
        let c = self::window(window)?;
        let editor = crate::editor::editor_of_tab(&c, c.selected_tab()?)?;
        let document = editor.document();
        Some((
            document.path().map(Path::to_path_buf),
            document.language().map(|l| l.name().to_string()),
        ))
    }

    fn caret_line(&self, window: WindowId) -> Option<(PathBuf, usize)> {
        let c = self::window(window)?;
        let editor = crate::editor::editor_of_tab(&c, c.selected_tab()?)?;
        let at = editor.selectedRange().location;
        let document = editor.document();
        let text = document.text();
        let line = text.line_of(text.utf16_to_byte(at.min(text.len_utf16())).ok()?);
        Some((document.path()?.to_path_buf(), line))
    }

    fn reveal_line(&self, window: WindowId, path: &Path, line: usize) {
        let Some(c) = self::window(window) else { return };
        c.open_file(path);
        // The keyboard focus stays where it is, such as in the list of frames the user goes through.
        crate::editor::with_opened(&c, path, move |editor| {
            let at = {
                let document = editor.document();
                let text = document.text();
                let byte = text.offset(line, 0, ide_document::Unit::Utf16).ok();
                byte.and_then(|byte| text.byte_to_utf16(byte).ok())
            };
            if let Some(at) = at {
                editor.setSelectedRange(objc2_foundation::NSRange::new(at, 0));
                editor.scrollRangeToVisible(objc2_foundation::NSRange::new(at, 0));
            }
        });
    }

    fn open_running(&self, window: WindowId, cwd: &Path, line: String, then: Box<dyn FnOnce(Option<u32>) + Send>) {
        match self::window(window) {
            Some(c) => crate::terminal::open_running(&c, cwd, line, then),
            None => then(None),
        }
    }
}

impl ide_ui_git::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_git::State> {
        context().git().clone()
    }

    fn unsaved_below(&self, root: &Path) -> Vec<PathBuf> {
        crate::editor::unsaved_below(root)
    }

    fn save_below(&self, root: &Path, then: Box<dyn FnOnce(bool)>) {
        crate::editor::save_below(root, then);
    }

    fn check_disk_below(&self, root: &Path) {
        crate::editor::check_disk_below(root);
    }

    fn select_thread_in(&self, folder: &Path) -> bool {
        let Some(tab) = ide_ui_agent::all().into_iter().find(|tab| tab.cwd() == folder) else {
            return false;
        };
        tab.select();
        true
    }

    fn thread_works_in(&self, window: WindowId, folder: &Path) -> bool {
        ide_ui_agent::working_in(folder)
            || self::window(window).is_some_and(|c| c.context().leitstand().working_in(folder))
    }

    fn open_thread(&self, window: WindowId, tree: &ide_git::ThreadTree) {
        if let Some(c) = self::window(window) {
            crate::worktrees::open_thread(&c, tree);
        }
    }

    fn claude_program(&self, env: &[(String, String)]) -> Option<PathBuf> {
        ide_ui_agent::claude_program(env)
    }

    fn endpoint(&self, id: &str) -> Option<ide_agent::Endpoint> {
        ide_ui_agent::own_agent::endpoint(id)
    }

    fn key(&self, endpoint: &ide_agent::Endpoint) -> Option<ide_agent::ApiKey> {
        ide_ui_agent::own_agent::key(endpoint)
    }

    fn provider_error(&self, error: &ide_agent::ProviderError) -> String {
        ide_ui_agent::own_agent::provider_error(error)
    }
}

impl ide_ui_agent::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_agent::State> {
        context().agent().clone()
    }

    fn recent_output(&self, window: WindowId) -> Option<(String, String)> {
        crate::terminal::recent_output(&*self::window(window)?)
    }

    fn editor_selection(&self, window: WindowId) -> Option<(PathBuf, (u32, u32), String)> {
        let controller = self::window(window)?;
        let mut editors: Vec<_> = crate::editor::all_editors()
            .into_iter()
            .filter(|e| e.controller().is_some_and(|c| std::ptr::eq(&*c, &*controller)))
            .collect();
        editors.sort_by_key(|e| e.isHiddenOrHasHiddenAncestor());
        editors.into_iter().find_map(|editor| {
            let range = editor.selectedRange();
            if range.length == 0 {
                return None;
            }
            let path = editor.document().path()?.to_path_buf();
            let string = editor.string();
            let text = string.substringWithRange(range).to_string();
            let before = string.substringToIndex(range.location).to_string();
            let first = u32::try_from(before.matches('\n').count() + 1).ok()?;
            let last = first + u32::try_from(text.trim_end_matches('\n').matches('\n').count()).ok()?;
            Some((path, (first, last), text))
        })
    }

    fn agent_diagnostics(&self, root: &Path) -> std::sync::Arc<dyn ide_agent::Diagnostics> {
        ide_ui_lsp::agent_diagnostics(root)
    }

    fn query_files(
        &self,
        window: WindowId,
        query: &str,
        limit: usize,
        answer: Box<dyn FnOnce(Vec<ide_project::Hit>) + Send>,
    ) {
        if let Some(c) = self::window(window) {
            c.query_files(query, limit, answer);
        }
    }

    fn git(&self) -> Option<ide_git::Git> {
        ide_ui_git::git()
    }

    fn unsaved_buffers(&self, root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        crate::editor::unsaved_buffers(root)
            .into_iter()
            .map(|(path, text)| (path, text.into_bytes()))
            .collect()
    }

    fn take_back_buffers(&self, window: WindowId, buffers: Vec<(PathBuf, Vec<u8>)>) {
        if let Some(c) = self::window(window) {
            crate::editor::take_back_buffers(&c, buffers);
        }
    }

    fn open_terminal(&self, window: WindowId, cwd: &Path) {
        if let Some(c) = self::window(window) {
            crate::terminal::open_tab(&c, Some(cwd));
        }
    }

    fn show_agent_settings(&self, mtm: MainThreadMarker) {
        crate::settings::show_agent(mtm);
    }

    fn show_agents_settings(&self, mtm: MainThreadMarker) {
        crate::settings::show_agents(mtm);
    }

    fn remember_claude_rules(&self, cwd: &Path) {
        crate::settings::remember_claude_rules(cwd);
    }

    fn turn_ended(&self, window: WindowId, cwd: &Path) {
        if let Some(c) = self::window(window) {
            crate::worktrees::turn_ended(&c, cwd);
        }
    }

    fn trust(&self, folder: &Path) {
        crate::project::trust(folder);
    }

    fn show_work(
        &self,
        window: WindowId,
        key: ide_ui_agent::AgentKey,
        part: ide_ui_agent::work::Part,
        anchor: &NSView,
    ) {
        if let Some(c) = self::window(window) {
            crate::activity::show_tab(&c, key, part, anchor);
        }
    }

    fn reread_file(&self, path: &Path) {
        crate::editor::check_disk_below(path);
    }
}

impl ide_ui_lsp::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_lsp::LspState> {
        crate::context::app().lsp().clone()
    }

    fn find_editor(&self, serial: ide_ui_lsp::EditorSerial) -> Option<Box<dyn ide_ui_lsp::editor::Editor>> {
        crate::editor::find_editor(serial).map(crate::editor::lsp_editor)
    }

    fn all_editors(&self) -> Vec<Box<dyn ide_ui_lsp::editor::Editor>> {
        crate::editor::all_editors()
            .into_iter()
            .map(crate::editor::lsp_editor)
            .collect()
    }

    fn editor_in(&self, window: WindowId, path: &Path) -> Option<Box<dyn ide_ui_lsp::editor::Editor>> {
        let controller = self::window(window)?;
        crate::editor::editor_for(&controller, path).map(crate::editor::lsp_editor)
    }

    fn when_editor_ready(&self, window: WindowId, path: &Path, ready: ide_ui_lsp::editor::Ready) -> bool {
        self::window(window).is_some_and(|controller| {
            controller
                .when_restored_editor(path, move |editor| ready(crate::editor::lsp_editor(editor.retain())))
                .is_ok()
        })
    }

    fn show_hierarchy(&self, window: WindowId, hierarchy: ide_ui_lsp::Hierarchy) {
        if let Some(controller) = self::window(window) {
            controller.show_hierarchy(hierarchy);
        }
    }

    fn code_font(&self, size: f64) -> Retained<objc2_app_kit::NSFont> {
        crate::editor::code_font(size)
    }

    fn palette_color(&self, token: splat_markdown::highlight::Token) -> Retained<objc2_app_kit::NSColor> {
        crate::editor::palette_color(token)
    }

    fn is_trusted(&self, folder: &Path) -> bool {
        crate::project::is_trusted(folder)
    }

    fn display_path(&self, path: &Path) -> String {
        crate::app::display_path(path)
    }
}
