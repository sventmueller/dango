//! A CI job's log in a tab: read only, without escape sequences, error and warning lines in colour, searchable with
//! ⌘F; read again while the job runs.

use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};

use ide_ci::log::{LineKind, LogLine};
use ide_ci::{Client, Job, State};
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSBezelStyle, NSBox, NSBoxType, NSButton, NSColor, NSControlSize, NSFont,
    NSForegroundColorAttributeName, NSLayoutAttribute, NSLineBreakMode, NSScrollView, NSStackView, NSStackViewGravity,
    NSStandardKeyBindingResponding, NSTextField, NSTextView, NSUserInterfaceLayoutDirection,
    NSUserInterfaceLayoutOrientation, NSView, NSWritingDirection,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::{text, work};
use crate::context::Registry;
use crate::util::{constant, guarded, label, ns, pin_edges, set_action};
use crate::window::{TabKind, WindowController};

pub struct LogIvars {
    id: u64,
    controller: Weak<WindowController>,
    window_root: PathBuf,
    client: Client,
    job: RefCell<Job>,
    run: String,
    ui: OnceCell<LogUi>,
    lines: RefCell<Vec<LogLine>>,
    loading: std::cell::Cell<bool>,
}

pub struct LogUi {
    pub text: Retained<NSTextView>,
    pub scroll: Retained<NSScrollView>,
    state: Retained<NSTextField>,
    agent: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `LogTab` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiLogTab"]
    #[ivars = LogIvars]
    pub struct LogTab;

    unsafe impl NSObjectProtocol for LogTab {}

    impl LogTab {
        #[unsafe(method(ideCiLogReload:))]
        fn reload_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.reload());
        }

        #[unsafe(method(ideCiLogAgent:))]
        fn agent_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let job = self.ivars().job.borrow().clone();
                if let Some(panel) = self.ivars().controller.load().and_then(|c| super::panel::of(&c)) {
                    panel.send_to_agent(Some(job));
                }
            });
        }

        #[unsafe(method(ideCiLogWeb:))]
        fn web_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let url = self.ivars().job.borrow().url.clone();
                super::ops::open_url(self.ivars().controller.load().as_deref(), &url);
            });
        }
    }
);

/// The CI log tabs of every window by id.
#[derive(Default)]
pub struct LogTabs {
    tabs: Registry<Weak<LogTab>>,
}

fn with_tab(id: u64, f: impl FnOnce(&LogTab)) {
    crate::context::app().ci_log_tabs.tabs.with(id, f);
}

fn live_tabs() -> Vec<Retained<LogTab>> {
    crate::context::app().ci_log_tabs.tabs.live()
}

/// The key of the log tab of `job` on `host`.
pub fn key(host: &str, job: &Job) -> String {
    format!("ci-log:{host}:{}", job.id)
}

/// Shows the log of `job` in its tab, opening one if there is none.
pub fn show(controller: &WindowController, window_root: &Path, client: Client, job: Job, run: String) {
    let key = key(&client.forge().host, &job);
    if controller.select_tab(&key) {
        return;
    }
    let title = tf("Log: {}", &[&job.name]);
    let tab = LogTab::new(controller, window_root.to_path_buf(), client, job, run);
    controller
        .context()
        .ci_log_tabs
        .tabs
        .insert(tab.ivars().id, Weak::from(&*tab));
    tab.reload();
    let text = tab.ui().text.clone();
    controller.add_tab(TabKind::Log, Some(key), &title, tab.into_super());
    if let Some(window) = text.window() {
        window.makeFirstResponder(Some(&text));
    }
}

/// Takes the jobs of a read of the window that shows `window_root`; a tab whose job runs reads its log again.
pub fn refresh_for(window_root: &Path, runs: &[ide_ci::panel::RunRow]) {
    for tab in live_tabs() {
        if tab.ivars().window_root != window_root {
            continue;
        }
        let id = tab.ivars().job.borrow().id;
        let Some(job) = runs.iter().flat_map(|r| r.jobs.iter()).find(|j| j.id == id) else {
            continue;
        };
        let was_active = tab.ivars().job.borrow().state.is_active();
        *tab.ivars().job.borrow_mut() = job.clone();
        tab.show_state();
        if was_active || job.state.is_active() {
            tab.reload();
        }
    }
}

