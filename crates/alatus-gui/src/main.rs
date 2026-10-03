slint::include_modules!();

mod display;

use alatus_core::settings::AlatusSettings;
use alatus_ipc::{BatteryProxy, LightingProxy, ThermalProxy};
use alatus_profile::dmi::{DmiReader, ProfileResolver};
use futures_util::StreamExt;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::Connection;

#[derive(Debug, Clone, Copy, Default)]
struct CpuStat {
    total: u64,
    idle: u64,
}

impl CpuStat {
    fn read() -> Option<Self> {
        let content = std::fs::read_to_string("/proc/stat").ok()?;
        let first_line = content.lines().next()?;
        if !first_line.starts_with("cpu ") {
            return None;
        }
        let fields: Vec<u64> = first_line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();
        if fields.len() < 4 {
            return None;
        }
        let total: u64 = fields.iter().sum();
        let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
        Some(CpuStat { total, idle })
    }

    fn usage_percentage(&self, prev: &CpuStat) -> u32 {
        let delta_total = self.total.saturating_sub(prev.total);
        let delta_idle = self.idle.saturating_sub(prev.idle);
        if delta_total == 0 {
            return 0;
        }
        let active = delta_total.saturating_sub(delta_idle);
        ((active as f64 / delta_total as f64) * 100.0).round().clamp(0.0, 100.0) as u32
    }
}

fn detect_screen_type(profile_name: Option<&str>) -> String {
    if let Some(name) = profile_name {
        let lower = name.to_lowercase();
        if lower.contains("oled") {
            return "OLED".to_string();
        } else if lower.contains("miniled") || lower.contains("mini-led") {
            return "MiniLED".to_string();
        } else if lower.contains("ips") {
            return "IPS".to_string();
        }
    }

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let edid_path = entry.path().join("edid");
            if let Ok(bytes) = std::fs::read(edid_path) {
                let s = String::from_utf8_lossy(&bytes).to_uppercase();
                if s.contains("OLED") || s.contains("ATNA") {
                    return "OLED".to_string();
                } else if s.contains("MINILED") || s.contains("MINI-LED") {
                    return "MiniLED".to_string();
                }
            }
        }
    }

    "IPS".to_string()
}

struct GuiService {
    handle: slint::Weak<MainWindow>,
}

