//! Terminal sessions without a window: the user's shell in a PTY with its screen kept by `alacritty_terminal`, keys,
//! mouse and paste turned into bytes, shell integration for prompts and the working directory, the program in front
//! with coding agents recognised, and what pasting an image does.
//!
//! The view draws `Session::snapshot` and sends input through the session; events arrive on the session's reader
//! thread.

#![deny(unsafe_code)]

mod devserver;
pub mod event;
pub mod foreground;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
pub mod input;
pub mod integration;
mod io;
pub mod paste;
mod scan;
pub mod session;
pub mod snapshot;
#[allow(unsafe_code, reason = "libc calls and the poller registration on the PTY")]
mod sys;

pub use event::{Clipboard, Event, WorkingDirectory};
pub use foreground::{Agent, Foreground};
pub use input::{InputModes, Key, KeyAction, KeyCode, Modifiers, MouseAction, MouseButton, MouseEvent, NamedKey};
pub use paste::{PasteAction, Refusal};
pub use session::{CellSide, Options, Program, Scroll, SelectionKind, Session, Size};
pub use snapshot::{Cell, CellFlags, Cursor, CursorShape, Damage, Highlight, Palette, Point, Rgb, Snapshot};
