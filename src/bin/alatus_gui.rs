// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

use alatus::services::config::{AlatusConfig, load_config, save_config_atomic};
use alatus::services::daemon_client::{DaemonClient, get_daemon_client};
use alatus::services::desktop_session::{
    apply_flicker_free_dimming, is_session_daemon_running, load_oled_metrics, query_display_info,
    query_session_status, read_portal_accent_color, save_oled_metrics, send_desktop_notification,
    set_panel_refresh_rate, set_session_accent_sync, set_session_auto_refresh,
    set_session_oled_care, set_session_oled_dim_level, trigger_session_pixel_refresh,
};
use alatus::services::firmware_mode::FirmwareMode;
use alatus::services::telemetry::read_thermal_telemetry;
use alatus::services::tray::{TrayEvent, start_tray_service};
use slint::{Color, ComponentHandle};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Clone, Copy, Debug)]
struct M3Palette {
    accent_color: Color,
    accent_container: Color,
    on_accent_container: Color,
    on_accent: Color,
}

fn derive_m3_palette(rgb_opt: Option<(u8, u8, u8)>) -> M3Palette {
    let (r, g, b) = rgb_opt.unwrap_or((208, 188, 255)); // Fallback: M3 Violet #d0bcff
    let accent_color = Color::from_rgb_u8(r, g, b);

    // Darkened container role (~35% luminance blended with dark surface)
    let cr = ((r as f32 * 0.35) as u8).max(35);
    let cg = ((g as f32 * 0.35) as u8).max(30);
    let cb = ((b as f32 * 0.35) as u8).max(45);
    let accent_container = Color::from_rgb_u8(cr, cg, cb);

    // Lightened container text role
    let ocr = (r as f32 * 0.65 + 255.0 * 0.35) as u8;
    let ocg = (g as f32 * 0.65 + 255.0 * 0.35) as u8;
    let ocb = (b as f32 * 0.65 + 255.0 * 0.35) as u8;
    let on_accent_container = Color::from_rgb_u8(ocr, ocg, ocb);

    // High-contrast text on solid accent
    let lum = 0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32);
    let on_accent = if lum > 140.0 {
        Color::from_rgb_u8(20, 18, 24)
    } else {
        Color::from_rgb_u8(255, 255, 255)
    };

    M3Palette {
        accent_color,
        accent_container,
        on_accent_container,
        on_accent,
    }
}

pub fn sanitize_product_model(raw: &str) -> String {
    let trimmed = raw.trim();
    // 1. If it contains an underscore, check if the suffix duplicates the preceding token
    if let Some((prefix, suffix)) = trimmed.rsplit_once('_') {
        let prefix_clean = prefix.trim();
        let suffix_clean = suffix.trim();
        if !suffix_clean.is_empty() && prefix_clean.ends_with(suffix_clean) {
            return prefix_clean.to_string();
        }
    }
    // 2. Also check if board_name sysfs exists and matches the suffix
    if let Ok(board) = std::fs::read_to_string("/sys/class/dmi/id/board_name") {
        let b = board.trim();
        if !b.is_empty() {
            let pattern = format!("_{b}");
            if let Some(stripped) = trimmed.strip_suffix(&pattern) {
                return stripped.trim().to_string();
            }
        }
    }
    trimmed.to_string()
}

fn get_product_model() -> String {
    let raw = std::fs::read_to_string("/sys/class/dmi/id/product_name")
        .unwrap_or_else(|_| "ASUS Zenbook / Vivobook".to_string());
    sanitize_product_model(&raw)
}

fn get_bios_version() -> String {
    std::fs::read_to_string("/sys/class/dmi/id/bios_version")
        .unwrap_or_else(|_| "--".to_string())
        .trim()
        .to_string()
}

fn get_kernel_version() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "Linux".to_string())
        .trim()
        .to_string()
}

fn get_compositor_name() -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if desktop.is_empty() {
        if session_type.is_empty() {
            "Unknown".to_string()
        } else {
            session_type
        }
    } else if session_type.is_empty() {
        desktop
    } else {
        format!("{desktop} ({session_type})")
    }
}

fn get_battery_model() -> String {
    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("BAT") {
                let model = std::fs::read_to_string(entry.path().join("model_name"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let mfr = std::fs::read_to_string(entry.path().join("manufacturer"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !model.is_empty() && !mfr.is_empty() {
                    return format!("{mfr} {model} ({name})");
                } else if !model.is_empty() {
                    return format!("{model} ({name})");
                } else {
                    return name;
                }
            }
        }
    }
    "Primary Battery".to_string()
}

fn get_bios_kernel_string() -> String {
    let bios = get_bios_version();
    let kernel = get_kernel_version();
    format!("{bios} | {kernel}")
}

struct DiagResult {
    status: &'static str,
    desc: String,
}

struct HardwareDiagnostics {
    wmi: DiagResult,
    rgb: DiagResult,
    oled: DiagResult,
    hotkey: DiagResult,
    charge: DiagResult,
}

async fn run_hardware_diagnostics() -> HardwareDiagnostics {
    // 1. ASUS WMI DebugFS (Queried via root daemon to avoid unprivileged user permission restrictions)
    let wmi = {
        if let Ok(client) = get_daemon_client().await {
            match client.get_hardware_diagnostics().await {
                Ok((true, desc)) => DiagResult {
                    status: "Supported",
                    desc,
                },
                Ok((false, desc)) => DiagResult {
                    status: "Degraded",
                    desc,
                },
                Err(e) => DiagResult {
                    status: "Degraded",
                    desc: format!("Daemon diagnostic query failed: {e}"),
                },
            }
        } else {
            let debugfs_path = std::path::Path::new("/sys/kernel/debug/asus-nb-wmi/dev_id");
            if debugfs_path.exists() {
                DiagResult {
                    status: "Supported",
                    desc: "ASUS WMI DebugFS active (/sys/kernel/debug/asus-nb-wmi)".to_string(),
                }
            } else if std::path::Path::new("/sys/devices/platform/asus-nb-wmi").exists() {
                DiagResult {
                    status: "Degraded",
                    desc: "Daemon offline; DebugFS requires root daemon permissions".to_string(),
                }
            } else {
                DiagResult {
                    status: "Missing",
                    desc: "No ASUS WMI platform or DebugFS endpoints detected".to_string(),
                }
            }
        }
    };

    // 2. ITE5570 LampArray
    let rgb = {
        let ite_driver = std::path::Path::new("/sys/bus/hid/drivers/ite5570");
        let has_lamparray = if ite_driver.exists() {
            true
        } else if let Ok(client) = get_daemon_client().await {
            client
                .get_rgb_status()
                .await
                .map(|s| s.available)
                .unwrap_or(false)
        } else {
            false
        };

        if has_lamparray {
            DiagResult {
                status: "Supported",
                desc: "ITE5570 LampArray Keyboard Controller detected".to_string(),
            }
        } else if std::path::Path::new("/sys/class/leds/asus::kbd_backlight").exists() {
            DiagResult {
                status: "Degraded",
                desc: "Monochrome keyboard backlight only (LampArray HID missing)".to_string(),
            }
        } else {
            DiagResult {
                status: "Missing",
                desc: "No LampArray HID controller or keyboard backlight found".to_string(),
            }
        }
    };

    // 3. OLED Flicker-Free Dimming
    let oled = {
        let kscreen = std::process::Command::new("kscreen-doctor")
            .arg("-v")
            .output();
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase();
        if kscreen.is_ok() && desktop.contains("KDE") {
            DiagResult {
                status: "Supported",
                desc: "KDE Plasma kscreen-doctor output dimming active".to_string(),
            }
        } else if std::env::var("WAYLAND_DISPLAY").is_ok() {
            DiagResult {
                status: "Degraded",
                desc: "Wayland session active; compositor software luminance fallback".to_string(),
            }
        } else {
            DiagResult {
                status: "Missing",
                desc: "No supported compositor flicker-free dimming interface".to_string(),
            }
        }
    };

    // 4. ACPI Hotkeys (Fn+F)
    let hotkey = {
        let asus_nb = std::path::Path::new("/sys/devices/platform/asus-nb-wmi");
        if asus_nb.exists() {
            DiagResult {
                status: "Supported",
                desc: "asus-nb-wmi evdev input active (Fn+F / code 482)".to_string(),
            }
        } else {
            DiagResult {
                status: "Degraded",
                desc: "asus-nb-wmi driver missing; falling back to generic keyboard evdev"
                    .to_string(),
            }
        }
    };

    // 5. Battery Charge Limit
    let charge = {
        let mut supported = false;
        let mut path_str = String::new();
        if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("BAT") {
                    let thresh = entry.path().join("charge_control_end_threshold");
                    if thresh.exists() {
                        supported = true;
                        path_str = thresh.display().to_string();
                        break;
                    }
                }
            }
        }
        if supported {
            DiagResult {
                status: "Supported",
                desc: format!("charge_control_end_threshold supported ({path_str})"),
            }
        } else {
            DiagResult {
                status: "Missing",
                desc: "charge_control_end_threshold not found in /sys/class/power_supply"
                    .to_string(),
            }
        }
    };

    HardwareDiagnostics {
        wmi,
        rgb,
        oled,
        hotkey,
        charge,
    }
}

