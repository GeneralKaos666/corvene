#!/usr/bin/env python3
"""Build the offline language extension index (crates/corvene-extensions/src/index.rs):
file suffix / file name -> the registries' extensions covering it, so the
diff's "no syntax highlighting" hint answers without a network round trip.

    python3 tools/ext-index/crawl.py [out.json] [--pages N]

Sources (open registries only; never the Visual Studio Marketplace):
- Open VSX: the Programming Languages category by downloads; each
  extension's `__ext_<suffix>` tags (from its `contributes.languages`).
- Zed: the extension listing (`provides` holds `languages` / `grammars`)
  joined with Zed's own suggestion table (crates/extension_suggest in
  zed-industries/zed), the only place Zed records file suffixes.
- Pulsar: `language-*` packages by downloads; the suffix is taken from the
  package name (the registry has no suffix index and reading every
  package's grammars would hammer GitHub).

The workflow .github/workflows/ext-index.yml runs this weekly and publishes
`language-extensions-index.json.gz` on the rolling `packs` release.
"""

from __future__ import annotations

import datetime as dt
import json
import re
import sys
import time
import urllib.parse
import urllib.request

USER_AGENT = "Corvene-ext-index/1.0 (+https://github.com/wasi-master/corvene)"
MAX_PER_SUFFIX = 8


def get_json(url: str, retries: int = 3):
    for attempt in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=60) as resp:
                return json.load(resp)
        except Exception as err:  # noqa: BLE001
            if attempt + 1 == retries:
                print(f"warning: {url}: {err}", file=sys.stderr)
                return None
            time.sleep(2 * (attempt + 1))
    return None


def get_text(url: str) -> str | None:
    try:
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.read().decode("utf-8", "replace")
    except Exception as err:  # noqa: BLE001
        print(f"warning: {url}: {err}", file=sys.stderr)
        return None


def candidate(registry: str, id: str, name: str, display: str, **rest) -> dict:
    out = {"registry": registry, "id": id, "name": name, "display_name": display, "download_url": rest.pop("download_url")}
    out.update({k: v for k, v in rest.items() if v is not None})
    return out


def open_vsx(pages: int, suffixes: dict, names: dict) -> None:
    size = 100
    for page in range(pages):
        data = get_json(
            "https://open-vsx.org/api/-/search?"
            + urllib.parse.urlencode({"category": "Programming Languages", "size": size, "offset": page * size, "sortBy": "downloadCount", "sortOrder": "desc"})
        )
        if not data or not data.get("extensions"):
            break
        for ext in data["extensions"]:
            detail = get_json(f"https://open-vsx.org/api/{ext['namespace']}/{ext['name']}")
            if not detail:
                continue
            tags = [t for t in detail.get("tags", []) if t.startswith("__ext_")]
            if not tags:
                continue
            c = candidate(
                "open-vsx",
                f"{ext['namespace']}.{ext['name']}",
                ext["name"],
                detail.get("displayName") or ext["name"],
                publisher=ext["namespace"],
                version=detail.get("version"),
                description=(detail.get("description") or "")[:200] or None,
                repository=detail.get("repository"),
                download_url=detail["files"]["download"],
                grammar="text-mate",
                suffixes=[t[len("__ext_"):].lower() for t in tags],
                downloads=int(detail.get("downloadCount") or 0),
            )
            for s in c["suffixes"]:
                suffixes.setdefault(s, []).append(c)
            time.sleep(0.2)


def zed(suggestions: dict[str, list[str]], suffixes: dict, names: dict) -> None:
    data = get_json("https://api.zed.dev/extensions?max_schema_version=1")
    if not data:
        return
    by_id: dict[str, dict] = {}
    for ext in data.get("data", []):
        if not ({"languages", "grammars"} & set(ext.get("provides", []))):
            continue
        by_id.setdefault(ext["id"], ext)
    for ext_id, ext in by_id.items():
        c = candidate(
            "zed",
            ext_id,
            ext_id,
            ext.get("name") or ext_id,
            publisher=(ext.get("authors") or [None])[0],
            version=ext.get("version"),
            description=(ext.get("description") or "")[:200] or None,
            repository=ext.get("repository"),
            download_url=f"https://api.zed.dev/extensions/{ext_id}/{ext.get('version')}/download",
            grammar="tree-sitter",
            suffixes=[s.lower() for s in suggestions.get(ext_id, [])],
            downloads=int(ext.get("download_count") or 0),
        )
        for s in suggestions.get(ext_id, []):
            if s.startswith(".") or (s and s[0].isupper()) or "." in s and not s.islower():
                names.setdefault(s.lower(), []).append(c)
            else:
                suffixes.setdefault(s.lower(), []).append(c)


def zed_suggestions() -> dict[str, list[str]]:
    text = get_text("https://raw.githubusercontent.com/zed-industries/zed/main/crates/extension_suggest/src/extension_suggest.rs")
    if not text:
        return {}
    body = text.split("SUGGESTIONS_BY_EXTENSION_ID")[1].split("];")[0]
    out: dict[str, list[str]] = {}
    for ext_id, items in re.findall(r'\(\s*"([^"]+)"\s*,\s*&\[([^\]]*)\]', body, re.S):
        out[ext_id] = re.findall(r'"([^"]+)"', items)
    return out


def pulsar(pages: int, suffixes: dict, names: dict) -> None:
    for page in range(1, pages + 1):
        data = get_json(f"https://api.pulsar-edit.dev/api/packages?page={page}&sort=downloads&direction=desc")
        if not data:
            break
        for pkg in data:
            name = pkg.get("name", "")
            if not name.startswith("language-"):
                continue
            suffix = name[len("language-"):].lower()
            version = (pkg.get("releases") or {}).get("latest") or (pkg.get("metadata") or {}).get("version")
            if not version:
                continue
            repo = pkg.get("repository")
            repo = repo.get("url") if isinstance(repo, dict) else repo
            deps = (pkg.get("metadata") or {}).get("dependencies") or {}
            c = candidate(
                "pulsar",
                name,
                name,
                name,
                publisher=None,
                version=version,
                description=((pkg.get("metadata") or {}).get("description") or "")[:200] or None,
                repository=repo,
                download_url=f"https://api.pulsar-edit.dev/api/packages/{urllib.parse.quote(name)}/versions/{urllib.parse.quote(version)}/tarball",
                grammar="tree-sitter" if any(k.startswith("tree-sitter") for k in deps) else "text-mate",
                suffixes=[suffix],
                downloads=int(pkg.get("downloads") or 0),
            )
            suffixes.setdefault(suffix, []).append(c)
        time.sleep(0.5)


def main(argv: list[str]) -> int:
    out = argv[0] if argv and not argv[0].startswith("--") else "language-extensions-index.json"
    pages = int(argv[argv.index("--pages") + 1]) if "--pages" in argv else 10
    suffixes: dict[str, list[dict]] = {}
    names: dict[str, list[dict]] = {}
    open_vsx(pages, suffixes, names)
    zed(zed_suggestions(), suffixes, names)
    pulsar(pages, suffixes, names)
    for table in (suffixes, names):
        for key, items in table.items():
            items.sort(key=lambda c: -c.get("downloads", 0))
            table[key] = items[:MAX_PER_SUFFIX]
    index = {
        "schema": 1,
        "generated": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "suffixes": dict(sorted(suffixes.items())),
        "filenames": dict(sorted(names.items())),
    }
    with open(out, "w") as f:
        json.dump(index, f, separators=(",", ":"))
    print(f"{out}: {len(suffixes)} suffixes, {len(names)} file names")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