#[zbus::interface(name = "org.alatus.Gui")]
impl GuiService {
    async fn show(&self) {
        let handle = self.handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle.upgrade() {
                let _ = ui.show();
                ui.window().set_minimized(false);
            }
        });
    }

    async fn raise(&self) {
        self.show().await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set Wayland Application ID environment variables for window managers
    std::env::set_var("WAYLAND_APP_ID", "alatus-gui");
    std::env::set_var("QT_WAYLAND_APP_ID", "alatus-gui");

    tracing_subscriber::fmt::init();
    tracing::info!("Starting Alatus GUI...");

    // Single-Instance Guard: check if another GUI instance is already running
    let session_conn = Connection::session().await.ok();
    if let Some(ref s_conn) = session_conn {
        let has_owner: Result<bool, _> = s_conn
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &("org.alatus.Gui",),
            )
            .await
            .and_then(|reply| reply.body().deserialize());

        if let Ok(true) = has_owner {
            tracing::info!("Another Alatus GUI instance is already running. Raising existing window and exiting.");
            let _ = s_conn
                .call_method(
                    Some("org.alatus.Gui"),
                    "/org/alatus/Gui",
                    Some("org.alatus.Gui"),
                    "Raise",
                    &(),
                )
                .await;
            std::process::exit(0);
        }
    }

    let main_window = MainWindow::new()?;
    // Now that the Slint platform context is initialized, register the Wayland XDG app_id
    if let Err(e) = slint::set_xdg_app_id("alatus-gui") {
        tracing::warn!("Failed to set XDG App ID: {e:?}");
    } else {
        tracing::info!("Successfully registered Wayland XDG App ID: 'alatus-gui'");
    }

    if let Some((r, g, b)) = display::get_kde_accent_color() {
        main_window.set_accent_color(slint::Color::from_rgb_u8(r, g, b));
    }
    let handle = main_window.as_weak();

    // Register GUI D-Bus service on session bus for single-instance raising
    if let Some(ref s_conn) = session_conn {
        let gui_service = GuiService {
            handle: main_window.as_weak(),
        };
        let _ = s_conn.object_server().at("/org/alatus/Gui", gui_service).await;
        let _ = s_conn.request_name("org.alatus.Gui").await;
    }

    // 0. Initial Real DMI & System Hardware Query via ProfileResolver
    let dmi_reader = DmiReader::new();
    let dmi_info = dmi_reader.read_dmi().unwrap_or_default();
    let resolver = ProfileResolver::new(vec![
        std::path::PathBuf::from("data/profiles"),
        std::path::PathBuf::from("/etc/alatus/profiles"),
        std::path::PathBuf::from("/usr/share/alatus/profiles"),
    ]);
    let resolved_profile = resolver.resolve(&dmi_info).ok();

    let raw_product = dmi_info.product_name.clone().unwrap_or_default();
    let board_name = dmi_info
        .board_name
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Motherboard".into());
    let bios_version = dmi_info
        .bios_version
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "--".into());

    let device_model = if let Some(ref prof) = resolved_profile {
        if !prof.metadata.name.trim().is_empty() {
            prof.metadata.name.trim().to_string()
        } else if !raw_product.is_empty() {
            raw_product.clone()
        } else {
            "Laptop Device".to_string()
        }
    } else if !raw_product.is_empty() {
        raw_product.clone()
    } else {
        "Laptop Device".to_string()
    };

    let screen_type = detect_screen_type(resolved_profile.as_ref().map(|p| p.metadata.name.as_str()));

    let kernel_version = match std::process::Command::new("uname").arg("-r").output() {
        Ok(out) => format!("Linux {}", String::from_utf8_lossy(&out.stdout).trim()),
        Err(_) => "Linux x86_64".to_string(),
    };

    let cpu_model = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|info| {
            info.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "Processor".to_string());

    let memory_total = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|info| {
            info.lines()
                .find(|l| l.starts_with("MemTotal:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<f64>().ok())
                .map(|kb| format!("{:.1} GB", kb / 1024.0 / 1024.0))
        })
        .unwrap_or_else(|| "-- GB".to_string());

    let fan_driver_info = {
        if std::path::Path::new("/sys/firmware/acpi/platform_profile").exists() {
            "ASUS ACPI / platform_profile (asus_wmi)".to_string()
        } else {
            "ASUS WMI Kernel Driver".to_string()
        }
    };

    let (ram_pct_init, ram_detail_init) = read_ram_stats();

    let kbd_cfg = display::read_keyboard_idle_config();
    let user_settings = AlatusSettings::load();
    let auto_refresh_init = user_settings.auto_refresh;
    let sync_accent_init = user_settings.sync_accent_color;
    let stay_in_tray_init = user_settings.stay_in_tray;
    let start_on_boot_init = user_settings.start_on_boot;
    let touchpad_gestures_init = user_settings.touchpad_gestures_active;

    let thermal_profile_init = user_settings.thermal_profile.clone();
    let cpu_tracker = Arc::new(Mutex::new(CpuStat::read().unwrap_or_default()));

    let handle_dmi = handle.clone();
    let screen_type_init = screen_type.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle_dmi.upgrade() {
            ui.set_device_model(device_model.into());
            ui.set_board_name(board_name.into());
            ui.set_bios_version(bios_version.into());
            ui.set_kernel_version(kernel_version.into());
            ui.set_device_serial("[ Hidden ]".into());
            ui.set_serial_revealed(false);
            ui.set_ram_usage_text(ram_pct_init.into());
            ui.set_ram_usage_detail(ram_detail_init.into());
            ui.set_cpu_model(cpu_model.into());
            ui.set_memory_total(memory_total.into());
            ui.set_fan_driver_info(fan_driver_info.into());
            ui.set_stay_in_tray(stay_in_tray_init);
            ui.set_start_on_boot(start_on_boot_init);
            ui.set_touchpad_gestures_active(touchpad_gestures_init);
            ui.set_kbd_idle_mode(kbd_cfg.mode.into());
            ui.set_kbd_idle_timeout_ac_min(kbd_cfg.timeout_ac_min as i32);
            ui.set_kbd_idle_timeout_bat_min(kbd_cfg.timeout_bat_min as i32);
            ui.set_auto_refresh_active(auto_refresh_init);
            ui.set_sync_accent_color(sync_accent_init);
            ui.set_screen_type(screen_type_init.into());
            ui.set_current_profile(thermal_profile_init.into());
        }
    });

    let conn = Connection::system().await.map_err(|e| {
        format!("Failed to connect to system D-Bus. Is alatusd daemon running? Error: {e}")
    })?;

    // 1. Initial Data Query: Battery
    let battery_proxy = BatteryProxy::new(&conn).await.ok();
    if let Some(ref bp) = battery_proxy {
        if let Ok(info) = bp.get_info().await {
            let target_limit = user_settings.battery_limit;
            if info.charge_limit != Some(target_limit) {
                let _ = bp.set_charge_limit(target_limit).await;
            }
            let active_limit = info.charge_limit.unwrap_or(target_limit);
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_battery_percentage(info.percentage as i32);
                    ui.set_battery_status(info.status.into());
                    ui.set_battery_limit(active_limit as i32);
                    if let Some(health) = info.health_percentage {
                        ui.set_battery_health(health as i32);
                    }
                    if let Some(microwatts) = info.power_now_microwatts {
                        let watts = microwatts as f64 / 1_000_000.0;
                        if watts > 0.05 {
                            ui.set_battery_power(format!("{:.1} W", watts).into());
                        } else if display::is_ac_connected() {
                            ui.set_battery_power("AC (Bypass)".into());
                        } else {
                            ui.set_battery_power("0.0 W".into());
                        }
                    } else if display::is_ac_connected() {
                        ui.set_battery_power("AC (Bypass)".into());
                    } else {
                        ui.set_battery_power("0.0 W".into());
                    }
                }
            })?;
        }
    }

    // 2. Initial Data Query: Thermal
    let thermal_proxy = ThermalProxy::new(&conn).await.ok();
    if let Some(ref tp) = thermal_proxy {
        let is_cpu_only = tp.is_cpu_only().await.unwrap_or(true);
        let handle_clone = handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle_clone.upgrade() {
                ui.set_is_cpu_only(is_cpu_only);
            }
        });

        if let Ok(current) = tp.get_current_profile().await {
            let target_profile = user_settings.thermal_profile.clone();
            if !current.eq_ignore_ascii_case(&target_profile) {
                let _ = tp.set_profile(target_profile.clone()).await;
            }
            let active_profile = target_profile;
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_current_profile(active_profile.into());
                }
            })?;
        }
        if let Ok(fans) = tp.get_fans().await {
            let cpu_pct = {
                let mut tracker = cpu_tracker.lock().await;
                let cur_stat = CpuStat::read().unwrap_or_default();
                let pct = cur_stat.usage_percentage(&tracker);
                *tracker = cur_stat;
                pct
            };
            let (cpu_only_text, rpm_subtext, cpu_rpm, gpu_rpm) = format_fan_telemetry(&fans, cpu_pct);
            let handle_clone = handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_cpu_fan_rpm(cpu_rpm);
                    ui.set_gpu_fan_rpm(gpu_rpm);
                    ui.set_fan_cpu_only_text(cpu_only_text.into());
                    ui.set_fan_rpm_subtext(rpm_subtext.into());
                }
            })?;
        }
    }

    // 3. Initial Data Query: Lighting
    let lighting_proxy = LightingProxy::new(&conn).await.ok();
    if let Some(ref lp) = lighting_proxy {
        if let Ok(state) = lp.get_state().await {
            let handle_clone = handle.clone();
            let has_multiple_modes = state.supported_modes.len() > 1;
            let supports_breathing = state
                .supported_modes
                .iter()
                .any(|m| m.eq_ignore_ascii_case("breathing"));
            let supports_rainbow = state
                .supported_modes
                .iter()
                .any(|m| m.eq_ignore_ascii_case("rainbow"));
            let supports_strobe = state
                .supported_modes
                .iter()
                .any(|m| m.eq_ignore_ascii_case("strobe"));
            let hex = format!("#{:02X}{:02X}{:02X}", state.r, state.g, state.b);
            let r = state.r;
            let g = state.g;
            let b = state.b;

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_clone.upgrade() {
                    ui.set_lighting_brightness(state.brightness as i32);
                    ui.set_lighting_mode(state.mode.into());
                    ui.set_has_multiple_modes(has_multiple_modes);
                    ui.set_supports_mode_breathing(supports_breathing);
                    ui.set_supports_mode_rainbow(supports_rainbow);
                    ui.set_supports_mode_strobe(supports_strobe);
                    ui.set_lighting_hex(hex.into());
                    ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                }
            })?;
        }
    }

    // 4. Initial Data Query: Display & OLED
    let disp_state = display::query_display_state().await;
    let handle_clone_disp = handle.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle_clone_disp.upgrade() {
            ui.set_is_kde(disp_state.is_kde);
            if let Some((r, g, b)) = disp_state.accent_color {
                ui.set_accent_color(slint::Color::from_rgb_u8(r, g, b));
            }
            ui.set_display_output_name(disp_state.output_name.into());
            ui.set_display_refresh_rate(disp_state.current_refresh_rate as i32);
            ui.set_oled_dimming_level(disp_state.current_dimming as i32);
            ui.set_target_mode_active(disp_state.target_mode_active);
            ui.set_panel_autohide_active(disp_state.panel_autohide);
            ui.set_panel_transparency_active(disp_state.panel_transparency);
            ui.set_dpms_pixel_refresh_active(disp_state.dpms_pixel_refresh);
        }
    });

    if sync_accent_init {
        if let Some((r, g, b)) = disp_state.accent_color {
            if let Some(ref lp) = lighting_proxy {
                let _ = lp.set_color(r, g, b).await;
            }
        }
    }

    // 5. Wire UI Callbacks: Battery Limit
    if let Some(bp) = battery_proxy.clone() {
        let handle_clone = handle.clone();
        main_window.on_set_battery_limit(move |limit| {
            let mut s = AlatusSettings::load();
            s.battery_limit = limit as u8;
            let _ = s.save();
            let bp_clone = bp.clone();
            let handle_inner = handle_clone.clone();
            tokio::spawn(async move {
                if let Ok(()) = bp_clone.set_charge_limit(limit as u8).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_battery_limit(limit);
                        }
                    });
                }
            });
        });
    }

    // 6. Wire UI Callbacks: Thermal Profile
    if let Some(tp) = thermal_proxy.clone() {
        let handle_clone = handle.clone();
        main_window.on_set_thermal_profile(move |mode| {
            let mut s = AlatusSettings::load();
            s.thermal_profile = mode.to_string();
            let _ = s.save();
            let tp_clone = tp.clone();
            let mode_str = mode.to_string();
            let handle_inner = handle_clone.clone();
            let _ = slint::invoke_from_event_loop({
                let h = handle_inner.clone();
                let m = mode_str.clone();
                move || {
                    if let Some(ui) = h.upgrade() {
                        ui.set_current_profile(m.into());
                    }
                }
            });
            tokio::spawn(async move {
                if let Err(e) = tp_clone.set_profile(mode_str.clone()).await {
                    tracing::error!("Failed to set thermal profile {mode_str}: {e}");
                }
            });
        });
    }

    // 7. Wire UI Callbacks: Lighting
    if let Some(lp) = lighting_proxy.clone() {
        let lp_brightness = lp.clone();
        let handle_b = handle.clone();
        main_window.on_set_lighting_brightness(move |level| {
            let mut s = AlatusSettings::load();
            s.lighting_brightness = level as u8;
            let _ = s.save();
            let lp_clone = lp_brightness.clone();
            let handle_inner = handle_b.clone();
            tokio::spawn(async move {
                if let Ok(()) = lp_clone.set_brightness(level as u8).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_brightness(level);
                        }
                    });
                }
            });
        });

        let lp_color = lp.clone();
        let handle_c = handle.clone();
        main_window.on_set_lighting_color(move |r, g, b| {
            let mut s = AlatusSettings::load();
            s.lighting_color = (r as u8, g as u8, b as u8);
            s.sync_accent_color = false;
            let _ = s.save();
            let lp_clone = lp_color.clone();
            let handle_inner = handle_c.clone();
            let hex = format!("#{r:02X}{g:02X}{b:02X}");
            tokio::spawn(async move {
                let _ = lp_clone.set_color(r as u8, g as u8, b as u8).await;
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_lighting_hex(hex.into());
                        ui.set_current_rgb_color(slint::Color::from_rgb_u8(
                            r as u8, g as u8, b as u8,
                        ));
                        ui.set_sync_accent_color(false);
                    }
                });
            });
        });

        let lp_picker = lp.clone();
        let handle_p = handle.clone();
        main_window.on_open_de_color_picker(move | | {
            let lp_clone = lp_picker.clone();
            let handle_inner = handle_p.clone();
            tokio::spawn(async move {
                if let Some((r, g, b)) = pick_color_from_de().await {
                    let mut s = AlatusSettings::load();
                    s.lighting_color = (r, g, b);
                    s.sync_accent_color = false;
                    let _ = s.save();
                    let _ = lp_clone.set_color(r, g, b).await;
                    let hex = format!("#{r:02X}{g:02X}{b:02X}");
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_hex(hex.into());
                            ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                            ui.set_sync_accent_color(false);
                        }
                    });
                }
            });
        });

        let lp_mode = lp.clone();
        let handle_m = handle.clone();
        main_window.on_set_lighting_mode(move |mode, speed| {
            let mut s = AlatusSettings::load();
            s.lighting_mode = mode.to_string();
            let _ = s.save();
            let lp_clone = lp_mode.clone();
            let mode_str = mode.to_string();
            let handle_inner = handle_m.clone();
            tokio::spawn(async move {
                if let Ok(()) = lp_clone.set_mode(mode_str.clone(), speed as u8).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_mode(mode_str.into());
                        }
                    });
                }
            });
        });
    }

    // 8. Wire UI Callbacks: Display & OLED
    let handle_disp_r = handle.clone();
    main_window.on_set_display_refresh(move |hz| {
        let mut s = AlatusSettings::load();
        s.display_refresh_rate = hz as u32;
        let _ = s.save();
        let handle_inner = handle_disp_r.clone();
        tokio::spawn(async move {
            let output_name = {
                handle_inner
                    .upgrade()
                    .map(|ui| ui.get_display_output_name().to_string())
                    .unwrap_or_else(|| "eDP-1".into())
            };
            if let Ok(()) = display::set_refresh_rate(&output_name, hz as u32).await {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_display_refresh_rate(hz);
                    }
                });
            }
        });
    });

    let handle_disp_d = handle.clone();
    main_window.on_set_oled_dimming(move |val| {
        let mut s = AlatusSettings::load();
        s.oled_dimming_level = val as u32;
        let _ = s.save();
        let handle_inner = handle_disp_d.clone();
        tokio::spawn(async move {
            let output_name = {
                handle_inner
                    .upgrade()
                    .map(|ui| ui.get_display_output_name().to_string())
                    .unwrap_or_else(|| "eDP-1".into())
            };
            if let Ok(()) = display::set_oled_dimming(&output_name, val as u32).await {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_oled_dimming_level(val);
                    }
                });
            }
        });
    });

    let handle_disp_tm = handle.clone();
    main_window.on_set_target_mode(move |enabled| {
        let mut s = AlatusSettings::load();
        s.target_mode_active = enabled;
        let _ = s.save();
        let handle_inner = handle_disp_tm.clone();
        tokio::spawn(async move {
            if let Ok(()) = display::set_target_mode(enabled).await {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_target_mode_active(enabled);
                    }
                });
            }
        });
    });

    let handle_disp_ah = handle.clone();
    main_window.on_set_panel_autohide(move |enabled| {
        let mut s = AlatusSettings::load();
        s.panel_autohide_active = enabled;
        let _ = s.save();
        let handle_inner = handle_disp_ah.clone();
        tokio::spawn(async move {
            if let Ok(()) = display::set_panel_autohide(enabled).await {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_panel_autohide_active(enabled);
                    }
                });
            }
        });
    });

    let handle_disp_tr = handle.clone();
    main_window.on_set_panel_transparency(move |enabled| {
        let mut s = AlatusSettings::load();
        s.panel_transparency_active = enabled;
        let _ = s.save();
        let handle_inner = handle_disp_tr.clone();
        tokio::spawn(async move {
            if let Ok(()) = display::set_panel_transparency(enabled).await {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_panel_transparency_active(enabled);
                    }
                });
            }
        });
    });

    let handle_disp_pr = handle.clone();
    main_window.on_set_dpms_pixel_refresh(move |enabled| {
        let mut s = AlatusSettings::load();
        s.dpms_pixel_refresh_active = enabled;
        let _ = s.save();
        let handle_inner = handle_disp_pr.clone();
        tokio::spawn(async move {
            if let Ok(()) = display::set_pixel_refresh_dpms(enabled).await {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_dpms_pixel_refresh_active(enabled);
                    }
                });
            }
        });
    });

    let handle_kbd_m = handle.clone();
    main_window.on_set_kbd_idle_mode(move |mode| {
        let m = mode.to_string();
        let mut s = AlatusSettings::load();
        s.kbd_idle_mode = m.clone();
        let _ = s.save();
        if let Some(ui) = handle_kbd_m.upgrade() {
            ui.set_kbd_idle_mode(mode.clone());
            let timeout_ac = ui.get_kbd_idle_timeout_ac_min() as u32;
            let timeout_bat = ui.get_kbd_idle_timeout_bat_min() as u32;
            tokio::spawn(async move {
                let _ = display::set_keyboard_idle_config(&m, timeout_ac, timeout_bat).await;
            });
        }
    });

    let handle_kbd_ac = handle.clone();
    main_window.on_set_kbd_idle_timeout_ac(move |timeout| {
        let mut s = AlatusSettings::load();
        s.kbd_idle_timeout_ac_min = timeout as u32;
        let _ = s.save();
        if let Some(ui) = handle_kbd_ac.upgrade() {
            ui.set_kbd_idle_timeout_ac_min(timeout);
            let mode = ui.get_kbd_idle_mode().to_string();
            let timeout_bat = ui.get_kbd_idle_timeout_bat_min() as u32;
            tokio::spawn(async move {
                let _ = display::set_keyboard_idle_config(&mode, timeout as u32, timeout_bat).await;
            });
        }
    });

    let handle_kbd_bat = handle.clone();
    main_window.on_set_kbd_idle_timeout_bat(move |timeout| {
        let mut s = AlatusSettings::load();
        s.kbd_idle_timeout_bat_min = timeout as u32;
        let _ = s.save();
        if let Some(ui) = handle_kbd_bat.upgrade() {
            ui.set_kbd_idle_timeout_bat_min(timeout);
            let mode = ui.get_kbd_idle_mode().to_string();
            let timeout_ac = ui.get_kbd_idle_timeout_ac_min() as u32;
            tokio::spawn(async move {
                let _ = display::set_keyboard_idle_config(&mode, timeout_ac, timeout as u32).await;
            });
        }
    });

    let handle_sync = handle.clone();
    let lp_sync = lighting_proxy.clone();
    main_window.on_toggle_sync_accent_color(move |val| {
        let mut s = AlatusSettings::load();
        s.sync_accent_color = val;
        let _ = s.save();
        let handle_inner = handle_sync.clone();
        let lp_clone = lp_sync.clone();
        tokio::spawn(async move {
            if val {
                if let Some((r, g, b)) = display::get_kde_accent_color() {
                    if let Some(ref lp) = lp_clone {
                        let _ = lp.set_color(r, g, b).await;
                    }
                    let hex = format!("#{r:02X}{g:02X}{b:02X}");
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = handle_inner.upgrade() {
                            ui.set_lighting_hex(hex.into());
                            ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                            ui.set_sync_accent_color(true);
                        }
                    });
                }
            } else {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = handle_inner.upgrade() {
                        ui.set_sync_accent_color(false);
                    }
                });
            }
        });
    });

    let handle_ar = handle.clone();
    main_window.on_set_auto_refresh(move |val| {
        let mut s = AlatusSettings::load();
        s.auto_refresh = val;
        let _ = s.save();
        let handle_inner = handle_ar.clone();
        tokio::spawn(async move {
            let h_init = handle_inner.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = h_init.upgrade() {
                    ui.set_auto_refresh_active(val);
                }
            });
            if val {
                let on_ac = display::is_ac_connected();
                let cmd = if on_ac {
                    display::get_on_ac_command()
                } else {
                    display::get_on_bat_command()
                };
                let mut c = tokio::process::Command::new("sh");
                c.arg("-c").arg(&cmd);
                if std::env::var("XDG_RUNTIME_DIR").is_err() {
                    c.env("XDG_RUNTIME_DIR", "/run/user/1000");
                }
                if std::env::var("WAYLAND_DISPLAY").is_err() {
                    c.env("WAYLAND_DISPLAY", "wayland-0");
                }
                if std::env::var("DISPLAY").is_err() {
                    c.env("DISPLAY", ":0");
                }
                let _ = c.status().await;
                let expected_hz = if on_ac { 120 } else { 60 };
                let h = handle_inner.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = h.upgrade() {
                        ui.set_display_refresh_rate(expected_hz);
                    }
                });
            }
        });
    });

    let handle_tray = handle.clone();
    main_window.on_toggle_stay_in_tray(move |val| {
        let mut s = AlatusSettings::load();
        s.stay_in_tray = val;
        let _ = s.save();
        let handle_inner = handle_tray.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle_inner.upgrade() {
                ui.set_stay_in_tray(val);
            }
        });
    });

    let handle_boot = handle.clone();
    main_window.on_toggle_start_on_boot(move |val| {
        let mut s = AlatusSettings::load();
        s.start_on_boot = val;
        let _ = s.save();
        let handle_inner = handle_boot.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle_inner.upgrade() {
                ui.set_start_on_boot(val);
            }
        });
        let action = if val { "enable" } else { "disable" };
        let _ = std::process::Command::new("systemctl")
            .args(["--user", action, "alatus-session.service"])
            .status();
    });

    let handle_gestures = handle.clone();
    main_window.on_toggle_touchpad_gestures(move |val| {
        let mut s = AlatusSettings::load();
        s.touchpad_gestures_active = val;
        let _ = s.save();
        let handle_inner = handle_gestures.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle_inner.upgrade() {
                ui.set_touchpad_gestures_active(val);
            }
        });
    });

    // Elevated Serial Number Reveal via Polkit / pkexec
    let handle_serial = handle.clone();
    main_window.on_reveal_serial(move || {
        let h = handle_serial.clone();
        tokio::spawn(async move {
            tracing::info!("User requested serial number reveal via pkexec elevated authentication...");
            let mut serial_result = None;

            // Direct pkexec invocation triggers the system Polkit GUI authentication prompt
            // every time the user requests to reveal the device serial number.
            let pkexec_res = tokio::process::Command::new("pkexec")
                .args(["cat", "/sys/class/dmi/id/product_serial"])
                .output()
                .await;

            match pkexec_res {
                Ok(output) if output.status.success() => {
                    let clean = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !clean.is_empty() && clean != "Unavailable" {
                        serial_result = Some(clean);
                    }
                }
                Ok(output) => {
                    tracing::warn!("pkexec authentication cancelled or failed: status={}", output.status);
                }
                Err(e) => {
                    tracing::error!("Failed to invoke pkexec: {e}");
                }
            }

            if let Some(serial) = serial_result {
                tracing::info!("Successfully authenticated via Polkit and read device serial number: {}", serial);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = h.upgrade() {
                        ui.set_device_serial(serial.into());
                        ui.set_serial_revealed(true);
                    }
                });
            } else {
                tracing::warn!("Failed or canceled elevated authorization for serial number");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = h.upgrade() {
                        ui.set_serial_revealed(false);
                        ui.set_device_serial("[ Hidden ]".into());
                    }
                });
            }
        });
    });

    let handle_hide_serial = handle.clone();
    main_window.on_hide_serial(move || {
        let h = handle_hide_serial.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = h.upgrade() {
                ui.set_serial_revealed(false);
                ui.set_device_serial("[ Hidden ]".into());
            }
        });
    });

    // 9. Background Signal Monitors for Live Hardware Updates
    if let Some(tp) = thermal_proxy.clone() {
        let handle_t = handle.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = tp.receive_profile_changed().await {
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        let new_profile = args.new_profile;
                        let handle_inner = handle_t.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle_inner.upgrade() {
                                ui.set_current_profile(new_profile.into());
                            }
                        });
                    }
                }
            }
        });
    }

    if let Some(ref lp) = lighting_proxy {
        let handle_l = handle.clone();
        let lp_stream = lp.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = lp_stream.receive_state_changed().await {
                while let Some(signal) = stream.next().await {
                    if let Ok(args) = signal.args() {
                        let state = args.state;
                        let handle_inner = handle_l.clone();
                        let hex = format!("#{:02X}{:02X}{:02X}", state.r, state.g, state.b);
                        let r = state.r;
                        let g = state.g;
                        let b = state.b;
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle_inner.upgrade() {
                                ui.set_lighting_brightness(state.brightness as i32);
                                ui.set_lighting_mode(state.mode.into());
                                ui.set_lighting_hex(hex.into());
                                ui.set_current_rgb_color(slint::Color::from_rgb_u8(r, g, b));
                            }
                        });
                    }
                }
            }
        });
    }

    // 10. Live Desktop Accent Color Synchronizer (300ms timer on UI thread)
    let accent_timer = slint::Timer::default();
    {
        let handle_accent = handle.clone();
        let lp_accent = lighting_proxy.clone();
        let mut last_color: Option<(u8, u8, u8)> = None;

        accent_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(300),
            move || {
                if let Some((r, g, b)) = display::get_kde_accent_color() {
                    if last_color != Some((r, g, b)) {
                        last_color = Some((r, g, b));
                        if let Some(ui) = handle_accent.upgrade() {
                            let new_accent = slint::Color::from_rgb_u8(r, g, b);
                            if ui.get_accent_color() != new_accent {
                                ui.set_accent_color(new_accent);
                            }
                            if ui.get_sync_accent_color() {
                                let hex = format!("#{r:02X}{g:02X}{b:02X}");
                                ui.set_lighting_hex(hex.into());
                                ui.set_current_rgb_color(new_accent);
                                if let Some(ref lp) = lp_accent {
                                    let lp = lp.clone();
                                    tokio::spawn(async move {
                                        let _ = lp.set_color(r, g, b).await;
                                    });
                                }
                            }
                        }
                    }
                }
            },
        );
    }

    // 11. Periodic Polling for Live Telemetry (Battery & Fans)
    let poller_timer = slint::Timer::default();
    let last_ac_state = Arc::new(Mutex::new(None::<bool>));
    {
        let bp_poll = battery_proxy.clone();
        let tp_poll = thermal_proxy.clone();
        let handle_poll = handle.clone();
        let cpu_tracker_poll = cpu_tracker.clone();
        let last_ac_poll = last_ac_state.clone();

        poller_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(1500),
            move || {
                let bp = bp_poll.clone();
                let tp = tp_poll.clone();
                let handle_inner = handle_poll.clone();
                let cpu_tracker_inner = cpu_tracker_poll.clone();
                let last_ac_poll = last_ac_poll.clone();

                tokio::spawn(async move {
                    if let Some(ref bp) = bp {
                        if let Ok(info) = bp.get_info().await {
                            let h = handle_inner.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.set_battery_percentage(info.percentage as i32);
                                    ui.set_battery_status(info.status.into());
                                    if let Some(limit) = info.charge_limit {
                                        ui.set_battery_limit(limit as i32);
                                    }
                                    if let Some(health) = info.health_percentage {
                                        ui.set_battery_health(health as i32);
                                    }
                                    if let Some(microwatts) = info.power_now_microwatts {
                                        let watts = microwatts as f64 / 1_000_000.0;
                                        if watts > 0.05 {
                                            ui.set_battery_power(format!("{:.1} W", watts).into());
                                        } else if display::is_ac_connected() {
                                            ui.set_battery_power("AC (Bypass)".into());
                                        } else {
                                            ui.set_battery_power("0.0 W".into());
                                        }
                                    } else if display::is_ac_connected() {
                                        ui.set_battery_power("AC (Bypass)".into());
                                    } else {
                                        ui.set_battery_power("0.0 W".into());
                                    }
                                }
                            });
                        }
                    }

                    // Auto refresh rate on power transition
                    let ac_online = display::is_ac_connected();
                    let mut last_ac = last_ac_poll.lock().await;
                    if *last_ac != Some(ac_online) {
                        let is_initial = last_ac.is_none();
                        *last_ac = Some(ac_online);
                        if !is_initial {
                            let s = AlatusSettings::load();
                            if s.auto_refresh {
                                let cmd = if ac_online {
                                    display::get_on_ac_command()
                                } else {
                                    display::get_on_bat_command()
                                };
                                let mut c = tokio::process::Command::new("sh");
                                c.arg("-c").arg(&cmd);
                                if std::env::var("XDG_RUNTIME_DIR").is_err() {
                                    c.env("XDG_RUNTIME_DIR", "/run/user/1000");
                                }
                                if std::env::var("WAYLAND_DISPLAY").is_err() {
                                    c.env("WAYLAND_DISPLAY", "wayland-0");
                                }
                                if std::env::var("DISPLAY").is_err() {
                                    c.env("DISPLAY", ":0");
                                }
                                let _ = c.status().await;
                                let expected_hz = if ac_online { 120 } else { 60 };
                                let h = handle_inner.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.set_display_refresh_rate(expected_hz);
                                    }
                                });
                            }
                        }
                    }

                    if let Some(ref tp) = tp {
                        if let Ok(fans) = tp.get_fans().await {
                            let cpu_pct = {
                                let mut tracker = cpu_tracker_inner.lock().await;
                                let cur_stat = CpuStat::read().unwrap_or_default();
                                let pct = cur_stat.usage_percentage(&tracker);
                                *tracker = cur_stat;
                                pct
                            };
                            let (cpu_only_text, rpm_subtext, cpu_rpm, gpu_rpm) =
                                format_fan_telemetry(&fans, cpu_pct);
                            let h = handle_inner.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.set_cpu_fan_rpm(cpu_rpm);
                                    ui.set_gpu_fan_rpm(gpu_rpm);
                                    ui.set_fan_cpu_only_text(cpu_only_text.into());
                                    ui.set_fan_rpm_subtext(rpm_subtext.into());
                                }
                            });
                        }

                        if let Ok(current) = tp.get_current_profile().await {
                            let h = handle_inner.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    if ui.get_current_profile() != current.as_str() {
                                        ui.set_current_profile(current.into());
                                    }
                                }
                            });
                        }
                    }

                    // Live RAM Telemetry update
                    let (ram_pct, ram_detail) = read_ram_stats();
                    let h_ram = handle_inner.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h_ram.upgrade() {
                            ui.set_ram_usage_text(ram_pct.into());
                            ui.set_ram_usage_detail(ram_detail.into());
                        }
                    });
                });
            },
        );
    }

    main_window.window().on_close_requested(move || {
        slint::CloseRequestResponse::HideWindow
    });

    main_window.run()?;
    Ok(())
}

