// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! Centralized structured telemetry and logging initialization.

use std::env;
use std::sync::Once;
use tracing_subscriber::filter::EnvFilter;

static INIT_ONCE: Once = Once::new();

/// Initializes structured logging with environment filtering and flexible output formats.
///
/// Configuration:
/// - Environment filter: Reads `RUST_LOG` or `ALATUS_LOG`, defaulting to `info`.
/// - Output format: Setting `ALATUS_LOG_FORMAT=json` or `LOG_FORMAT=json` enables JSON formatting.
/// - Duplicate guards: Uses `try_init` and `Once` to ensure no panics or conflicting initializations.
pub fn init_tracing(service_name: &str) {
    let _ = try_init_tracing(service_name);
}

/// Attempts to initialize structured logging, returning an error if a subscriber is already active.
pub fn try_init_tracing(
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut result: Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> = Ok(());

    INIT_ONCE.call_once(|| {
        let env_filter = EnvFilter::try_from_env("ALATUS_LOG")
            .or_else(|_| EnvFilter::try_from_default_env())
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let format = env::var("ALATUS_LOG_FORMAT")
            .or_else(|_| env::var("LOG_FORMAT"))
            .unwrap_or_default()
            .to_ascii_lowercase();

        if format == "json" {
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(true)
                .json()
                .finish();

            if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
                result = Err(Box::new(e));
            } else {
                tracing::debug!(
                    service = service_name,
                    "Structured JSON logging initialized"
                );
            }
        } else {
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(false)
                .finish();

            if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
                result = Err(Box::new(e));
            } else {
                tracing::debug!(service = service_name, "Standard logging initialized");
            }
        }
    });

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_tracing_idempotent() {
        // Calling init_tracing multiple times must not panic or crash
        init_tracing("test-service-1");
        init_tracing("test-service-2");
        assert!(try_init_tracing("test-service-3").is_ok());
    }
}
