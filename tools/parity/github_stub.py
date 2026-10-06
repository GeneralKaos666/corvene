"""A stub GitHub REST API for the parity harness (`github_stub: true`).

Corvene's Issues and Releases views (flags `345-issues`, `346-releases`)
read and write through the real API client, which no scenario can point at
GitHub (no account, no network). `start()` serves fixed answers on a free
port, as a GitHub Enterprise Server would under `/api/v3`; the `github:
stub` step then runs the `fake-github` control hook, which signs Corvene in
to `http://127.0.0.1:<port>/api/v3` with an injected token and makes the
fixture repository the stub's `octocat/parity-fixture`.

Dates are fixed far in the past, so relative times stay the same from run
to run ("last year"). A created issue is `#101`; a created release gets
id 9. `generate-notes` answers for any tag; with `GITHUB_STUB_NO_NOTES=1`
it answers 404 (a GitHub Enterprise Server before 3.5), so Corvene builds
the notes from the local history instead.
"""

from __future__ import annotations

import json
import os
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

OWNER, NAME = "octocat", "parity-fixture"


def _user(login: str) -> dict:
    return {"login": login, "id": hash(login) & 0xFFFF, "avatar_url": None, "html_url": f"https://github.com/{login}"}


LABELS = [
    {"name": "bug", "color": "d73a4a", "description": "Something isn't working"},
    {"name": "documentation", "color": "0075ca", "description": "Improvements or additions to documentation"},
    {"name": "enhancement", "color": "a2eeef", "description": "New feature or request"},
    {"name": "good first issue", "color": "7057ff", "description": "Good for newcomers"},
]
ASSIGNEES = [_user("octocat"), _user("mona"), _user("hubot")]


def _issue(number: int, title: str, state: str, created: str, labels: list[str], assignees: list[str],
           comments: int = 0, body: str | None = None) -> dict:
    by_name = {l["name"]: l for l in LABELS}
    return {
        "number": number, "id": 1000 + number, "node_id": f"I_stub{number}", "title": title, "state": state,
        "state_reason": "completed" if state == "closed" else None,
        "body": body if body is not None else f"### Steps\n\n1. Open the app\n2. Watch it\n\nSee #{max(number - 1, 1)} for the history.",
        "html_url": f"http://127.0.0.1/{OWNER}/{NAME}/issues/{number}",
        "created_at": created, "updated_at": created, "closed_at": created if state == "closed" else None,
        "user": _user("octocat"), "labels": [by_name[l] for l in labels], "assignees": [_user(a) for a in assignees],
        "comments": comments,
    }


OPEN_ISSUES = [
    _issue(42, "Crash on launch with an empty repository list", "open", "2024-03-01T10:00:00Z", ["bug"], ["octocat"], 2),
    _issue(41, "Dark mode for the tutorial panel", "open", "2024-02-20T10:00:00Z", ["enhancement", "good first issue"], [], 1),
    _issue(38, "Document the release process", "open", "2024-02-02T10:00:00Z", ["documentation"], ["mona"], 0),
]
CLOSED_ISSUES = [
    _issue(37, "Typo in the welcome screen", "closed", "2024-01-05T10:00:00Z", ["bug"], ["octocat"], 3),
]


def _release(rid: int, tag: str, name: str, draft: bool, prerelease: bool, created: str, assets: bool = True) -> dict:
    web = f"http://127.0.0.1/{OWNER}/{NAME}"
    return {
        "id": rid, "tag_name": tag, "name": name, "draft": draft, "prerelease": prerelease,
        "body": (f"## What's Changed\n* Keep scroll position on refresh by @octocat in #{rid * 3}\n"
                 f"* Show commit signature status by @mona in #{rid * 3 + 1}\n\n**Full Changelog**: {web}/compare/v0.0.9...{tag}"),
        "html_url": f"{web}/releases/tag/{tag}", "author": _user("octocat"),
        "created_at": created, "published_at": None if draft else created, "target_commitish": "main",
        "tarball_url": f"{web}/archive/refs/tags/{tag}.tar.gz",
        "assets": [] if not assets else [
            {"name": f"Corvene-{tag.lstrip('v')}.dmg", "size": 24117248, "download_count": 128,
             "browser_download_url": f"{web}/releases/download/{tag}/Corvene.dmg"},
            {"name": "SHA256SUMS", "size": 412, "download_count": 9,
             "browser_download_url": f"{web}/releases/download/{tag}/SHA256SUMS"},
        ],
    }


