// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Universal FreeDesktop StatusNotifierItem (SNI) and DBusMenu tray implementation.
//!
//! Fully compatible across KDE Plasma, GNOME (AppIndicator), XFCE, wlroots, and Hyprland (Waybar).

use std::collections::HashMap;
use tokio::sync::mpsc;
use zbus::interface;
use zbus::zvariant::{ObjectPath, StructureBuilder, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ToggleWindow,
    ShowWindow,
    SetThermalMode(u32),
    TriggerPixelRefresh,
    Quit,
}

pub struct StatusNotifierItem {
    tx: mpsc::UnboundedSender<TrayEvent>,
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    fn category(&self) -> &str {
        "Hardware"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "io.strixwolf.alatus"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "Alatus"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> i32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        "io.strixwolf.alatus"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn menu(&self) -> ObjectPath<'_> {
        ObjectPath::try_from("/StatusNotifierMenu").unwrap()
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    #[allow(clippy::type_complexity)]
    fn tool_tip(&self) -> (&str, Vec<(i32, i32, Vec<u8>)>, &str, &str) {
        (
            "io.strixwolf.alatus",
            Vec::new(),
            "Alatus - ASUS Hardware Control Center",
            "Left-click: toggle window\nRight-click: options",
        )
    }

    async fn activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayEvent::ToggleWindow);
    }

    async fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayEvent::ToggleWindow);
    }

    async fn context_menu(&self, _x: i32, _y: i32) {
        // Modern tray hosts query DBusMenu at /StatusNotifierMenu directly
    }

    async fn scroll(&self, _delta: i32, _orientation: &str) {}
}

pub struct DBusMenu {
    tx: mpsc::UnboundedSender<TrayEvent>,
}

impl DBusMenu {
    fn item_props(id: i32) -> HashMap<String, Value<'static>> {
        let mut props = HashMap::new();
        match id {
            1 => {
                props.insert("label".to_string(), Value::from("Alatus Hardware Control"));
                props.insert("enabled".to_string(), Value::from(false));
            }
            2 | 7 | 9 => {
                props.insert("type".to_string(), Value::from("separator"));
                props.insert("label".to_string(), Value::from(""));
                props.insert("enabled".to_string(), Value::from(false));
            }
            3 => {
                props.insert("label".to_string(), Value::from("Quiet Mode"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            4 => {
                props.insert("label".to_string(), Value::from("Balanced Mode"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            5 => {
                props.insert("label".to_string(), Value::from("Performance Mode"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            6 => {
                props.insert("label".to_string(), Value::from("Full Speed Mode"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            8 => {
                props.insert("label".to_string(), Value::from("OLED Care: Pixel Refresh"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            10 => {
                props.insert("label".to_string(), Value::from("Open Control Center"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            11 => {
                props.insert("label".to_string(), Value::from("Quit Alatus"));
                props.insert("enabled".to_string(), Value::from(true));
            }
            _ => {}
        }
        props
    }

    fn build_child_structure(id: i32) -> Value<'static> {
        let props = Self::item_props(id);
        let mut builder = StructureBuilder::new();
        builder.push_value(Value::I32(id));
        builder.push_value(Value::from(props));
        let empty_sub: Vec<Value<'static>> = Vec::new();
        builder.push_value(Value::from(empty_sub));
        Value::Structure(builder.build().unwrap())
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl DBusMenu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[allow(clippy::type_complexity)]
    async fn get_layout(
        &self,
        _parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<(
        u32,
        (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>),
    )> {
        let item_ids = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let children: Vec<Value<'static>> = item_ids
            .iter()
            .map(|&id| Self::build_child_structure(id))
            .collect();

        let root_props = HashMap::<String, Value<'static>>::new();
        Ok((1, (0, root_props, children)))
    }

    async fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<Vec<(i32, HashMap<String, Value<'static>>)>> {
        let mut res = Vec::new();
        for id in ids {
            res.push((id, Self::item_props(id)));
        }
        Ok(res)
    }

    async fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<Value<'static>> {
        let props = Self::item_props(id);
        if let Some(val) = props.get(name) {
            Ok(val.clone())
        } else {
            Ok(Value::from(""))
        }
    }

    async fn event(
        &self,
        id: i32,
        event_id: &str,
        _data: Value<'_>,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        if event_id == "clicked" {
            match id {
                3 => {
                    let _ = self.tx.send(TrayEvent::SetThermalMode(0));
                }
                4 => {
                    let _ = self.tx.send(TrayEvent::SetThermalMode(1));
                }
                5 => {
                    let _ = self.tx.send(TrayEvent::SetThermalMode(2));
                }
                6 => {
                    let _ = self.tx.send(TrayEvent::SetThermalMode(3));
                }
                8 => {
                    let _ = self.tx.send(TrayEvent::TriggerPixelRefresh);
                }
                10 => {
                    let _ = self.tx.send(TrayEvent::ShowWindow);
                }
                11 => {
                    let _ = self.tx.send(TrayEvent::Quit);
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn about_to_show(&self, _id: i32) -> zbus::fdo::Result<bool> {
        Ok(false)
    }
}

pub async fn start_tray_service(
    session_conn: &zbus::Connection,
    event_tx: mpsc::UnboundedSender<TrayEvent>,
) -> Result<(), zbus::Error> {
    let sni = StatusNotifierItem {
        tx: event_tx.clone(),
    };
    let menu = DBusMenu { tx: event_tx };

    let _ = session_conn
        .object_server()
        .at("/StatusNotifierItem", sni)
        .await?;
    let _ = session_conn
        .object_server()
        .at("/StatusNotifierMenu", menu)
        .await?;

    let pid = std::process::id();
    let service_name = format!("org.kde.StatusNotifierItem-{pid}-1");
    let _ = session_conn.request_name(service_name).await;

    // Register with StatusNotifierWatcher if available
    let watcher = zbus::Proxy::new(
        session_conn,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await;

    if let Ok(proxy) = watcher {
        let _ = proxy
            .call::<_, _, ()>("RegisterStatusNotifierItem", &("/StatusNotifierItem",))
            .await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dbusmenu_child_signature() {
        let mut props = HashMap::new();
        props.insert("label".to_string(), Value::from("Test"));
        props.insert("enabled".to_string(), Value::from(true));

        let mut builder = StructureBuilder::new();
        builder.push_value(Value::I32(1));
        builder.push_value(Value::from(props));
        let empty_sub: Vec<Value<'static>> = Vec::new();
        builder.push_value(Value::from(empty_sub));
        let structure = builder.build().unwrap();

        assert_eq!(structure.signature(), "(ia{sv}av)");
        let val = Value::Structure(structure);
        assert_eq!(val.value_signature(), "(ia{sv}av)");
    }

    #[test]
    fn test_dbusmenu_all_items_signature() {
        for id in 1..=11 {
            let child = DBusMenu::build_child_structure(id);
            assert_eq!(
                child.value_signature(),
                "(ia{sv}av)",
                "Menu item {id} must strictly have (ia{{sv}}av) signature"
            );
        }
    }
}
