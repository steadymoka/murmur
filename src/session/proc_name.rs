/// Resolve a process name from its PID using platform-specific APIs.

#[cfg(target_os = "macos")]
pub fn from_pid(pid: libc::pid_t) -> Option<String> {
    extern "C" {
        fn proc_name(
            pid: libc::c_int,
            buffer: *mut libc::c_char,
            buffersize: u32,
        ) -> libc::c_int;
    }
    let mut buf = [0u8; 256];
    let len = unsafe { proc_name(pid, buf.as_mut_ptr() as *mut libc::c_char, buf.len() as u32) };
    if len > 0 {
        Some(String::from_utf8_lossy(&buf[..len as usize]).to_string())
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
pub fn from_pid(pid: libc::pid_t) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn from_pid(_pid: libc::pid_t) -> Option<String> {
    None
}
