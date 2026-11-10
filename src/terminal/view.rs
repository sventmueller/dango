//! The terminal view: a layer-backed view that draws a session's screen and hands it keys, mouse, pastes and drops.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use ide_core::settings::{Settings, keys};
use ide_process::Supervisor;
use ide_terminal::{Clipboard, Damage, Event, Foreground, Options, Program, Session, Size, Snapshot};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibilityValueChangedNotification, NSAppearanceCustomization, NSAppearanceNameAccessibilityHighContrastAqua,
    NSAppearanceNameAccessibilityHighContrastDarkAqua, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBeep, NSColor,
    NSDragOperation, NSDraggingInfo, NSEvent, NSGraphicsContext, NSLayoutConstraintOrientation, NSMenu, NSMenuItem,
    NSMenuItemValidation, NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypeString, NSStackView,
    NSStackViewDistribution, NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions, NSUserInterfaceLayoutDirection,
    NSUserInterfaceLayoutOrientation, NSView, NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification,
    NSWorkspace,
};
use objc2_core_graphics::CGContext;
use objc2_foundation::{
    NSArray, NSAttributedString, NSDate, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRange,
    NSRangePointer, NSRect, NSSize, NSString, NSTimer, NSUInteger,
};
use splat_i18n::{t, tf};

use super::banner::Banner;
use super::cli::{CliState, Launch};
use super::input::KeyState;
use super::mouse::MouseState;
use super::palette::{self, Theme};
use super::render::{self, Fonts, Overlay};
use crate::util::{constant, guarded, item, main_after, ns, separator, set_target};
use crate::window::WindowController;

/// The shortest time between two snapshots, about a frame of a fast display.
const FRAME: Duration = Duration::from_millis(8);
const BLINK: f64 = 0.53;
const BELL_FLASH: Duration = Duration::from_millis(140);
/// Bells closer than this ring once.
const BELL_GAP: Duration = Duration::from_millis(250);
/// VoiceOver hears of new output at most this often.
const ANNOUNCE_GAP: Duration = Duration::from_millis(500);
const NOTICE_TIME: Duration = Duration::from_secs(8);

/// What a terminal view keeps.
pub struct Ivars {
    pub(super) id: u64,
    pub(super) controller: Weak<WindowController>,
    font_size: Cell<f64>,
    pub(super) tab: Cell<Option<isize>>,
    pub(super) session: RefCell<Option<Session>>,
    fonts: RefCell<Option<Rc<Fonts>>>,
    pub(super) theme: RefCell<Theme>,
    pub(super) screen: RefCell<Option<Snapshot>>,
    /// The working directory the shell last reported, else the one it started in.
    pub(super) cwd: RefCell<PathBuf>,
    pub(super) title: RefCell<Option<String>>,
    pub(super) state: State,
    pub(super) keys: KeyState,
    pub(super) mouse: MouseState,
    banners: RefCell<Option<Banners>>,
    blink: RefCell<Option<Retained<NSTimer>>>,
    observers: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
    /// The CLI the terminal runs instead of a shell, for a CLI thread (I5).
    pub(super) cli: Option<CliState>,
}

