// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::thermal::format_mode;
use crate::services::daemon_client::{DaemonClient, DaemonClientError};
use crate::services::firmware_mode::FirmwareMode;
use crate::services::telemetry::{
    RgbPayload, StatusPayload, format_waybar_payload, read_battery_telemetry,
    read_thermal_telemetry,
};

pub async fn build_status_payload(
    client: &DaemonClient,
) -> Result<StatusPayload, DaemonClientError> {
    let mode = client.get_firmware_mode().await?;
    let charge_limit = client.get_charge_limit().await?;
    let on_ac = client.get_on_ac().await.unwrap_or(true);
    let rgb_status = client.get_rgb_status().await.ok();

    let mode_str = format_mode(mode).to_string();
    let thermal = read_thermal_telemetry();
    let power_source = if on_ac { "AC" } else { "Battery" }.to_string();

    let (hex, brightness) = match &rgb_status {
        Some(rgb) if rgb.available => (
            format!("#{:02X}{:02X}{:02X}", rgb.red, rgb.green, rgb.blue),
            rgb.brightness,
        ),
        _ => ("#000000".to_string(), 0),
    };

    let waybar = format_waybar_payload(
        &mode_str,
        on_ac,
        charge_limit,
        thermal.temp_c,
        thermal.fan_rpm,
    );

    Ok(StatusPayload {
        text: waybar.text.clone(),
        alt: waybar.alt.clone(),
        tooltip: waybar.tooltip.clone(),
        class: waybar.class.clone(),
        percentage: waybar.percentage,
        firmware_mode: mode_str,
        charge_limit,
        power_source,
        on_ac,
        rgb: RgbPayload { hex, brightness },
        thermal,
        waybar,
    })
}

pub async fn emit_status_line(client: &DaemonClient) {
    use std::io::Write;
    if let Ok(payload) = build_status_payload(client).await {
        let json_res = serde_json::to_string(&payload);
        if let Ok(json) = json_res {
            println!("{}", json);
            let _ = std::io::stdout().flush();
        }
    }
}

pub async fn handle_status_watch(client: &DaemonClient) -> Result<(), DaemonClientError> {
    use futures_util::StreamExt;
    use tokio::time::{Duration, interval};
    use zbus::{MatchRule, MessageStream};

    // Emit immediately on startup
    emit_status_line(client).await;

    let conn = client.connection();

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path_namespace("/io/strixwolf/alatus")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?
        .build();

    let mut stream = MessageStream::for_match_rule(rule, conn, Some(16))
        .await
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    let mut ticker = interval(Duration::from_secs(3));
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            Some(_) = stream.next() => {
                emit_status_line(client).await;
            }
            _ = ticker.tick() => {
                emit_status_line(client).await;
            }
        }
    }

    Ok(())
}

pub async fn handle_status(
    client: &DaemonClient,
    json_mode: bool,
    watch: bool,
) -> Result<(), DaemonClientError> {
    if watch {
        return handle_status_watch(client).await;
    }

    let mode = client.get_firmware_mode().await?;
    let charge_limit = client.get_charge_limit().await?;
    let deep_sleep = client.get_deep_sleep_active().await?;
    let on_ac = client.get_on_ac().await.unwrap_or(true);
    let auto_th = client.get_auto_thermal_profile().await.unwrap_or(false);
    let rgb_status = client.get_rgb_status().await.ok();

    if json_mode {
        let payload = build_status_payload(client).await?;
        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
        println!("{}", json);
        return Ok(());
    }

    println!("Alatus System Status");
    println!("────────────────────");
    println!("Firmware Mode:  {}", format_mode(mode));
    println!("Charge Limit:   {}%", charge_limit);
    println!(
        "Deep Sleep:     {}",
        if deep_sleep { "Active" } else { "Inactive" }
    );
    println!(
        "Power Source:   {}",
        if on_ac { "AC Connected" } else { "Battery" }
    );
    println!(
        "Auto Thermal:   {}",
        if auto_th { "Enabled" } else { "Disabled" }
    );

    match rgb_status {
        Some(rgb) if rgb.available => {
            println!("RGB Device:     {} ({})", rgb.model, rgb.path);
            println!(
                "RGB Color:      #{:02X}{:02X}{:02X} @ {}%",
                rgb.red, rgb.green, rgb.blue, rgb.brightness
            );
        }
        _ => {
            println!("RGB Device:     None detected");
        }
    }

    Ok(())
}

