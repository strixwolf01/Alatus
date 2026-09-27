// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Desktop and OSD notification helpers for user session events.

use zbus::Connection;

/// Resolves the absolute path to a custom mode SVG icon, ensuring it exists on disk.
/// Falls back to writing the embedded SVG to runtime cache if not yet installed in system dirs.
pub fn resolve_mode_icon(mode_key: &str) -> String {
    let installed_path = format!("/usr/share/alatus/icons/modes/{mode_key}.svg");
    if std::path::Path::new(&installed_path).exists() {
        return installed_path;
    }

    let hicolor_path = format!(
        "/usr/share/icons/hicolor/scalable/apps/alatus-mode-{}.svg",
        mode_key.replace('_', "-")
    );
    if std::path::Path::new(&hicolor_path).exists() {
        return hicolor_path;
    }

    // Check workspace assets if running from source checkout
    let dev_path = format!("assets/icons/modes/{mode_key}.svg");
    if let Ok(canon) = std::fs::canonicalize(&dev_path)
        && canon.exists()
    {
        return canon.to_string_lossy().to_string();
    }

    // Fallback to standard FreeDesktop notification icons
    match mode_key {
        "quiet" => "power-profile-power-saver".to_string(),
        "balanced" => "power-profile-balanced".to_string(),
        "performance" | "full_speed" => "power-profile-performance".to_string(),
        _ => "preferences-system-power".to_string(),
    }
}

/// Formats the notification title, description, and custom icon path for a thermal mode.
pub fn thermal_mode_notification(mode: u32) -> (&'static str, &'static str, String) {
    match mode {
        0 => (
            "Balanced Mode",
            "Standard acoustic and power profile applied.",
            resolve_mode_icon("balanced"),
        ),
        1 => (
            "Quiet Mode",
            "Silent fan curves and energy-saving profile applied.",
            resolve_mode_icon("quiet"),
        ),
        2 => (
            "Performance Mode",
            "High boost clocks and dynamic cooling applied.",
            resolve_mode_icon("performance"),
        ),
        3 => (
            "Full Speed Mode",
            "Maximum cooling and sustained high performance.",
            resolve_mode_icon("full_speed"),
        ),
        _ => (
            "Thermal Mode",
            "Profile updated.",
            resolve_mode_icon("balanced"),
        ),
    }
}

pub async fn send_desktop_notification(
    session_conn: &Connection,
    summary: &str,
    body: &str,
) -> Result<u32, zbus::Error> {
    let proxy = zbus::Proxy::new(
        session_conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;

    let actions: Vec<&str> = Vec::new();
    let mut hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
        std::collections::HashMap::new();

    let icon_path = "/usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg";
    let icon_name = if std::path::Path::new(icon_path).exists() {
        hints.insert("image-path", zbus::zvariant::Value::from(icon_path));
        hints.insert("image_path", zbus::zvariant::Value::from(icon_path));
        icon_path
    } else {
        "io.strixwolf.alatus"
    };

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Alatus", 0u32, icon_name, summary, body, actions, hints, 5000i32,
            ),
        )
        .await?;

    Ok(id)
}

pub async fn send_osd_notification(
    session_conn: &Connection,
    summary: &str,
    body: &str,
    icon: &str,
) -> Result<u32, zbus::Error> {
    let proxy = zbus::Proxy::new(
        session_conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;

    let actions: Vec<&str> = Vec::new();
    let mut hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
        std::collections::HashMap::new();
    hints.insert("urgency", zbus::zvariant::Value::U8(1));
    hints.insert("transient", zbus::zvariant::Value::Bool(true));
    hints.insert(
        "category",
        zbus::zvariant::Value::from("x-kde.display-profile"),
    );
    hints.insert(
        "x-canonical-private-synchronous",
        zbus::zvariant::Value::from("thermal-profile-osd"),
    );

    if icon.starts_with('/') || icon.starts_with("file://") {
        hints.insert("image-path", zbus::zvariant::Value::from(icon));
        hints.insert("image_path", zbus::zvariant::Value::from(icon));
    }

    let id: u32 = proxy
        .call(
            "Notify",
            &("Alatus", 0u32, icon, summary, body, actions, hints, 2000i32),
        )
        .await?;

    Ok(id)
}
