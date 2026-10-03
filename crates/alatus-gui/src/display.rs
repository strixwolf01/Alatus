//! KDE Plasma / Wayland display management (refresh rate, OLED DC dimming, target mode, OLED care).
//! References Ayuz implementation via kscreen-doctor and KDE Plasma D-Bus/kconfig.

use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct ModeSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OutputMode {
    pub id: String,
    pub name: String,
    #[serde(rename = "refreshRate")]
    pub refresh_rate: f64,
    pub size: ModeSize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KScreenOutput {
    pub name: String,
    pub connected: bool,
    pub enabled: bool,
    #[serde(rename = "currentModeId")]
    pub current_mode_id: Option<String>,
    #[serde(default)]
    pub modes: Vec<OutputMode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KScreenConfig {
    #[serde(default)]
    pub outputs: Vec<KScreenOutput>,
}

#[derive(Debug, Clone)]
pub struct DisplayState {
    pub is_kde: bool,
    pub accent_color: Option<(u8, u8, u8)>,
    pub output_name: String,
    pub current_refresh_rate: u32,
    pub supported_refresh_rates: Vec<u32>,
    pub current_dimming: u32,
    pub target_mode_active: bool,
    pub panel_autohide: bool,
    pub panel_transparency: bool,
    pub dpms_pixel_refresh: bool,
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            is_kde: true,
            accent_color: None,
            output_name: "eDP-1".to_string(),
            current_refresh_rate: 120,
            supported_refresh_rates: vec![60, 120],
            current_dimming: 100,
            target_mode_active: false,
            panel_autohide: false,
            panel_transparency: false,
            dpms_pixel_refresh: false,
        }
    }
}

pub async fn query_display_state() -> DisplayState {
    let mut state = DisplayState::default();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    state.is_kde = desktop.contains("kde");
    state.accent_color = get_kde_accent_color();

    if !state.is_kde {
        return state;
    }

    // 1. Query kscreen-doctor -j
    if let Ok(output) = tokio::process::Command::new("kscreen-doctor")
        .arg("-j")
        .output()
        .await
    {
        if let Ok(config) = serde_json::from_slice::<KScreenConfig>(&output.stdout) {
            if let Some(out) = config.outputs.into_iter().find(|o| {
                o.connected
                    && o.enabled
                    && (o.name.contains("eDP") || o.name.contains("DP") || o.name.contains("HDMI"))
            }) {
                state.output_name = out.name.clone();

                let mut current_size = None;
                if let Some(ref cur_id) = out.current_mode_id {
                    if let Some(cur_m) = out.modes.iter().find(|m| m.id == *cur_id) {
                        state.current_refresh_rate = cur_m.refresh_rate.round() as u32;
                        current_size = Some((cur_m.size.width, cur_m.size.height));
                    }
                }

                // Collect available refresh rates for current resolution
                if let Some((w, h)) = current_size {
                    let mut rates: Vec<u32> = out
                        .modes
                        .iter()
                        .filter(|m| m.size.width == w && m.size.height == h)
                        .map(|m| m.refresh_rate.round() as u32)
                        .collect();
                    rates.sort_unstable();
                    rates.dedup();
                    if !rates.is_empty() {
                        state.supported_refresh_rates = rates;
                    }
                }
            }
        }
    }

    // 2. Query target mode (diminactive)
    state.target_mode_active = read_kwin_bool("Plugins", "diminactiveEnabled").unwrap_or(false);

    // 3. Query DPMS idle time
    if let Some(idle) = read_power_idle_time() {
        state.dpms_pixel_refresh = idle <= 300;
    }

    state
}

pub async fn set_refresh_rate(output_name: &str, target_hz: u32) -> Result<(), String> {
    // Find matching mode ID via kscreen-doctor -j
    let output = tokio::process::Command::new("kscreen-doctor")
        .arg("-j")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let config: KScreenConfig =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;

    let out = config
        .outputs
        .into_iter()
        .find(|o| o.name == output_name || o.name == "eDP-1")
        .ok_or_else(|| "Display output not found".to_string())?;

    let cur_id = out.current_mode_id.unwrap_or_default();
    let cur_size = out
        .modes
        .iter()
        .find(|m| m.id == cur_id)
        .map(|m| (m.size.width, m.size.height));

    let target_mode = out.modes.iter().find(|m| {
        let size_matches = match cur_size {
            Some((w, h)) => m.size.width == w && m.size.height == h,
            None => true,
        };
        size_matches && (m.refresh_rate.round() as u32) == target_hz
    });

    let mode_arg = if let Some(m) = target_mode {
        format!("output.{}.mode.{}", out.name, m.id)
    } else {
        format!("output.{}.mode.{}", out.name, target_hz)
    };

    let status = tokio::process::Command::new("kscreen-doctor")
        .arg(&mode_arg)
        .status()
        .await
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "kscreen-doctor returned status {:?}",
            status.code()
        ))
    }
}

pub async fn set_oled_dimming(output_name: &str, percentage: u32) -> Result<(), String> {
    let pct = percentage.clamp(10, 100);
    let arg = format!("output.{}.dimming.{}", output_name, pct);
    let status = tokio::process::Command::new("kscreen-doctor")
        .arg(&arg)
        .status()
        .await
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "kscreen-doctor returned status {:?}",
            status.code()
        ))
    }
}

