//! Sample data for API-backed UI that cannot be reached without a signed-in
//! GitHub account: the `CORVENE_POPUP` dev hook and the Test Notifications
//! dialog (GHD's "Test UI components" / `TestNotifications` play the same
//! role with real API data).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::host::Host;
use corvene_github::api::{
    ApiIdentity, ApiIssueComment, ApiPullRequestReview, ApiPullRequestReviewState,
};
use corvene_models::{
    CheckConclusion, CheckStatus, GitHubRepository, JobStep, PullRequest, PullRequestRef, RefCheck,
    WorkflowRun,
};

/// `seconds` ago as the API's ISO-8601 UTC timestamp.
pub fn iso_ago(seconds: u64) -> String {
    let t = SystemTime::now()
        .checked_sub(Duration::from_secs(seconds))
        .unwrap_or(UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (t / 86_400, t % 86_400);
    // civil-from-days (Howard Hinnant)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn octocat() -> ApiIdentity {
    ApiIdentity {
        id: 583_231,
        login: "octocat".into(),
        html_url: Some("https://github.com/octocat".into()),
        name: Some("The Octocat".into()),
        email: None,
        avatar_url: Some("https://avatars.githubusercontent.com/u/583231?v=4".into()),
        kind: Some("User".into()),
    }
}

/// A GitHub repository for repositories that have none.
fn stand_in_github_repository() -> GitHubRepository {
    GitHubRepository {
        endpoint: "https://api.github.com".into(),
        owner: "wasi-master".into(),
        name: "corvene-demo".into(),
        html_url: "https://github.com/wasi-master/corvene-demo".into(),
        clone_url: "https://github.com/wasi-master/corvene-demo.git".into(),
        default_branch: Some("main".into()),
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
        node_id: None,
    }
}

/// The selected repository's GitHub repository, or a stand-in.
pub fn github_repository(repo: u64, cx: &dyn Host) -> GitHubRepository {
    cx.state_ref()
        .repository(repo)
        .and_then(|r| r.github.clone())
        .unwrap_or_else(stand_in_github_repository)
}

pub fn pull_request(repo: u64, cx: &dyn Host) -> PullRequest {
    pull_request_in(github_repository(repo, cx))
}

/// The sample pull request of the stand-in repository.
pub fn sample_pull_request() -> PullRequest {
    pull_request_in(stand_in_github_repository())
}

fn pull_request_in(github: GitHubRepository) -> PullRequest {
    PullRequest {
        number: 42,
        title: "Render pull request bodies as Markdown".into(),
        created_at: iso_ago(3 * 86_400),
        updated_at: iso_ago(2 * 3600),
        head: PullRequestRef {
            ref_name: "feature/markdown".into(),
            sha: "4f1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c".into(),
            repository: Some(github.clone()),
        },
        base: PullRequestRef {
            ref_name: "main".into(),
            sha: "0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b".into(),
            repository: Some(github),
        },
        author: "wasi-master".into(),
        draft: false,
        body: "Renders **Markdown** natively.".into(),
        assignees: Vec::new(),
        requested_reviewers: Vec::new(),
    }
}

/// A body that exercises every block `corvene_ui::markdown` lays out.
pub const REVIEW_BODY: &str = "Nice work on the **renderer**! A few things before this lands:\n\n\
### Blocking\n\n\
1. The `Walk` should flush *inline* text before a nested list.\n\
2. Links like [the CommonMark spec](https://spec.commonmark.org) must open in the browser.\n\n\
- Bare URLs: https://github.com/wasi-master/corvene\n\
- ~~Tables~~ degrade to plain text\n\
\x20 - nested item\n\n\
> Quote from the style guide: keep the vertical rhythm at 16 px.\n\n\
```rust\nfn render(blocks: &[Block]) -> Div {\n    todo!()\n}\n```\n\n\
---\n\n\
Thanks!";

pub fn review(state: ApiPullRequestReviewState) -> ApiPullRequestReview {
    ApiPullRequestReview {
        id: 80,
        user: octocat(),
        body: if state == ApiPullRequestReviewState::Approved {
            String::new()
        } else {
            REVIEW_BODY.into()
        },
        html_url: "https://github.com/wasi-master/corvene-demo/pull/42#pullrequestreview-80".into(),
        submitted_at: iso_ago(2 * 3600),
        state,
    }
}

pub fn comment() -> ApiIssueComment {
    ApiIssueComment {
        id: 1,
        body: "Could you add a screenshot of the **dark theme** too? :eyes:\n\n\
               See `.docs/ghd-theme-tokens.md` for the colours."
            .into(),
        html_url: "https://github.com/wasi-master/corvene-demo/pull/42#issuecomment-1".into(),
        user: octocat(),
        created_at: iso_ago(25 * 60),
    }
}

fn step(number: u64, name: &str, conclusion: CheckConclusion, secs: u64) -> JobStep {
    JobStep {
        name: name.into(),
        number,
        status: CheckStatus::Completed,
        conclusion: Some(conclusion),
        started_at: Some(iso_ago(600 + secs)),
        completed_at: Some(iso_ago(600)),
    }
}

pub fn failed_checks() -> Vec<RefCheck> {
    let workflow = WorkflowRun {
        id: 7,
        workflow_id: 3,
        name: "CI".into(),
        event: "pull_request".into(),
        check_suite_id: Some(11),
        created_at: iso_ago(900),
    };
    let check = |id: u64,
                 name: &str,
                 description: &str,
                 conclusion: CheckConclusion,
                 steps: Option<Vec<JobStep>>| RefCheck {
        id,
        name: name.into(),
        description: description.into(),
        status: CheckStatus::Completed,
        conclusion: Some(conclusion),
        app_name: "GitHub Actions".into(),
        html_url: Some(format!(
            "https://github.com/wasi-master/corvene-demo/actions/runs/7/job/{id}"
        )),
        check_suite_id: Some(11),
        actions_workflow: steps.as_ref().map(|_| workflow.clone()),
        job_steps: steps,
    };
    vec![
        check(
            101,
            "test (macos-15)",
            "Process completed with exit code 101.",
            CheckConclusion::Failure,
            Some(vec![
                step(1, "Set up job", CheckConclusion::Success, 2),
                step(2, "Checkout", CheckConclusion::Success, 3),
                step(3, "cargo clippy", CheckConclusion::Success, 94),
                step(4, "cargo test --workspace", CheckConclusion::Failure, 211),
                step(5, "Post Checkout", CheckConclusion::Skipped, 0),
            ]),
        ),
        check(
            102,
            "fmt",
            "Successful in 21s",
            CheckConclusion::Success,
            Some(vec![
                step(1, "Set up job", CheckConclusion::Success, 1),
                step(2, "cargo fmt --check", CheckConclusion::Success, 20),
            ]),
        ),
        check(
            103,
            "Vercel",
            "Deployment has failed",
            CheckConclusion::Failure,
            None,
        ),
    ]
}

/// A sample Actions job log for the failed `test (macos-15)` job of
/// [`failed_checks`] (`CORVENE_POPUP=job-log`, flag `347-actions-job-logs`).
pub fn job_log() -> String {
    let stamp = |s: u64| format!("2024-05-01T10:{:02}:{:02}.0000000Z", s / 60, s % 60);
    let lines: Vec<(u64, &str)> = vec![
        (0, "Current runner version: '2.317.0'"),
        (0, "##[group]Operating System"),
        (0, "macOS"),
        (0, "15.0"),
        (0, "##[endgroup]"),
        (1, "##[group]Run actions/checkout@v4"),
        (1, "with:"),
        (1, "  fetch-depth: 0"),
        (1, "##[endgroup]"),
        (2, "Syncing repository: wasi-master/corvene-demo"),
        (3, "##[group]Run cargo clippy --all-targets -- -D warnings"),
        (
            3,
            "\u{1b}[36;1mcargo clippy --all-targets -- -D warnings\u{1b}[0m",
        ),
        (3, "shell: /bin/bash -e {0}"),
        (3, "##[endgroup]"),
        (
            10,
            "\u{1b}[1m\u{1b}[32m    Checking\u{1b}[0m corvene-core v0.1.0",
        ),
        (
            97,
            "\u{1b}[1m\u{1b}[32m    Finished\u{1b}[0m `dev` profile [unoptimized + debuginfo] target(s) in 1m 34s",
        ),
        (98, "##[group]Run cargo test --workspace"),
        (98, "\u{1b}[36;1mcargo test --workspace\u{1b}[0m"),
        (98, "shell: /bin/bash -e {0}"),
        (98, "##[endgroup]"),
        (
            140,
            "\u{1b}[1m\u{1b}[32m     Running\u{1b}[0m unittests src/lib.rs (target/debug/deps/corvene_core-8f2a1c)",
        ),
        (141, "running 412 tests"),
        (
            300,
            "test job_log::tests::searches_case_insensitively ... \u{1b}[32mok\u{1b}[0m",
        ),
        (
            301,
            "test remote::tests::fetch_skips_unchanged ... \u{1b}[31mFAILED\u{1b}[0m",
        ),
        (302, "failures:"),
        (302, "---- remote::tests::fetch_skips_unchanged stdout ----"),
        (
            302,
            "thread 'remote::tests::fetch_skips_unchanged' panicked at crates/corvene-core/src/remote.rs:2710:9:",
        ),
        (302, "assertion `left == right` failed"),
        (302, "  left: 2"),
        (302, " right: 1"),
        (
            302,
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
        ),
        (
            303,
            "test result: \u{1b}[31mFAILED\u{1b}[0m. 411 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 162.03s",
        ),
        (
            309,
            "\u{1b}[1m\u{1b}[31merror\u{1b}[0m: test failed, to rerun pass `-p corvene-core --lib`",
        ),
        (309, "##[error]Process completed with exit code 101."),
        (310, "Post job cleanup."),
        (310, "##[group]Run actions/checkout@v4"),
        (310, "##[endgroup]"),
        (310, "Cleaning up orphan processes"),
    ];
    lines
        .into_iter()
        .map(|(s, text)| format!("{} {text}\n", stamp(s)))
        .collect()
}

/// GHD `TestNotificationType`: the sample notification of each kind for
/// `repo` (the Test Notifications dialog).
pub fn notification(
    kind: crate::notifications::TestNotificationType,
    repo: u64,
    cx: &dyn Host,
) -> crate::notifications::PullRequestNotification {
    use crate::notifications::{NotificationKind, TestNotificationType};
    let github = github_repository(repo, cx);
    let pull_request = pull_request(repo, cx);
    let kind = match kind {
        TestNotificationType::PullRequestReview => NotificationKind::PullRequestReview {
            review: review(ApiPullRequestReviewState::ChangesRequested),
        },
        TestNotificationType::PullRequestComment => {
            NotificationKind::PullRequestComment { comment: comment() }
        }
        TestNotificationType::ChecksFailed => NotificationKind::ChecksFailed {
            commit_sha: pull_request.head.sha.clone(),
            checks: failed_checks(),
        },
    };
    crate::notifications::PullRequestNotification {
        repo,
        owner: github.owner,
        name: github.name,
        pull_request,
        kind,
    }
}
