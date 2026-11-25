//! A CI job's log in a tab: read only, without escape sequences, error and warning lines in colour, searchable with
//! ⌘F; read again while the job runs.

use std::cell::{Cell, OnceCell, RefCell};
use std::hash::{Hash, Hasher};
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
use crate::util::{constant, guarded, label, ns, pin_edges, pin_sides, set_action, with_editing};
use crate::window::{TabKind, WindowController};

pub(crate) struct LogIvars {
    id: u64,
    controller: Weak<WindowController>,
    window_root: PathBuf,
    client: Client,
    job: RefCell<Job>,
    run: String,
    ui: OnceCell<LogUi>,
    #[cfg(feature = "selftest")]
    lines: RefCell<Vec<LogLine>>,
    /// What the text view shows of the log, `None` while it shows a message instead.
    shown: Cell<Option<Shown>>,
    /// Whether a read was left out while the tab was out of view, which it catches up on in view.
    stale: Cell<bool>,
    loading: Cell<bool>,
}

pub(crate) struct LogUi {
    pub(crate) text: Retained<NSTextView>,
    pub(crate) scroll: Retained<NSScrollView>,
    state: Retained<NSTextField>,
    agent: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `LogTab` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiLogTab"]
    #[ivars = LogIvars]
    pub(crate) struct LogTab;

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
pub(crate) struct LogTabs {
    tabs: Registry<Weak<LogTab>>,
}

fn with_tab(id: u64, f: impl FnOnce(&LogTab)) {
    crate::context::app().ci_log_tabs().tabs.with(id, f);
}

/// The key of the log tab of `job` on `host`.
pub(crate) fn key(host: &str, job: &Job) -> String {
    format!("ci-log:{host}:{}", job.id)
}

/// Shows the log of `job` in its tab, opening one if there is none.
pub(crate) fn show(controller: &WindowController, window_root: &Path, client: Client, job: Job, run: String) {
    let key = key(&client.forge().host, &job);
    if controller.select_tab(&key) {
        came_to_front(controller);
        return;
    }
    let title = tf("Log: {}", &[&job.name]);
    let tab = LogTab::new(controller, window_root.to_path_buf(), client, job, run);
    controller
        .context()
        .ci_log_tabs()
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
pub(crate) fn refresh_for(window_root: &Path, runs: &[ide_ci::panel::RunRow]) {
    for tab in crate::context::app().ci_log_tabs().tabs.live() {
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
            tab.reload_in_view();
        }
    }
}

/// A tab of `controller`'s window may have come to the front: the log tabs that left out a read and show now read.
pub(crate) fn came_to_front(controller: &WindowController) {
    for tab in controller.context().ci_log_tabs().tabs.live() {
        if tab
            .ivars()
            .controller
            .load()
            .is_some_and(|c| std::ptr::eq(&*c, controller))
        {
            tab.catch_up();
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
            id: controller.context().ci_log_tabs().tabs.next_id(),
            controller: Weak::from(controller),
            window_root,
            client,
            job: RefCell::new(job),
            run,
            ui: OnceCell::new(),
            #[cfg(feature = "selftest")]
            lines: RefCell::new(Vec::new()),
            shown: Cell::new(None),
            stale: Cell::new(false),
            loading: Cell::new(false),
        };
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let tab: Retained<LogTab> =
            unsafe { msg_send![super(LogTab::alloc(mtm).set_ivars(ivars)), initWithFrame: frame] };
        let _ = tab.ivars().ui.set(LogUi::new(mtm, &tab));
        tab.show_state();
        let weak = Weak::from(&*tab);
        let came = move || {
            if let Some(tab) = weak.load() {
                tab.catch_up();
            }
        };
        ide_ui_common::panel::watch_view(&tab, came, None);
        tab
    }

    /// Whether the tab shows: in the window tab in front and not hidden.
    fn in_view(&self) -> bool {
        !self.isHiddenOrHasHiddenAncestor() && self.window().is_some_and(|w| crate::window::in_front(&w))
    }

    /// Reads the log again if the tab shows, else once it comes into view.
    fn reload_in_view(&self) {
        if self.in_view() {
            self.reload();
        } else {
            self.ivars().stale.set(true);
        }
    }

    fn catch_up(&self) {
        if self.ivars().stale.get() && self.in_view() {
            self.reload();
        }
    }

    pub(crate) fn ui(&self) -> &LogUi {
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
    pub(crate) fn reload(&self) {
        if self.ivars().loading.replace(true) {
            return;
        }
        self.ivars().stale.set(false);
        let client = self.ivars().client.clone();
        let job = self.ivars().job.borrow().clone();
        let id = self.ivars().id;
        let before = self.ivars().shown.get();
        let queued = work::run(
            move || {
                client
                    .log(&job)
                    .map(|raw| Prepared::of(ide_ci::log::lines(&raw), before))
            },
            move |result| {
                with_tab(id, |tab| {
                    tab.ivars().loading.set(false);
                    match result {
                        Some(Ok(Some(prepared))) => tab.show_lines(prepared),
                        Some(Err(error)) => tab.show_message(&text::problem(&error)),
                        Some(Ok(None)) | None => {}
                    }
                });
            },
        );
        if !queued {
            self.ivars().loading.set(false);
        }
    }

    fn show_message(&self, message: &str) {
        self.ivars().shown.set(None);
        self.ui().text.setString(&ns(message));
    }

    fn show_lines(&self, prepared: Prepared) {
        let ui = self.ui();
        // SAFETY: The text view keeps its text storage for as long as it lives.
        let Some(storage) = (unsafe { ui.text.textStorage() }) else {
            return;
        };
        if prepared.from > 0 && storage.length() != prepared.from {
            self.ivars().shown.set(None);
            return self.reload();
        }
        let at_end = is_at_end(&ui.scroll);
        if prepared.from == 0 {
            ui.text.setString(&ns(&prepared.text));
        }
        let foreground = constant!(NSForegroundColorAttributeName);
        with_editing(&storage, || {
            if prepared.from > 0 {
                storage.replaceCharactersInRange_withString(NSRange::new(prepared.from, 0), &ns(&prepared.text));
                let recolor = NSRange::new(prepared.recolor, storage.length() - prepared.recolor);
                storage.removeAttribute_range(foreground, recolor);
            }
            for &(offset, length, kind) in &prepared.marked {
                let color = match kind {
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
            }
        });
        self.ivars().shown.set(Some(prepared.shown));
        #[cfg(feature = "selftest")]
        {
            *self.ivars().lines.borrow_mut() = prepared.lines;
        }
        if at_end || self.ivars().job.borrow().state == State::Failed {
            // SAFETY: A nil sender is what AppKit's own key bindings pass.
            unsafe { ui.text.scrollToEndOfDocument(None) };
        }
    }
}

/// What a log tab shows of a log, so the next read can tell whether it only grew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    bytes: usize,
    utf16: usize,
    hash: u64,
}

impl Shown {
    fn of(text: &str) -> Shown {
        Shown {
            bytes: text.len(),
            utf16: text.encode_utf16().count(),
            hash: hash(text),
        }
    }
}

fn hash(text: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// A log made ready on the CI lane, so the main thread only hands it to the view: the text from `from` on, where each
/// marked line from `recolor` on starts and how long it is in UTF-16 code units with its kind, and its lines.
struct Prepared {
    /// The whole text when `from` is 0, else what follows the text shown.
    text: String,
    /// Where `text` goes, in UTF-16 code units: 0 puts it in place of all, else it follows what shows.
    from: usize,
    /// Where the line that `text` continues starts, from which the colours are laid again.
    recolor: usize,
    marked: Vec<(usize, usize, LineKind)>,
    shown: Shown,
    #[cfg(feature = "selftest")]
    lines: Vec<LogLine>,
}

impl Prepared {
    /// `lines` as the tab shows them after `before`; `None` when they show already.
    fn of(lines: Vec<LogLine>, before: Option<Shown>) -> Option<Prepared> {
        let text = ide_ci::log::text(&lines);
        let shown = Shown::of(&text);
        if before == Some(shown) {
            return None;
        }
        let grown = before.filter(|before| {
            before.bytes > 0
                && before.bytes < text.len()
                && text.is_char_boundary(before.bytes)
                && hash(&text[..before.bytes]) == before.hash
        });
        let from = grown.map_or(0, |before| before.utf16);
        let mut marked = Vec::new();
        let mut recolor = 0;
        let mut offset = 0usize;
        for line in &lines {
            let length = line.text.encode_utf16().count();
            if offset <= from {
                recolor = offset;
            }
            if line.kind != LineKind::Plain {
                marked.push((offset, length, line.kind));
            }
            offset += length + 1;
        }
        marked.retain(|&(offset, _, _)| offset >= recolor);
        let text = match grown {
            Some(before) => text[before.bytes..].to_string(),
            None => text,
        };
        Some(Prepared {
            text,
            from,
            recolor,
            marked,
            shown,
            #[cfg(feature = "selftest")]
            lines,
        })
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
            pin_sides(view, &stack, 0.0);
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
        constant!(objc2_app_kit::NSFontWeightRegular),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, kind: LineKind) -> LogLine {
        LogLine {
            text: text.into(),
            kind,
        }
    }

    #[test]
    fn a_prepared_log_marks_its_lines_where_the_text_has_them() {
        let prepared = Prepared::of(
            vec![
                line("plain", LineKind::Plain),
                line("Error: ä😀", LineKind::Error),
                line("Run", LineKind::Section),
            ],
            None,
        )
        .expect("nothing shows yet");
        assert_eq!(prepared.text, "plain\nError: ä😀\nRun");
        assert_eq!((prepared.from, prepared.recolor), (0, 0));
        assert_eq!(prepared.marked, [(6, 10, LineKind::Error), (17, 3, LineKind::Section)]);
        assert_eq!(prepared.shown, Shown::of("plain\nError: ä😀\nRun"));
    }

    #[test]
    fn a_log_read_again_unchanged_leaves_the_tab_as_it_is() {
        let lines = || vec![line("a", LineKind::Plain), line("Error: b", LineKind::Error)];
        let shown = Prepared::of(lines(), None).map(|p| p.shown);
        assert!(Prepared::of(lines(), shown).is_none());
    }

    #[test]
    fn a_log_that_only_grew_hands_over_its_new_tail_with_the_colours_from_its_last_line_on() {
        let before = Shown::of("ä\nError: b");
        let prepared = Prepared::of(
            vec![
                line("ä", LineKind::Plain),
                line("Error: b", LineKind::Error),
                line("c", LineKind::Plain),
                line("Warning: d", LineKind::Warning),
            ],
            Some(before),
        )
        .expect("the log grew");
        assert_eq!(prepared.text, "\nc\nWarning: d");
        assert_eq!((prepared.from, prepared.recolor), (10, 2));
        assert_eq!(prepared.marked, [(2, 8, LineKind::Error), (13, 10, LineKind::Warning)]);
        assert_eq!(prepared.shown, Shown::of("ä\nError: b\nc\nWarning: d"));
    }

    #[test]
    fn a_log_whose_start_changed_is_shown_anew() {
        let before = Shown::of("a\nb");
        let prepared = Prepared::of(
            vec![
                line("x", LineKind::Plain),
                line("b", LineKind::Plain),
                line("c", LineKind::Plain),
            ],
            Some(before),
        )
        .expect("the log changed");
        assert_eq!((prepared.text.as_str(), prepared.from), ("x\nb\nc", 0));
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
