use crate::models::memory::MemoryAccess;

pub const MEMORY_SESSION_MISSING: &str =
    "Sesión de memoria no registrada; cierra el juego y relánzalo desde RO-Launcher.";

pub const MEMORY_RELAY_ACTION: &str =
    "Cierra las instancias antiguas y relanza el juego desde RO-Launcher.";

pub fn memory_access_label(access: MemoryAccess) -> &'static str {
    match access {
        MemoryAccess::ProcessVmReadv => "Disponible mediante process_vm_readv",
        MemoryAccess::ProcMem => "Disponible mediante /proc/<pid>/mem",
        MemoryAccess::NoReadableWritableRegion => "Sin región legible/escribible para preflight",
        MemoryAccess::OutsideSupervisor => "Proceso fuera del supervisor",
        MemoryAccess::YamaDenied => "Permiso denegado por Yama",
        MemoryAccess::ProcessExitedOrReused => "Proceso terminado o PID reutilizado",
        MemoryAccess::BackendError => "Backend de memoria no disponible",
    }
}

pub fn memory_start_error(access: MemoryAccess) -> String {
    format!("{} {}", memory_access_label(access), MEMORY_RELAY_ACTION)
}

pub fn memory_access_preflight_token(access: MemoryAccess) -> &'static str {
    match access {
        MemoryAccess::ProcessVmReadv => "process_vm_readv",
        MemoryAccess::ProcMem => "proc_mem",
        MemoryAccess::NoReadableWritableRegion => "no_readable_writable_region",
        MemoryAccess::OutsideSupervisor => "outside_supervisor",
        MemoryAccess::YamaDenied => "yama_denied",
        MemoryAccess::ProcessExitedOrReused => "process_exited_or_reused",
        MemoryAccess::BackendError => "backend_error",
    }
}
