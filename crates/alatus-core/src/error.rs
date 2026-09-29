use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlatusError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Sysfs error at {path}: {message}")]
    Sysfs { path: PathBuf, message: String },

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Hardware capability not supported: {0}")]
    UnsupportedCapability(&'static str),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Profile error: {0}")]
    Profile(String),

    #[error("D-Bus IPC error: {0}")]
    Ipc(String),

    #[error("Driver error [{driver}]: {message}")]
    Driver {
        driver: &'static str,
        message: String,
    },
}
