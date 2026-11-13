//! macOS's microphone permission. Lowspeak's recorder only reports it as missing and never asks, and macOS lists an
//! app under Privacy & Security > Microphone only once it has asked; so the first dictation asks.

use std::sync::{Mutex, PoisonError};

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

use ide_ui_common::util::main_soon;

/// Whether macOS has an answer about the microphone yet.
pub fn undecided() -> bool {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return false;
    }
    macos_undecided()
}

/// Whether macOS itself has no answer about the microphone yet, whatever the self-test plays.
pub fn macos_undecided() -> bool {
    // SAFETY: AVMediaTypeAudio is an immutable constant that AVFoundation sets when it loads.
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
        return false;
    };
    // SAFETY: `audio` is AVFoundation's own media type constant; the class method has no other precondition.
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
    status == AVAuthorizationStatus::NotDetermined
}

/// Shows macOS's question; `answered` runs on the main thread with whether the user allowed it.
pub fn ask(answered: impl FnOnce(bool) + Send + 'static) {
    // SAFETY: AVMediaTypeAudio is an immutable constant that AVFoundation sets when it loads.
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
        return answered(false);
    };
    let handler = RcBlock::new(once_on_main(answered));
    // SAFETY: `audio` is AVFoundation's media type constant, and the handler, which AVFoundation calls once on a queue
    // of its own, only takes the callback out of its mutex and hands it to the main thread.
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
}

/// `answered` as the handler AVFoundation calls on a queue of its own: the answer reaches the main thread, once.
pub fn once_on_main(answered: impl FnOnce(bool) + Send + 'static) -> impl Fn(Bool) + Send + Sync + 'static {
    let answered = Mutex::new(Some(answered));
    move |granted: Bool| {
        let answered = answered.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(answered) = answered {
            let granted = granted.as_bool();
            main_soon(move || answered(granted));
        }
    }
}
