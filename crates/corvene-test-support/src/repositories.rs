//! Port of `app/test/helpers/repositories.ts`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;

use crate::exec::{exec, exec_ok};
use crate::fixture::get_fixture_path;
use crate::repository_scaffolding::{Tree, TreeEntry, make_commit, switch_to};
use crate::temp::create_temp_directory;

/// GitHub Desktop's `DefaultGitDescription` (`lib/git/description.ts`):
/// what `git init` writes to `.git/description`.
pub use corvene_git::DEFAULT_GIT_DESCRIPTION;

/// What GitHub Desktop's repository helpers return (a `Repository` model,
/// or a path for `setupFixtureRepository`): a directory in a temporary
/// location, removed once the last clone of the `TestRepo` is dropped.
/// Pass [`path`](Self::path) to `corvene_git` functions together with
/// [`git()`](crate::git).
#[derive(Clone, Debug)]
pub struct TestRepo {
    dir: Arc<TempDir>,
    path: PathBuf,
    /// Other temporary directories this repository needs ([`Self::keep_alive`]).
    companions: Vec<Arc<TempDir>>,
}

impl TestRepo {
    /// Take ownership of a temporary directory (from
    /// [`create_temp_directory`]); the repository is its root.
    pub fn from_temp_dir(dir: TempDir) -> Self {
        let path = dir.path().to_path_buf();
        Self {
            dir: Arc::new(dir),
            path,
            companions: Vec::new(),
        }
    }

    /// The working directory (GitHub Desktop's `repository.path`).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `path` joined with a relative path inside the repository.
    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.path.join(relative)
    }

    /// `<path>/.git`.
    pub fn git_dir(&self) -> PathBuf {
        self.path.join(".git")
    }

    /// The temporary directory this repository lives in (its root unless
    /// made with [`subdirectory`](Self::subdirectory)).
    pub fn temp_dir(&self) -> &Path {
        self.dir.path()
    }

    /// Another repository inside this one's temporary directory (a
    /// submodule, a nested fixture repository), kept alive by it:
    /// `new Repository(Path.join(repo.path, 'foo/submodule'), ...)`.
    pub fn subdirectory(&self, relative: impl AsRef<Path>) -> Self {
        Self {
            dir: self.dir.clone(),
            path: self.path.join(relative),
            companions: self.companions.clone(),
        }
    }

    /// Keep `other`'s temporary directory until this repository is dropped
    /// (a remote or submodule source this one points at). GitHub Desktop
    /// removes every temporary directory when the test ends; Rust removes
    /// each one when its last owner is dropped.
    pub fn keep_alive(&mut self, other: &TestRepo) {
        self.companions.push(other.dir.clone());
        self.companions.extend(other.companions.iter().cloned());
    }

    /// The `corvene_models::Repository` for store-level code: GitHub
    /// Desktop's `new Repository(path, -1, null, false)` (Corvene ids are
    /// unsigned, so the id is 0).
    pub fn model(&self) -> corvene_models::Repository {
        corvene_models::Repository::new(0, self.path.clone())
    }
}

impl AsRef<Path> for TestRepo {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

/// GitHub Desktop's `setupFixtureRepository(t, repositoryName)`: copy
/// `fixtures/<name>` into a new temporary directory and rename every `_git`
/// in it (directories and gitlink files, at any depth) to `.git`.
///
/// # Panics
///
/// When the fixture does not exist or cannot be copied.
pub fn setup_fixture_repository(repository_name: &str) -> TestRepo {
    let fixture = get_fixture_path([repository_name]);
    assert!(
        fixture.is_dir(),
        "no fixture repository {}",
        fixture.display()
    );
    let dir = create_temp_directory();
    copy_dir_all(&fixture, dir.path());

    let mut renames = Vec::new();
    collect_named(dir.path(), "_git", &mut renames);
    // deepest first, so renaming a directory never moves a match not yet renamed
    renames.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for from in renames {
        let to = from.with_file_name(".git");
        std::fs::rename(&from, &to)
            .unwrap_or_else(|e| panic!("rename {} to .git: {e}", from.display()));
    }
    TestRepo::from_temp_dir(dir)
}

fn copy_dir_all(from: &Path, to: &Path) {
    let entries =
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("read {}: {e}", from.display()));
        let source = entry.path();
        let target = to.join(entry.file_name());
        let kind = entry
            .file_type()
            .unwrap_or_else(|e| panic!("stat {}: {e}", source.display()));
        if kind.is_dir() {
            std::fs::create_dir(&target)
                .unwrap_or_else(|e| panic!("create {}: {e}", target.display()));
            copy_dir_all(&source, &target);
        } else {
            std::fs::copy(&source, &target)
                .unwrap_or_else(|e| panic!("copy {}: {e}", source.display()));
        }
    }
}