impl LogTab {
    fn new(
        controller: &WindowController,
        window_root: PathBuf,
        client: Client,
        job: Job,
        run: String,
    ) -> Retained<LogTab> {
        let mtm: MainThreadMarker = controller.mtm();
        let ivars = LogIvars {
            id: controller.context().ci_log_tabs.tabs.next_id(),
            controller: Weak::from(controller),
            window_root,
            client,
            job: RefCell::new(job),
            run,
            ui: OnceCell::new(),
            lines: RefCell::new(Vec::new()),
            loading: std::cell::Cell::new(false),
        };
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let tab: Retained<LogTab> =
            unsafe { msg_send![super(LogTab::alloc(mtm).set_ivars(ivars)), initWithFrame: frame] };
        let _ = tab.ivars().ui.set(LogUi::new(mtm, &tab));
        tab.show_state();
        tab
    }

    pub fn ui(&self) -> &LogUi {
        self.ivars().ui.get().expect("the tab builds its views in `new`")
    }

    fn show_state(&self) {
        let job = self.ivars().job.borrow();
        let seconds = job.seconds(ide_ci::now()).map(text::took).unwrap_or_default();
        let line = [self.ivars().run.clone(), text::word(job.state).to_string(), seconds]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let ui = self.ui();
        ui.state.setStringValue(&ns(&line));
        ui.state.setTextColor(Some(&text::color(job.state)));
        ui.agent.setHidden(job.state != State::Failed);
    }

    /// Reads the log again.
    pub fn reload(&self) {
        if self.ivars().loading.replace(true) {
            return;
        }
        let client = self.ivars().client.clone();
        let job = self.ivars().job.borrow().clone();
        let id = self.ivars().id;
        let queued = work::run(
            move || client.log(&job).map(|raw| ide_ci::log::lines(&raw)),
            move |result| {
                with_tab(id, |tab| {
                    tab.ivars().loading.set(false);
                    match result {
                        Some(Ok(lines)) => tab.show_lines(lines),
                        Some(Err(error)) => tab.show_message(&text::problem(&error)),
                        None => {}
                    }
                });
            },
        );
        if !queued {
            self.ivars().loading.set(false);
        }
    }

    fn show_message(&self, message: &str) {
        self.ui().text.setString(&ns(message));
    }

    fn show_lines(&self, lines: Vec<LogLine>) {
        let ui = self.ui();
        let at_end = is_at_end(&ui.scroll);
        ui.text.setString(&ns(&ide_ci::log::text(&lines)));
        // SAFETY: The text view keeps its text storage for as long as it lives.
        if let Some(storage) = unsafe { ui.text.textStorage() } {
            let foreground = constant!(NSForegroundColorAttributeName);
            storage.beginEditing();
            let mut offset = 0usize;
            for line in &lines {
                let length = line.text.encode_utf16().count();
                let color = match line.kind {
                    LineKind::Error => Some(NSColor::systemRedColor()),
                    LineKind::Warning => Some(NSColor::systemOrangeColor()),
                    LineKind::Section => Some(NSColor::systemBlueColor()),
                    LineKind::Plain => None,
                };
                if let Some(color) = color {
                    // SAFETY: The range lies within the text just set, line by line; the value is a colour as the
                    // attribute wants.
                    unsafe {
                        storage.addAttribute_value_range(foreground, &color, NSRange::new(offset, length));
                    }
                }
                offset += length + 1;
            }
            storage.endEditing();
        }
        *self.ivars().lines.borrow_mut() = lines;
        if at_end || self.ivars().job.borrow().state == State::Failed {
            // SAFETY: A nil sender is what AppKit's own key bindings pass.
            unsafe { ui.text.scrollToEndOfDocument(None) };
        }
    }

    /// The lines on view, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn lines(&self) -> Vec<LogLine> {
        self.ivars().lines.borrow().clone()
    }
}

/// Whether `scroll` shows the end of its document, so new lines keep it there.
fn is_at_end(scroll: &NSScrollView) -> bool {
    let Some(document) = scroll.documentView() else {
        return true;
    };
    let visible = scroll.contentView().bounds();
    visible.origin.y + visible.size.height >= document.frame().size.height - 4.0
}

