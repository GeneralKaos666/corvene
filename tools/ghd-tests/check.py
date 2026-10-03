#!/usr/bin/env python3
"""Check that every GitHub Desktop test case is ported or skipped.

Sources of truth:

- `upstream.tsv` — every `it()` / `test()` in GHD's `app/test`, from
  `extract.py` (file, describe › … › case, line).
- `// GHD: <file> › <describe › … › case>` comments in Rust sources. The
  comment sits right above a test (its attributes and `fn`). One test may
  carry several markers and one case may be marked on several tests. A
  test attribute (`#[test]`, `#[gpui::test]`…) must stand between the
  marker and the `fn`. An `#[ignore = "ghd: <kind>: <why>"]` there makes
  the case "ignored" (ported, but not passing yet) with that reason; kinds:
  `bug`, `missing`, `todo`, `deviation`, `env`.
- `skips/*.tsv` — `<file>\t<case or *>\t<reason>` for cases that are not
  ported. `*` skips the whole file. Reasons start with a kind:
  `n/a:` (Electron/React/TypeScript-only), `omitted:` (feature left out
  by design), `todo:` (a `.docs/TODO.md` item), `deviation:` (a
  `.docs/deviations.md` difference, `deviation: NNN-slug: …` when it has a
  flag).

Malformed reasons, markers that name no upstream case or stand above
something other than a test, and cases both ported and skipped are errors.

Usage:
  check.py              summary, exit 1 if anything is unaccounted for
  check.py --missing    list cases neither ported nor skipped
  check.py --ignored    list ported-but-ignored cases with reasons
  check.py --file F     per-case status for one GHD test file
  check.py --write      also rewrite coverage.tsv
"""

from __future__ import annotations

import argparse
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
MARKER = re.compile(r"^\s*//\s*GHD:\s*(\S+?)\s+›\s+(.+?)\s*$")
FN = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")
# `#[ignore]`, `#[ignore = "…"]` and `#[cfg_attr(<cfg>, ignore = "…")]`
IGNORE = re.compile(
    r'#\[(?:cfg_attr\([^\]]*?,\s*)?ignore(?:\s*=\s*"((?:[^"\\]|\\.)*)")?\s*\)?\]'
)
TEST_ATTR = re.compile(r"^\s*#\[\s*(?:\w+::)*test\b")
IGNORE_REASON = re.compile(r"^ghd: (bug|missing|todo|deviation|env): \S")
SKIP_REASON = re.compile(r"^(n/a|omitted|todo|deviation):\s*\S")


def load_upstream() -> list[tuple[str, str]]:
    rows = []
    for line in (HERE / "upstream.tsv").read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        file, case, _line = line.split("\t")
        # markers and skips are read with trailing whitespace stripped
        rows.append((file, case.rstrip()))
    return rows


def load_skips() -> tuple[dict[tuple[str, str], str], dict[str, str], list[str]]:
    cases: dict[tuple[str, str], str] = {}
    files: dict[str, str] = {}
    errors: list[str] = []
    for path in sorted((HERE / "skips").glob("*.tsv")):
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not line.strip() or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) != 3 or not parts[2].strip():
                errors.append(f"{path.name}:{n}: expected <file>\\t<case|*>\\t<reason>")
                continue
            file, case, reason = (p.strip() for p in parts)
            if not SKIP_REASON.match(reason):
                errors.append(
                    f"{path.name}:{n}: reason must start with n/a:, omitted:, todo: or deviation:"
                )
            if case == "*":
                files[file] = reason
            else:
                cases[(file, case)] = reason
    return cases, files, errors