pub async fn set_target_mode(enabled: bool) -> Result<(), String> {
    let val_str = if enabled { "true" } else { "false" };
    let _ = tokio::process::Command::new("kwriteconfig6")
        .args([
            "--file",
            "kwinrc",
            "--group",
            "Plugins",
            "--key",
            "diminactiveEnabled",
            "--type",
            "bool",
            val_str,
        ])
        .status()
        .await;

    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(&conn, "org.kde.KWin", "/Effects", "org.kde.kwin.Effects")
        .await
        .map_err(|e| e.to_string())?;

    let method = if enabled {
        "loadEffect"
    } else {
        "unloadEffect"
    };
    let _: Result<(), zbus::Error> = proxy.call(method, &("diminactive",)).await;
    Ok(())
}

pub async fn set_panel_autohide(enabled: bool) -> Result<(), String> {
    let hiding = if enabled { "autohide" } else { "none" };
    let script = format!("panels().forEach(function(p){{p.hiding='{}';}})", hiding);
    evaluate_plasmashell_script(&script).await
}

pub async fn set_panel_transparency(enabled: bool) -> Result<(), String> {
    let opacity = if enabled { "transparent" } else { "opaque" };
    let script = format!("panels().forEach(function(p){{p.opacity='{}';}})", opacity);
    evaluate_plasmashell_script(&script).await
}

pub async fn set_pixel_refresh_dpms(enabled: bool) -> Result<(), String> {
    let idle_time = if enabled { "300" } else { "600" };
    for group in ["AC", "Battery"] {
        let _ = tokio::process::Command::new("kwriteconfig6")
            .args([
                "--file",
                "powermanagementprofilesrc",
                "--group",
                group,
                "--group",
                "DPMSControl",
                "--key",
                "idleTime",
                idle_time,
            ])
            .status()
            .await;
    }
    Ok(())
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

fn read_kwin_bool(group: &str, key: &str) -> Option<bool> {
    let output = Command::new("kreadconfig6")
        .args([
            "--file",
            "kwinrc",
            "--group",
            group,
            "--key",
            key,
            "--default",
            "false",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase();
    Some(s == "true")
}

fn read_power_idle_time() -> Option<u32> {
    let output = Command::new("kreadconfig6")
        .args([
            "--file",
            "powermanagementprofilesrc",
            "--group",
            "AC",
            "--group",
            "DPMSControl",
            "--key",
            "idleTime",
            "--default",
            "600",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    s.parse::<u32>().ok()
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

#[derive(Debug, Clone)]
pub struct KeyboardIdleConfig {
    pub mode: String, // "always_on", "battery_ac", "battery_only"
    pub timeout_ac_min: u32,
    pub timeout_bat_min: u32,
}

pub fn read_keyboard_idle_config() -> KeyboardIdleConfig {
    let settings = alatus_core::settings::AlatusSettings::load();
    let idle_ac = read_powerdevil_u32("AC", "DimKeyboard", "idleTime").unwrap_or(0);
    let idle_bat = read_powerdevil_u32("Battery", "DimKeyboard", "idleTime").unwrap_or(0);

    let mode = if !settings.kbd_idle_mode.is_empty() {
        settings.kbd_idle_mode.clone()
    } else if idle_ac > 0 && idle_bat > 0 {
        "battery_ac".to_string()
    } else if idle_bat > 0 {
        "battery_only".to_string()
    } else {
        "always_on".to_string()
    };

    let ac_min = if settings.kbd_idle_timeout_ac_min > 0 {
        settings.kbd_idle_timeout_ac_min
    } else if idle_ac > 0 {
        (idle_ac / 60).max(1)
    } else {
        1
    };

    let bat_min = if settings.kbd_idle_timeout_bat_min > 0 {
        settings.kbd_idle_timeout_bat_min
    } else if idle_bat > 0 {
        (idle_bat / 60).max(1)
    } else {
        1
    };

    KeyboardIdleConfig {
        mode,
        timeout_ac_min: ac_min,
        timeout_bat_min: bat_min,
    }
}

pub async fn set_keyboard_idle_config(mode: &str, timeout_ac_min: u32, timeout_bat_min: u32) -> Result<(), String> {
    let ac_secs = (timeout_ac_min * 60).to_string();
    let bat_secs = (timeout_bat_min * 60).to_string();
    let (ac_time, bat_time) = match mode {
        "battery_ac" => (ac_secs.as_str(), ac_secs.as_str()),
        "battery_only" => ("0", bat_secs.as_str()),
        _ => ("0", "0"),
    };

    for (group, time) in [("AC", ac_time), ("Battery", bat_time)] {
        let _ = Command::new("kwriteconfig6")
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

    // Trigger reparseConfiguration on KDE Solid PowerManagement if running
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

    Ok(())
}

fn read_powerdevil_u32(profile: &str, group: &str, key: &str) -> Option<u32> {
    let output = Command::new("kreadconfig6")
        .args([
            "--file",
            "powerdevilrc",
            "--group",
            profile,
            "--group",
            group,
            "--key",
            key,
            "--default",
            "0",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    s.parse::<u32>().ok()
}

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

