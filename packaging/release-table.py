#!/usr/bin/env python3
"""Write the Downloads table of a release's notes from its assets.

    gh release view v0.1.0 --json assets,body | release-table.py --base <download url> > notes.md

Reads `{"assets": [{"name": …}], "body": …}` on stdin and prints the body
with a `## Downloads` section: platforms as rows, architectures as columns,
every asset a short link in its cell (`.dmg`, `.deb`, `foss`, …). A section
this script wrote earlier is replaced, the rest of the body is kept. The
section goes last and under a heading, so the Release Notes dialog
(`corvane_core::release_notes`) does not read it as notes.
"""

import argparse
import json
import re
import sys

ROWS = ["Windows", "macOS", "Linux", "Android"]
COLUMNS = ["x86_64", "x86 (32-bit)", "ARMv7", "ARM64", "Universal"]
# the spellings of an architecture in a file name, most specific first
ARCHITECTURES = [
    ("Universal", r"universal"),
    ("x86_64", r"x86_64|amd64|x64"),
    ("ARM64", r"aarch64|arm64"),
    ("ARMv7", r"armv7l?|armhf|arm32|aarch32"),
    ("x86 (32-bit)", r"x86_32|i[36]86|x86"),
]
START, END = "<!-- downloads -->", "<!-- /downloads -->"
# how the links of one cell are ordered
ORDER = [".dmg", ".zip", ".exe", ".msi", ".AppImage", ".deb", ".rpm", "foss", "play"]


def classify(name):
    """(row, column, label) of an asset, or None for one the table leaves out."""
    lower = name.lower()
    extension = re.search(r"\.(dmg|zip|exe|msi|appimage|deb|rpm|apk)$", lower)
    if not extension or not lower.startswith("corvane"):
        return None
    extension = extension.group(1)
    if extension == "apk" or "android" in lower:
        row = "Android"
    elif extension in ("exe", "msi") or "windows" in lower:
        row = "Windows"
    elif extension in ("appimage", "deb", "rpm") or "linux" in lower:
        row = "Linux"
    elif extension == "dmg" or "macos" in lower:
        row = "macOS"
    else:
        return None
    column = next(
        (column for column, pattern in ARCHITECTURES if re.search(rf"(?<![a-z0-9])({pattern})(?![a-z0-9])", lower)),
        # an Android package that names no ABI carries every one
        "Universal" if row == "Android" else None,
    )
    if column is None:
        return None
    if row == "Android":
        # the flavour: Corvane-<v>-android-<flavour>[-<arch>].apk
        flavour = re.search(r"android-([a-z]+)[-.]", lower)
        label = flavour.group(1) if flavour else ".apk"
    else:
        label = ".AppImage" if extension == "appimage" else f".{extension}"
    if lower.startswith("corvane-full"):
        label = f"Full {label}"
    return row, column, label


def rank(label):
    full = label.startswith("Full ")
    base = label.removeprefix("Full ")
    return (full, ORDER.index(base) if base in ORDER else len(ORDER), base)


def table(names, base):
    cells = {}
    for name in names:
        if found := classify(name):
            row, column, label = found
            cells.setdefault((row, column), []).append((label, name))
    lines = [
        "| | " + " | ".join(COLUMNS) + " |",
        "|---|" + ":---:|" * len(COLUMNS),
    ]
    for row in ROWS:
        line = []
        for column in COLUMNS:
            links = sorted(cells.get((row, column), []), key=lambda link: rank(link[0]))
            line.append(" · ".join(f"[{label}]({base}/{name})" for label, name in links) or "—")
        lines.append(f"| **{row}** | " + " | ".join(line) + " |")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", required=True, help="…/releases/download/<tag>")
    args = parser.parse_args()
    release = json.load(sys.stdin)
    section = "\n".join(
        ["## Downloads", "", START, "", table([a["name"] for a in release["assets"]], args.base.rstrip("/")), "", END]
    )
    body = (release.get("body") or "").replace("\r\n", "\n")
    old = re.compile(rf"\n*## Downloads\n+{re.escape(START)}.*?{re.escape(END)}\n*", re.S)
    body = old.sub("\n", body).strip()
    print(f"{body}\n\n{section}" if body else section)


if __name__ == "__main__":
    main()
