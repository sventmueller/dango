//! The pasteboard: copying the selection, pasting and dropping text, files and images (E9), and text programs copy
//! with OSC 52.
//!
//! Files come first, as Finder puts their names and icons on the pasteboard too; text comes before an image, as many
//! apps add a picture of what was copied.

use std::path::PathBuf;

use ide_terminal::{PasteAction, Refusal};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, ClassType};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF, NSPasteboardURLReadingFileURLsOnlyKey,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSNumber, NSURL};
use splat_i18n::tf;

use super::view::TerminalView;
use crate::util::{constant, ns};

/// Puts `text` on the general pasteboard.
pub fn store(text: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(&ns(text), constant!(NSPasteboardTypeString));
}

pub fn text(pasteboard: &NSPasteboard) -> Option<String> {
    pasteboard
        .stringForType(constant!(NSPasteboardTypeString))
        .map(|text| text.to_string())
}

/// The files on `pasteboard`, as Finder copies or drags them.
pub fn files(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let classes = NSArray::from_slice(&[NSURL::class()]);
    let yes = NSNumber::numberWithBool(true);
    let value: &AnyObject = &yes;
    let only_files = NSDictionary::from_slices(&[constant!(NSPasteboardURLReadingFileURLsOnlyKey)], &[value]);
    // SAFETY: Pasteboards read NSURL objects, and the option takes a boolean number.
    let urls = unsafe { pasteboard.readObjectsForClasses_options(&classes, Some(&only_files)) };
    let Some(urls) = urls else { return vec![] };
    urls.iter()
        .filter_map(|object| object.downcast::<NSURL>().ok())
        .filter_map(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .collect()
}

/// The image on `pasteboard` as PNG data.
pub fn png(pasteboard: &NSPasteboard) -> Option<Vec<u8>> {
    if let Some(data) = pasteboard.dataForType(constant!(NSPasteboardTypePNG)) {
        return Some(data.to_vec());
    }
    let tiff: Retained<NSData> = match pasteboard.dataForType(constant!(NSPasteboardTypeTIFF)) {
        Some(data) => data,
        None if NSImage::canInitWithPasteboard(pasteboard) => {
            NSImage::initWithPasteboard(NSImage::alloc(), pasteboard)?.TIFFRepresentation()?
        }
        None => return None,
    };
    let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
    // SAFETY: An empty properties dictionary is valid for PNG.
    let data = unsafe { bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) };
    data.map(|data| data.to_vec())
}

/// Whether `pasteboard` holds something the terminal takes.
pub fn accepts(pasteboard: &NSPasteboard) -> bool {
    text(pasteboard).is_some()
        || !files(pasteboard).is_empty()
        || pasteboard.dataForType(constant!(NSPasteboardTypePNG)).is_some()
        || NSImage::canInitWithPasteboard(pasteboard)
}

impl TerminalView {
    pub(super) fn copy_selection(&self) {
        if let Some(text) = self.selected_text() {
            store(&text);
        }
    }

    /// Pastes what `pasteboard` holds: files as their paths, text, else an image as `Session::paste_image` decides,
    /// which is returned.
    pub(super) fn paste_from(&self, pasteboard: &NSPasteboard) -> Option<PasteAction> {
        let paths = files(pasteboard);
        if !paths.is_empty() {
            self.with_session(|session| session.paste_paths(&paths));
            return None;
        }
        if let Some(text) = text(pasteboard) {
            self.with_session(|session| session.paste(&text));
            return None;
        }
        let png = png(pasteboard)?;
        let dir = ide_terminal::paste::default_image_dir();
        let pasted = self.with_session(|session| session.paste_image(&png, &dir))?;
        match &pasted {
            Ok(PasteAction::Refuse(Refusal::Remote(program))) => self.notify(&tf(
                "Kein Bild eingefügt: „{}“ läuft auf einem anderen Rechner, der den Pfad nicht kennt.",
                &[program],
            )),
            Err(error) => self.notify(&tf("Das Bild ließ sich nicht einfügen: {}", &[&error.to_string()])),
            Ok(_) => {}
        }
        pasted.ok()
    }

    /// Pastes files dropped onto the view as their paths, or dropped text; whether there was either.
    pub(super) fn drop_from(&self, pasteboard: &NSPasteboard) -> bool {
        let paths = files(pasteboard);
        let text = text(pasteboard);
        if paths.is_empty() && text.is_none() {
            return false;
        }
        self.with_session(|session| match text {
            _ if !paths.is_empty() => session.paste_paths(&paths),
            Some(text) => session.paste(&text),
            None => {}
        });
        if let Some(window) = self.window() {
            window.makeFirstResponder(Some(self));
        }
        true
    }
}