fn collect_named(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name() == name {
            out.push(path.clone());
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            collect_named(&path, name, out);
        }
    }
}

/// GitHub Desktop's `setupEmptyRepository(t)`: an empty repository on
/// `master`, written by hand rather than by `git init` (see
/// [`setup_empty_repository_with_default_branch`]).
pub fn setup_empty_repository() -> TestRepo {
    setup_empty_repository_with_default_branch("master")
}

/// GitHub Desktop's `setupEmptyRepository(t, defaultBranchName)`: a new
/// temporary directory with the `.git` skeleton GitHub Desktop writes
/// (`objects`, `refs/tags`, `refs/heads`, `info`, `HEAD` pointing at
/// `refs/heads/<default_branch>`, a `config` with `ignorecase = true` on
/// Linux only and `precomposeunicode = true`, and the default
/// `description`).
///
/// # Panics
///
/// When the files cannot be written.
pub fn setup_empty_repository_with_default_branch(default_branch: &str) -> TestRepo {
    let dir = create_temp_directory();
    let repo = dir.path();
    for sub in [
        ".git",
        ".git/objects",
        ".git/refs",
        ".git/refs/tags",
        ".git/refs/heads",
        ".git/info",
    ] {
        let path = repo.join(sub);
        std::fs::create_dir(&path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    }
    let ignore_case = if cfg!(target_os = "linux") {
        "true"
    } else {
        "false"
    };
    let files = [
        (".git/HEAD", format!("ref: refs/heads/{default_branch}\n")),
        (
            ".git/config",
            format!(
                "[core]\nrepositoryformatversion = 0\nfilemode = true\nbare = false\n\
                 logallrefupdates = true\nignorecase = {ignore_case}\nprecomposeunicode = true\n"
            ),
        ),
        (".git/description", DEFAULT_GIT_DESCRIPTION.to_string()),
    ];
    for (name, contents) in files {
        let path = repo.join(name);
        std::fs::write(&path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
    TestRepo::from_temp_dir(dir)
}

/// GitHub Desktop's `setupEmptyRepositoryDefaultMain(t)`: as
/// [`setup_empty_repository`], on `main`.
pub fn setup_empty_repository_default_main() -> TestRepo {
    setup_empty_repository_with_default_branch("main")
}

/// GitHub Desktop's `setupEmptyDirectory(t)`: an empty directory that is
/// not a repository, for testing how git errors are handled.
pub fn setup_empty_directory() -> TestRepo {
    TestRepo::from_temp_dir(create_temp_directory())
}

/// GitHub Desktop's `setupConflictedRepo(t)`: a merge of `master` into
/// `other-branch` (the current branch) that conflicts in `foo`.
pub fn setup_conflicted_repo() -> TestRepo {
    let repo = setup_empty_repository();
    make_commit(&repo, &Tree::new([TreeEntry::new("foo", "")]));
    // create this branch starting from the first commit, but don't checkout
    // it because we want to create a divergent history
    exec(["branch", "other-branch"], repo.path());
    make_commit(&repo, &Tree::new([TreeEntry::new("foo", "b1")]));
    switch_to(&repo, "other-branch");
    make_commit(&repo, &Tree::new([TreeEntry::new("foo", "b2")]));
    exec(["merge", "master"], repo.path());
    repo
}

/// GitHub Desktop's `setupConflictedRepoWithUnrelatedCommittedChange(t)`:
/// [`setup_conflicted_repo`] where `perlin`, committed as `perlin`, has the
/// uncommitted contents `noise` during the conflicted merge.
pub fn setup_conflicted_repo_with_unrelated_committed_change() -> TestRepo {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::new([
            TreeEntry::new("foo", ""),
            TreeEntry::new("perlin", "perlin"),
        ]),
    );
    exec(["branch", "other-branch"], repo.path());
    make_commit(&repo, &Tree::new([TreeEntry::new("foo", "b1")]));
    switch_to(&repo, "other-branch");
    make_commit(&repo, &Tree::new([TreeEntry::new("foo", "b2")]));
    write(&repo, "perlin", "noise");
    exec(["merge", "master"], repo.path());
    repo
}

/// GitHub Desktop's `setupConflictedRepoWithMultipleFiles(t)`: a merge of
/// `master` into `other-branch` (the current branch) that conflicts in
/// `foo`, `bar` (deleted on `master`), `baz` and `cat` (added on both), with
/// an untracked `dog`.
pub fn setup_conflicted_repo_with_multiple_files() -> TestRepo {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::new([TreeEntry::new("foo", "b0"), TreeEntry::new("bar", "b0")]),
    );
    exec(["branch", "other-branch"], repo.path());
    make_commit(
        &repo,
        &Tree::new([
            TreeEntry::new("foo", "b1"),
            TreeEntry::removed("bar"),
            TreeEntry::new("baz", "b1"),
            TreeEntry::new("cat", "b1"),
        ]),
    );
    switch_to(&repo, "other-branch");
    make_commit(
        &repo,
        &Tree::new([
            TreeEntry::new("foo", "b2"),
            TreeEntry::new("bar", "b2"),
            TreeEntry::new("baz", "b2"),
            TreeEntry::new("cat", "b2"),
        ]),
    );
    write(&repo, "dog", "touch");
    exec(["merge", "master"], repo.path());
    repo
}

/// GitHub Desktop's `setupTwoCommitRepo(t)`: `good-file` and `great-file`
/// added in one commit and changed in a second, on `master`.
pub fn setup_two_commit_repo() -> TestRepo {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::new([
            TreeEntry::new("good-file", "wishes it was great"),
            TreeEntry::new("great-file", "wishes it was good"),
        ]),
    );
    make_commit(
        &repo,
        &Tree::new([
            TreeEntry::new("good-file", "is great"),
            TreeEntry::new("great-file", "is good"),
        ]),
    );
    repo
}

