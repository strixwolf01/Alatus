//! StatusNotifierItem tray implementation using ksni.
//! Provides KDE Plasma / FreeDesktop system tray integration with quick switches.

use ksni::menu::*;
use ksni::{MenuItem, ToolTip, Tray, TrayService};

pub struct AlatusTray {
    pub current_profile: String,
    pub current_limit: u32,
    pub current_refresh: u32,
}

impl Default for AlatusTray {
    fn default() -> Self {
        Self {
            current_profile: "Balanced".to_string(),
            current_limit: 80,
            current_refresh: 120,
        }
    }
}

impl Tray for AlatusTray {
    fn id(&self) -> String {
        "alatus".into()
    }

    fn title(&self) -> String {
        "Alatus Hardware Suite".into()
    }

    fn icon_name(&self) -> String {
        "alatus-gui".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Alatus Hardware Suite".into(),
            description: format!(
                "Profile: {}\nBattery Limit: {}%\nRefresh: {}Hz",
                self.current_profile, self.current_limit, self.current_refresh
            ),
            icon_name: "alatus-gui".into(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        tracing::info!("Tray activated: Spawning alatus-gui...");
        let _ = std::process::Command::new("alatus-gui").spawn();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            MenuItem::Standard(StandardItem {
                label: "Open Alatus GUI".into(),
                icon_name: "preferences-system-power".into(),
                activate: Box::new(|_| {
                    let _ = std::process::Command::new("alatus-gui").spawn();
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::SubMenu(SubMenu {
                label: format!("Thermal Profile: {}", self.current_profile),
                submenu: vec![
                    MenuItem::Standard(StandardItem {
                        label: if self.current_profile == "Quiet" {
                            "● Quiet"
                        } else {
                            "  Quiet"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["profile", "set", "Quiet"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(StandardItem {
                        label: if self.current_profile == "Balanced" {
                            "● Balanced"
                        } else {
                            "  Balanced"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["profile", "set", "Balanced"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(StandardItem {
                        label: if self.current_profile == "Performance" {
                            "● Performance"
                        } else {
                            "  Performance"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["profile", "set", "Performance"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(StandardItem {
                        label: if self.current_profile == "FullSpeed"
                            || self.current_profile == "Full Speed"
                        {
                            "● Full Speed"
                        } else {
                            "  Full Speed"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["profile", "set", "FullSpeed"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            MenuItem::SubMenu(SubMenu {
                label: format!("Battery Limit: {}%", self.current_limit),
                submenu: vec![
                    MenuItem::Standard(StandardItem {
                        label: if self.current_limit == 60 {
                            "● 60% (Max Longevity)"
                        } else {
                            "  60% (Max Longevity)"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["battery", "limit", "60"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(StandardItem {
                        label: if self.current_limit == 80 {
                            "● 80% (Balanced Care)"
                        } else {
                            "  80% (Balanced Care)"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["battery", "limit", "80"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(StandardItem {
                        label: if self.current_limit == 100 {
                            "● 100% (Full Capacity)"
                        } else {
                            "  100% (Full Capacity)"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("alatus")
                                .args(["battery", "limit", "100"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            MenuItem::SubMenu(SubMenu {
                label: format!("Display Refresh: {} Hz", self.current_refresh),
                submenu: vec![
                    MenuItem::Standard(StandardItem {
                        label: if self.current_refresh == 60 {
                            "● 60 Hz (Power Saving)"
                        } else {
                            "  60 Hz (Power Saving)"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("kscreen-doctor")
                                .args(["output.eDP-1.mode.2"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(StandardItem {
                        label: if self.current_refresh == 120 {
                            "● 120 Hz (High Smoothness)"
                        } else {
                            "  120 Hz (High Smoothness)"
                        }
                        .into(),
                        activate: Box::new(|_| {
                            let _ = std::process::Command::new("kscreen-doctor")
                                .args(["output.eDP-1.mode.1"])
                                .spawn();
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit Session Agent".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }),
        ]
    }
}

pub fn spawn_tray() -> ksni::Handle<AlatusTray> {
    let service = TrayService::new(AlatusTray::default());
    let handle = service.handle();
    service.spawn();
    handle
}
