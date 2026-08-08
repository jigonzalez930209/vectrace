#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${REPO_ROOT}/target/dist"

VERSION="$(
  awk '
    /^\[package\]/ { in_pkg=1; next }
    /^\[/ { in_pkg=0 }
    in_pkg && /^version[[:space:]]*=/ {
      if (match($0, /"[^"]+"/)) {
        print substr($0, RSTART+1, RLENGTH-2)
        exit
      }
    }
  ' "${REPO_ROOT}/Cargo.toml"
)"
[[ -n "${VERSION}" ]] || { echo "error: could not read version from Cargo.toml" >&2; exit 1; }

echo "=== Packaging Vectrace ${VERSION} ==="
mkdir -p "${DIST_DIR}"

echo "[1/4] Building standalone binary (tar.gz)..."
cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"
TAR_FILE="${DIST_DIR}/vectrace-v${VERSION}-x86_64-linux.tar.gz"
tar -czf "${TAR_FILE}" -C "${REPO_ROOT}/target/release" vectrace
echo "-> Created ${TAR_FILE}"

echo "[2/4] Building AppImage..."
bash "${REPO_ROOT}/packaging/appimage/build-appimage.sh"
cp "${REPO_ROOT}/target/Vectrace-x86_64.AppImage" "${DIST_DIR}/Vectrace-v${VERSION}-x86_64.AppImage"
echo "-> Created ${DIST_DIR}/Vectrace-v${VERSION}-x86_64.AppImage"

if command -v cargo-deb &> /dev/null; then
    echo "[3/4] Building Debian package (.deb)..."
    cargo deb --output "${DIST_DIR}/vectrace_${VERSION}_amd64.deb"
    echo "-> Created ${DIST_DIR}/vectrace_${VERSION}_amd64.deb"
else
    echo "[3/4] cargo-deb not found, skipping .deb generation"
fi

if command -v cargo-generate-rpm &> /dev/null; then
    echo "[4/4] Building RPM package (.rpm)..."
    cargo generate-rpm --output "${DIST_DIR}/vectrace-${VERSION}-1.x86_64.rpm"
    echo "-> Created ${DIST_DIR}/vectrace-${VERSION}-1.x86_64.rpm"
else
    echo "[4/4] cargo-generate-rpm not found, skipping .rpm generation"
fi

echo "=== Release Build Artifacts Ready in ${DIST_DIR} ==="
ls -lh "${DIST_DIR}"