async fn copy_to_clipboard(text: &str) {
    if tokio::process::Command::new("wl-copy")
        .arg(text)
        .output()
        .await
        .is_ok()
    {
        return;
    }
    if let Ok(mut child) = tokio::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = child.stdin.take() {
            let text = text.to_string();
            tokio::spawn(async move {
                let _ = stdin.write_all(text.as_bytes()).await;
            });
        }
    }
}

fn get_cpu_specs() -> String {
    let mut model_name = String::new();
    let mut thread_count = 0;
    let mut core_count = 0;

    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("model name")
                && model_name.is_empty()
                && let Some(pos) = trimmed.find(':')
            {
                model_name = trimmed[pos + 1..].trim().to_string();
            } else if trimmed.starts_with("processor") {
                thread_count += 1;
            } else if trimmed.starts_with("cpu cores")
                && core_count == 0
                && let Some(pos) = trimmed.find(':')
            {
                core_count = trimmed[pos + 1..].trim().parse::<u32>().unwrap_or(0);
            }
        }
    }

    let max_freq_str =
        std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
            .or_else(|_| {
                std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq")
            })
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|khz| format!(" @ {:.2} GHz", khz / 1_000_000.0))
            .unwrap_or_default();

    if model_name.is_empty() {
        return "Unknown Processor".to_string();
    }

    let cleaned_model = model_name
        .replace("(R)", "")
        .replace("(TM)", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let cores_text = if core_count > 0 && thread_count > 0 {
        format!(" ({core_count} Cores, {thread_count} Threads{max_freq_str})")
    } else if thread_count > 0 {
        format!(" ({thread_count} Threads{max_freq_str})")
    } else {
        max_freq_str
    };

    format!("{cleaned_model}{cores_text}")
}

fn get_memory_specs() -> String {
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;

    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("MemTotal:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    total_kb = parts[1].parse().unwrap_or(0);
                }
            } else if trimmed.starts_with("MemAvailable:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    avail_kb = parts[1].parse().unwrap_or(0);
                }
            }
        }
    }

    if total_kb > 0 {
        let total_gib = total_kb as f64 / 1024.0 / 1024.0;
        let avail_gib = avail_kb as f64 / 1024.0 / 1024.0;
        format!("{total_gib:.1} GiB ({avail_gib:.1} GiB available)")
    } else {
        "--".to_string()
    }
}

fn get_motherboard_specs() -> String {
    let vendor = std::fs::read_to_string("/sys/class/dmi/id/board_vendor")
        .unwrap_or_default()
        .trim()
        .to_string();
    let name = std::fs::read_to_string("/sys/class/dmi/id/board_name")
        .unwrap_or_default()
        .trim()
        .to_string();

    if vendor.is_empty() && name.is_empty() {
        "--".to_string()
    } else if vendor.is_empty() {
        name
    } else if name.is_empty() {
        vendor
    } else {
        format!("{vendor} {name}")
    }
}

fn get_bios_detailed() -> String {
    let version = std::fs::read_to_string("/sys/class/dmi/id/bios_version")
        .unwrap_or_default()
        .trim()
        .to_string();
    let date = std::fs::read_to_string("/sys/class/dmi/id/bios_date")
        .unwrap_or_default()
        .trim()
        .to_string();

    if version.is_empty() && date.is_empty() {
        "--".to_string()
    } else if date.is_empty() {
        version
    } else {
        format!("{version} ({date})")
    }
}

pub fn sanitize_gpu_specs(raw: &str, driver: &str) -> String {
    let mut s = raw.trim();

    // 1. Strip PCI slot & controller type prefix before colon (e.g. "00:02.0 VGA compatible controller [0300]: ")
    if let Some(colon_idx) = s.rfind(": ") {
        s = &s[colon_idx + 2..];
    } else if let Some(colon_idx) = s.find(':') {
        let prefix = &s[..colon_idx];
        if !prefix.contains('[') {
            s = s[colon_idx + 1..].trim();
        }
    }

    // 2. Detect Vendor
    let lower_all = s.to_lowercase();
    let vendor = if lower_all.contains("intel") {
        Some("Intel")
    } else if lower_all.contains("nvidia") || lower_all.contains("geforce") {
        Some("NVIDIA")
    } else if lower_all.contains("advanced micro devices")
        || lower_all.contains("amd/ati")
        || lower_all.contains("radeon")
        || lower_all
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w == "amd" || w == "ati")
    {
        Some("AMD")
    } else {
        None
    };

    // 3. Extract bracketed tokens
    let mut marketing_name: Option<String> = None;
    let mut non_bracket_segments = Vec::new();
    let mut rest = s;

    while let Some(open) = rest.find('[') {
        non_bracket_segments.push(&rest[..open]);
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find(']') {
            let inside = after_open[..close].trim();
            let is_pci_id = inside.contains(':') && inside.len() <= 10;
            let is_class_code = inside.len() == 4 && inside.chars().all(|c| c.is_ascii_hexdigit());
            let is_vendor_tag = inside.eq_ignore_ascii_case("AMD/ATI")
                || inside.eq_ignore_ascii_case("AMD")
                || inside.eq_ignore_ascii_case("INTEL")
                || inside.eq_ignore_ascii_case("NVIDIA");

            if !is_pci_id && !is_class_code && !is_vendor_tag && marketing_name.is_none() {
                marketing_name = Some(inside.to_string());
            }
            rest = &after_open[close + 1..];
        } else {
            non_bracket_segments.push(after_open);
            rest = "";
            break;
        }
    }
    non_bracket_segments.push(rest);

    let remaining_text = non_bracket_segments.join(" ");

    // 4. Clean remaining text to extract architecture/codename or fallback name
    let mut codename_tokens = Vec::new();
    let mut skipping_rev = false;

    for token in remaining_text.split_whitespace() {
        if token.starts_with("(rev") || token.starts_with("rev") || token == "(rev" {
            skipping_rev = true;
            if token.ends_with(')') {
                skipping_rev = false;
            }
            continue;
        }
        if skipping_rev {
            if token.ends_with(')') {
                skipping_rev = false;
            }
            continue;
        }

        let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        let lower = cleaned.to_lowercase();
        if lower.is_empty()
            || lower == "corporation"
            || lower == "inc"
            || lower == "compatible"
            || lower == "controller"
            || lower == "advanced"
            || lower == "micro"
            || lower == "devices"
            || lower == "vga"
            || lower == "3d"
            || lower == "display"
        {
            continue;
        }

        // Only filter vendor name if we already identified a bracketed marketing name
        if marketing_name.is_some() && (lower == "amd" || lower == "intel" || lower == "nvidia") {
            continue;
        }

        codename_tokens.push(cleaned);
    }

    let codename = codename_tokens.join(" ");

    let primary_str = if let Some(mkt) = marketing_name {
        let mut full_mkt = mkt;
        let mkt_lower = full_mkt.to_lowercase();
        let already_has_vendor = mkt_lower.starts_with("intel")
            || mkt_lower.starts_with("amd")
            || mkt_lower.starts_with("nvidia");

        if !already_has_vendor && let Some(v) = vendor {
            full_mkt = format!("{v} {full_mkt}");
        }
        if !codename.is_empty() {
            format!("{full_mkt} ({codename})")
        } else {
            full_mkt
        }
    } else if !codename.is_empty() {
        if let Some(v) = vendor {
            let v_lower = v.to_lowercase();
            let code_lower = codename.to_lowercase();
            if !code_lower.starts_with(&v_lower) {
                format!("{v} {codename}")
            } else {
                codename
            }
        } else {
            codename
        }
    } else {
        "Integrated Graphics".to_string()
    };

    let d = driver.trim();
    if !d.is_empty() {
        format!("{primary_str} [driver: {d}]")
    } else {
        primary_str
    }
}

