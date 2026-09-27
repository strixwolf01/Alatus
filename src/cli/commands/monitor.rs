// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Real-time terminal hardware telemetry monitor.
//!
//! Provides an interactive ANSI dashboard displaying live temperatures,
//! fan RPMs, battery charge & wattage, thermal mode, and power source without
//! external terminal UI dependencies.

use std::io::{stdout, Write};
use std::time::Duration;
use tokio::time::sleep;

use crate::client::AlatusClient;
use crate::services::daemon_client::DaemonClientError;
use crate::services::firmware_mode::FirmwareMode;
use crate::telemetry::collectors::battery::read_battery_telemetry;
use crate::telemetry::collectors::thermal::read_thermal_telemetry;

pub async fn handle_monitor(
    client: &AlatusClient,
    interval_ms: u64,
) -> Result<(), DaemonClientError> {
    let interval = Duration::from_millis(interval_ms.max(100));

    // Hide terminal cursor
    print!("\x1b[?25l");
    let _ = stdout().flush();

    // Ensure cursor is restored on exit
    struct CursorGuard;
    impl Drop for CursorGuard {
        fn drop(&mut self) {
            println!("\x1b[?25h\x1b[0m");
            let _ = stdout().flush();
        }
    }
    let _cursor_guard = CursorGuard;

    let mut tick_count = 0u64;

    loop {
        tick_count += 1;
        render_frame(client, tick_count).await?;

        tokio::select! {
            _ = sleep(interval) => {}
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    Ok(())
}

async fn render_frame(client: &AlatusClient, tick: u64) -> Result<(), DaemonClientError> {
    let mut out = stdout();

    let thermal = read_thermal_telemetry();
    let battery = read_battery_telemetry();

    let mode = client
        .get_firmware_mode()
        .await
        .unwrap_or(FirmwareMode::Balanced);
    let on_ac = client.get_on_ac().await.unwrap_or(true);
    let charge_limit = client.get_charge_limit().await.unwrap_or(100);

    let profile_desc = match mode {
        FirmwareMode::Balanced => "Balanced",
        FirmwareMode::Quiet => "Quiet",
        FirmwareMode::High => "Performance",
        FirmwareMode::Full => "Full Speed",
        FirmwareMode::Unknown(_) => "Unknown",
    };

    // ANSI: Move cursor to home (1,1) and clear screen
    write!(out, "\x1b[2J\x1b[H").map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    // Header
    writeln!(out, "\x1b[1;36m╔══════════════════════════════════════════════════════════════════════╗\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    writeln!(out, "\x1b[1;36m║\x1b[1;37m                 ALATUS LIVE HARDWARE MONITOR (Tick: {:<6})        \x1b[1;36m║\x1b[0m", tick)
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    writeln!(out, "\x1b[1;36m╠══════════════════════════════════════════════════════════════════════╣\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    // System & Thermal Profile
    let power_badge = if on_ac {
        "\x1b[1;32m[AC POWERED]\x1b[0m"
    } else {
        "\x1b[1;33m[BATTERY]\x1b[0m"
    };
    writeln!(
        out,
        "\x1b[1;36m║\x1b[0m Profile: \x1b[1;35m{:<14}\x1b[0m Power: {:<20} Charge Limit: \x1b[1;33m{:>3}%\x1b[0m \x1b[1;36m║\x1b[0m",
        profile_desc, power_badge, charge_limit
    ).map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    writeln!(out, "\x1b[1;36m╠══════════════════════════════════════════════════════════════════════╣\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    writeln!(out, "\x1b[1;36m║\x1b[1;37m THERMAL & COOLING                                                    \x1b[1;36m║\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    let temp_str = format!("{}°C", thermal.temp_c);
    let fan1_str = format!("{} RPM", thermal.fan_rpm);
    let fan2_str = thermal
        .fan2_rpm
        .map(|r| format!("{r} RPM"))
        .unwrap_or_else(|| "N/A".to_string());

    writeln!(
        out,
        "\x1b[1;36m║\x1b[0m  Package Temp: \x1b[1;31m{:<8}\x1b[0m Fan 1: \x1b[1;34m{:<12}\x1b[0m Fan 2: \x1b[1;34m{:<12}\x1b[0m    \x1b[1;36m║\x1b[0m",
        temp_str, fan1_str, fan2_str
    ).map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    writeln!(out, "\x1b[1;36m╠══════════════════════════════════════════════════════════════════════╣\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    writeln!(out, "\x1b[1;36m║\x1b[1;37m BATTERY & POWER METRICS                                              \x1b[1;36m║\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    if let Some(bat) = battery {
        let wattage = format!("{:.2} W", bat.rate_watts);
        let voltage = format!("{:.2} V", bat.voltage_v);
        let current = format!("{:.2} A", bat.current_a);
        let capacity_bar = render_bar(bat.capacity, 20);
        writeln!(
            out,
            "\x1b[1;36m║\x1b[0m  Capacity: [{}] {:>3}%  Rate: {:<10} State: {:<8} \x1b[1;36m║\x1b[0m",
            capacity_bar, bat.capacity, wattage, bat.status
        ).map_err(|e| DaemonClientError::IoError(e.to_string()))?;
        writeln!(
            out,
            "\x1b[1;36m║\x1b[0m  Voltage:  {:<10} Current: {:<10}                             \x1b[1;36m║\x1b[0m",
            voltage, current
        ).map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    } else {
        writeln!(out, "\x1b[1;36m║\x1b[0m  Battery subsystem not detected or unavailable                      \x1b[1;36m║\x1b[0m")
            .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    }

    writeln!(out, "\x1b[1;36m╚══════════════════════════════════════════════════════════════════════╝\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;
    writeln!(out, "\x1b[2mPress Ctrl+C to exit monitor loop\x1b[0m")
        .map_err(|e| DaemonClientError::IoError(e.to_string()))?;

    out.flush()
        .map_err(|e| DaemonClientError::IoError(e.to_string()))
}

fn render_bar(percentage: u32, width: usize) -> String {
    let clamped = percentage.min(100) as usize;
    let filled = (clamped * width) / 100;
    let empty = width.saturating_sub(filled);

    let color = if clamped > 50 {
        "\x1b[32m" // Green
    } else if clamped > 20 {
        "\x1b[33m" // Yellow
    } else {
        "\x1b[31m" // Red
    };

    format!("{}{}{}\x1b[0m", color, "█".repeat(filled), "░".repeat(empty))
}
