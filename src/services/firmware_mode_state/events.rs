// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use futures_util::stream::{Stream, StreamExt as _, select_all};
use zbus::{Connection, MatchRule, MessageStream};

use super::types::EventKind;
use crate::services::dbus::ASUSD_SERVICE;

// ── D-Bus identifiers for the event subscription ─────────────────────────────

const UPOWER_SERVICE: &str = "org.freedesktop.UPower";
const UPOWER_DEVICE_IFACE: &str = "org.freedesktop.UPower.Device";
const UPOWER_DEVICES_NS: &str = "/org/freedesktop/UPower/devices";
const LOGIN1_SERVICE: &str = "org.freedesktop.login1";
const LOGIN1_MANAGER_IFACE: &str = "org.freedesktop.login1.Manager";

pub fn nameowner_is_asusd(msg: zbus::Message) -> bool {
    let Ok((name, _old, _new)) = msg.body().deserialize::<(String, String, String)>() else {
        return false;
    };
    name == ASUSD_SERVICE
}

pub fn login1_is_resume(msg: zbus::Message) -> bool {
    let Ok((sleeping,)) = msg.body().deserialize::<(bool,)>() else {
        return false;
    };
    !sleeping
}

/// Subscribes to D-Bus signals that can indicate power or resume changes.
///
/// Returns the live system-bus connection alongside a merged stream of
/// [`EventKind`]s. Match rules are registered with the bus and deregistered
/// when the stream (and connection) are dropped.
pub async fn subscribe_firmware_mode_events()
-> Result<(Connection, impl Stream<Item = EventKind> + Send), zbus::Error> {
    let conn = Connection::system().await?;

    // UPower device Changed (battery / AC adapter status transitions).
    let upower_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(UPOWER_SERVICE)?
        .path_namespace(UPOWER_DEVICES_NS)?
        .interface(UPOWER_DEVICE_IFACE)?
        .member("Changed")?
        .build();
    let upower = MessageStream::for_match_rule(upower_rule, &conn, Some(16)).await?;

    // asusd (re)started: org.freedesktop.DBus.NameOwnerChanged, arg0 filtered.
    let name_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .add_arg(ASUSD_SERVICE)?
        .build();
    let nameowner = MessageStream::for_match_rule(name_rule, &conn, Some(16)).await?;

    // logind resume: PrepareForSleep, arg0='false'.
    let resume_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(LOGIN1_SERVICE)?
        .interface(LOGIN1_MANAGER_IFACE)?
        .member("PrepareForSleep")?
        .add_arg("false")?
        .build();
    let resume = MessageStream::for_match_rule(resume_rule, &conn, Some(16)).await?;

    let merged = select_all(vec![
        upower
            .filter_map(|msg| async move { msg.ok().map(|_| EventKind::AcBatteryChanged) })
            .boxed(),
        nameowner
            .filter_map(|msg| async move {
                msg.ok()
                    .filter(|m| nameowner_is_asusd(m.clone()))
                    .map(|_| EventKind::AsusdRestarted)
            })
            .boxed(),
        resume
            .filter_map(|msg| async move {
                msg.ok()
                    .filter(|m| login1_is_resume(m.clone()))
                    .map(|_| EventKind::Resume)
            })
            .boxed(),
    ]);

    Ok((conn, merged))
}
