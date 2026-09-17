// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::hardware::capabilities::UnavailableReason;
use thiserror::Error;

/// Low-level driver communication and hardware control errors.
#[derive(Debug, Error)]
pub enum DriverError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Hardware unavailable: {0}")]
    Unavailable(UnavailableReason),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Hardware communication failure: {0}")]
    Communication(String),

    #[error("Unsupported driver operation: {0}")]
    Unsupported(String),
}
