#!/usr/bin/env python3
"""Launch-time, first-paint and idle-memory comparison of desktop Git clients.

    python3 tools/perf/apps.py [--runs 5] [--idle 60] [--only corvene,ghd,gitkraken] [--json out.json]

Every app is measured the same way, from outside: the executable is spawned
directly (no `open`, which returns before the app starts), and the time until
the first on-screen window of that process (CGWindowList, no Screen Recording
permission needed) is the launch time. Electron apps are also started with
`--remote-debugging-port` and asked, over the Chrome DevTools Protocol, for the
renderer's first-contentful-paint (absolute, from `performance.timeOrigin`),
since an Electron window can appear before anything is drawn in it. Corvene
draws before its window shows, so its window time is its first paint; its
`main window opened` log line is recorded too.

Idle memory is taken after `--idle` seconds on the last run: RSS (`ps`) and the
physical footprint (`footprint`, what Activity Monitor's Memory column shows)
summed over the process and its helpers (Electron's renderer, GPU and
utility processes; Corvene's git children).

Corvene runs against an isolated data directory (`CORVENE_DATA_DIR`) that
already has a repository added, so no Welcome flow shows; pass it with
`--corvene-data`. GitHub Desktop and GitKraken open whatever they opened
last. Load average is recorded with every sample: run on a quiet machine.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import socket
import statistics
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

try:
    import Quartz  # pyobjc
except ImportError:  # pragma: no cover
    sys.exit("needs pyobjc (pip install pyobjc-framework-Quartz)")

ROOT = Path(__file__).resolve().parents[2]

APPS = {
    "corvene": {
        "name": "Corvene",
        "exe": ROOT / "target" / "release" / "corvene",
        "bundle": None,
        "electron": False,
        "quit": None,  # SIGTERM
    },
    "ghd": {
        "name": "GitHub Desktop",
        "exe": Path("/Applications/GitHub Desktop.app/Contents/MacOS/GitHub Desktop"),
        "bundle": Path("/Applications/GitHub Desktop.app"),
        "electron": True,
        "quit": "GitHub Desktop",
    },
    "gitkraken": {
        "name": "GitKraken",
        "exe": Path("/Applications/GitKraken.app/Contents/MacOS/GitKraken"),
        "bundle": Path("/Applications/GitKraken.app"),
        "electron": True,
        "quit": "GitKraken",
    },
}


def load_average() -> float:
    return os.getloadavg()[0]


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def descendants(pid: int) -> list[int]:
    """pid and every process below it."""
    out = subprocess.run(["ps", "-axo", "pid=,ppid="], capture_output=True, text=True).stdout
    children: dict[int, list[int]] = {}
    for line in out.splitlines():
        p, pp = (int(x) for x in line.split())
        children.setdefault(pp, []).append(p)
    found, stack = [], [pid]
    while stack:
        p = stack.pop()
        found.append(p)
        stack.extend(children.get(p, []))
    return found


def process_memory(pids: list[int]) -> dict:
    """RSS (ps) and physical footprint (footprint) in MB, summed over pids."""
    rss_kb = 0
    live = []
    for pid in pids:
        out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        if out:
            rss_kb += int(out)
            live.append(pid)
    footprint_mb = None
    try:
        total = 0.0
        for pid in live:
            out = subprocess.run(["footprint", "-p", str(pid)], capture_output=True, text=True).stdout
            # "zsh [16395]: 64-bit    Footprint: 2176 KB (16384 bytes per page)"
            match = re.search(r"Footprint:\s+([\d.]+)\s*(KB|MB|GB)", out)
            if match:
                value = float(match.group(1))
                total += {"KB": value / 1024, "MB": value, "GB": value * 1024}[match.group(2)]
        footprint_mb = round(total, 1) if total else None
    except FileNotFoundError:
        pass
    return {"processes": len(live), "rss_mb": round(rss_kb / 1024, 1), "footprint_mb": footprint_mb}


def first_window_elapsed(pid: int, t0: float, timeout: float) -> float | None:
    """Seconds from t0 until pid has an on-screen, normal-layer window taller than 200 px."""
    deadline = t0 + timeout
    while time.perf_counter() < deadline:
        windows = Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID) or []
        for w in windows:
            if w.get("kCGWindowOwnerPID") != pid or w.get("kCGWindowLayer", 1) != 0:
                continue
            bounds = w.get("kCGWindowBounds", {})
            if bounds.get("Height", 0) > 200 and bounds.get("Width", 0) > 300:
                return time.perf_counter() - t0
        time.sleep(0.004)
    return None


def cdp_first_paint(port: int, t0_epoch_ms: float, timeout: float) -> dict:
    """The renderer's paint timings over CDP, as ms since the spawn."""
    deadline = time.time() + timeout
    targets = []
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=1) as res:
                targets = [t for t in json.load(res) if t.get("type") == "page"]
            if targets:
                break
        except OSError:
            pass
        time.sleep(0.05)
    if not targets:
        return {}
    try:
        import websocket  # websocket-client, as tools/parity uses
    except ImportError:
        return {"error": "websocket-client not installed"}
    ws = websocket.create_connection(targets[0]["webSocketDebuggerUrl"], timeout=10)
    expression = """
        (() => {
          const paint = Object.fromEntries(performance.getEntriesByType('paint').map(e => [e.name, e.startTime]));
          return JSON.stringify({ timeOrigin: performance.timeOrigin, paint, ready: document.readyState });
        })()
    """
    result = {}
    while time.time() < deadline:
        ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {"expression": expression, "returnByValue": True}}))
        reply = json.loads(ws.recv())
        value = reply.get("result", {}).get("result", {}).get("value")
        if value:
            data = json.loads(value)
            paint = data["paint"]
            if "first-contentful-paint" in paint:
                origin = data["timeOrigin"]
                result = {
                    "renderer_start_ms": round(origin - t0_epoch_ms),
                    "first_paint_ms": round(origin + paint.get("first-paint", 0) - t0_epoch_ms),
                    "first_contentful_paint_ms": round(origin + paint["first-contentful-paint"] - t0_epoch_ms),
                }
                break
        time.sleep(0.05)
    ws.close()
    return result


