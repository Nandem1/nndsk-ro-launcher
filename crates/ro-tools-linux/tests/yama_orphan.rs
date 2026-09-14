//! Linux fixture: intermediate exits and target is reparented away from the test runner.
//! With ptrace_scope=1, a non-ancestor cannot read the target's memory.

use ro_tools_core::MemoryReader;
use ro_tools_linux::{
    capture_process_identity, is_descendant_of, read_ppid, verify_process_identity,
    ProcMemoryReader,
};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const MAGIC: u32 = 0x015F_F908;

static TARGET_ADDR: AtomicU32 = AtomicU32::new(0);

fn read_ptrace_scope() -> Option<u32> {
    std::fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope")
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn read_ppid_from_stat(pid: u32) -> Option<u32> {
    read_ppid(pid).ok()
}

fn kill_by_identity(pid: u32) {
    let Some(identity) = capture_process_identity(pid) else {
        return;
    };
    assert!(
        verify_process_identity(&identity),
        "target identity changed before SIGKILL"
    );
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
    for _ in 0..50 {
        if !verify_process_identity(&identity) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn memory_read_denied(pid: u32, address: u32) -> bool {
    let reader = ProcMemoryReader::open(pid).expect("target exists");
    match reader.read_u32(address) {
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            msg.contains("not permitted")
                || msg.contains("eperm")
                || msg.contains("eacces")
                || msg.contains("sin permiso ptrace")
        }
        Ok(_) => false,
    }
}

fn run_target() -> ! {
    let size = std::mem::size_of::<u32>();
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_32BIT,
            -1,
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        eprintln!("mmap failed");
        std::process::exit(1);
    }
    let addr = ptr as u32;
    unsafe {
        *(ptr as *mut u32) = MAGIC;
    }
    println!("{pid} {addr}", pid = std::process::id(), addr = addr);
    let _ = std::io::stdout().flush();
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn close_inherited_fds_from(min_fd: i32) {
    unsafe {
        let max = libc::sysconf(libc::_SC_OPEN_MAX);
        let max = if max < 0 { 1024 } else { max as i32 };
        for fd in min_fd..max {
            libc::close(fd);
        }
    }
}

fn run_intermediate() -> ! {
    let exe = std::env::current_exe().expect("current_exe");
    let mut child = unsafe {
        Command::new(&exe)
            .env("RO_YAMA_ROLE", "target")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .pre_exec(|| {
                close_inherited_fds_from(3);
                Ok(())
            })
            .spawn()
    }
    .expect("spawn target");
    let stdout = child.stdout.take().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("read target line");
    print!("{line}");
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

fn run_runner() {
    let test_pid = std::process::id();
    let exe = std::env::current_exe().expect("current_exe");
    let mut intermediate = Command::new(&exe)
        .env("RO_YAMA_ROLE", "intermediate")
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn intermediate");
    let stdout = intermediate.stdout.take().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("read intermediate line");
    let _ = intermediate.wait();

    let mut parts = line.split_whitespace();
    let target_pid: u32 = parts.next().expect("pid").parse().expect("pid parse");
    let target_addr: u32 = parts.next().expect("addr").parse().expect("addr parse");
    TARGET_ADDR.store(target_addr, Ordering::SeqCst);

    std::thread::sleep(Duration::from_millis(100));

    let ppid = read_ppid_from_stat(target_pid).expect("target ppid");
    assert!(
        !is_descendant_of(target_pid, test_pid),
        "target should not remain descendant of test runner (ppid={ppid}, test={test_pid})"
    );

    let scope = read_ptrace_scope();
    if scope == Some(1) {
        assert!(
            memory_read_denied(target_pid, target_addr),
            "expected memory read denied under ptrace_scope=1 for non-ancestor"
        );
    } else {
        eprintln!("note: skipping Yama EPERM assertion (ptrace_scope={scope:?}, expected 1 on CI)");
    }

    kill_by_identity(target_pid);
}

fn main() {
    match std::env::var("RO_YAMA_ROLE").as_deref() {
        Ok("target") => run_target(),
        Ok("intermediate") => run_intermediate(),
        _ => run_runner(),
    }
}
