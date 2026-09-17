#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Alatus Contributors
# Universal OS detection, package builder, and single-elevation local deployment script

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

detect_package_format() {
    if [[ -n "${FORCE_PKG_FORMAT:-}" ]]; then
        echo "${FORCE_PKG_FORMAT}"
        return 0
    fi

    if [[ -f /etc/os-release ]]; then
        # shellcheck disable=SC1091
        source /etc/os-release
        local os_id="${ID:-}"
        local os_like="${ID_LIKE:-}"
        local os_info="${os_id} ${os_like}"

        case "${os_info}" in
            *fedora*|*rhel*|*centos*|*rocky*|*almalinux*|*suse*|*opensuse*|*amzn*)
                echo "rpm"
                return 0
                ;;
            *debian*|*ubuntu*|*pop*|*mint*|*kali*|*raspbian*|*elementary*|*zorin*)
                echo "deb"
                return 0
                ;;
        esac
    fi

    # Fallback to detected package managers
    if command -v rpm >/dev/null 2>&1 && (command -v dnf >/dev/null 2>&1 || command -v zypper >/dev/null 2>&1 || command -v rpm-ostree >/dev/null 2>&1); then
        echo "rpm"
        return 0
    elif command -v dpkg >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
        echo "deb"
        return 0
    fi

    echo "source"
    return 0
}

# Parse optional command line flags
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rpm)
            FORCE_PKG_FORMAT="rpm"
            shift
            ;;
        --deb)
            FORCE_PKG_FORMAT="deb"
            shift
            ;;
        --source)
            FORCE_PKG_FORMAT="source"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--rpm | --deb | --source]"
            echo "Auto-detects host OS and builds/installs the compatible package format."
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            echo "Usage: $0 [--rpm | --deb | --source]" >&2
            exit 1
            ;;
    esac
done

PKG_FORMAT="$(detect_package_format)"
echo "=== Detected target package format: ${PKG_FORMAT} ==="

cd "${WORKSPACE_ROOT}"

echo "=== 1. Building Release Binaries with GUI Feature ==="
cargo build --release --bin alatus --bin alatusd --bin alatus-session --features gui

PKG_FILE=""
if [[ "${PKG_FORMAT}" == "rpm" ]]; then
    echo "=== 2. Generating RPM Package ==="
    cargo generate-rpm
    PKG_FILE="$(find "${WORKSPACE_ROOT}/target/generate-rpm" -maxdepth 1 -name '*.rpm' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -f2- -d' ' || true)"
    if [[ -z "${PKG_FILE}" || ! -f "${PKG_FILE}" ]]; then
        echo "Error: Generated RPM package not found under target/generate-rpm/" >&2
        exit 1
    fi
    echo "Found generated RPM: ${PKG_FILE}"
elif [[ "${PKG_FORMAT}" == "deb" ]]; then
    echo "=== 2. Generating DEB Package ==="
    cargo deb --no-build
    PKG_FILE="$(find "${WORKSPACE_ROOT}/target/debian" -maxdepth 1 -name '*.deb' -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -f2- -d' ' || true)"
    if [[ -z "${PKG_FILE}" || ! -f "${PKG_FILE}" ]]; then
        echo "Error: Generated DEB package not found under target/debian/" >&2
        exit 1
    fi
    echo "Found generated DEB: ${PKG_FILE}"
else
    echo "=== 2. Source/Direct Install Mode Selected ==="
fi

echo "=== 3. Stopping User Services & Cleaning Stale Desktop Autostart ==="
systemctl --user stop alatus-session.service 2>/dev/null || true
systemctl --user stop ascend-session.service 2>/dev/null || true
pkill -f "alatus gui" 2>/dev/null || true
pkill -f "alatus-gui" 2>/dev/null || true
pkill -f "ascend-gui" 2>/dev/null || true
rm -f "${HOME}/.config/autostart/io.strixwolf.alatus.desktop" "${HOME}/.config/autostart/io.strixwolf.alatus.autostart.desktop" 2>/dev/null || true

echo "=== 4. Root Operations (Single Elevation) ==="
if [[ $EUID -eq 0 ]]; then
    ELEVATE_CMD=""
elif sudo -n true 2>/dev/null; then
    ELEVATE_CMD="sudo"
elif command -v pkexec >/dev/null 2>&1; then
    echo "Elevating privileges via pkexec..."
    ELEVATE_CMD="pkexec"
