//! A ScreenCaptureKit stream of the measured app's windows within its window's frame, reduced to fingerprints and
//! the cells each frame changed.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGMainDisplayID;
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNumber, NSString};
use objc2_screen_capture_kit::{
    SCContentFilter, SCFrameStatus, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamDelegate,
    SCStreamFrameInfoDisplayTime, SCStreamFrameInfoStatus, SCStreamOutput, SCStreamOutputType,
};

use crate::compare::clock::{self, Timebase};
use crate::compare::detect::Frame;
use crate::compare::fingerprint::{Fingerprint, Image, Roi, THRESHOLD, fingerprint};
use crate::compare::stats::whole;

/// A whole frame as RGBA, kept for `--debug-frames`.
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) rgba: Vec<u8>,
}

/// How many frames of each kind arrived, to tell a silent stream from a still picture.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Counts {
    /// Complete frames, each recorded.
    pub(crate) complete: usize,
    /// Complete frames that differed from the one before.
    pub(crate) changed: usize,
    /// Idle, blank and other frames, which ScreenCaptureKit sends when nothing changed.
    pub(crate) other: usize,
    /// Complete frames whose time or pixels could not be read.
    pub(crate) unreadable: usize,
    /// How long after its display time the last frame arrived, in ns; far off zero means the clocks disagree.
    pub(crate) lag_ns: i64,
}

/// The frames recorded so far.
#[derive(Default)]
struct Recording {
    frames: Vec<Frame>,
    previous: Option<Fingerprint>,
    stopped: Option<String>,
    counts: Counts,
    snapshot: Option<Snapshot>,
}

struct Shared {
    timebase: Timebase,
    roi: Roi,
    keep_snapshots: bool,
    recording: Mutex<Recording>,
}

fn lock(shared: &Shared) -> MutexGuard<'_, Recording> {
    shared.recording.lock().unwrap_or_else(PoisonError::into_inner)
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "XtaskCompareFrames"]
    #[ivars = Arc<Shared>]
    struct Output;

    unsafe impl NSObjectProtocol for Output {}

    unsafe impl SCStreamOutput for Output {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(&self, _stream: &SCStream, buffer: &CMSampleBuffer, kind: SCStreamOutputType) {
            if kind == SCStreamOutputType::Screen {
                record(self.ivars(), buffer);
            }
        }
    }

    unsafe impl SCStreamDelegate for Output {
        #[unsafe(method(stream:didStopWithError:))]
        fn did_stop(&self, _stream: &SCStream, error: &NSError) {
            lock(self.ivars()).stopped = Some(error.localizedDescription().to_string());
        }
    }
);

impl Output {
    fn new(shared: Arc<Shared>) -> Retained<Output> {
        let this = Output::alloc().set_ivars(shared);
        // SAFETY: NSObject's init on a freshly allocated object whose ivars are set.
        unsafe { msg_send![super(this), init] }
    }
}

/// What ScreenCaptureKit says of a frame: its `SCFrameStatus` and when it was displayed, in host ticks.
fn frame_info(buffer: &CMSampleBuffer) -> Option<(isize, Option<u64>)> {
    // SAFETY: reading the attachments of a valid sample buffer without creating any.
    let attachments = unsafe { buffer.sample_attachments_array(false) }?;
    // SAFETY: the attachments are a CFArray of CFDictionaries with CFString keys, toll-free bridged with NSArray of
    // NSDictionary; the reference lives no longer than `attachments`.
    let array: &NSArray<NSDictionary<NSString, AnyObject>> = unsafe {
        CFRetained::as_ptr(&attachments)
            .cast::<NSArray<NSDictionary<NSString, AnyObject>>>()
            .as_ref()
    };
    let info = array.firstObject()?;
    // SAFETY: ScreenCaptureKit's constant keys, valid for the process' lifetime.
    let (status_key, time_key) = unsafe { (SCStreamFrameInfoStatus, SCStreamFrameInfoDisplayTime) };
    let status = info
        .objectForKey(status_key)?
        .downcast::<NSNumber>()
        .ok()?
        .integerValue();
    let ticks = info
        .objectForKey(time_key)
        .and_then(|time| time.downcast::<NSNumber>().ok())
        .map(|time| time.unsignedLongLongValue());
    Some((status, ticks))
}

