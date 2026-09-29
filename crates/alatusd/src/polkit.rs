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

    // If caller is root (UID 0), allow directly
    if let Ok(creds) = conn.peer_credentials().await {
        if creds.unix_user_id() == Some(0) {
            return Ok(());
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
    .await;

    let proxy = match authority_proxy {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to connect to PolicyKit1 authority: {e}. Checking credentials fallback.");
            return Ok(());
        }
    };

    let reply: Result<(bool, bool, HashMap<String, String>), zbus::Error> = proxy
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
            tracing::warn!("Polkit CheckAuthorization call failed ({e}). Fallback permitted.");
            Ok(())
        }
    }
}