pub async fn handle_monitor(
    client: &DaemonClient,
    interval_ms: u64,
) -> Result<(), DaemonClientError> {
    let interval = std::time::Duration::from_millis(interval_ms.max(100));

    // Hide cursor during monitoring
    print!("\x1B[?25l");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let res = run_monitor_loop(client, interval).await;

    // Restore cursor and finish with a newline
    println!("\x1B[?25h");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    res
}

pub async fn run_monitor_loop(
    client: &DaemonClient,
    interval: std::time::Duration,
) -> Result<(), DaemonClientError> {
    loop {
        let mode = client
            .get_firmware_mode()
            .await
            .unwrap_or(FirmwareMode::Unknown(0));
        let charge_limit = client.get_charge_limit().await.unwrap_or(0);
        let deep_sleep = client.get_deep_sleep_active().await.unwrap_or(false);
        let on_ac = client.get_on_ac().await.unwrap_or(true);
        let auto_th = client.get_auto_thermal_profile().await.unwrap_or(false);
        let rgb_status = client.get_rgb_status().await.ok();

        let daemon_active = client.check_active().await.unwrap_or(false);

        let thermal = read_thermal_telemetry();
        let battery = read_battery_telemetry();

        // ANSI clear screen (\x1B[2J) and cursor home (\x1B[H)
        print!("\x1B[2J\x1B[H");

        println!("┌────────────────────────────────────────────────────────┐");
        println!("│               ALATUS HARDWARE MONITOR                  │");
        if !daemon_active {
            println!("│ [!] WARNING: alatusd service is NOT active             │");
        }
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Power & Profile:                                       │");
        println!("│   Firmware Mode:    {:<35}│", format_mode(mode));
        println!(
            "│   Auto-Thermal:     {:<35}│",
            if auto_th { "Enabled" } else { "Disabled" }
        );
        println!(
            "│   Power Source:     {:<35}│",
            if on_ac { "AC Connected" } else { "Battery" }
        );
        println!(
            "│   Deep Sleep:       {:<35}│",
            if deep_sleep { "Active" } else { "Inactive" }
        );
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Battery Telemetry:                                     │");
        println!(
            "│   Charge Limit:     {:<35}│",
            format!("{}%", charge_limit)
        );
        if let Some(bat) = battery {
            let bat_stat = format!("{} ({}%)", bat.status, bat.capacity);
            let rate_label = if bat.status.eq_ignore_ascii_case("charging") {
                "Charge Rate"
            } else {
                "Discharge Rate"
            };
            let bat_rate = format!(
                "{:.2} W ({:.2} V @ {:.2} A)",
                bat.rate_watts, bat.voltage_v, bat.current_a
            );
            println!("│   Battery Status:   {:<35}│", bat_stat);
            println!("│   {:<18}{:<35}│", format!("{}:", rate_label), bat_rate);
        } else {
            println!("│   Battery:          {:<35}│", "Not detected");
        }
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Thermal & Cooling:                                     │");
        println!(
            "│   CPU Temperature:  {:<35}│",
            format!("{}°C", thermal.temp_c)
        );
        println!(
            "│   Fan 1 RPM:        {:<35}│",
            format!("{} RPM", thermal.fan_rpm)
        );
        if let Some(f2) = thermal.fan2_rpm {
            println!("│   Fan 2 RPM:        {:<35}│", format!("{} RPM", f2));
        }
        println!("├────────────────────────────────────────────────────────┤");
        println!("│ Keyboard Lighting:                                     │");
        match rgb_status {
            Some(rgb) if rgb.available => {
                let rgb_str = format!(
                    "#{:02X}{:02X}{:02X} @ {}%",
                    rgb.red, rgb.green, rgb.blue, rgb.brightness
                );
                println!("│   RGB Backlight:    {:<35}│", rgb_str);
            }
            _ => {
                println!("│   RGB Backlight:    {:<35}│", "None detected");
            }
        }
        println!("└────────────────────────────────────────────────────────┘");
        println!(
            "  Press [Ctrl+C] to exit. Refresh: {}ms",
            interval.as_millis()
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            _ = tokio::time::sleep(interval) => {}
        }
    }

    Ok(())
}