def quit_app(app: dict, proc: subprocess.Popen) -> None:
    """Ask politely (AppleScript quit), then terminate, then kill the whole tree."""
    if app["quit"]:
        try:
            subprocess.run(["osascript", "-e", f'tell application "{app["quit"]}" to quit'], capture_output=True, timeout=8)
        except subprocess.TimeoutExpired:
            pass  # GitKraken does not answer the Apple event; fall through
    try:
        proc.wait(8)
    except subprocess.TimeoutExpired:
        proc.terminate()
        try:
            proc.wait(8)
        except subprocess.TimeoutExpired:
            pass
    for pid in descendants(proc.pid):
        try:
            os.kill(pid, 9)
        except ProcessLookupError:
            pass
    try:
        proc.wait(5)
    except subprocess.TimeoutExpired:
        pass
    # Electron helpers may outlive the main process for a moment
    time.sleep(2)


def wait_for_quiet(max_load: float, timeout: float) -> float:
    """Block until the 1-minute load average drops under max_load (or timeout passes); returns the load."""
    deadline = time.time() + timeout
    load = load_average()
    while load > max_load and time.time() < deadline:
        print(f"  load {load:.0f} > {max_load:.0f}, waiting…", flush=True)
        time.sleep(30)
        load = load_average()
    return load


def corvene_window_log(stdout_log: Path, since: float) -> int | None:
    """Corvene's own `main window opened elapsed_ms=…` line: stderr in debug builds, ~/Library/Logs/Corvene in release ones."""
    candidates = [stdout_log]
    logs_dir = Path.home() / "Library" / "Logs" / "Corvene"
    if logs_dir.is_dir():
        candidates += sorted(logs_dir.glob("corvene.log*"), key=lambda f: f.stat().st_mtime)[-1:]
    for log in candidates:
        try:
            if log.stat().st_mtime < since:
                continue
            matches = re.findall(r"main window opened.*?elapsed_ms[=: ]+(\d+)|elapsed_ms[=: ]+(\d+).*?main window opened", log.read_text(errors="replace"))
        except OSError:
            continue
        if matches:
            a, b = matches[-1]
            return int(a or b)
    return None


