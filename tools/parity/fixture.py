"""Deterministic fixture repository shown by both apps.

Fixed author, committer and dates make every copy byte-identical (same SHAs),
so GHD and Corvene each get their own copy (no index.lock races, destructive
scenarios stay possible) that renders the same text.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

NAME = "parity-fixture"
AUTHOR = ("Parity Bot", "parity@example.com")

_COMMITS = [
    ("2026-08-03T09:12:00+00:00", "Initial commit", "", {
        "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n",
        ".gitignore": "target/\n*.log\n",
    }),
    ("2026-08-05T14:40:00+00:00", "Add the command line entry point", "Parses arguments and prints a greeting.", {
        "src/main.rs": 'fn main() {\n    let name = std::env::args().nth(1).unwrap_or_else(|| "world".into());\n    println!("Hello, {name}!");\n}\n',
        "Cargo.toml": '[package]\nname = "parity-fixture"\nversion = "0.1.0"\nedition = "2024"\n',
    }),
    ("2026-08-11T10:05:00+00:00", "Write the user guide", "", {
        "docs/guide.md": "# Guide\n\n1. Build with `cargo build`.\n2. Run `parity-fixture <name>`.\n\nThat is all.\n",
        "docs/old.md": "Outdated notes that will be deleted.\n",
    }),
    ("2026-08-19T16:30:00+00:00", "Support a --shout flag", "Upper-cases the greeting.\n\nCloses #4.", {
        "src/main.rs": 'fn main() {\n    let args: Vec<String> = std::env::args().skip(1).collect();\n    let shout = args.iter().any(|a| a == "--shout");\n    let name = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "world".into());\n    let line = format!("Hello, {name}!");\n    println!("{}", if shout { line.to_uppercase() } else { line });\n}\n',
    }),
    ("2026-09-02T08:00:00+00:00", "Add a logo", "", {
        "assets/logo.svg": '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="7" fill="#2da44e"/></svg>\n',
    }),
    ("2026-09-14T11:45:00+00:00", "Explain flags in the README", "", {
        "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n\n## Flags\n\n- `--shout`: upper-case the greeting\n",
    }),
]

_WORKING_CHANGES = {
    "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n\n## Flags\n\n- `--shout`: upper-case the greeting\n- `--quiet`: print nothing\n\n## License\n\nMIT\n",
    "notes.txt": "Remember to update the changelog.\n",
    "src/lib.rs": "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {name}!\")\n}\n",
    # one changed word (intra-line highlight) next to a line long enough to wrap
    "src/main.rs": 'fn main() {\n    let args: Vec<String> = std::env::args().skip(1).collect();\n    let shout = args.iter().any(|a| a == "--loud");\n    let name = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "world".into())).trim().to_string(); // falls back to $USER, then to "world"\n    let line = format!("Hello, {name}!");\n    println!("{}", if shout { line.to_uppercase() } else { line });\n}\n',
}
_DELETED = ["docs/old.md"]

# `repo-coauthors`: commits by several people (`AvatarStack`), the newest
# first in History: six people (the stack's "more" sliver), four, three
# (a different committer), two
_PEOPLE = [
    ("Mona Lisa", "mona@example.com"),
    ("Hubot", "hubot@example.com"),
    ("Octo Cat", "octocat@example.com"),
    ("Jane Doe", "jane@example.com"),
    ("John Roe", "john@example.com"),
]
_CO_AUTHORED = [
    ("2026-09-20T09:00:00+00:00", "Pair on the greeting", 1, None),
    ("2026-09-21T09:00:00+00:00", "Apply the review", 1, _PEOPLE[2]),
    ("2026-09-22T09:00:00+00:00", "Mob on the parser", 3, None),
    ("2026-09-23T09:00:00+00:00", "Team cleanup", 5, None),
]


def _git(repo: Path, *args: str, date: str | None = None, committer: tuple[str, str] = AUTHOR):
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(
        GIT_AUTHOR_NAME=AUTHOR[0], GIT_AUTHOR_EMAIL=AUTHOR[1],
        GIT_COMMITTER_NAME=committer[0], GIT_COMMITTER_EMAIL=committer[1],
        GIT_CONFIG_NOSYSTEM="1",
    )
    if date:
        env.update(GIT_AUTHOR_DATE=date, GIT_COMMITTER_DATE=date)
    subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True)


def remove_tree(path: Path) -> None:
    """`shutil.rmtree` that also removes read-only files: git's objects are,
    and Windows refuses to delete those."""
    import os
    import stat
    import sys

    def writable(function, target, _):
        os.chmod(target, stat.S_IWRITE)
        function(target)

    if sys.version_info >= (3, 12):
        shutil.rmtree(path, onexc=writable)
    else:
        shutil.rmtree(path, onerror=writable)


def _signed(parent: Path, repo: Path) -> None:
    """`1214-commit-signatures`: signed commits on top, newest first a
    tampered (bad) SSH signature, an SSH key the allowed signers file does
    not list, a GPG key no keyring here has (when gpg can make one) and a
    good SSH signature. The keys live in `<parent>/parity-signing`; the
    repository's own `gpg.ssh.allowedSignersFile` lists the good one, so
    no environment is needed to verify."""
    keys = parent / "parity-signing"
    if keys.exists():
        remove_tree(keys)
    keys.mkdir(parents=True)
    for name in ("good", "unknown"):
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", AUTHOR[1], "-f", str(keys / name)],
                       check=True, capture_output=True)
    allowed = keys / "allowed_signers"
    allowed.write_text(f"{AUTHOR[1]} {(keys / 'good.pub').read_text()}")
    _git(repo, "config", "gpg.ssh.allowedSignersFile", str(allowed))

    def commit(summary: str, date: str, *sign: str):
        (repo / "SIGNED.md").write_text(f"{summary}\n")
        _git(repo, "add", "-A")
        _git(repo, *sign, "commit", "-q", "-S", "-m", summary, date=date)

    def ssh(key: str) -> tuple[str, ...]:
        return ("-c", "gpg.format=ssh", "-c", f"user.signingkey={keys / key}.pub")

    commit("Sign with a known SSH key", "2026-09-25T09:00:00+00:00", *ssh("good"))
    # a short GNUPGHOME: gpg-agent's socket path has a length limit
    import tempfile
    home = Path(tempfile.mkdtemp(prefix="pg"))
    home.chmod(0o700)
    env = {**os.environ, "GNUPGHOME": str(home)}
    made = shutil.which("gpg") and subprocess.run(
        ["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "", "--quick-gen-key",
         f"{AUTHOR[0]} <{AUTHOR[1]}>", "ed25519", "sign", "never"], env=env, capture_output=True).returncode == 0
    if made:
        os.environ["GNUPGHOME"], saved = str(home), os.environ.get("GNUPGHOME")
        try:
            commit("Sign with a GPG key nobody has", "2026-09-25T10:00:00+00:00",
                   "-c", "gpg.format=openpgp", "-c", f"user.signingkey={AUTHOR[1]}")
        finally:
            if saved is None:
                del os.environ["GNUPGHOME"]
            else:
                os.environ["GNUPGHOME"] = saved
        subprocess.run(["gpgconf", "--kill", "gpg-agent"], env=env, capture_output=True)
    remove_tree(home)
    commit("Sign with an SSH key nobody listed", "2026-09-25T11:00:00+00:00", *ssh("unknown"))
    commit("Sign and then change the message", "2026-09-25T12:00:00+00:00", *ssh("good"))
    raw = subprocess.run(["git", "cat-file", "commit", "HEAD"], cwd=repo, check=True, capture_output=True).stdout
    tampered = raw.replace(b"\n\nSign and then change", b"\n\nSigned and then changed", 1)
    oid = subprocess.run(["git", "hash-object", "-t", "commit", "-w", "--stdin"], cwd=repo, input=tampered,
                         check=True, capture_output=True).stdout.decode().strip()
    _git(repo, "update-ref", "HEAD", oid)


_PATCH = """diff --git a/docs/guide.md b/docs/guide.md
--- a/docs/guide.md
+++ b/docs/guide.md
@@ -3,4 +3,5 @@
 1. Build with `cargo build`.
 2. Run `parity-fixture <name>`.
