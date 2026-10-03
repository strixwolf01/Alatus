//! Polkit authorization check via org.freedesktop.PolicyKit1 D-Bus authority.

use std::collections::HashMap;
use zbus::message::Header;
use zbus::zvariant::Value;
use zbus::Connection;

pub async fn check_authorization(
    conn: &Connection,
    header: &Header<'_>,
    action_id: &str,
) -> Result<(), zbus::fdo::Error> {
    let sender = match header.sender() {
        Some(s) => s.as_str(),
        None => return Err(zbus::fdo::Error::Failed("Missing message sender".into())),
    };

    // Query the actual caller's UID via org.freedesktop.DBus.GetConnectionUnixUser
    let dbus_proxy = zbus::fdo::DBusProxy::new(conn)
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to connect to D-Bus daemon: {e}")))?;

    if let Ok(bus_name) = sender.try_into() {
        if let Ok(uid) = dbus_proxy.get_connection_unix_user(bus_name).await {
            // If the actual client process is running as root (UID 0), bypass polkit
            if uid == 0 {
                return Ok(());
            }
        }
    }

    // Call org.freedesktop.PolicyKit1.Authority.CheckAuthorization
    let mut subject_details = HashMap::new();
    subject_details.insert("name", Value::from(sender));
    let subject = ("system-bus-name", subject_details);

    let details: HashMap<&str, &str> = HashMap::new();
    let flags: u32 = 1; // AllowUserInteraction = 1
    let cancellation_id = "";

    let authority_proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await
    .map_err(|e| zbus::fdo::Error::Failed(format!("PolicyKit authority unavailable: {e}")))?;

    let reply: Result<(bool, bool, HashMap<String, String>), zbus::Error> = authority_proxy
        .call(
            "CheckAuthorization",
            &(subject, action_id, details, flags, cancellation_id),
        )
        .await;

    match reply {
        Ok((is_authorized, _is_challenge, _details)) => {
            if is_authorized {
                Ok(())
            } else {
                Err(zbus::fdo::Error::Failed(format!(
                    "Polkit authorization rejected for action {action_id}"
                )))
            }
        }
        Err(e) => {
            Err(zbus::fdo::Error::Failed(format!(
                "Polkit CheckAuthorization call failed: {e}"
            )))
        }
    }
}