def launch_once(key: str, app: dict, run: int, args, corvene_data: Path | None, measure_memory: bool) -> tuple[dict, dict | None]:
    """One launch: window and paint times, and (on request) idle memory after `--idle` seconds."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("CORVENE_")}
    cmd = [str(app["exe"])]
    port = None
    log = Path(args.workdir) / f"{key}-{run}.log"
    if app["electron"]:
        port = free_port()
        cmd += [f"--remote-debugging-port={port}", "--remote-allow-origins=*"]
    if key == "corvene":
        if corvene_data:
            env["CORVENE_DATA_DIR"] = str(corvene_data)
        env["CORVENE_LOG"] = "info"
    load = wait_for_quiet(args.max_load, args.max_load_wait) if args.max_load else load_average()
    memory = None
    with open(log, "wb") as log_file:
        t0_epoch_ms = time.time() * 1000
        t0 = time.perf_counter()
        proc = subprocess.Popen(cmd, env=env, stdout=log_file, stderr=subprocess.STDOUT)
        window = first_window_elapsed(proc.pid, t0, timeout=90)
        sample = {
            "run": run,
            "load_1m": round(load, 1),
            "window_ms": round(window * 1000) if window is not None else None,
        }
        if app["electron"] and port:
            sample.update(cdp_first_paint(port, t0_epoch_ms, timeout=90))
        # let the app finish loading its repository before it is quit
        time.sleep(8)
        if key == "corvene":
            sample["log_window_ms"] = corvene_window_log(log, t0_epoch_ms / 1000)
        print(f"  {app['name']} run {run}", json.dumps(sample), flush=True)
        if measure_memory:
            print(f"  {app['name']}: idling {args.idle}s for memory…", flush=True)
            time.sleep(args.idle)
            memory = process_memory(descendants(proc.pid))
            memory["load_1m"] = round(load_average(), 1)
            print(f"  {app['name']} memory", json.dumps(memory), flush=True)
        quit_app(app, proc)
    return sample, memory


def summarise(app: dict, samples: list[dict], memory: dict | None) -> dict:
    windows = [s["window_ms"] for s in samples if s["window_ms"] is not None]
    fcps = [s["first_contentful_paint_ms"] for s in samples if s.get("first_contentful_paint_ms")]
    logs = [s["log_window_ms"] for s in samples if s.get("log_window_ms")]
    if app["bundle"]:
        out = subprocess.run(["du", "-sk", str(app["bundle"])], capture_output=True, text=True).stdout.split()
        size_mb = round(int(out[0]) / 1024) if out else None
    else:
        size_mb = round(app["exe"].stat().st_size / 1024 / 1024, 1)
    return {
        "name": app["name"],
        "samples": samples,
        "window_ms_median": round(statistics.median(windows)) if windows else None,
        "window_ms_min": min(windows) if windows else None,
        "first_contentful_paint_ms_median": round(statistics.median(fcps)) if fcps else None,
        "first_contentful_paint_ms_min": min(fcps) if fcps else None,
        "log_window_ms_median": round(statistics.median(logs)) if logs else None,
        "memory": memory,
        "installed_size_mb": size_mb,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--idle", type=int, default=60)
    parser.add_argument("--only", default="corvene,ghd,gitkraken")
    parser.add_argument("--corvene-data", type=Path, default=None, help="CORVENE_DATA_DIR with a repository added")
    parser.add_argument("--workdir", type=Path, default=ROOT / "target" / "perf" / "apps")
    parser.add_argument("--json", type=Path, default=None)
    parser.add_argument("--max-load", type=float, default=0, help="wait for the 1-minute load average to drop under this before each launch")
    parser.add_argument("--max-load-wait", type=float, default=1800, help="seconds to wait for a quiet machine at most")
    args = parser.parse_args()
    args.workdir.mkdir(parents=True, exist_ok=True)

    results = {
        "machine": subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip(),
        "macos": subprocess.run(["sw_vers", "-productVersion"], capture_output=True, text=True).stdout.strip(),
        "date": time.strftime("%Y-%m-%d"),
        "apps": {},
    }
    keys = [k for k in args.only.split(",") if APPS[k]["exe"].exists()]
    for k in args.only.split(","):
        if k not in keys:
            print(f"{APPS[k]['name']}: not installed, skipped", flush=True)
    # round-robin, so every app meets the same machine conditions
    samples: dict[str, list[dict]] = {k: [] for k in keys}
    memory: dict[str, dict | None] = {k: None for k in keys}
    for run in range(1, args.runs + 1):
        print(f"\n== round {run} of {args.runs} ==", flush=True)
        for k in keys:
            sample, mem = launch_once(k, APPS[k], run, args, args.corvene_data, measure_memory=run == args.runs)
            samples[k].append(sample)
            if mem:
                memory[k] = mem
    for k in keys:
        results["apps"][k] = summarise(APPS[k], samples[k], memory[k])
        print(k, json.dumps({x: y for x, y in results["apps"][k].items() if x != "samples"}), flush=True)
    if args.json:
        args.json.write_text(json.dumps(results, indent=2))
        print("\nwrote", args.json)
    return 0


if __name__ == "__main__":
    sys.exit(main())