RELEASES = [
    _release(3, "v0.2.0", "", True, False, "2024-04-01T10:00:00Z", assets=False),
    _release(2, "v0.1.1-beta.1", "Beta 1", False, True, "2024-03-10T10:00:00Z"),
    _release(1, "v0.1.0", "First release", False, False, "2024-02-15T10:00:00Z"),
]


class _Handler(BaseHTTPRequestHandler):
    server_version = "github-stub/1"

    def log_message(self, *_args):
        pass

    def _send(self, status: int, body, headers: dict | None = None) -> None:
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("X-GitHub-Request-Id", "STUB:1")
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(data)

    def _body(self) -> dict:
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        try:
            return json.loads(raw or b"{}")
        except json.JSONDecodeError:
            return {}

    def do_GET(self):  # noqa: N802
        path, _, query = self.path.partition("?")
        repo = f"/api/v3/repos/{OWNER}/{NAME}"
        params = dict(p.split("=", 1) for p in query.split("&") if "=" in p)
        if path == "/api/v3/user":
            # the token's scopes: Corvene's sign-in, without `write:public_key`
            # (Settings › Integrations' Add to GitHub then asks to sign in again,
            # flag 350)
            return self._send(200, {**_user("octocat"), "name": "Mona Lisa Octocat", "plan": {"name": "free"}},
                              {"X-OAuth-Scopes": ", ".join(self.server.scopes)})
        if path == repo:
            return self._send(200, {"name": NAME, "owner": _user(OWNER), "html_url": f"http://127.0.0.1/{OWNER}/{NAME}",
                                    "clone_url": f"http://127.0.0.1/{OWNER}/{NAME}.git", "default_branch": "main",
                                    "private": False, "fork": False, "parent": None, "node_id": "R_stub",
                                    "permissions": {"admin": True, "push": True, "pull": True}})
        if path == f"{repo}/issues":
            state = params.get("state", "open")
            issues = {"open": OPEN_ISSUES, "closed": CLOSED_ISSUES, "all": OPEN_ISSUES + CLOSED_ISSUES}[state]
            return self._send(200, issues + self.server.created_issues if state != "closed" else issues)
        if path == f"{repo}/labels":
            return self._send(200, LABELS)
        if path == f"{repo}/assignees":
            return self._send(200, ASSIGNEES)
        if path == f"{repo}/releases":
            return self._send(200, self.server.created_releases + RELEASES)
        if path.startswith(f"{repo}/releases/tags/"):
            tag = path.rsplit("/", 1)[1]
            for r in self.server.created_releases + RELEASES:
                if r["tag_name"] == tag:
                    return self._send(200, r)
            return self._send(404, {"message": "Not Found"})
        # CI (`334-branch-ci-status`, `347-actions-job-logs`): every ref has the
        # same three check runs of one Actions workflow run; job 101 failed
        if path.startswith(f"{repo}/commits/") and path.endswith("/status"):
            return self._send(200, {"state": "pending", "total_count": 0, "statuses": []})
        if path.startswith(f"{repo}/commits/") and path.endswith("/check-runs"):
            return self._send(200, {"total_count": len(CHECK_RUNS), "check_runs": CHECK_RUNS})
        if path == f"{repo}/actions/runs":
            return self._send(200, {"total_count": 1, "workflow_runs": [WORKFLOW_RUN]})
        if path == f"{repo}/actions/runs/{WORKFLOW_RUN['id']}/jobs":
            return self._send(200, {"total_count": len(JOBS), "jobs": JOBS})
        if path.startswith(f"{repo}/actions/jobs/") and path.endswith("/logs"):
            job_id = int(path.rsplit("/", 2)[1])
            if job_id != 101:
                return self._send(404, {"message": "Not Found"})
            data = JOB_LOG.encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return None
        return self._send(404, {"message": f"stub: no GET {path}"})

    def do_POST(self):  # noqa: N802
        path = self.path.partition("?")[0]
        repo = f"/api/v3/repos/{OWNER}/{NAME}"
        body = self._body()
        self.server.posts.append((path, body))
        if path == f"{repo}/issues":
            number = 101 + len(self.server.created_issues)
            issue = _issue(number, body.get("title", ""), "open", "2024-05-01T10:00:00Z",
                           [l for l in body.get("labels", []) if any(x["name"] == l for x in LABELS)],
                           body.get("assignees", []), 0, body.get("body", ""))
            self.server.created_issues.insert(0, issue)
            return self._send(201, issue)
        if path == f"{repo}/releases/generate-notes":
            if os.environ.get("GITHUB_STUB_NO_NOTES") == "1":
                return self._send(404, {"message": "Not Found"})
            tag = body.get("tag_name", "")
            previous = body.get("previous_tag_name") or "v0.0.9"
            return self._send(200, {"name": f"{tag} generated", "body": (
                f"## What's Changed\n* Explain flags in the README by @octocat in #7\n* Add a logo by @octocat in #6\n\n"
                f"**Full Changelog**: http://127.0.0.1/{OWNER}/{NAME}/compare/{previous}...{tag}")})
        if path == f"{repo}/releases":
            rid = 9 + len(self.server.created_releases)
            release = _release(rid, body.get("tag_name", ""), body.get("name") or "", bool(body.get("draft")),
                               bool(body.get("prerelease")), "2024-05-01T10:00:00Z", assets=False)
            release["body"] = body.get("body", "")
            self.server.created_releases.insert(0, release)
            return self._send(201, release)
        if path == "/api/graphql":
            return self._send(200, {"data": {"createLinkedBranch": {"linkedBranch": {"id": "LB_stub"}}}})
        if path == "/api/v3/user/keys":
            if "write:public_key" not in self.server.scopes:
                return self._send(404, {"message": "Not Found"})
            return self._send(201, {"id": 1, "title": body.get("title", ""), "key": body.get("key", "")})
        return self._send(404, {"message": f"stub: no POST {path}"})


