#!/usr/bin/env python3
"""List every test case in GitHub Desktop's `app/test` tree.

GHD's unit tests use `node:test` (`describe` / `it` / `test`). This walks
each `*-test.ts(x)` file with a small JS tokenizer (strings, template
literals and comments are skipped so braces inside them do not count) and
prints one row per case:

    <file relative to app/test>\t<describe › … › it>\t<line>

Usage: extract.py <path to desktop/app/test> > upstream.tsv
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

CALL = re.compile(r"\b(describe|it|test|suite)(?:\.(skip|only|todo))?\s*\(")


def mask(src: str) -> str:
    """Blank out comments and string bodies (keeping length and newlines).

    String delimiters stay so titles can be read back from the original
    text at the same offsets.
    """
    out = list(src)
    i, n = 0, len(src)
    # stack of brace depths at which a template literal's ${ was opened
    tmpl: list[int] = []
    depth = 0

    def blank(a: int, b: int) -> None:
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    def scan_template(i: int) -> int:
        """i points just past a backtick (or a closing } of ${}). Returns
        the index after the closing backtick, or after `${`."""
        start = i
        while i < n:
            c = src[i]
            if c == "\\":
                i += 2
                continue
            if c == "`":
                blank(start, i)
                return i + 1
            if c == "$" and i + 1 < n and src[i + 1] == "{":
                blank(start, i)
                tmpl.append(depth)
                return i + 2
            i += 1
        blank(start, n)
        return n

    prev_sig = ""
    while i < n:
        c = src[i]
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            blank(i, j)
            i = j
            continue
        if c in "'\"":
            j = i + 1
            while j < n and src[j] != c and src[j] != "\n":
                j += 2 if src[j] == "\\" else 1
            blank(i + 1, j)
            i = j + 1
            prev_sig = c
            continue
        if c == "`":
            i = scan_template(i + 1)
            prev_sig = "`"
            continue
        if c == "/" and prev_sig and prev_sig in "(,=:[!&|?{};":
            # regex literal
            j = i + 1
            in_class = False
            while j < n and src[j] != "\n":
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == "[":
                    in_class = True
                elif src[j] == "]":
                    in_class = False
                elif src[j] == "/" and not in_class:
                    break
                j += 1
            blank(i + 1, j)
            i = j + 1
            prev_sig = "/"
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            if tmpl and tmpl[-1] == depth:
                tmpl.pop()
                i = scan_template(i + 1)
                prev_sig = "`"
                continue
            depth -= 1
        if not c.isspace():
            prev_sig = c
        i += 1
    return "".join(out)


def read_title(src: str, masked: str, pos: int) -> tuple[str, int]:
    """Read the first argument of a call whose `(` is at pos-1."""
    i = pos
    while i < len(src) and src[i].isspace():
        i += 1
    if i >= len(src):
        return "", i
    q = src[i]
    if q in "'\"`":
        j = i + 1
        parts = []
        while j < len(src) and src[j] != q:
            if src[j] == "\\" and j + 1 < len(src):
                parts.append(src[j + 1])
                j += 2
                continue
            parts.append(src[j])
            j += 1
        title = "".join(parts)
        # concatenated literals: 'a' + 'b'
        k = j + 1
        m = re.match(r"\s*\+\s*(['\"`])", src[k:])
        if m:
            rest, k2 = read_title(src, masked, k + m.start(1))
            return title + rest, k2
        return title, j + 1
    # non-literal title (identifier, call): keep the raw expression
    m = re.match(r"[^,)]+", src[i:])
    return ("<" + m.group(0).strip() + ">") if m else "<expr>", i


def extract(path: Path, rel: str) -> list[tuple[str, str, int]]:
    src = path.read_text(encoding="utf-8")
    masked = mask(src)
    # brace depth at every offset
    depth_at = []
    d = 0
    for ch in masked:
        depth_at.append(d)
        if ch == "{":
            d += 1
        elif ch == "}":
            d -= 1
    rows = []
    # stack of (describe title, depth inside its body)
    stack: list[tuple[str, int]] = []
    for m in CALL.finditer(masked):
        kind = m.group(1)
        start = m.start()
        # skip member calls like foo.it( or identifiers like `submit(`
        if start > 0 and (masked[start - 1] == "." or masked[start - 1].isalnum() or masked[start - 1] == "_"):
            continue
        here = depth_at[start]
        while stack and stack[-1][1] > here:
            stack.pop()
        title, _ = read_title(src, masked, m.end())
        line = src.count("\n", 0, start) + 1
        if kind in ("describe", "suite"):
            stack.append((title, here + 1))
        else:
            path_ = " › ".join([t for t, _ in stack] + [title])
            rows.append((rel, path_, line))
    return rows


def main() -> None:
    root = Path(sys.argv[1])
    files = sorted(
        p
        for p in root.rglob("*-test.ts*")
        if "node_modules" not in p.parts and "e2e" not in p.parts
    )
    seen: dict[tuple[str, str], int] = {}
    for p in files:
        rel = p.relative_to(root).as_posix()
        for rel_, case, line in extract(p, rel):
            key = (rel_, case)
            n = seen.get(key, 0) + 1
            seen[key] = n
            if n > 1:
                case = f"{case} #{n}"
            print(f"{rel_}\t{case}\t{line}")


if __name__ == "__main__":
    main()
