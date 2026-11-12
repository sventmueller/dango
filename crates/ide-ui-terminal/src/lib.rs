//! How a terminal tab draws its screen (E8, E76): the monospaced faces and cells drawn with Core Graphics or as Metal
//! quads over a texture of sprites, the colours of both appearances, line and block characters as shapes, what a
//! ⌘-click points at, Mac keys as terminal keys, and what the view decides without AppKit. The view itself lives in
//! the app.
//!
//! Increase Contrast gives the dark appearance a black ground:
//!
//! ```
//! use ide_terminal::Rgb;
//! use ide_ui_terminal::palette::theme;
//!
//! assert_eq!(theme(true, true).palette.background, Rgb::new(0, 0, 0));
//! assert_ne!(theme(true, false).palette.background, Rgb::new(0, 0, 0));
//! ```

mod boxdraw;
pub mod gpu;
pub mod keys;
pub mod links;
pub mod model;
pub mod palette;
pub mod render;
pub mod sprites;
