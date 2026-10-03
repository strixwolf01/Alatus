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

impl AlatusTray {
    pub fn set_profile(&mut self, profile: &str) {
        self.current_profile = profile.to_string();
        let p = profile.to_string();
        std::thread::spawn(move || {
            let _ = std::process::Command::new("alatus")
                .args(["profile", "set", &p])
                .status();
        });
    }

    pub fn set_battery_limit(&mut self, limit: u32) {
        self.current_limit = limit;
        std::thread::spawn(move || {
            let _ = std::process::Command::new("alatus")
                .args(["battery", "limit", &limit.to_string()])
                .status();
        });
    }

    pub fn set_refresh_rate(&mut self, rate: u32) {
        self.current_refresh = rate;
        let mode = if rate == 60 {
            "output.eDP-1.mode.2"
        } else {
            "output.eDP-1.mode.1"
        };
        std::thread::spawn(move || {
            let _ = std::process::Command::new("kscreen-doctor")
                .args([mode])
                .status();
        });
    }
}

pub fn open_or_raise_gui() {
    std::thread::spawn(|| {
        if let Ok(conn) = zbus::blocking::Connection::session() {
            let has_owner: Result<bool, _> = conn
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "NameHasOwner",
                    &("org.alatus.Gui",),
                )
                .and_then(|reply| reply.body().deserialize());

            if let Ok(true) = has_owner {
                tracing::info!("org.alatus.Gui is already running on session bus. Calling Raise...");
                let _ = conn.call_method(
                    Some("org.alatus.Gui"),
                    "/org/alatus/Gui",
                    Some("org.alatus.Gui"),
                    "Raise",
                    &(),
                );
                return;
            }
        }
        tracing::info!("Spawning new alatus-gui process...");
        let _ = std::process::Command::new("alatus-gui").spawn();
    });
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
        tracing::info!("Tray activated: Open or raise alatus-gui...");
        open_or_raise_gui();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            MenuItem::Standard(StandardItem {
                label: "Open Alatus GUI".into(),
                icon_name: "preferences-system-power".into(),
                activate: Box::new(|_| {
                    open_or_raise_gui();
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::SubMenu(SubMenu {
                label: format!("Thermal Profile: {}", self.current_profile),
                submenu: vec![
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "Quiet".into(),
                        checked: self.current_profile == "Quiet",
                        activate: Box::new(|tray| {
                            tray.set_profile("Quiet");
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "Balanced".into(),
                        checked: self.current_profile == "Balanced",
                        activate: Box::new(|tray| {
                            tray.set_profile("Balanced");
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "Performance".into(),
                        checked: self.current_profile == "Performance",
                        activate: Box::new(|tray| {
                            tray.set_profile("Performance");
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "Full Speed".into(),
                        checked: self.current_profile == "FullSpeed"
                            || self.current_profile == "Full Speed",
                        activate: Box::new(|tray| {
                            tray.set_profile("FullSpeed");
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            MenuItem::SubMenu(SubMenu {
                label: format!("Battery Limit: {}%", self.current_limit),
                submenu: vec![
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "60% (Max Longevity)".into(),
                        checked: self.current_limit == 60,
                        activate: Box::new(|tray| {
                            tray.set_battery_limit(60);
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "80% (Balanced Care)".into(),
                        checked: self.current_limit == 80,
                        activate: Box::new(|tray| {
                            tray.set_battery_limit(80);
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "100% (Full Capacity)".into(),
                        checked: self.current_limit == 100,
                        activate: Box::new(|tray| {
                            tray.set_battery_limit(100);
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            MenuItem::SubMenu(SubMenu {
                label: format!("Display Refresh: {} Hz", self.current_refresh),
                submenu: vec![
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "60 Hz (Power Saving)".into(),
                        checked: self.current_refresh == 60,
                        activate: Box::new(|tray| {
                            tray.set_refresh_rate(60);
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Checkmark(CheckmarkItem {
                        label: "120 Hz (High Smoothness)".into(),
                        checked: self.current_refresh == 120,
                        activate: Box::new(|tray| {
                            tray.set_refresh_rate(120);
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
                    let _ = std::process::Command::new("systemctl")
                        .args(["--user", "stop", "alatus-session.service"])
                        .spawn();
                    std::thread::spawn(|| {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        std::process::exit(0);
                    });
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
