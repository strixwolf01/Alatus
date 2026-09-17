#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Alatus Contributors
# Build release binaries and package Debian (.deb) archive

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== 1. Checking cargo-deb ==="
if ! command -v cargo-deb >/dev/null 2>&1; then
    echo "Error: cargo-deb not found. Install it via 'cargo install cargo-deb'" >&2
    exit 1
fi

echo "=== 2. Building Release Binaries with GUI Feature ==="
cd "${WORKSPACE_ROOT}"
cargo build --release --bin alatus --bin alatusd --bin alatus-session --features gui

echo "=== 3. Generating DEB Package ==="
cargo deb --no-build

DEB_FILE="$(find "${WORKSPACE_ROOT}/target/debian" -maxdepth 1 -name '*.deb' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -f2- -d' ' || true)"

if [[ -z "${DEB_FILE}" || ! -f "${DEB_FILE}" ]]; then
    echo "Error: Generated DEB package not found under target/debian/" >&2
    exit 1
fi

echo "Successfully generated DEB package: ${DEB_FILE}"