elif command -v sudo >/dev/null 2>&1; then
    echo "Elevating privileges via sudo (enter password once if prompted)..."
    sudo -v
    ELEVATE_CMD="sudo"
else
    echo "Elevating privileges via pkexec..."
    ELEVATE_CMD="pkexec"
fi

ROOT_SCRIPT="$(mktemp /tmp/alatus_deploy_root.XXXXXX.sh)"
cat << 'ROOT_EOF' > "${ROOT_SCRIPT}"
#!/usr/bin/env bash
set -euo pipefail

PKG_FORMAT="${1:-}"
PKG_FILE="${2:-}"

echo "Stopping system daemons..."
systemctl stop alatusd.service 2>/dev/null || true
systemctl stop ascend-daemon.service 2>/dev/null || true
systemctl stop asusd.service 2>/dev/null || true

# Package Removal
for pkg in alatus ascend ayuz asusctl; do
    if [[ "${PKG_FORMAT}" == "rpm" ]] && rpm -q "${pkg}" >/dev/null 2>&1; then
        echo "Removing existing RPM package: ${pkg}"
        rpm -e "${pkg}" 2>/dev/null || true
    elif [[ "${PKG_FORMAT}" == "deb" ]] && dpkg -s "${pkg}" >/dev/null 2>&1; then
        echo "Removing existing DEB package: ${pkg}"
        dpkg -r "${pkg}" 2>/dev/null || true
    fi
done

# Orphan & Manual Installation Cleanup
for bin in /usr/bin/alatus /usr/bin/alatusd /usr/bin/alatus-session /usr/bin/alatus-gui /usr/bin/ascend /usr/bin/ascend-daemon /usr/bin/ascend-session /usr/bin/ascend-gui /usr/bin/ayuz; do
    if [[ -f "${bin}" || -L "${bin}" ]]; then
        echo "Removing manual binary remnant: ${bin}"
        rm -f "${bin}"
    fi
done

for desktop in /usr/share/applications/io.strixwolf.alatus.desktop /usr/share/applications/alatus-gui.desktop /usr/share/applications/io.strixwolf.ascend.desktop /usr/share/applications/ascend-gui.desktop /usr/share/applications/ascend.desktop /usr/share/applications/io.strixwolf.ayuz.desktop /etc/xdg/autostart/io.strixwolf.alatus.desktop /etc/xdg/autostart/io.strixwolf.alatus.autostart.desktop; do
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

for unit in /usr/lib/systemd/system/alatusd.service /usr/lib/systemd/user/alatus-session.service /usr/lib/systemd/user/default.target.wants/alatus-session.service /usr/lib/systemd/user/graphical-session.target.wants/alatus-session.service /usr/lib/systemd/system/ascend-daemon.service /usr/lib/systemd/user/ascend-session.service /usr/lib/systemd/user/default.target.wants/ascend-session.service; do
    if [[ -f "${unit}" || -L "${unit}" ]]; then
        echo "Removing unit remnant: ${unit}"
        rm -f "${unit}"
    fi
done

# Package Installation
if [[ "${PKG_FORMAT}" == "rpm" ]]; then
    echo "Installing RPM: ${PKG_FILE}"
    rpm -Uvh --replacepkgs "${PKG_FILE}"
elif [[ "${PKG_FORMAT}" == "deb" ]]; then
    echo "Installing DEB: ${PKG_FILE}"
    dpkg -i "${PKG_FILE}" 2>/dev/null || apt-get install -y -f "${PKG_FILE}"
else
    echo "Installing via direct Makefile targets..."
    make -C "${WORKSPACE_ROOT}" install
    if ! getent group alatus-rgb >/dev/null; then
        groupadd -r alatus-rgb 2>/dev/null || true
    fi
    if [ -x /usr/bin/udevadm ]; then
        udevadm control --reload-rules || true
        udevadm trigger --subsystem-match=hidraw || true
    fi
    systemctl daemon-reload || true
    systemctl enable --now alatusd.service >/dev/null 2>&1 || true
    systemctl --global enable alatus-session.service >/dev/null 2>&1 || true
fi

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

chmod 755 "${ROOT_SCRIPT}"
${ELEVATE_CMD} "${ROOT_SCRIPT}" "${PKG_FORMAT}" "${PKG_FILE}"
rm -f "${ROOT_SCRIPT}"

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

