// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::services::tray::TrayEvent;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{error, info};

pub fn get_autostart_desktop_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("autostart").join("io.strixwolf.alatus.desktop")
}

pub fn is_autostart_enabled() -> bool {
    let path = get_autostart_desktop_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if content.contains("Hidden=true") || content.contains("X-GNOME-Autostart-enabled=false") {
            return false;
        }
        if content.contains("X-GNOME-Autostart-enabled=true") {
            return true;
        }
    }
    // Check legacy desktop files
    let legacy_autostart = path.with_file_name("io.strixwolf.alatus.autostart.desktop");
    if let Ok(content) = std::fs::read_to_string(&legacy_autostart) {
        if content.contains("Hidden=true") || content.contains("X-GNOME-Autostart-enabled=false") {
            return false;
        }
        return true;
    }
    let legacy_ascend = path.with_file_name("io.strixwolf.ascend.desktop");
    if legacy_ascend.exists() {
        return true;
    }

    // Check user configuration
    let config = crate::services::config::load_config();
    config.autostart_enabled
}

pub fn set_autostart_enabled(enabled: bool) {
    let path = get_autostart_desktop_path();
    let legacy_autostart = path.with_file_name("io.strixwolf.alatus.autostart.desktop");
    let legacy_ascend = path.with_file_name("io.strixwolf.ascend.desktop");
    if enabled {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = r#"[Desktop Entry]
Type=Application
Name=Alatus
GenericName=ASUS Hardware Control Center
Comment=Hardware control suite for ASUS laptops on Linux
Exec=alatus gui --tray
Icon=io.strixwolf.alatus
Terminal=false
Categories=Settings;HardwareSettings;
StartupNotify=false
X-GNOME-Autostart-enabled=true
X-KDE-autostart-after=panel
"#;
        if let Err(e) = std::fs::write(&path, content) {
            error!("Failed to write autostart desktop file: {e}");
        } else {
            info!("Autostart desktop entry created at {:?}", path);
            let _ = std::fs::remove_file(&legacy_autostart);
            let _ = std::fs::remove_file(&legacy_ascend);
        }
    } else {
        let system_autostart =
            std::path::Path::new("/etc/xdg/autostart/io.strixwolf.alatus.desktop");
        if system_autostart.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let override_content = r#"[Desktop Entry]
Type=Application
Name=Alatus
Exec=alatus gui --tray
Hidden=true
X-GNOME-Autostart-enabled=false
"#;
            let _ = std::fs::write(&path, override_content);
        } else {
            let _ = std::fs::remove_file(&path);
        }
        let _ = std::fs::remove_file(&legacy_autostart);
        let _ = std::fs::remove_file(&legacy_ascend);
    }
}

pub struct GuiIpcServer {
    pub tray_tx: mpsc::UnboundedSender<TrayEvent>,
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
