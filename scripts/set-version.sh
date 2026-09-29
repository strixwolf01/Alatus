#!/usr/bin/env bash
# scripts/set-version.sh - Set and synchronize the version across Alatus packages and Cargo workspace
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ $# -ne 1 ]; then
    echo "Usage: $0 <version> (e.g. 0.1.0)" >&2
    exit 1
fi

NEW_VERSION="$1"

# Validate version format XX.XX.XX (semantic versioning: numbers separated by dots)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version '$NEW_VERSION' does not match format XX.XX.XX (e.g. 0.1.0)" >&2
    exit 1
fi

echo "==> Setting Alatus version to $NEW_VERSION"

# 1. Update Cargo.toml workspace.package version
CARGO_TOML="$REPO_ROOT/Cargo.toml"
if [ -f "$CARGO_TOML" ]; then
    sed -i -E "s/^(version = \")[^\"]+(\")/\1$NEW_VERSION\2/" "$CARGO_TOML"
    echo "  [x] Updated $CARGO_TOML"
fi

# 2. Update Arch Linux PKGBUILD
PKGBUILD="$REPO_ROOT/packaging/arch/PKGBUILD"
if [ -f "$PKGBUILD" ]; then
    sed -i -E "s/^pkgver=[0-9]+\.[0-9]+\.[0-9]+/pkgver=$NEW_VERSION/" "$PKGBUILD"
    sed -i -E "s/^pkgrel=[0-9]+/pkgrel=1/" "$PKGBUILD"
    echo "  [x] Updated $PKGBUILD"
fi

# 3. Update RPM spec
RPM_SPEC="$REPO_ROOT/packaging/rpm/alatus.spec"
if [ -f "$RPM_SPEC" ]; then
    sed -i -E "s/^(Version:[[:space:]]+)[0-9]+\.[0-9]+\.[0-9]+/\1$NEW_VERSION/" "$RPM_SPEC"
    sed -i -E "s/^(Release:[[:space:]]+)[0-9]+/\11/" "$RPM_SPEC"
    echo "  [x] Updated $RPM_SPEC"
fi

# 4. Update Debian changelog
DEB_CHANGELOG="$REPO_ROOT/packaging/deb/debian/changelog"
if [ -f "$DEB_CHANGELOG" ]; then
    DATE_STR=$(date -R)
    NEW_ENTRY="alatus ($NEW_VERSION-1) unstable; urgency=medium

  * Release $NEW_VERSION

 -- StrixWolf <strixwolf@example.com>  $DATE_STR
"
    printf "%s\n%s" "$NEW_ENTRY" "$(cat "$DEB_CHANGELOG")" > "$DEB_CHANGELOG"
    echo "  [x] Updated $DEB_CHANGELOG"
fi

echo "==> Successfully synchronized version $NEW_VERSION"
