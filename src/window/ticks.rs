//! Missed ticks of a window (SN-M2, docs/snappy.md): an action marked at its input misses its tick when the frame
//! that shows its effect is committed more than one refresh of the display after the input. The window's display link
//! runs only while a marked action waits, so an idle window costs nothing.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSView;
use objc2_foundation::{NSObject, NSObjectProtocol, NSRunLoop};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink};

use crate::util::{constant, guarded};

/// The refresh assumed before the display link said its own, 60 Hz.
const PERIOD: f64 = 1.0 / 60.0;
/// How many refreshes an action may wait for a tick before the link counts as not ticking.
#[cfg(any(test, feature = "selftest"))]
const STALE: f64 = 8.0;

/// An action marked and not yet decided.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Open {
    id: u64,
    /// When the input came, in media time.
    marked: f64,
    /// When the run loop committed the frame after the action's main thread step.
    committed: Option<f64>,
}

/// What the display link counted.
#[derive(Default)]
struct Count {
    open: RefCell<Vec<Open>>,
    next: Cell<u64>,
    met: Cell<usize>,
    missed: Cell<usize>,
    ticks: Cell<usize>,
    period: Cell<f64>,
}

/// What the actions marked since the last reading came to.
#[cfg(any(test, feature = "selftest"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reading {
    /// The display link did not tick for every action, so nothing can be said.
    Unavailable,
    /// Of `actions` decided, `missed` missed their tick.
    Counted { actions: usize, missed: usize },
}

/// Settles the `open` actions at a tick at `now`, one refresh `period` long: committed within a refresh of the input is
/// met, later or not by now and a refresh past the input is missed; how many of each.
fn decide(open: &mut Vec<Open>, now: f64, period: f64) -> (usize, usize) {
    let (mut met, mut missed) = (0, 0);
    open.retain(|action| match action.committed {
        Some(committed) if committed - action.marked <= period => {
            met += 1;
            false
        }
        Some(_) => {
            missed += 1;
            false
        }
        None if now - action.marked > period => {
            missed += 1;
            false
        }
        None => true,
    });
    (met, missed)
}

impl Count {
    fn tick(&self, link: &CADisplayLink) -> bool {
        self.ticks.set(self.ticks.get() + 1);
        let period = link.targetTimestamp() - link.timestamp();
        let period = if period > 0.0 { period } else { link.duration() };
        if period > 0.0 {
            self.period.set(period);
        }
        let (met, missed) = decide(&mut self.open.borrow_mut(), CACurrentMediaTime(), self.period());
        self.met.set(self.met.get() + met);
        self.missed.set(self.missed.get() + missed);
        for _ in 0..missed {
            drop(ide_process::signpost::begin(ide_process::signpost::name!(
                "missed tick"
            )));
        }
        !self.open.borrow().is_empty()
    }

    fn period(&self) -> f64 {
        let period = self.period.get();
        if period > 0.0 { period } else { PERIOD }
    }