/// GitHub Desktop's `setupLocalForkOfRepository(t, upstream)`: `git clone
/// --local` of `upstream` into a new temporary directory, so `origin`
/// points at the local "upstream" repository (kept alive by the fork).
///
/// # Panics
///
/// When the clone fails.
pub fn setup_local_fork_of_repository(upstream: &TestRepo) -> TestRepo {
    let dir = create_temp_directory();
    exec_ok(
        [
            OsStr::new("clone"),
            OsStr::new("--local"),
            upstream.path().as_os_str(),
            dir.path().as_os_str(),
        ],
        dir.path(),
    );
    let mut fork = TestRepo::from_temp_dir(dir);
    fork.keep_alive(upstream);
    fork
}

/// GitHub Desktop's `setupRepositoryWithUninitializedSubmodule(t)`:
/// [`setup_two_commit_repo`] with a branch `branch-with-submodule` that adds
/// another two-commit repository as the submodule `test-submodule`; the
/// current branch is `master` and the submodule is uninitialized (its
/// `.git/modules` entry and its directory are removed).
pub fn setup_repository_with_uninitialized_submodule() -> TestRepo {
    let mut repo = setup_two_commit_repo();
    let submodule = setup_two_commit_repo();
    exec(["checkout", "-b", "branch-with-submodule"], repo.path());
    exec(
        [
            OsStr::new("-c"),
            OsStr::new("protocol.file.allow=always"),
            OsStr::new("submodule"),
            OsStr::new("add"),
            submodule.path().as_os_str(),
            OsStr::new("test-submodule"),
        ],
        repo.path(),
    );
    exec(["commit", "-m", "Add submodule"], repo.path());
    exec(["checkout", "master"], repo.path());
    let _ = std::fs::remove_dir_all(repo.join(".git/modules/test-submodule"));
    let _ = std::fs::remove_dir_all(repo.join("test-submodule"));
    // `.gitmodules` points at the source: GitHub Desktop keeps it until the
    // test ends, so initializing the submodule later still works
    repo.keep_alive(&submodule);
    repo
}

fn write(repo: &TestRepo, relative: &str, contents: &str) {
    let path = repo.join(relative);
    std::fs::write(&path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}
