//! Port of GitHub Desktop's `app/test/unit/git/worktree-test.ts`.
//!
//! Corvene equivalents of GitHub Desktop's `lib/git/worktree.ts`:
//!
//! - `parseWorktreePorcelainOutput(stdout)` is
//!   `corvene_git::parse_worktree_porcelain` (bytes in, `WorktreeEntry`s
//!   out: `type` is `kind`, `isDetached` / `isLocked` / `isPrunable` are
//!   `is_detached` / `is_locked` / `is_prunable`, `branch: null` is `None`,
//!   paths are `PathBuf`s).
//! - `listWorktrees(repository)` is `corvene_git::list_worktrees`.
//! - `listWorktreesFromGitDir(gitDir)` has no Corvene function
//!   ([`list_worktrees_from_git_dir`] is a stand-in).
//! - `resolveMainWorktreePath(repository)` has no Corvene function either:
//!   `Dispatcher::recover_missing_worktree` (`corvene-core/src/worktrees.rs`)
//!   picks the main worktree inline from the recorded `main_worktree_path`
//!   or the last `git worktree list`, and Corvene's `Repository` has no
//!   `gitDir`. [`resolve_main_worktree_path`] is a stand-in that takes the
//!   git dir beside the repository. Corvene records `main_worktree_path` on
//!   every refresh and has no git-dir fallback (`.docs/deviations.md`, "A
//!   deleted linked worktree falls back to its main worktree").
//!
//! GitHub Desktop's tests leave the linked worktrees they add beside the
//! temporary repository (`<repo>-wt-a`); [`Siblings`] removes them.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use corvene_models::{Repository, WorktreeEntry, WorktreeType};
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, exec, git, make_commit, setup_empty_repository_with_default_branch,
};

/// Stand-in for GitHub Desktop's `listWorktreesFromGitDir(gitDir)`
/// (`lib/git/worktree.ts`): `git --git-dir <gitDir> worktree list
/// --porcelain -z` run in `gitDir`. Replace this with the `corvene_git`
/// function once there is one and remove the `#[ignore]`.
fn list_worktrees_from_git_dir(_git_dir: &Path) -> Vec<WorktreeEntry> {
    unimplemented!("corvene_git has no listWorktreesFromGitDir")
}

/// Stand-in for GitHub Desktop's `resolveMainWorktreePath(repository)`
/// (`lib/git/worktree.ts`) with `repository.gitDir` passed beside the
/// Corvene `Repository` (which has `path` and `main_worktree_path` but no
/// git dir). Replace this with the Corvene function once there is one and
/// remove the `#[ignore]`s.
fn resolve_main_worktree_path(
    _repository: &Repository,
    _git_dir: Option<&Path>,
) -> Option<PathBuf> {
    unimplemented!("Corvene has no resolveMainWorktreePath")
}

/// `repo.path + suffix`.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s: OsString = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Removes the linked worktree directories a test adds beside its
/// repository when it ends.
struct Siblings(Vec<PathBuf>);

impl Siblings {
    fn add(&mut self, path: &Path) -> PathBuf {
        self.0.push(path.to_path_buf());
        path.to_path_buf()
    }
}

