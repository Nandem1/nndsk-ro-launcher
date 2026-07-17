//! Linux adapters for ro-tools-core ports.

pub mod combat_uinput;
pub mod input_perms;
pub mod keyboard;
pub mod proc_memory;
pub mod resolve_pid;
pub mod wine_process;

pub use input_perms::{detect_input_permissions, detect_uinput_permissions, InputPermStatus};
pub use keyboard::{key_label_to_keycode, KeyboardMonitor, KeyboardPassthrough};

pub use combat_uinput::{CombatUinput, COMBAT_KEYBOARD_NAME, COMBAT_MOUSE_NAME};
pub use proc_memory::{address_in_maps, ProcMemoryReader};
pub use resolve_pid::resolve_best_game_pid;
pub use wine_process::{
    find_game_processes, normalize_prefix, resolve_game_pid, GameProcessCandidate,
};
