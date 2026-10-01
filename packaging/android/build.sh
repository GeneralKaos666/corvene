#!/bin/bash
# Build Corvane's Android package: the bundled git (git/build.sh), the Rust
# library with cargo-ndk, then the APK with Gradle.
#
#   packaging/android/build.sh [debug|profiling|release] [foss|play]
#
# debug is for compiling quickly; its unoptimised library draws several times
# slower than a release, so judge speed on a device with profiling (the
# workspace's optimised profile with symbols, in Gradle's debug package:
# debuggable, so simpleperf and run-as work) or release.
#
# Without a flavour both packages are built (app/build.gradle.kts). A release
# is signed when CORVANE_ANDROID_KEYSTORE, CORVANE_ANDROID_KEYSTORE_PASSWORD,
# CORVANE_ANDROID_KEY_ALIAS and CORVANE_ANDROID_KEY_PASSWORD are set.
#
# Needs the Android SDK and NDK (ANDROID_HOME, ANDROID_NDK_HOME), a JDK 17+
# (JAVA_HOME), `cargo install cargo-ndk`, the Rust targets of the ABIS, and
# for the bundled git: make, perl and Go.
#
# Env: ABIS (default "arm64-v8a x86_64"), SKIP_GRADLE=1 (library only),
# BUNDLE=1 (also the .aab for Google Play, with the grammar module).
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
# cargo's profile and Gradle's build type
case "$PROFILE" in
  debug) profile_flag= VARIANT=debug ;;
  profiling) profile_flag="--profile profiling" VARIANT=debug ;;
  release) profile_flag=--release VARIANT=release ;;
  *) echo "unknown profile $PROFILE" >&2; exit 2 ;;
esac

# git, its HTTPS helper, ssh and git-lfs, once per ABI (git/build.sh)
missing=
for abi in $ABIS; do
  [ -f "$JNI_LIBS/$abi/libgit.so" ] || missing="$missing $abi"
done
[ -z "$missing" ] || packaging/android/git/build.sh $missing

# `--lib`: the activity loads libcorvane.so; Android has no use for the binary.
# API 26 is the minimum the manifest declares.
cargo ndk "${targets[@]}" -P 26 -o "$JNI_LIBS" build -p corvane --lib $profile_flag

# git's askpass helper (crates/corvane-askpass), an executable packaged like
# the bundled git's
cargo ndk "${targets[@]}" -P 26 build -p corvane-askpass $profile_flag
for abi in $ABIS; do
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android ;;
    x86_64) triple=x86_64-linux-android ;;
  esac
  cp "target/$triple/$PROFILE/corvane-askpass" "$JNI_LIBS/$abi/libcorvane-askpass.so"
done

# A debug library carries a gigabyte of debug info; the package keeps the
# symbol table (backtraces) only.
STRIP="$(find "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt" -name llvm-strip | head -1)"
for abi in $ABIS; do
  "$STRIP" --strip-debug "$JNI_LIBS/$abi/libcorvane.so"
  "$STRIP" "$JNI_LIBS/$abi/libcorvane-askpass.so"
  # cargo-ndk also copies the cdylib of a dependency that libcorvane.so
  # already links statically
  rm -f "$JNI_LIBS/$abi"/libandroid_native_keyring_store-*.so
done

[ -n "${SKIP_GRADLE:-}" ] && exit 0
cd packaging/android
# assemble[Foss|Play]<Debug|Release>, of the application only: the grammar
# feature module is not part of an APK
flavour="${2:-}"
task=":app:assemble$(echo "${flavour:0:1}" | tr a-z A-Z)${flavour:1}$(echo "${VARIANT:0:1}" | tr a-z A-Z)${VARIANT:1}"
./gradlew --no-daemon "$task"
find app/build/outputs/apk -name '*.apk'

# BUNDLE=1: also the bundle Google Play takes (the play flavour with the
# on-demand grammar module, which grammars.sh builds first)
if [ -n "${BUNDLE:-}" ]; then
  ABIS="$ABIS" "$ROOT/packaging/android/grammars.sh"
  ./gradlew --no-daemon ":app:bundlePlay$(echo "${VARIANT:0:1}" | tr a-z A-Z)${VARIANT:1}"
  find app/build/outputs/bundle -name '*.aab'
fi
