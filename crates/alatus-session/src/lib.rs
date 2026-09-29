pub mod notifier;
pub mod tray;

pub use notifier::DesktopNotifier;
pub use tray::{register_tray_watcher, StatusNotifierItemService, TRAY_OBJECT_PATH};
