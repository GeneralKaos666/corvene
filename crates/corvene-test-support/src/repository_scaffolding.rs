//! Port of `app/test/helpers/repository-scaffolding.ts`.

use std::ffi::OsStr;

use crate::exec::exec;
use crate::repositories::TestRepo;
use crate::temp::create_temp_directory;

/// GitHub Desktop's `TreeEntry`: one file [`make_commit`] writes or
/// removes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEntry {
    /// The relative path of the file in the repository.
    pub path: String,
    /// The contents to write; `None` (GitHub Desktop's `null`) removes the
    /// file with `git rm` before committing.
    pub contents: Option<Vec<u8>>,
}

impl TreeEntry {
    /// `{ path, contents }` (a string or a `Buffer`).
    pub fn new(path: impl Into<String>, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            contents: Some(contents.into()),
        }
    }

    /// `{ path, contents: null }`.
    pub fn removed(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: None,
        }
    }
}

/// GitHub Desktop's `Tree`: the entries of one commit and its optional
/// message (`'commit'` when `None` or empty).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
    pub commit_message: Option<String>,
}

impl Tree {
    /// `{ entries }`.
    pub fn new(entries: impl IntoIterator<Item = TreeEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            commit_message: None,
        }
    }

    /// `{ commitMessage, entries }`.
    pub fn with_message(
        commit_message: impl Into<String>,
        entries: impl IntoIterator<Item = TreeEntry>,
    ) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            commit_message: Some(commit_message.into()),
        }
    }
}

/// GitHub Desktop's `cloneRepository(t, repository)`: `git clone` of a
/// local repository into a new temporary directory, so push, pull and fetch
/// can be tested without the network. Like GitHub Desktop it does not check
/// the exit code. The clone keeps `repository` alive (its `origin`).
pub fn clone_repository(repository: &TestRepo) -> TestRepo {
    let dir = create_temp_directory();
    exec(
        [
            OsStr::new("clone"),
            repository.path().as_os_str(),
            OsStr::new("--"),
            dir.path().as_os_str(),
        ],
        dir.path(),
    );
    let mut clone = TestRepo::from_temp_dir(dir);
    clone.keep_alive(repository);
    clone
}

/// GitHub Desktop's `makeCommit(repository, tree)`: write each entry (or
/// `git rm` it when its contents are `None`) and `git add` it, then `git
/// commit -m <message>`. As in GitHub Desktop, parent directories are not
/// created and git's exit codes are not checked.
///
/// # Panics
///
/// When a file cannot be written (GitHub Desktop's `writeFile` rejects).
pub fn make_commit(repository: &TestRepo, tree: &Tree) {
    for entry in &tree.entries {
        match &entry.contents {
            None => {
                exec(["rm", entry.path.as_str()], repository.path());
            }
            Some(contents) => {
                let full_path = repository.join(&entry.path);
                std::fs::write(&full_path, contents)
                    .unwrap_or_else(|e| panic!("write {}: {e}", full_path.display()));
                exec(["add", entry.path.as_str()], repository.path());
            }
        }
    }
    let message = tree
        .commit_message
        .as_deref()
        .filter(|m| !m.is_empty())
        .unwrap_or("commit");
    exec(["commit", "-m", message], repository.path());
}

/// GitHub Desktop's `createBranch(repository, branch, startPoint)`: `git
/// branch <branch> <start_point>` without checking it out.
///
/// # Panics
///
/// When `branch` already resolves to something.
pub fn create_branch(repository: &TestRepo, branch: &str, start_point: &str) {
    let result = exec(["rev-parse", "--verify", branch], repository.path());
    if result.exit_code == 128 {
        // ref does not exist: create the branch
        exec(["branch", branch, start_point], repository.path());
    } else {
        panic!(
            "Branch {branch} already exists and resolves to '{}'",
            result.stdout
        );
    }
}

/// GitHub Desktop's `switchTo(repository, branch)`: `git checkout <branch>`,
/// or `git checkout -b <branch>` when it does not exist yet.
pub fn switch_to(repository: &TestRepo, branch: &str) {
    let result = exec(["rev-parse", "--verify", branch], repository.path());
    if result.exit_code == 128 {
        // ref does not exist: create the branch and check it out
        exec(["checkout", "-b", branch], repository.path());
    } else {
        exec(["checkout", branch], repository.path());
    }
}

/// GitHub Desktop's `cloneLocalRepository(t, repository)`: `git clone --
/// <repository> <new temporary directory>`, run in `repository`. The clone
/// keeps `repository` alive (its `origin`). GitHub Desktop's model of the
/// clone has `missing: true`, which none of the code its tests run reads;
/// [`TestRepo::model`] keeps `missing: false`.
///
/// # Panics
///
/// When git exits with 128.
pub fn clone_local_repository(repository: &TestRepo) -> TestRepo {
    let dir = create_temp_directory();
    let result = exec(
        [
            OsStr::new("clone"),
            OsStr::new("--"),
            repository.path().as_os_str(),
            dir.path().as_os_str(),
        ],
        repository.path(),
    );
    assert_ne!(result.exit_code, 128, "{result:?}");
    let mut clone = TestRepo::from_temp_dir(dir);
    clone.keep_alive(repository);
    clone
}
