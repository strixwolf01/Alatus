//! Power monitor & dynamic display refresh rate switcher.
//!
//! Automatically runs `on ac` (120Hz) or `on bat` (60Hz) display modes
//! when the power adapter connects or disconnects, and manages DE accent sync.

use alatus_core::settings::AlatusSettings;
use alatus_ipc::LightingProxy;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::notifier::DesktopNotifier;

pub fn is_ac_connected() -> bool {
    // 1. Scan /sys/class/power_supply for any non-Battery power source (Mains, USB, UCSI, AC)
    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let path = entry.path();
            let type_path = path.join("type");
            if let Ok(t) = std::fs::read_to_string(type_path) {
                let trimmed = t.trim();
                if !trimmed.eq_ignore_ascii_case("battery") {
                    let online_path = path.join("online");
                    if let Ok(val) = std::fs::read_to_string(online_path) {
                        if val.trim() == "1" {
                            return true;
                        }
                    }
                }
            }
        }
    }

    // 2. Battery status check: if not Discharging, AC power is definitely connected
    if let Ok(status) = std::fs::read_to_string("/sys/class/power_supply/BAT0/status") {
        let s = status.trim();
        if !s.eq_ignore_ascii_case("discharging") {
            return true;
        } else {
            return false;
        }
    }

    true
}

pub fn get_on_ac_command() -> String {
    let search_paths = [
        "temp/on ac.txt",
        "/usr/share/alatus/on ac.txt",
        "/usr/share/alatus/on-ac.txt",
        "/etc/alatus/on ac.txt",
        "/etc/alatus/on-ac.txt",
    ];
    for p in search_paths {
        if let Ok(content) = std::fs::read_to_string(p) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        for name in &["on-ac.txt", "on ac.txt", "on-ac.sh"] {
            let p = std::path::Path::new(&home).join(format!(".config/alatus/{name}"));
            if let Ok(content) = std::fs::read_to_string(p) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    "kscreen-doctor output.eDP-1.mode.1".to_string()
}

pub fn get_on_bat_command() -> String {
    let search_paths = [
        "temp/on bat.txt",
        "/usr/share/alatus/on bat.txt",
        "/usr/share/alatus/on-bat.txt",
        "/etc/alatus/on bat.txt",
        "/etc/alatus/on-bat.txt",
    ];
    for p in search_paths {
        if let Ok(content) = std::fs::read_to_string(p) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        for name in &["on-bat.txt", "on bat.txt", "on-bat.sh"] {
            let p = std::path::Path::new(&home).join(format!(".config/alatus/{name}"));
            if let Ok(content) = std::fs::read_to_string(p) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    "kscreen-doctor output.eDP-1.mode.2".to_string()
}

pub fn get_kde_accent_color() -> Option<(u8, u8, u8)> {
    // 1. KDE kdeglobals (instant file read, <0.1ms)
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::Path::new(&home).join(".config/kdeglobals");
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut general_accent = None;
            let mut selection_bg = None;
            let mut in_general = false;
            let mut in_selection = false;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_general = trimmed.eq_ignore_ascii_case("[General]");
                    in_selection = trimmed.eq_ignore_ascii_case("[Colors:Selection]");
                    continue;
                }

                if in_general && trimmed.starts_with("AccentColor=") {
                    let val = trimmed.trim_start_matches("AccentColor=").trim();
                    if let Some(rgb) = parse_comma_rgb(val) {
                        general_accent = Some(rgb);
                    }
                } else if in_selection && trimmed.starts_with("BackgroundNormal=") {
                    let val = trimmed.trim_start_matches("BackgroundNormal=").trim();
                    if let Some(rgb) = parse_comma_rgb(val) {
                        selection_bg = Some(rgb);
                    }
                }
            }

            if let Some(rgb) = general_accent.or(selection_bg) {
                return Some(rgb);
            }
        }
    }

    // 2. Try FreeDesktop Portal
    if let Ok(output) = std::process::Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "Read",
            "ss",
            "org.freedesktop.appearance",
            "accent-color",
        ])
        .output()
    {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            if let Some(start) = out_str.find("(ddd)") {
                let rest = &out_str[start + 5..];
                let nums: Vec<f64> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                if nums.len() >= 3 {
                    let r = (nums[0] * 255.0).round().clamp(0.0, 255.0) as u8;
                    let g = (nums[1] * 255.0).round().clamp(0.0, 255.0) as u8;
                    let b = (nums[2] * 255.0).round().clamp(0.0, 255.0) as u8;
                    return Some((r, g, b));
                }
            }
        }
    }

    None
}

