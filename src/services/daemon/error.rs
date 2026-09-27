// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Daemon D-Bus error types.

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.strixwolf.alatus.Error")]
pub enum DaemonError {
    CapabilityUnavailable(String),
    CapabilityUnsupported(String),
    InvalidArgument(String),
    PermissionDenied(String),
    NotSupported(String),
    IoError(String),
    BackendUnavailable(String),
}

impl From<DaemonError> for zbus::fdo::Error {
    fn from(e: DaemonError) -> Self {
        match e {
            DaemonError::CapabilityUnavailable(msg) => zbus::fdo::Error::Failed(msg),
            DaemonError::CapabilityUnsupported(msg) => zbus::fdo::Error::NotSupported(msg),
            DaemonError::InvalidArgument(msg) => zbus::fdo::Error::InvalidArgs(msg),
            DaemonError::PermissionDenied(msg) => zbus::fdo::Error::Failed(msg),
            DaemonError::NotSupported(msg) => zbus::fdo::Error::NotSupported(msg),
            DaemonError::IoError(msg) => zbus::fdo::Error::Failed(msg),
            DaemonError::BackendUnavailable(msg) => zbus::fdo::Error::Failed(msg),
        }
    }
}

impl From<DaemonError> for zbus::Error {
    fn from(e: DaemonError) -> Self {
        zbus::Error::from(zbus::fdo::Error::from(e))
    }
}
