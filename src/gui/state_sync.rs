// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::hardware::CapabilityState;
use crate::services::daemon_client::get_daemon_client;
use slint::Color;

#[derive(Clone, Copy, Debug)]
pub struct M3Palette {
    pub accent_color: Color,
    pub accent_container: Color,
    pub on_accent_container: Color,
    pub on_accent: Color,
}

pub fn derive_m3_palette(rgb_opt: Option<(u8, u8, u8)>) -> M3Palette {
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

pub fn get_product_model() -> String {
    let raw = std::fs::read_to_string("/sys/class/dmi/id/product_name")
        .unwrap_or_else(|_| "ASUS Zenbook / Vivobook".to_string());
    sanitize_product_model(&raw)
}

pub fn get_bios_version() -> String {
    std::fs::read_to_string("/sys/class/dmi/id/bios_version")
        .unwrap_or_else(|_| "--".to_string())
        .trim()
        .to_string()
}

pub fn get_kernel_version() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "Linux".to_string())
        .trim()
        .to_string()
}

pub fn get_compositor_name() -> String {
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

pub fn get_battery_model() -> String {
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

pub fn get_bios_kernel_string() -> String {
    let bios = get_bios_version();
    let kernel = get_kernel_version();
    format!("{bios} | {kernel}")
}

pub struct DiagResult {
    pub status: &'static str,
    pub desc: String,
}

pub struct HardwareDiagnostics {
    pub wmi: DiagResult,
    pub rgb: DiagResult,
    pub oled: DiagResult,
    pub hotkey: DiagResult,
    pub charge: DiagResult,
}

pub async fn run_hardware_diagnostics() -> HardwareDiagnostics {
    let client_res = get_daemon_client().await;
    let caps_opt = if let Ok(ref client) = client_res {
        client.get_capabilities().await.ok()
    } else {
        None
    };

    // 1. ASUS WMI DebugFS / ACPI Platform Thermal
    let wmi = if let Some(ref caps) = caps_opt {
        match &caps.thermal {
            CapabilityState::Supported(_) => DiagResult {
                status: "Supported",
                desc: "ASUS ACPI / WMI platform profile active".to_string(),
            },
            CapabilityState::Unavailable(reason) => DiagResult {
                status: "Degraded",
                desc: format!("Thermal driver unavailable: {reason}"),
            },
            CapabilityState::Unsupported => DiagResult {
                status: "Missing",
                desc: "Thermal profile switching not supported on this profile".to_string(),
            },
        }
    } else if let Ok(ref client) = client_res {
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
    };

    // 2. Keyboard RGB Backlight
    let rgb = if let Some(ref caps) = caps_opt {
        match &caps.rgb {
            CapabilityState::Supported(details) => DiagResult {
                status: "Supported",
                desc: format!(
                    "Keyboard RGB controller active ({} zone{})",
                    details.supported_zones.len(),
                    if details.supported_zones.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            },
            CapabilityState::Unavailable(reason) => DiagResult {
                status: "Degraded",
                desc: format!("Keyboard RGB unavailable: {reason}"),
            },
            CapabilityState::Unsupported => DiagResult {
                status: "Missing",
                desc: "Keyboard RGB backlighting not equipped on this profile".to_string(),
            },
        }
    } else {
        let ite_driver = std::path::Path::new("/sys/bus/hid/drivers/ite5570");
        let has_lamparray = if ite_driver.exists() {
            true
        } else if let Ok(ref client) = client_res {
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
    let charge = if let Some(ref caps) = caps_opt {
        match &caps.battery {
            CapabilityState::Supported(details) => DiagResult {
                status: "Supported",
                desc: format!(
                    "charge_control_end_threshold active ({}-{}%)",
                    details.min_threshold, details.max_threshold
                ),
            },
            CapabilityState::Unavailable(reason) => DiagResult {
                status: "Degraded",
                desc: format!("Battery charge controller unavailable: {reason}"),
            },
            CapabilityState::Unsupported => DiagResult {
                status: "Missing",
                desc: "Battery charge threshold control not supported on this profile".to_string(),
            },
        }
    } else {
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

pub async fn copy_to_clipboard(text: &str) {
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

pub fn get_cpu_specs() -> String {
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

pub fn get_memory_specs() -> String {
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

pub fn get_motherboard_specs() -> String {
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

pub fn get_bios_detailed() -> String {
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

pub fn get_gpu_specs() -> String {
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
pub struct BatteryHealthDetails {
    pub status: String,
    pub pct: String,
    pub health: String,
    pub cycles: String,
    pub model: String,
    pub manufacturer: String,
}

pub fn get_battery_health_details() -> BatteryHealthDetails {
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