fn get_gpu_specs() -> String {
    if let Ok(output) = std::process::Command::new("lspci").args(["-nnk"]).output()
        && let Ok(text) = String::from_utf8(output.stdout)
    {
        let mut gpu_line = String::new();
        let mut driver_line = String::new();
        let mut in_vga_block = false;

        for line in text.lines() {
            let trimmed = line.trim();
            let lower = trimmed.to_lowercase();
            if lower.contains("vga compatible")
                || lower.contains("3d controller")
                || lower.contains("display controller")
            {
                in_vga_block = true;
                if gpu_line.is_empty() {
                    gpu_line = trimmed.to_string();
                }
            } else if in_vga_block && trimmed.starts_with("Kernel driver in use:") {
                driver_line = trimmed
                    .replace("Kernel driver in use:", "")
                    .trim()
                    .to_string();
                break;
            } else if !line.starts_with('\t') && !line.starts_with(' ') && in_vga_block {
                in_vga_block = false;
            }
        }

        if !gpu_line.is_empty() {
            return sanitize_gpu_specs(&gpu_line, &driver_line);
        }
    }

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("card")
                && !name.contains('-')
                && let Ok(target) = std::fs::read_link(entry.path().join("device/driver"))
            {
                let driver_name = target.file_name().unwrap_or_default().to_string_lossy();
                return format!("DRM Graphics Device [driver: {driver_name}]");
            }
        }
    }

    "Integrated Graphics".to_string()
}

#[derive(Default)]
struct BatteryHealthDetails {
    status: String,
    pct: String,
    health: String,
    cycles: String,
    model: String,
    manufacturer: String,
}

fn get_battery_health_details() -> BatteryHealthDetails {
    let mut details = BatteryHealthDetails {
        status: "Unknown".to_string(),
        pct: "--%".to_string(),
        health: "--%".to_string(),
        cycles: "0 Cycles".to_string(),
        model: "--".to_string(),
        manufacturer: "--".to_string(),
    };

    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("BAT")
                && let Ok(uevent) = std::fs::read_to_string(path.join("uevent"))
            {
                let mut charge_full = 0f64;
                let mut charge_full_design = 0f64;

                for line in uevent.lines() {
                    let trimmed = line.trim();
                    if let Some((k, v)) = trimmed.split_once('=') {
                        match k {
                            "POWER_SUPPLY_STATUS" => details.status = v.to_string(),
                            "POWER_SUPPLY_CAPACITY" => details.pct = format!("{v}%"),
                            "POWER_SUPPLY_CYCLE_COUNT" => details.cycles = format!("{v} Cycles"),
                            "POWER_SUPPLY_MODEL_NAME" => details.model = v.to_string(),
                            "POWER_SUPPLY_MANUFACTURER" => details.manufacturer = v.to_string(),
                            "POWER_SUPPLY_CHARGE_FULL" | "POWER_SUPPLY_ENERGY_FULL" => {
                                charge_full = v.parse::<f64>().unwrap_or(0.0);
                            }
                            "POWER_SUPPLY_CHARGE_FULL_DESIGN"
                            | "POWER_SUPPLY_ENERGY_FULL_DESIGN" => {
                                charge_full_design = v.parse::<f64>().unwrap_or(0.0);
                            }
                            _ => {}
                        }
                    }
                }

                if charge_full > 0.0 && charge_full_design > 0.0 {
                    let health_pct = (charge_full / charge_full_design * 100.0).clamp(0.0, 100.0);
                    let condition = if health_pct >= 80.0 {
                        " (Good)"
                    } else if health_pct >= 50.0 {
                        " (Fair)"
                    } else {
                        " (Degraded)"
                    };
                    details.health = format!("{health_pct:.1}%{condition}");
                }
                break;
            }
        }
    }

    details
}

fn get_autostart_desktop_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("autostart").join("io.strixwolf.alatus.desktop")
}

fn is_autostart_enabled() -> bool {
    let path = get_autostart_desktop_path();
    if path.exists() {
        return true;
    }
    // Check legacy ascend desktop file
    let legacy = path.with_file_name("io.strixwolf.ascend.desktop");
    legacy.exists()
}

fn set_autostart_enabled(enabled: bool) {
    let path = get_autostart_desktop_path();
    let legacy = path.with_file_name("io.strixwolf.ascend.desktop");
    if enabled {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = r#"[Desktop Entry]
Type=Application
Name=Alatus
GenericName=Hardware Control Center
Comment=ASUS Vivobook and Zenbook Hardware Management
Exec=alatus-gui --minimized
Icon=io.strixwolf.alatus
Terminal=false
Categories=HardwareSettings;Settings;Utility;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#;
        if let Err(e) = std::fs::write(&path, content) {
            error!("Failed to write autostart desktop file: {e}");
        } else {
            info!("Autostart desktop entry created at {:?}", path);
            let _ = std::fs::remove_file(&legacy);
        }
    } else {
        if let Err(e) = std::fs::remove_file(&path) {
            error!("Failed to remove autostart desktop file: {e}");
        } else {
            info!("Autostart desktop entry removed from {:?}", path);
        }
        let _ = std::fs::remove_file(&legacy);
    }
}

enum GuiAction {
    SetThermalMode(i32),
    ToggleOledCare(bool),
    TriggerPixelRefresh,
    SetOledDimLevel(i32),
    SetRefreshRate(i32),
    SetRgbBrightness(i32),
    SetRgbPreset(i32),
    ApplyCustomRgb(u8, u8, u8),
    SetChargeLimit(u32),
    RevealSerial,
    CopySerial,
    RunDiagnostics,
    ToggleTray(bool),
    ToggleAutostart(bool),
}