+3. Pass `--shout` to shout.
 
 That is all.
diff --git a/CONTRIBUTING.md b/CONTRIBUTING.md
new file mode 100644
--- /dev/null
+++ b/CONTRIBUTING.md
@@ -0,0 +1,3 @@
+# Contributing
+
+Open a pull request.
"""


def _tools(parent: Path, repo: Path) -> None:
    """Ignored files in `repo`, and beside it `parity-fixture.patch` (a plain
    diff) and `parity-fixture.mbox` (two `git format-patch` commits made on a
    branch that is deleted again)."""
    (repo / "build.log").write_text("build output\n")
    (repo / "target" / "debug").mkdir(parents=True, exist_ok=True)
    (repo / "target" / "debug" / "parity-fixture").write_text("binary\n")
    (parent / f"{NAME}.patch").write_text(_PATCH)
    date = "2026-09-30T10:00:00+00:00"
    _git(repo, "checkout", "-q", "-b", "changelog", date=date)
    (repo / "CHANGELOG.md").write_text("# Changelog\n")
    _git(repo, "add", "CHANGELOG.md", date=date)
    _git(repo, "commit", "-q", "-m", "Add a changelog", date=date)
    (repo / "CHANGELOG.md").write_text("# Changelog\n\n- 0.1.0\n")
    _git(repo, "commit", "-q", "-am", "Note 0.1.0 in the changelog", date=date)
    mbox = subprocess.run(["git", "format-patch", "-2", "--stdout"], cwd=repo, check=True,
                          capture_output=True, text=True).stdout
    (parent / f"{NAME}.mbox").write_text(mbox)
    _git(repo, "checkout", "-q", "main", date=date)
    _git(repo, "branch", "-q", "-D", "changelog", date=date)


# `repo-pull-request`: the stub GitHub API's pull request #7, `feature/login`
# into `main`, two commits (the second one "not pushed yet" as far as the stub
# is concerned: its pull request head is the first); the review threads in
# `github_stub.py` name lines of these files. The stub names the bare
# `origin` as the pull request's repository, so the branch matches it.
_PULL_REQUEST_COMMITS = [
    ("2026-09-16T09:00:00+00:00", "Add a --login flag", {
        "src/main.rs": 'fn main() {\n'
                       '    let args: Vec<String> = std::env::args().skip(1).collect();\n'
                       '    let shout = args.iter().any(|a| a == "--shout");\n'
                       '    let login = args.iter().any(|a| a == "--login");\n'
                       '    let name = args.iter().find(|a| !a.starts_with("--")).cloned();\n'
                       '    let name = name.unwrap_or_else(|| "world".into());\n'
                       '    let greeting = if login {\n'
                       '        format!("Welcome back, {name}!")\n'
                       '    } else {\n'
                       '        format!("Hello, {name}!")\n'
                       '    };\n'
                       '    if shout {\n'
                       '        println!("{}", greeting.to_uppercase());\n'
                       '    } else {\n'
                       '        println!("{greeting}");\n'
                       '    }\n'
                       '}\n',
        "README.md": "# Parity fixture\n\nA small repository for GitHub Desktop parity runs.\n\n## Flags\n\n"
                     "- `--shout`: upper-case the greeting\n- `--login`: greet a returning user\n",
    }),
    ("2026-09-17T10:00:00+00:00", "Document the greeter", {
        "src/main.rs": '//! The parity fixture\'s greeter.\n'
                       'fn main() {\n'
                       '    let args: Vec<String> = std::env::args().skip(1).collect();\n'
                       '    let shout = args.iter().any(|a| a == "--shout");\n'
                       '    let login = args.iter().any(|a| a == "--login");\n'
                       '    let name = args.iter().find(|a| !a.starts_with("--")).cloned();\n'
                       '    let name = name.unwrap_or_else(|| "world".into());\n'
                       '    let greeting = if login {\n'
                       '        format!("Welcome back, {name}!")\n'
                       '    } else {\n'
                       '        format!("Hello, {name}!")\n'
                       '    };\n'
                       '    if shout {\n'
                       '        println!("{}", greeting.to_uppercase());\n'
                       '    } else {\n'
                       '        println!("{greeting}");\n'
                       '    }\n'
                       '}\n',
        "docs/guide.md": "# Guide\n\n1. Build with `cargo build`.\n2. Run `parity-fixture <name>`.\n"
                         "3. Add `--login` to greet a returning user.\n\nThat is all.\n",
    }),
]
def _structure(parent: Path, repo: Path, lfs_url: str | None) -> None:
    """Submodules and Git LFS files in `repo` (flags 1111-1113).

    `vendor/lib` is checked out one commit past the one recorded,
    `vendor/theme` was deinitialized (its `.git/modules` stays, so Update
    needs no clone from a local path, which git refuses in submodules) and
    `tools/scripts` is up to date. `art/*.psd` are LFS files; `lfs_url` (the
    `lfs_stub.py` server) is their LFS server, where `art/hero.psd` is
    locked by Mona Lisa and `art/logo.psd` by the user. Both are changed in
    the working tree."""
    date = "2026-09-26T10:00:00+00:00"
    sources = {"vendor/lib": "parity-lib", "vendor/theme": "parity-theme", "tools/scripts": "parity-scripts"}
    for name in sources.values():
        source = parent / name
        if source.exists():
            remove_tree(source)
        source.mkdir(parents=True)
        _git(source, "init", "-q", "-b", "main")
        _git(source, "config", "commit.gpgsign", "false")
        (source / "README.md").write_text(f"# {name}\n")
        _git(source, "add", "-A", date=date)
        _git(source, "commit", "-q", "-m", f"Start {name}", date=date)
    for path, name in sources.items():
        _git(repo, "-c", "protocol.file.allow=always", "submodule", "add", "-q", str((parent / name).resolve()), path, date=date)
    _git(repo, "commit", "-q", "-m", "Add submodules", date=date)
    # vendor/lib: a newer commit checked out
    lib = parent / sources["vendor/lib"]
    (lib / "lib.txt").write_text("more\n")
    _git(lib, "add", "-A", date=date)
    _git(lib, "commit", "-q", "-m", "Grow the library", date=date)
    _git(repo / "vendor" / "lib", "pull", "-q", "origin", "main", date=date)
    # vendor/theme: deinitialized
    _git(repo, "submodule", "deinit", "-q", "-f", "vendor/theme", date=date)
    # Git LFS
    _git(repo, "lfs", "install", "--local", date=date)
    (repo / ".gitattributes").write_text("*.psd filter=lfs diff=lfs merge=lfs -text\n")
    (repo / "art").mkdir(exist_ok=True)
    for name in ("hero", "logo", "banner"):
        (repo / "art" / f"{name}.psd").write_text(f"{name} artwork v1\n")
    _git(repo, "add", ".gitattributes", "art", date=date)
    _git(repo, "commit", "-q", "-m", "Add artwork", date=date)
    if lfs_url:
        _git(repo, "config", "lfs.url", lfs_url)
    for name in ("hero", "logo"):
        (repo / "art" / f"{name}.psd").write_text(f"{name} artwork v2\n")


def _remotes(parent: Path, repo: Path) -> None:
    """A second bare remote, `fork`, with all of `main`, and more tags: an
    annotated `v0.2.0` with a message and a lightweight `nightly`."""
    fork = parent / f"{NAME}-fork.git"
    if fork.exists():
        remove_tree(fork)
    _git(repo, "init", "-q", "--bare", str(fork))
    _git(repo, "remote", "add", "fork", str(fork))
    _git(repo, "push", "-q", "fork", "main:refs/heads/main")
    _git(repo, "fetch", "-q", "fork")
    _git(repo, "tag", "-a", "-m", "Second release", "v0.2.0", "HEAD~2",
         date="2026-09-26T12:00:00+00:00")
    _git(repo, "tag", "nightly", "HEAD")


def build(parent: Path, remote: bool = False, coauthors: bool = False, graph: bool = False,
          signed: bool = False, reflog: bool = False, tools: bool = False,
          structure: bool = False, lfs_url: str | None = None, remotes: bool = False,
          pull_request: bool = False) -> Path:
    """(Re)create `<parent>/parity-fixture` and return its path.

    With `remote`, a bare `<parent>/parity-fixture.git` is added as `origin`
    and `main` is pushed up to its third commit, so the branch is two ahead
    with the `v0.1.0` tag still to push (History's unpushed indicators, the
    toolbar's Push origin). With `coauthors`, `_CO_AUTHORED` commits go on
    top. With `graph`, a merged branch and an octopus merge of two more go on
    top (History's commit graph). With `signed`, see `_signed`. With
    `reflog`, HEAD's reflog gets a rebase, a branch deleted after use and
    three commits a hard reset left behind (Recent Activity,
    `1216-recent-activity`). With `tools`, see `_tools` (Clean Untracked
    Files and Apply Patch, flags 1105 and 1106). With `structure`, see
    `_structure` (submodules, sparse checkout and LFS locks, flags
    1111-1113). With `remotes` (and `remote`), see `_remotes` (Repository
    Settings' remotes and Branch › Tags…, flags 1109 and 1219). With
    `pull_request` (and `remote`), `main` is pushed whole and `feature/login`
    is checked out with `_PULL_REQUEST_COMMITS` pushed
    (`348-pull-request-review`)."""
    repo = parent / NAME
    if repo.exists():
        remove_tree(repo)
    repo.mkdir(parents=True)
    _git(repo, "init", "-q", "-b", "main")
    for key, value in (("user.name", AUTHOR[0]), ("user.email", AUTHOR[1]), ("commit.gpgsign", "false"), ("tag.gpgsign", "false")):
        _git(repo, "config", key, value)
    for i, (date, summary, body, files) in enumerate(_COMMITS):
        for rel, text in files.items():
            p = repo / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        _git(repo, "add", "-A")
        msg = summary + (f"\n\n{body}" if body else "")
        _git(repo, "commit", "-q", "-m", msg, date=date)
        if i == 3:
            _git(repo, "branch", "feature/login")
            _git(repo, "branch", "bugfix/typo-in-guide")
    _git(repo, "tag", "v0.1.0", "HEAD~1")
    if coauthors:
        for i, (date, summary, count, committer) in enumerate(_CO_AUTHORED):
            (repo / "TEAM.md").write_text("".join(f"- {n}\n" for n, _ in _PEOPLE[: i + 1]))
            _git(repo, "add", "-A")
            trailers = "".join(f"\nCo-authored-by: {n} <{e}>" for n, e in _PEOPLE[:count])
            _git(repo, "commit", "-q", "-m", f"{summary}\n{trailers}", date=date, committer=committer or AUTHOR)
    if graph:
        def commit(rel: str, summary: str, date: str):
            p = repo / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(f"{summary}\n")
            _git(repo, "add", "-A")
            _git(repo, "commit", "-q", "-m", summary, date=date)

        _git(repo, "checkout", "-q", "-b", "topic/lanes", "HEAD~1")
        commit("graph/lanes.md", "Draw the lanes", "2026-09-24T09:00:00+00:00")
        _git(repo, "checkout", "-q", "main")
        commit("graph/plan.md", "Plan the graph", "2026-09-24T10:00:00+00:00")
        _git(repo, "merge", "-q", "--no-ff", "-m", "Merge branch 'topic/lanes'", "topic/lanes",
             date="2026-09-24T11:00:00+00:00")
        for name, hour in (("one", 12), ("two", 13)):
            _git(repo, "checkout", "-q", "-b", f"topic/{name}", "main")
            commit(f"graph/{name}.md", f"Add part {name}", f"2026-09-24T{hour}:00:00+00:00")
        _git(repo, "checkout", "-q", "main")
        _git(repo, "merge", "-q", "--no-ff", "-m", "Merge parts one and two", "topic/one", "topic/two",
             date="2026-09-24T14:00:00+00:00")
        # a branch that is not merged (All branches)
        _git(repo, "checkout", "-q", "-b", "topic/later", "main")
        commit("graph/later.md", "Start the next part", "2026-09-24T15:00:00+00:00")
        _git(repo, "checkout", "-q", "main")
    if signed:
        _signed(parent, repo)
    if reflog:
        def day(hour: int) -> str:
            return f"2026-09-25T{hour:02}:00:00+00:00"

        def commit_at(rel: str, summary: str, hour: int):
            p = repo / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(f"{summary}\n")
            _git(repo, "add", "-A", date=day(hour))
            _git(repo, "commit", "-q", "-m", summary, date=day(hour))

        _git(repo, "checkout", "-q", "-b", "topic/rebase", "HEAD~1", date=day(8))
        commit_at("notes/one.md", "Write the first note", 8)
        commit_at("notes/two.md", "Write the second note", 9)
        _git(repo, "rebase", "-q", "main", date=day(10))
        _git(repo, "checkout", "-q", "main", date=day(10))
        _git(repo, "checkout", "-q", "-b", "experiment", date=day(11))
        commit_at("experiment/idea.md", "Try an idea", 11)
        commit_at("experiment/more.md", "Push the idea further", 12)
        _git(repo, "checkout", "-q", "main", date=day(13))
        _git(repo, "branch", "-q", "-D", "experiment", date=day(13))
        for hour, name in ((14, "A"), (15, "B"), (16, "C")):
            commit_at(f"drafts/{name}.md", f"Draft {name}", hour)
        _git(repo, "reset", "-q", "--hard", "HEAD~3", date=day(17))
    if remote:
        bare = parent / f"{NAME}.git"
        if bare.exists():
            remove_tree(bare)
        _git(repo, "init", "-q", "--bare", str(bare))
        _git(repo, "remote", "add", "origin", str(bare))
        # the two newest commits and the tag stay behind
        _git(repo, "push", "-q", "origin", "main~2:refs/heads/main")
        _git(repo, "branch", "-q", "--set-upstream-to=origin/main", "main")
    if remote and remotes:
        _remotes(parent, repo)
    if tools:
        _tools(parent, repo)
    if structure:
        _structure(parent, repo, lfs_url)
    if pull_request and remote:
        _git(repo, "push", "-q", "origin", "main")
        # the fixture already has a stale `feature/login`: reset it onto main
        _git(repo, "checkout", "-q", "-B", "feature/login", "main")
        for date, summary, files in _PULL_REQUEST_COMMITS:
            for rel, text in files.items():
                p = repo / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
            _git(repo, "add", "-A")
            _git(repo, "commit", "-q", "-m", summary, date=date)
        _git(repo, "push", "-q", "-u", "origin", "feature/login")
    for rel, text in _WORKING_CHANGES.items():
        p = repo / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
    for rel in _DELETED:
        (repo / rel).unlink()
    return repo


if __name__ == "__main__":
    import sys

    print(build(Path(sys.argv[1] if len(sys.argv) > 1 else ".")))
