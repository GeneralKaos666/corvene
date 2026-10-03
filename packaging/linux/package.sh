#!/bin/bash
# Build Corvene's Linux packages from a release build, for this machine's
# architecture (amd64 / x86_64 or arm64 / aarch64) or, cross-compiled, for
# TARGET (i686-unknown-linux-gnu, armv7-unknown-linux-gnueabihf):
#   target/linux/corvene_<version>_<amd64|arm64|i386|armhf>.deb
#   target/linux/Corvene-<version>-<x86_64|aarch64|i686|armhf>.AppImage   (when appimagetool is found)
#
# Layout (both):
#   usr/lib/corvene/corvene          the app
#   usr/lib/corvene/bin/corvene      the command line tool (corvene.sh)
#   usr/bin/corvene -> ../lib/corvene/bin/corvene   (.deb only)
#   usr/share/applications/com.wasimaster.corvene.desktop
#   usr/share/icons/hicolor/{256x256,scalable}/apps/com.wasimaster.corvene.*
#
# Env: SKIP_BUILD=1 (reuse target/release/corvene), PACKAGE_BIN=<binary> (package
# that binary instead, e.g. CI's debug build), APPIMAGETOOL=<path>,
# TARGET=<Rust target> (cross-compile: needs the target's C compiler and
# its development libraries, the way release.yml installs them; the build
# machine's appimagetool fetches the target's AppImage runtime).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(cargo metadata --no-deps --format-version 1 |
  python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "corvene"))')"
# TARGET's architecture, else the build machine's (release.yml builds the
# 64-bit ones natively)
case "${TARGET:-$(uname -m)}" in
  x86_64*) ARCH_DEB=amd64 ARCH_APPIMAGE=x86_64 ;;
  aarch64* | arm64) ARCH_DEB=arm64 ARCH_APPIMAGE=aarch64 ;;
  i686*) ARCH_DEB=i386 ARCH_APPIMAGE=i686 ;;
  armv7*) ARCH_DEB=armhf ARCH_APPIMAGE=armhf ;;
  *) echo "unsupported architecture ${TARGET:-$(uname -m)}" >&2; exit 1 ;;
esac
OUT="$ROOT/target/linux"
ID=com.wasimaster.corvene

if [ -z "${SKIP_BUILD:-}" ] && [ -z "${PACKAGE_BIN:-}" ]; then
  cargo build --release -p corvene ${TARGET:+--target "$TARGET"}
fi
BIN="${PACKAGE_BIN:-$ROOT/target/${TARGET:+$TARGET/}release/corvene}"
[ -x "$BIN" ] || { echo "no $BIN" >&2; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT"

# stage the shared tree into $1
stage() {
  local root="$1"
  install -Dm755 "$BIN" "$root/usr/lib/corvene/corvene"
  install -Dm755 packaging/linux/corvene.sh "$root/usr/lib/corvene/bin/corvene"
  install -Dm644 packaging/linux/$ID.desktop "$root/usr/share/applications/$ID.desktop"
  install -Dm644 assets/icon/Corvene-256.png "$root/usr/share/icons/hicolor/256x256/apps/$ID.png"
  install -Dm644 assets/icon/Corvene.svg "$root/usr/share/icons/hicolor/scalable/apps/$ID.svg"
}

# ---- .deb -------------------------------------------------------------
DEB="$OUT/deb"
stage "$DEB"
mkdir -p "$DEB/usr/bin"
ln -s ../lib/corvene/bin/corvene "$DEB/usr/bin/corvene"
SIZE_KB="$(du -sk "$DEB/usr" | cut -f1)"
mkdir -p "$DEB/DEBIAN"
cat > "$DEB/DEBIAN/control" <<EOF
Package: corvene
Version: $VERSION
Architecture: $ARCH_DEB
Maintainer: Corvene <corvene@wasimaster.com>
Installed-Size: $SIZE_KB
Depends: git (>= 1:2.38), libc6 (>= 2.35), libxcb1, libxkbcommon0, libxkbcommon-x11-0, libfontconfig1, libfreetype6, libvulkan1, xdg-utils, perl
Recommends: mesa-vulkan-drivers, gnome-keyring | kwalletmanager | keepassxc, hunspell-en-us, fonts-noto-core
Section: devel
Priority: optional
Homepage: https://github.com/wasi-master/corvene
Description: Native Git client that looks and works like GitHub Desktop
 Corvene is a native reimplementation of GitHub Desktop 3.6.6: the same
 changes, history, branches and pull request workflow, without Electron.
EOF
cat > "$DEB/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
EOF
cp "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"
chmod 755 "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"
dpkg-deb --root-owner-group --build "$DEB" "$OUT/corvene_${VERSION}_${ARCH_DEB}.deb" >/dev/null
rm -rf "$DEB"
echo "built $OUT/corvene_${VERSION}_${ARCH_DEB}.deb"

# ---- AppImage -----------------------------------------------------------
TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -z "$TOOL" ]; then
  echo "appimagetool not found: skipping the AppImage" >&2
  exit 0
fi
APPDIR="$OUT/Corvene.AppDir"
stage "$APPDIR"
# AppImage desktop entries name the executable without a path
sed -e 's|^Exec=.*|Exec=corvene %U|' -e 's|^TryExec=.*||' \
  packaging/linux/$ID.desktop > "$APPDIR/$ID.desktop"
cp assets/icon/Corvene-256.png "$APPDIR/$ID.png"
install -Dm755 packaging/linux/AppRun "$APPDIR/AppRun"
ARCH=$ARCH_APPIMAGE "$TOOL" --no-appstream "$APPDIR" "$OUT/Corvene-${VERSION}-${ARCH_APPIMAGE}.AppImage" >/dev/null
rm -rf "$APPDIR"
echo "built $OUT/Corvene-${VERSION}-${ARCH_APPIMAGE}.AppImage"