fn setup_window(
    w: &AppWindow,
    tx: &mpsc::UnboundedSender<GuiAction>,
    tray_ref: Arc<AtomicBool>,
    cfg: &AlatusConfig,
    initial_palette: &M3Palette,
) {
    w.set_tray_enabled(cfg.tray_enabled);
    w.set_autostart_enabled(is_autostart_enabled());

    w.set_thermal_mode(cfg.thermal_mode as i32);
    w.set_charge_limit(cfg.charge_limit as i32);
    w.set_rgb_brightness(cfg.rgb_brightness as i32);
    w.set_selected_rgb_preset(cfg.rgb_preset);
    w.set_custom_red(cfg.custom_rgb.0 as i32);
    w.set_custom_green(cfg.custom_rgb.1 as i32);
    w.set_custom_blue(cfg.custom_rgb.2 as i32);
    w.set_oled_care_enabled(cfg.oled_care_enabled);
    w.set_oled_dim_level(cfg.oled_dim_level as i32);
    w.set_selected_refresh_rate(cfg.refresh_rate as i32);

    // Initial Hardware & Desktop theming setup
    w.set_hw_model(get_product_model().into());
    w.set_hw_bios(get_bios_version().into());
    w.set_hw_kernel(get_kernel_version().into());
    w.set_hw_compositor(get_compositor_name().into());
    w.set_hw_battery_id(get_battery_model().into());
    w.set_hw_bios_kernel(get_bios_kernel_string().into());

    // Exhaustive System Specifications
    w.set_hw_cpu(get_cpu_specs().into());
    w.set_hw_ram(get_memory_specs().into());
    w.set_hw_motherboard(get_motherboard_specs().into());
    w.set_hw_bios_detailed(get_bios_detailed().into());
    w.set_hw_gpu(get_gpu_specs().into());

    // Dedicated Battery Health & Telemetry
    let bat = get_battery_health_details();
    w.set_hw_battery_status(bat.status.into());
    w.set_hw_battery_pct(bat.pct.into());
    w.set_hw_battery_health(bat.health.into());
    w.set_hw_battery_cycles(bat.cycles.into());
    w.set_hw_battery_manufacturer(bat.manufacturer.into());

    w.set_accent_color(initial_palette.accent_color);
    w.set_accent_container(initial_palette.accent_container);
    w.set_on_accent_container(initial_palette.on_accent_container);
    w.set_on_accent(initial_palette.on_accent);

    // Wire Slint callbacks to dispatch commands to Tokio
    {
        let tx = tx.clone();
        w.on_set_thermal_mode(move |mode| {
            let _ = tx.send(GuiAction::SetThermalMode(mode));
        });
    }
    {
        let tx = tx.clone();
        w.on_toggle_oled_care(move |enable| {
            let _ = tx.send(GuiAction::ToggleOledCare(enable));
        });
    }
    {
        let tx = tx.clone();
        w.on_trigger_pixel_refresh(move || {
            let _ = tx.send(GuiAction::TriggerPixelRefresh);
        });
    }
    {
        let tx = tx.clone();
        w.on_set_oled_dim_level(move |val| {
            let _ = tx.send(GuiAction::SetOledDimLevel(val));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_refresh_rate(move |rate| {
            let _ = tx.send(GuiAction::SetRefreshRate(rate));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_rgb_brightness(move |brightness| {
            let _ = tx.send(GuiAction::SetRgbBrightness(brightness));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_rgb_preset(move |preset| {
            let _ = tx.send(GuiAction::SetRgbPreset(preset));
        });
    }
    {
        let tx = tx.clone();
        w.on_apply_custom_rgb(move |r, g, b| {
            let _ = tx.send(GuiAction::ApplyCustomRgb(
                r.clamp(0, 255) as u8,
                g.clamp(0, 255) as u8,
                b.clamp(0, 255) as u8,
            ));
        });
    }
    {
        let tx = tx.clone();
        w.on_set_charge_limit(move |limit| {
            let _ = tx.send(GuiAction::SetChargeLimit(limit.clamp(50, 100) as u32));
        });
    }
    {
        let tx = tx.clone();
        w.on_reveal_serial(move || {
            let _ = tx.send(GuiAction::RevealSerial);
        });
    }
    {
        let tx = tx.clone();
        w.on_copy_serial(move || {
            let _ = tx.send(GuiAction::CopySerial);
        });
    }
    {
        let tx = tx.clone();
        w.on_run_diagnostics(move || {
            let _ = tx.send(GuiAction::RunDiagnostics);
        });
    }
    {
        let tx = tx.clone();
        w.on_toggle_tray(move |val| {
            let _ = tx.send(GuiAction::ToggleTray(val));
        });
    }
    {
        let tx = tx.clone();
        w.on_toggle_autostart(move |val| {
            let _ = tx.send(GuiAction::ToggleAutostart(val));
        });
    }

    // Intercept window close request: hide to tray if enabled, or terminate app cleanly
    {
        let weak_close = w.as_weak();
        w.window().on_close_requested(move || {
            if tray_ref.load(Ordering::Relaxed) {
                if let Some(win) = weak_close.upgrade() {
                    let _ = win.hide();
                }
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            }
        });
    }
}

thread_local! {
    static APP_WINDOW: std::cell::RefCell<Option<AppWindow>> = const { std::cell::RefCell::new(None) };
}

fn with_app_window<F: FnOnce(&AppWindow)>(f: F) {
    APP_WINDOW.with(|cell| {
        if let Some(ref w) = *cell.borrow() {
            f(w);
        }
    });
}

struct GuiIpcServer {
    tray_tx: mpsc::UnboundedSender<TrayEvent>,
}

#[zbus::interface(name = "io.strixwolf.alatus.Gui")]
impl GuiIpcServer {
    async fn show_window(&self) -> zbus::fdo::Result<()> {
        let _ = self.tray_tx.send(TrayEvent::ShowWindow);
        Ok(())
    }

    async fn toggle_window(&self) -> zbus::fdo::Result<()> {
        let _ = self.tray_tx.send(TrayEvent::ToggleWindow);
        Ok(())
    }

    async fn quit(&self) -> zbus::fdo::Result<()> {
        let _ = self.tray_tx.send(TrayEvent::Quit);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting Alatus Hardware Control Center GUI...");

    // Associate Wayland window with desktop entry and scalable SVG icon
    let _ = slint::set_xdg_app_id("io.strixwolf.alatus");

    // D-Bus Single-Instance Guard: Prevent multiple instances from spawning duplicate tray icons
    let (tray_tx, mut tray_rx) = mpsc::unbounded_channel::<TrayEvent>();
    let session_conn = match zbus::Connection::session().await {
        Ok(conn) => {
            let already_running = if let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await
                && let Ok(name) = zbus::names::WellKnownName::try_from("io.strixwolf.alatus.Gui")
            {
                dbus.name_has_owner(name.into()).await.unwrap_or(false)
            } else {
                false
            };

            if already_running {
                info!("Alatus GUI is already running. Raising active window.");
                if let Ok(proxy) = zbus::Proxy::new(
                    &conn,
                    "io.strixwolf.alatus.Gui",
                    "/io/strixwolf/alatus/Gui",
                    "io.strixwolf.alatus.Gui",
                )
                .await
                {
                    let _ = proxy.call::<_, _, ()>("ShowWindow", &()).await;
                }
                std::process::exit(0);
            }

            // Register IPC server before requesting well-known name
            let ipc_server = GuiIpcServer {
                tray_tx: tray_tx.clone(),
            };
            if let Err(e) = conn
                .object_server()
                .at("/io/strixwolf/alatus/Gui", ipc_server)
                .await
            {
                warn!("Failed to register GUI IPC server on D-Bus: {e}");
            } else {
                info!("GUI IPC server registered at /io/strixwolf/alatus/Gui");
            }

            match conn
                .request_name_with_flags(
                    "io.strixwolf.alatus.Gui",
                    zbus::fdo::RequestNameFlags::DoNotQueue.into(),
                )
                .await
            {
                Ok(zbus::fdo::RequestNameReply::PrimaryOwner)
                | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => {
                    info!("Acquired primary D-Bus name io.strixwolf.alatus.Gui");
                    Some(conn)
                }
                Ok(zbus::fdo::RequestNameReply::Exists)
                | Ok(zbus::fdo::RequestNameReply::InQueue) => {
                    info!("Alatus GUI is already running. Raising active window.");
                    if let Ok(proxy) = zbus::Proxy::new(
                        &conn,
                        "io.strixwolf.alatus.Gui",
                        "/io/strixwolf/alatus/Gui",
                        "io.strixwolf.alatus.Gui",
                    )
                    .await
                    {
                        let _ = proxy.call::<_, _, ()>("ShowWindow", &()).await;
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    warn!("Failed to request D-Bus name io.strixwolf.alatus.Gui: {e}");
                    Some(conn)
                }
            }
        }
        Err(e) => {
            warn!("Failed to connect to D-Bus session bus: {e}");
            None
        }
    };

    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.iter().any(|a| a == "--minimized" || a == "-m");

    // Unified configuration persistence
    let config = load_config();
    let config_arc = Arc::new(std::sync::Mutex::new(config.clone()));
    let tray_enabled_atomic = Arc::new(AtomicBool::new(config.tray_enabled));
    let (tx, mut rx) = mpsc::unbounded_channel::<GuiAction>();
    let last_user_action = Arc::new(AtomicU64::new(0));

    // Restore persisted hardware configuration on startup
    {
        let startup_cfg = config.clone();
        tokio::spawn(async move {
            if let Ok(client) = get_daemon_client().await {
                let _ = client.set_charge_limit(startup_cfg.charge_limit).await;
                let mode = match startup_cfg.thermal_mode {
                    0 => FirmwareMode::Quiet,
                    1 => FirmwareMode::Balanced,
                    2 => FirmwareMode::High,
                    3 => FirmwareMode::Full,
                    _ => FirmwareMode::Balanced,
                };
                let _ = client.set_firmware_mode(mode).await;
                let _ = client.set_rgb_brightness(startup_cfg.rgb_brightness).await;
                if startup_cfg.rgb_preset == 0 {
                    let _ = set_session_accent_sync(true).await;
                } else if startup_cfg.rgb_preset == -1 {
                    let _ = set_session_accent_sync(false).await;
                    let (r, g, b) = startup_cfg.custom_rgb;
                    let _ = client.set_rgb_color(r, g, b).await;
                }
            }
            let _ = set_session_oled_care(startup_cfg.oled_care_enabled).await;
            let _ = set_session_oled_dim_level(startup_cfg.oled_dim_level).await;
        });
    }

    let initial_rgb = if let Some(ref conn) = session_conn {
        read_portal_accent_color(conn).await.unwrap_or(None)
    } else {
        None
    };
    let initial_palette = derive_m3_palette(initial_rgb);

    let revealed_serial: Arc<std::sync::Mutex<String>> =
        Arc::new(std::sync::Mutex::new(String::new()));

    // Universal FreeDesktop StatusNotifierItem & DBusMenu system tray service
    if let Some(ref conn) = session_conn {
        if let Err(e) = start_tray_service(conn, tray_tx.clone()).await {
            warn!("Failed to initialize system tray service: {e}");
        } else {
            info!("System tray service registered successfully on session bus.");
        }
    } else {
        warn!("Session D-Bus connection unavailable; system tray registration skipped.");
    }

    let tray_action_tx = tx.clone();
    let tray_enabled_tray = Arc::clone(&tray_enabled_atomic);
    let config_tray = Arc::clone(&config_arc);
    let initial_palette_tray = initial_palette;
    let tx_tray = tx.clone();
    tokio::spawn(async move {
        while let Some(evt) = tray_rx.recv().await {
            match evt {
                TrayEvent::ToggleWindow => {
                    let tx = tx_tray.clone();
                    let tray_ref = Arc::clone(&tray_enabled_tray);
                    let cfg_arc = Arc::clone(&config_tray);
                    let palette = initial_palette_tray;
                    let _ = slint::invoke_from_event_loop(move || {
                        APP_WINDOW.with(|cell| {
                            let mut b = cell.borrow_mut();
                            if b.is_none() {
                                match AppWindow::new() {
                                    Ok(w) => {
                                        let cfg =
                                            cfg_arc.lock().map(|c| c.clone()).unwrap_or_default();
                                        setup_window(&w, &tx, tray_ref, &cfg, &palette);
                                        let _ = w.show();
                                        *b = Some(w);
                                    }
                                    Err(e) => error!("Failed to create AppWindow: {e}"),
                                }
                            } else if let Some(ref w) = *b {
                                if w.window().is_visible() {
                                    let _ = w.hide();
                                } else {
                                    w.window().set_minimized(false);
                                    let _ = w.show();
                                }
                            }
                        });
                    });
                }
                TrayEvent::ShowWindow => {
                    let tx = tx_tray.clone();
                    let tray_ref = Arc::clone(&tray_enabled_tray);
                    let cfg_arc = Arc::clone(&config_tray);
                    let palette = initial_palette_tray;
                    let _ = slint::invoke_from_event_loop(move || {
                        APP_WINDOW.with(|cell| {
                            let mut b = cell.borrow_mut();
                            if b.is_none() {
                                match AppWindow::new() {
                                    Ok(w) => {
                                        let cfg =
                                            cfg_arc.lock().map(|c| c.clone()).unwrap_or_default();
                                        setup_window(&w, &tx, tray_ref, &cfg, &palette);
                                        let _ = w.show();
                                        *b = Some(w);
                                    }
                                    Err(e) => error!("Failed to create AppWindow: {e}"),
                                }
                            } else if let Some(ref w) = *b {
                                w.window().set_minimized(false);
                                let _ = w.show();
                            }
                        });
                    });
                }
                TrayEvent::SetThermalMode(mode) => {
                    let _ = tray_action_tx.send(GuiAction::SetThermalMode(mode as i32));
                }
                TrayEvent::TriggerPixelRefresh => {
                    let _ = tray_action_tx.send(GuiAction::TriggerPixelRefresh);
                }
                TrayEvent::Quit => {
                    let _ = slint::quit_event_loop();
                }
            }
        }
    });

    if !start_minimized {
        let w = AppWindow::new()?;
        let cfg = config_arc.lock().map(|c| c.clone()).unwrap_or_default();
        setup_window(
            &w,
            &tx,
            Arc::clone(&tray_enabled_atomic),
            &cfg,
            &initial_palette,
        );
        w.show()?;
        APP_WINDOW.with(|cell| {
            *cell.borrow_mut() = Some(w);
        });
    } else {
        info!("Alatus GUI running minimized in system tray (no window mapped).");
    }

    // Trigger initial hardware compatibility diagnostics
    let _ = tx.send(GuiAction::RunDiagnostics);

    let last_user_action_processor = last_user_action.clone();
    let tray_enabled_action = tray_enabled_atomic.clone();
    let config_action = Arc::clone(&config_arc);
    let revealed_serial_action = Arc::clone(&revealed_serial);

    // Background task to process GUI action events asynchronously
    tokio::spawn(async move {
        while let Some(action) = rx.recv().await {
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            last_user_action_processor.store(now_secs, Ordering::Relaxed);

            match action {
                GuiAction::SetThermalMode(mode_idx) => {
                    let mode = match mode_idx {
                        0 => FirmwareMode::Quiet,
                        1 => FirmwareMode::Balanced,
                        2 => FirmwareMode::High,
                        3 => FirmwareMode::Full,
                        _ => FirmwareMode::Balanced,
                    };
                    info!("GUI Action: Set thermal mode to {mode:?}");
                    let _ = slint::invoke_from_event_loop(move || {
                        with_app_window(|w| {
                            w.set_thermal_mode(mode_idx);
                        });
                    });
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.thermal_mode = mode_idx as u32;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_firmware_mode(mode).await {
                                error!("Failed to set firmware mode on daemon: {e}");
                            } else {
                                info!("Firmware mode updated to {mode:?} successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for thermal mode: {e}"),
                    }
                }
                GuiAction::ToggleOledCare(enable) => {
                    info!("GUI Action: Toggle OLED care to {enable}");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.oled_care_enabled = enable;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    if let Err(e) = set_session_oled_care(enable).await {
                        warn!("Session daemon OLED care toggle failed: {e}");
                    } else {
                        info!("OLED Care set to {enable} successfully.");
                    }
                }
                GuiAction::TriggerPixelRefresh => {
                    info!("GUI Action: Trigger pixel refresh conditioning cycle");
                    let refresh_res = trigger_session_pixel_refresh()
                        .await
                        .map_err(|e| e.to_string());
                    let success = match refresh_res {
                        Ok(res) => {
                            info!("Session daemon pixel refresh responded: {res}");
                            res
                        }
                        Err(e) => {
                            warn!(
                                "Session daemon D-Bus call failed: {e}. Executing local fallback conditioning cycle."
                            );
                            let mut metrics = load_oled_metrics();
                            metrics.refresh_count += 1;
                            metrics.active_screen_seconds = 0;
                            metrics.last_refresh_timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            save_oled_metrics(&metrics);
                            if let Ok(conn) = zbus::Connection::session().await {
                                let _ = send_desktop_notification(
                                    &conn,
                                    "ASUS OLED Care: Pixel Refresh Started",
                                    "Conditioning OLED panel to relieve subpixel stress...",
                                )
                                .await;
                            }
                            tokio::spawn(async move {
                                apply_flicker_free_dimming(None, 50).await;
                                tokio::time::sleep(Duration::from_millis(1500)).await;
                                let metrics = load_oled_metrics();
                                apply_flicker_free_dimming(None, metrics.oled_dim_level).await;
                                if let Ok(conn) = zbus::Connection::session().await {
                                    let _ = send_desktop_notification(
                                        &conn,
                                        "ASUS OLED Care: Pixel Refresh Completed",
                                        "Panel conditioning cycle finished successfully.",
                                    )
                                    .await;
                                }
                            });
                            true
                        }
                    };
                    let msg = if success { "Cycle Active" } else { "Failed" };
                    let _ = slint::invoke_from_event_loop(move || {
                        with_app_window(|w| {
                            w.set_refresh_status_text(msg.into());
                        });
                    });
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(4)).await;
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_refresh_status_text("".into());
                            });
                        });
                    });
                }
                GuiAction::SetOledDimLevel(level) => {
                    let target = (level as u32).clamp(10, 100);
                    info!("GUI Action: Set Flicker-Free OLED dim level to {target}%");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.oled_dim_level = target;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    let mut metrics = load_oled_metrics();
                    metrics.oled_dim_level = target;
                    save_oled_metrics(&metrics);
                    let set_res = set_session_oled_dim_level(target)
                        .await
                        .map_err(|e| e.to_string());
                    if let Err(e) = set_res {
                        warn!("Session daemon set OLED dim level failed: {e}");
                        apply_flicker_free_dimming(None, target).await;
                    } else {
                        info!("Flicker-Free OLED dim level set to {target}% successfully.");
                    }
                }
                GuiAction::SetRefreshRate(hz) => {
                    info!("GUI Action: Set display refresh rate to {hz} Hz");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.refresh_rate = hz as u32;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    if hz == 0 {
                        if let Err(e) = set_session_auto_refresh(true).await {
                            warn!("Session auto refresh toggle failed: {e}");
                        }
                        if let Ok(client) = get_daemon_client().await
                            && let Ok(on_ac) = client.get_on_ac().await
                        {
                            let target_hz = if on_ac { 120.0 } else { 60.0 };
                            if let Err(e) = set_panel_refresh_rate(target_hz).await {
                                error!("Failed to set panel refresh rate: {e}");
                            } else {
                                info!("Auto refresh switched panel to {target_hz:.1} Hz.");
                            }
                        }
                    } else {
                        if let Err(e) = set_session_auto_refresh(false).await {
                            warn!("Session disable auto refresh failed: {e}");
                        }
                        if let Err(e) = set_panel_refresh_rate(hz as f64).await {
                            error!("Failed to set panel refresh rate to {hz} Hz: {e}");
                        } else {
                            info!("Panel refresh rate set to {hz} Hz successfully.");
                        }
                    }
                }
                GuiAction::SetRgbBrightness(percent) => {
                    let val = percent.clamp(0, 100) as u32;
                    info!("GUI Action: Set RGB brightness to {val}%");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.rgb_brightness = val;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_rgb_brightness(val).await {
                                error!("Failed to set RGB brightness on daemon: {e}");
                            } else {
                                info!("RGB brightness set to {val}% successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for RGB brightness: {e}"),
                    }
                }
                GuiAction::SetRgbPreset(preset) => {
                    info!("GUI Action: Set RGB preset to {preset}");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.rgb_preset = preset;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match preset {
                        0 => {
                            if let Err(e) = set_session_accent_sync(true).await {
                                warn!("Failed to enable session accent sync: {e}");
                            } else {
                                info!("Desktop accent synchronization enabled.");
                            }
                            if let Ok(conn) = zbus::Connection::session().await
                                && let Ok(Some((r, g, b))) = read_portal_accent_color(&conn).await
                            {
                                match get_daemon_client().await {
                                    Ok(client) => {
                                        if let Err(e) = client.set_rgb_color(r, g, b).await {
                                            error!("Failed to set Auto-Sync RGB color: {e}");
                                        } else {
                                            info!(
                                                "Auto-Sync RGB color (#{r:02X}{g:02X}{b:02X}) applied successfully."
                                            );
                                        }
                                    }
                                    Err(e) => error!("Daemon unavailable for Auto-Sync RGB: {e}"),
                                }
                            }
                        }
                        1 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(61, 174, 233).await {
                                        error!("Failed to set RGB preset Cyan: {e}");
                                    } else {
                                        info!("Preset Cyan applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset Cyan: {e}"),
                            }
                        }
                        2 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(186, 104, 200).await {
                                        error!("Failed to set RGB preset Purple: {e}");
                                    } else {
                                        info!("Preset Purple applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset Purple: {e}"),
                            }
                        }
                        3 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(255, 255, 255).await {
                                        error!("Failed to set RGB preset White: {e}");
                                    } else {
                                        info!("Preset White applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset White: {e}"),
                            }
                        }
                        4 => {
                            let _ = set_session_accent_sync(false).await;
                            match get_daemon_client().await {
                                Ok(client) => {
                                    if let Err(e) = client.set_rgb_color(255, 140, 0).await {
                                        error!("Failed to set RGB preset Amber: {e}");
                                    } else {
                                        info!("Preset Amber applied successfully.");
                                    }
                                }
                                Err(e) => error!("Daemon unavailable for RGB preset Amber: {e}"),
                            }
                        }
                        _ => {}
                    }
                }
                GuiAction::ApplyCustomRgb(r, g, b) => {
                    info!("GUI Action: Apply custom RGB color ({r}, {g}, {b})");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.custom_rgb = (r, g, b);
                        cfg_guard.rgb_preset = -1;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    let _ = set_session_accent_sync(false).await;
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_rgb_color(r, g, b).await {
                                error!("Failed to set custom RGB color: {e}");
                            } else {
                                info!("Custom RGB color ({r}, {g}, {b}) applied successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for custom RGB color: {e}"),
                    }
                }
                GuiAction::SetChargeLimit(limit) => {
                    let limit = limit.clamp(50, 100);
                    info!("GUI Action: Set charge limit to {limit}%");
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.charge_limit = limit;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                    match get_daemon_client().await {
                        Ok(client) => {
                            if let Err(e) = client.set_charge_limit(limit).await {
                                error!("Failed to set charge limit on daemon: {e}");
                            } else {
                                info!("Charge limit set to {limit}% successfully.");
                            }
                        }
                        Err(e) => error!("Daemon unavailable for charge limit: {e}"),
                    }
                }
                GuiAction::RevealSerial => {
                    let serial_holder = Arc::clone(&revealed_serial_action);
                    tokio::spawn(async move {
                        let output = tokio::process::Command::new("pkexec")
                            .args(["cat", "/sys/class/dmi/id/product_serial"])
                            .output()
                            .await;
                        let serial = match output {
                            Ok(out) if out.status.success() => {
                                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                                if !s.is_empty() {
                                    s
                                } else {
                                    "Unavailable".to_string()
                                }
                            }
                            _ => {
                                if let Ok(client) = DaemonClient::connect().await
                                    && let Ok(s) = client.get_product_serial().await
                                    && !s.is_empty()
                                    && s != "Unavailable"
                                {
                                    s
                                } else {
                                    "Auth Failed".to_string()
                                }
                            }
                        };
                        if let Ok(mut guard) = serial_holder.lock() {
                            *guard = serial.clone();
                        }
                        let serial_disp = serial.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_hw_serial(serial_disp.into());
                                w.set_hw_serial_revealed(true);
                            });
                        });
                    });
                }
                GuiAction::CopySerial => {
                    let serial_holder = Arc::clone(&revealed_serial_action);
                    tokio::spawn(async move {
                        let serial_to_copy =
                            serial_holder.lock().map(|g| g.clone()).unwrap_or_default();

                        if !serial_to_copy.is_empty()
                            && serial_to_copy != "••••••••••••"
                            && serial_to_copy != "Auth Failed"
                            && serial_to_copy != "Unavailable"
                        {
                            copy_to_clipboard(&serial_to_copy).await;
                            let _ = slint::invoke_from_event_loop(move || {
                                with_app_window(|w| {
                                    w.set_hw_copy_status("Copied to Clipboard".into());
                                });
                            });
                            tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(2500)).await;
                                let _ = slint::invoke_from_event_loop(move || {
                                    with_app_window(|w| {
                                        w.set_hw_copy_status("".into());
                                    });
                                });
                            });
                        }
                    });
                }
                GuiAction::RunDiagnostics => {
                    tokio::spawn(async move {
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_is_diagnosing(true);
                            });
                        });

                        let diags = run_hardware_diagnostics().await;

                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_window(|w| {
                                w.set_diag_wmi_status(diags.wmi.status.into());
                                w.set_diag_wmi_desc(diags.wmi.desc.into());

                                w.set_diag_rgb_status(diags.rgb.status.into());
                                w.set_diag_rgb_desc(diags.rgb.desc.into());

                                w.set_diag_oled_status(diags.oled.status.into());
                                w.set_diag_oled_desc(diags.oled.desc.into());

                                w.set_diag_hotkey_status(diags.hotkey.status.into());
                                w.set_diag_hotkey_desc(diags.hotkey.desc.into());

                                w.set_diag_charge_status(diags.charge.status.into());
                                w.set_diag_charge_desc(diags.charge.desc.into());

                                w.set_is_diagnosing(false);
                            });
                        });
                    });
                }
                GuiAction::ToggleTray(enabled) => {
                    info!("GUI Action: Toggle system tray: {enabled}");
                    tray_enabled_action.store(enabled, Ordering::Relaxed);
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.tray_enabled = enabled;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                }
                GuiAction::ToggleAutostart(enabled) => {
                    info!("GUI Action: Toggle launch on startup: {enabled}");
                    set_autostart_enabled(enabled);
                    if let Ok(mut cfg_guard) = config_action.lock() {
                        cfg_guard.autostart_enabled = enabled;
                        let _ = save_config_atomic(&cfg_guard);
                    }
                }
            }
        }
    });

    // Periodic telemetry and theme polling loop (1.5 seconds interval)
    let last_user_action_telemetry = last_user_action.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1500));
        let mut daemon_client_opt = get_daemon_client().await.ok();
        let session_conn_opt = zbus::Connection::session().await.ok();

        loop {
            interval.tick().await;

            // Reconnect daemon client if necessary
            if daemon_client_opt.is_none() {
                daemon_client_opt = get_daemon_client().await.ok();
            }

            // Read sensors
            let thermal = read_thermal_telemetry();
            let cpu_temp_str = format!("{}°C", thermal.temp_c);
            let fan_rpm_str = if thermal.fan_rpm > 0 {
                format!("{} RPM", thermal.fan_rpm)
            } else {
                "0 RPM".to_string()
            };

            let mut root_daemon_active = false;
            let mut power_source_str = "AC".to_string();
            let mut thermal_mode_idx_opt = None;
            let mut rgb_brightness_opt = None;
            let mut charge_limit_opt = None;

            if let Some(ref client) = daemon_client_opt {
                root_daemon_active = true;
                if let Ok(on_ac) = client.get_on_ac().await {
                    power_source_str = if on_ac {
                        "AC".to_string()
                    } else {
                        "Battery".to_string()
                    };
                }
                if let Ok(mode) = client.get_firmware_mode().await {
                    thermal_mode_idx_opt = Some(match mode {
                        FirmwareMode::Quiet => 0,
                        FirmwareMode::Balanced => 1,
                        FirmwareMode::High => 2,
                        FirmwareMode::Full => 3,
                        FirmwareMode::Unknown(_) => 1,
                    });
                }
                if let Ok(rgb) = client.get_rgb_status().await {
                    rgb_brightness_opt = Some(rgb.brightness as i32);
                }
                if let Ok(limit) = client.get_charge_limit().await {
                    charge_limit_opt = Some(limit.clamp(50, 100) as i32);
                }
            } else {
                daemon_client_opt = None;
            }

            // Dynamic Desktop Accent Color polling
            let mut dynamic_palette_opt = None;
            if let Some(ref conn) = session_conn_opt
                && let Ok(Some(rgb)) = read_portal_accent_color(conn).await
            {
                dynamic_palette_opt = Some(derive_m3_palette(Some(rgb)));
            }

            let session_status = query_session_status().await;
            let display_info = query_display_info().await;
            let session_daemon_active = is_session_daemon_running().await || session_status.running;

            let refresh_rate_selection = if session_status.auto_refresh {
                0
            } else if display_info.refresh_rate >= 100.0 {
                120
            } else {
                60
            };

            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let user_recently_active =
                now_secs.saturating_sub(last_user_action_telemetry.load(Ordering::Relaxed)) < 3;

            let bat = get_battery_health_details();
            let _ = slint::invoke_from_event_loop(move || {
                with_app_window(|w| {
                    if !w.window().is_visible() {
                        return;
                    }

                    if let Some(palette) = dynamic_palette_opt {
                        w.set_accent_color(palette.accent_color);
                        w.set_accent_container(palette.accent_container);
                        w.set_on_accent_container(palette.on_accent_container);
                        w.set_on_accent(palette.on_accent);
                    }
                    w.set_cpu_temp(cpu_temp_str.into());
                    w.set_fan_rpm(fan_rpm_str.into());
                    w.set_power_source(power_source_str.into());
                    w.set_active_connector(display_info.connector.into());
                    w.set_refresh_rate(format!("{:.1} Hz", display_info.refresh_rate).into());
                    w.set_oled_screen_hours(
                        format!("{:.1}", session_status.pixel_refresh_hours).into(),
                    );
                    w.set_oled_refresh_count(session_status.pixel_refresh_count as i32);
                    w.set_root_daemon_status(if root_daemon_active {
                        "Active".into()
                    } else {
                        "Inactive".into()
                    });
                    w.set_session_daemon_status(if session_daemon_active {
                        "Active".into()
                    } else {
                        "Inactive".into()
                    });

                    // Keep live battery status & health current
                    w.set_hw_battery_status(bat.status.into());
                    w.set_hw_battery_pct(bat.pct.into());
                    w.set_hw_battery_health(bat.health.into());
                    w.set_hw_battery_cycles(bat.cycles.into());

                    // Only synchronize interactive settings if user is not actively adjusting them
                    if !user_recently_active {
                        if let Some(mode_idx) = thermal_mode_idx_opt {
                            w.set_thermal_mode(mode_idx);
                        }
                        if let Some(rgb_brightness) = rgb_brightness_opt {
                            w.set_rgb_brightness(rgb_brightness);
                        }
                        if let Some(limit) = charge_limit_opt {
                            w.set_charge_limit(limit);
                        }
                        if session_daemon_active {
                            w.set_selected_refresh_rate(refresh_rate_selection);
                            w.set_oled_care_enabled(session_status.oled_care);
                            w.set_oled_dim_level(session_status.oled_dim_level as i32);
                            if session_status.sync_accent {
                                w.set_selected_rgb_preset(0);
                            }
                        }
                    }
                });
            });
        }
    });

    slint::run_event_loop_until_quit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_window_initialization() {
        match AppWindow::new() {
            Ok(w) => {
                assert_eq!(w.get_current_tab(), 0);
                assert_eq!(w.get_thermal_mode(), 1);
                assert!(w.get_oled_care_enabled());
                assert_eq!(w.get_oled_dim_level(), 100);
                assert_eq!(w.get_charge_limit(), 80);
                assert_eq!(w.get_rgb_brightness(), 80);
                assert_eq!(w.get_selected_refresh_rate(), 120);
                assert_eq!(w.get_selected_rgb_preset(), 0);
                assert_eq!(w.get_hw_serial(), "••••••••••••");
                assert!(!w.get_hw_serial_revealed());
                assert!(w.get_tray_enabled());
                assert!(!w.get_autostart_enabled());
            }
            Err(e) => {
                // In headless sandbox/CI without display socket, verify error is platform/display related
                let err_str = e.to_string();
                assert!(
                    err_str.contains("display")
                        || err_str.contains("Display")
                        || err_str.contains("platform")
                        || err_str.contains("Platform")
                        || std::env::var("DISPLAY").is_err()
                        || std::env::var("WAYLAND_DISPLAY").is_err(),
                    "Unexpected window initialization error: {err_str}"
                );
            }
        }
    }

    #[test]
    fn test_derive_m3_palette_fallback() {
        let palette = derive_m3_palette(None);
        // Fallback is M3 Violet (208, 188, 255)
        assert_eq!(palette.accent_color.red(), 208);
        assert_eq!(palette.accent_color.green(), 188);
        assert_eq!(palette.accent_color.blue(), 255);
    }

    #[test]
    fn test_derive_m3_palette_custom() {
        let palette = derive_m3_palette(Some((61, 174, 233)));
        assert_eq!(palette.accent_color.red(), 61);
        assert_eq!(palette.accent_color.green(), 174);
        assert_eq!(palette.accent_color.blue(), 233);
    }

    #[test]
    fn test_sanitize_product_model() {
        assert_eq!(
            sanitize_product_model("ASUS Vivobook S 15 S5506MA_S5506MA"),
            "ASUS Vivobook S 15 S5506MA"
        );
        assert_eq!(
            sanitize_product_model("Zenbook 14 OLED UX3405MA_UX3405MA"),
            "Zenbook 14 OLED UX3405MA"
        );
        assert_eq!(
            sanitize_product_model("ROG Zephyrus G14"),
            "ROG Zephyrus G14"
        );
        assert_eq!(
            sanitize_product_model("  ASUS Vivobook S 14 OLED S5406SA_S5406SA  "),
            "ASUS Vivobook S 14 OLED S5406SA"
        );
    }

    #[test]
    fn test_sanitize_gpu_specs() {
        // Intel Arc Graphics (Meteor Lake-P)
        let intel_arc = "00:02.0 VGA compatible controller [0300]: Intel Corporation Meteor Lake-P [Intel Arc Graphics] [8086:7d55] (rev 08)";
        assert_eq!(
            sanitize_gpu_specs(intel_arc, "i915"),
            "Intel Arc Graphics (Meteor Lake-P) [driver: i915]"
        );

        // AMD Radeon 780M (Phoenix1)
        let amd_phoenix = "03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Phoenix1 [Radeon 780M] [1002:15bf] (rev c8)";
        assert_eq!(
            sanitize_gpu_specs(amd_phoenix, "amdgpu"),
            "AMD Radeon 780M (Phoenix1) [driver: amdgpu]"
        );

        // NVIDIA GeForce RTX 4070
        let nvidia_rtx = "01:00.0 3D controller [0302]: NVIDIA Corporation AD106M [GeForce RTX 4070 Max-Q / Mobile] [10de:2860] (rev a1)";
        assert_eq!(
            sanitize_gpu_specs(nvidia_rtx, "nvidia"),
            "NVIDIA GeForce RTX 4070 Max-Q / Mobile (AD106M) [driver: nvidia]"
        );

        // Intel Alder Lake-P Iris Xe
        let intel_iris = "00:02.0 VGA compatible controller [0300]: Intel Corporation Alder Lake-P [Iris Xe Graphics] [8086:46a6] (rev 0c)";
        assert_eq!(
            sanitize_gpu_specs(intel_iris, "i915"),
            "Intel Iris Xe Graphics (Alder Lake-P) [driver: i915]"
        );

        // AMD Rembrandt Radeon 680M
        let amd_rembrandt =
            "Advanced Micro Devices, Inc. [AMD/ATI] Rembrandt [Radeon 680M] [1002:1681]";
        assert_eq!(
            sanitize_gpu_specs(amd_rembrandt, "amdgpu"),
            "AMD Radeon 680M (Rembrandt) [driver: amdgpu]"
        );

        // Plain Intel UHD without architecture bracket
        let intel_uhd = "Intel Corporation UHD Graphics 620 [8086:5917] (rev 07)";
        assert_eq!(
            sanitize_gpu_specs(intel_uhd, "i915"),
            "Intel UHD Graphics 620 [driver: i915]"
        );

        // Without driver
        assert_eq!(
            sanitize_gpu_specs(intel_arc, ""),
            "Intel Arc Graphics (Meteor Lake-P)"
        );

        // Fallback
        assert_eq!(sanitize_gpu_specs("", ""), "Integrated Graphics");
    }
}