fn format_fan_telemetry(fans: &[alatus_ipc::FanStatusMsg], cpu_pct: u32) -> (String, String, i32, i32) {
    let cpu_rpm = fans.first().map(|f| f.current_rpm as i32).unwrap_or(0);
    let gpu_rpm = fans.get(1).map(|f| f.current_rpm as i32).unwrap_or(0);

    let (cpu_only_text, rpm_subtext) = if fans.len() >= 2 {
        (
            format!(" CPU : {}% ", cpu_pct),
            format!("{} RPM / {} RPM", cpu_rpm, gpu_rpm),
        )
    } else if let Some(fan) = fans.first() {
        (
            format!(" CPU : {}% ", cpu_pct),
            format!("{} RPM", fan.current_rpm),
        )
    } else {
        (format!(" CPU : {}% ", cpu_pct), "".to_string())
    };

    (cpu_only_text, rpm_subtext, cpu_rpm, gpu_rpm)
}

async fn pick_color_from_de() -> Option<(u8, u8, u8)> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let is_kde = desktop.contains("kde");

    let output = if is_kde && std::path::Path::new("/usr/bin/kdialog").exists() {
        tokio::process::Command::new("kdialog")
            .arg("--getcolor")
            .output()
            .await
            .ok()
    } else if std::path::Path::new("/usr/bin/zenity").exists() {
        tokio::process::Command::new("zenity")
            .arg("--color-selection")
            .arg("--show-palette")
            .output()
            .await
            .ok()
    } else if std::path::Path::new("/usr/bin/kdialog").exists() {
        tokio::process::Command::new("kdialog")
            .arg("--getcolor")
            .output()
            .await
            .ok()
    } else {
        None
    };

    if let Some(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return parse_color_string(&stdout);
        }
    }
    None
}

