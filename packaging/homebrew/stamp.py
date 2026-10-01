#!/usr/bin/env python3
"""Rewrite `version` and `sha256` values in the cask (Casks/corvane.rb).

    stamp.py [--version V] [--macos SHA] [--x86_64-linux SHA] [--arm64-linux SHA]

`--macos` fills both `arm` and `intel` (one universal zip). Values left out
keep what the cask has, so the macOS and Linux halves of a release can stamp
it in turn. `--cask` names another file than the one next to this script.
"""

import argparse
import pathlib
import re
import sys

parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
parser.add_argument("--cask", default=pathlib.Path(__file__).parent / "Casks/corvane.rb")
parser.add_argument("--version")
parser.add_argument("--macos")
parser.add_argument("--x86_64-linux")
parser.add_argument("--arm64-linux")
args = parser.parse_args()

path = pathlib.Path(args.cask)
text = path.read_text()


def replace(pattern, value):
    global text
    text, count = re.subn(pattern, lambda m: m.group(1) + value + m.group(2), text, flags=re.M)
    if count != 1:
        sys.exit(f"{path}: expected one match for {pattern!r}, found {count}")


def sha(key, value):
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        sys.exit(f"not a sha256: {value!r}")
    replace(rf'^(\s+(?:sha256 )?{key}:\s+")[0-9a-f]{{64}}(")', value)


if args.version:
    replace(r'^(  version ")[^"]+(")', args.version)
if args.macos:
    sha("arm", args.macos)
    sha("intel", args.macos)
if args.x86_64_linux:
    sha("x86_64_linux", args.x86_64_linux)
if args.arm64_linux:
    sha("arm64_linux", args.arm64_linux)

path.write_text(text)
print(f"updated {path}")
