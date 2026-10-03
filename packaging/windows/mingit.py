#!/usr/bin/env python3
"""Writes packaging/windows/mingit.iss: the MinGit release the Windows
installer offers when it finds no git on the PC (corvene.iss, the `mingit`
task), pinned by file name, size and sha256 for each architecture.

    packaging/windows/mingit.py [--tag v2.56.0.windows.1]

Without --tag the latest Git for Windows release is taken. The three MinGit
zips are downloaded (~40 MB each) to compute what Inno Setup needs: the
sha256 Setup checks the download against (`Hash`) and the uncompressed size
(`ExternalSize`, the progress bar's share for the extraction). GITHUB_TOKEN,
when set, is sent to the GitHub API. Run again to move to a newer git; the
.iss is checked in.
"""

import argparse
import hashlib
import io
import json
import os
import sys
import urllib.request
import zipfile
from pathlib import Path

REPO = "git-for-windows/git"
# Inno Setup's /DArch value -> the MinGit asset's architecture suffix
ARCHES = {"x86_64": "64-bit", "i686": "32-bit", "aarch64": "arm64"}


def api(url: str) -> dict:
    request = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json"})
    if token := os.environ.get("GITHUB_TOKEN"):
        request.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=600) as response:
        return response.read()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--tag", help="a Git for Windows release tag (default: the latest)")
    args = parser.parse_args()
    release = api(
        f"https://api.github.com/repos/{REPO}/releases/tags/{args.tag}"
        if args.tag
        else f"https://api.github.com/repos/{REPO}/releases/latest"
    )
    tag = release["tag_name"]
    # v2.56.0.windows.1 -> 2.56.0
    version = tag.removeprefix("v").split(".windows.")[0]
    assets = {asset["name"]: asset for asset in release["assets"]}
    entries = []
    for arch, suffix in ARCHES.items():
        name = f"MinGit-{version}-{suffix}.zip"
        asset = assets.get(name)
        if asset is None:
            print(f"{tag} has no {name}", file=sys.stderr)
            return 1
        print(f"downloading {name} ({asset['size']} bytes)", file=sys.stderr)
        data = fetch(asset["browser_download_url"])
        sha256 = hashlib.sha256(data).hexdigest()
        listed = asset.get("digest", "")
        if listed and listed != f"sha256:{sha256}":
            print(f"{name}: downloaded {sha256}, GitHub lists {listed}", file=sys.stderr)
            return 1
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            names = archive.namelist()
            if "cmd/git.exe" not in names:
                print(f"{name} has no cmd/git.exe at its root", file=sys.stderr)
                return 1
            extracted = sum(info.file_size for info in archive.infolist())
        entries.append((arch, name, extracted, sha256))

    lines = [
        "; The MinGit the installer offers when no git is on the PC (corvene.iss,",
        "; the `mingit` task), written by mingit.py: do not edit by hand.",
        f'#define MinGitVersion "{version}"',
        f'#define MinGitTag "{tag}"',
        f'#define MinGitUrl "https://github.com/{REPO}/releases/download/{tag}/"',
    ]
    for index, (arch, name, extracted, sha256) in enumerate(entries):
        lines.append(f'#{"if" if index == 0 else "elif"} Arch == "{arch}"')
        lines.append(f'  #define MinGitFile "{name}"')
        lines.append(f"  #define MinGitExtractedSize {extracted}")
        lines.append(f'  #define MinGitSha256 "{sha256}"')
    lines.append("#else")
    lines.append('  #error no MinGit for this architecture')
    lines.append("#endif")
    out = Path(__file__).resolve().parent / "mingit.iss"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\r\n")
    print(f"wrote {out} ({tag})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
