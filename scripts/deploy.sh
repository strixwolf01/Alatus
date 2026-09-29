#!/usr/bin/env bash
# scripts/deploy.sh - Local deploy script for Alatus
# Detects the Linux distribution and installs the native package (RPM, DEB, Arch)
# so the system package manager tracks all files. If the distro is unsupported,
# falls back to clean manual installation.
# Requests root permission at most ONCE (sudo with polkit fallback).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

detect_distro() {
    local family="generic"
    if [ -f /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        local distro="${ID:-unknown}"
        local id_like="${ID_LIKE:-}"

        case "$distro" in
            fedora|rhel|centos|almalinux|rocky|ol|nobara|silverblue|bazzite|opensuse*|sles)
                family="rpm"
                ;;
            debian|ubuntu|pop|mint|linuxmint|elementary|kali|raspbian|zorin)
                family="deb"
                ;;
            arch|manjaro|endeavouros|garuda|artix)
                family="arch"
                ;;
            *)
                for like in $id_like; do
                    case "$like" in
                        fedora|rhel|centos|suse)
                            family="rpm"
                            break
                            ;;
                        debian|ubuntu)
                            family="deb"
                            break
                            ;;
                        arch)
                            family="arch"
                            break
                            ;;
                    esac
                done
                ;;
        esac
    fi

    # Secondary heuristic: check available package managers
    if [ "$family" = "generic" ]; then
        if command -v rpm >/dev/null 2>&1 && (command -v dnf >/dev/null 2>&1 || command -v rpmbuild >/dev/null 2>&1); then
            family="rpm"
        elif command -v dpkg >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
            family="deb"
        elif command -v pacman >/dev/null 2>&1; then
            family="arch"
        fi
    fi

    echo "$family"
}

