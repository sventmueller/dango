//! The terminal view: a layer-backed view that draws a session's screen and hands it keys, mouse, pastes and drops.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use ide_core::settings::{Settings, keys};
use ide_terminal::{Session, Size, Snapshot};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAccessibilityHighContrastAqua,
    NSAppearanceNameAccessibilityHighContrastDarkAqua, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBitmapImageRep, NSColor, NSDragOperation, NSDraggingInfo, NSEvent, NSLayoutConstraintOrientation, NSMenu,
    NSMenuItem, NSMenuItemValidation, NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypeString, NSStackView,
    NSStackViewDistribution, NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions, NSUserInterfaceLayoutDirection,
    NSUserInterfaceLayoutOrientation, NSView, NSViewLayerContentsRedrawPolicy, NSWindowDidBecomeKeyNotification,
    NSWindowDidChangeOcclusionStateNotification, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRange,
    NSRangePointer, NSRect, NSSize, NSString, NSTimer, NSUInteger,
};
use splat_i18n::t;

use super::banner::Banner;
use super::input::KeyState;
use super::mouse::MouseState;
use crate::gpu::Surface;
use crate::model::{self, Command, State};
use crate::palette::{self, Theme};
use crate::render::{self, Fonts};
use ide_ui_common::constant;
use ide_ui_common::util::{guarded, item, ns, separator, set_target};

/// What a terminal view keeps.
pub struct Ivars {
    pub(super) id: u64,
    /// The id of the window the view's tab is in.
    window: u64,
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
    /// Drawing on the GPU; none where the view draws with Core Graphics.
    pub(super) gpu: Option<RefCell<Surface>>,
    pub(super) blink: RefCell<Option<Retained<NSTimer>>>,
    observers: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
}

impl Drop for Ivars {
    fn drop(&mut self) {
        if let Some(timer) = self.blink.get_mut().take() {
            timer.invalidate();
        }
        remove_observers(self.observers.get_mut());
        self.mouse.stop_autoscroll();
        super::forget(self.id);
    }
}

pub(super) struct Banners {
    pub(super) dev_server: Banner,
    pub(super) notice: Banner,
    pub(super) url: RefCell<Option<String>>,
}

/// What a window notification the view observes calls.
type Handler = fn(&TerminalView);

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

        #[unsafe(method(wantsUpdateLayer))]
        fn wants_update_layer(&self) -> bool {
            self.ivars().gpu.is_some() && !self.ivars().state.picturing.get()
        }

        #[unsafe(method(cacheDisplayInRect:toBitmapImageRep:))]
        fn cache_display(&self, rect: NSRect, bitmap: &NSBitmapImageRep) {
            let picturing = &self.ivars().state.picturing;
            picturing.set(true);
            // SAFETY: The superclass's implementation, with this method's signature.
            let () = unsafe { msg_send![super(self), cacheDisplayInRect: rect, toBitmapImageRep: bitmap] };
            picturing.set(false);
        }

        #[unsafe(method(updateLayer))]
        fn update_layer(&self) {
            guarded((), || self.draw_gpu());
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let () = unsafe { msg_send![super(self), setFrameSize: size] };
            guarded((), || {
                self.layout_grid();
                self.redraw_layer();
            });
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            guarded((), || self.moved_to_window());
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing(&self) {
            guarded((), || {
                self.layout_grid();
                self.redraw_layer();
            });
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
                super::host().opened(self.window_id(), super::Opened::TerminalHere(self.cwd()));
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
    /// A terminal view for a tab of window `window` that starts in `cwd`, and what the tab shows: the view under its
    /// banners.
    pub fn new(mtm: MainThreadMarker, window: u64, cwd: PathBuf) -> (Retained<TerminalView>, Retained<NSView>) {
        let settings = Settings::load_user();
        super::set_option_as_meta(settings.get(&keys::OPTION_AS_META));
        let ivars = Ivars {
            id: super::next_id(),
            window,
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
            gpu: Surface::new().map(RefCell::new),
            blink: RefCell::new(None),
            observers: RefCell::new(vec![]),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSView's designated initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setWantsLayer(true);
        if let (Some(gpu), Some(layer)) = (&this.ivars().gpu, this.layer()) {
            this.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::OnSetNeedsDisplay);
            // AppKit sends `updateLayer` only for a backing layer of its own, so the Metal layer lies inside that one.
            layer.addSublayer(gpu.borrow().layer());
        }
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

    pub fn id(&self) -> u64 {
        self.ivars().id
    }

    /// The id of the window the view's tab is in.
    pub fn window_id(&self) -> u64 {
        self.ivars().window
    }

    /// The id of the view's tab.
    pub fn tab(&self) -> Option<isize> {
        self.ivars().tab.get()
    }

    /// The working directory the shell last reported, else the one it started in.
    pub fn cwd(&self) -> PathBuf {
        self.ivars().cwd.borrow().clone()
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

    fn context_menu(&self) -> Retained<NSMenu> {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        for entry in model::menu() {
            match entry {
                Some((title, command)) => {
                    let entry = item(mtm, title, Some(action(command)), "");
                    set_target(&entry, self);
                    menu.addItem(&entry);
                }
                None => menu.addItem(&separator(mtm)),
            }
        }
        menu
    }

    fn validate(&self, item: &NSMenuItem) -> bool {
        let Some(command) = item.action().and_then(command) else {
            return true;
        };
        let started = self.ivars().session.borrow().is_some();
        let selection = command == Command::Copy && self.selected_text().is_some();
        model::enabled(command, started, selection, || {
            super::clip::accepts(&NSPasteboard::generalPasteboard())
        })
    }

    pub(super) fn fonts(&self) -> Rc<Fonts> {
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
    pub fn set_font_size(&self, size: f64) {
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
    pub(super) fn grid_size(&self) -> Option<Size> {
        let bounds = self.bounds().size;
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return None;
        }
        let m = self.metrics();
        let (columns, lines) = m.fit(bounds.width, bounds.height);
        let lines = self.ivars().state.grid_lines(lines);
        Some(Size {
            columns,
            lines,
            cell_width: m.pixels(m.cell_width),
            cell_height: m.pixels(m.cell_height),
        })
    }

    /// Fits the grid to the view and tells the program of a new size.
    pub(super) fn layout_grid(&self) {
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
        let handlers: [(&NSString, Handler); 3] = [
            (constant!(NSWindowDidBecomeKeyNotification), TerminalView::focus_changed),
            (constant!(NSWindowDidResignKeyNotification), TerminalView::focus_changed),
            (
                constant!(NSWindowDidChangeOcclusionStateNotification),
                TerminalView::occlusion_changed,
            ),
        ];
        for (name, handler) in handlers {
            let view = Weak::from(self);
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                guarded((), || {
                    if let Some(view) = view.load() {
                        handler(&view);
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
}

/// The action of the menu item for `command`.
fn action(command: Command) -> Sel {
    match command {
        Command::Copy => sel!(copy:),
        Command::Paste => sel!(paste:),
        Command::SelectAll => sel!(selectAll:),
        Command::Clear => sel!(ideClearTerminal:),
        Command::NewTerminalHere => sel!(ideNewTerminalHere:),
    }
}

/// The command a menu item's `action` stands for.
fn command(action: Sel) -> Option<Command> {
    [
        Command::Copy,
        Command::Paste,
        Command::SelectAll,
        Command::Clear,
        Command::NewTerminalHere,
    ]
    .into_iter()
    .find(|&command| self::action(command) == action)
}
