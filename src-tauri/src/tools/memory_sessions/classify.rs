use crate::models::memory::MemoryAccess;

#[derive(Debug, Clone, Copy)]
pub struct PreflightEvidence {
    pub identity_alive: bool,
    pub descendant_of_ancestor: bool,
    pub has_rw_region: bool,
    pub vm_result: Result<(), i32>,
    pub proc_mem_result: Result<(), i32>,
}

pub fn classify_preflight(evidence: PreflightEvidence) -> MemoryAccess {
    if !evidence.identity_alive {
        return MemoryAccess::ProcessExitedOrReused;
    }
    if !evidence.has_rw_region {
        return MemoryAccess::NoReadableWritableRegion;
    }
    if evidence.vm_result.is_ok() {
        return MemoryAccess::ProcessVmReadv;
    }
    if evidence.proc_mem_result.is_ok() {
        return MemoryAccess::ProcMem;
    }
    let vm_denied = evidence
        .vm_result
        .err()
        .is_some_and(|errno| errno == libc::EPERM || errno == libc::EACCES);
    let mem_denied = evidence
        .proc_mem_result
        .err()
        .is_some_and(|errno| errno == libc::EPERM || errno == libc::EACCES);
    if vm_denied && mem_denied {
        if evidence.descendant_of_ancestor {
            return MemoryAccess::YamaDenied;
        }
        return MemoryAccess::OutsideSupervisor;
    }
    MemoryAccess::BackendError
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_success_wins() {
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: true,
                descendant_of_ancestor: true,
                has_rw_region: true,
                vm_result: Ok(()),
                proc_mem_result: Err(libc::EPERM),
            }),
            MemoryAccess::ProcessVmReadv
        );
    }

    #[test]
    fn proc_mem_fallback() {
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: true,
                descendant_of_ancestor: true,
                has_rw_region: true,
                vm_result: Err(libc::EPERM),
                proc_mem_result: Ok(()),
            }),
            MemoryAccess::ProcMem
        );
    }

    #[test]
    fn yama_vs_outside_supervisor() {
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: true,
                descendant_of_ancestor: true,
                has_rw_region: true,
                vm_result: Err(libc::EPERM),
                proc_mem_result: Err(libc::EPERM),
            }),
            MemoryAccess::YamaDenied
        );
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: true,
                descendant_of_ancestor: false,
                has_rw_region: true,
                vm_result: Err(libc::EPERM),
                proc_mem_result: Err(libc::EPERM),
            }),
            MemoryAccess::OutsideSupervisor
        );
    }

    #[test]
    fn dead_identity_is_process_exited_or_reused() {
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: false,
                descendant_of_ancestor: true,
                has_rw_region: true,
                vm_result: Ok(()),
                proc_mem_result: Ok(()),
            }),
            MemoryAccess::ProcessExitedOrReused
        );
    }

    #[test]
    fn missing_rw_region_is_not_usable() {
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: true,
                descendant_of_ancestor: true,
                has_rw_region: false,
                vm_result: Ok(()),
                proc_mem_result: Ok(()),
            }),
            MemoryAccess::NoReadableWritableRegion
        );
    }

    #[test]
    fn backend_error_for_other_errno() {
        assert_eq!(
            classify_preflight(PreflightEvidence {
                identity_alive: true,
                descendant_of_ancestor: true,
                has_rw_region: true,
                vm_result: Err(libc::EFAULT),
                proc_mem_result: Err(libc::EFAULT),
            }),
            MemoryAccess::BackendError
        );
    }
}
