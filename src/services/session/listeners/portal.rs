// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! XDG Desktop Portal event listener stream.

use zbus::{Connection, MatchRule, MessageStream};

pub async fn create_portal_stream(conn: &Connection) -> Result<MessageStream, zbus::Error> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Settings")?
        .member("SettingChanged")?
        .build();
    MessageStream::for_match_rule(rule, conn, Some(16)).await
}
