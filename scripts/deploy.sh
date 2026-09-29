#!/usr/bin/env bash
# scripts/deploy.sh - Local deploy script for Alatus
# Cleans all leftovers from legacy/previous versions, uninstalls any installed version,
# builds and installs the new clean suite, requesting root permission at most ONCE (sudo with polkit fallback).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ==============================================================================
# Privileged Worker Phase (Executes as root once elevated)
# ==============================================================================
if [ "${1:-}" = "--privileged-worker" ]; then
    shift
    SRC_DIR="${1:-$REPO_ROOT}"

    if [ "$EUID" -ne 0 ]; then
        echo "Error: Privileged worker must be run as root." >&2
        exit 1
    fi

    echo "==> [Root Task] Stopping active services and processes..."
    # Terminate running Alatus processes
    pkill -x alatusd 2>/dev/null || true
    pkill -x alatus-session 2>/dev/null || true
    pkill -x alatus-gui 2>/dev/null || true

    if [ -d /run/systemd/system ]; then
        systemctl stop alatusd.service 2>/dev/null || true
        systemctl disable alatusd.service 2>/dev/null || true
        systemctl --global disable alatus-session.service 2>/dev/null || true
    fi

    echo "==> [Root Task] Removing any previously installed package versions..."
    # 1. RPM package removal if present
    if command -v rpm >/dev/null 2>&1; then
        EXISTING_RPM=$(rpm -qa | grep -i '^alatus' || true)
        if [ -n "$EXISTING_RPM" ]; then
            echo "  -> Removing installed RPM package: $EXISTING_RPM"
            rpm -e --nodeps "$EXISTING_RPM" 2>/dev/null || true
        fi
    fi

    # 2. Debian package removal if present
    if command -v dpkg >/dev/null 2>&1; then
        if dpkg -s alatus 2>/dev/null | grep -q "Status: install ok installed"; then
            echo "  -> Removing installed Debian package: alatus"
            dpkg -P alatus 2>/dev/null || true
        fi
    fi

    echo "==> [Root Task] Thoroughly purging legacy and leftover files..."
    # Binaries
    rm -f /usr/bin/alatus /usr/bin/alatusd /usr/bin/alatus-session /usr/bin/alatus-gui /usr/bin/alatus-cli
    rm -f /usr/local/bin/alatus /usr/local/bin/alatusd /usr/local/bin/alatus-session /usr/local/bin/alatus-gui /usr/local/bin/alatus-cli

    # Systemd units (system & user)
    rm -f /usr/lib/systemd/system/alatusd.service /etc/systemd/system/alatusd.service
    rm -f /usr/lib/systemd/system/multi-user.target.wants/alatusd.service
    rm -f /etc/systemd/system/multi-user.target.wants/alatusd.service
    rm -f /usr/lib/systemd/user/alatus-session.service /etc/systemd/user/alatus-session.service
    rm -f /usr/lib/systemd/user/default.target.wants/alatus-session.service
    rm -f /usr/lib/systemd/user/graphical-session.target.wants/alatus-session.service
    rm -f /etc/systemd/user/default.target.wants/alatus-session.service
    rm -f /etc/systemd/user/graphical-session.target.wants/alatus-session.service

    # D-Bus & Polkit
    rm -f /usr/share/dbus-1/system.d/org.alatus.Daemon.conf /etc/dbus-1/system.d/org.alatus.Daemon.conf
    rm -f /usr/share/dbus-1/system.d/io.strixwolf.alatus.Daemon.conf /etc/dbus-1/system.d/io.strixwolf.alatus.Daemon.conf
    rm -f /usr/share/polkit-1/actions/org.alatus.policy
    rm -f /usr/share/polkit-1/actions/io.strixwolf.alatus.policy

    # Desktop, autostart, & udev rules
    rm -f /usr/share/applications/org.alatus.gui.desktop
    rm -f /usr/share/applications/io.strixwolf.alatus.desktop
    rm -f /etc/xdg/autostart/io.strixwolf.alatus.desktop
    rm -f /usr/lib/udev/rules.d/99-alatus-rgb.rules

    # Data directories
    rm -rf /usr/share/alatus
    rm -rf /var/lib/alatus

    # Legacy icons & pixmaps
    rm -f /usr/share/pixmaps/alatus-gui.svg /usr/share/pixmaps/io.strixwolf.alatus.svg
    rm -f /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg
    rm -f /usr/share/icons/hicolor/scalable/apps/alatus-gui.svg
    rm -f /usr/share/icons/hicolor/scalable/apps/alatus-mode-*.svg
    for s in 32 48 64 128 256; do
        rm -f "/usr/share/icons/hicolor/${s}x${s}/apps/io.strixwolf.alatus.png"
        rm -f "/usr/share/icons/hicolor/${s}x${s}/apps/alatus-gui.png"
    done

    # Completions & metainfo
    rm -f /usr/share/bash-completion/completions/alatus
    rm -f /usr/share/fish/vendor_completions.d/alatus.fish
    rm -f /usr/share/zsh/site-functions/_alatus
    rm -f /usr/share/metainfo/io.strixwolf.alatus.metainfo.xml

    echo "==> [Root Task] Installing freshly built Alatus suite to /usr..."
    make -C "$SRC_DIR" install PREFIX=/usr

    echo "==> [Root Task] Reloading system services..."
    if [ -d /run/systemd/system ]; then
        systemctl daemon-reload || true
        systemctl reload dbus.service 2>/dev/null || killall -HUP dbus-daemon 2>/dev/null || true
        systemctl enable --now alatusd.service || true
    fi

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
    fi

    echo "==> [Root Task] Privileged installation complete!"
    exit 0
