#!/bin/bash
# Cross-build the git Corvene bundles on Android: git itself, the HTTPS
# transport (curl + OpenSSL, statically linked into git-remote-https), the
# OpenSSH client and git-lfs. Termux's build recipes are the reference for
# the flags.
#
#   packaging/android/git/build.sh [abi…]      default: arm64-v8a armeabi-v7a x86_64 x86
#
# An Android app can only execute files from its native library directory,
# where the installer extracts what the APK holds as lib/<abi>/lib*.so. So
# every executable is packaged under such a name:
#
#   libgit.so                 git (all builtins)
#   libgit-remote-https.so    git-remote-http / -https
#   libssh.so                 ssh
#   libgit-lfs.so             git-lfs
#   libgit-sh-*.so …          git's shell scripts (submodule, mergetool)
#
# At start-up Corvene links the names git expects to them
# (crates/corvene-platform/src/android/git.rs) and sets GIT_EXEC_PATH.
#
# Needs ANDROID_NDK_HOME, make, perl (OpenSSL's Configure) and Go (git-lfs).
# Env: API (26), OUT (app/src/main/jniLibs), WORK (target/android-git),
# SKIP_LFS=1, SKIP_SSH=1.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
. "$HERE/versions.sh"

API="${API:-26}"
OUT="${OUT:-$ROOT/packaging/android/app/src/main/jniLibs}"
WORK="${WORK:-$ROOT/target/android-git}"
DOWNLOADS="$WORK/downloads"
ABIS="${*:-arm64-v8a armeabi-v7a x86_64 x86}"
JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"

: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME}"
TOOLCHAIN="$(ls -d "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/ | head -1)"
TOOLCHAIN="${TOOLCHAIN%/}"
[ -x "$TOOLCHAIN/bin/clang" ] || { echo "no NDK toolchain in $ANDROID_NDK_HOME" >&2; exit 1; }
export PATH="$TOOLCHAIN/bin:$PATH"
# OpenSSL's Configure looks the NDK up here
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# fetch <urls> <sha256>: the verified tarball's path (urls: mirrors, in order)
fetch() {
  local urls="$1" url file
  file="$DOWNLOADS/${urls##*/}"
  mkdir -p "$DOWNLOADS"
  if [ ! -f "$file" ]; then
    for url in $urls; do
      curl -fsSL --retry 3 --connect-timeout 30 -o "$file.part" "$url" && mv "$file.part" "$file" && break
      echo "download failed: $url" >&2
    done
  fi
  [ -f "$file" ] || exit 1
  local actual
  actual="$(sha256 "$file")"
  if [ "$actual" != "$2" ]; then
    echo "$file: SHA-256 $actual, expected $2" >&2
    exit 1
  fi
  echo "$file"
}

# unpack <tarball> <dir>: a fresh source tree
unpack() {
  # an empty name: fetch failed inside its command substitution
  [ -n "$1" ] || exit 1
  rm -rf "$2"
  mkdir -p "$2"
  tar -xf "$1" -C "$2" --strip-components 1
}