pub async fn handle_capabilities(
    client: &DaemonClient,
    json_mode: bool,
) -> Result<(), DaemonClientError> {
    let caps = client.get_capabilities().await?;
    if json_mode {
        let json = serde_json::to_string_pretty(&caps)
            .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
        println!("{json}");
        return Ok(());
    }

    println!(
        "Alatus Platform Capabilities (Schema v{})",
        caps.schema_version
    );
    println!("─────────────────────────────────────────────────────────────────────────────");
    println!("{:<12} {:<15} Details", "Subsystem", "Status");
    println!("─────────────────────────────────────────────────────────────────────────────");

    let format_cap = |name: &str, status: &str, details: &str| {
        println!("{:<12} {:<15} {}", name, status, details);
    };

    match &caps.rgb {
        crate::hardware::CapabilityState::Supported(details) => {
            format_cap(
                "RGB",
                "Supported",
                &format!(
                    "max_brightness: {}, timeout: {}, zones: {:?}",
                    details.max_brightness,
                    details.supports_inactivity_timeout,
                    details.supported_zones
                ),
            );
        }
        crate::hardware::CapabilityState::Unavailable(reason) => {
            format_cap("RGB", "Unavailable", &reason.to_string());
        }
        crate::hardware::CapabilityState::Unsupported => {
            format_cap("RGB", "Unsupported", "Not equipped on this device");
        }
    }

    match &caps.thermal {
        crate::hardware::CapabilityState::Supported(details) => {
            let modes_str: Vec<String> = details
                .supported_modes
                .iter()
                .map(|m| m.to_string())
                .collect();
            format_cap(
                "Thermal",
                "Supported",
                &format!(
                    "modes: [{}], fans: {}",
                    modes_str.join(", "),
                    details.fan_count
                ),
            );
        }
        crate::hardware::CapabilityState::Unavailable(reason) => {
            format_cap("Thermal", "Unavailable", &reason.to_string());
        }
        crate::hardware::CapabilityState::Unsupported => {
            format_cap("Thermal", "Unsupported", "Not equipped on this device");
        }
    }

    match &caps.battery {
        crate::hardware::CapabilityState::Supported(details) => {
            format_cap(
                "Battery",
                "Supported",
                &format!(
                    "threshold range: {}..={}%, charge_control: {}",
                    details.min_threshold, details.max_threshold, details.supports_charge_threshold
                ),
            );
        }
        crate::hardware::CapabilityState::Unavailable(reason) => {
            format_cap("Battery", "Unavailable", &reason.to_string());
        }
        crate::hardware::CapabilityState::Unsupported => {
            format_cap("Battery", "Unsupported", "Not equipped on this device");
        }
    }

    match &caps.display {
        crate::hardware::CapabilityState::Supported(details) => {
            let rates_str: Vec<String> = details
                .supported_refresh_rates
                .iter()
                .map(|r| format!("{r}Hz"))
                .collect();
            format_cap(
                "Display",
                "Supported",
                &format!(
                    "flicker_free_dimming: {}, refresh_rates: [{}]",
                    details.supports_flicker_free_dimming,
                    rates_str.join(", ")
                ),
            );
        }
        crate::hardware::CapabilityState::Unavailable(reason) => {
            format_cap("Display", "Unavailable", &reason.to_string());
        }
        crate::hardware::CapabilityState::Unsupported => {
            format_cap("Display", "Unsupported", "Not equipped on this device");
        }
    }

    Ok(())
}
