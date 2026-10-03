//! Server-side D-Bus service implementation for org.alatus.System.

use crate::polkit::check_authorization;
use alatus_ipc::SystemInfoMsg;
use zbus::interface;
use zbus::message::Header;
use zbus::Connection;

pub struct SystemService;

impl Default for SystemService {
    fn default() -> Self {
        Self
    }
}

impl SystemService {
    pub fn new() -> Self {
        Self
    }
}

#[interface(name = "org.alatus.System")]
impl SystemService {
    async fn get_serial_number(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> zbus::fdo::Result<String> {
        check_authorization(conn, &hdr, "org.alatus.read-system-info").await?;

        let serial = std::fs::read_to_string("/sys/class/dmi/id/product_serial")
            .unwrap_or_else(|_| "Unavailable".to_string())
            .trim()
            .to_string();

        Ok(serial)
    }

    async fn get_system_info(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> zbus::fdo::Result<SystemInfoMsg> {
        check_authorization(conn, &hdr, "org.alatus.read-system-info").await?;

        let serial = std::fs::read_to_string("/sys/class/dmi/id/product_serial")
            .unwrap_or_else(|_| "Unavailable".to_string())
            .trim()
            .to_string();

        let product = std::fs::read_to_string("/sys/class/dmi/id/product_name")
            .unwrap_or_else(|_| "Laptop Device".to_string())
            .trim()
            .to_string();

        let board = std::fs::read_to_string("/sys/class/dmi/id/board_name")
            .unwrap_or_else(|_| "Motherboard".to_string())
            .trim()
            .to_string();

        let bios = std::fs::read_to_string("/sys/class/dmi/id/bios_version")
            .unwrap_or_else(|_| "--".to_string())
            .trim()
            .to_string();

        Ok(SystemInfoMsg {
            serial_number: serial,
            product_name: product,
            board_name: board,
            bios_version: bios,
        })
    }
}
