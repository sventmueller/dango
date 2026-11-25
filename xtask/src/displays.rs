//! Virtual displays for the selftest, with the size and scale of the main display, placed beyond the bottom right
//! corner of the user's displays so that they touch them at a corner at most and the pointer does not wander onto
//! them. They use CoreGraphics' private `CGVirtualDisplay`, as display tools such as DeskPad do, and go away with the
//! value that made them or with this process.

use std::time::{Duration, Instant};

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::msg_send;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBeginDisplayConfiguration, CGCompleteDisplayConfiguration, CGConfigureDisplayOrigin, CGConfigureOption,
    CGDisplayBounds, CGDisplayConfigRef, CGDisplayCopyDisplayMode, CGDisplayMode, CGError, CGGetActiveDisplayList,
    CGMainDisplayID, CGWindowListCopyWindowInfo, CGWindowListOption,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

/// How long macOS may take to show new displays to its apps.
const APPEARING: Duration = Duration::from_secs(10);

/// Displays that last as long as this value.
pub(crate) struct Displays {
    made: Vec<Retained<AnyObject>>,
    /// The display number of each, which the app finds its screen by.
    pub(crate) ids: Vec<u32>,
    _queue: DispatchRetained<DispatchQueue>,
}

impl Displays {
    /// `count` displays, or why macOS made none.
    pub(crate) fn make(count: u32) -> Result<Displays, String> {
        let mode = CGDisplayCopyDisplayMode(CGMainDisplayID()).ok_or("the main display has no mode")?;
        let points = (CGDisplayMode::width(Some(&mode)), CGDisplayMode::height(Some(&mode)));
        let scale = (CGDisplayMode::pixel_width(Some(&mode)) / points.0.max(1)).max(1);
        let user = active().into_iter().map(|id| CGDisplayBounds(id)).collect::<Vec<_>>();
        let queue = DispatchQueue::new("selftest displays", None);
        let mut displays = Displays {
            made: Vec::new(),
            ids: Vec::new(),
            _queue: queue,
        };
        for serial in 1..=count {
            let (display, id) = displays.make_one(serial, points, scale)?;
            displays.made.push(display);
            displays.ids.push(id);
        }
        displays.place(&user)?;
        displays.wait_until_shown(&user)?;
        Ok(displays)
    }

    fn make_one(
        &self,
        serial: u32,
        (width, height): (usize, usize),
        scale: usize,
    ) -> Result<(Retained<AnyObject>, u32), String> {
        let class = |name: &std::ffi::CStr| AnyClass::get(name).ok_or("macOS has no CGVirtualDisplay");
        let (wide, high) = (
            u32::try_from(width).unwrap_or(1920),
            u32::try_from(height).unwrap_or(1080),
        );
        let pixels = |points: u32| points.saturating_mul(u32::try_from(scale).unwrap_or(1));
        // SAFETY: The messages are those of CoreGraphics' virtual display classes, each with the argument types its
        // property or initializer declares; the queue outlives the displays as a field of the same value.
        unsafe {
            let descriptor: Retained<AnyObject> = msg_send![class(c"CGVirtualDisplayDescriptor")?, new];
            let name = NSString::from_str(&format!("Selftest {serial}"));
            let _: () = msg_send![&*descriptor, setName: &*name];
            let _: () = msg_send![&*descriptor, setMaxPixelsWide: pixels(wide)];
            let _: () = msg_send![&*descriptor, setMaxPixelsHigh: pixels(high)];
            let millimeters = CGSize::new(f64::from(wide) * 0.2, f64::from(high) * 0.2);
            let _: () = msg_send![&*descriptor, setSizeInMillimeters: millimeters];
            let _: () = msg_send![&*descriptor, setVendorID: 0x5e1f_u32];
            let _: () = msg_send![&*descriptor, setProductID: 0x5e1f_u32];
            let _: () = msg_send![&*descriptor, setSerialNum: serial];
            let queue = std::ptr::from_ref::<DispatchQueue>(&self._queue).cast::<AnyObject>();
            let _: () = msg_send![&*descriptor, setQueue: queue];
            let display: Allocated<AnyObject> = msg_send![class(c"CGVirtualDisplay")?, alloc];
            let display: Option<Retained<AnyObject>> = msg_send![display, initWithDescriptor: &*descriptor];
            let display = display.ok_or("macOS made no virtual display")?;
            let mode: Allocated<AnyObject> = msg_send![class(c"CGVirtualDisplayMode")?, alloc];
            let mode: Retained<AnyObject> = msg_send![mode, initWithWidth: wide, height: high, refreshRate: 60.0_f64];
            let settings: Retained<AnyObject> = msg_send![class(c"CGVirtualDisplaySettings")?, new];
            let _: () = msg_send![&*settings, setHiDPI: u32::from(scale > 1)];
            let modes = NSArray::from_retained_slice(&[mode]);
            let _: () = msg_send![&*settings, setModes: &*modes];
            let applied: Bool = msg_send![&*display, applySettings: &*settings];
            if !applied.as_bool() {
                return Err(format!("macOS took no mode of {wide}×{high} for a virtual display"));
            }
            let id: u32 = msg_send![&*display, displayID];
            Ok((display, id))
        }
    }

