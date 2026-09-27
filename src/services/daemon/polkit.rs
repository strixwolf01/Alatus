// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! PolicyKit authorization checks for privileged daemon D-Bus operations.

use std::collections::HashMap;
use zbus::Connection;

use super::error::DaemonError;

pub fn parse_polkit_result(is_authorized: bool) -> Result<(), DaemonError> {
    if is_authorized {
        Ok(())
    } else {
        Err(DaemonError::PermissionDenied(
            "Polkit authorization failed".to_string(),
        ))
    }
}

pub async fn check_polkit(
    conn: &Connection,
    sender: &zbus::names::UniqueName<'_>,
    action_id: &str,
) -> Result<(), DaemonError> {
    let mut details = HashMap::new();
    details.insert(
        "name".to_string(),
        zbus::zvariant::Value::from(sender.as_str()),
    );

    let subject = ("system-bus-name".to_string(), details);
    let empty_details: HashMap<String, String> = HashMap::new();
    let flags: u32 = 1; // AllowUserInteraction
    let cancellation_id = "";

    let proxy = match zbus::Proxy::new(
        conn,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return Err(DaemonError::BackendUnavailable(format!(
                "PolicyKit not available: {e}"
            )));
        }
    };

    let response: Result<(bool, bool, HashMap<String, String>), zbus::Error> = proxy
        .call(
            "CheckAuthorization",
            &(subject, action_id, empty_details, flags, cancellation_id),
        )
        .await;

    match response {
        Ok((is_authorized, _, _)) => parse_polkit_result(is_authorized),
        Err(e) => {
            tracing::error!("Polkit check failed with D-Bus error: {e}");
            Err(DaemonError::PermissionDenied(format!(
                "Polkit communication failed: {e}"
            )))
        }
    }
}
