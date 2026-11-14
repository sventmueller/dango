//! How a terminal view brings its screen up: a snapshot per frame of output, drawn with Core Graphics or on the GPU,
//! the blinking cursor, and the word to VoiceOver that the text changed.

use std::ptr::NonNull;
use std::time::Instant;

use block2::RcBlock;
use ide_terminal::{Damage, Session, Snapshot};
use objc2::DefinedClass;
use objc2::rc::Weak;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSAccessibilityValueChangedNotification, NSGraphicsContext, NSWindowOcclusionState};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSDate, NSRect, NSTimer};
use splat_i18n::t;

use super::view::TerminalView;
use crate::model::{self, Frame};
use crate::palette::Theme;
use crate::render::{self, Canvas, Overlay};
use crate::sprites::QuadCanvas;
use ide_ui_common::constant;
use ide_ui_common::util::{guarded, main_after};

/// How long the cursor shows and hides while it blinks, in seconds.
const BLINK: f64 = 0.53;

impl TerminalView {
    /// Output arrived; a new snapshot follows within a frame unless the view is hidden.
    pub(super) fn redraw_requested(&self) {
        let wait = match self
            .ivars()
            .state
            .frame(Instant::now(), self.isHiddenOrHasHiddenAncestor())
        {
            Frame::Stale | Frame::Pending => return,
            Frame::Now => return self.refresh(false),
            Frame::After(wait) => wait,
        };
        let id = self.id();
        main_after(wait, move || {
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
        self.ivars().state.framed(Instant::now());
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
                    let selected = model::selection_lines(previous.as_ref().and_then(|b| b.selection), now.selection);
                    for line in changed.chain([before, now.cursor.point.line]).chain(selected) {
                        self.setNeedsDisplayInRect(m.line_rect(line, width));
                    }
                }
                _ => self.setNeedsDisplay(true),
            }
        }
        self.update_blink();
        self.announce();
    }

    /// Draws with Core Graphics, where there is no GPU and for pictures of the view.
    pub(super) fn draw(&self, dirty: NSRect) {
        let _interval = ide_process::signpost::begin(ide_process::signpost::name!("terminal frame"));
        let Some(context) = NSGraphicsContext::currentContext() else {
            return;
        };
        let ctx = context.CGContext();
        // Drawing the window into a bitmap passes a rectangle beyond the view and does not clip to it.
        let bounds = self.bounds();
        CGContext::clip_to_rect(Some(&ctx), bounds);
        let dirty = render::intersect(dirty, bounds);
        let fonts = self.fonts();
        self.with_frame(|theme, screen, overlay| match screen {
            Some(screen) => render::draw(&ctx, &fonts, theme, screen, overlay, dirty),
            None => render::fill(&ctx, theme.palette.background, dirty),
        });
    }

    /// Draws a frame on the GPU into the view's layer, or once the window shows again if it is hidden behind others.
    pub(super) fn draw_gpu(&self) {
        let _interval = ide_process::signpost::begin(ide_process::signpost::name!("terminal frame"));
        let Some(surface) = &self.ivars().gpu else { return };
        let shown = self
            .window()
            .is_some_and(|window| window.occlusionState().contains(NSWindowOcclusionState::Visible));
        // A hidden window takes no drawables back, so asking for one would stall until it times out.
        self.ivars().state.unpresented.set(!shown);
        if !shown {
            return;
        }
        let bounds = self.bounds();
        let fonts = self.fonts();
        self.with_frame(|theme, screen, overlay| {
            let paint = |canvas: &mut QuadCanvas| match screen {
                Some(screen) => render::paint(canvas, &fonts, theme, screen, overlay, bounds),
                None => canvas.fill(theme.palette.background, 1.0, bounds),
            };
            let clear = theme.palette.background;
            surface.borrow_mut().present(&fonts, bounds, clear, &paint);
        });
    }

    /// Runs `f` with what a frame shows: the colours, the screen once the session has started, and what lies over it.
    fn with_frame(&self, f: impl FnOnce(&Theme, Option<&Snapshot>, &Overlay)) {
        let theme = self.ivars().theme.borrow();
        let screen = self.ivars().screen.borrow();
        let state = &self.ivars().state;
        let marked = self.ivars().keys.marked.borrow();
        let final_line = screen.as_ref().and_then(|screen| {
            (state.exited.get() && screen.display_offset == 0)
                .then(|| (model::final_line(screen), t("[Prozess beendet]")))
        });
        let overlay = Overlay {
            focused: state.focused.get(),
            cursor_on: !state.cursor_off.get(),
            marked: &marked,
            final_line,
            flash: state.flashing(Instant::now()),
        };
        f(&theme, screen.as_ref(), &overlay);
    }

    /// A GPU frame follows a change of size or scale, which Core Graphics drawing gets from AppKit on its own.
    pub(super) fn redraw_layer(&self) {
        if self.ivars().gpu.is_some() {
            self.setNeedsDisplay(true);
        }
    }

    pub(super) fn occlusion_changed(&self) {
        if self.ivars().state.unpresented.get() {
            self.setNeedsDisplay(true);
        }
    }

    /// Starts or stops the cursor's blinking as the program asks while the view has the focus.
    pub(super) fn update_blink(&self) {
        let state = &self.ivars().state;
        let cursor_blinks = self
            .ivars()
            .screen
            .borrow()
            .as_ref()
            .is_some_and(|screen| screen.cursor.blinking && screen.cursor.visible);
        let blinking = state.blinking(cursor_blinks);
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
        if !self.ivars().state.announce(Instant::now()) {
            return;
        }
        let element: &AnyObject = self;
        let changed = constant!(NSAccessibilityValueChangedNotification);
        // SAFETY: The element is this view and the name AppKit's value-changed notification.
        unsafe { objc2_app_kit::NSAccessibilityPostNotification(element, changed) };
    }
}