    /// Chains the displays corner to corner from the bottom right corner of `user`'s displays, for this process only.
    fn place(&self, user: &[CGRect]) -> Result<(), String> {
        let (mut x, mut y) = corner(user);
        let mut config: CGDisplayConfigRef = std::ptr::null_mut();
        // SAFETY: `config` is an out-parameter that the completion below consumes.
        let begun = unsafe { CGBeginDisplayConfiguration(&raw mut config) };
        check(begun, "begin placing the displays")?;
        for id in &self.ids {
            let size = CGDisplayBounds(*id).size;
            // SAFETY: `config` is the configuration begun above and `id` a display that exists.
            let placed = unsafe { CGConfigureDisplayOrigin(config, *id, whole(x), whole(y)) };
            check(placed, "place a display")?;
            (x, y) = (x + size.width, y + size.height);
        }
        // SAFETY: `config` is the configuration begun above, completed once.
        let done = unsafe { CGCompleteDisplayConfiguration(config, CGConfigureOption::ForAppOnly) };
        check(done, "place the displays")
    }

    fn wait_until_shown(&self, user: &[CGRect]) -> Result<(), String> {
        let started = Instant::now();
        let (x, y) = corner(user);
        while started.elapsed() < APPEARING {
            let shown = active();
            if self.ids.iter().all(|id| shown.contains(id))
                && self
                    .ids
                    .iter()
                    .all(|id| CGDisplayBounds(*id).origin.x >= x && CGDisplayBounds(*id).origin.y >= y)
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "the virtual displays did not show within {} s",
            APPEARING.as_secs()
        ))
    }
}

/// Where a window of the process `pid` shows on a display that is none of `ids`, if one does.
pub(crate) fn outside(pid: u32, ids: &[u32]) -> Option<String> {
    let user: Vec<CGRect> = active()
        .into_iter()
        .filter(|id| !ids.contains(id))
        .map(|id| CGDisplayBounds(id))
        .collect();
    let (frame, layer, title) = windows(pid)
        .into_iter()
        .find(|(frame, _, _)| user.iter().any(|display| overlap(frame, display)))?;
    Some(format!(
        "{}, {} ({} × {}), layer {layer}, titled {title:?}",
        frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
    ))
}

fn check(error: CGError, what: &str) -> Result<(), String> {
    if error == CGError::Success {
        Ok(())
    } else {
        Err(format!("macOS could not {what}: CGError {}", error.0))
    }
}

/// The numbers of the displays macOS shows now.
fn active() -> Vec<u32> {
    let mut ids = [0_u32; 32];
    let mut count = 0_u32;
    let room = u32::try_from(ids.len()).unwrap_or(0);
    // SAFETY: The buffer holds as many numbers as the call is told.
    let error = unsafe { CGGetActiveDisplayList(room, ids.as_mut_ptr(), &raw mut count) };
    if error != CGError::Success {
        return Vec::new();
    }
    ids[..count as usize].to_vec()
}

