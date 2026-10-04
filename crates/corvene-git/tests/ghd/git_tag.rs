//! Port of GitHub Desktop's `app/test/unit/git/tag-test.ts`.
//!
//! Corvene equivalents of what the tests call:
//!
//! - `createTag(repository, name, sha)` is `corvene_git::create_tag` with an
//!   empty message (`git tag -a -m '' <name> <sha>`, GitHub Desktop's call;
//!   a message is flag `823`'s addition), `deleteTag` is
//!   `corvene_git::delete_tag`.
//! - `getCommit` / `getCommits(repository, 'HEAD', n)` are
//!   `corvene_git::get_commits` ([`get_commit`]); a commit's `tags` are
//!   `Commit::tags`.
//! - `getAllTags(repository)` (a map of tag name to commit) is
//!   `corvene_git::get_all_tags` ([`get_all_tags`]).
//! - `fetchTagsToPush(repository, remote, branch)` is
//!   `corvene_git::fetch_tags_to_push` ([`fetch_tags_to_push`]). The setup
//!   runs Corvene's own calls: `getRemotes` / `findDefaultRemote` are `corvene_git::get_remotes`
//!   / `find_default_remote`, `push` is `corvene_git::push`, `createBranch`
//!   is `corvene_git::create_branch`, `getBranches` is [`get_branches`],
//!   `checkoutBranch` is `corvene_git::checkout_branch`, `createCommit` is
//!   [`create_commit`].

use std::collections::HashMap;

use corvene_models::Remote;
use corvene_test_support::{
    TestRepo, create_commit, get_branches, get_commit, get_status_or_throw, git, git_error_message,
    setup_fixture_repository, setup_local_fork_of_repository,
};

/// GitHub Desktop's `createTag(repository, name, targetCommitSha)`.
fn create_tag(
    repository: &TestRepo,
    name: &str,
    target_commit_sha: &str,
) -> Result<(), corvene_git::GitError> {
    corvene_git::create_tag(git(), repository.path(), name, target_commit_sha, "")
}

/// GitHub Desktop's `getAllTags(repository)` (`lib/git/tag.ts`).
fn get_all_tags(repository: &TestRepo) -> HashMap<String, String> {
    corvene_git::get_all_tags(repository.path()).expect("getAllTags")
}

/// GitHub Desktop's `fetchTagsToPush(repository, remote, branchName)`.
fn fetch_tags_to_push(repository: &TestRepo, remote: &Remote, branch_name: &str) -> Vec<String> {
    corvene_git::fetch_tags_to_push(git(), repository.path(), &remote.name, branch_name, None)
        .expect("fetchTagsToPush")
}

// GHD: unit/git/tag-test.ts › git/tag › createTag › creates a tag with the given name
#[test]
fn creates_a_tag_with_the_given_name() {
    let repository = setup_fixture_repository("test-repo");

    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");

    let commit = get_commit(&repository, "HEAD");
    let commit = commit.expect("commit");
    assert_eq!(commit.tags, vec!["my-new-tag"]);
}

// GHD: unit/git/tag-test.ts › git/tag › createTag › creates a tag with the a comma in it
#[test]
fn creates_a_tag_with_the_a_comma_in_it() {
    let repository = setup_fixture_repository("test-repo");

    create_tag(&repository, "my-new-tag,has-a-comma", "HEAD").expect("createTag");

    let commit = get_commit(&repository, "HEAD");
    let commit = commit.expect("commit");
    assert_eq!(commit.tags, vec!["my-new-tag,has-a-comma"]);
}

// GHD: unit/git/tag-test.ts › git/tag › createTag › creates multiple tags
#[test]
fn creates_multiple_tags() {
    let repository = setup_fixture_repository("test-repo");

    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");
    create_tag(&repository, "another-tag", "HEAD").expect("createTag");

    let commit = get_commit(&repository, "HEAD");
    let commit = commit.expect("commit");
    assert_eq!(commit.tags, vec!["my-new-tag", "another-tag"]);
}

// GHD: unit/git/tag-test.ts › git/tag › createTag › creates a tag on a specified commit
#[test]
fn creates_a_tag_on_a_specified_commit() {
    let repository = setup_fixture_repository("test-repo");

    let commits = corvene_git::get_commits(repository.path(), "HEAD", 0, 2).expect("getCommits");
    let commit_sha = commits[1].sha.clone();

    create_tag(&repository, "my-new-tag", &commit_sha).expect("createTag");

    let commit = get_commit(&repository, &commit_sha);

    let commit = commit.expect("commit");
    assert_eq!(commit.tags, vec!["my-new-tag"]);
}

// GHD: unit/git/tag-test.ts › git/tag › createTag › fails when creating a tag with a name that already exists
#[test]
fn fails_when_creating_a_tag_with_a_name_that_already_exists() {
    let repository = setup_fixture_repository("test-repo");

    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");

    let err = create_tag(&repository, "my-new-tag", "HEAD").expect_err("createTag rejects");
    // `/already exists/i`
    let message = git_error_message(&err);
    assert!(
        message.to_lowercase().contains("already exists"),
        "{message}"
    );
}

