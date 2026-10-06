"""A stub Git LFS locking API for the parity harness (`lfs_stub: true`).

Corvene's LFS locks (flag `1113-lfs-locks`) ask `git lfs locks`, `lock` and
`unlock`, which talk to the repository's LFS server. `start()` serves the
locking API (https://github.com/git-lfs/git-lfs/blob/main/docs/api/locking.md)
on a free port; the harness points the fixture's `lfs.url` at `url()`.
Nothing asks for credentials: every request is the user `ME`.

Two locks exist from the start: `art/hero.psd` held by Mona Lisa (someone
else's) and `art/logo.psd` held by `ME`. A new lock gets the next id. With
`unsupported=True` (or `LFS_STUB_NO_LOCKS=1`) every request is a 404, as on
a server without locking.

Run it on its own with `python3 lfs_stub.py [port]`.
"""

from __future__ import annotations

import json
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

ME = "Parity Tester"
LOCKED_AT = "2024-03-04T10:00:00Z"
CONTENT_TYPE = "application/vnd.git-lfs+json"


def _initial_locks() -> list[dict]:
    return [
        {"id": "1", "path": "art/hero.psd", "locked_at": LOCKED_AT, "owner": {"name": "Mona Lisa"}},
        {"id": "2", "path": "art/logo.psd", "locked_at": LOCKED_AT, "owner": {"name": ME}},
    ]


class _Handler(BaseHTTPRequestHandler):
    server: "_Server"

    def log_message(self, *_args):  # quiet
        pass

    def _send(self, code: int, body: dict | None = None):
        data = json.dumps(body if body is not None else {}).encode()
        self.send_response(code)
        self.send_header("Content-Type", CONTENT_TYPE)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _body(self) -> dict:
        length = int(self.headers.get("Content-Length") or 0)
        if not length:
            return {}
        try:
            return json.loads(self.rfile.read(length))
        except ValueError:
            return {}

    def _route(self) -> str:
        path = urlparse(self.path).path
        i = path.find("/locks")
        return path[i:] if i >= 0 else path

    def do_GET(self):  # noqa: N802
        if self.server.unsupported:
            return self._send(404, {"message": "Not Found"})
        if self._route() != "/locks":
            return self._send(404, {"message": "Not Found"})
        query = parse_qs(urlparse(self.path).query)
        locks = self.server.locks
        if "path" in query:
            locks = [l for l in locks if l["path"] == query["path"][0]]
        if "id" in query:
            locks = [l for l in locks if l["id"] == query["id"][0]]
        self._send(200, {"locks": locks})

    def do_POST(self):  # noqa: N802
        if self.server.unsupported:
            return self._send(404, {"message": "Not Found"})
        route = self._route()
        body = self._body()
        with self.server.lock:
            locks = self.server.locks
            if route == "/locks/verify":
                ours = [l for l in locks if l["owner"]["name"] == ME]
                theirs = [l for l in locks if l["owner"]["name"] != ME]
                return self._send(200, {"ours": ours, "theirs": theirs})
            if route == "/locks":
                path = body.get("path", "")
                held = next((l for l in locks if l["path"] == path), None)
                if held:
                    return self._send(409, {"lock": held, "message": "already created lock"})
                self.server.next_id += 1
                lock = {"id": str(self.server.next_id), "path": path, "locked_at": LOCKED_AT,
                        "owner": {"name": ME}}
                locks.append(lock)
                return self._send(201, {"lock": lock})
            if route.startswith("/locks/") and route.endswith("/unlock"):
                lock_id = route[len("/locks/"):-len("/unlock")]
                held = next((l for l in locks if l["id"] == lock_id), None)
                if not held:
                    return self._send(404, {"message": "unable to find lock"})
                if held["owner"]["name"] != ME and not body.get("force"):
                    return self._send(403, {"message": f"lock {lock_id} is owned by {held['owner']['name']}"})
                locks.remove(held)
                return self._send(200, {"lock": held})
        self._send(404, {"message": "Not Found"})


class _Server(HTTPServer):
    def __init__(self, unsupported: bool, port: int = 0):
        super().__init__(("127.0.0.1", port), _Handler)
        self.unsupported = unsupported
        self.locks = _initial_locks()
        self.next_id = 2
        self.lock = threading.Lock()


class Stub:
    def __init__(self, unsupported: bool = False, port: int = 0):
        self.server = _Server(unsupported or os.environ.get("LFS_STUB_NO_LOCKS") == "1", port)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    @property
    def port(self) -> int:
        return self.server.server_address[1]

    def url(self) -> str:
        """The `lfs.url` for a fixture repository."""
        return f"http://127.0.0.1:{self.port}/parity-fixture.git/info/lfs"

    def stop(self):
        self.server.shutdown()
        self.server.server_close()


def start(unsupported: bool = False) -> Stub:
    return Stub(unsupported)


if __name__ == "__main__":
    stub = Stub(port=int(sys.argv[1]) if len(sys.argv) > 1 else 0)
    print(stub.url(), flush=True)
    stub.thread.join()
