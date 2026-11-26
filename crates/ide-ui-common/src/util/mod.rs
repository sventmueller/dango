//! Helpers every UI crate shares: AppKit shorthands, guards, the main thread's lanes, layout and threads.

mod appkit;
mod guard;
mod lanes;
mod layout;
pub mod live;
mod memory;
mod observe;
mod own_symbol;
pub mod stall;
mod thread;
mod undo;

pub use appkit::{
    acknowledge, borderless_button, four_colours, hide_from_voiceover, item, label, ns, separator, set_action,
    set_label, set_shortcut, set_target, set_title_element, submenu, symbol, symbol_at, symbol_button, tagged, tinted,
};
pub(crate) use guard::UndoGroupGuard;
pub use guard::{FlagGuard, OnDrop, with_editing};
pub use lanes::{after_frame, guarded, main_after, main_on_run_loop, main_soon};
pub use layout::{activate, auto_layout, center_in, center_y, fixed_size, pin_edges, pin_inset, pin_sides};
pub use memory::give_back_memory_soon;
pub use observe::{Observation, observe};
pub use own_symbol::{FACTORY_SYMBOL, is_own_symbol, own_symbol_in};
pub use thread::{Joined, spawn, spawn_joined, spawn_with};
pub use undo::undo_step;
