#!/usr/bin/env bash
# scripts/build-rpm.sh - Build RPM package for Fedora/RHEL/openSUSE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION=$(grep -m1 '^version = ' "$REPO_ROOT/Cargo.toml" | cut -d '"' -f2)
DIST_DIR="$REPO_ROOT/dist"
RPMBUILD_DIR="$REPO_ROOT/target/rpmbuild"

echo "==> Building RPM package for Alatus v$VERSION..."

# 1. Ensure binaries are built
cargo build --release

# 2. Setup isolated RPM build environment
rm -rf "$RPMBUILD_DIR"
mkdir -p "$RPMBUILD_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
mkdir -p "$DIST_DIR"

# 3. Build RPM package using rpmbuild
rpmbuild -bb \
    --define "_topdir $RPMBUILD_DIR" \
    --define "_rpmdir $DIST_DIR" \
    --define "_sourcedir $REPO_ROOT" \
    --define "_specdir $RPMBUILD_DIR/SPECS" \
    "$REPO_ROOT/packaging/rpm/alatus.spec"

find "$RPMBUILD_DIR/RPMS" -type f -name "*.rpm" -exec cp {} "$DIST_DIR/" \; 2>/dev/null || true
find "$DIST_DIR/x86_64" -type f -name "*.rpm" -exec cp {} "$DIST_DIR/" \; 2>/dev/null || true

echo "==> RPM package generated successfully:"
find "$DIST_DIR" -maxdepth 1 -type f -name "*.rpm"
