//! Port of GitHub Desktop's `app/test/unit/format-commit-message-test.ts`.
//!
//! GitHub Desktop's `formatCommitMessage(repository, context)`
//! (`lib/format-commit-message.ts`) joins the summary and description and,
//! when there are trailers, merges them in with `git interpret-trailers`
//! (`mergeTrailers`). Corvene does the same in two steps, which
//! `Dispatcher::commit` (`corvene-core/src/dispatcher.rs`) runs in this
//! order: `corvene_git::format_message(summary, description)` then
//! `corvene_git::merge_trailers(git, workdir, message, trailers)`.
//! [`format_commit_message`] makes those two calls. GHD's `description:
//! null` is an empty description (the commit form's text box is never
//! null in Corvene) and an `ITrailer` is a `(token, value)` pair.

use corvene_test_support::{TestRepo, git, setup_empty_repository};

/// GitHub Desktop's `ICommitContext` (the fields these tests use).
struct CommitContext<'a> {
    summary: &'a str,
    description: Option<&'a str>,
    trailers: Option<Vec<(String, String)>>,
}

/// GitHub Desktop's `formatCommitMessage(repository, context)`.
fn format_commit_message(repository: &TestRepo, context: CommitContext<'_>) -> String {
    let message =
        corvene_git::format_message(context.summary, context.description.unwrap_or_default());
    let trailers = context.trailers.unwrap_or_default();
    corvene_git::merge_trailers(git(), repository.path(), &message, &trailers)
        .expect("mergeTrailers")
}

/// GitHub Desktop's `{ token, value }`.
fn trailer(token: &str, value: &str) -> (String, String) {
    (token.to_string(), value.to_string())
}

// GHD: unit/format-commit-message-test.ts › formatCommitMessage › always adds trailing newline
#[test]
fn always_adds_trailing_newline() {
    let repo = setup_empty_repository();

    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "test",
                description: None,
                trailers: None,
            }
        ),
        "test\n"
    );
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "test",
                description: Some("test"),
                trailers: None,
            }
        ),
        "test\n\ntest\n"
    );
}

// GHD: unit/format-commit-message-test.ts › formatCommitMessage › omits description when null
#[test]
fn omits_description_when_null() {
    let repo = setup_empty_repository();
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "test",
                description: None,
                trailers: None,
            }
        ),
        "test\n"
    );
}

// GHD: unit/format-commit-message-test.ts › formatCommitMessage › omits description when empty string
#[test]
fn omits_description_when_empty_string() {
    let repo = setup_empty_repository();
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "test",
                description: Some(""),
                trailers: None,
            }
        ),
        "test\n"
    );
}

// GHD: unit/format-commit-message-test.ts › formatCommitMessage › adds two newlines between summary and description
#[test]
fn adds_two_newlines_between_summary_and_description() {
    let repo = setup_empty_repository();
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "foo",
                description: Some("bar"),
                trailers: None,
            }
        ),
        "foo\n\nbar\n"
    );
}

// GHD: unit/format-commit-message-test.ts › formatCommitMessage › appends trailers to a summary-only message
#[test]
fn appends_trailers_to_a_summary_only_message() {
    let repo = setup_empty_repository();
    let trailers = vec![
        trailer("Co-Authored-By", "Markus Olsson <niik@github.com>"),
        trailer("Signed-Off-By", "nerdneha <nerdneha@github.com>"),
    ];
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "foo",
                description: None,
                trailers: Some(trailers),
            }
        ),
        "foo\n\n".to_string()
            + "Co-Authored-By: Markus Olsson <niik@github.com>\n"
            + "Signed-Off-By: nerdneha <nerdneha@github.com>\n"
    );
}

// GHD: unit/format-commit-message-test.ts › formatCommitMessage › appends trailers to a regular message
#[test]
fn appends_trailers_to_a_regular_message() {
    let repo = setup_empty_repository();
    let trailers = vec![
        trailer("Co-Authored-By", "Markus Olsson <niik@github.com>"),
        trailer("Signed-Off-By", "nerdneha <nerdneha@github.com>"),
    ];
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "foo",
                description: Some("bar"),
                trailers: Some(trailers),
            }
        ),
        "foo\n\nbar\n\n".to_string()
            + "Co-Authored-By: Markus Olsson <niik@github.com>\n"
            + "Signed-Off-By: nerdneha <nerdneha@github.com>\n"
    );
}

// note, this relies on the default git config
// GHD: unit/format-commit-message-test.ts › formatCommitMessage › merges duplicate trailers
#[test]
fn merges_duplicate_trailers() {
    let repo = setup_empty_repository();
    let trailers = vec![
        trailer("Co-Authored-By", "Markus Olsson <niik@github.com>"),
        trailer("Signed-Off-By", "nerdneha <nerdneha@github.com>"),
    ];
    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "foo",
                description: Some("Co-Authored-By: Markus Olsson <niik@github.com>"),
                trailers: Some(trailers),
            }
        ),
        "foo\n\n".to_string()
            + "Co-Authored-By: Markus Olsson <niik@github.com>\n"
            + "Signed-Off-By: nerdneha <nerdneha@github.com>\n"
    );
}

// note, this relies on the default git config
// GHD: unit/format-commit-message-test.ts › formatCommitMessage › fixes up malformed trailers when trailers are given
#[test]
fn fixes_up_malformed_trailers_when_trailers_are_given() {
    let repo = setup_empty_repository();
    let trailers = vec![trailer("Signed-Off-By", "nerdneha <nerdneha@github.com>")];

    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary: "foo",
                // note the lack of space after :
                description: Some("Co-Authored-By:Markus Olsson <niik@github.com>"),
                trailers: Some(trailers),
            }
        ),
        "foo\n\n".to_string()
            + "Co-Authored-By: Markus Olsson <niik@github.com>\n"
            + "Signed-Off-By: nerdneha <nerdneha@github.com>\n"
    );
}

// note, this relies on the default git config
// GHD: unit/format-commit-message-test.ts › formatCommitMessage › doesn't treat --- as end of commit message
#[test]
fn doesnt_treat_dashes_as_end_of_commit_message() {
    let repo = setup_empty_repository();
    let trailers = vec![trailer("Signed-Off-By", "nerdneha <nerdneha@github.com>")];

    let summary = "foo";
    let description = "hello\n---\nworld\n\nCo-Authored-By: Markus Olsson <niik@github.com>";

    assert_eq!(
        format_commit_message(
            &repo,
            CommitContext {
                summary,
                description: Some(description),
                trailers: Some(trailers),
            }
        ),
        "foo\n\nhello\n---\nworld\n\n".to_string()
            + "Co-Authored-By: Markus Olsson <niik@github.com>\n"
            + "Signed-Off-By: nerdneha <nerdneha@github.com>\n"
    );
}
