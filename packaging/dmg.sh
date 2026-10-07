#!/usr/bin/env bash
# Builds the macOS disk image: Corvene.app next to an /Applications link over
# a background with an arrow between them, so the drag target sits beside the
# app as in other installers.
#
#   packaging/dmg.sh <Corvene.app> <out.dmg>
#   packaging/dmg.sh --background        # regenerate assets/dmg/background.tiff
#
# The layout lives in the volume's .DS_Store, which Finder writes over
# AppleScript on a read-write image, so this needs a logged-in session (the
# GitHub macOS runners have one). It fails when Finder cannot lay the image
# out; DMG_PLAIN=1 builds it without the layout instead.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ART="$ROOT/assets/dmg"

if [[ "${1:-}" == "--background" ]]; then
  # needs rsvg-convert (brew install librsvg); 1x and 2x in one TIFF
  tmp="$(mktemp -d)"
  rsvg-convert -w 540 -h 380 "$ART/background.svg" -o "$tmp/bg.png"
  rsvg-convert -w 1080 -h 760 "$ART/background.svg" -o "$tmp/bg@2x.png"
  tiffutil -cathidpicheck "$tmp/bg.png" "$tmp/bg@2x.png" -out "$ART/background.tiff"
  rm -rf "$tmp"
  echo "wrote assets/dmg/background.tiff"
  exit 0
fi

[[ $# -eq 2 ]] || { echo "usage: packaging/dmg.sh <app> <out.dmg> | --background" >&2; exit 2; }
APP="$1"
DMG="$2"
APP_NAME="$(basename "$APP")"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/stage"
RW="$WORK/rw.dmg"
mkdir -p "$STAGE/.background"
ditto "$APP" "$STAGE/$APP_NAME"
ln -s /Applications "$STAGE/Applications"
cp "$ART/background.tiff" "$STAGE/.background/background.tiff"

# A unique name while Finder lays it out: Finder finds volumes by name, so a
# mounted Corvene image would otherwise get the layout instead. It is mounted
# under /Volumes because Finder lists no disk for a custom mount point.
VOL="Corvene-dmg-$$"
hdiutil create -quiet -volname "$VOL" -srcfolder "$STAGE" -fs HFS+ -format UDRW -ov "$RW"
DEV="$(hdiutil attach -readwrite -noverify -noautoopen "$RW" | awk '/Apple_HFS/ {print $1; exit}')"
MNT="/Volumes/$VOL"
# Finder or Spotlight can hold the volume for a moment after the layout
# (EBUSY), so detaching retries before forcing it
detach() {
  local i
  for i in 1 2 3 4 5; do
    hdiutil detach -quiet "$DEV" && return 0
    sleep 2
  done
  hdiutil detach -quiet -force "$DEV"
}

if [[ "${DMG_PLAIN:-0}" != "1" ]]; then
  # the window is 540×380 points of content (the background's size) under a
  # 28-point title bar; the icons centre on the arrow's ends in background.svg
  if ! osascript - "$VOL" "$APP_NAME" <<'OSA'; then
on run argv
  set volName to item 1 of argv
  set appName to item 2 of argv
  tell application "Finder"
    set d to disk volName
    open d
    delay 1
    set w to container window of d
    set current view of w to icon view
    set toolbar visible of w to false
    set statusbar visible of w to false
    set bounds of w to {200, 120, 740, 528}
    set opts to icon view options of w
    set arrangement of opts to not arranged
    set icon size of opts to 128
    set text size of opts to 13
    set background picture of opts to file ".background:background.tiff" of d
    set position of item appName of w to {140, 160}
    set position of item "Applications" of w to {400, 160}
    update d without registering applications
    delay 1
    close w
  end tell
end run
OSA
    detach
    echo "dmg: Finder could not lay out the image (DMG_PLAIN=1 builds it without the layout)" >&2
    exit 1
  fi
fi

rm -rf "$MNT/.fseventsd"
chmod -Rf go-w "$MNT" 2>/dev/null || true
diskutil rename "$DEV" Corvene >/dev/null
sync
detach
rm -f "$DMG"
hdiutil convert -quiet "$RW" -format UDZO -imagekey zlib-level=9 -o "$DMG"