/// The frame's BGRA pixels as RGBA.
fn snapshot(image: &Image<'_>) -> Snapshot {
    let mut rgba = Vec::with_capacity(image.width * image.height * 4);
    for row in 0..image.height {
        let start = row * image.stride;
        let Some(line) = image.bytes.get(start..start + image.width * 4) else {
            break;
        };
        for px in line.as_chunks::<4>().0 {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    Snapshot {
        width: image.width,
        height: rgba.len() / (image.width * 4).max(1),
        rgba,
    }
}

/// The fingerprint of the frame's `roi`, with the whole frame if `keep` asks for it, if its pixels can be read.
fn print_of(buffer: &CMSampleBuffer, roi: Roi, keep: bool) -> Option<(Fingerprint, Option<Snapshot>)> {
    // SAFETY: the image buffer of a valid sample buffer, retained for this function.
    let image = unsafe { buffer.image_buffer() }?;
    // SAFETY: locking a pixel buffer for reading, unlocked below before it is released.
    if unsafe { CVPixelBufferLockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly) } != 0 {
        return None;
    }
    let base = CVPixelBufferGetBaseAddress(&image);
    let (width, height, stride) = (
        CVPixelBufferGetWidth(&image),
        CVPixelBufferGetHeight(&image),
        CVPixelBufferGetBytesPerRow(&image),
    );
    let print = (!base.is_null()).then(|| {
        // SAFETY: a locked BGRA pixel buffer holds `stride * height` readable bytes from its base address.
        let bytes = unsafe { std::slice::from_raw_parts(base.cast::<u8>(), stride * height) };
        let image = Image {
            bytes,
            width,
            height,
            stride,
        };
        (fingerprint(&image, roi), keep.then(|| snapshot(&image)))
    });
    // SAFETY: undoes the lock above.
    unsafe { CVPixelBufferUnlockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly) };
    print
}

/// Records a complete frame: its time, the cells it changed and how much text it shows.
fn record(shared: &Shared, buffer: &CMSampleBuffer) {
    let Some((status, ticks)) = frame_info(buffer) else {
        lock(shared).counts.unreadable += 1;
        return;
    };
    if status != SCFrameStatus::Complete.0 {
        lock(shared).counts.other += 1;
        return;
    }
    // SAFETY: the presentation time of a valid sample buffer, which ScreenCaptureKit stamps on the host clock.
    let presented = || clock::seconds_to_ns(unsafe { buffer.presentation_time_stamp().seconds() });
    let Some(at_ns) = ticks.map(|t| shared.timebase.ns(t)).or_else(presented) else {
        lock(shared).counts.unreadable += 1;
        return;
    };
    let Some((print, picture)) = print_of(buffer, shared.roi, shared.keep_snapshots) else {
        lock(shared).counts.unreadable += 1;
        return;
    };
    let lag = i64::try_from(clock::now_ns()).unwrap_or(i64::MAX) - i64::try_from(at_ns).unwrap_or(i64::MAX);
    let mut recording = lock(shared);
    recording.counts.complete += 1;
    recording.counts.lag_ns = lag;
    if picture.is_some() {
        recording.snapshot = picture;
    }
    let changes = recording
        .previous
        .as_ref()
        .map(|previous| print.changes_from(previous, THRESHOLD))
        .filter(|changes| !changes.is_empty());
    if changes.is_some() {
        recording.counts.changed += 1;
    }
    let spread = print.spread();
    recording.previous = Some(print);
    recording.frames.push(Frame { at_ns, changes, spread });
}

/// A value of ScreenCaptureKit's handed from its queue to the waiting thread.
struct Handed<T>(T);

// SAFETY: the value is only handed over once and then used by the receiving thread alone; ScreenCaptureKit's
// content and errors are immutable objects.
unsafe impl<T> Send for Handed<T> {}

/// The windows, displays and apps ScreenCaptureKit may capture.
pub(crate) fn shareable(timeout: Duration) -> Result<Retained<SCShareableContent>, String> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |content: *mut SCShareableContent, error: *mut NSError| {
        // SAFETY: ScreenCaptureKit hands a valid object or null each; retaining keeps it past the block.
        let (content, error) = unsafe { (Retained::retain(content), Retained::retain(error)) };
        let result = content
            .ok_or_else(|| error.map_or_else(|| "no content".to_string(), |e| e.localizedDescription().to_string()));
        let _ = tx.send(Handed(result));
    });
    // SAFETY: the completion handler is a valid block that lives as long as ScreenCaptureKit retains it.
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    match rx.recv_timeout(timeout) {
        Ok(Handed(result)) => result.map_err(|e| format!("ScreenCaptureKit lists nothing to capture: {e}")),
        Err(_) => Err(format!(
            "ScreenCaptureKit listed nothing within {} s",
            timeout.as_secs()
        )),
    }
}

/// Waits for a completion handler's error, if any; none means success.
fn completed(start: impl FnOnce(&block2::DynBlock<dyn Fn(*mut NSError)>), timeout: Duration) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |error: *mut NSError| {
        // SAFETY: a valid error or null, retained past the block.
        let error = unsafe { Retained::retain(error) }.map(|e| e.localizedDescription().to_string());
        let _ = tx.send(error);
    });
    start(&block);
    match rx.recv_timeout(timeout) {
        Ok(None) => Ok(()),
        Ok(Some(error)) => Err(error),
        Err(_) => Err(format!("no answer within {} s", timeout.as_secs())),
    }
}

