#!/bin/bash
# Build Corvane's Android package: the bundled git (git/build.sh), the Rust
# library with cargo-ndk, then the APK with Gradle.
#
#   packaging/android/build.sh [debug|release]
#
# Needs the Android SDK and NDK (ANDROID_HOME, ANDROID_NDK_HOME), a JDK 17+
# (JAVA_HOME), `cargo install cargo-ndk`, the Rust targets of the ABIS, and
# for the bundled git: make, perl and Go.
#
# Env: ABIS (default "arm64-v8a x86_64"), SKIP_GRADLE=1 (library only).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
PROFILE="${1:-debug}"
ABIS="${ABIS:-arm64-v8a x86_64}"
JNI_LIBS="$ROOT/packaging/android/app/src/main/jniLibs"

targets=()
for abi in $ABIS; do
  targets+=(-t "$abi")
done
profile_flag=
[ "$PROFILE" = release ] && profile_flag=--release

# git, its HTTPS helper, ssh and git-lfs, once per ABI (git/build.sh)
missing=
for abi in $ABIS; do
  [ -f "$JNI_LIBS/$abi/libgit.so" ] || missing="$missing $abi"
done
[ -z "$missing" ] || packaging/android/git/build.sh $missing

# `--lib`: the activity loads libcorvane.so; Android has no use for the binary.
# API 26 is the minimum the manifest declares.
cargo ndk "${targets[@]}" -P 26 -o "$JNI_LIBS" build -p corvane --lib $profile_flag

# A debug library carries a gigabyte of debug info; the package keeps the
# symbol table (backtraces) only.
STRIP="$(find "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt" -name llvm-strip | head -1)"
for abi in $ABIS; do
  "$STRIP" --strip-debug "$JNI_LIBS/$abi/libcorvane.so"
done

[ -n "${SKIP_GRADLE:-}" ] && exit 0
cd packaging/android
if [ "$PROFILE" = release ]; then
  ./gradlew --no-daemon assembleRelease
else
  ./gradlew --no-daemon assembleDebug
fi
find app/build/outputs/apk -name '*.apk'
