// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Power source, thermal OSD, and screensaver D-Bus event listener streams.

use zbus::{Connection, MatchRule, MessageStream};

pub async fn create_power_stream(conn: &Connection) -> Result<MessageStream, zbus::Error> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("io.strixwolf.alatus.Daemon")?
        .member("PowerSourceChanged")?
        .build();
    MessageStream::for_match_rule(rule, conn, Some(16)).await
}

pub async fn create_thermal_osd_stream(conn: &Connection) -> Result<MessageStream, zbus::Error> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("io.strixwolf.alatus.Daemon")?
        .member("ThermalOsdTriggered")?
        .build();
    MessageStream::for_match_rule(rule, conn, Some(16)).await
}

pub async fn create_screensaver_streams(
    conn: &Connection,
) -> Result<(MessageStream, MessageStream), zbus::Error> {
    let rule_fdo = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.ScreenSaver")?
        .member("ActiveChanged")?
        .build();
    let rule_gnome = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.gnome.ScreenSaver")?
        .member("ActiveChanged")?
        .build();

    let stream_fdo = MessageStream::for_match_rule(rule_fdo, conn, Some(16)).await?;
    let stream_gnome = MessageStream::for_match_rule(rule_gnome, conn, Some(16)).await?;
    Ok((stream_fdo, stream_gnome))
}
