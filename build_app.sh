#!/bin/bash
set -e

# Skrip Otomatis Pembundelan Aplikasi macOS (.app)
# Proyek: WinDisk Maker

APP_NAME="WinDisk Maker"
APP_BUNDLE="${APP_NAME}.app"
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_DIR="${PROJECT_DIR}/target/release"
BINARY_NAME="win-rufus-mac"

echo "=========================================="
echo "⚡ Membuat Bundle Aplikasi macOS: ${APP_NAME}"
echo "=========================================="

# 1. Kompilasi mode release yang teroptimasi
echo "[1/4] Mengompilasi kode dalam mode --release..."
cd "${PROJECT_DIR}"
cargo build --release

# 2. Buat struktur folder aplikasi .app
echo "[2/4] Menyiapkan struktur folder .app..."
rm -rf "${PROJECT_DIR}/${APP_BUNDLE}"
mkdir -p "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${PROJECT_DIR}/${APP_BUNDLE}/Contents/Resources"

# 3. Salin executable biner
echo "[3/4] Menyalin binary executable..."
cp "${TARGET_DIR}/${BINARY_NAME}" "${PROJECT_DIR}/${APP_BUNDLE}/Contents/MacOS/windisk-maker"
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
    <string>1.2.0</string>
    <key>CFBundleVersion</key>
    <string>1.2.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

echo "=========================================="
echo "✅ SUKSES! Aplikasi telah selesai dibuat:"
echo "👉 ${PROJECT_DIR}/${APP_BUNDLE}"
echo "=========================================="
echo "💡 Anda bisa langsung mengklik ganda ${APP_BUNDLE}"
echo "   atau memindahkannya ke folder /Applications!"