fi

# ==============================================================================
# Main Unprivileged Phase (Runs as current user)
# ==============================================================================
VERSION=$(grep -m1 '^version = ' "$REPO_ROOT/Cargo.toml" | cut -d '"' -f2)
echo "=========================================================="
echo "          Alatus v$VERSION Local Deployment"
echo "=========================================================="

echo "==> [Step 1/3] Building release binaries as $(whoami)..."
cargo build --release

echo "==> [Step 2/3] Checking root elevation method (requesting once)..."
if [ "$EUID" -eq 0 ]; then
    echo "  -> Running directly as root."
    "$REPO_ROOT/scripts/deploy.sh" --privileged-worker "$REPO_ROOT"
elif command -v sudo >/dev/null 2>&1 && sudo -v; then
    echo "  -> Elevated via sudo."
    sudo "$REPO_ROOT/scripts/deploy.sh" --privileged-worker "$REPO_ROOT"
elif command -v pkexec >/dev/null 2>&1; then
    echo "  -> sudo not permitted/available; elevating via PolicyKit (pkexec)..."
    pkexec "$REPO_ROOT/scripts/deploy.sh" --privileged-worker "$REPO_ROOT"
else
    echo "Error: Neither sudo nor pkexec is available. Root privileges are required to install system services." >&2
    exit 1
fi

echo "==> [Step 3/3] Setting up user session agent..."
if [ -n "${XDG_RUNTIME_DIR:-}" ] && [ -d "/run/user/$UID" ]; then
    systemctl --user daemon-reload 2>/dev/null || true
    systemctl --user enable --now alatus-session.service 2>/dev/null || true
fi

echo "=========================================================="
echo "       Alatus v$VERSION Successfully Deployed!"
echo "=========================================================="
echo "Binaries installed:"
echo "  - CLI:     /usr/bin/alatus"
echo "  - Daemon:  /usr/bin/alatusd"
echo "  - Session: /usr/bin/alatus-session"
echo "  - GUI:     /usr/bin/alatus-gui"
echo ""
echo "Try running:"
echo "  alatus battery status"
echo "  alatus thermal status"
echo "  alatus lighting status"
echo "  alatus-gui"
