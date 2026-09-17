#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Alatus Contributors
# Local RPM build, packaging, and single-elevation deployment script

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== 1. Building Release Binaries with GUI Feature ==="
cd "${WORKSPACE_ROOT}"
cargo build --release --bin alatus --bin alatusd --bin alatus-session --features gui

echo "=== 2. Generating RPM Package ==="
cargo generate-rpm

RPM_FILE="$(find "${WORKSPACE_ROOT}/target/generate-rpm" -maxdepth 1 -name '*.rpm' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -f2- -d' ' || true)"

if [[ -z "${RPM_FILE}" || ! -f "${RPM_FILE}" ]]; then
    echo "Error: Generated RPM package not found under target/generate-rpm/" >&2
    exit 1
fi

echo "Found generated RPM: ${RPM_FILE}"

echo "=== 3. Stopping User Services ==="
systemctl --user stop alatus-session.service 2>/dev/null || true
systemctl --user stop ascend-session.service 2>/dev/null || true

echo "=== 4. Root Operations (Single Elevation) ==="
if [[ $EUID -eq 0 ]]; then
    ELEVATE_CMD=""
elif command -v sudo >/dev/null 2>&1; then
    echo "Elevating privileges via sudo (enter password once if prompted)..."
    sudo -v
    ELEVATE_CMD="sudo"
else
    echo "Elevating privileges via pkexec..."
    ELEVATE_CMD="pkexec"
fi

${ELEVATE_CMD} env RPM_FILE="${RPM_FILE}" bash << 'ROOT_EOF'
set -euo pipefail

echo "Stopping system daemons..."
systemctl stop alatusd.service 2>/dev/null || true
systemctl stop ascend-daemon.service 2>/dev/null || true
systemctl stop asusd.service 2>/dev/null || true

# Package Removal
for pkg in alatus ascend ayuz asusctl; do
    if rpm -q "${pkg}" >/dev/null 2>&1; then
        echo "Removing existing package: ${pkg}"
        rpm -e "${pkg}" 2>/dev/null || true
    fi
done

# Orphan & Manual Installation Cleanup
for bin in /usr/bin/alatus /usr/bin/alatusd /usr/bin/alatus-session /usr/bin/alatus-gui /usr/bin/ascend /usr/bin/ascend-daemon /usr/bin/ascend-session /usr/bin/ascend-gui /usr/bin/ayuz; do
    if [[ -f "${bin}" ]]; then
        echo "Removing manual binary remnant: ${bin}"
        rm -f "${bin}"
    fi
done

for desktop in /usr/share/applications/io.strixwolf.alatus.desktop /usr/share/applications/alatus-gui.desktop /usr/share/applications/io.strixwolf.ascend.desktop /usr/share/applications/ascend-gui.desktop /usr/share/applications/ascend.desktop /usr/share/applications/io.strixwolf.ayuz.desktop; do
    if [[ -f "${desktop}" || -L "${desktop}" ]]; then
        echo "Removing desktop remnant: ${desktop}"
        rm -f "${desktop}"
    fi
done

for icon in /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg /usr/share/icons/hicolor/scalable/apps/alatus-gui.svg /usr/share/pixmaps/io.strixwolf.alatus.svg /usr/share/pixmaps/alatus-gui.svg /usr/share/icons/hicolor/scalable/apps/io.strixwolf.ascend.svg /usr/share/icons/hicolor/scalable/apps/ascend-gui.svg /usr/share/pixmaps/io.strixwolf.ascend.svg /usr/share/pixmaps/ascend-gui.svg /usr/share/icons/hicolor/scalable/apps/io.strixwolf.ayuz.svg; do
    if [[ -f "${icon}" || -L "${icon}" ]]; then
        echo "Removing icon remnant: ${icon}"
        rm -f "${icon}"
    fi
done

for size in 32 48 64 128 256; do
    for icon in /usr/share/icons/hicolor/${size}x${size}/apps/io.strixwolf.alatus.png /usr/share/icons/hicolor/${size}x${size}/apps/alatus-gui.png /usr/share/icons/hicolor/${size}x${size}/apps/io.strixwolf.ascend.png /usr/share/icons/hicolor/${size}x${size}/apps/ascend-gui.png; do
        if [[ -f "${icon}" || -L "${icon}" ]]; then
            rm -f "${icon}"
        fi
    done
done

for unit in /usr/lib/systemd/system/alatusd.service /usr/lib/systemd/user/alatus-session.service /usr/lib/systemd/user/default.target.wants/alatus-session.service /usr/lib/systemd/system/ascend-daemon.service /usr/lib/systemd/user/ascend-session.service /usr/lib/systemd/user/default.target.wants/ascend-session.service; do
    if [[ -f "${unit}" || -L "${unit}" ]]; then
        echo "Removing unit remnant: ${unit}"
        rm -f "${unit}"
    fi
done

echo "Installing RPM: ${RPM_FILE}"
rpm -Uvh --replacepkgs "${RPM_FILE}"

echo "Desktop Entry & Icon Aliasing..."
ln -sf io.strixwolf.alatus.svg /usr/share/icons/hicolor/scalable/apps/alatus-gui.svg 2>/dev/null || true
mkdir -p /usr/share/pixmaps 2>/dev/null || true
ln -sf /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg /usr/share/pixmaps/io.strixwolf.alatus.svg 2>/dev/null || true
ln -sf /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg /usr/share/pixmaps/alatus-gui.svg 2>/dev/null || true
for size in 32 48 64 128 256; do
    mkdir -p "/usr/share/icons/hicolor/${size}x${size}/apps" 2>/dev/null || true
    ln -sf "io.strixwolf.alatus.png" "/usr/share/icons/hicolor/${size}x${size}/apps/alatus-gui.png" 2>/dev/null || true
done

if [ -x /usr/bin/update-desktop-database ]; then
    /usr/bin/update-desktop-database /usr/share/applications 2>/dev/null || true
fi
if [ -x /usr/bin/gtk-update-icon-cache ]; then
    /usr/bin/gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi

echo "Reloading systemd daemon and starting alatusd.service..."
systemctl daemon-reload
systemctl restart alatusd.service || true
ROOT_EOF

echo "=== 5. Finalizing User Session ==="
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental 2>/dev/null || true
fi
systemctl --user daemon-reload || true
systemctl --user restart alatus-session.service || true

echo "=== 6. Verifying Service Status ==="
systemctl is-active alatusd.service || true
systemctl --user is-active alatus-session.service || true
echo "Deployment completed successfully."
