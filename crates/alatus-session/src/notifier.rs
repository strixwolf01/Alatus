//! Freedesktop desktop notifications client via org.freedesktop.Notifications.

use std::collections::HashMap;
use zbus::zvariant::Value;
use zbus::Connection;

pub struct DesktopNotifier {
    session_conn: Connection,
}

impl DesktopNotifier {
    pub fn new(session_conn: Connection) -> Self {
        Self { session_conn }
    }

    pub async fn notify(
        &self,
        summary: &str,
        body: &str,
        icon: &str,
        expire_ms: i32,
    ) -> Result<u32, zbus::Error> {
        let proxy = zbus::Proxy::new(
            &self.session_conn,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await?;

        let app_name = "Alatus";
        let replaces_id: u32 = 0;
        let actions: &[&str] = &[];
        let hints: HashMap<&str, Value> = HashMap::new();

        let reply: u32 = proxy
            .call(
                "Notify",
                &(
                    app_name,
                    replaces_id,
                    icon,
                    summary,
                    body,
                    actions,
                    hints,
                    expire_ms,
                ),
            )
            .await?;

        Ok(reply)
    }
}
