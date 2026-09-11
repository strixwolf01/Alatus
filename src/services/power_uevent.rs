// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Kernel netlink uevent watcher for power supply changes (AC plug / unplug).

use tokio::sync::mpsc;

/// Spawns a background thread listening for kernel netlink uevents on `SUBSYSTEM=power_supply`.
/// Returns an mpsc receiver that emits `()` whenever a power supply transition occurs.
pub fn spawn_power_uevent_listener() -> Option<mpsc::Receiver<()>> {
    let (tx, rx) = mpsc::channel::<()>(16);

    let fd = unsafe {
        let sock = libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::NETLINK_KOBJECT_UEVENT,
        );
        if sock < 0 {
            tracing::warn!(
                "Failed to create AF_NETLINK socket: {}",
                std::io::Error::last_os_error()
            );
            return None;
        }

        let mut sa: libc::sockaddr_nl = std::mem::zeroed();
        sa.nl_family = libc::AF_NETLINK as u16;
        sa.nl_pid = 0; // Kernel assigns
        sa.nl_groups = 1; // Kernel broadcast group 1 (uevents)

        let res = libc::bind(
            sock,
            &sa as *const _ as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        );
        if res < 0 {
            tracing::warn!(
                "Failed to bind AF_NETLINK socket: {}",
                std::io::Error::last_os_error()
            );
            libc::close(sock);
            return None;
        }
        sock
    };

    let builder = std::thread::Builder::new().name("power-uevent".into());
    let spawn_res = builder.spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
            if n <= 0 {
                break;
            }

            let slice = &buf[..n as usize];
            // Uevents are sequences of NUL-terminated strings: ACTION=change\0SUBSYSTEM=power_supply\0...
            if slice.windows(12).any(|w| w == b"power_supply") && tx.blocking_send(()).is_err() {
                break;
            }
        }
        unsafe { libc::close(fd) };
    });

    if spawn_res.is_err() {
        unsafe { libc::close(fd) };
        return None;
    }

    Some(rx)
}
