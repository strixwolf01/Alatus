#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Alatus Contributors
# Build release binaries and package RPM archive

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== 1. Checking cargo-generate-rpm ==="
if ! command -v cargo-generate-rpm >/dev/null 2>&1; then
    echo "Error: cargo-generate-rpm not found. Install it via 'cargo install cargo-generate-rpm'" >&2
    exit 1
fi

echo "=== 2. Building Release Binaries with GUI Feature ==="
cd "${WORKSPACE_ROOT}"
cargo build --release --bin alatus --bin alatusd --bin alatus-session --features gui

echo "=== 3. Generating RPM Package ==="
cargo generate-rpm

RPM_FILE="$(find "${WORKSPACE_ROOT}/target/generate-rpm" -maxdepth 1 -name '*.rpm' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -f2- -d' ' || true)"

if [[ -z "${RPM_FILE}" || ! -f "${RPM_FILE}" ]]; then
    echo "Error: Generated RPM package not found under target/generate-rpm/" >&2
    exit 1
fi

echo "Successfully generated RPM package: ${RPM_FILE}"

