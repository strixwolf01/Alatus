// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Desktop environment detection and Wayland socket discovery.

pub mod gnome;
pub mod kde;
pub mod wlroots;

pub use gnome::*;
pub use kde::*;
pub use wlroots::*;

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopEnv {
    Kde,
    Gnome,
    Wlroots,
    #[default]
    Other,
}

pub fn detect_desktop() -> DesktopEnv {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();
    if desktop.contains("KDE") {
        DesktopEnv::Kde
    } else if desktop.contains("GNOME") || desktop.contains("UBUNTU") {
        DesktopEnv::Gnome
    } else if desktop.contains("HYPRLAND")
        || desktop.contains("SWAY")
        || desktop.contains("WLROOTS")
        || desktop.contains("WAYFIRE")
        || desktop.contains("RIVER")
    {
        DesktopEnv::Wlroots
    } else {
        DesktopEnv::Other
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandEnv {
    pub wayland_display: String,
    pub xdg_runtime_dir: String,
    pub xdg_current_desktop: Option<String>,
}

/// Dynamically discovers the active Wayland display environment.
pub fn resolve_wayland_env() -> Option<WaylandEnv> {
    let env_wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/run/user/{uid}")
    });
    let env_desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .or_else(|| std::env::var("XDG_SESSION_DESKTOP").ok());

    resolve_wayland_env_from(
        env_wayland.as_deref(),
        Path::new(&xdg_runtime_dir),
        env_desktop.as_deref(),
    )
}

/// Pure helper for resolving Wayland environment parameters from explicit inputs or socket scans.
pub fn resolve_wayland_env_from(
    env_wayland: Option<&str>,
    runtime_dir: &Path,
    env_desktop: Option<&str>,
) -> Option<WaylandEnv> {
    let desktop = env_desktop
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| Some("KDE".to_string()));

    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();

    let wayland_display = if let Some(w) = env_wayland.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        w.to_string()
    } else {
        if !runtime_dir.is_dir() {
            return None;
        }

        let mut candidate_sockets = Vec::new();
        if let Ok(entries) = fs::read_dir(runtime_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("wayland-") && !file_name.ends_with(".lock") {
                    use std::os::unix::fs::FileTypeExt;
                    if let Ok(meta) = entry.metadata()
                        && (meta.file_type().is_socket() || meta.is_file())
                    {
                        candidate_sockets.push(file_name);
                    }
                }
            }
        }

        candidate_sockets.sort();
        candidate_sockets.into_iter().next()?
    };

    Some(WaylandEnv {
        wayland_display,
        xdg_runtime_dir: runtime_dir_str,
        xdg_current_desktop: desktop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_resolve_wayland_env_from_explicit_env() {
        let dummy_runtime = Path::new("/run/user/1000");
        let res = resolve_wayland_env_from(Some("wayland-1"), dummy_runtime, Some("KDE"));
        assert!(res.is_some());
        let env = res.unwrap();
        assert_eq!(env.wayland_display, "wayland-1");
        assert_eq!(env.xdg_runtime_dir, "/run/user/1000");
        assert_eq!(env.xdg_current_desktop.as_deref(), Some("KDE"));
    }

    #[test]
    fn test_resolve_wayland_env_from_socket_scan() {
        let temp_dir =
            std::env::temp_dir().join(format!("alatus_wayland_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let socket_path = temp_dir.join("wayland-0");
        let lock_path = temp_dir.join("wayland-0.lock");
        let _ = fs::write(&socket_path, b"");
        let _ = fs::write(&lock_path, b"");

        let res = resolve_wayland_env_from(None, &temp_dir, None);
        assert!(res.is_some());
        let env = res.unwrap();
        assert_eq!(env.wayland_display, "wayland-0");
        assert_eq!(env.xdg_current_desktop.as_deref(), Some("KDE"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_wayland_env_from_no_sockets() {
        let temp_dir =
            std::env::temp_dir().join(format!("alatus_wayland_empty_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let lock_path = temp_dir.join("wayland-0.lock");
        let _ = fs::write(&lock_path, b"");

        let res = resolve_wayland_env_from(None, &temp_dir, None);
        assert!(res.is_none());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_wayland_env_from_nonexistent_dir() {
        let dummy = Path::new("/nonexistent/runtime/dir/test_123");
        let res = resolve_wayland_env_from(None, dummy, None);
        assert!(res.is_none());
    }
}

