// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

pub const ASUSD_SERVICE: &str = "org.asuslinux.Daemon";

/// Checks whether the `asusd` service is currently running and owns its D-Bus name.
pub async fn is_asusd_running() -> bool {
    if let Ok(conn) = zbus::Connection::system().await
        && let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await
        && let Ok(name) = zbus::names::WellKnownName::try_from(ASUSD_SERVICE)
    {
        return proxy.name_has_owner(name.into()).await.unwrap_or(false);
    }
    false
}
