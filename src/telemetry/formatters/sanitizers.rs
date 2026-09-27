// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Hardware name, DMI, and GPU string sanitization utilities.

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
        .unwrap_or_else(|_| "Hardware Device".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

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
