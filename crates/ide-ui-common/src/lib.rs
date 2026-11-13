//! What every AppKit part of the app shares: building controls and menu items, hops to the main thread, callbacks
//! that contain a panic, the system log, one undoable step in a text view, and the AI tab's sizes, fonts and colours
//! with the contrast rules of agent accents (E66).
//!
//! A system callback that panics gives its fallback instead of unwinding into the system's frames:
//!
//! ```
//! use ide_ui_common::util::guarded;
//!
//! assert_eq!(guarded(0, || 7), 7);
//! assert_eq!(guarded(0, || -> i32 { panic!("a bug") }), 0);
//! ```

pub mod diag;
pub mod file_outline;
pub mod panel;
pub mod registry;
pub mod style;
pub mod util;
pub mod window;
