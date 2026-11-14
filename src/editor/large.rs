//! Opening a large file: it is read on the disk lane and its tab opens once the read is done, so a slow volume never
//! holds up the main thread; what wanted its editor right away gets it then.

use std::path::{Path, PathBuf};

use ide_document::Document;

use super::{EditorTextView, open_editors, opened};
use crate::window::WindowController;

/// Files above this many bytes are read on the disk lane.
const LARGE_FILE: u64 = 1 << 20;

/// How many times `with_opened` looks again for the tab of a large file whose read runs, 50 ms apart.
const TRIES: u32 = 200;

/// Whether `path` is a file large enough to be read on the disk lane.
pub(super) fn is_large(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() > LARGE_FILE)
}

/// Reads the large file `path` on the disk lane and opens its tab then; whether the lane took the read.
pub(super) fn open(controller: &WindowController, path: &Path, font_size: f64, preview: bool) -> bool {
    let (window, path, read) = (controller.id(), path.to_path_buf(), path.to_path_buf());
    let queued = ide_core::lane::DISK.ask(
        move || ide_document::FileContent::read(&read),
        move |content| {
            crate::util::main_soon(move || {
                let Some(controller) = crate::host::window(window) else {
                    return;
                };
                if controller.tab_with_key(&path.to_string_lossy()).is_some() {
                    return controller.open_file(&path);
                }
                let content = content.unwrap_or_else(|| Err(std::io::ErrorKind::Interrupted.into()));
                let document = content.map(|content| Document::from_file(path.clone(), content));
                opened(&controller, &path, document, font_size, preview);
            });
        },
    );
    queued.is_ok()
}

/// Runs `then` with the editor of `path` in `controller`: now, or once the tab of a large file opens after its read.
pub(super) fn with_opened(
    controller: &WindowController,
    path: &Path,
    then: impl FnOnce(&EditorTextView) + Send + 'static,
) {
    look(controller.id(), path.to_path_buf(), TRIES, then);
}

fn look(window: u64, path: PathBuf, tries: u32, then: impl FnOnce(&EditorTextView) + Send + 'static) {
    let Some(controller) = crate::host::window(window) else {
        return;
    };
    let view = open_editors().into_iter().find(|v| {
        v.document().path() == Some(path.as_path()) && v.controller().is_some_and(|c| std::ptr::eq(&*c, &*controller))
    });
    match view {
        Some(view) => then(&view),
        None if tries > 0 && !crate::preview::is_document(&path) && is_large(&path) => {
            crate::util::main_after(std::time::Duration::from_millis(50), move || {
                look(window, path, tries - 1, then);
            });
        }
        None => {}
    }
}
