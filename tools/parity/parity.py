#!/usr/bin/env python3
"""GitHub Desktop ⇄ Corvene 1:1 parity harness.

Runs YAML scenarios (tools/parity/scenarios) against a private GitHub Desktop
instance and a Corvene instance side by side: same window size, same default
panel widths, same fixture repository, same input at the same window points.
Every `snap` step captures both apps, diffs them and fails the step when the
difference exceeds its threshold. Results: `<out>/index.html` (side by side,
swipe, blink, diff overlay, per-region crops with offset / colour hints and
the GHD DOM path of each region) and `<out>/results.json`.

    python3 tools/parity/parity.py                 # every scenario, dark + light
    python3 tools/parity/parity.py toolbar dialogs --themes dark
    python3 tools/parity/parity.py --list

Exit status 1 when any snap exceeds its threshold (or a scenario errors).
See tools/parity/README.md.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import shutil
import subprocess
import sys
import threading
import time
import traceback
from datetime import datetime
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import accounts  # noqa: E402
import fixture  # noqa: E402
import imgdiff  # noqa: E402
import report  # noqa: E402
from drivers import Corvene, Ghd, page_height, page_rect, park_pointer  # noqa: E402


class Absent:
    """Stands in for Corvene under `--ghd-only` (spec extraction runs)."""

    name = "corvene"
    scale = 2.0

    def __getattr__(self, _name):
        return lambda *args, **kwargs: {"w": 0, "h": 0}

DEFAULTS = {
    "width": 1367,
    "height": 814,
    "threshold": 1.0,  # % of differing pixels allowed per snap
    "tolerance": 6,  # max channel delta still equal on flat pixels (fills, borders, hover backgrounds)
    "edge_tolerance": 40,  # … and next to high-contrast edges (glyph anti-aliasing)
    "radius": 1.0,  # points of positional slack (anti-aliasing / subpixel text)
    "settle": 350,  # ms after every action
}


def load_scenarios(names: list[str]) -> list[dict]:
    out = []
    for path in sorted((HERE / "scenarios").glob("*.yaml")):
        for doc in yaml.safe_load_all(path.read_text()):
            if not doc:
                continue
            doc.setdefault("name", path.stem)
            doc["_file"] = path.name
            # `platforms: [win32]`: what only one platform's GHD has
            if "platforms" in doc and sys.platform not in doc["platforms"]:
                continue
            if not names or any(fnmatch.fnmatch(doc["name"], n) or fnmatch.fnmatch(path.stem, n) for n in names):
                out.append(doc)
    return out


def both(fa, fb):
    """Run two callables concurrently; re-raise the first failure."""
    errors = []

    def wrap(f):
        try:
            f()
        except BaseException as e:  # noqa: BLE001
            errors.append(e)

    ts = [threading.Thread(target=wrap, args=(f,)) for f in (fa, fb)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    if errors:
        raise errors[0]


class Run:
    def __init__(self, args):
        self.args = args
        self.out = Path(args.out)
        self.binary = Path(args.corvene)

    # -- one scenario in one theme ---------------------------------------
    def scenario(self, sc: dict, theme: str) -> dict:
        cfg = {**DEFAULTS, **{k: v for k, v in sc.items() if k in DEFAULTS}}
        if self.args.size:
            # a screen too small for the scenarios' window (Windows does not
            # let a window outgrow it): both apps at a size that fits
            cfg["width"], cfg["height"] = (int(n) for n in self.args.size.lower().split("x"))
        for k in ("threshold", "tolerance", "edge_tolerance", "radius", "settle"):
            if getattr(self.args, k) is not None:
                cfg[k] = getattr(self.args, k)
        slug = f"{sc['name']}-{theme}"
        work = self.out / "work" / slug
        shots = self.out / "shots" / slug
        if work.exists():
            fixture.remove_tree(work)
        shots.mkdir(parents=True, exist_ok=True)
        (work / "logs").mkdir(parents=True)
        setup = sc.get("setup", "repo")
        # parents "n" / "u": one glyph apart and the same advance in SF (1079
        # units), so paths shown in either app lay out and wrap identically
        # `repo-remote`: the same fixture behind a bare `origin`, two commits
        # ahead of it; `repo-coauthors`: with commits by several people on top
        # `repo-graph`: with merges on top (the commit graph)
        with_repo = setup in ("repo", "repo-remote", "repo-coauthors", "repo-graph")
        remote = setup == "repo-remote"
        coauthors = setup == "repo-coauthors"
        graph = setup == "repo-graph"
        repo_g = fixture.build(work / "n", remote, coauthors, graph) if with_repo else None
        repo_c = fixture.build(work / "u", remote, coauthors, graph) if with_repo else None

        ghd = Ghd(work / "ghd-profile", work / "logs" / "ghd.log", sc.get("ghd_env"))
        cv = Absent() if self.args.ghd_only else Corvene(self.binary, work / "corvene-data", work / "logs" / "corvene.log", theme,
                                                         sc.get("corvene_flags"))
        result = {"name": sc["name"], "theme": theme, "file": sc["_file"], "description": sc.get("description", ""), "snaps": [], "error": None, "notes": []}
        started = time.time()
        try:
            both(ghd.start, cv.start)
            if abs(ghd.scale - cv.scale) > 0.01:
                result["notes"].append(f"scale differs: GHD {ghd.scale} vs Corvene {cv.scale}")
            ls = {"theme": theme}
            if setup != "welcome":
                ls["has-shown-welcome-flow"] = "1"
            ls.update(sc.get("ghd_local_storage", {}))

            def setup_ghd():
                ghd.configure(cfg["width"], page_height(cfg["height"]), cv.scale, ls, freeze=not self.args.no_freeze)
                if repo_g and not ghd.add_repository(repo_g):
                    raise RuntimeError("GHD: could not add the fixture repository")
                time.sleep(1.2)

            def setup_cv():
                info = cv.resize(cfg["width"], page_height(cfg["height"]))
                if not self.args.ghd_only and (
                    int(info["w"]) != cfg["width"] or int(info["h"]) != page_height(cfg["height"])
                ):
                    result["notes"].append(f"Corvene viewport is {info['w']}x{info['h']}")
                if setup != "welcome":
                    cv.hook("complete-welcome")
                if repo_c:
                    cv.hook("add-repo", str(repo_c))
                if not self.args.ghd_only:
                    time.sleep(2.0)

            both(setup_ghd, setup_cv)
            self.fixtures = [r for r in (repo_g, repo_c) if r]
            for i, step in enumerate(sc.get("steps", [])):
                self.step(i, step, ghd, cv, cfg, shots, result)
                if self.args.fail_fast and any(not s["pass"] for s in result["snaps"]):
                    break
        except Exception as e:  # noqa: BLE001
            result["error"] = f"{type(e).__name__}: {e}"
            result["traceback"] = traceback.format_exc()
        finally:
            if self.args.keep_open:
                input(f"[{slug}] both apps left open - press Enter to close them… ")
            elif self.args.hold_open:
                ghd.set_menu_pop(True)
                print(f"    holding both apps open for {self.args.hold_open}s (GHD menus pop for real)", flush=True)
                time.sleep(self.args.hold_open)
            both(ghd.stop, cv.stop)
            if not self.args.keep_work:
                shutil.rmtree(work / "ghd-profile", ignore_errors=True)
        result["seconds"] = round(time.time() - started, 1)
        result["pass"] = result["error"] is None and all(s["pass"] for s in result["snaps"])
        return result

    # -- steps -----------------------------------------------------------
    def step(self, i, step: dict, ghd: Ghd, cv: Corvene, cfg, shots: Path, result):
        step = dict(step)
        wait = step.pop("wait", None)
        per_app = {"ghd": step.pop("ghd", None), "corvene": step.pop("corvene", None)}
        if "snap" in step:
            self.snap(step["snap"], i, ghd, cv, cfg, shots, result)
            return
        if "context_menu" in step:
            # both apps' last contextual menu, compared as item lists
            name = step["context_menu"] if isinstance(step["context_menu"], str) else f"menu{i}"
            g, c = ghd.menu_items(), cv.menu_items() if not self.args.ghd_only else []
            # an empty GHD menu means the right-click missed: never a pass
            ok = bool(g) and (self.args.ghd_only or g == c)
            result.setdefault("menus", []).append({"name": name, "ghd": g, "corvene": c, "pass": ok})
            if not ok:
                result["snaps"].append({"name": f"menu: {name}", "stem": "", "note": "native menu items differ",
                                        "percent": 100.0, "coverage": 0.0, "threshold": 0, "pass": False,
                                        "size_mismatch": "", "ghd": "", "corvene": "", "diff": "", "regions": [],
                                        "menu": {"ghd": g, "corvene": c}})
            print(f"    {'ok  ' if ok else 'FAIL'} menu {name}: {len(g)} GHD / {len(c)} Corvene items", flush=True)
            # Corvene's real menu held its main thread; let queued work land
            time.sleep((wait if wait is not None else cfg["settle"]) / 1000)
            return
        if "context_menu_pick" in step:
            label = step["context_menu_pick"]
            ghd.pick_menu(label)
            if not self.args.ghd_only:
                cv.pick_menu(label)
            time.sleep((wait if wait is not None else cfg["settle"]) / 1000)
            return
        if "context_menu_dismiss" in step:
            ghd.dismiss_menu()
            return
        if "dump" in step:
            d = step["dump"]
            d = {"name": d} if isinstance(d, str) else d
            path = shots / f"{i:02d}-{d['name']}-ghd-dom.json"
            path.write_text(json.dumps(ghd.dump(d.get("root", "body")), indent=1))
            result.setdefault("dumps", []).append(path.name)
            return
        if "fixture" in step:
            # `fixture: move`: both apps' fixture directories disappear (the
            # missing repository view); nothing is sent to either app
            if step["fixture"] != "move":
                raise ValueError(f"unknown fixture action {step['fixture']!r}")
            for repo in self.fixtures:
                repo.rename(repo.with_name(repo.name + "-moved"))
            return
        if not step and not any(per_app.values()) and wait is not None:
            time.sleep(wait / 1000)
            return
        # targets resolve in GHD's DOM; both apps get the same point
        resolved = {}
        for key in ("hover", "click", "dblclick", "rclick", "press", "release"):
            if key in step:
                resolved[key] = ghd.resolve(step[key])
        for app, drv in (("ghd", ghd), ("corvene", cv)):
            action = per_app[app] if per_app[app] is not None else step
            if isinstance(action, dict) and action.get("skip"):
                continue
            self.apply(action, resolved, drv, ghd)
        time.sleep((wait if wait is not None else cfg["settle"]) / 1000)

    def apply(self, action: dict, resolved: dict, drv, ghd: Ghd):
        mods = action.get("mods", "")
        pt = lambda key: resolved.get(key) or ghd.resolve(action[key])  # noqa: E731
        if "hover" in action:
            drv.move(*pt("hover"), mods=mods)
        if "click" in action:
            drv.click(*pt("click"), clicks=action.get("clicks", 1), mods=mods)
        if "dblclick" in action:
            drv.click(*pt("dblclick"), clicks=2, mods=mods)
        if "rclick" in action:
            # opens a contextual menu: GHD's is recorded, Corvene's pops,
            # is recorded and closes itself (compare with `context_menu`)
            if drv.name == "ghd":
                drv.eval("window.__parityMenu=null")
            drv.click(*pt("rclick"), button="right", mods=mods)
        if "press" in action:
            drv.down(*pt("press"), mods=mods)
        if "release" in action:
            x, y = pt("release")
            drv.move(x, y, mods=mods, pressed=True)
            drv.up(x, y, mods=mods)
        if "drag" in action:
            d = action["drag"]
            (x, y), (x2, y2) = ghd.resolve(d["from"]), ghd.resolve(d["to"])
            drv.drag(x, y, x2, y2, d.get("steps", 12))
        if "scroll" in action:
            s = action["scroll"]
            x, y = ghd.resolve(s["at"])
            drv.scroll(x, y, s.get("dx", 0), s.get("dy", 0))
        if "key" in action:
            drv.key(action["key"])
        if "type" in action:
            drv.type(action["type"])
        if "menu" in action:
            drv.menu(action["menu"])
        if "popup" in action:
            p = action["popup"]
            name = p.get(drv.name) if isinstance(p, dict) else p
            if name:
                drv.popup(name)
        if "accounts" in action:
            # fake signed-in accounts (`accounts.py`)
            if drv.name == "ghd":
                drv.eval(accounts.ghd_js(action["accounts"]))
            else:
                drv.hook("fake-accounts", accounts.corvene_arg(action["accounts"]))
        if "eval" in action and drv.name == "ghd":
            drv.eval(action["eval"])
        if "action" in action and drv.name == "corvene":
            drv.cmd("action", name=action["action"])
        if "hook" in action and drv.name == "corvene":
            h = action["hook"]
            drv.hook(h["name"], h.get("arg", ""))
        if "resize" in action:
            w, h = action["resize"]
            if drv.name == "ghd":
                drv.resize(w, h)
            else:
                drv.resize(w, h)

    @staticmethod
    def stable_snap(drv, path: Path, timeout: float = 4.0, interval: float = 0.25):
        """Capture until two consecutive frames agree (async work such as
        syntax highlighting or avatars has landed). A few hundred changed
        pixels (a blinking caret) still count as stable."""
        import numpy as np
        from PIL import Image

        tmp = path.with_suffix(".prev.png")
        drv.snap(path)
        prev = np.asarray(Image.open(path).convert("RGB"))
        deadline = time.time() + timeout
        while time.time() < deadline:
            time.sleep(interval)
            drv.snap(tmp)
            cur = np.asarray(Image.open(tmp).convert("RGB"))
            changed = cur.shape != prev.shape or int((np.abs(cur.astype(int) - prev).max(axis=2) > 8).sum()) > 400
            tmp.replace(path)
            if not changed:
                return
            prev = cur

    def snap(self, spec, i, ghd: Ghd, cv: Corvene, cfg, shots: Path, result):
        spec = {"name": spec} if isinstance(spec, str) else dict(spec)
        name = spec.get("name", f"step{i}")
        stem = f"{i:02d}-{name}"
        pg, pc = shots / f"{stem}-ghd.png", shots / f"{stem}-corvene.png"
        if self.args.ghd_only:
            ghd.snap(pg)
            result["snaps"].append({"name": name, "stem": stem, "note": spec.get("note", ""), "ghd_only": True,
                                    "percent": 0.0, "coverage": 0.0, "threshold": 0, "pass": True, "size_mismatch": "",
                                    "ghd": pg.name, "corvene": "", "diff": "", "regions": []})
            print(f"    snap {name}", flush=True)
            return
        if spec.get("corvene_only"):
            # a Corvene-only surface (a dialog GHD lacks): recorded, not compared
            self.stable_snap(cv, pc)
            result["snaps"].append({"name": name, "stem": stem, "note": spec.get("note", ""), "corvene_only": True,
                                    "percent": 0.0, "coverage": 0.0, "threshold": 0, "pass": True, "size_mismatch": "",
                                    "ghd": "", "corvene": pc.name, "diff": "", "regions": []})
            print(f"    snap {name} (Corvene only)", flush=True)
            return
        both(lambda: self.stable_snap(ghd, pg), lambda: self.stable_snap(cv, pc))
        threshold = spec.get("threshold", cfg["threshold"])
        res = imgdiff.compare(
            pg, pc, shots, stem, cv.scale,
            tolerance=spec.get("tolerance", cfg["tolerance"]),
            edge_tolerance=spec.get("edge_tolerance", cfg["edge_tolerance"]),
            radius_pt=spec.get("radius", cfg["radius"]),
            masks=[page_rect(m) for m in spec.get("mask") or []] or None,
            region=page_rect(spec.get("region")),
        )
        for reg in res.regions:
            try:
                reg.element = ghd.describe(reg.x + reg.w / 2, reg.y + reg.h / 2)
            except Exception:  # noqa: BLE001
                pass
        entry = {
            "name": name,
            "stem": stem,
            "note": spec.get("note", ""),
            "percent": round(res.percent, 3),
            "coverage": round(res.coverage, 2),
            "threshold": threshold,
            "pass": res.percent <= threshold,
            "size_mismatch": res.size_mismatch,
            "ghd": pg.name,
            "corvene": pc.name,
            "diff": f"{stem}-diff.png",
            "regions": [
                {
                    "rect": [round(r.x, 1), round(r.y, 1), round(r.w, 1), round(r.h, 1)],
                    "share": round(r.share, 3),
                    "hint": r.hint(),
                    "shift": r.shift,
                    "ghd_color": r.ghd_color,
                    "corvene_color": r.corvene_color,
                    "element": r.element,
                    "crop": r.crop,
                }
                for r in res.regions
            ],
        }
        result["snaps"].append(entry)
        mark = "ok  " if entry["pass"] else "FAIL"
        print(f"    {mark} {name:<34} {entry['percent']:7.3f}% px (≤ {threshold}%)  {entry['coverage']:6.2f}% blocks", flush=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("scenarios", nargs="*", help="scenario names or file stems (globs ok)")
    ap.add_argument("--themes", default="dark,light")
    ap.add_argument("--out", default=str(ROOT / "target" / "parity" / datetime.now().strftime("%Y%m%d-%H%M%S")))
    ap.add_argument(
        "--corvene",
        default=str(ROOT / "target" / "debug" / ("corvene.exe" if sys.platform == "win32" else "corvene")),
    )
    ap.add_argument("--build", action="store_true", help="cargo build -p corvene --features snapshots first")
    ap.add_argument("--size", metavar="WxH", help="window size for both apps instead of the scenarios' 1367x814")
    ap.add_argument("--threshold", type=float)
    ap.add_argument("--tolerance", type=int)
    ap.add_argument("--edge-tolerance", type=int)
    ap.add_argument("--radius", type=float)
    ap.add_argument("--settle", type=int)
    ap.add_argument("--no-freeze", action="store_true", help="keep GHD CSS transitions")
    ap.add_argument("--fail-fast", action="store_true")
    ap.add_argument("--keep-open", action="store_true", help="pause before closing the apps")
    ap.add_argument("--hold-open", type=int, default=0, metavar="SECONDS",
                    help="keep both apps open this long after the steps (manual / screen-capture passes)")
    ap.add_argument("--keep-work", action="store_true", help="keep profiles / data dirs")
    ap.add_argument("--ghd-only", action="store_true", help="drive GHD alone: captures and DOM dumps, no comparison")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    scenarios = load_scenarios(args.scenarios)
    if args.list:
        for sc in scenarios:
            print(f"{sc['name']:<32} {sc['_file']:<24} {sc.get('description', '')}")
        return 0
    if not scenarios:
        print("no scenarios matched", file=sys.stderr)
        return 2
    if args.build:
        subprocess.run(["cargo", "build", "-p", "corvene", "--features", "snapshots"], cwd=ROOT, check=True)
    if not args.ghd_only and not Path(args.corvene).exists():
        print(f"{args.corvene} missing: cargo build -p corvene --features snapshots (or --build)", file=sys.stderr)
        return 2

    park_pointer()
    run = Run(args)
    run.out.mkdir(parents=True, exist_ok=True)
    # one run at a time: two runs fight over focus, ports and GHD's shared helpers
    lock = open(run.out.parent / ".lock", "w")
    try:
        if sys.platform == "win32":
            import msvcrt

            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl

            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print("another parity run is in progress", file=sys.stderr)
        return 2
    latest = run.out.parent / "latest"
    if latest.is_symlink() or latest.exists():
        latest.unlink()
    try:
        latest.symlink_to(run.out.name)
    except OSError:
        # Windows without Developer Mode: no symlinks; a text file names the run
        latest.write_text(run.out.name)
    results = []
    for theme in [t.strip() for t in args.themes.split(",") if t.strip()]:
        for sc in scenarios:
            print(f"[{theme}] {sc['name']}", flush=True)
            r = run.scenario(sc, theme)
            if r["error"]:
                print(f"    ERROR {r['error']}", flush=True)
            results.append(r)
            report.write(run.out, results, DEFAULTS)
    failed = [r for r in results if not r["pass"]]
    snaps = [s for r in results for s in r["snaps"]]
    print(f"\n{len(snaps) - sum(not s['pass'] for s in snaps)}/{len(snaps)} snaps within threshold; "
          f"{len(failed)} of {len(results)} scenario runs failing")
    print(f"report: {run.out / 'index.html'}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
