#!/bin/bash
# Build Corvene's Flatpak bundle from a Linux release archive
# (packaging/linux/package.sh's .tar.gz):
#
#   packaging/flatpak/build.sh target/linux/Corvene-<version>-linux-x86_64.tar.gz
#     → target/linux/Corvene-<version>-x86_64.flatpak
#   (Corvene-Full-….tar.gz → Corvene-Full-<version>-<arch>.flatpak)
#
# for the build machine's architecture (x86_64 or aarch64: Flathub's
# runtime has no 32-bit builds). Needs flatpak and flatpak-builder; the
# runtime and SDK (com.wasimaster.corvene.yml) are installed per user from
# Flathub. Install the bundle with
#   flatpak install --user Corvene-<version>-<arch>.flatpak
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ARCHIVE="$(cd "$(dirname "${1:?usage: build.sh <Corvene-…-linux-<arch>.tar.gz> [out-dir]}")" && pwd)/$(basename "$1")"
OUT="${2:-$ROOT/target/linux}"
ID=com.wasimaster.corvene

# Corvene[-Full]-<version>-linux-<arch>.tar.gz
base="$(basename "$ARCHIVE" .tar.gz)"
ARCH="${base##*-linux-}"
NAME_VERSION="${base%-linux-*}"
case "$ARCH" in
  x86_64 | aarch64) ;;
  *) echo "no Flatpak for $ARCH (x86_64 and aarch64 only)" >&2; exit 1 ;;
esac
[[ "$ARCH" == "$(uname -m)" ]] || { echo "build the $ARCH Flatpak on an $ARCH machine" >&2; exit 1; }

WORK="$ROOT/target/flatpak"
rm -rf "$WORK"
mkdir -p "$WORK" "$OUT"
cp "$ROOT/packaging/flatpak/$ID.yml" "$WORK/"
cp "$ARCHIVE" "$WORK/corvene.tar.gz"

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
# --disable-rofiles-fuse: no FUSE in containers and CI runners
flatpak-builder --user --install-deps-from=flathub --disable-rofiles-fuse --force-clean \
  --default-branch=stable --repo="$WORK/repo" "$WORK/build" "$WORK/$ID.yml"
# the runtime comes from Flathub when the bundle is installed
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$WORK/repo" "$OUT/$NAME_VERSION-$ARCH.flatpak" "$ID" stable
rm -rf "$WORK"
echo "built $OUT/$NAME_VERSION-$ARCH.flatpak"
