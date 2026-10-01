#!/usr/bin/env bash
# Build the release assets (see packaging/release.md):
#
#   packaging/release.sh                 # release build → target/release-assets/
#   FULL=1 packaging/release.sh          # the "full" variant (packs compiled in)
#   SKIP_BUILD=1 packaging/release.sh    # reuse target/release/corvane
#   UPDATE_CASK=1 packaging/release.sh   # also stamp packaging/homebrew/Casks/corvane.rb (macOS half)
#   WITH_PACKS=1 packaging/release.sh    # also the pack archives (packs.sh) → target/release-assets/packs/
#   ALLOW_ADHOC=1 packaging/release.sh   # without the code-signing certificate (testing)
#
# Output: Corvane[-Full]-<version>-macos-<universal|arch>.zip (+ .dmg) and the
# cask's sha256 on stdout. Creates no git tags or remotes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/target/release-assets"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
VARIANT="Corvane"
FEATURES=()
if [[ "${FULL:-0}" == "1" ]]; then
  VARIANT="Corvane-Full"
  FEATURES=(--features full)
fi

# --- build ------------------------------------------------------------------
# C code (tree-sitter and, in the full build, every grammar) compiles for
# Info.plist's LSMinimumSystemVersion, not for the build machine's macOS
# (the arm64 slice starts at 11.0, the first Apple Silicon release)
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-10.15}"
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "building $VARIANT $VERSION (release)…"
  # --no-default-features: precompiled Metal shaders (needs Xcode), not the
  # development builds' runtime compilation
  (cd "$ROOT" && cargo build --release -p corvane --no-default-features "${FEATURES[@]}")
fi

# a universal binary when both per-target builds exist (CI: two `cargo build
# --target` runs with SKIP_BUILD=1), else the native one
ARCH_TAG="$(uname -m)"
[[ "$ARCH_TAG" == "arm64" ]] && ARCH_TAG="arm64" || ARCH_TAG="x86_64"
BIN="$ROOT/target/release/corvane"
if [[ -x "$ROOT/target/aarch64-apple-darwin/release/corvane" && -x "$ROOT/target/x86_64-apple-darwin/release/corvane" ]]; then
  mkdir -p "$ROOT/target/release"
  lipo -create -output "$BIN" \
    "$ROOT/target/aarch64-apple-darwin/release/corvane" \
    "$ROOT/target/x86_64-apple-darwin/release/corvane"
  ARCH_TAG="universal"
fi
[[ -x "$BIN" ]] || { echo "no release binary at $BIN" >&2; exit 1; }

"$ROOT/packaging/bundle.sh" release
APP="$ROOT/target/bundle/Corvane.app"
# An ad-hoc release has a new designated requirement, so every install would
# ask for its Keychain items again (packaging/signing-cert.sh)
if ! codesign -dvv "$APP" 2>&1 | grep -q '^Authority=Corvane Self-Signed$' && [[ "${ALLOW_ADHOC:-0}" != "1" ]]; then
  echo "Corvane.app is not signed with \"Corvane Self-Signed\": run packaging/signing-cert.sh import <p12>, or ALLOW_ADHOC=1" >&2
  exit 1
fi
mkdir -p "$OUT"

# --- assets -----------------------------------------------------------------
BASE="$VARIANT-$VERSION-macos-$ARCH_TAG"
ZIP="$OUT/$BASE.zip"
DMG="$OUT/$BASE.dmg"
rm -f "$ZIP" "$DMG"
(cd "$(dirname "$APP")" && ditto -c -k --sequesterRsrc --keepParent "$(basename "$APP")" "$ZIP")
hdiutil create -quiet -volname Corvane -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "zip: $ZIP ($(stat -f%z "$ZIP") bytes)"
echo "dmg: $DMG"

# the packs go to the `packs` release, not next to the app assets
if [[ "${WITH_PACKS:-0}" == "1" ]]; then
  "$ROOT/packaging/packs.sh" "$OUT/packs"
fi

# --- cask -------------------------------------------------------------------
SHA="$(shasum -a 256 "$ZIP" | cut -d' ' -f1)"
echo "cask sha256: $SHA"
if [[ "${UPDATE_CASK:-0}" == "1" && "${FULL:-0}" != "1" ]]; then
  # the Linux hashes come from the AppImages (release.yml's `cask` job)
  "$ROOT/packaging/homebrew/stamp.py" --version "$VERSION" --macos "$SHA"
fi
echo "assets in $OUT"
