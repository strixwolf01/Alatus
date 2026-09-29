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
