#!/bin/bash
# Prepare the snap's build folder from a Linux release archive
# (packaging/linux/package.sh's .tar.gz):
#
#   packaging/snap/prepare.sh target/linux/Corvene-<version>-linux-x86_64.tar.gz target/snap
#   (cd target/snap && snapcraft pack)   # → corvene_<version>_amd64.snap
#
# release.yml builds it with snapcore/action-build (LXD) on an x86_64 and an
# arm64 runner, and renames the result Corvene[-Full]-<version>-<arch>.snap.
# The archive is unpacked with its top folder stripped, so the snap holds
# the same usr/ tree as the .deb.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ARCHIVE="${1:?usage: prepare.sh <Corvene-…-linux-<arch>.tar.gz> <dir>}"
DIR="${2:?usage: prepare.sh <archive> <dir>}"
VERSION="$(basename "$ARCHIVE" .tar.gz | sed -E 's/^Corvene(-Full)?-(.*)-linux-[^-]+$/\2/')"

rm -rf "$DIR"
mkdir -p "$DIR/snap" "$DIR/tree"
tar -xzf "$ARCHIVE" -C "$DIR/tree" --strip-components=1
tar -C "$DIR/tree" -czf "$DIR/corvene.tar.gz" usr
rm -rf "$DIR/tree"
sed "s|@VERSION@|$VERSION|" "$ROOT/packaging/snap/snapcraft.yaml" > "$DIR/snap/snapcraft.yaml"
echo "prepared $DIR (corvene $VERSION)"