// GHD: unit/git/tag-test.ts › git/tag › deleteTag › deletes a tag with the given name
#[test]
fn deletes_a_tag_with_the_given_name() {
    let repository = setup_fixture_repository("test-repo");

    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");
    corvene_git::delete_tag(git(), repository.path(), "my-new-tag").expect("deleteTag");

    let commit = get_commit(&repository, "HEAD");
    assert_eq!(commit.map(|c| c.tags.len()), Some(0));
}

// GHD: unit/git/tag-test.ts › git/tag › getAllTags › returns an empty map when the repository has no tags
#[test]
fn returns_an_empty_map_when_the_repository_has_no_tags() {
    let repository = setup_fixture_repository("test-repo");

    assert!(get_all_tags(&repository).is_empty());
}

// GHD: unit/git/tag-test.ts › git/tag › getAllTags › returns all the created tags
#[test]
fn returns_all_the_created_tags() {
    let repository = setup_fixture_repository("test-repo");

    let commit = get_commit(&repository, "HEAD");
    let commit = commit.expect("commit");

    create_tag(&repository, "my-new-tag", &commit.sha).expect("createTag");
    create_tag(&repository, "another-tag", &commit.sha).expect("createTag");

    assert_eq!(
        get_all_tags(&repository),
        HashMap::from([
            ("my-new-tag".to_string(), commit.sha.clone()),
            ("another-tag".to_string(), commit.sha.clone()),
        ])
    );
}

/// `fetchTagsToPush`'s `setup`: a local fork of `test-repo-with-tags` and
/// its `origin`.
fn fetch_tags_setup() -> (TestRepo, Remote, TestRepo) {
    let remote_repository = setup_fixture_repository("test-repo-with-tags");
    let repository = setup_local_fork_of_repository(&remote_repository);

    let remotes = corvene_git::get_remotes(git(), repository.path()).expect("getRemotes");
    let origin_remote = corvene_git::find_default_remote(&remotes)
        .cloned()
        .expect("couldn't find origin remote");

    (repository, origin_remote, remote_repository)
}

// GHD: unit/git/tag-test.ts › git/tag › fetchTagsToPush › returns an empty array when there are no tags to get pushed
#[test]
fn returns_an_empty_array_when_there_are_no_tags_to_get_pushed() {
    let (repository, origin_remote, _remote) = fetch_tags_setup();
    assert_eq!(
        fetch_tags_to_push(&repository, &origin_remote, "master").len(),
        0
    );
}

// GHD: unit/git/tag-test.ts › git/tag › fetchTagsToPush › returns local tags that haven't been pushed
#[test]
fn returns_local_tags_that_havent_been_pushed() {
    let (repository, origin_remote, _remote) = fetch_tags_setup();
    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");

    assert_eq!(
        fetch_tags_to_push(&repository, &origin_remote, "master"),
        vec!["my-new-tag"]
    );
}

// GHD: unit/git/tag-test.ts › git/tag › fetchTagsToPush › returns an empty array after pushing the tag
#[test]
fn returns_an_empty_array_after_pushing_the_tag() {
    let (repository, origin_remote, _remote) = fetch_tags_setup();
    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");

    corvene_git::push(
        git(),
        repository.path(),
        &origin_remote.name,
        "master",
        None,
        &["my-new-tag".to_string()],
        false,
        None,
        &mut |_, _| {},
    )
    .expect("push");

    assert_eq!(
        fetch_tags_to_push(&repository, &origin_remote, "master"),
        Vec::<String>::new()
    );
}

// GHD: unit/git/tag-test.ts › git/tag › fetchTagsToPush › does not return a tag created on a non-pushed branch
#[test]
fn does_not_return_a_tag_created_on_a_non_pushed_branch() {
    let (repository, origin_remote, _remote) = fetch_tags_setup();
    // Create a tag on a local branch that's not pushed to the remote.
    let branch_name = "new-branch";
    corvene_git::create_branch(git(), repository.path(), branch_name, Some("master"), false)
        .expect("createBranch");
    let branch = get_branches(repository.path(), &[&format!("refs/heads/{branch_name}")])
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("Could not create branch {branch_name}"));

    std::fs::write(repository.join("README.md"), "Hi world\n").unwrap();
    let status = get_status_or_throw(&repository);
    let files = status.files;

    corvene_git::checkout_branch(git(), repository.path(), &branch).expect("checkoutBranch");
    let commit_sha = create_commit(&repository, "a commit", &files);
    create_tag(&repository, "my-new-tag", &commit_sha).expect("createTag");

    assert_eq!(
        fetch_tags_to_push(&repository, &origin_remote, "master"),
        Vec::<String>::new()
    );
}

// GHD: unit/git/tag-test.ts › git/tag › fetchTagsToPush › returns unpushed tags even if it fails to push the branch
#[test]
fn returns_unpushed_tags_even_if_it_fails_to_push_the_branch() {
    // Create a new commit on the remote repository so the `git push` command
    // that fetchUnpushedTags() does fails.
    let (repository, origin_remote, remote_repository) = fetch_tags_setup();
    std::fs::write(remote_repository.join("README.md"), "Hi world\n").unwrap();
    let status = get_status_or_throw(&remote_repository);
    let files = status.files;
    create_commit(&remote_repository, "a commit", &files);

    create_tag(&repository, "my-new-tag", "HEAD").expect("createTag");

    assert_eq!(
        fetch_tags_to_push(&repository, &origin_remote, "master"),
        vec!["my-new-tag"]
    );
}
