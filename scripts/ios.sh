#!/usr/bin/env bash
# Builds Aural for iOS and packages build/Aural.app. On the simulator (the default) it also
# boots one, installs the app and launches it with its log in this terminal.
#
#   ./scripts/ios.sh                 # simulator, debug
#   ./scripts/ios.sh sim --release   # simulator, release
#   ./scripts/ios.sh device --release  # iPhone over the network: signs with the profile Xcode
#                                     # made for the bundle id and installs with devicectl
#
#   AURAL_BUNDLE_ID   bundle id (default dev.aural.app; a free Apple ID needs one of its own)
#   AURAL_DEVICE      which iPhone, as `xcrun devicectl list devices` names it
set -euo pipefail
cd "$(dirname "$0")/.."

WHERE="${1:-sim}"
shift || true
PROFILE=debug
CARGO_FLAGS=()
for arg in "$@"; do
  [ "$arg" = "--release" ] && PROFILE=release && CARGO_FLAGS+=(--release)
done

case "$WHERE" in
  sim) TARGET=aarch64-apple-ios-sim ;;
  device) TARGET=aarch64-apple-ios ;;
  *) echo "usage: $0 [sim|device] [--release]" >&2; exit 1 ;;
esac

BUNDLE_ID="${AURAL_BUNDLE_ID:-dev.aural.app}"
cargo build -p aural --bin aural --target "$TARGET" ${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"}

APP=build/Aural.app
rm -rf "$APP"
mkdir -p "$APP"
cp "target/$TARGET/$PROFILE/aural" "$APP/aural"
cp assets/logos/aural_icon.png "$APP/AppIcon.png"

cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>aural</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleName</key><string>Aural</string>
  <key>CFBundleDisplayName</key><string>Aural</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleIconFiles</key><array><string>AppIcon.png</string></array>
  <key>LSRequiresIPhoneOS</key><true/>
  <key>MinimumOSVersion</key><string>15.0</string>
  <key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
  <key>UILaunchScreen</key><dict/>
  <key>UIRequiresFullScreen</key><true/>
  <key>UIStatusBarHidden</key><true/>
  <key>UISupportedInterfaceOrientations</key>
  <array>
    <string>UIInterfaceOrientationPortrait</string>
    <string>UIInterfaceOrientationLandscapeLeft</string>
    <string>UIInterfaceOrientationLandscapeRight</string>
  </array>
  <key>UIBackgroundModes</key><array><string>audio</string></array>
  <key>NSLocalNetworkUsageDescription</key>
  <string>Aural finds and controls other Aurals on your Wi-Fi with Aural Connect.</string>
  <key>NSBonjourServices</key><array><string>_aural._tcp</string></array>
</dict>
</plist>
PLIST

if [ "$WHERE" = device ]; then
  # The profile Xcode made for $BUNDLE_ID (or a wildcard one), the newest first.
  PROFILE_FILE=""
  PLIST_TMP="$(mktemp -t aural-profile)"
  for file in $(ls -t "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles/"*.mobileprovision \
                      "$HOME/Library/MobileDevice/Provisioning Profiles/"*.mobileprovision 2>/dev/null); do
    security cms -D -i "$file" > "$PLIST_TMP" 2>/dev/null || continue
    app_id="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:application-identifier' "$PLIST_TMP" 2>/dev/null || true)"
    case "$app_id" in
      *".$BUNDLE_ID" | *".*") PROFILE_FILE="$file"; break ;;
    esac
  done
  if [ -z "$PROFILE_FILE" ]; then
    echo "No provisioning profile for $BUNDLE_ID. Create an iOS app in Xcode with that bundle id," >&2
    echo "pick your team and run it once on the iPhone, then run this again." >&2
    exit 1
  fi
  TEAM="$(/usr/libexec/PlistBuddy -c 'Print :TeamIdentifier:0' "$PLIST_TMP")"
  IDENTITY="${AURAL_IDENTITY:-$(security find-identity -v -p codesigning | grep -m1 "Apple Development" | sed -E 's/^ *[0-9]+\) ([0-9A-F]{40}).*/\1/')}"
  if [ -z "$IDENTITY" ]; then
    echo "No Apple Development certificate. Sign in to your Apple ID in Xcode > Settings > Accounts." >&2
    exit 1
  fi

  # The app gets exactly the profile's identity, team and debugging entitlements.
  ENTITLEMENTS="$(mktemp -t aural-entitlements)"
  /usr/libexec/PlistBuddy -x -c 'Print :Entitlements' "$PLIST_TMP" > "$ENTITLEMENTS"
  /usr/libexec/PlistBuddy -c "Set :application-identifier $TEAM.$BUNDLE_ID" "$ENTITLEMENTS"
  cp "$PROFILE_FILE" "$APP/embedded.mobileprovision"
  codesign --force --sign "$IDENTITY" --entitlements "$ENTITLEMENTS" --timestamp=none "$APP"
  echo "Signed $APP for team $TEAM with $(basename "$PROFILE_FILE")"

  # The iPhone, paired with Xcode and on the same network (or AURAL_DEVICE to pick one).
  DEVICE="${AURAL_DEVICE:-}"
  if [ -z "$DEVICE" ]; then
    LIST="$(mktemp -t aural-devices)"
    xcrun devicectl list devices --json-output "$LIST" >/dev/null
    DEVICE="$(python3 -c '
import json, sys
devices = json.load(open(sys.argv[1]))["result"]["devices"]
phones = [d for d in devices if d["hardwareProperties"].get("platform") == "iOS"]
print(phones[0]["identifier"] if phones else "")' "$LIST")"
  fi
  if [ -z "$DEVICE" ]; then
    echo "Xcode does not see an iPhone. Pair it in Xcode > Window > Devices and Simulators." >&2
    exit 1
  fi
  xcrun devicectl device install app --device "$DEVICE" "$APP"
  xcrun devicectl device process launch --device "$DEVICE" "$BUNDLE_ID"
  exit 0
fi

codesign --force --sign - "$APP" >/dev/null
DEVICE="${AURAL_SIMULATOR:-$(xcrun simctl list devices available | grep -m1 -E 'iPhone' | sed -E 's/.*\(([0-9A-F-]{36})\).*/\1/')}"
xcrun simctl boot "$DEVICE" 2>/dev/null || true
open -a Simulator
xcrun simctl install "$DEVICE" "$APP"
xcrun simctl launch --console-pty --terminate-running-process "$DEVICE" "$BUNDLE_ID"