/// A running stream of one app's windows.
pub(crate) struct Capture {
    stream: Retained<SCStream>,
    _output: Retained<Output>,
    _queue: DispatchRetained<DispatchQueue>,
    shared: Arc<Shared>,
}

impl Capture {
    /// Starts capturing the windows of `pid` within `frame`, in points of the main display, at up to 120 frames a
    /// second, one pixel per point.
    pub(crate) fn start(pid: libc::pid_t, frame: CGRect, roi: Roi, timeout: Duration) -> Result<Capture, String> {
        Capture::start_keeping(pid, frame, roi, false, timeout)
    }

    /// `start`, keeping the latest whole frame for `snapshot` if `keep` asks for it.
    pub(crate) fn start_keeping(
        pid: libc::pid_t,
        frame: CGRect,
        roi: Roi,
        keep: bool,
        timeout: Duration,
    ) -> Result<Capture, String> {
        let content = shareable(timeout)?;
        let display_id = CGMainDisplayID();
        // SAFETY: plain getters of the content ScreenCaptureKit returned.
        let (display, app) = unsafe {
            (
                content.displays().iter().find(|d| d.displayID() == display_id),
                content.applications().iter().find(|a| a.processID() == pid),
            )
        };
        let display = display.ok_or("ScreenCaptureKit does not list the main display")?;
        let app = app.ok_or_else(|| format!("ScreenCaptureKit does not list the process {pid} yet"))?;
        // SAFETY: a filter of a listed display and app, and a configuration of plain values.
        let (filter, config) = unsafe {
            let filter = SCContentFilter::initWithDisplay_includingApplications_exceptingWindows(
                SCContentFilter::alloc(),
                &display,
                &NSArray::from_retained_slice(&[app]),
                &NSArray::new(),
            );
            let config = SCStreamConfiguration::new();
            let origin = display.frame().origin;
            config.setSourceRect(CGRect::new(
                CGPoint::new(frame.origin.x - origin.x, frame.origin.y - origin.y),
                CGSize::new(frame.size.width, frame.size.height),
            ));
            config.setWidth(whole(frame.size.width));
            config.setHeight(whole(frame.size.height));
            config.setMinimumFrameInterval(CMTime::new(1, 120));
            config.setPixelFormat(kCVPixelFormatType_32BGRA);
            config.setQueueDepth(3);
            config.setShowsCursor(false);
            (filter, config)
        };
        let shared = Arc::new(Shared {
            timebase: clock::timebase(),
            roi,
            keep_snapshots: keep,
            recording: Mutex::new(Recording::default()),
        });
        let output = Output::new(Arc::clone(&shared));
        let queue = DispatchQueue::new("xtask.compare.frames", None);
        // SAFETY: the stream gets a valid filter, configuration and delegate, which `Capture` keeps alive.
        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &config,
                Some(ProtocolObject::from_ref(&*output)),
            )
        };
        // SAFETY: the output and its serial queue outlive the stream, both held by `Capture`.
        unsafe {
            stream.addStreamOutput_type_sampleHandlerQueue_error(
                ProtocolObject::from_ref(&*output),
                SCStreamOutputType::Screen,
                Some(&queue),
            )
        }
        .map_err(|e| format!("ScreenCaptureKit took no output: {}", e.localizedDescription()))?;
        completed(
            |done| {
                // SAFETY: starting a configured stream with a valid completion handler.
                unsafe { stream.startCaptureWithCompletionHandler(Some(done)) }
            },
            timeout,
        )
        .map_err(|e| format!("the capture did not start: {e}"))?;
        Ok(Capture {
            stream,
            _output: output,
            _queue: queue,
            shared,
        })
    }

    /// Runs `f` on the frames recorded so far.
    pub(crate) fn with_frames<R>(&self, f: impl FnOnce(&[Frame]) -> R) -> R {
        f(&lock(&self.shared).frames)
    }

    /// Forgets the recorded frames, keeping the last fingerprint to compare the next frame with.
    pub(crate) fn clear(&self) {
        lock(&self.shared).frames.clear();
    }

    /// The frames counted since the stream started.
    pub(crate) fn counts(&self) -> Counts {
        lock(&self.shared).counts
    }

    /// The fingerprint of the latest frame.
    pub(crate) fn print(&self) -> Option<Fingerprint> {
        lock(&self.shared).previous.clone()
    }

    /// The latest whole frame, if the stream keeps them.
    pub(crate) fn snapshot(&self) -> Option<Snapshot> {
        lock(&self.shared).snapshot.clone()
    }

    /// The region the fingerprints cover.
    pub(crate) fn roi(&self) -> Roi {
        self.shared.roi
    }

    /// Why the stream stopped on its own, if it did.
    pub(crate) fn stopped(&self) -> Option<String> {
        lock(&self.shared).stopped.clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = completed(
            |done| {
                // SAFETY: stopping a started stream with a valid completion handler.
                unsafe { self.stream.stopCaptureWithCompletionHandler(Some(done)) }
            },
            Duration::from_secs(3),
        );
    }
}