impl Drop for Ivars {
    fn drop(&mut self) {
        if let Some(timer) = self.blink.get_mut().take() {
            timer.invalidate();
        }
        remove_observers(self.observers.get_mut());
        self.mouse.stop_autoscroll();
        super::forget(self.id);
        if self.cli.is_some()
            && let Some(controller) = self.controller.load()
        {
            crate::activity::refresh(&controller);
            crate::agent_panel::refresh(&controller);
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    pub(super) first_responder: Cell<bool>,
    pub(super) focused: Cell<bool>,
    pub(super) exited: Cell<bool>,
    pub(super) prompts: Cell<usize>,
    /// Output arrived while the view was hidden.
    stale: Cell<bool>,
    frame_pending: Cell<bool>,
    last_frame: Cell<Option<Instant>>,
    cursor_off: Cell<bool>,
    last_bell: Cell<Option<Instant>>,
    flash_until: Cell<Option<Instant>>,
    announced: Cell<Option<Instant>>,
    pub(super) size: Cell<Option<Size>>,
    notices: Cell<u64>,
}

pub(super) struct Banners {
    pub(super) dev_server: Banner,
    notice: Banner,
    url: RefCell<Option<String>>,
}

fn remove_observers(observers: &mut Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>) {
    let center = NSNotificationCenter::defaultCenter();
    for token in observers.drain(..) {
        // SAFETY: The token came from this center's block-based `addObserverForName`.
        unsafe { center.removeObserver(token.as_ref()) };
    }
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `TerminalView` does not implement Drop; its ivars do, and
    // objc2 drops them when the view is deallocated.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETerminalView"]
    #[ivars = Ivars]
    pub struct TerminalView;

    unsafe impl NSObjectProtocol for TerminalView {}

    unsafe impl NSTextInputClient for TerminalView {
        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &AnyObject, _range: NSRange) {
            guarded((), || self.insert(string));
        }

        #[unsafe(method(doCommandBySelector:))]
        fn do_command_by_selector(&self, selector: Sel) {
            guarded((), || self.command(selector));
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked_text(&self, string: &AnyObject, selected: NSRange, _range: NSRange) {
            guarded((), || self.set_marked(string, selected));
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            guarded((), || self.unmark());
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            guarded(NSRange::new(super::input::NOT_FOUND, 0), || self.marked_selection())
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            guarded(NSRange::new(super::input::NOT_FOUND, 0), || self.marked_range_now())
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            guarded(false, || self.has_marked())
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        fn attributed_substring(&self, _range: NSRange, _actual: NSRangePointer) -> Option<Retained<NSAttributedString>> {
            None
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes(&self) -> Retained<NSArray<NSString>> {
            NSArray::new()
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect(&self, _range: NSRange, _actual: NSRangePointer) -> NSRect {
            guarded(NSRect::ZERO, || self.cursor_on_screen())
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index(&self, _point: NSPoint) -> NSUInteger {
            super::input::NOT_FOUND
        }
    }

    unsafe impl NSMenuItemValidation for TerminalView {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(false, || self.validate(item))
        }
    }

    impl TerminalView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            guarded((), || self.draw(dirty));
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let () = unsafe { msg_send![super(self), setFrameSize: size] };
            guarded((), || self.layout_grid());
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            guarded((), || self.moved_to_window());
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing(&self) {
            guarded((), || self.layout_grid());
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn did_change_appearance(&self) {
            guarded((), || self.update_theme());
        }

        #[unsafe(method(viewDidUnhide))]
        fn did_unhide(&self) {
            guarded((), || self.unhidden());
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            // SAFETY: The superclass's implementation, with this method's signature.
            let accepted: bool = unsafe { msg_send![super(self), becomeFirstResponder] };
            if accepted {
                guarded((), || {
                    self.ivars().state.first_responder.set(true);
                    self.focus_changed();
                });
            }
            accepted
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            // SAFETY: The superclass's implementation, with this method's signature.
            let resigned: bool = unsafe { msg_send![super(self), resignFirstResponder] };
            if resigned {
                guarded((), || {
                    self.ivars().state.first_responder.set(false);
                    self.discard_marked();
                    self.focus_changed();
                });
            }
            resigned
        }

        #[unsafe(method(keyDown:))]
        fn key_down_event(&self, event: &NSEvent) {
            if !guarded(true, || self.key_down(event)) {
                // SAFETY: The superclass's implementation, with this method's signature.
                let () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up_event(&self, event: &NSEvent) {
            guarded((), || self.key_up(event));
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed_event(&self, event: &NSEvent) {
            guarded((), || self.flags_changed(event));
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down_event(&self, event: &NSEvent) {
            guarded((), || self.mouse_down(event));
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged_event(&self, event: &NSEvent) {
            guarded((), || self.mouse_dragged(event));
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up_event(&self, event: &NSEvent) {
            guarded((), || self.mouse_up(event));
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down_event(&self, event: &NSEvent) {
            if !guarded(false, || self.other_button(event, true)) {
                // SAFETY: The superclass's implementation, which shows the context menu.
                let () = unsafe { msg_send![super(self), rightMouseDown: event] };
            }
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up_event(&self, event: &NSEvent) {
            guarded((), || {
                self.other_button(event, false);
            });
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged_event(&self, event: &NSEvent) {
            guarded((), || self.mouse_moved(event));
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down_event(&self, event: &NSEvent) {
            guarded((), || {
                self.other_button(event, true);
            });
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up_event(&self, event: &NSEvent) {
            guarded((), || {
                self.other_button(event, false);
            });
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged_event(&self, event: &NSEvent) {
            guarded((), || self.mouse_moved(event));
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved_event(&self, event: &NSEvent) {
            guarded((), || self.mouse_moved(event));
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel_event(&self, event: &NSEvent) {
            guarded((), || self.scroll_wheel(event));
        }

        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, event: &NSEvent) {
            guarded((), || self.update_pointer(event.modifierFlags(), Some(event.locationInWindow())));
        }

        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, _event: &NSEvent) -> Option<Retained<NSMenu>> {
            guarded(None, || Some(self.context_menu()))
        }

        #[unsafe(method(copy:))]
        fn copy_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.copy_selection());
        }

        #[unsafe(method(paste:))]
        fn paste_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.paste_from(&NSPasteboard::generalPasteboard());
            });
        }

        #[unsafe(method(selectAll:))]
        fn select_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.select_all());
        }

        #[unsafe(method(ideClearTerminal:))]
        fn clear_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.clear());
        }

        #[unsafe(method(ideNewTerminalHere:))]
        fn new_terminal_here_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let cwd = self.ivars().cwd.borrow().clone();
                if let Some(controller) = self.ivars().controller.load() {
                    super::open_tab(&controller, Some(cwd.as_path()));
                }
            });
        }

        #[unsafe(method(ideOpenDevServer:))]
        fn open_dev_server_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.open_dev_server());
        }

        #[unsafe(method(ideDismissDevServer:))]
        fn dismiss_dev_server_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_banners(|banners| banners.dev_server.hide()));
        }

        #[unsafe(method(ideDismissNotice:))]
        fn dismiss_notice_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_banners(|banners| banners.notice.hide()));
        }

        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            guarded(NSDragOperation::None, || {
                if super::clip::accepts(&sender.draggingPasteboard()) {
                    NSDragOperation::Copy
                } else {
                    NSDragOperation::None
                }
            })
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            guarded(false, || self.drop_from(&sender.draggingPasteboard()))
        }

        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Option<Retained<NSString>> {
            Some(constant!(objc2_app_kit::NSAccessibilityTextAreaRole).retain())
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Option<Retained<NSString>> {
            guarded(None, || Some(ns(&self.label())))
        }

        #[unsafe(method_id(accessibilityValue))]
        fn accessibility_value(&self) -> Option<Retained<AnyObject>> {
            guarded(None, || Some(ns(self.screen_text().text()).into()))
        }

        #[unsafe(method(accessibilityNumberOfCharacters))]
        fn accessibility_characters(&self) -> isize {
            guarded(0, || isize::try_from(self.screen_text().length()).unwrap_or(0))
        }

        #[unsafe(method_id(accessibilitySelectedText))]
        fn accessibility_selected_text(&self) -> Option<Retained<NSString>> {
            guarded(None, || Some(ns(&self.selected_text().unwrap_or_default())))
        }

        #[unsafe(method(accessibilitySelectedTextRange))]
        fn accessibility_selected_range(&self) -> NSRange {
            guarded(NSRange::new(0, 0), || NSRange::new(self.cursor_offset(), 0))
        }

        #[unsafe(method(accessibilityVisibleCharacterRange))]
        fn accessibility_visible_range(&self) -> NSRange {
            guarded(NSRange::new(0, 0), || NSRange::new(0, self.screen_text().length()))
        }

        #[unsafe(method(accessibilityInsertionPointLineNumber))]
        fn accessibility_insertion_line(&self) -> isize {
            guarded(0, || self.cursor_line())
        }

        #[unsafe(method(accessibilityLineForIndex:))]
        fn accessibility_line_for_index(&self, index: isize) -> isize {
            guarded(0, || {
                let line = self.screen_text().line_of(usize::try_from(index).unwrap_or(0));
                isize::try_from(line).unwrap_or(0)
            })
        }

        #[unsafe(method(accessibilityRangeForLine:))]
        fn accessibility_range_for_line(&self, line: isize) -> NSRange {
            guarded(NSRange::new(0, 0), || {
                let found = usize::try_from(line).ok().and_then(|line| self.screen_text().line_range(line));
                found.map_or(NSRange::new(0, 0), |(location, length)| NSRange::new(location, length))
            })
        }

        #[unsafe(method_id(accessibilityStringForRange:))]
        fn accessibility_string_for_range(&self, range: NSRange) -> Option<Retained<NSString>> {
            guarded(None, || {
                let text = self.screen_text().substring(range.location, range.length)?;
                Some(ns(&text))
            })
        }

        #[unsafe(method(accessibilityFrameForRange:))]
        fn accessibility_frame_for_range(&self, range: NSRange) -> NSRect {
            guarded(NSRect::ZERO, || self.frame_for_range(range))
        }
    }
);

