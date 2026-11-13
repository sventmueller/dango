//! A terminal tab's view and how it draws its screen (E8, E9, E76): the layer-backed view that hands a session keys,
//! mouse, pastes and drops and shows banners above it; the monospaced faces and cells drawn with Core Graphics or as
//! Metal quads over a texture of sprites, the colours of both appearances, line and block characters as shapes, what
//! a ⌘-click points at, Mac keys as terminal keys, and what the view decides without AppKit. The tab and the
//! detection of what runs in it stay in the app, which a view reaches through `Host`, naming its window by id.
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

mod access;
mod banner;
mod boxdraw;
mod clip;
mod frame;
pub mod gpu;
mod host;
mod input;
#[cfg(feature = "selftest")]
pub mod inspect;
pub mod keys;
pub mod links;
pub mod model;
mod mouse;
pub mod palette;
pub mod render;
mod session;
pub mod sprites;
mod view;

#[cfg(feature = "selftest")]
use host::latest_id;
pub use host::{Host, Opened, State, all_views, find, install, set_option_as_meta};
use host::{forget, host, listener, next_id, option_as_meta, register};
pub use view::TerminalView;