impl Drop for Siblings {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn entry_path(path: &str) -> PathBuf {
    // GitHub Desktop's `Path.normalize(path)`
    PathBuf::from(path)
}

fn parse(output: &str) -> Vec<WorktreeEntry> {
    corvene_git::parse_worktree_porcelain(output.as_bytes())
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › returns empty array for empty output
#[test]
fn returns_empty_array_for_empty_output() {
    assert_eq!(parse(""), vec![]);
    assert_eq!(parse("  \n  "), vec![]);
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses a single main worktree
#[test]
fn parses_a_single_main_worktree() {
    let output = [
        "worktree /path/to/repo",
        "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
        "branch refs/heads/main",
    ]
    .join("\0")
        + "\0";

    let entries = parse(&output);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        WorktreeEntry {
            path: entry_path("/path/to/repo"),
            head: "abc1234abc1234abc1234abc1234abc1234abc123".to_string(),
            branch: Some("refs/heads/main".to_string()),
            is_detached: false,
            kind: WorktreeType::Main,
            is_locked: false,
            is_prunable: false,
        }
    );
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses multiple worktrees
#[test]
fn parses_multiple_worktrees() {
    let output = [
        [
            "worktree /path/to/repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/linked",
            "HEAD def5678def5678def5678def5678def5678def567",
            "branch refs/heads/feature",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].kind, WorktreeType::Main);
    assert_eq!(entries[0].path, entry_path("/path/to/repo"));

    assert_eq!(entries[1].kind, WorktreeType::Linked);
    assert_eq!(entries[1].path, entry_path("/path/to/linked"));
    assert_eq!(entries[1].branch.as_deref(), Some("refs/heads/feature"));
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses detached HEAD worktree
#[test]
fn parses_detached_head_worktree() {
    let output = [
        [
            "worktree /path/to/repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/detached",
            "HEAD def5678def5678def5678def5678def5678def567",
            "detached",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert_eq!(entries.len(), 2);

    assert!(entries[1].is_detached);
    assert_eq!(entries[1].branch, None);
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses locked worktree
#[test]
fn parses_locked_worktree() {
    let output = [
        [
            "worktree /path/to/repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/locked-wt",
            "HEAD def5678def5678def5678def5678def5678def567",
            "branch refs/heads/locked-branch",
            "locked",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert!(entries[1].is_locked);
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses locked worktree with reason
#[test]
fn parses_locked_worktree_with_reason() {
    let output = [
        [
            "worktree /path/to/repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/locked-wt",
            "HEAD def5678def5678def5678def5678def5678def567",
            "branch refs/heads/locked-branch",
            "locked reason why it is locked",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert!(entries[1].is_locked);
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses prunable worktree
#[test]
fn parses_prunable_worktree() {
    let output = [
        [
            "worktree /path/to/repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/prunable-wt",
            "HEAD def5678def5678def5678def5678def5678def567",
            "branch refs/heads/stale",
            "prunable gitdir file points to non-existent location",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert!(entries[1].is_prunable);
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses paths with spaces
#[test]
fn parses_paths_with_spaces() {
    let output = [
        [
            "worktree /path/to/my repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/my other worktree",
            "HEAD def5678def5678def5678def5678def5678def567",
            "branch refs/heads/feature",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert_eq!(entries[0].path, entry_path("/path/to/my repo"));
    assert_eq!(entries[1].path, entry_path("/path/to/my other worktree"));
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses worktree with locked and prunable flags combined
#[test]
fn parses_worktree_with_locked_and_prunable_flags_combined() {
    let output = [
        [
            "worktree /path/to/repo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/bad-wt",
            "HEAD def5678def5678def5678def5678def5678def567",
            "detached",
            "locked",
            "prunable",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert!(entries[1].is_detached);
    assert!(entries[1].is_locked);
    assert!(entries[1].is_prunable);
    assert_eq!(entries[1].branch, None);
}

// GHD: unit/git/worktree-test.ts › git/worktree › parseWorktreePorcelainOutput › parses paths with newlines
#[test]
fn parses_paths_with_newlines() {
    let output = [
        [
            "worktree /path/to/my\nrepo",
            "HEAD abc1234abc1234abc1234abc1234abc1234abc123",
            "branch refs/heads/main",
        ]
        .join("\0"),
        [
            "worktree /path/to/my\nother\nworktree",
            "HEAD def5678def5678def5678def5678def5678def567",
            "branch refs/heads/feature",
        ]
        .join("\0"),
    ]
    .join("\0\0")
        + "\0";

    let entries = parse(&output);
    assert_eq!(entries[0].path, entry_path("/path/to/my\nrepo"));
    assert_eq!(entries[1].path, entry_path("/path/to/my\nother\nworktree"));
}

/// `listWorktrees`' `checkedOutBranches(worktrees)`: the branch refs of the
/// worktrees.
fn checked_out_branches(worktrees: &[WorktreeEntry]) -> HashSet<String> {
    worktrees
        .iter()
        .filter_map(|wt| wt.branch.clone())
        .collect()
}

fn list_worktrees(repo: &TestRepo) -> Vec<WorktreeEntry> {
    corvene_git::list_worktrees(git(), repo.path()).expect("listWorktrees")
}

/// `setupEmptyRepository(t, 'main')` with `README` committed.
fn main_repo_with_readme() -> TestRepo {
    let repo = setup_empty_repository_with_default_branch("main");
    make_commit(&repo, &Tree::new([TreeEntry::new("README", "hello")]));
    repo
}

// GHD: unit/git/worktree-test.ts › git/worktree › listWorktrees › returns only main worktree branch when there are no linked worktrees
#[test]
fn returns_only_main_worktree_branch_when_there_are_no_linked_worktrees() {
    let repo = main_repo_with_readme();

    let branches = checked_out_branches(&list_worktrees(&repo));
    assert_eq!(branches.len(), 1);
    assert!(branches.contains("refs/heads/main"));
}

// GHD: unit/git/worktree-test.ts › git/worktree › listWorktrees › returns branches checked out in linked worktrees
#[test]
fn returns_branches_checked_out_in_linked_worktrees() {
    let repo = main_repo_with_readme();
    let mut siblings = Siblings(Vec::new());
    exec(["branch", "feature-a"], repo.path());
    let wt_a = siblings.add(&with_suffix(repo.path(), "-wt-a"));
    exec(
        [
            OsString::from("worktree"),
            "add".into(),
            wt_a.into_os_string(),
            "feature-a".into(),
        ],
        repo.path(),
    );

    let branches = checked_out_branches(&list_worktrees(&repo));
    assert!(branches.contains("refs/heads/feature-a"));
    assert!(branches.contains("refs/heads/main"));
    assert_eq!(branches.len(), 2);
}

// GHD: unit/git/worktree-test.ts › git/worktree › listWorktrees › handles multiple linked worktrees
#[test]
fn handles_multiple_linked_worktrees() {
    let repo = main_repo_with_readme();
    let mut siblings = Siblings(Vec::new());
    exec(["branch", "feature-a"], repo.path());
    exec(["branch", "feature-b"], repo.path());
    let wt_a = siblings.add(&with_suffix(repo.path(), "-wt-a"));
    exec(
        [
            OsString::from("worktree"),
            "add".into(),
            wt_a.into_os_string(),
            "feature-a".into(),
        ],
        repo.path(),
    );
    let wt_b = siblings.add(&with_suffix(repo.path(), "-wt-b"));
    exec(
        [
            OsString::from("worktree"),
            "add".into(),
            wt_b.into_os_string(),
            "feature-b".into(),
        ],
        repo.path(),
    );

    let branches = checked_out_branches(&list_worktrees(&repo));
    assert!(branches.contains("refs/heads/feature-a"));
    assert!(branches.contains("refs/heads/feature-b"));
    assert!(branches.contains("refs/heads/main"));
    assert_eq!(branches.len(), 3);
}

// GHD: unit/git/worktree-test.ts › git/worktree › listWorktrees › handles detached HEAD worktrees
#[test]
fn handles_detached_head_worktrees() {
    let repo = main_repo_with_readme();
    let mut siblings = Siblings(Vec::new());

    let stdout = exec(["rev-parse", "HEAD"], repo.path()).stdout;
    let sha = stdout.trim();
    let wt = siblings.add(&with_suffix(repo.path(), "-wt-detached"));
    exec(
        [
            OsString::from("worktree"),
            "add".into(),
            "--detach".into(),
            wt.into_os_string(),
            sha.into(),
        ],
        repo.path(),
    );

    let branches = checked_out_branches(&list_worktrees(&repo));
    assert_eq!(branches.len(), 1);
    assert!(branches.contains("refs/heads/main"));
}

/// `git rev-parse --git-dir` in `worktree_path`, resolved against it
/// (`Path.resolve(worktreePath, stdout.trim())`).
fn git_dir_of(worktree_path: &Path) -> PathBuf {
    let stdout = exec(["rev-parse", "--git-dir"], worktree_path).stdout;
    worktree_path.join(stdout.trim())
}

// GHD: unit/git/worktree-test.ts › git/worktree › listWorktrees › lists worktrees from a git dir after a linked worktree directory is removed
#[test]
#[ignore = "ghd: missing: corvene_git has no listWorktreesFromGitDir (lib/git/worktree.ts)"]
fn lists_worktrees_from_a_git_dir_after_a_linked_worktree_directory_is_removed() {
    let repo = main_repo_with_readme();
    let mut siblings = Siblings(Vec::new());
    exec(["branch", "feature-a"], repo.path());

    let worktree_path = siblings.add(&with_suffix(repo.path(), "-wt-a"));
    exec(
        [
            OsString::from("worktree"),
            "add".into(),
            worktree_path.clone().into_os_string(),
            "feature-a".into(),
        ],
        repo.path(),
    );

    let git_dir = git_dir_of(&worktree_path);

    std::fs::remove_dir_all(&worktree_path).unwrap();

    let worktrees = list_worktrees_from_git_dir(&git_dir);
    let main_worktree = worktrees.iter().find(|wt| wt.kind == WorktreeType::Main);
    let repo_path = std::fs::canonicalize(repo.path()).unwrap();
    let resolved_worktree_path = with_suffix(&repo_path, "-wt-a");

    assert_eq!(main_worktree.map(|wt| wt.path.clone()), Some(repo_path));
    assert!(
        worktrees
            .iter()
            .any(|wt| wt.path == resolved_worktree_path && wt.is_prunable)
    );
}

/// `resolveMainWorktreePath`'s `setupWorktree(t)`: a repository with one
/// linked worktree; the realpath of the main worktree, the linked worktree
/// path and the linked worktree's admin git dir.
struct WorktreeSetup {
    repo: TestRepo,
    main_path: PathBuf,
    worktree_path: PathBuf,
    git_dir: PathBuf,
    _siblings: Siblings,
}

fn setup_worktree() -> WorktreeSetup {
    let repo = main_repo_with_readme();
    let mut siblings = Siblings(Vec::new());
    exec(["branch", "feature-a"], repo.path());

    let worktree_path = siblings.add(&with_suffix(repo.path(), "-wt-a"));
    exec(
        [
            OsString::from("worktree"),
            "add".into(),
            worktree_path.clone().into_os_string(),
            "feature-a".into(),
        ],
        repo.path(),
    );

    let git_dir = git_dir_of(&worktree_path);

    WorktreeSetup {
        main_path: std::fs::canonicalize(repo.path()).unwrap(),
        repo,
        worktree_path,
        git_dir,
        _siblings: siblings,
    }
}

/// `new Repository(path, 1, null, false, null, {}, false, gitDir,
/// mainWorktreePath)` without the git dir, which Corvene's model lacks.
fn repository(path: &Path, main_worktree_path: Option<PathBuf>) -> Repository {
    let mut repository = Repository::new(1, path);
    repository.main_worktree_path = main_worktree_path;
    repository
}

// GHD: unit/git/worktree-test.ts › git/worktree › resolveMainWorktreePath › resolves from the persisted path once the worktree metadata is gone
#[test]
#[ignore = "ghd: missing: Corvene has no resolveMainWorktreePath (lib/git/worktree.ts); Dispatcher::recover_missing_worktree picks the main worktree inline"]
fn resolves_from_the_persisted_path_once_the_worktree_metadata_is_gone() {
    let setup = setup_worktree();

    // Desktop switched onto the worktree, recording the main worktree it
    // came from.
    let selected = repository(&setup.worktree_path, Some(setup.main_path.clone()));
    let git_dir = Some(setup.git_dir.as_path());

    // `git worktree remove` deletes the working directory *and* the admin
    // metadata under <main>/.git/worktrees/<name>.
    exec(
        [
            OsString::from("worktree"),
            "remove".into(),
            "--force".into(),
            setup.worktree_path.clone().into_os_string(),
        ],
        setup.repo.path(),
    );

    assert_eq!(
        resolve_main_worktree_path(&selected, git_dir),
        Some(setup.main_path.clone())
    );
}

// GHD: unit/git/worktree-test.ts › git/worktree › resolveMainWorktreePath › falls back to the git dir when no path was persisted
#[test]
#[ignore = "ghd: deviation: deviations.md 'A deleted linked worktree falls back to its main worktree': Corvene keeps no git dir per repository, so an entry saved without main_worktree_path cannot recover (no git-dir fallback)"]
fn falls_back_to_the_git_dir_when_no_path_was_persisted() {
    let setup = setup_worktree();

    // A repository record written before the main worktree path was
    // persisted has a git dir but no main worktree path.
    let selected = repository(&setup.worktree_path, None);
    let git_dir = Some(setup.git_dir.as_path());

    // A plain delete leaves the admin metadata intact, so the worktree set
    // is still discoverable through it.
    std::fs::remove_dir_all(&setup.worktree_path).unwrap();

    assert_eq!(
        resolve_main_worktree_path(&selected, git_dir),
        Some(setup.main_path.clone())
    );
}

// GHD: unit/git/worktree-test.ts › git/worktree › resolveMainWorktreePath › returns null when the repository is already the main worktree
#[test]
#[ignore = "ghd: missing: Corvene has no resolveMainWorktreePath (lib/git/worktree.ts); Dispatcher::recover_missing_worktree picks the main worktree inline"]
fn returns_null_when_the_repository_is_already_the_main_worktree() {
    let setup = setup_worktree();

    let selected = repository(&setup.main_path, Some(setup.main_path.clone()));
    let git_dir = setup.repo.join(".git");

    assert_eq!(resolve_main_worktree_path(&selected, Some(&git_dir)), None);
}

// GHD: unit/git/worktree-test.ts › git/worktree › resolveMainWorktreePath › falls back to the git dir when the persisted path is stale
#[test]
#[ignore = "ghd: deviation: deviations.md 'A deleted linked worktree falls back to its main worktree': Corvene keeps no git dir per repository (it re-records main_worktree_path on every refresh), so a stale recorded path has no git-dir fallback"]
fn falls_back_to_the_git_dir_when_the_persisted_path_is_stale() {
    let setup = setup_worktree();

    // A persisted path can outlive the location it names — a repository moved
    // outside Desktop, say. It shouldn't stop us resolving the main worktree
    // by the means that still work.
    let selected = repository(
        &setup.worktree_path,
        Some(setup.main_path.join("no").join("longer").join("here")),
    );
    let git_dir = Some(setup.git_dir.as_path());

    assert_eq!(
        resolve_main_worktree_path(&selected, git_dir),
        Some(setup.main_path.clone())
    );
}

// GHD: unit/git/worktree-test.ts › git/worktree › resolveMainWorktreePath › returns null when neither a persisted path nor a git dir is available
#[test]
#[ignore = "ghd: missing: Corvene has no resolveMainWorktreePath (lib/git/worktree.ts); Dispatcher::recover_missing_worktree picks the main worktree inline"]
fn returns_null_when_neither_a_persisted_path_nor_a_git_dir_is_available() {
    let setup = setup_worktree();

    let selected = repository(&setup.worktree_path, None);

    assert_eq!(resolve_main_worktree_path(&selected, None), None);
}
