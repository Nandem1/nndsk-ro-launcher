use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

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
        std::process::exit(1);
    }
    let addr = ptr as u32;
    unsafe {
        *(ptr as *mut u32) = 0x015F_F908;
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
    let exe = std::env::current_exe().expect("exe");
    let mut child = unsafe {
        Command::new(exe)
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
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    print!("{line}");
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

fn main() {
    match std::env::var("RO_YAMA_ROLE").as_deref() {
        Ok("target") => run_target(),
        Ok("intermediate") => run_intermediate(),
        _ => std::process::exit(2),
    }
}