def _check_run(run_id: int, name: str, conclusion: str, started: str, completed: str) -> dict:
    return {"id": run_id, "name": name, "status": "completed", "conclusion": conclusion,
            "check_suite": {"id": 11}, "app": {"name": "GitHub Actions", "slug": "github-actions"},
            "started_at": started, "completed_at": completed,
            "html_url": f"http://127.0.0.1/{OWNER}/{NAME}/actions/runs/7/job/{run_id}", "pull_requests": []}


CHECK_RUNS = [
    _check_run(101, "test (macos-15)", "failure", "2024-05-01T10:00:00Z", "2024-05-01T10:05:10Z"),
    _check_run(102, "lint", "success", "2024-05-01T10:00:00Z", "2024-05-01T10:01:34Z"),
    _check_run(103, "build (ubuntu-latest)", "success", "2024-05-01T10:00:00Z", "2024-05-01T10:03:02Z"),
]

WORKFLOW_RUN = {"id": 7, "workflow_id": 3, "name": "CI", "created_at": "2024-05-01T09:59:50Z",
                "check_suite_id": 11, "event": "pull_request"}


def _step(number: int, name: str, conclusion: str, started: str, completed: str) -> dict:
    return {"name": name, "number": number, "status": "completed", "conclusion": conclusion,
            "started_at": started, "completed_at": completed}


JOBS = [
    {"id": 101, "name": "test (macos-15)", "status": "completed", "conclusion": "failure",
     "html_url": CHECK_RUNS[0]["html_url"], "steps": [
         _step(1, "Set up job", "success", "2024-05-01T10:00:00Z", "2024-05-01T10:00:02Z"),
         _step(2, "Checkout", "success", "2024-05-01T10:00:02Z", "2024-05-01T10:00:05Z"),
         _step(3, "cargo clippy", "success", "2024-05-01T10:00:05Z", "2024-05-01T10:01:39Z"),
         _step(4, "cargo test --workspace", "failure", "2024-05-01T10:01:39Z", "2024-05-01T10:05:10Z"),
         _step(5, "Post Checkout", "skipped", "2024-05-01T10:05:10Z", "2024-05-01T10:05:10Z")]},
    {"id": 102, "name": "lint", "status": "completed", "conclusion": "success",
     "html_url": CHECK_RUNS[1]["html_url"], "steps": [
         _step(1, "Set up job", "success", "2024-05-01T10:00:00Z", "2024-05-01T10:00:02Z"),
         _step(2, "cargo fmt --check", "success", "2024-05-01T10:00:02Z", "2024-05-01T10:01:34Z")]},
    {"id": 103, "name": "build (ubuntu-latest)", "status": "completed", "conclusion": "success",
     "html_url": CHECK_RUNS[2]["html_url"], "steps": [
         _step(1, "Set up job", "success", "2024-05-01T10:00:00Z", "2024-05-01T10:00:02Z"),
         _step(2, "cargo build", "success", "2024-05-01T10:00:02Z", "2024-05-01T10:03:02Z")]},
]