impl LogUi {
    fn new(mtm: MainThreadMarker, tab: &LogTab) -> LogUi {
        let ivars = tab.ivars();
        let title = label(mtm, &ivars.job.borrow().name);
        title.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::systemFontSize())));
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let state = label(mtm, "");
        state.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let button = |title: &str, action| {
            // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
            let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
            set_action(&button, tab, action);
            button.setBezelStyle(NSBezelStyle::Push);
            button.setControlSize(NSControlSize::Small);
            button.setFont(Some(&ide_ui_common::style::small_system()));
            button
        };
        let reload = button(t("Neu laden"), sel!(ideCiLogReload:));
        let agent = button(t("An Agent senden"), sel!(ideCiLogAgent:));
        let web = button(t("Im Browser öffnen"), sel!(ideCiLogWeb:));
        let header = NSStackView::new(mtm);
        header.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        header.setSpacing(ide_ui_common::style::space::M);
        header.setEdgeInsets(NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });
        header.addView_inGravity(&title, NSStackViewGravity::Leading);
        header.addView_inGravity(&state, NSStackViewGravity::Leading);
        for view in [&*agent as &NSView, &reload, &web] {
            header.addView_inGravity(view, NSStackViewGravity::Trailing);
        }
        let (scroll, text) = text_view(mtm, &ivars.job.borrow().name);
        let line = NSBox::new(mtm);
        line.setBoxType(NSBoxType::Separator);
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::Leading);
        stack.setSpacing(ide_ui_common::style::space::NONE);
        for view in [&*header as &NSView, &line, &scroll] {
            stack.addArrangedSubview(view);
            view.leadingAnchor()
                .constraintEqualToAnchor(&stack.leadingAnchor())
                .setActive(true);
            view.trailingAnchor()
                .constraintEqualToAnchor(&stack.trailingAnchor())
                .setActive(true);
        }
        scroll.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Vertical);
        tab.addSubview(&stack);
        pin_edges(&stack, tab);
        LogUi {
            text,
            scroll,
            state,
            agent,
        }
    }
}

/// A read-only, monospaced text view with the find bar, left to right whatever the interface's direction.
fn text_view(mtm: MainThreadMarker, job: &str) -> (Retained<NSScrollView>, Retained<NSTextView>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    let text = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    text.setEditable(false);
    text.setSelectable(true);
    text.setRichText(false);
    text.setUsesFindBar(true);
    text.setIncrementalSearchingEnabled(true);
    let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
    text.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
        (size - 1.0).max(9.0),
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightRegular },
    )));
    text.setTextContainerInset(NSSize::new(8.0, 8.0));
    text.setHorizontallyResizable(true);
    text.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    text.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
    // SAFETY: A text view keeps its text container alive as long as it lives.
    if let Some(container) = unsafe { text.textContainer() } {
        container.setWidthTracksTextView(false);
        container.setContainerSize(NSSize::new(f64::MAX, f64::MAX));
    }
    text.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    text.setBaseWritingDirection(NSWritingDirection::LeftToRight);
    text.setAccessibilityLabel(Some(&ns(&tf("Log von {}", &[job]))));
    scroll.setDocumentView(Some(&text));
    (scroll, text)
}

/// The lines of the log tab of `job` in `controller`'s window, and whether its find bar shows; with `find`, after
/// the find command ⌘F sends.
#[cfg(feature = "selftest")]
pub fn inspect(controller: &WindowController, job: u64, find: bool) -> Option<(Vec<LogLine>, bool)> {
    let tab = live_tabs().into_iter().find(|tab| {
        tab.ivars().job.borrow().id == job
            && tab
                .ivars()
                .controller
                .load()
                .is_some_and(|c| std::ptr::eq(&*c, controller))
    })?;
    let ui = tab.ui();
    if find {
        let item = crate::util::item(tab.mtm(), "", Some(sel!(performFindPanelAction:)), "");
        item.setTag(1);
        // SAFETY: The sender is a menu item whose tag names the find action, as the Find menu sends it.
        unsafe { ui.text.performFindPanelAction(Some(&item)) };
    }
    Some((
        tab.lines(),
        objc2_app_kit::NSTextFinderBarContainer::isFindBarVisible(&*ui.scroll),
    ))
}