impl TerminalView {
    /// A terminal view for a tab of `controller` that starts in `cwd`, and what the tab shows: the view under its
    /// banners.
    pub(super) fn new(
        mtm: MainThreadMarker,
        controller: &WindowController,
        cwd: PathBuf,
        cli: Option<Launch>,
    ) -> (Retained<TerminalView>, Retained<NSView>) {
        let settings = Settings::load_user();
        super::set_option_as_meta(settings.get(&keys::OPTION_AS_META));
        let ivars = Ivars {
            id: super::next_id(),
            controller: Weak::from(controller),
            font_size: Cell::new(settings.get(&keys::FONT_SIZE)),
            tab: Cell::new(None),
            session: RefCell::new(None),
            fonts: RefCell::new(None),
            theme: RefCell::new(palette::theme(false, false)),
            screen: RefCell::new(None),
            cwd: RefCell::new(cwd),
            title: RefCell::new(None),
            state: State::default(),
            keys: KeyState::default(),
            mouse: MouseState::default(),
            banners: RefCell::new(None),
            blink: RefCell::new(None),
            observers: RefCell::new(vec![]),
            cli: cli.map(CliState::new),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setWantsLayer(true);
        // Programs lay out their output for a left-to-right grid, whatever the interface language.
        this.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
        this.registerForDraggedTypes(&NSArray::from_slice(&[
            constant!(NSPasteboardTypeFileURL),
            constant!(NSPasteboardTypeString),
        ]));
        let options = NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::CursorUpdate
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY: A tracking area holds its owner weakly, and the view owns the area.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                options,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this.update_theme();
        super::register(&this);
        let root = this.assemble(mtm);
        (this, root)
    }

    pub(super) fn id(&self) -> u64 {
        self.ivars().id
    }

    /// The banners above the view and the view in a column, the view taking the height they leave.
    fn assemble(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setDistribution(NSStackViewDistribution::Fill);
        stack.setSpacing(0.0);
        let target: &AnyObject = self;
        let dev_server = Banner::new(
            mtm,
            "network",
            &NSColor::controlAccentColor().colorWithAlphaComponent(0.14),
            target,
            Some((t("Im Browser-Tab öffnen"), sel!(ideOpenDevServer:))),
            sel!(ideDismissDevServer:),
        );
        let notice = Banner::new(
            mtm,
            "exclamationmark.triangle",
            &NSColor::systemOrangeColor().colorWithAlphaComponent(0.16),
            target,
            None,
            sel!(ideDismissNotice:),
        );
        for view in [dev_server.view(), notice.view(), self] {
            stack.addArrangedSubview(view);
            view.leadingAnchor()
                .constraintEqualToAnchor(&stack.leadingAnchor())
                .setActive(true);
            view.trailingAnchor()
                .constraintEqualToAnchor(&stack.trailingAnchor())
                .setActive(true);
        }
        self.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        self.setContentCompressionResistancePriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        *self.ivars().banners.borrow_mut() = Some(Banners {
            dev_server,
            notice,
            url: RefCell::new(None),
        });
        stack.into_super()
    }

    pub(super) fn with_banners(&self, f: impl FnOnce(&Banners)) {
        if let Some(banners) = self.ivars().banners.borrow().as_ref() {
            f(banners);
        }
    }

    /// Starts the shell, or the CLI of a CLI thread, with the login shell's environment `env`.
    pub(super) fn spawn(&self, env: &[(String, String)]) {
        let mut options = Options::new(self.ivars().cwd.borrow().clone());
        options.palette = self.ivars().theme.borrow().palette.clone();
        if let Some(cli) = &self.ivars().cli {
            options.program = Program::Command {
                program: cli.program.clone(),
                args: cli.args.clone(),
            };
            options.shell_integration = None;
            options.interrupt_on_close = true;
        }
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
                crate::diag::report("starting a terminal", &error);
                self.notify(&tf("Das Terminal konnte nicht starten: {}", &[&error.to_string()]));
            }
        }
    }

    /// Runs `f` with the session once it has started.
    pub(super) fn with_session<R>(&self, f: impl FnOnce(&Session) -> R) -> Option<R> {
        self.ivars().session.borrow().as_ref().map(f)
    }

    /// Reacts to what the session reports.
    pub(super) fn handle(&self, event: Event) {
        if let Some(cli) = &self.ivars().cli {
            let now = std::time::Instant::now();
            match &event {
                Event::Redraw => cli.output(self.with_session(Session::content_changes).unwrap_or(0), now),
                Event::Title(title) => cli.title(title.as_deref()),
                Event::Bell => cli.bell(now),
                Event::Exited(_) => cli.exited(),
                _ => {}
            }
        }
        let watched = matches!(event, Event::Redraw | Event::Title(_) | Event::Bell | Event::Exited(_));
        self.dispatch(event);
        if watched {
            self.cli_changed(false);
        }
    }

    fn dispatch(&self, event: Event) {
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
        let shown = title.clone().filter(|t| !t.trim().is_empty());
        *self.ivars().title.borrow_mut() = shown.clone();
        let (Some(controller), Some(tab)) = (self.ivars().controller.load(), self.ivars().tab.get()) else {
            return;
        };
        // A CLI thread's tab keeps the CLI's name; its title shows in the activity view.
        if self.ivars().cli.is_some() {
            return;
        }
        controller.set_tab_title(tab, shown.as_deref().unwrap_or(t("Terminal")));
    }

    /// The tab's id, once the tab exists; a title that arrived before is shown now.
    pub(super) fn set_tab(&self, id: isize) {
        self.ivars().tab.set(Some(id));
        let title = self.ivars().title.borrow().clone();
        if title.is_some() {
            self.set_title(title);
        }
    }

    pub(super) fn label(&self) -> String {
        match self.ivars().title.borrow().as_deref() {
            Some(title) => tf("Terminal: {}", &[title]),
            None => t("Terminal").to_string(),
        }
    }

    fn bell(&self) {
        let state = &self.ivars().state;
        let now = Instant::now();
        if state.last_bell.get().is_some_and(|last| now - last < BELL_GAP) {
            return;
        }
        state.last_bell.set(Some(now));
        let visible = !self.isHiddenOrHasHiddenAncestor() && self.window().is_some_and(|w| w.isKeyWindow());
        if !visible || NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
            NSBeep();
            return;
        }
        state.flash_until.set(Some(now + BELL_FLASH));
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

    fn open_dev_server(&self) {
        let mut url = None;
        self.with_banners(|banners| {
            url = banners.url.borrow().clone();
            banners.dev_server.hide();
        });
        if let (Some(url), Some(controller)) = (url, self.ivars().controller.load()) {
            crate::browser::open_tab(&controller, Some(url.as_str()));
        }
    }

    /// Shows `text` above the terminal for a few seconds.
    pub(super) fn notify(&self, text: &str) {
        let notices = &self.ivars().state.notices;
        let shown = notices.get() + 1;
        notices.set(shown);
        self.with_banners(|banners| banners.notice.show(text));
        let id = self.id();
        main_after(NOTICE_TIME, move || {
            if let Some(view) = super::find(id)
                && view.ivars().state.notices.get() == shown
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

    fn clear(&self) {
        self.with_session(|session| {
            let shell = matches!(session.foreground(), Foreground::Shell(_));
            session.clear(shell && !session.input_modes().alternate_screen);
        });
        self.refresh(true);
    }

    fn select_all(&self) {
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

    fn context_menu(&self) -> Retained<NSMenu> {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        let entries: [Option<(&str, Sel)>; 7] = [
            Some((t("Kopieren"), sel!(copy:))),
            Some((t("Einsetzen"), sel!(paste:))),
            Some((t("Alles auswählen"), sel!(selectAll:))),
            None,
            Some((t("Terminal leeren"), sel!(ideClearTerminal:))),
            None,
            Some((t("Neues Terminal hier"), sel!(ideNewTerminalHere:))),
        ];
        for entry in entries {
            match entry {
                Some((title, action)) => {
                    let entry = item(mtm, title, Some(action), "");
                    set_target(&entry, self);
                    menu.addItem(&entry);
                }
                None => menu.addItem(&separator(mtm)),
            }
        }
        menu
    }

    fn validate(&self, item: &NSMenuItem) -> bool {
        let action = item.action();
        let started = self.ivars().session.borrow().is_some();
        if action == Some(sel!(copy:)) {
            return self.selected_text().is_some();
        }
        if action == Some(sel!(paste:)) {
            return started && super::clip::accepts(&NSPasteboard::generalPasteboard());
        }
        if [sel!(selectAll:), sel!(ideClearTerminal:)].map(Some).contains(&action) {
            return started;
        }
        true
    }

    fn fonts(&self) -> Rc<Fonts> {
        let scale = self.window().map_or(2.0, |window| window.backingScaleFactor());
        let mut fonts = self.ivars().fonts.borrow_mut();
        match fonts.as_ref() {
            Some(current) if (current.metrics.scale - scale).abs() < f64::EPSILON => current.clone(),
            _ => {
                let made = Rc::new(Fonts::new(self.ivars().font_size.get(), scale));
                *fonts = Some(made.clone());
                made
            }
        }
    }

    /// Draws with fonts of `size` points from now on, which changes the cells and so the grid.
    pub(super) fn set_font_size(&self, size: f64) {
        if (self.ivars().font_size.replace(size) - size).abs() < f64::EPSILON {
            return;
        }
        self.ivars().fonts.borrow_mut().take();
        self.layout_grid();
        self.refresh(true);
    }

    /// The cell metrics, for turning positions into cells.
    pub(super) fn metrics(&self) -> render::Metrics {
        self.fonts().metrics
    }

    /// The grid that fits the view; after the program ended, one line less, for the line that says so.
    fn grid_size(&self) -> Option<Size> {
        let bounds = self.bounds().size;
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return None;
        }
        let m = self.metrics();
        let (columns, mut lines) = m.fit(bounds.width, bounds.height);
        if self.ivars().state.exited.get() {
            lines = lines.saturating_sub(1).max(1);
        }
        Some(Size {
            columns,
            lines,
            cell_width: m.pixels(m.cell_width),
            cell_height: m.pixels(m.cell_height),
        })
    }

    /// Fits the grid to the view and tells the program of a new size.
    fn layout_grid(&self) {
        let Some(size) = self.grid_size() else { return };
        let state = &self.ivars().state;
        if state.size.get() == Some(size) {
            return;
        }
        state.size.set(Some(size));
        self.with_session(|session| session.resize(size));
        self.refresh(true);
    }

    fn moved_to_window(&self) {
        remove_observers(&mut self.ivars().observers.borrow_mut());
        let Some(window) = self.window() else {
            if let Some(timer) = self.ivars().blink.borrow_mut().take() {
                timer.invalidate();
            }
            return;
        };
        let center = NSNotificationCenter::defaultCenter();
        for name in [
            constant!(NSWindowDidBecomeKeyNotification),
            constant!(NSWindowDidResignKeyNotification),
        ] {
            let view = Weak::from(self);
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                guarded((), || {
                    if let Some(view) = view.load() {
                        view.focus_changed();
                    }
                });
            });
            // SAFETY: The block holds the view weakly and runs on the main thread, where windows post these.
            let token =
                unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), Some(&window), None, &block) };
            self.ivars().observers.borrow_mut().push(token);
        }
        self.update_theme();
        self.layout_grid();
        self.focus_changed();
    }

    fn unhidden(&self) {
        if self.ivars().state.stale.get() {
            self.refresh(true);
        }
        let Some(window) = self.window() else { return };
        let idle = window.firstResponder().is_none_or(|responder| {
            let window_itself: &AnyObject = &window;
            let responder: &AnyObject = &responder;
            std::ptr::eq(responder, window_itself)
                || responder
                    .downcast_ref::<NSView>()
                    .is_some_and(NSView::isHiddenOrHasHiddenAncestor)
        });
        if idle {
            window.makeFirstResponder(Some(self));
        }
    }

    fn update_theme(&self) {
        let names = NSArray::from_slice(&[
            constant!(NSAppearanceNameAqua),
            constant!(NSAppearanceNameDarkAqua),
            constant!(NSAppearanceNameAccessibilityHighContrastAqua),
            constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
        ]);
        let best = self.effectiveAppearance().bestMatchFromAppearancesWithNames(&names);
        let is = |name: &NSString| best.as_deref() == Some(name);
        let dark =
            is(constant!(NSAppearanceNameDarkAqua)) || is(constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua));
        let high_contrast = is(constant!(NSAppearanceNameAccessibilityHighContrastAqua))
            || is(constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua));
        let theme = palette::theme(dark, high_contrast);
        if *self.ivars().theme.borrow() == theme {
            return;
        }
        self.with_session(|session| session.set_palette(theme.palette.clone()));
        *self.ivars().theme.borrow_mut() = theme;
        self.refresh(true);
    }

    pub(super) fn focus_changed(&self) {
        let state = &self.ivars().state;
        let focused = state.first_responder.get() && self.window().is_some_and(|w| w.isKeyWindow());
        if state.focused.replace(focused) == focused {
            return;
        }
        state.cursor_off.set(false);
        self.with_session(|session| session.focus(focused));
        self.refresh(false);
    }

    /// Output arrived; a new snapshot follows within a frame unless the view is hidden.
    fn redraw_requested(&self) {
        let state = &self.ivars().state;
        if self.isHiddenOrHasHiddenAncestor() {
            state.stale.set(true);
            return;
        }
        if state.frame_pending.get() {
            return;
        }
        let since = state.last_frame.get().map_or(FRAME, |last| last.elapsed());
        if since >= FRAME {
            self.refresh(false);
            return;
        }
        state.frame_pending.set(true);
        let id = self.id();
        main_after(FRAME - since, move || {
            if let Some(view) = super::find(id) {
                view.ivars().state.frame_pending.set(false);
                view.refresh(false);
            }
        });
    }

    /// Takes a snapshot of the screen and marks the lines that changed for drawing; `all` marks every line.
    pub(super) fn refresh(&self, all: bool) {
        let Some(screen) = self.with_session(Session::snapshot) else {
            self.setNeedsDisplay(true);
            return;
        };
        let state = &self.ivars().state;
        state.last_frame.set(Some(Instant::now()));
        state.stale.set(false);
        let m = self.metrics();
        let width = self.bounds().size.width;
        let previous = self.ivars().screen.borrow_mut().replace(screen);
        {
            let screen = self.ivars().screen.borrow();
            let Some(now) = screen.as_ref() else { return };
            let same_shape = previous
                .as_ref()
                .is_some_and(|before| before.lines == now.lines && before.columns == now.columns);
            match &now.damage {
                Damage::Lines(lines) if same_shape && !all => {
                    let before = previous.as_ref().map_or(now.cursor.point.line, |b| b.cursor.point.line);
                    let changed = lines.iter().map(|line| line.line);
                    for line in changed.chain([before, now.cursor.point.line]) {
                        self.setNeedsDisplayInRect(m.line_rect(line, width));
                    }
                }
                _ => self.setNeedsDisplay(true),
            }
        }
        self.update_blink();
        self.announce();
    }

    fn draw(&self, dirty: NSRect) {
        let Some(context) = NSGraphicsContext::currentContext() else {
            return;
        };
        let ctx = context.CGContext();
        // Drawing the window into a bitmap passes a rectangle beyond the view and does not clip to it.
        let bounds = self.bounds();
        CGContext::clip_to_rect(Some(&ctx), bounds);
        let dirty = render::intersect(dirty, bounds);
        let theme = self.ivars().theme.borrow();
        let screen = self.ivars().screen.borrow();
        let Some(screen) = screen.as_ref() else {
            render::fill(&ctx, theme.palette.background, dirty);
            return;
        };
        let fonts = self.fonts();
        let state = &self.ivars().state;
        let marked = self.ivars().keys.marked.borrow();
        let final_line =
            (state.exited.get() && screen.display_offset == 0).then(|| (final_line(screen), t("[Prozess beendet]")));
        let overlay = Overlay {
            focused: state.focused.get(),
            cursor_on: !state.cursor_off.get(),
            marked: &marked,
            final_line,
            flash: state.flash_until.get().is_some_and(|until| Instant::now() < until),
        };
        render::draw(&ctx, &fonts, &theme, screen, &overlay, dirty);
    }

    /// Starts or stops the cursor's blinking as the program asks while the view has the focus.
    fn update_blink(&self) {
        let state = &self.ivars().state;
        let blinking = state.focused.get()
            && !state.exited.get()
            && self
                .ivars()
                .screen
                .borrow()
                .as_ref()
                .is_some_and(|screen| screen.cursor.blinking && screen.cursor.visible);
        let mut timer = self.ivars().blink.borrow_mut();
        match (blinking, timer.is_some()) {
            (true, false) => {
                let view = Weak::from(self);
                let block = RcBlock::new(move |_: NonNull<NSTimer>| {
                    guarded((), || {
                        if let Some(view) = view.load() {
                            let off = &view.ivars().state.cursor_off;
                            off.set(!off.get());
                            view.invalidate_cursor();
                        }
                    });
                });
                // SAFETY: The block holds the view weakly; the main run loop fires the timer on the main thread.
                *timer = Some(unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(BLINK, true, &block) });
            }
            (false, true) => {
                if let Some(timer) = timer.take() {
                    timer.invalidate();
                }
                state.cursor_off.set(false);
            }
            _ => {}
        }
    }

    /// Shows the cursor at once after a key, and keeps it shown for a full blink.
    pub(super) fn wake_cursor(&self) {
        if self.ivars().state.cursor_off.replace(false) {
            self.invalidate_cursor();
        }
        if let Some(timer) = self.ivars().blink.borrow().as_ref() {
            timer.setFireDate(&NSDate::dateWithTimeIntervalSinceNow(BLINK));
        }
    }

    pub(super) fn invalidate_cursor(&self) {
        let line = self
            .ivars()
            .screen
            .borrow()
            .as_ref()
            .map(|screen| screen.cursor.point.line);
        if let Some(line) = line {
            let width = self.bounds().size.width;
            self.setNeedsDisplayInRect(self.metrics().line_rect(line, width));
        }
    }

    /// Lets VoiceOver know the text changed, at most every half second.
    fn announce(&self) {
        let state = &self.ivars().state;
        let now = Instant::now();
        if !state.focused.get() || state.announced.get().is_some_and(|last| now - last < ANNOUNCE_GAP) {
            return;
        }
        state.announced.set(Some(now));
        let element: &AnyObject = self;
        let changed = constant!(NSAccessibilityValueChangedNotification);
        // SAFETY: The element is this view and the name AppKit's value-changed notification.
        unsafe { objc2_app_kit::NSAccessibilityPostNotification(element, changed) };
    }
}

/// The line that says the program ended: where the cursor stands if that line is empty, else the next one, which may
/// be the line kept free below the grid.
fn final_line(screen: &Snapshot) -> usize {
    let cursor = screen.cursor.point;
    let blank = screen
        .rows
        .get(cursor.line)
        .is_some_and(|row| row.iter().all(|cell| cell.ch == ' ' || cell.ch == '\0'));
    if blank { cursor.line } else { cursor.line + 1 }
}
