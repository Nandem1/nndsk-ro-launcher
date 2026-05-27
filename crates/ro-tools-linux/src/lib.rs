//! Linux adapters for ro-tools-core ports.

pub mod input_perms;
pub mod keyboard;
pub mod proc_memory;
pub mod resolve_pid;
pub mod wine_process;
pub mod ydotool;

pub use input_perms::{detect_input_permissions, InputPermStatus};
pub use keyboard::{key_label_to_keycode, KeyboardMonitor, KeyboardPassthrough};

pub use proc_memory::{address_in_maps, ProcMemoryReader};
pub use resolve_pid::resolve_best_game_pid;
pub use wine_process::{
    find_game_processes, normalize_prefix, resolve_game_pid, GameProcessCandidate,
};
pub use ydotool::{
    autopot_input_installed, current_gid, current_uid, is_ydotool_responsive,
    is_ydotool_socket_ready, remove_stale_ydotool_socket, ydotool_installed, ydotool_socket_path,
    ydotoold_installed, LazyYdotoolInput, YdotoolInput,
};
