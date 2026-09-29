#!/usr/bin/env bash
# scripts/build-deb.sh - Build Debian/Ubuntu DEB package
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION=$(grep -m1 '^version = ' "$REPO_ROOT/Cargo.toml" | cut -d '"' -f2)
ARCH="amd64"
PKG_NAME="alatus_${VERSION}-1_${ARCH}"
BUILD_DIR="$REPO_ROOT/target/deb/$PKG_NAME"
DIST_DIR="$REPO_ROOT/dist"

echo "==> Building Debian package for Alatus v$VERSION ($ARCH)..."

# 1. Ensure binaries are built
cargo build --release

# 2. Setup staging directory
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/DEBIAN"
mkdir -p "$DIST_DIR"

# 3. Install files into staging directory
make install DESTDIR="$BUILD_DIR" PREFIX=/usr

# 4. Generate binary DEBIAN control and maintainer scripts
cat << EOF > "$BUILD_DIR/DEBIAN/control"
Package: alatus
Version: ${VERSION}-1
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: StrixWolf <strixwolf@example.com>
Depends: systemd, dbus, policykit-1 | polkitd, hicolor-icon-theme
Homepage: https://github.com/strixwolf01/Alatus
Description: Linux hardware control suite for ASUS laptops
 Alatus is a modular Linux hardware control suite for ASUS laptops.
 It provides fine-grained control over battery charging thresholds,
 thermal fan profiles, keyboard RGB lighting, and ASUS WMI hotkeys.
EOF

cat << 'EOF' > "$BUILD_DIR/DEBIAN/postinst"
#!/bin/sh
set -e

if [ "$1" = "configure" ]; then
    systemctl daemon-reload >/dev/null 2>&1 || true
    systemctl reload dbus.service >/dev/null 2>&1 || true
    systemctl enable --now alatusd.service >/dev/null 2>&1 || true
    systemctl --global enable alatus-session.service >/dev/null 2>&1 || true
    if [ -x /usr/bin/gtk-update-icon-cache ]; then
        gtk-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
    fi
    if [ -x /usr/bin/update-desktop-database ]; then
        update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
    fi
fi
exit 0
EOF

cat << 'EOF' > "$BUILD_DIR/DEBIAN/prerm"
#!/bin/sh
set -e

if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    systemctl disable --now alatusd.service >/dev/null 2>&1 || true
    systemctl --global disable alatus-session.service >/dev/null 2>&1 || true
fi
exit 0
EOF

chmod 755 "$BUILD_DIR/DEBIAN"
chmod 644 "$BUILD_DIR/DEBIAN/control"
chmod 755 "$BUILD_DIR/DEBIAN/postinst" "$BUILD_DIR/DEBIAN/prerm"

# 5. Build package with dpkg-deb
dpkg-deb --build --root-owner-group "$BUILD_DIR" "$DIST_DIR/${PKG_NAME}.deb"

echo "==> Debian package generated at: $DIST_DIR/${PKG_NAME}.deb"
