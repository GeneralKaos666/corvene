#!/usr/bin/env python3
"""Interaction latency of GitHub Desktop and GitKraken on the `big` fixture,
to set against tools/perf/bench.py's Corvene numbers.

    python3 tools/perf/fixture.py big target/perf/big
    python3 tools/perf/apps_latency.py [--repo target/perf/big] [--runs 5] [--only ghd,gitkraken] [--json out.json]

GitHub Desktop runs as a private instance under the Chrome DevTools Protocol
(tools/parity/drivers.Ghd: fresh profile, welcome skipped, the repository
added through its own `open-repository` CLI action). Input is real
Chromium input (`Input.dispatchMouseEvent`, `Input.dispatchKeyEvent`), and
the time is measured inside the renderer: from the input event's own
`timeStamp` to the `requestAnimationFrame` whose frame first shows the diff
of the file that was selected (its path in the diff header, the diff body no
longer loading and holding rows that differ from the previous file's). That
is the same definition `bench.py` uses for Corvene (input until the frame
that draws the requested state) at the renderer's frame granularity.

Cases, as in bench.py: `select file (click)` on rows 3, 0 and 5; `next file (↓)`
five times from row 0; `open 5k-line diff` by clicking `src/big.rs` after
another file.

GitKraken is launched with `-p <repo>` and `--remote-debugging-port` (it
keeps its own profile; no isolation flag exists), the work-in-progress row of
its graph is clicked to open the files panel, and a file row opens its Monaco
diff editor. Its file list has no keyboard navigation, so there is no
"next file (↓)" case for it.
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "parity"))
from drivers import Ghd  # noqa: E402

W, H = 1367, 814

# In-page watcher: arms on the next mousedown/keydown, then polls every frame
# until the diff header names TARGET and the diff body shows new rows.
WATCHER = """
(() => {
  const target = %(target)s;
  const header = () => %(header)s;
  const body = () => {
    const rows = document.querySelectorAll(%(rows)s);
    return { loading: %(loading)s, rows: rows.length,
             first: (rows[0]?.textContent ?? '') + '|' + (rows[Math.min(rows.length - 1, 5)]?.textContent ?? '') };
  };
  const before = { header: header(), ...body() };
  const m = window.__latency = { target, before, t0: null, done: null, frames: 0 };
  const arm = (e) => { if (m.t0 === null) m.t0 = e.timeStamp; };
  document.addEventListener('mousedown', arm, { capture: true, once: true });
  document.addEventListener('keydown', arm, { capture: true, once: true });
  const tick = (ts) => {
    m.frames++;
    if (m.t0 !== null) {
      const h = header(); const b = body();
      // the header follows the selection at once while the body still shows the previous file: wait for new rows
      const changed = h !== null && (h === target || h.endsWith(target)) && !b.loading && b.rows > 0 && b.first !== before.first;
      if (changed) { m.done = ts; m.after = { header: h, ...b }; return; }
    }
    if (m.frames < 100000) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  return JSON.stringify(before);
})()
"""

GHD_DOM = {
    "header": "document.querySelector('.diff-container .header .path-text-component')?.textContent ?? null",
    "rows": "'.side-by-side-diff .row, .diff-code-mirror .CodeMirror-line'",
    "loading": "(() => { const sw = document.querySelector('.seamless-diff-switcher'); return !!sw && /is-loading|loading/.test(sw.className); })()",
}
# GitKraken: the diff is a Monaco diff editor; its header spells the path as "dir / name"
GK_DOM = {
    "header": "document.querySelector('.file-name')?.textContent.replace(/\\s+/g, '') ?? null",
    "rows": "'.monaco-diff-editor .editor.modified .view-line'",
    "loading": "false",
}

ROW_RECT = """
(() => {
  const rows = [...document.querySelectorAll('#changes-list .list-item')];
  const row = rows.find(r => r.querySelector('.path-text-component')?.textContent === %(path)s);
  if (!row) return null;
  const r = row.getBoundingClientRect();
  return { x: r.left + 120, y: r.top + r.height / 2 };
})()
"""


def click(call, x: float, y: float) -> None:
    """A real left click through CDP: move, press (with the button state set), a short hold, release."""
    call("Input.dispatchMouseEvent", type="mouseMoved", x=x, y=y)
    time.sleep(0.05)
    call("Input.dispatchMouseEvent", type="mousePressed", x=x, y=y, button="left", buttons=1, clickCount=1)
    time.sleep(0.01)
    call("Input.dispatchMouseEvent", type="mouseReleased", x=x, y=y, button="left", buttons=0, clickCount=1)


class GhdLatency:
    def __init__(self, repo: Path, runs: int, workdir: Path):
        self.repo, self.runs = repo, runs
        self.workdir = workdir
        self.ghd: Ghd | None = None
        self.results: dict[str, list[float]] = {}

    def start(self):
        profile = Path(tempfile.mkdtemp(prefix="ghd-perf-", dir=self.workdir))
        self.ghd = Ghd(profile, profile / "ghd.log")
        self.ghd.start()
        self.ghd.configure(W, H, 2.0, {"has-shown-welcome-flow": "1"}, freeze=False)
        if not self.ghd.add_repository(self.repo):
            raise RuntimeError("GitHub Desktop did not add the repository")
        self.ghd.wait_for("document.querySelectorAll('#changes-list .list-item').length > 5", 120)
        time.sleep(3)  # let the first diff and status settle

    def stop(self):
        if self.ghd:
            self.ghd.stop()
            self.ghd = None

    # -- helpers --------------------------------------------------------------
    def files(self) -> list[str]:
        return self.ghd.eval(
            "[...document.querySelectorAll('#changes-list .list-item')]"
            ".map(r => r.querySelector('.path-text-component')?.textContent).filter(Boolean)"
        )

    def arm(self, target: str):
        self.ghd.eval(WATCHER % {"target": json.dumps(target), **GHD_DOM})

    def result(self, timeout: float = 30) -> float:
        deadline = time.time() + timeout
        while time.time() < deadline:
            m = self.ghd.eval("JSON.stringify(window.__latency)")
            m = json.loads(m) if m else {}
            if m.get("done") is not None and m.get("t0") is not None:
                return m["done"] - m["t0"]
            time.sleep(0.01)
        raise RuntimeError(f"timeout waiting for {json.loads(self.ghd.eval('JSON.stringify(window.__latency)'))}")

    def click_row(self, path: str):
        rect = self.ghd.eval(ROW_RECT % {"path": json.dumps(path)})
        if not rect:
            raise RuntimeError(f"row {path} is not rendered")
        click(self.ghd.call, rect["x"], rect["y"])

    def key_down_arrow(self):
        for kind in ("keyDown", "keyUp"):
            self.ghd.call("Input.dispatchKeyEvent", type=kind, key="ArrowDown", code="ArrowDown", windowsVirtualKeyCode=40, nativeVirtualKeyCode=40)

    def shown(self) -> str | None:
        return self.ghd.eval("document.querySelector('.diff-container .header .path-text-component')?.textContent ?? null")

    def select(self, path: str) -> float:
        if self.shown() == path:
            files = self.files()
            index = files.index(path)
            self.select(files[index + 1 if index + 1 < len(files) else index - 1])
        self.arm(path)
        self.click_row(path)
        return self.result()

    def record(self, case: str, ms: float):
        self.results.setdefault(case, []).append(ms)
        print(f"  {case}: {ms:.0f} ms", flush=True)

    # -- cases ----------------------------------------------------------------
    def run(self):
        files = self.files()
        print(f"  {len(files)} rows rendered; first {files[:3]}", flush=True)
        for _ in range(self.runs):
            # select file (click): rows 3, 0, 5 as bench.py
            for i in (3, 0, 5):
                self.record("select file (click)", self.select(files[i]))
            # next file (↓): from row 0, five times
            self.select(files[0])
            for i in range(1, 6):
                self.arm(files[i])
                self.key_down_arrow()
                self.record("next file (↓)", self.result())
            # open 5k-line diff
            big = "src/big.rs" if "src/big.rs" in files else files[0]
            other = files[1] if big == files[0] else files[0]
            self.select(other)
            self.record("open 5k-line diff", self.select(big))

    def summary(self) -> dict:
        return {
            case: {"median_ms": round(statistics.median(v)), "p90_ms": round(sorted(v)[max(0, int(len(v) * 0.9) - 1)]), "min_ms": round(min(v)), "n": len(v)}
            for case, v in self.results.items()
        }


class GitKrakenLatency:
    """GitKraken under CDP: the WIP row opens the files panel, a file row the Monaco diff."""

    EXE = Path("/Applications/GitKraken.app/Contents/MacOS/GitKraken")

    def __init__(self, repo: Path, runs: int, workdir: Path):
        self.repo, self.runs, self.workdir = repo, runs, workdir
        self.proc = None
        self.ws = None
        self._id = 0
        self.results: dict[str, list[float]] = {}

    def start(self):
        import subprocess
        import urllib.request

        import websocket

        from drivers import free_port

        # GitKraken is single-instance: a second launch hands over to the first and exits
        running = subprocess.run(["pgrep", "-f", "/Applications/GitKraken.app/Contents/MacOS/GitKraken"], capture_output=True, text=True).stdout.split()
        if running:
            raise RuntimeError(f"GitKraken is already running (pid {running[0]}): quit it first")
        port = free_port()
        log = open(self.workdir / "gitkraken.log", "ab")
        self.proc = subprocess.Popen(
            [str(self.EXE), f"--remote-debugging-port={port}", "--remote-allow-origins=*", "-p", str(self.repo)],
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        deadline = time.time() + 120
        while time.time() < deadline:
            try:
                tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=1))
                pages = [t for t in tabs if t.get("type") == "page" and t.get("title") == "GitKraken Desktop"]
                if pages:
                    self.ws = websocket.create_connection(pages[0]["webSocketDebuggerUrl"], suppress_origin=True, timeout=60)
                    break
            except OSError:
                pass
            time.sleep(0.25)
        else:
            raise RuntimeError("GitKraken did not expose its page over CDP")
        self.wait_for("document.querySelectorAll('.graph-row').length > 10", 120)
        time.sleep(3)
        # the work-in-progress row carries the changed-file count readout
        rect_js = (
            "(() => { const r = document.querySelector('.tiny-files-readout')?.closest('.graph-row'); if (!r) return null;"
            " const b = r.getBoundingClientRect(); return { x: b.left + 100, y: b.top + b.height / 2 }; })()"
        )
        opened = "document.querySelectorAll('.unstage .file-node-list .file-node').length > 2"
        for attempt in range(6):
            rect = self.eval(rect_js)
            if not rect:
                time.sleep(2)
                continue
            # the row's label area, left of the changed-files readout, selects the row
            self.click_at(rect["x"], rect["y"])
            try:
                self.wait_for(opened, 10)
                break
            except RuntimeError:
                continue
        else:
            raise RuntimeError("GitKraken did not open its work-in-progress files panel")
        time.sleep(2)

    def stop(self):
        import os
        import signal

        if self.ws:
            self.ws.close()
            self.ws = None
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(8)
            except Exception:
                os.kill(self.proc.pid, signal.SIGKILL)
            self.proc = None
        time.sleep(2)

    # -- protocol -------------------------------------------------------------
    def call(self, method: str, **params):
        self._id += 1
        self.ws.send(json.dumps({"id": self._id, "method": method, "params": params}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == self._id:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})

    def eval(self, expression: str):
        r = self.call("Runtime.evaluate", expression=expression, returnByValue=True, awaitPromise=True)
        if "exceptionDetails" in r:
            raise RuntimeError(f"eval failed: {r['exceptionDetails']}")
        return r.get("result", {}).get("value")

    def wait_for(self, expression: str, timeout: float) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.eval(f"!!({expression})"):
                return True
            time.sleep(0.15)
        raise RuntimeError(f"timeout waiting for {expression}")

    def click_at(self, x: float, y: float):
        click(self.call, x, y)

    # -- cases ----------------------------------------------------------------
    # the unstaged list only: the staged files sit in a second grid with the same classes
    GRID = "document.querySelector('.unstage .ReactVirtualized__Grid.file-node-list')"
    # a row's path without the "Stage File" overlay buttons a hovered row shows
    NAME = (
        "(n => { const c = n.querySelector('.file-node-contents') || n; const o = c.querySelector('.file-node-overlay-buttons');"
        " return (c.innerText || '').replace(o ? (o.innerText || '') : '', '').replace(/\\s+/g, ''); })"
    )
    ROW_PX = 32

    def rendered(self) -> list[str]:
        return self.eval(f"[...document.querySelectorAll('.unstage .file-node-list .file-node')].map({self.NAME})")

    def files(self) -> list[str]:
        """Every unstaged file, in list order: the list is virtualized and only a few rows tall, so scroll through it."""
        names: list[str] = []
        top = 0
        height = self.eval(f"{self.GRID}.scrollHeight") or 0
        while top < height:
            self.eval(f"{self.GRID}.scrollTop = {top}")
            time.sleep(0.15)
            for name in self.rendered():
                if name not in names:
                    names.append(name)
            top += self.ROW_PX * 3
        self.eval(f"{self.GRID}.scrollTop = 0")
        time.sleep(0.2)
        return names

    def shown(self) -> str | None:
        return self.eval("document.querySelector('.file-name')?.textContent.replace(/\\s+/g, '') ?? null")

    def select(self, path: str) -> float:
        files = getattr(self, "_files", None) or self.files()
        self._files = files
        index = files.index(path)
        shown = self.shown()
        if shown and shown.endswith(path):
            # already on screen: nothing would change; show a neighbour first
            self.select(files[index + 1 if index + 1 < len(files) else index - 1])
        # the panel shows about three rows: scroll the row to the top of the list, then click it
        self.eval(f"{self.GRID}.scrollTop = {index * self.ROW_PX}")
        rect_js = (
            "(() => { const n = [...document.querySelectorAll('.unstage .file-node-list .file-node')]"
            f".find(n => {self.NAME}(n) === {json.dumps(path)}); if (!n) return null;"
            " const b = n.getBoundingClientRect(); const l = n.closest('.file-node-list').getBoundingClientRect();"
            " const y = b.top + b.height / 2; if (y < l.top + 1 || y > l.bottom - 1) return null;"
            # react-virtualized turns pointer events off while it considers the grid scrolling
            " const inner = document.querySelector('.unstage .ReactVirtualized__Grid__innerScrollContainer');"
            " if (inner && getComputedStyle(inner).pointerEvents === 'none') return null;"
            " return { x: b.left + 120, y, listHeight: l.height }; })()"
        )
        rect = None
        for _ in range(30):
            time.sleep(0.1)
            rect = self.eval(rect_js)
            if rect:
                break
        time.sleep(0.2)
        if not rect:
            raise RuntimeError(f"GitKraken row {path} is not rendered")
        self.eval(WATCHER % {"target": json.dumps(path), **GK_DOM})
        self.click_at(rect["x"], rect["y"])
        deadline = time.time() + 60
        try:
            while time.time() < deadline:
                m = json.loads(self.eval("JSON.stringify(window.__latency)") or "{}")
                if m.get("done") is not None and m.get("t0") is not None:
                    return m["done"] - m["t0"]
                time.sleep(0.01)
            raise RuntimeError(f"timeout waiting for {path}")
        finally:
            # park the pointer over the diff so no row shows its hover buttons
            self.call("Input.dispatchMouseEvent", type="mouseMoved", x=500, y=500)

    def record(self, case: str, ms: float):
        self.results.setdefault(case, []).append(ms)
        print(f"  {case}: {ms:.0f} ms", flush=True)

    def run(self):
        files = self._files = self.files()
        print(f"  {len(files)} unstaged files; first {files[:3]}", flush=True)
        for _ in range(self.runs):
            for i in (3, 0, 5):
                self.record("select file (click)", self.select(files[i]))
            # GitKraken's file list has no keyboard navigation: no "next file (↓)" case
            big = "src/big.rs" if "src/big.rs" in files else files[0]
            other = files[1] if big == files[0] else files[0]
            self.select(other)
            self.record("open 5k-line diff", self.select(big))

    summary = GhdLatency.summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, default=ROOT / "target" / "perf" / "big")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--only", default="ghd,gitkraken")
    parser.add_argument("--workdir", type=Path, default=ROOT / "target" / "perf" / "apps")
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args()
    args.workdir.mkdir(parents=True, exist_ok=True)
    out = {}
    if "ghd" in args.only:
        print("== GitHub Desktop ==", flush=True)
        bench = GhdLatency(args.repo, args.runs, args.workdir)
        bench.start()
        try:
            bench.run()
        finally:
            bench.stop()
        out["ghd"] = bench.summary()
        print(json.dumps(out["ghd"], indent=1))
    if "gitkraken" in args.only:
        print("== GitKraken ==", flush=True)
        bench = GitKrakenLatency(args.repo, args.runs, args.workdir)
        bench.start()
        try:
            bench.run()
        finally:
            bench.stop()
        out["gitkraken"] = bench.summary()
        print(json.dumps(out["gitkraken"], indent=1))
    if args.json:
        args.json.write_text(json.dumps(out, indent=2))
        print("wrote", args.json)
    return 0


if __name__ == "__main__":
    sys.exit(main())
