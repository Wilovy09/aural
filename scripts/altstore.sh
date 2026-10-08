#!/usr/bin/env bash
# Builds build/Aural.ipa and the AltStore Classic source that lists it, build/altstore.json.
# With --publish it also puts both on GitHub Releases:
#
#   ios-v<version>-<build>   one release per build, holding Aural.ipa
#   altstore                 a fixed release holding only altstore.json, replaced every time,
#                            so the url added in AltStore never changes:
#                            https://github.com/<owner>/<repo>/releases/download/altstore/altstore.json
#
#   ./scripts/altstore.sh             # build and write the source, publish nothing
#   ./scripts/altstore.sh --publish   # also create the release and update the source (needs gh)
set -euo pipefail
cd "$(dirname "$0")/.."

PUBLISH=false
[ "${1:-}" = "--publish" ] && PUBLISH=true

REPO="${AURAL_REPO:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
VERSION="$(sed -nE 's/^version = "(.*)"/\1/p' Cargo.toml | head -1)"
BUILD="$(git rev-list --count HEAD)"
TAG="ios-v$VERSION-$BUILD"
export AURAL_BUILD="$BUILD"

./scripts/ios.sh ipa

# The source so far, so earlier builds stay listed under the new one.
PREVIOUS="$(mktemp -d)"
gh release download altstore --repo "$REPO" --pattern altstore.json --dir "$PREVIOUS" 2>/dev/null || true

python3 - "$REPO" "$VERSION" "$BUILD" "$TAG" "$PREVIOUS/altstore.json" <<'PY'
import datetime, json, os, plistlib, subprocess, sys

repo, version, build, tag, previous = sys.argv[1:]
with open("build/Aural.app/Info.plist", "rb") as file:
    info = plistlib.load(file)
raw = f"https://raw.githubusercontent.com/{repo}/main"
subject = subprocess.run(
    ["git", "log", "-1", "--format=%s"], capture_output=True, text=True
).stdout.strip()

release = {
    "version": version,
    "buildVersion": build,
    "date": datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat(),
    "localizedDescription": subject,
    "downloadURL": f"https://github.com/{repo}/releases/download/{tag}/Aural.ipa",
    "size": os.path.getsize("build/Aural.ipa"),
    "minOSVersion": info["MinimumOSVersion"],
}
older = []
if os.path.exists(previous):
    with open(previous) as file:
        for app in json.load(file).get("apps", []):
            if app.get("bundleIdentifier") == info["CFBundleIdentifier"]:
                older = [
                    kept for kept in app.get("versions", [])
                    if kept.get("buildVersion") != build
                ]

source = {
    "name": "Aural",
    "subtitle": "A YouTube Music client for TV, phone and desktop.",
    "description": "Aural plays your YouTube Music library with synced lyrics, a fullscreen "
    "player and Aural Connect. Built with Rust and Freya.",
    "iconURL": f"{raw}/assets/logos/aural_icon.png",
    "website": f"https://github.com/{repo}",
    "tintColor": "#F5F5F5",
    "apps": [
        {
            "name": "Aural",
            "bundleIdentifier": info["CFBundleIdentifier"],
            "developerName": repo.split("/")[0],
            "subtitle": "YouTube Music with synced lyrics.",
            "localizedDescription": "Your YouTube Music library, search, artist pages, synced "
            "and translated lyrics, likes, a queue you can shape, and Aural Connect to play on "
            "another Aural on your Wi-Fi.",
            "iconURL": f"{raw}/assets/logos/aural_icon.png",
            "tintColor": "#F5F5F5",
            "category": "entertainment",
            "screenshots": [
                f"{raw}/assets/screenshots/player.jpg",
            ],
            "versions": [release] + older,
            # AltStore refuses an app whose permissions differ from the ones listed here.
            "appPermissions": {
                "entitlements": [],
                "privacy": {
                    key: value
                    for key, value in info.items()
                    if key.endswith("UsageDescription")
                },
            },
        }
    ],
    "news": [],
}
with open("build/altstore.json", "w") as file:
    json.dump(source, file, indent=2, ensure_ascii=False)
print(f"Wrote build/altstore.json ({len(older) + 1} versions)")
PY

if ! $PUBLISH; then
  echo "Source url once published: https://github.com/$REPO/releases/download/altstore/altstore.json"
  echo "Run again with --publish to put $TAG and the source on GitHub Releases."
  exit 0
fi

gh release create "$TAG" build/Aural.ipa --repo "$REPO" \
  --title "Aural $VERSION ($BUILD) for iOS" \
  --notes "Install with AltStore Classic: add the source https://github.com/$REPO/releases/download/altstore/altstore.json"
if gh release view altstore --repo "$REPO" >/dev/null 2>&1; then
  gh release upload altstore build/altstore.json --repo "$REPO" --clobber
else
  gh release create altstore build/altstore.json --repo "$REPO" --latest=false \
    --title "AltStore source" \
    --notes "The AltStore Classic source for Aural. Add this url in AltStore: https://github.com/$REPO/releases/download/altstore/altstore.json"
fi
echo "Published $TAG. Source: https://github.com/$REPO/releases/download/altstore/altstore.json"
