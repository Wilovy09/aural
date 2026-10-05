#!/usr/bin/env bash
# Builds build/aural.apk from build/jniLibs without Gradle: aapt2 links the manifest and
# resources, the native libraries are added under lib/<abi>/, then zipalign and apksigner
# (debug keystore) finish it.
set -euo pipefail
cd "$(dirname "$0")/.."

SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
TOOLS="$SDK/build-tools/$(ls "$SDK/build-tools" | sort -V | tail -1)"
PLATFORM="$SDK/platforms/$(ls "$SDK/platforms" | sort -V | tail -1)/android.jar"
OUT=build/apk
rm -rf "$OUT" && mkdir -p "$OUT/res" "$OUT/stage/lib"

# Every build gets a newer versionCode (minutes since 2026-01-01): launchers cache an app's
# icon and banner per version, so a reinstall at the same code keeps showing the old ones.
VERSION_CODE=$(( ($(date +%s) - 1767225600) / 60 ))
"$TOOLS/aapt2" compile --dir android/res -o "$OUT/res.zip"
"$TOOLS/aapt2" link -I "$PLATFORM" --manifest android/AndroidManifest.xml \
  --version-code "$VERSION_CODE" --replace-version \
  -o "$OUT/unaligned.apk" "$OUT/res.zip"

JAVA_HOME="${JAVA_HOME:-$(/usr/libexec/java_home -v 17 2>/dev/null || echo /opt/homebrew/opt/openjdk@17)}"
mkdir -p "$OUT/classes"
"$JAVA_HOME/bin/javac" -nowarn -source 8 -target 8 -Xlint:-options -cp "$PLATFORM" \
  -d "$OUT/classes" $(find android/java -name '*.java')
"$TOOLS/d8" --min-api 28 --lib "$PLATFORM" --output "$OUT/stage" $(find "$OUT/classes" -name '*.class')

cp -R build/jniLibs/* "$OUT/stage/lib/"
(cd "$OUT/stage" && zip -qr -0 ../unaligned.apk lib classes.dex)

"$TOOLS/zipalign" -f -p 4 "$OUT/unaligned.apk" "$OUT/aligned.apk"
"$TOOLS/apksigner" sign --ks "$HOME/.android/debug.keystore" --ks-pass pass:android \
  --key-pass pass:android --out build/aural.apk "$OUT/aligned.apk"
echo "build/aural.apk ($(du -h build/aural.apk | cut -f1))"
