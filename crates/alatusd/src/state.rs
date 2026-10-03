//! System-level state persistence for alatusd.
//!
//! Preserves hardware states (battery charge limit, thermal profile) across
//! daemon restarts, system reboots, and sleep/wake cycles.

pub fn load_daemon_state() -> Option<(Option<u8>, Option<String>)> {
    let content = std::fs::read_to_string("/var/lib/alatus/state.json").ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let limit = val
        .get("battery_limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as u8);
    let profile = val
        .get("thermal_profile")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some((limit, profile))
}

pub fn save_daemon_battery_limit(limit: u8) {
    update_daemon_state(|v| {
        v["battery_limit"] = serde_json::json!(limit);
    });
}

pub fn save_daemon_thermal_profile(profile: &str) {
    update_daemon_state(|v| {
        v["thermal_profile"] = serde_json::json!(profile);
    });
}

fn update_daemon_state<F>(update_fn: F)
where
    F: FnOnce(&mut serde_json::Value),
{
    let path = std::path::Path::new("/var/lib/alatus/state.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut val = std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    update_fn(&mut val);
    if let Ok(data) = serde_json::to_string_pretty(&val) {
        let _ = std::fs::write(path, data);
    }
}
