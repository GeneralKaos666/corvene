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


def build(parent: Path, remote: bool = False, coauthors: bool = False) -> Path:
    """(Re)create `<parent>/parity-fixture` and return its path.

    With `remote`, a bare `<parent>/parity-fixture.git` is added as `origin`
    and `main` is pushed up to its third commit, so the branch is two ahead
    with the `v0.1.0` tag still to push (History's unpushed indicators, the
    toolbar's Push origin). With `coauthors`, `_CO_AUTHORED` commits go on
    top."""
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
    if remote:
        bare = parent / f"{NAME}.git"
        if bare.exists():
            remove_tree(bare)
        _git(repo, "init", "-q", "--bare", str(bare))
        _git(repo, "remote", "add", "origin", str(bare))
        # the two newest commits and the tag stay behind
        _git(repo, "push", "-q", "origin", "main~2:refs/heads/main")
        _git(repo, "branch", "-q", "--set-upstream-to=origin/main", "main")
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