def load_markers(
    errors: list[str],
) -> dict[tuple[str, str], list[tuple[str, str | None]]]:
    """(file, case) -> [(where, ignore reason or None)]"""
    found: dict[tuple[str, str], list[tuple[str, str | None]]] = defaultdict(list)
    for rs in sorted((ROOT / "crates").rglob("*.rs")):
        if "target" in rs.parts:
            continue
        text = rs.read_text(encoding="utf-8", errors="replace")
        if "GHD:" not in text:
            continue
        lines = text.splitlines()
        pending: list[tuple[str, str, int]] = []
        ignore: str | None = None
        is_test = False
        attr_depth = 0  # open brackets of an attribute spanning lines
        for i, line in enumerate(lines):
            m = MARKER.match(line)
            if m:
                pending.append((m.group(1), m.group(2), i + 1))
                continue
            if not pending:
                continue
            stripped = line.strip()
            if attr_depth > 0:
                attr_depth += stripped.count("[") - stripped.count("]")
                continue
            if stripped.startswith("#"):
                attr_depth = max(stripped.count("[") - stripped.count("]"), 0)
            if not stripped.startswith("//"):
                im = IGNORE.search(line)
                if im:
                    ignore = im.group(1) or ""
                    if not IGNORE_REASON.match(ignore):
                        errors.append(
                            f"{rs.relative_to(ROOT).as_posix()}:{i + 1}: ignore reason must be"
                            ' "ghd: <bug|missing|todo|deviation|env>: <why>" on one line'
                        )
                if TEST_ATTR.match(line):
                    is_test = True
            fm = FN.match(line)
            if fm and is_test:
                where = f"{rs.relative_to(ROOT).as_posix()}:{fm.group(1)}"
                for file, case, _ in pending:
                    found[(file, case)].append((where, ignore))
                pending, ignore, is_test = [], None, False
            elif fm or (stripped and not stripped.startswith(("#", "//"))):
                # marker not followed by a test fn
                for file, case, ln in pending:
                    found[(file, case)].append(
                        (f"{rs.relative_to(ROOT).as_posix()}:{ln}:DANGLING", None)
                    )
                pending, ignore, is_test = [], None, False
    return found


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--missing", action="store_true")
    ap.add_argument("--ignored", action="store_true")
    ap.add_argument("--file")
    ap.add_argument("--write", action="store_true")
    args = ap.parse_args()

    upstream = load_upstream()
    known = set(upstream)
    known_files = {f for f, _ in upstream}
    skip_cases, skip_files, errors = load_skips()
    markers = load_markers(errors)

    status: list[tuple[str, str, str, str]] = []
    for file, case in upstream:
        marks = markers.get((file, case))
        if marks:
            dangling = [w for w, _ in marks if w.endswith(":DANGLING")]
            if dangling:
                errors.append(f"marker not above a #[test] fn: {dangling[0]}")
            ignored = [(w, r) for w, r in marks if r is not None]
            passing = [w for w, r in marks if r is None and not w.endswith(":DANGLING")]
            if passing and not ignored:
                status.append((file, case, "ported", ", ".join(passing)))
            elif ignored:
                w, r = ignored[0]
                status.append((file, case, "ignored", f"{r} ({w})"))
            continue
        reason = skip_cases.get((file, case)) or skip_files.get(file)
        if reason:
            status.append((file, case, "skipped", reason))
        else:
            status.append((file, case, "missing", ""))

    for key in markers:
        if key not in known:
            where = markers[key][0][0]
            errors.append(f"marker names no upstream case: {key[0]} › {key[1]} ({where})")
    for key in skip_cases:
        if key not in known:
            errors.append(f"skip names no upstream case: {key[0]} › {key[1]}")
        elif key in markers:
            errors.append(f"case both ported and skipped: {key[0]} › {key[1]}")
    for file in skip_files:
        if file not in known_files:
            errors.append(f"skip names no upstream file: {file}")

    if args.file:
        for file, case, st, info in status:
            if file == args.file:
                print(f"{st:8} {case}  {info}")
        return 0
    if args.missing:
        for file, case, st, _ in status:
            if st == "missing":
                print(f"{file}\t{case}")
    if args.ignored:
        for file, case, st, info in status:
            if st == "ignored":
                print(f"{file}\t{case}\t{info}")

    counts = Counter(st for _, _, st, _ in status)
    kinds = Counter(
        info.split(":", 1)[0].split()[0]
        for _, _, st, info in status
        if st == "skipped"
    )
    print(
        f"{len(upstream)} upstream cases: {counts['ported']} ported, "
        f"{counts['ignored']} ignored, {counts['skipped']} skipped "
        f"({', '.join(f'{k} {v}' for k, v in sorted(kinds.items()))}), "
        f"{counts['missing']} missing",
        file=sys.stderr,
    )
    for e in errors:
        print(f"error: {e}", file=sys.stderr)

    if args.write:
        with (HERE / "coverage.tsv").open("w", encoding="utf-8") as out:
            out.write("file\tcase\tstatus\twhere or reason\n")
            for row in status:
                out.write("\t".join(row) + "\n")

    return 1 if counts["missing"] or errors else 0


if __name__ == "__main__":
    sys.exit(main())
