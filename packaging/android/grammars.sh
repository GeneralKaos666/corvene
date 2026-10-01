#!/bin/bash
# Build the play flavour's grammar module: every tree-sitter grammar unit as
# a native library in grammars/src/main/jniLibs, with the index the loader
# reads (corvane_highlight::treesitter::load_pack) next to them.
#
#   packaging/android/grammars.sh
#
# Needs ANDROID_NDK_HOME and the grammar sources (tools/ts-queries/fetch.py,
# with the tree-sitter CLI for the grammars without a committed parser.c).
# Env: ABIS (default "arm64-v8a x86_64"), UNITS (default: all).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ABIS="${ABIS:-arm64-v8a x86_64}"
OUT="$ROOT/packaging/android/grammars/src/main/jniLibs"
NDK_BIN="$(dirname "$(find "${ANDROID_NDK_HOME:?needs ANDROID_NDK_HOME}/toolchains/llvm/prebuilt" -name llvm-nm | head -1)")"
export CORVANE_CC="$NDK_BIN/clang" CORVANE_CXX="$NDK_BIN/clang++"
UNITS="${UNITS:-$(python3 "$ROOT/tools/ts-queries/gen.py" --units all | cut -d' ' -f1)}"

rm -rf "$OUT"
for abi in $ABIS; do
  case "$abi" in
    arm64-v8a) target=aarch64-linux-android26 ;;
    x86_64) target=x86_64-linux-android26 ;;
    *) echo "unknown ABI $abi" >&2; exit 2 ;;
  esac
  mkdir -p "$OUT/$abi"
  built=()
  for unit in $UNITS; do
    lib="$OUT/$abi/libcorvane_ts_$unit.so"
    if python3 "$ROOT/tools/ts-queries/build_unit.py" "$unit" "$lib" --target "$target" 2>/dev/null; then
      built+=("$unit")
    else
      echo "warning: the $unit grammar did not build for $abi; left out" >&2
      rm -f "$lib"
    fi
  done
  # the pack index, its unit files renamed to the libraries above
  python3 "$ROOT/tools/ts-queries/gen.py" --ext so --index all "${built[@]}" |
    python3 -c '
import json, os, sys
index = json.load(sys.stdin)
for unit in index["units"]:
    name = os.path.basename(unit["file"]).split(".")[0]
    unit["file"] = "libcorvane_ts_" + name + ".so"
json.dump(index, sys.stdout)
' > "$OUT/$abi/libcorvane_ts_index.so"
  echo "$abi: ${#built[@]} units, $(du -sh "$OUT/$abi" | cut -f1)"
done