build_abi() {
  # triple: what configure scripts call the ABI; clang: the NDK's compiler
  # for it (32-bit ARM's is named after the armv7a it targets)
  local abi="$1" triple clang openssl_target goarch
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android openssl_target=android-arm64 goarch=arm64 ;;
    armeabi-v7a) triple=arm-linux-androideabi clang=armv7a-linux-androideabi openssl_target=android-arm goarch=arm ;;
    x86_64) triple=x86_64-linux-android openssl_target=android-x86_64 goarch=amd64 ;;
    x86) triple=i686-linux-android openssl_target=android-x86 goarch=386 ;;
    *) echo "unsupported ABI $abi" >&2; exit 1 ;;
  esac
  local cc="$TOOLCHAIN/bin/${clang:-$triple}$API-clang"
  local build="$WORK/$abi"
  local prefix="$build/prefix"
  local out="$OUT/$abi"
  mkdir -p "$build" "$prefix" "$out"
  export CC="$cc" AR=llvm-ar RANLIB=llvm-ranlib STRIP=llvm-strip
  # 16 KB pages (Android 15 devices) for everything linked here
  local ldflags="-Wl,-z,max-page-size=16384"

  # ── OpenSSL (static) ──────────────────────────────────────────────────
  if [ ! -f "$prefix/lib/libssl.a" ]; then
    unpack "$(fetch "$OPENSSL_URL" "$OPENSSL_SHA256")" "$build/openssl"
    (
      cd "$build/openssl"
      # its Configure picks the compiler from the target and the PATH
      unset CC AR RANLIB
      ./Configure "$openssl_target" -D__ANDROID_API__="$API" \
        no-shared no-tests no-apps no-docs no-engine no-module no-legacy \
        --prefix="$prefix" --libdir=lib --openssldir=/system/etc/security
      make -j"$JOBS" build_libs
      make install_dev
    ) >"$build/openssl.log" 2>&1 || { tail -40 "$build/openssl.log" >&2; exit 1; }
  fi

  # ── nghttp2 (static): HTTP/2 for curl ─────────────────────────────────
  if [ ! -f "$prefix/lib/libnghttp2.a" ]; then
    unpack "$(fetch "$NGHTTP2_URL" "$NGHTTP2_SHA256")" "$build/nghttp2"
    (
      cd "$build/nghttp2"
      ./configure --host="$triple" --prefix="$prefix" --enable-lib-only \
        --disable-shared --enable-static --disable-examples --disable-python-bindings
      make -j"$JOBS" -C lib
      make -C lib install
    ) >"$build/nghttp2.log" 2>&1 || { tail -40 "$build/nghttp2.log" >&2; exit 1; }
    # curl was configured without it
    rm -f "$prefix/lib/libcurl.a" "$build/git/git"
  fi

  # ── libiconv (static): bionic has iconv only from API 28 ──────────────
  if [ ! -f "$prefix/lib/libiconv.a" ]; then
    unpack "$(fetch "$LIBICONV_URL" "$LIBICONV_SHA256")" "$build/libiconv"
    (
      cd "$build/libiconv"
      ./configure --host="$triple" --prefix="$prefix" \
        --disable-shared --enable-static --disable-nls --enable-extra-encodings
      make -j"$JOBS"
      make install
    ) >"$build/libiconv.log" 2>&1 || { tail -40 "$build/libiconv.log" >&2; exit 1; }
    rm -f "$build/git/git"
  fi

  # ── curl (static, HTTP and HTTPS only) ────────────────────────────────
  if [ ! -f "$prefix/lib/libcurl.a" ]; then
    unpack "$(fetch "$CURL_URL" "$CURL_SHA256")" "$build/curl"
    (
      cd "$build/curl"
      ./configure --host="$triple" --prefix="$prefix" \
        --disable-shared --enable-static \
        --with-openssl="$prefix" --with-zlib \
        --with-ca-path=/system/etc/security/cacerts --without-ca-bundle \
        --without-libpsl --without-brotli --without-zstd --with-nghttp2="$prefix" \
        --without-libidn2 --without-librtmp --without-libssh2 \
        --disable-ldap --disable-ldaps --disable-rtsp --disable-dict \
        --disable-telnet --disable-tftp --disable-pop3 --disable-imap \
        --disable-smb --disable-smtp --disable-gopher --disable-mqtt \
        --disable-ftp --disable-file --disable-manual --disable-docs \
        --disable-libcurl-option --disable-unix-sockets
      make -j"$JOBS" -C lib
      make -C lib install
      make -C include install
      make install-pkgconfigDATA install-binSCRIPTS
    ) >"$build/curl.log" 2>&1 || { tail -40 "$build/curl.log" >&2; exit 1; }
  fi

  # ── git ───────────────────────────────────────────────────────────────
  if [ ! -f "$build/git/git" ]; then
    unpack "$(fetch "$GIT_URL" "$GIT_SHA256")" "$build/git"
    (
      cd "$build/git"
      # Termux's fixes for bionic (patches/README.md)
      patch -p1 <"$HERE/patches/git-disable-fdsan.patch"
      patch -p0 <"$HERE/patches/git-run-command.c.patch"
      patch -p0 <"$HERE/patches/git-config.c.patch"
      # SHELL_PATH is the device's shell; the build's generator scripts
      # run with the build machine's
      sed -i.orig 's|\$(QUIET_GEN)\$(SHELL_PATH) |$(QUIET_GEN)/bin/sh |' Makefile
      sed -i.orig 's|^\$(SHELL_PATH) |/bin/sh |' shared.mak
      # uname_S (on the command line, config.mak is read too late): the
      # Makefile configures for the machine it runs on. Bionic
      # before API 28 has no iconv (GNU libiconv is linked in) and no
      # getrandom; there is no gettext, Perl, Python or Tcl on the device,
      # and the shell is /system/bin/sh.
      cat >config.mak <<MAK
CC = $cc
AR = llvm-ar
STRIP = llvm-strip
HOSTCC = cc
CFLAGS = -O2 -fPIE
LDFLAGS = -pie $ldflags
prefix = /usr
SHELL_PATH = /system/bin/sh
NO_GETTEXT = 1
NO_PERL = 1
NO_PYTHON = 1
NO_TCLTK = 1
NO_EXPAT = 1
ICONVDIR = $prefix
NEEDS_LIBICONV = 1
NO_OPENSSL = 1
NO_INSTALL_HARDLINKS = 1
NO_NSEC = 1
NO_RUST = 1
PTHREAD_LIBS =
NO_GECOS_IN_PWENT = 1
NO_SVN_TESTS = 1
CSPRNG_METHOD =
HAVE_GETDELIM = 1
CURL_CONFIG = $prefix/bin/curl-config
CURL_CFLAGS = -I$prefix/include
CURL_LDFLAGS = -L$prefix/lib -lcurl -lnghttp2 -lssl -lcrypto -lz
MAK
      make -j"$JOBS" SHELL=/bin/sh uname_S=Linux uname_O=Android git git-remote-http git-sh-setup git-sh-i18n \
        git-submodule git-mergetool git-mergetool--lib
    ) >"$build/git.log" 2>&1 || { tail -40 "$build/git.log" >&2; exit 1; }
  fi
  llvm-strip -o "$out/libgit.so" "$build/git/git"
  llvm-strip -o "$out/libgit-remote-https.so" "$build/git/git-remote-http"
  for script in git-sh-setup git-sh-i18n git-submodule git-mergetool git-mergetool--lib; do
    cp "$build/git/$script" "$out/lib$script.so"
  done

  # ── OpenSSH client ────────────────────────────────────────────────────
  if [ -z "${SKIP_SSH:-}" ]; then
    if [ ! -f "$build/openssh/ssh-keygen" ]; then
      unpack "$(fetch "$OPENSSH_URL" "$OPENSSH_SHA256")" "$build/openssh"
      (
        cd "$build/openssh"
        # An app has no passwd entry worth the name: its home is $HOME
        # (Corvene's app-private storage). The configure answers below are
        # Termux's for bionic.
        patch -p1 <"$HERE/patches/openssh-home.patch"
        # Termux's: bionic declares no bzero here, and an app may not
        # create hard links (the known_hosts backup)
        patch -p1 <"$HERE/patches/openssh-openbsd-compat_explicit_bzero.c.patch"
        patch -p1 <"$HERE/patches/openssh-hostfile.c.patch"
        # No SSHFP lookups (VerifyHostKeyDNS): the compat resolver needs
        # libresolv internals bionic does not export (Termux links ldns).
        cat >openbsd-compat/getrrsetbyname.c <<'STUB'
#include "includes.h"
#if !defined(HAVE_GETRRSETBYNAME) && !defined(HAVE_LDNS)
#include "getrrsetbyname.h"
int
getrrsetbyname(const char *hostname, unsigned int rdclass,
    unsigned int rdtype, unsigned int flags, struct rrsetinfo **res)
{
	return (ERRSET_FAIL);
}
void
freerrset(struct rrsetinfo *rrset)
{
}
#endif
STUB
        ./configure --host="$triple" --prefix=/usr \
          --with-ssl-dir="$prefix" --without-zlib-version-check \
          --with-pie --disable-strip --without-pam --without-selinux \
          --disable-lastlog --disable-utmp --disable-utmpx --disable-wtmp \
          --disable-wtmpx --disable-libutil --disable-pututline \
          --disable-pututxline --disable-pkcs11 --disable-security-key \
          --without-stackprotect --with-privsep-path=/data/local/tmp \
          --sysconfdir=/data/local/tmp/ssh \
          --with-cflags=-Dfd_mask=int LDFLAGS="$ldflags" \
          CPPFLAGS="-DHAVE_ATTRIBUTE__SENTINEL__=1 -DBROKEN_SETRESGID" \
          ac_cv_func_endgrent=yes ac_cv_func_fmt_scaled=no \
          ac_cv_func_getlastlogxbyname=no ac_cv_func_readpassphrase=no \
          ac_cv_func_strnvis=no ac_cv_header_sys_un_h=yes \
          ac_cv_lib_crypt_crypt=no ac_cv_search_getrrsetbyname=no \
          ac_cv_func_bzero=yes ac_cv_member_struct_passwd_pw_gecos=no
        make -j"$JOBS" ssh ssh-keygen
      ) >"$build/openssh.log" 2>&1 || { tail -40 "$build/openssh.log" >&2; exit 1; }
    fi
    llvm-strip -o "$out/libssh.so" "$build/openssh/ssh"
    # Options › Git › SSH key creates keys with it
    llvm-strip -o "$out/libssh-keygen.so" "$build/openssh/ssh-keygen"
  fi

  # ── git-lfs ───────────────────────────────────────────────────────────
  if [ -z "${SKIP_LFS:-}" ]; then
    if [ ! -f "$build/git-lfs/git-lfs" ]; then
      unpack "$(fetch "$GIT_LFS_URL" "$GIT_LFS_SHA256")" "$build/git-lfs"
      (
        cd "$build/git-lfs"
        # GOOS=android links with the NDK (bionic's resolver, TLS)
        GOOS=android GOARCH="$goarch" GOARM=7 CGO_ENABLED=1 CC="$cc" \
          GOFLAGS=-mod=mod CGO_LDFLAGS="$ldflags" \
          go build -trimpath -ldflags "-s -w" -o git-lfs .
      ) >"$build/git-lfs.log" 2>&1 || { tail -40 "$build/git-lfs.log" >&2; exit 1; }
    fi
    cp "$build/git-lfs/git-lfs" "$out/libgit-lfs.so"
  fi

  ls -l "$out"
}

for abi in $ABIS; do
  echo "== $abi"
  build_abi "$abi"
done
