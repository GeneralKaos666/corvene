#!/usr/bin/env bash
# The key Corvene's Android packages are signed with (packaging/release.md,
# "Android").
#
#   packaging/android/keystore.sh create [dir]    # new keystore + password in dir (default ~/.corvene-signing)
#   packaging/android/keystore.sh secrets [dir]   # store both as the repository's Actions secrets (gh)
#   packaging/android/keystore.sh env [dir]       # print the exports build.sh signs a release with
#
# Android installs an update only over a package signed with the same key:
# losing the keystore or its password means every user has to uninstall
# (and lose their settings) before installing a package signed with a new
# one. Keep both in the password manager.
set -euo pipefail

DIR="${2:-$HOME/.corvene-signing}"
STORE="$DIR/corvene-android.jks"
PASSWORD_FILE="$DIR/corvene-android.password"
ALIAS=corvene
# the JDK's keytool (macOS ships a stub that only asks for a JDK)
KEYTOOL="${JAVA_HOME:+$JAVA_HOME/bin/}keytool"

case "${1:-}" in
  create)
    [[ -e "$STORE" ]] && { echo "$STORE exists; a new key cannot update installed packages" >&2; exit 1; }
    mkdir -p "$DIR" && chmod 700 "$DIR"
    umask 077
    /usr/bin/openssl rand -base64 30 | tr -d '\n=+/' > "$PASSWORD_FILE"
    # PKCS12 has one password for the store and the key; 10000 days is past
    # Google Play's minimum (a validity ending after 2033)
    "$KEYTOOL" -genkeypair -keystore "$STORE" -storetype PKCS12 \
      -storepass:file "$PASSWORD_FILE" -alias "$ALIAS" \
      -keyalg RSA -keysize 4096 -sigalg SHA256withRSA -validity 10000 \
      -dname "CN=Corvene, O=Corvene" || { rm -f "$PASSWORD_FILE"; exit 1; }
    echo "created $STORE (password in $PASSWORD_FILE)"
    "$KEYTOOL" -list -v -keystore "$STORE" -storepass:file "$PASSWORD_FILE" -alias "$ALIAS" | grep -E 'SHA256:|Valid from'
    ;;
  secrets)
    [[ -f "$STORE" && -f "$PASSWORD_FILE" ]] || { echo "no keystore in $DIR: run \`$0 create\`" >&2; exit 1; }
    base64 < "$STORE" | tr -d '\n' | gh secret set CORVENE_ANDROID_KEYSTORE_BASE64
    gh secret set CORVENE_ANDROID_KEYSTORE_PASSWORD < "$PASSWORD_FILE"
    gh secret set CORVENE_ANDROID_KEY_PASSWORD < "$PASSWORD_FILE"
    gh secret set CORVENE_ANDROID_KEY_ALIAS --body "$ALIAS"
    ;;
  env)
    [[ -f "$STORE" && -f "$PASSWORD_FILE" ]] || { echo "no keystore in $DIR: run \`$0 create\`" >&2; exit 1; }
    echo "export CORVENE_ANDROID_KEYSTORE='$STORE'"
    echo "export CORVENE_ANDROID_KEYSTORE_PASSWORD=\"\$(cat '$PASSWORD_FILE')\""
    echo "export CORVENE_ANDROID_KEY_PASSWORD=\"\$(cat '$PASSWORD_FILE')\""
    echo "export CORVENE_ANDROID_KEY_ALIAS=$ALIAS"
    ;;
  *)
    sed -n '2,12p' "$0" >&2
    exit 2
    ;;
esac
