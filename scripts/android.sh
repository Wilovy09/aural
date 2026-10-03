#!/usr/bin/env bash
# Cross-compiles libaural.so for the given ABIs (default arm64-v8a) into build/jniLibs, then
# packages build/aural.apk.
set -euo pipefail
cd "$(dirname "$0")/.."

SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_HOME="$SDK"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$SDK/ndk/$(ls "$SDK/ndk" | sort -V | tail -1)}"
export ANDROID_NDK="$ANDROID_NDK_HOME"
export ANDROID_JAR="${ANDROID_JAR:-$SDK/platforms/$(ls "$SDK/platforms" | sort -V | tail -1)/android.jar}"
export JAVA_HOME="${JAVA_HOME:-$(/usr/libexec/java_home -v 17 2>/dev/null || echo /opt/homebrew/opt/openjdk@17)}"
export PATH="$PATH:$(ls -d "$SDK"/cmake/*/bin | sort -V | tail -1)"

ABIS=("${@:-arm64-v8a}")
TARGETS=()
for abi in "${ABIS[@]}"; do TARGETS+=(-t "$abi"); done

rm -rf build/jniLibs
cargo ndk "${TARGETS[@]}" -P 28 -o build/jniLibs build --release -p aural --lib
./scripts/apk.sh
