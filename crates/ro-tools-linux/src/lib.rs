//! Linux adapters for ro-tools-core ports.

pub mod combat_uinput;
pub mod input_perms;
pub mod keyboard;
pub mod proc_memory;
pub mod resolve_pid;
pub mod wine_process;

pub use input_perms::{detect_input_permissions, detect_uinput_permissions, InputPermStatus};
pub use keyboard::{key_label_to_keycode, KeyboardMonitor, KeyboardPassthrough};

pub use combat_uinput::{CombatUinput, COMBAT_DEVICE_NAME};
pub use proc_memory::{
    address_in_maps, find_all_writable_bytes, find_all_writable_bytes_with_reader,
    find_first_writable_bytes, first_rw_u32_region, parse_writable_regions, read_launcher_uid,
    read_process_uid, read_ptrace_scope, scan_writable_u32, scan_writable_u32_with_reader,
    vm_read_errno, MemoryReadDiagnostic, ProcMemoryReader,
};
pub use resolve_pid::resolve_best_game_pid;
pub use wine_process::{
    capture_process_identity, find_game_processes, find_prefix_processes, is_descendant_of,
    is_leftover_from_comm_and_argv0, is_prefix_leftover_executable, is_prefix_leftover_process,
    normalize_prefix, process_executable_label, read_ppid, resolve_game_pid,
    verify_process_identity, GameProcessCandidate, ProcessIdentity,
};