# ==============================================================================
# Privileged Worker Phase (Executes as root once elevated)
# ==============================================================================
if [ "${1:-}" = "--privileged-worker" ]; then
    shift
    SRC_DIR="${1:-$REPO_ROOT}"
    FAMILY="${2:-generic}"
    PKG_PATH="${3:-manual}"

    if [ "$EUID" -ne 0 ]; then
        echo "Error: Privileged worker must be run as root." >&2
        exit 1
    fi

    echo "==> [Root Task] Stopping active services and processes..."
    pkill -x alatusd 2>/dev/null || true
    pkill -x alatus-session 2>/dev/null || true
    pkill -x alatus-gui 2>/dev/null || true

    if [ -d /run/systemd/system ]; then
        systemctl stop alatusd.service 2>/dev/null || true
        systemctl disable alatusd.service 2>/dev/null || true
        systemctl --global disable alatus-session.service 2>/dev/null || true
    fi

    echo "==> [Root Task] Removing any previously installed conflicting packages..."
    if command -v rpm >/dev/null 2>&1; then
        EXISTING_RPM=$(rpm -qa | grep -i '^alatus' || true)
        if [ -n "$EXISTING_RPM" ]; then
            echo "  -> Removing existing RPM package: $EXISTING_RPM"
            rpm -e --nodeps "$EXISTING_RPM" 2>/dev/null || true
        fi
    fi

    if command -v dpkg >/dev/null 2>&1; then
        if dpkg -s alatus 2>/dev/null | grep -q "Status: install ok installed"; then
            echo "  -> Removing existing Debian package: alatus"
            dpkg -P alatus 2>/dev/null || true
        fi
    fi

    echo "==> [Root Task] Thoroughly purging legacy and leftover files..."
    rm -f /usr/bin/alatus /usr/bin/alatusd /usr/bin/alatus-session /usr/bin/alatus-gui /usr/bin/alatus-cli
    rm -f /usr/local/bin/alatus /usr/local/bin/alatusd /usr/local/bin/alatus-session /usr/local/bin/alatus-gui /usr/local/bin/alatus-cli
    rm -f /usr/lib/systemd/system/alatusd.service /etc/systemd/system/alatusd.service
    rm -f /usr/lib/systemd/system/multi-user.target.wants/alatusd.service
    rm -f /etc/systemd/system/multi-user.target.wants/alatusd.service
    rm -f /usr/lib/systemd/user/alatus-session.service /etc/systemd/user/alatus-session.service
    rm -f /usr/lib/systemd/user/default.target.wants/alatus-session.service
    rm -f /usr/lib/systemd/user/graphical-session.target.wants/alatus-session.service
    rm -f /etc/systemd/user/default.target.wants/alatus-session.service
    rm -f /etc/systemd/user/graphical-session.target.wants/alatus-session.service
    rm -f /usr/share/dbus-1/system.d/org.alatus.Daemon.conf /etc/dbus-1/system.d/org.alatus.Daemon.conf
    rm -f /usr/share/dbus-1/system.d/io.strixwolf.alatus.Daemon.conf /etc/dbus-1/system.d/io.strixwolf.alatus.Daemon.conf
    rm -f /usr/share/polkit-1/actions/org.alatus.policy
    rm -f /usr/share/polkit-1/actions/io.strixwolf.alatus.policy
    rm -f /usr/share/applications/org.alatus.gui.desktop
    rm -f /usr/share/applications/io.strixwolf.alatus.desktop
    rm -f /etc/xdg/autostart/io.strixwolf.alatus.desktop
    rm -f /usr/lib/udev/rules.d/99-alatus-rgb.rules
    rm -rf /usr/share/alatus
    rm -rf /var/lib/alatus
    rm -f /usr/share/pixmaps/alatus-gui.svg /usr/share/pixmaps/io.strixwolf.alatus.svg
    rm -f /usr/share/icons/hicolor/scalable/apps/io.strixwolf.alatus.svg
    rm -f /usr/share/icons/hicolor/scalable/apps/alatus-gui.svg
    rm -f /usr/share/icons/hicolor/scalable/apps/alatus-mode-*.svg
    for s in 32 48 64 128 256; do
        rm -f "/usr/share/icons/hicolor/${s}x${s}/apps/io.strixwolf.alatus.png"
        rm -f "/usr/share/icons/hicolor/${s}x${s}/apps/alatus-gui.png"
    done
    rm -f /usr/share/bash-completion/completions/alatus
    rm -f /usr/share/fish/vendor_completions.d/alatus.fish
    rm -f /usr/share/zsh/site-functions/_alatus
    rm -f /usr/share/metainfo/io.strixwolf.alatus.metainfo.xml

    # Installation phase based on detected distro packaging
    case "$FAMILY" in
        rpm)
            echo "==> [Root Task] Installing native RPM package via package manager..."
            echo "    Package: $PKG_PATH"
            if command -v dnf >/dev/null 2>&1; then
                dnf install -y --allowerasing "$PKG_PATH" || rpm -Uvh --replacepkgs --nodeps "$PKG_PATH"
            elif command -v zypper >/dev/null 2>&1; then
                zypper --non-interactive install --allow-unsigned-rpm "$PKG_PATH" || rpm -Uvh --replacepkgs --nodeps "$PKG_PATH"
            else
                rpm -Uvh --replacepkgs --nodeps "$PKG_PATH"
            fi
            ;;
        deb)
            echo "==> [Root Task] Installing native DEB package via dpkg..."
            echo "    Package: $PKG_PATH"
            dpkg -i "$PKG_PATH" || (command -v apt-get >/dev/null 2>&1 && apt-get install -f -y)
            ;;
        arch)
            echo "==> [Root Task] Installing native Arch package via pacman..."
            echo "    Package: $PKG_PATH"
            pacman -U --noconfirm "$PKG_PATH"
            ;;
        generic|*)
            echo "==> [Root Task] Distro not supported for native packaging; installing files manually to /usr..."
            make -C "$SRC_DIR" install PREFIX=/usr
            ;;
    esac

    echo "==> [Root Task] Ensuring system services are active..."
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

    echo "==> [Root Task] Privileged installation completed successfully!"
    exit 0
fi

# ==============================================================================
# Main Unprivileged Phase (Runs as current user)
# ==============================================================================
VERSION=$(grep -m1 '^version = ' "$REPO_ROOT/Cargo.toml" | cut -d '"' -f2)
FAMILY=$(detect_distro)

echo "=========================================================="
echo "          Alatus v$VERSION Local Deployment"
echo "=========================================================="
echo "Detected distribution packaging family: [$FAMILY]"

