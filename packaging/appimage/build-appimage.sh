#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APPDIR="${REPO_ROOT}/target/AppDir"

echo "=== Building Vectrace Release Binary ==="
cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"

echo "=== Preparing AppDir Structure ==="
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/scalable/apps"

cp "${REPO_ROOT}/target/release/vectrace" "${APPDIR}/usr/bin/"
cp "${REPO_ROOT}/assets/com.vectrace.Vectrace.desktop" "${APPDIR}/usr/share/applications/"
cp "${REPO_ROOT}/assets/com.vectrace.Vectrace.desktop" "${APPDIR}/vectrace.desktop"
cp "${REPO_ROOT}/assets/vectrace.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/"
cp "${REPO_ROOT}/assets/vectrace.svg" "${APPDIR}/vectrace.svg"
cp "${REPO_ROOT}/assets/vectrace.svg" "${APPDIR}/.DirIcon"
cp "${REPO_ROOT}/packaging/appimage/AppRun" "${APPDIR}/AppRun"

chmod +x "${APPDIR}/AppRun"
chmod +x "${APPDIR}/usr/bin/vectrace"

echo "=== Generating AppImage ==="
if ! command -v appimagetool &> /dev/null; then
    echo "Downloading appimagetool..."
    wget -q "https://github.com/AppImage/AppImageKit/releases/download/13/appimagetool-x86_64.AppImage" -O /tmp/appimagetool
    chmod +x /tmp/appimagetool
    APPIMAGETOOL="/tmp/appimagetool"
else
    APPIMAGETOOL="appimagetool"
fi

ARCH=x86_64 "${APPIMAGETOOL}" "${APPDIR}" "${REPO_ROOT}/target/Vectrace-x86_64.AppImage"

echo "=== AppImage successfully created at target/Vectrace-x86_64.AppImage ==="
