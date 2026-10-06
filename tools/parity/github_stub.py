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

With a fixture repository (`start(repo)`, the `repo-pull-request` setup) the
stub also serves pull request #7, `feature/login` into `main`, whose head is
the branch's first commit (the second counts as not pushed yet), its check
runs, and the GraphQL operations of `corvene_github::review` by operation
name: the review threads (one open, one resolved range, one outdated, one
pending), the overview, and the mutations, which change the stub's threads
so a reload shows the reply, the resolved state, the new comment or the
submitted review (`348-pull-request-review`).
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

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


PR_NUMBER = 7
PR_NODE = "PR_stub7"
WEB = f"http://127.0.0.1/{OWNER}/{NAME}"


def _git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout.strip()


class PullRequestFixture:
    """Pull request #7 over the fixture repository, with its review threads
    and the mutations' effects."""

    def __init__(self, repo: Path):
        self.head = _git(repo, "rev-parse", "feature/login~1")
        self.tip = _git(repo, "rev-parse", "feature/login")
        self.base = _git(repo, "rev-parse", "origin/main")
        # the pull request's repository is the fixture's bare origin, as `git
        # remote -v` prints it, so the checked-out branch matches it
        self.clone_url = _git(repo, "remote", "get-url", "origin")
        self.comments = 0
        self.pending_review = "PRR_pending"
        self.submitted = []
        self.conversation = []
        self.threads = [
            self._thread("PRRT_1", "src/main.rs", 4, None, "RIGHT", False, False, [
                self._comment("octocat", "Should `--login` also accept a user name?", 40),
                self._comment("mona", "Not yet: the greeting only changes its words.", 38),
            ]),
            self._thread("PRRT_2", "src/main.rs", 9, 7, "RIGHT", True, False, [
                self._comment("hubot", "Collapse this into one `format!` with a conditional verb.", 50),
            ], resolved_by="mona"),
            self._thread("PRRT_3", "src/main.rs", None, None, "RIGHT", False, True, [
                self._comment("octocat", "Upper-casing the whole greeting also shouts the name.", 60),
            ], original_line=13),
            self._thread("PRRT_4", "README.md", 8, None, "RIGHT", False, False, [
                self._comment("octocat", "Say what a returning user is.", 1, pending=True),
            ]),
        ]

    def _comment(self, login: str, body: str, hours_ago: int, pending: bool = False) -> dict:
        self.comments += 1
        hour = 23 - min(hours_ago, 23)
        return {
            "id": f"PRRC_{self.comments}", "databaseId": 500 + self.comments, "body": body,
            "createdAt": f"2024-03-{1 + hours_ago // 24:02d}T{hour:02d}:00:00Z",
            "updatedAt": f"2024-03-{1 + hours_ago // 24:02d}T{hour:02d}:00:00Z",
            "url": f"{WEB}/pull/{PR_NUMBER}#discussion_r{500 + self.comments}",
            "state": "PENDING" if pending else "SUBMITTED", "outdated": False,
            "viewerCanUpdate": login == "octocat", "viewerCanDelete": login == "octocat",
            "viewerDidAuthor": login == "octocat", "diffHunk": "@@ -1,6 +1,8 @@\n fn main() {",
            "author": {"login": login, "avatarUrl": None}, "originalCommit": {"oid": self.head},
            "commit": {"oid": self.head}, "replyTo": None,
            "pullRequestReview": {"id": self.pending_review if pending else "PRR_1",
                                  "state": "PENDING" if pending else "COMMENTED"},
        }

    def _thread(self, tid: str, path: str, line, start_line, side: str, resolved: bool, outdated: bool,
                comments: list, resolved_by: str | None = None, original_line: int | None = None) -> dict:
        return {
            "id": tid, "path": path, "line": line, "startLine": start_line,
            "originalLine": original_line if original_line is not None else line,
            "originalStartLine": start_line, "diffSide": side, "startDiffSide": side if start_line else None,
            "isResolved": resolved, "isOutdated": outdated,
            "resolvedBy": {"login": resolved_by} if resolved_by else None,
            "viewerCanResolve": True, "viewerCanUnresolve": True, "viewerCanReply": True,
            "comments": {"nodes": comments},
        }

    def pending_count(self) -> int:
        return sum(1 for t in self.threads for c in t["comments"]["nodes"] if c["state"] == "PENDING")

    def rest(self) -> dict:
        repo = {"name": NAME, "owner": _user(OWNER), "html_url": WEB, "clone_url": self.clone_url,
                "default_branch": "main", "private": False, "fork": False, "parent": None,
                "node_id": "R_stub", "permissions": {"admin": True, "push": True, "pull": True}}
        return {
            "number": PR_NUMBER, "title": "Add a --login flag", "state": "open", "draft": False,
            "created_at": "2024-02-28T10:00:00Z", "updated_at": "2024-03-03T10:00:00Z",
            "user": _user("mona"), "body": "Greets returning users with `--login`.\n\nCloses #41.",
            "head": {"ref": "feature/login", "sha": self.head, "repo": repo},
            "base": {"ref": "main", "sha": self.base, "repo": repo},
            "assignees": [], "requested_reviewers": [_user("hubot")],
            "html_url": f"{WEB}/pull/{PR_NUMBER}",
        }

    def overview(self) -> dict:
        pending = [{"id": self.pending_review, "body": "", "author": {"login": "octocat"},
                    "comments": {"totalCount": self.pending_count()}}] if self.pending_count() else []
        reviews = [{"id": "PRR_1", "state": "CHANGES_REQUESTED", "body": "A couple of things before this lands.",
                    "submittedAt": "2024-03-01T12:00:00Z", "url": f"{WEB}/pull/{PR_NUMBER}#pullrequestreview-1",
                    "author": {"login": "hubot", "avatarUrl": None}, "comments": {"totalCount": 1}}]
        reviews += self.submitted
        timeline = [
            {"__typename": "PullRequestCommit", "commit": {"oid": self.head, "messageHeadline": "Add a --login flag",
             "committedDate": "2024-02-28T09:00:00Z", "author": {"name": "Mona", "user": {"login": "mona"}}}},
            {"__typename": "ReviewRequestedEvent", "createdAt": "2024-02-28T10:05:00Z", "actor": {"login": "mona"},
             "requestedReviewer": {"__typename": "User", "login": "hubot"}},
            {"__typename": "IssueComment", "id": "IC_1", "body": "CI is green on this one.",
             "createdAt": "2024-02-29T08:00:00Z", "url": f"{WEB}/pull/{PR_NUMBER}#issuecomment-1",
             "author": {"login": "octocat", "avatarUrl": None}},
            {"__typename": "LabeledEvent", "createdAt": "2024-02-29T09:00:00Z", "actor": {"login": "mona"},
             "label": {"name": "enhancement", "color": "a2eeef"}},
            dict(reviews[0], __typename="PullRequestReview"),
        ] + self.conversation + [dict(r, __typename="PullRequestReview") for r in self.submitted]
        return {
            "id": PR_NODE, "number": PR_NUMBER, "title": "Add a --login flag",
            "body": "Greets returning users with `--login`.\n\n- [x] flag\n- [ ] docs\n\nCloses #41.",
            "state": "OPEN", "isDraft": False, "createdAt": "2024-02-28T10:00:00Z", "mergedAt": None,
            "closedAt": None, "url": f"{WEB}/pull/{PR_NUMBER}", "author": {"login": "mona", "avatarUrl": None},
            "baseRefName": "main", "headRefName": "feature/login", "baseRefOid": self.base,
            "headRefOid": self.head, "isCrossRepository": False, "headRepository": {"nameWithOwner": f"{OWNER}/{NAME}"},
            "mergeable": "MERGEABLE", "reviewDecision": "CHANGES_REQUESTED", "additions": 14, "deletions": 3,
            "changedFiles": 2, "viewerCanUpdate": True,
            "commits": {"totalCount": 1, "nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS"}}}]},
            "labels": {"nodes": [{"name": "enhancement", "color": "a2eeef"}]},
            "milestone": {"title": "v0.2"}, "assignees": {"nodes": [{"login": "mona"}]},
            "reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "User", "login": "hubot", "avatarUrl": None}}]},
            "latestOpinionatedReviews": {"nodes": reviews},
            "pendingReviews": {"nodes": pending},
            "timelineItems": {"totalCount": len(timeline), "nodes": timeline},
        }

    def find_thread(self, tid: str) -> dict | None:
        return next((t for t in self.threads if t["id"] == tid), None)

    def graphql(self, query: str, variables: dict) -> dict:
        """The answer's `data` for one of `corvene_github::review`'s operations."""
        m = re.match(r"\s*(query|mutation)\s+(\w+)", query)
        op = m.group(2) if m else ""
        v = variables or {}
        if op == "ReviewThreads":
            return {"repository": {"pullRequest": {"id": PR_NODE, "reviewThreads": {
                "pageInfo": {"hasNextPage": False, "endCursor": None}, "nodes": self.threads}}}}
        if op == "PullRequestOverview":
            return {"viewer": {"login": "octocat"}, "repository": {"pullRequest": self.overview()}}
        if op == "AddThreadReply":
            thread = self.find_thread(v.get("thread", ""))
            if thread is None:
                return {"addPullRequestReviewThreadReply": None}
            comment = self._comment("octocat", v.get("body", ""), 0, pending=bool(v.get("review")))
            comment["replyTo"] = {"id": thread["comments"]["nodes"][0]["id"]}
            thread["comments"]["nodes"].append(comment)
            return {"addPullRequestReviewThreadReply": {"comment": {"id": comment["id"]}}}
        if op in ("ResolveThread", "UnresolveThread"):
            thread = self.find_thread(v.get("thread", ""))
            if thread is None:
                return {"resolveReviewThread": None, "unresolveReviewThread": None}
            thread["isResolved"] = op == "ResolveThread"
            thread["resolvedBy"] = {"login": "octocat"} if thread["isResolved"] else None
            key = "resolveReviewThread" if op == "ResolveThread" else "unresolveReviewThread"
            return {key: {"thread": {"id": thread["id"], "isResolved": thread["isResolved"]}}}
        if op == "StartReview":
            return {"addPullRequestReview": {"pullRequestReview": {"id": self.pending_review, "state": "PENDING"}}}
        if op in ("AddReviewThread", "AddSingleComment"):
            inp = v.get("input") if op == "AddReviewThread" else (v.get("threads") or [{}])[0]
            pending = op == "AddReviewThread"
            tid = f"PRRT_{len(self.threads) + 1}"
            thread = self._thread(tid, inp.get("path", ""), inp.get("line"), inp.get("startLine"),
                                  inp.get("side", "RIGHT"), False, False,
                                  [self._comment("octocat", inp.get("body", ""), 0, pending=pending)])
            self.threads.append(thread)
            if pending:
                return {"addPullRequestReviewThread": {"thread": {"id": tid}}}
            return {"addPullRequestReview": {"pullRequestReview": {"id": f"PRR_{tid}", "state": "COMMENTED"}}}
        if op in ("SubmitReview", "AddReview"):
            event = v.get("event", "COMMENT")
            state = {"APPROVE": "APPROVED", "REQUEST_CHANGES": "CHANGES_REQUESTED"}.get(event, "COMMENTED")
            count = 0
            for t in self.threads:
                for c in t["comments"]["nodes"]:
                    if c["state"] == "PENDING":
                        c["state"] = "SUBMITTED"
                        c["pullRequestReview"] = {"id": "PRR_submitted", "state": state}
                        count += 1
            self.submitted.append({"id": "PRR_submitted", "state": state, "body": v.get("body", ""),
                                   "submittedAt": "2024-03-04T10:00:00Z",
                                   "url": f"{WEB}/pull/{PR_NUMBER}#pullrequestreview-9",
                                   "author": {"login": "octocat", "avatarUrl": None},
                                   "comments": {"totalCount": count}})
            key = "submitPullRequestReview" if op == "SubmitReview" else "addPullRequestReview"
            return {key: {"pullRequestReview": {"id": "PRR_submitted", "state": state}}}
        if op == "DeleteReview":
            for t in self.threads:
                t["comments"]["nodes"] = [c for c in t["comments"]["nodes"] if c["state"] != "PENDING"]
            self.threads = [t for t in self.threads if t["comments"]["nodes"]]
            return {"deletePullRequestReview": {"pullRequestReview": {"id": self.pending_review, "state": "DISMISSED"}}}
        if op == "UpdateReviewComment":
            for t in self.threads:
                for c in t["comments"]["nodes"]:
                    if c["id"] == v.get("comment"):
                        c["body"] = v.get("body", "")
            return {"updatePullRequestReviewComment": {"pullRequestReviewComment": {"id": v.get("comment")}}}
        if op == "DeleteReviewComment":
            for t in self.threads:
                t["comments"]["nodes"] = [c for c in t["comments"]["nodes"] if c["id"] != v.get("comment")]
            self.threads = [t for t in self.threads if t["comments"]["nodes"]]
            return {"deletePullRequestReviewComment": {"pullRequestReview": {"id": self.pending_review}}}
        if op == "AddConversationComment":
            cid = f"IC_{len(self.conversation) + 2}"
            self.conversation.append({"__typename": "IssueComment", "id": cid, "body": v.get("body", ""),
                                      "createdAt": "2024-03-04T09:00:00Z",
                                      "url": f"{WEB}/pull/{PR_NUMBER}#issuecomment-{cid}",
                                      "author": {"login": "octocat", "avatarUrl": None}})
            return {"addComment": {"commentEdge": {"node": {"id": cid}}}}
        return {"createLinkedBranch": {"linkedBranch": {"id": "LB_stub"}}}