PKG_PATH="manual"
case "$FAMILY" in
    rpm)
        echo "==> [Step 1/3] Preparing native RPM package..."
        "$REPO_ROOT/scripts/build-rpm.sh"
        PKG_PATH=$(find "$REPO_ROOT/dist" -maxdepth 1 -name "alatus-*.rpm" | head -n1)
        if [ -z "$PKG_PATH" ] || [ ! -f "$PKG_PATH" ]; then
            echo "Warning: RPM build did not produce a package in dist/. Falling back to manual install."
            FAMILY="generic"
            cargo build --release
            PKG_PATH="manual"
        else
            echo "  -> RPM ready: $PKG_PATH"
        fi
        ;;
    deb)
        echo "==> [Step 1/3] Preparing native DEB package..."
        "$REPO_ROOT/scripts/build-deb.sh"
        PKG_PATH=$(find "$REPO_ROOT/dist" -maxdepth 1 -name "alatus_*.deb" | head -n1)
        if [ -z "$PKG_PATH" ] || [ ! -f "$PKG_PATH" ]; then
            echo "Warning: DEB build did not produce a package in dist/. Falling back to manual install."
            FAMILY="generic"
            cargo build --release
            PKG_PATH="manual"
        else
            echo "  -> DEB ready: $PKG_PATH"
        fi
        ;;
    arch)
        if command -v makepkg >/dev/null 2>&1; then
            echo "==> [Step 1/3] Preparing native Arch package..."
            (cd "$REPO_ROOT/packaging/arch" && makepkg -f --nodeps --clean)
            PKG_PATH=$(find "$REPO_ROOT/packaging/arch" -name "alatus-*.pkg.tar.zst" | head -n1)
        fi
        if [ -z "$PKG_PATH" ] || [ "$PKG_PATH" = "manual" ] || [ ! -f "$PKG_PATH" ]; then
            echo "Note: Arch makepkg unavailable or skipped; building release binaries for manual install."
            FAMILY="generic"
            cargo build --release
            PKG_PATH="manual"
        else
            echo "  -> Arch package ready: $PKG_PATH"
        fi
        ;;
    generic|*)
        echo "==> [Step 1/3] Distro not supported for native packaging; building release binaries..."
        cargo build --release
        FAMILY="generic"
        PKG_PATH="manual"
        ;;
esac

echo "==> [Step 2/3] Requesting root permission once for installation..."
if [ "$EUID" -eq 0 ]; then
    echo "  -> Running directly as root."
    "$REPO_ROOT/scripts/deploy.sh" --privileged-worker "$REPO_ROOT" "$FAMILY" "$PKG_PATH"
elif command -v sudo >/dev/null 2>&1 && sudo -v; then
    echo "  -> Elevated via sudo."
    sudo "$REPO_ROOT/scripts/deploy.sh" --privileged-worker "$REPO_ROOT" "$FAMILY" "$PKG_PATH"
elif command -v pkexec >/dev/null 2>&1; then
    echo "  -> sudo not permitted/available; elevating via PolicyKit (pkexec)..."
    pkexec "$REPO_ROOT/scripts/deploy.sh" --privileged-worker "$REPO_ROOT" "$FAMILY" "$PKG_PATH"
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
echo "       Alatus v$VERSION Successfully Deployed ($FAMILY)!"
echo "=========================================================="
if [ "$FAMILY" = "rpm" ]; then
    echo "Package status: $(rpm -q alatus)"
elif [ "$FAMILY" = "deb" ]; then
    echo "Package status: $(dpkg -s alatus | grep '^Status:')"
elif [ "$FAMILY" = "arch" ]; then
    echo "Package status: $(pacman -Q alatus 2>/dev/null || true)"
fi
echo ""
echo "Binaries verified:"
echo "  - CLI:     $(command -v alatus)"
echo "  - Daemon:  $(command -v alatusd)"
echo "  - Session: $(command -v alatus-session)"
echo "  - GUI:     $(command -v alatus-gui)"
echo ""
echo "Try running:"
echo "  alatus battery status"
echo "  alatus thermal status"
echo "  alatus lighting status"
echo "  alatus-gui"
