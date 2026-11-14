//! The document model every consumer without a window reads: the text of one file shared by all views that show it,
//! edits in AppKit's UTF-16 units with what a language server needs to follow them, positions in every unit, the file
//! it comes from and goes back to, changes made to that file by other programs, copies against a crash, colours,
//! bracket pairs and folds by lines, and its indentation and the whitespace a save tidies.
//!
//! AppKit keeps the text it shows and its undo; this model follows every edit the view reports through
//! `Document::apply`, and edits that start here, like a reload or an agent's new text, come back as `Change`s for the
//! view to make.

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

mod disk;
mod document;
mod edit;
mod explain;
pub mod merge;
pub mod recovery;
pub mod spacing;
pub mod syntax;
mod text;

pub use disk::{FileContent, FileKey, MAX_FILE_BYTES, ReadOnly, save_target, stamp};
pub use document::{
    DiskCheck, DiskProbe, Document, LineEnding, LineEndings, Outcome, SaveError, SaveJob, Saved, Snapshot,
};
pub use edit::{Change, Edit, EditError};
pub use splat_file_state::{Encoding, FileFormat, Fingerprint, Stamp};
pub use text::{Point, PositionError, Text, Unit};
