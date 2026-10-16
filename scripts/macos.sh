#!/usr/bin/env bash
# Builds Aural for macOS and packages build/macos/Aural.app and build/Aural.dmg.
#
#   ./scripts/macos.sh              # this Mac's architecture
#   ./scripts/macos.sh --universal  # Apple silicon and Intel in one binary
#   ./scripts/macos.sh --install    # also copies Aural.app to /Applications
#
#   AURAL_BUNDLE_ID   bundle id (default dev.aural.app)
#   AURAL_IDENTITY    a "Developer ID Application" identity to sign with; ad hoc otherwise,
#                     which runs on this Mac but Gatekeeper blocks on others
set -euo pipefail
cd "$(dirname "$0")/.."

UNIVERSAL=0
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --universal) UNIVERSAL=1 ;;
    --install) INSTALL=1 ;;
    *) echo "usage: $0 [--universal] [--install]" >&2; exit 1 ;;
  esac
done

# The workspace's version, and the commit count as the build number, which only ever grows.
VERSION="$(sed -nE 's/^version = "(.*)"/\1/p' Cargo.toml | head -1)"
BUILD="${AURAL_BUILD:-$(git rev-list --count HEAD)}"
BUNDLE_ID="${AURAL_BUNDLE_ID:-dev.aural.app}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

OUT=build/macos
APP="$OUT/Aural.app"
rm -rf "$OUT"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

if [ "$UNIVERSAL" = 1 ]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build -p aural --bin aural --release --target "$target"
  done
  lipo -create -output "$APP/Contents/MacOS/aural" \
    target/aarch64-apple-darwin/release/aural target/x86_64-apple-darwin/release/aural
else
  cargo build -p aural --bin aural --release
  cp target/release/aural "$APP/Contents/MacOS/aural"
fi

# The icon, every size macOS asks for, from the SVG when ImageMagick can draw it.
ICONSET="$OUT/AppIcon.iconset"
mkdir -p "$ICONSET"
SOURCE="$OUT/icon-1024.png"
if command -v magick >/dev/null; then
  magick -background none -density 384 assets/logos/aural_icon.svg -resize 1024x1024 "$SOURCE"
else
  sips -z 1024 1024 assets/logos/aural_icon.png --out "$SOURCE" >/dev/null
fi
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$SOURCE" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$SOURCE" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET" "$SOURCE"

# macOS 26 draws the icon from assets/AppIcon.icon (Icon Composer), with its own dark and
# tinted looks instead of a grey one the system makes up. Xcode 26's actool compiles it into
# Assets.car and writes an AppIcon.icns from it for older macOS, over the one above. Without
# that actool, the .icns above is the icon.
ICON_NAME=""
xcrun actool assets/AppIcon.icon --compile "$APP/Contents/Resources" --platform macosx \
  --minimum-deployment-target "$MACOSX_DEPLOYMENT_TARGET" --app-icon AppIcon \
  --output-partial-info-plist "$OUT/icon.plist" >/dev/null 2>&1 || true
if [ -f "$APP/Contents/Resources/Assets.car" ]; then
  ICON_NAME="<key>CFBundleIconName</key><string>AppIcon</string>"
fi
rm -f "$OUT/icon.plist"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>aural</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleName</key><string>Aural</string>
  <key>CFBundleDisplayName</key><string>Aural</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$BUILD</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  $ICON_NAME
  <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSLocalNetworkUsageDescription</key>
  <string>Aural finds and controls other Aurals on your Wi-Fi with Aural Connect.</string>
  <key>NSBonjourServices</key><array><string>_aural._tcp</string></array>
</dict>
</plist>
PLIST

if [ -n "${AURAL_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$AURAL_IDENTITY" "$APP"
else
  codesign --force --sign - "$APP"
fi

# A disk image with the app and a link to /Applications to drag it onto.
STAGE="$OUT/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f build/Aural.dmg
hdiutil create -quiet -volname "Aural" -srcfolder "$STAGE" -fs HFS+ -format UDZO build/Aural.dmg
rm -rf "$STAGE"
echo "Packaged $APP and build/Aural.dmg ($VERSION, build $BUILD)"

if [ "$INSTALL" = 1 ]; then
  rm -rf /Applications/Aural.app
  cp -R "$APP" /Applications/
  echo "Installed /Applications/Aural.app"
fi
