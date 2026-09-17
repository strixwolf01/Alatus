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
    if path.exists() {
        return true;
    }
    // Check legacy ascend desktop file
    let legacy = path.with_file_name("io.strixwolf.ascend.desktop");
    legacy.exists()
}

pub fn set_autostart_enabled(enabled: bool) {
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
Exec=alatus gui --minimized
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