# the failed job's log, as `GET /actions/jobs/101/logs` serves it (timestamps,
# `##[group]` / `##[error]` commands and ANSI colours)
_LOG_LINES = [
    (0, "Current runner version: '2.317.0'"),
    (0, "##[group]Operating System"), (0, "macOS"), (0, "15.0"), (0, "##[endgroup]"),
    (1, "##[group]Run actions/checkout@v4"), (1, "with:"), (1, "  fetch-depth: 0"), (1, "##[endgroup]"),
    (2, "Syncing repository: octocat/parity-fixture"),
    (3, "##[group]Run cargo clippy --all-targets -- -D warnings"),
    (3, "\x1b[36;1mcargo clippy --all-targets -- -D warnings\x1b[0m"), (3, "shell: /bin/bash -e {0}"),
    (3, "##[endgroup]"),
    (10, "\x1b[1m\x1b[32m    Checking\x1b[0m corvene-core v0.1.0"),
    (97, "\x1b[1m\x1b[32m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 1m 34s"),
    (98, "##[group]Run cargo test --workspace"), (98, "\x1b[36;1mcargo test --workspace\x1b[0m"),
    (98, "shell: /bin/bash -e {0}"), (98, "##[endgroup]"),
    (140, "\x1b[1m\x1b[32m     Running\x1b[0m unittests src/lib.rs (target/debug/deps/corvene_core-8f2a1c)"),
    (141, "running 412 tests"),
    (300, "test job_log::tests::searches_case_insensitively ... \x1b[32mok\x1b[0m"),
    (301, "test remote::tests::fetch_skips_unchanged ... \x1b[31mFAILED\x1b[0m"),
    (302, "failures:"), (302, "---- remote::tests::fetch_skips_unchanged stdout ----"),
    (302, "thread 'remote::tests::fetch_skips_unchanged' panicked at crates/corvene-core/src/remote.rs:2710:9:"),
    (302, "assertion `left == right` failed"), (302, "  left: 2"), (302, " right: 1"),
    (302, "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"),
    (303, "test result: \x1b[31mFAILED\x1b[0m. 411 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 162.03s"),
    (309, "\x1b[1m\x1b[31merror\x1b[0m: test failed, to rerun pass `-p corvene-core --lib`"),
    (309, "##[error]Process completed with exit code 101."),
    (310, "Post job cleanup."), (310, "##[group]Run actions/checkout@v4"), (310, "##[endgroup]"),
    (310, "Cleaning up orphan processes"),
]
JOB_LOG = "".join(f"2024-05-01T10:{s // 60:02}:{s % 60:02}.0000000Z {text}\n" for s, text in _LOG_LINES)


class Stub(HTTPServer):
    def __init__(self):
        super().__init__(("127.0.0.1", 0), _Handler)
        self.created_issues: list[dict] = []
        self.created_releases: list[dict] = []
        self.posts: list[tuple[str, dict]] = []
        self.scopes: list[str] = os.environ.get(
            "GITHUB_STUB_SCOPES", "repo workflow read:user user:email").split()
        self.thread = threading.Thread(target=self.serve_forever, daemon=True)

    @property
    def port(self) -> int:
        return self.server_address[1]

    def hook_arg(self) -> str:
        """The `fake-github` control hook's argument."""
        return json.dumps({"port": self.port, "owner": OWNER, "name": NAME, "login": "octocat"})

    def start(self) -> "Stub":
        self.thread.start()
        return self

    def stop(self) -> None:
        self.shutdown()
        self.server_close()


def start() -> Stub:
    return Stub().start()


if __name__ == "__main__":
    stub = start()
    print(f"github stub on http://127.0.0.1:{stub.port}/api/v3")
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        stub.stop()