class _Handler(BaseHTTPRequestHandler):
    server_version = "github-stub/1"

    def log_message(self, *_args):
        pass

    def _send(self, status: int, body) -> None:
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("X-GitHub-Request-Id", "STUB:1")
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
            return self._send(200, {**_user("octocat"), "name": "Mona Lisa Octocat", "plan": {"name": "free"}})
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
        pr = self.server.pull_request
        if pr is not None:
            if path == f"{repo}/pulls":
                return self._send(200, [pr.rest()] if params.get("state", "open") != "closed" else [])
            if path == f"{repo}/pulls/{PR_NUMBER}":
                return self._send(200, pr.rest())
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
            pr = self.server.pull_request
            if pr is None:
                return self._send(200, {"data": {"createLinkedBranch": {"linkedBranch": {"id": "LB_stub"}}}})
            return self._send(200, {"data": pr.graphql(body.get("query", ""), body.get("variables") or {})})
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
    def __init__(self, repo: Path | None = None):
        super().__init__(("127.0.0.1", 0), _Handler)
        self.created_issues: list[dict] = []
        self.created_releases: list[dict] = []
        self.posts: list[tuple[str, dict]] = []
        # `348-pull-request-review`: pull request #7 over the fixture
        self.pull_request = PullRequestFixture(repo) if repo else None
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


def start(repo: Path | None = None) -> Stub:
    return Stub(repo).start()


if __name__ == "__main__":
    stub = start()
    print(f"github stub on http://127.0.0.1:{stub.port}/api/v3")
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        stub.stop()