/// The bottom right corner of `displays` together, in the global coordinates of CoreGraphics.
fn corner(displays: &[CGRect]) -> (f64, f64) {
    displays.iter().fold((0.0, 0.0), |(x, y), display| {
        (
            x.max(display.origin.x + display.size.width),
            y.max(display.origin.y + display.size.height),
        )
    })
}

/// A coordinate of the display arrangement, which macOS keeps in whole points.
fn whole(value: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "display coordinates are whole points far inside i32, and `as` saturates beyond"
    )]
    let whole = value.round() as i32;
    whole
}

fn overlap(a: &CGRect, b: &CGRect) -> bool {
    a.origin.x < b.origin.x + b.size.width
        && b.origin.x < a.origin.x + a.size.width
        && a.origin.y < b.origin.y + b.size.height
        && b.origin.y < a.origin.y + a.size.height
}

/// The frames of the windows of `pid` that show now, each with its layer and title.
fn windows(pid: u32) -> Vec<(CGRect, i32, String)> {
    let Some(list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionOnScreenOnly, 0) else {
        return Vec::new();
    };
    // SAFETY: The list is a CFArray of CFDictionaries with CFString keys, toll-free bridged; it outlives the borrow.
    let list: &NSArray<NSDictionary<NSString, AnyObject>> = unsafe {
        CFRetained::as_ptr(&list)
            .cast::<NSArray<NSDictionary<NSString, AnyObject>>>()
            .as_ref()
    };
    let number = |info: &NSDictionary<NSString, AnyObject>, key: &str| {
        info.objectForKey(&NSString::from_str(key))
            .and_then(|value| value.downcast::<NSNumber>().ok())
    };
    list.iter()
        .filter(|info| number(info, "kCGWindowOwnerPID").is_some_and(|n| n.unsignedIntValue() == pid))
        .filter_map(|info| {
            let bounds = info
                .objectForKey(&NSString::from_str("kCGWindowBounds"))?
                .downcast::<NSDictionary>()
                .ok()?;
            // SAFETY: The window server's bounds are a dictionary of CFString keys and CFNumber values.
            let bounds = unsafe { Retained::cast_unchecked::<NSDictionary<NSString, AnyObject>>(bounds) };
            let value = |key| number(&bounds, key).map(|n| n.doubleValue());
            let frame = CGRect::new(
                CGPoint::new(value("X")?, value("Y")?),
                CGSize::new(value("Width")?, value("Height")?),
            );
            let layer = number(&info, "kCGWindowLayer").map_or(0, |n| n.intValue());
            let title = info
                .objectForKey(&NSString::from_str("kCGWindowName"))
                .and_then(|value| value.downcast::<NSString>().ok())
                .map(|name| name.to_string())
                .unwrap_or_default();
            Some((frame, layer, title))
        })
        .filter(|(frame, _, _)| frame.size.width > 1.0 && frame.size.height > 1.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_displays_start_at_the_bottom_right_corner_of_all_the_users_displays() {
        let main = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(2560.0, 1440.0));
        let left = CGRect::new(CGPoint::new(-1920.0, 200.0), CGSize::new(1920.0, 1600.0));
        assert_eq!(corner(&[main, left]), (2560.0, 1800.0));
        assert_eq!(corner(&[]), (0.0, 0.0));
    }

    #[test]
    fn frames_that_only_touch_do_not_overlap() {
        let main = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(100.0, 100.0));
        let corner = CGRect::new(CGPoint::new(100.0, 100.0), CGSize::new(50.0, 50.0));
        let inside = CGRect::new(CGPoint::new(90.0, 90.0), CGSize::new(50.0, 50.0));
        assert!(!overlap(&main, &corner));
        assert!(overlap(&main, &inside));
    }
}