fn current_epoch_sec() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn start_idle_activity_monitor(
    mut shutdown: watch::Receiver<bool>,
) -> std::sync::Arc<std::sync::atomic::AtomicU64> {
    use std::sync::atomic::{AtomicU64, Ordering};
    let last_active = std::sync::Arc::new(AtomicU64::new(current_epoch_sec()));
    let last_active_clone = last_active.clone();

    tokio::spawn(async move {
        let mut watched_nodes = std::collections::HashSet::new();
        loop {
            if let Ok(entries) = std::fs::read_dir("/dev/input") {
                for entry in entries.flatten() {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name.starts_with("event") && !watched_nodes.contains(&file_name) {
                        let path = entry.path();
                        if let Ok(device) = evdev::Device::open(&path) {
                            if let Ok(mut stream) = device.into_event_stream() {
                                watched_nodes.insert(file_name.clone());
                                let last_act = last_active_clone.clone();
                                let mut shut = shutdown.clone();
                                tokio::spawn(async move {
                                    loop {
                                        tokio::select! {
                                            _ = shut.changed() => break,
                                            res = stream.next_event() => {
                                                match res {
                                                    Ok(_) => {
                                                        last_act.store(current_epoch_sec(), Ordering::Relaxed);
                                                    }
                                                    Err(_) => break,
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }

            tokio::select! {
                _ = shutdown.changed() => break,
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
        }
    });

    last_active
}

fn parse_comma_rgb(val: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = val.split(',').collect();
    if parts.len() >= 3 {
        let r = parts[0].trim().parse::<u8>().ok()?;
        let g = parts[1].trim().parse::<u8>().ok()?;
        let b = parts[2].trim().parse::<u8>().ok()?;
        return Some((r, g, b));
    }
    None
}

async fn execute_shell_command(cmd_str: &str) {
    tracing::info!("Executing display mode command: {}", cmd_str);
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(cmd_str);
    if std::env::var("XDG_RUNTIME_DIR").is_err() {
        cmd.env("XDG_RUNTIME_DIR", "/run/user/1000");
    }
    if std::env::var("WAYLAND_DISPLAY").is_err() {
        cmd.env("WAYLAND_DISPLAY", "wayland-0");
    }
    if std::env::var("DISPLAY").is_err() {
        cmd.env("DISPLAY", ":0");
    }
    let res = cmd.status().await;
    tracing::info!("Display mode command completed with result: {:?}", res);
}

pub async fn run_power_listener(
    notifier: Arc<DesktopNotifier>,
    mut shutdown: watch::Receiver<bool>,
    lighting_proxy: Option<LightingProxy<'static>>,
) {
    use std::sync::atomic::Ordering;
    tracing::info!("Starting Power & Display Refresh Monitor...");

    let mut last_ac: Option<bool> = None;
    let mut last_accent: Option<(u8, u8, u8)> = None;
    let mut interval = tokio::time::interval(Duration::from_millis(1000));

    let last_activity_tracker = start_idle_activity_monitor(shutdown.clone());
    let mut backlight_idle_asleep = false;

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = interval.tick() => {}
        }

        let settings = AlatusSettings::load();
        let ac_online = is_ac_connected();

        // Check for AC / Battery transitions
        if last_ac != Some(ac_online) {
            let is_initial = last_ac.is_none();
            last_ac = Some(ac_online);

            if settings.auto_refresh {
                if ac_online {
                    let cmd = get_on_ac_command();
                    execute_shell_command(&cmd).await;
                    if !is_initial {
                        let _ = notifier
                            .notify(
                                "Display Refresh Rate",
                                "Switched to 120Hz (AC Connected)",
                                "video-display",
                                2000,
                            )
                            .await;
                    }
                } else {
                    let cmd = get_on_bat_command();
                    execute_shell_command(&cmd).await;
                    if !is_initial {
                        let _ = notifier
                            .notify(
                                "Display Refresh Rate",
                                "Switched to 60Hz (Battery Mode)",
                                "battery",
                                2000,
                            )
                            .await;
                    }
                }
            }
        }

        // Check for Desktop Accent Color sync
        if settings.sync_accent_color {
            if let Some((r, g, b)) = get_kde_accent_color() {
                if last_accent != Some((r, g, b)) {
                    last_accent = Some((r, g, b));
                    if let Some(ref lp) = lighting_proxy {
                        let _ = lp.set_color(r, g, b).await;
                        tracing::info!("Synced keyboard backlight to DE accent color: #{r:02X}{g:02X}{b:02X}");
                    }
                }
            }
        }

        // Check for Keyboard Backlight Idle Timeout
        let now_sec = current_epoch_sec();
        let last_sec = last_activity_tracker.load(Ordering::Relaxed);
        let idle_sec = now_sec.saturating_sub(last_sec);

        let (should_sleep, timeout_sec) = match settings.kbd_idle_mode.as_str() {
            "battery_ac" => (true, (settings.kbd_idle_timeout_ac_min as u64) * 60),
            "battery_only" => (!ac_online, (settings.kbd_idle_timeout_bat_min as u64) * 60),
            _ => (false, 0),
        };

        if should_sleep && idle_sec >= timeout_sec {
            if !backlight_idle_asleep {
                if let Some(ref lp) = lighting_proxy {
                    let _ = lp.set_brightness(0).await;
                    backlight_idle_asleep = true;
                    tracing::info!("Keyboard backlight slept due to {}s idle", idle_sec);
                }
            }
        } else if backlight_idle_asleep {
            if let Some(ref lp) = lighting_proxy {
                let target_brightness = if settings.lighting_brightness > 0 { settings.lighting_brightness } else { 3 };
                let _ = lp.set_brightness(target_brightness).await;
                if settings.sync_accent_color {
                    if let Some((r, g, b)) = get_kde_accent_color() {
                        let _ = lp.set_color(r, g, b).await;
                    }
                } else {
                    let (r, g, b) = settings.lighting_color;
                    let _ = lp.set_color(r, g, b).await;
                }
                let _ = lp.set_mode(settings.lighting_mode.clone(), 1).await;
                backlight_idle_asleep = false;
                tracing::info!("Keyboard backlight restored to level {} on user activity", target_brightness);
            }
        }
    }
}

pub async fn apply_saved_settings_if_needed(
    system_conn: &zbus::Connection,
    _notifier: &Arc<DesktopNotifier>,
) {
    tracing::info!("Validating and applying saved hardware/desktop settings...");
    let settings = AlatusSettings::load();

    // 1. Battery Charge Limit
    if let Ok(bp) = alatus_ipc::BatteryProxy::new(system_conn).await {
        if let Ok(info) = bp.get_info().await {
            if info.charge_limit != Some(settings.battery_limit) {
                tracing::info!(
                    "Restoring battery charge limit from saved settings: {}%",
                    settings.battery_limit
                );
                let _ = bp.set_charge_limit(settings.battery_limit).await;
            }
        }
    }

    // 2. Thermal Profile
    if let Ok(tp) = alatus_ipc::ThermalProxy::new(system_conn).await {
        if let Ok(current) = tp.get_current_profile().await {
            if !current.eq_ignore_ascii_case(&settings.thermal_profile) {
                tracing::info!(
                    "Restoring thermal profile from saved settings: {}",
                    settings.thermal_profile
                );
                let _ = tp.set_profile(settings.thermal_profile.clone()).await;
            }
        }
    }

    // 3. Lighting State
    if let Ok(lp) = alatus_ipc::LightingProxy::new(system_conn).await {
        let _ = lp.set_brightness(settings.lighting_brightness).await;
        let _ = lp.set_mode(settings.lighting_mode.clone(), 1).await;
        if settings.sync_accent_color {
            if let Some((r, g, b)) = get_kde_accent_color() {
                let _ = lp.set_color(r, g, b).await;
                tracing::info!("Restored lighting to DE accent color: #{r:02X}{g:02X}{b:02X}");
            }
        } else {
            let (r, g, b) = settings.lighting_color;
            let _ = lp.set_color(r, g, b).await;
        }
    }

    // 4. Keyboard Idle Config (KDE powerdevil)
    let ac_secs = (settings.kbd_idle_timeout_ac_min * 60).to_string();
    let bat_secs = (settings.kbd_idle_timeout_bat_min * 60).to_string();
    let (ac_time, bat_time) = match settings.kbd_idle_mode.as_str() {
        "battery_ac" => (ac_secs.as_str(), ac_secs.as_str()),
        "battery_only" => ("0", bat_secs.as_str()),
        _ => ("0", "0"),
    };
    for (group, time) in [("AC", ac_time), ("Battery", bat_time)] {
        let _ = std::process::Command::new("kwriteconfig6")
            .args([
                "--file",
                "powerdevilrc",
                "--group",
                group,
                "--group",
                "DimKeyboard",
                "--key",
                "idleTime",
                time,
            ])
            .status();
    }
    if let Ok(conn) = zbus::Connection::session().await {
        if let Ok(proxy) = zbus::Proxy::new(
            &conn,
            "org.kde.Solid.PowerManagement",
            "/org/kde/Solid/PowerManagement",
            "org.kde.Solid.PowerManagement",
        ).await {
            let _: Result<(), zbus::Error> = proxy.call("reparseConfiguration", &()).await;
        }
    }

    // 5. Display Refresh Rate & Dimming
    if settings.auto_refresh {
        let ac = is_ac_connected();
        let cmd = if ac { get_on_ac_command() } else { get_on_bat_command() };
        execute_shell_command(&cmd).await;
    } else {
        let cmd = format!("kscreen-doctor output.eDP-1.mode.{}", settings.display_refresh_rate);
        execute_shell_command(&cmd).await;
    }

    if settings.oled_dimming_level > 0 && settings.oled_dimming_level <= 100 {
        let cmd = format!("kscreen-doctor output.eDP-1.dimming.{}", settings.oled_dimming_level);
        execute_shell_command(&cmd).await;
    }

    // 6. OLED Care Tweaks
    if settings.target_mode_active {
        let _ = tokio::process::Command::new("kwriteconfig6")
            .args(["--file", "kwinrc", "--group", "Plugins", "--key", "diminactiveEnabled", "--type", "bool", "true"])
            .status().await;
    }
    if settings.panel_autohide_active {
        let script = "panels().forEach(function(p){p.hiding='autohide';})";
        let _ = evaluate_plasmashell_script(script).await;
    }
    if settings.panel_transparency_active {
        let script = "panels().forEach(function(p){p.opacity='transparent';})";
        let _ = evaluate_plasmashell_script(script).await;
    }
    if settings.dpms_pixel_refresh_active {
        for group in ["AC", "Battery"] {
            let _ = tokio::process::Command::new("kwriteconfig6")
                .args(["--file", "powermanagementprofilesrc", "--group", group, "--group", "DPMSControl", "--key", "idleTime", "300"])
                .status().await;
        }
    }
}

async fn evaluate_plasmashell_script(script: &str) -> Result<(), String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.kde.plasmashell",
        "/PlasmaShell",
        "org.kde.PlasmaShell",
    )
    .await
    .map_err(|e| e.to_string())?;

    let _: Result<String, zbus::Error> = proxy.call("evaluateScript", &(script,)).await;
    Ok(())
}
