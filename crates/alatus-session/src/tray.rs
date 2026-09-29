//! StatusNotifierItem implementation for Freedesktop system tray.

use zbus::interface;
use zbus::Connection;

pub const TRAY_OBJECT_PATH: &str = "/StatusNotifierItem";

pub struct StatusNotifierItemService {
    current_profile: std::sync::Mutex<String>,
}

impl Default for StatusNotifierItemService {
    fn default() -> Self {
        Self {
            current_profile: std::sync::Mutex::new("Balanced".into()),
        }
    }
}

impl StatusNotifierItemService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_profile(&self, profile: &str) {
        if let Ok(mut p) = self.current_profile.lock() {
            *p = profile.to_string();
        }
    }
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItemService {
    #[zbus(property)]
    pub fn category(&self) -> &str {
        "Hardware"
    }

    #[zbus(property)]
    pub fn id(&self) -> &str {
        "alatus"
    }

    #[zbus(property)]
    pub fn title(&self) -> &str {
        "Alatus Hardware Suite"
    }

    #[zbus(property)]
    pub fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    pub fn icon_name(&self) -> &str {
        "preferences-system-power"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn tool_tip(&self) -> (&str, Vec<(&str, &str)>, &str) {
        (
            "preferences-system-power",
            vec![("Alatus", "ASUS Hardware Control")],
            "Alatus Active",
        )
    }

    async fn activate(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        tracing::info!("Tray icon clicked: Attempting to launch alatus-gui...");
        let _ = tokio::process::Command::new("alatus-gui").spawn();
        Ok(())
    }

    async fn secondary_activate(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        Ok(())
    }

    async fn context_menu(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        Ok(())
    }
}

pub async fn register_tray_watcher(session_conn: &Connection) -> Result<(), zbus::Error> {
    let watcher_proxy = zbus::Proxy::new(
        session_conn,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await;

    if let Ok(watcher) = watcher_proxy {
        let service: &str = session_conn.unique_name().map(|n| n.as_str()).unwrap_or("");
        let _: Result<(), zbus::Error> =
            watcher.call("RegisterStatusNotifierItem", &(service)).await;
        tracing::info!("Registered Alatus with StatusNotifierWatcher");
    } else {
        tracing::debug!("StatusNotifierWatcher not present in this session.");
    }

    Ok(())
}
