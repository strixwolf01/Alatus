#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Alatus Contributors
# Local RPM build, packaging, and pkexec deployment script

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== 1. Building Release Binaries with GUI Feature ==="
cd "${WORKSPACE_ROOT}"
cargo build --release --bin alatus --bin alatusd --bin alatus-session --bin alatus-gui --features gui

echo "=== 2. Generating RPM Package ==="
cargo generate-rpm

RPM_FILE="$(find "${WORKSPACE_ROOT}/target/generate-rpm" -maxdepth 1 -name '*.rpm' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -f2- -d' ' || true)"

if [[ -z "${RPM_FILE}" || ! -f "${RPM_FILE}" ]]; then
    echo "Error: Generated RPM package not found under target/generate-rpm/" >&2
    exit 1
fi

echo "Found generated RPM: ${RPM_FILE}"

echo "=== 3. Mandatory Pre-Installation Clean Slate & Teardown Protocol ==="
# 1. Service Teardown
systemctl --user stop alatus-session.service 2>/dev/null || true
systemctl --user stop ascend-session.service 2>/dev/null || true
pkexec systemctl stop alatusd.service 2>/dev/null || true
pkexec systemctl stop ascend-daemon.service 2>/dev/null || true
pkexec systemctl stop asusd.service 2>/dev/null || true

# 2. Package Removal
for pkg in alatus ascend ayuz asusctl; do
    if rpm -q "${pkg}" >/dev/null 2>&1; then
        echo "Removing existing package: ${pkg}"
        pkexec rpm -e "${pkg}" 2>/dev/null || true
    fi
done

# 3. Orphan & Manual Installation Cleanup (Strict Scope Guardrail)
for bin in /usr/bin/alatus /usr/bin/alatusd /usr/bin/alatus-session /usr/bin/alatus-gui /usr/bin/ascend /usr/bin/ascend-daemon /usr/bin/ascend-session /usr/bin/ascend-gui /usr/bin/ayuz; do
    if [[ -f "${bin}" ]]; then
        echo "Removing manual binary remnant: ${bin}"
        pkexec rm -f "${bin}"
    fi
done

for desktop in /usr/share/applications/io.strixwolf.alatus.desktop /usr/share/applications/alatus-gui.desktop /usr/share/applications/io.strixwolf.ascend.desktop /usr/share/applications/ascend-gui.desktop /usr/share/applications/ascend.desktop /usr/share/applications/io.strixwolf.ayuz.desktop; do
    if [[ -f "${desktop}" || -L "${desktop}" ]]; then
        echo "Removing desktop remnant: ${desktop}"
        pkexec rm -f "${desktop}"
    fi
done

for icon in /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg /usr/share/icons/hicolor/scalable/apps/alatus-gui.svg /usr/share/pixmaps/io.strixwolf.alatus.svg /usr/share/pixmaps/alatus-gui.svg /usr/share/icons/hicolor/scalable/apps/io.strixwolf.ascend.svg /usr/share/icons/hicolor/scalable/apps/ascend-gui.svg /usr/share/pixmaps/io.strixwolf.ascend.svg /usr/share/pixmaps/ascend-gui.svg /usr/share/icons/hicolor/scalable/apps/io.strixwolf.ayuz.svg; do
    if [[ -f "${icon}" || -L "${icon}" ]]; then
        echo "Removing icon remnant: ${icon}"
        pkexec rm -f "${icon}"
    fi
done

for size in 32 48 64 128 256; do
    for icon in /usr/share/icons/hicolor/${size}x${size}/apps/io.strixwolf.alatus.png /usr/share/icons/hicolor/${size}x${size}/apps/alatus-gui.png /usr/share/icons/hicolor/${size}x${size}/apps/io.strixwolf.ascend.png /usr/share/icons/hicolor/${size}x${size}/apps/ascend-gui.png; do
        if [[ -f "${icon}" || -L "${icon}" ]]; then
            pkexec rm -f "${icon}"
        fi
    done
done

for unit in /usr/lib/systemd/system/alatusd.service /usr/lib/systemd/user/alatus-session.service /usr/lib/systemd/user/default.target.wants/alatus-session.service /usr/lib/systemd/system/ascend-daemon.service /usr/lib/systemd/user/ascend-session.service /usr/lib/systemd/user/default.target.wants/ascend-session.service; do
    if [[ -f "${unit}" || -L "${unit}" ]]; then
        echo "Removing unit remnant: ${unit}"
        pkexec rm -f "${unit}"
    fi
done

echo "=== 4. Fresh Deployment via pkexec ==="
pkexec rpm -ivh "${RPM_FILE}"

echo "=== 5. Desktop Entry & Icon Aliasing ==="
pkexec ln -sf io.strixwolf.alatus.svg /usr/share/icons/hicolor/scalable/apps/alatus-gui.svg 2>/dev/null || true
pkexec mkdir -p /usr/share/pixmaps 2>/dev/null || true
pkexec ln -sf /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg /usr/share/pixmaps/io.strixwolf.alatus.svg 2>/dev/null || true
pkexec ln -sf /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg /usr/share/pixmaps/alatus-gui.svg 2>/dev/null || true
for size in 32 48 64 128 256; do
    pkexec mkdir -p "/usr/share/icons/hicolor/${size}x${size}/apps" 2>/dev/null || true
    pkexec ln -sf "io.strixwolf.alatus.png" "/usr/share/icons/hicolor/${size}x${size}/apps/alatus-gui.png" 2>/dev/null || true
done

echo "=== 6. Updating Desktop & Icon Databases ==="
if [ -x /usr/bin/update-desktop-database ]; then
    pkexec /usr/bin/update-desktop-database /usr/share/applications 2>/dev/null || true
fi
if [ -x /usr/bin/gtk-update-icon-cache ]; then
    pkexec /usr/bin/gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental 2>/dev/null || true
fi

echo "=== 7. Reloading and Starting Services ==="
pkexec systemctl daemon-reload
pkexec systemctl restart alatusd.service || true
systemctl --user daemon-reload || true
systemctl --user restart alatus-session.service || true

echo "=== 8. Verifying Service Status ==="
systemctl is-active alatusd.service || true
echo "Deployment completed successfully."