    fn committed(&self, id: u64, at: f64) {
        if let Some(action) = self.open.borrow_mut().iter_mut().find(|a| a.id == id) {
            action.committed = Some(at);
        }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `TickTarget` does not implement Drop.
    /// The target of a window's display link for missed ticks.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEWindowTickTarget"]
    #[ivars = Weak<Count>]
    struct TickTarget;

    unsafe impl NSObjectProtocol for TickTarget {}

    impl TickTarget {
        #[unsafe(method(ideWindowTick:))]
        fn tick(&self, link: &CADisplayLink) {
            guarded((), || {
                let waiting = self.ivars().upgrade().is_some_and(|count| count.tick(link));
                if !waiting {
                    link.setPaused(true);
                }
            });
        }
    }
);

impl TickTarget {
    fn new(mtm: MainThreadMarker, count: &Rc<Count>) -> Retained<TickTarget> {
        let this = Self::alloc(mtm).set_ivars(Rc::downgrade(count));
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

/// The missed ticks of one window.
#[derive(Default)]
pub(crate) struct Ticks {
    count: Rc<Count>,
    link: RefCell<Option<Retained<CADisplayLink>>>,
}

impl Drop for Ticks {
    fn drop(&mut self) {
        // The link retains its target until it is invalidated.
        if let Some(link) = self.link.get_mut().take() {
            link.invalidate();
        }
    }
}

impl Ticks {
    /// Marks an action whose input came now, during the main thread step that carries it out, in the window of `view`.
    pub(crate) fn mark(&self, view: &NSView) {
        let count = &self.count;
        let id = count.next.get() + 1;
        count.next.set(id);
        count.open.borrow_mut().push(Open {
            id,
            marked: CACurrentMediaTime(),
            committed: None,
        });
        let weak = Rc::downgrade(count);
        crate::util::after_frame(move || {
            if let Some(count) = weak.upgrade() {
                count.committed(id, CACurrentMediaTime());
            }
        });
        self.run(view);
    }

    fn run(&self, view: &NSView) {
        let mut link = self.link.borrow_mut();
        let link = link.get_or_insert_with(|| {
            let target = TickTarget::new(view.mtm(), &self.count);
            // SAFETY: `ideWindowTick:` takes the link as its one argument; the target holds the count weakly, and
            // `Ticks` invalidates the link when it goes.
            let link = unsafe { view.displayLinkWithTarget_selector(&target, sel!(ideWindowTick:)) };
            let modes = constant!(objc2_foundation::NSRunLoopCommonModes);
            // SAFETY: The main run loop in its common modes, so ticks count while the user drags.
            unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), modes) };
            link
        });
        if link.isPaused() {
            link.setPaused(false);
        }
    }

    #[cfg(any(test, feature = "selftest"))]
    /// What the actions marked since the last reading came to, starting the count again; actions still waiting for
    /// their tick count once it comes. Unavailable when an action waited longer than [`STALE`] refreshes.
    pub(crate) fn take(&self) -> Reading {
        let count = &self.count;
        let now = CACurrentMediaTime();
        let stale = now - STALE * count.period();
        // An action that waited this long for a tick sits in a window the display link does not tick for, as one
        // hidden behind others.
        let lost = count.open.borrow().iter().filter(|a| a.marked < stale).count();
        count.open.borrow_mut().retain(|a| a.marked >= stale);
        let reading = if lost > 0 || (count.ticks.get() == 0 && count.met.get() + count.missed.get() == 0) {
            Reading::Unavailable
        } else {
            Reading::Counted {
                actions: count.met.get() + count.missed.get(),
                missed: count.missed.get(),
            }
        };
        count.met.set(0);
        count.missed.set(0);
        count.ticks.set(0);
        reading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(id: u64, marked: f64, committed: Option<f64>) -> Open {
        Open { id, marked, committed }
    }

    #[test]
    fn an_action_committed_within_a_refresh_meets_its_tick_and_a_later_one_misses_it() {
        let period = 1.0 / 60.0;
        let mut actions = vec![open(1, 10.0, Some(10.01)), open(2, 10.0, Some(10.03))];
        assert_eq!(decide(&mut actions, 10.04, period), (1, 1));
        assert!(actions.is_empty());
    }

    #[test]
    fn an_action_not_committed_waits_for_a_refresh_and_then_misses() {
        let period = 1.0 / 60.0;
        let mut actions = vec![open(1, 10.0, None)];
        assert_eq!(decide(&mut actions, 10.01, period), (0, 0));
        assert_eq!(actions.len(), 1);
        assert_eq!(decide(&mut actions, 10.02, period), (0, 1));
        assert!(actions.is_empty());
    }

    #[test]
    fn without_actions_nothing_was_read() {
        let ticks = Ticks::default();
        assert_eq!(ticks.take(), Reading::Unavailable);
    }

    #[test]
    fn an_action_whose_tick_never_came_makes_the_reading_unavailable() {
        let ticks = Ticks::default();
        ticks.count.met.set(3);
        ticks.count.ticks.set(3);
        ticks
            .count
            .open
            .borrow_mut()
            .push(open(1, CACurrentMediaTime() - 10.0, None));
        assert_eq!(ticks.take(), Reading::Unavailable);
        ticks.count.met.set(3);
        ticks.count.ticks.set(3);
        assert_eq!(ticks.take(), Reading::Counted { actions: 3, missed: 0 });
    }
}
