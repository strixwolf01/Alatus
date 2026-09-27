// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! XDG Desktop Portal accent color extraction and synchronization.

use zbus::zvariant::Value;
use zbus::Connection;

/// Robustly extracts (r, g, b) in [0, 255] range from XDG Desktop Portal accent-color value.
///
/// Handles both D-Bus (ddd) tuples/structs and arrays of doubles/numbers, unwrapping nested variants.
pub fn parse_accent_color(val: &Value<'_>) -> Option<(u8, u8, u8)> {
    match val {
        Value::Value(inner) => parse_accent_color(inner),
        Value::Structure(s) => {
            let fields = s.fields();
            if fields.len() >= 3 {
                let r = extract_float(&fields[0])?;
                let g = extract_float(&fields[1])?;
                let b = extract_float(&fields[2])?;
                Some((to_u8_color(r), to_u8_color(g), to_u8_color(b)))
            } else {
                None
            }
        }
        Value::Array(arr) => {
            let elements = arr.inner();
            if elements.len() >= 3 {
                let r = extract_float(&elements[0])?;
                let g = extract_float(&elements[1])?;
                let b = extract_float(&elements[2])?;
                Some((to_u8_color(r), to_u8_color(g), to_u8_color(b)))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_float(v: &Value<'_>) -> Option<f64> {
    match v {
        Value::Value(inner) => extract_float(inner),
        Value::F64(f) => Some(*f),
        Value::U8(u) => Some(*u as f64 / 255.0),
        Value::U16(u) => Some(*u as f64 / 65535.0),
        Value::U32(u) => Some(*u as f64 / 255.0),
        _ => None,
    }
}

fn to_u8_color(f: f64) -> u8 {
    (f.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub async fn read_portal_accent_color(
    conn: &Connection,
) -> Result<Option<(u8, u8, u8)>, zbus::Error> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .await?;

    let res: Result<zbus::zvariant::OwnedValue, _> = proxy
        .call("Read", &("org.freedesktop.appearance", "accent-color"))
        .await;

    match res {
        Ok(val) => Ok(parse_accent_color(&val)),
        Err(e) => Err(e),
    }
}

pub async fn set_session_accent_sync(enable: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        super::dbus::SESSION_BUS_NAME,
        super::dbus::SESSION_OBJECT_PATH,
        super::dbus::SESSION_BUS_NAME,
    )
    .await?;
    let () = proxy.call("SetSyncAccent", &enable).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{StructureBuilder, Value};

    #[test]
    fn test_parse_accent_color_from_struct() {
        let mut builder = StructureBuilder::new();
        builder.push_field(Value::F64(1.0));
        builder.push_field(Value::F64(0.5));
        builder.push_field(Value::F64(0.0));
        let val = Value::Structure(builder.build().unwrap());

        let color = parse_accent_color(&val);
        assert_eq!(color, Some((255, 128, 0)));
    }

    #[test]
    fn test_parse_accent_color_from_array() {
        let array = vec![Value::F64(0.0), Value::F64(1.0), Value::F64(0.5)];
        let val = Value::Array(array.into());

        let color = parse_accent_color(&val);
        assert_eq!(color, Some((0, 255, 128)));
    }

    #[test]
    fn test_parse_accent_color_nested_value() {
        let mut builder = StructureBuilder::new();
        builder.push_field(Value::F64(0.2));
        builder.push_field(Value::F64(0.4));
        builder.push_field(Value::F64(0.6));
        let s = Value::Structure(builder.build().unwrap());
        let val = Value::Value(Box::new(s));

        let color = parse_accent_color(&val);
        assert_eq!(color, Some((51, 102, 153)));
    }
}
