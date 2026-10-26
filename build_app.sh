#!/bin/bash
set -e

# Skrip Otomatis Pembundelan Aplikasi macOS (.app)
# Proyek: WinDisk Maker

APP_NAME="WinDisk Maker"
APP_BUNDLE="${APP_NAME}.app"
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_DIR="${PROJECT_DIR}/target/release"
BINARY_NAME="windisk-maker"

echo "=========================================="
echo "⚡ Membuat Bundle Aplikasi macOS: ${APP_NAME}"
echo "=========================================="

# 1. Kompilasi mode release yang teroptimasi
echo "[1/4] Mengompilasi kode dalam mode --release..."
cd "${PROJECT_DIR}"
cargo build --release

# Tentukan target arsitektur lain agar bundle bisa universal (Intel + Apple Silicon).
NATIVE_BINARY="${TARGET_DIR}/${BINARY_NAME}"
OTHER_TARGET=""
case "$(uname -m)" in
    arm64) OTHER_TARGET="x86_64-apple-darwin" ;;
    x86_64) OTHER_TARGET="aarch64-apple-darwin" ;;
esac

# 2. Buat struktur folder aplikasi .app
echo "[2/4] Menyiapkan struktur folder .app..."
rm -rf "${PROJECT_DIR}/${APP_BUNDLE}"
mkdir -p "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${PROJECT_DIR}/${APP_BUNDLE}/Contents/Resources"

# 3. Salin executable biner (universal jika target arsitektur lain tersedia)
echo "[3/4] Menyalin binary executable..."
if [ -f "${NATIVE_BINARY}" ]; then
    if [ -n "${OTHER_TARGET}" ] && command -v rustup >/dev/null 2>&1 \
        && rustup target list --installed 2>/dev/null | grep -qx "${OTHER_TARGET}" \
        && command -v lipo >/dev/null 2>&1; then
        echo "Membangun binary universal (${OTHER_TARGET})..."
        cargo build --release --target "${OTHER_TARGET}"
        OTHER_BINARY="${PROJECT_DIR}/target/${OTHER_TARGET}/release/${BINARY_NAME}"
        if [ -f "${OTHER_BINARY}" ]; then
            lipo -create "${NATIVE_BINARY}" "${OTHER_BINARY}" -output "${TARGET_DIR}/${BINARY_NAME}-universal"
            cp "${TARGET_DIR}/${BINARY_NAME}-universal" "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS/windisk-maker"
        else
            echo "Peringatan: binary ${OTHER_TARGET} tidak ditemukan; memakai binary native."
            cp "${NATIVE_BINARY}" "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS/windisk-maker"
        fi
    else
        echo "Peringatan: target ${OTHER_TARGET:-lain} belum terpasang; app hanya untuk $(uname -m)."
        cp "${NATIVE_BINARY}" "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS/windisk-maker"
    fi
elif [ -f "${TARGET_DIR}/win-rufus-mac" ]; then
    cp "${TARGET_DIR}/win-rufus-mac" "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS/windisk-maker"
else
    echo "Error: binary windisk-maker tidak ditemukan setelah proses build."
    exit 1
fi
chmod +x "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS/windisk-maker"

# 4. Pasang icon dan Info.plist
echo "[4/4] Memasang Info.plist dan AppIcon..."
if [ -f "${PROJECT_DIR}/resources/AppIcon.icns" ]; then
    cp "${PROJECT_DIR}/resources/AppIcon.icns" "${PROJECT_DIR}/${APP_BUNDLE}/Contents/Resources/AppIcon.icns"
fi

cat << 'PLIST' > "${PROJECT_DIR}/${APP_BUNDLE}/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>windisk-maker</string>
    <key>CFBundleIdentifier</key>
    <string>com.windisk.maker</string>
    <key>CFBundleName</key>
    <string>WinDisk Maker</string>
    <key>CFBundleDisplayName</key>
    <string>WinDisk Maker</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.3.0</string>
    <key>CFBundleVersion</key>
    <string>1.3.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

# Tanda tangani secara ad-hoc agar binary universal dapat dijalankan di Apple Silicon.
if command -v codesign >/dev/null 2>&1; then
    if ! codesign --force --deep --sign - "${PROJECT_DIR}/${APP_BUNDLE}" >/dev/null 2>&1; then
        echo "Peringatan: gagal menandatangani bundle secara ad-hoc."
    fi
fi

# Salin otomatis ke /Applications jika ada
if [ -d "/Applications" ]; then
    echo "Memperbarui /Applications/${APP_BUNDLE}..."
    rm -rf "/Applications/${APP_BUNDLE}"
    cp -r "${PROJECT_DIR}/${APP_BUNDLE}" "/Applications/${APP_BUNDLE}"
fi

echo "=========================================="
echo "✅ SUKSES! Aplikasi telah selesai dibuat dan diperbarui di:"
echo "👉 ${PROJECT_DIR}/${APP_BUNDLE}"
echo "👉 /Applications/${APP_BUNDLE}"
echo "=========================================="