fn parse_color_string(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Format 1: #RRGGBB or #RRGGBBAA or RRGGBB
    let hex_part = s.trim_start_matches('#');
    if hex_part.len() >= 6 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&hex_part[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex_part[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex_part[4..6], 16).ok()?;
        return Some((r, g, b));
    }

    // Format 2: rgb(r, g, b) or rgba(r, g, b, a)
    if let Some(inner) = s
        .strip_prefix("rgb(")
        .and_then(|t| t.strip_suffix(')'))
        .or_else(|| s.strip_prefix("rgba(").and_then(|t| t.strip_suffix(')')))
    {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            return Some((r, g, b));
        }
    }

    None
}

fn read_ram_stats() -> (String, String) {
    if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
        let mut total_kb = 0.0;
        let mut avail_kb = 0.0;
        for line in info.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb) = line.split_whitespace().nth(1).and_then(|s| s.parse::<f64>().ok()) {
                    total_kb = kb;
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(kb) = line.split_whitespace().nth(1).and_then(|s| s.parse::<f64>().ok()) {
                    avail_kb = kb;
                }
            }
        }
        if total_kb > 0.0 {
            let used_kb = (total_kb - avail_kb).max(0.0);
            let pct = ((used_kb / total_kb) * 100.0).round() as u32;
            let used_gb = used_kb / 1024.0 / 1024.0;
            let total_gb = total_kb / 1024.0 / 1024.0;
            return (
                format!("{}%", pct),
                format!("{:.1} GB / {:.1} GB", used_gb, total_gb),
            );
        }
    }
    ("0%".to_string(), "-- / --".to_string())
}

