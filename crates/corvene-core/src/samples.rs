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

/// `348-pull-request-review`: sample review threads on the sample pull
/// request: one open on a line, one resolved, one outdated, one pending.
pub fn review_threads() -> Vec<corvene_github::review::ReviewThread> {
    use corvene_github::review::{DiffSide, ReviewActor, ReviewComment, ReviewThread};
    let actor = |login: &str| {
        Some(ReviewActor {
            login: login.into(),
            avatar_url: None,
        })
    };
    let comment = |id: &str, login: &str, body: &str, hours: u64, pending: bool| ReviewComment {
        id: id.into(),
        database_id: None,
        author: actor(login),
        body: body.into(),
        created_at: iso_ago(hours * 3600),
        updated_at: iso_ago(hours * 3600),
        url: String::new(),
        pending,
        outdated: false,
        viewer_can_update: pending,
        viewer_can_delete: pending,
        viewer_did_author: pending,
        original_commit: Some("4f1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c".into()),
        commit: Some("4f1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c".into()),
        diff_hunk: "@@ -1,3 +1,4 @@\n fn main() {\n+    let name = \"world\";".into(),
        reply_to: None,
        review_id: Some("R_sample".into()),
        review_state: Some(if pending { "PENDING" } else { "COMMENTED" }.into()),
    };
    vec![
        ReviewThread {
            id: "T_1".into(),
            path: "src/main.rs".into(),
            line: Some(2),
            diff_side: Some(DiffSide::Right),
            original_line: Some(2),
            viewer_can_resolve: true,
            viewer_can_reply: true,
            comments: vec![
                comment(
                    "C_1",
                    "octocat",
                    "Should this read the name from the arguments?",
                    30,
                    false,
                ),
                comment(
                    "C_2",
                    "wasi-master",
                    "It does on the next line; this is the default.",
                    28,
                    false,
                ),
            ],
            ..Default::default()
        },
        ReviewThread {
            id: "T_2".into(),
            path: "README.md".into(),
            line: Some(7),
            start_line: Some(5),
            diff_side: Some(DiffSide::Right),
            start_diff_side: Some(DiffSide::Right),
            original_line: Some(7),
            is_resolved: true,
            resolved_by: Some("wasi-master".into()),
            viewer_can_unresolve: true,
            viewer_can_reply: true,
            comments: vec![comment(
                "C_3",
                "hubot",
                "Typo: *greating* → greeting.",
                50,
                false,
            )],
            ..Default::default()
        },
        ReviewThread {
            id: "T_3".into(),
            path: "src/main.rs".into(),
            line: None,
            diff_side: Some(DiffSide::Right),
            original_line: Some(9),
            is_outdated: true,
            viewer_can_resolve: true,
            viewer_can_reply: true,
            comments: vec![comment(
                "C_4",
                "octocat",
                "This branch was unreachable.",
                72,
                false,
            )],
            ..Default::default()
        },
        ReviewThread {
            id: "T_4".into(),
            path: "src/main.rs".into(),
            line: Some(3),
            diff_side: Some(DiffSide::Left),
            original_line: Some(3),
            viewer_can_resolve: true,
            viewer_can_reply: true,
            comments: vec![comment(
                "C_5",
                "wasi-master",
                "Keep the old greeting as a fallback?",
                1,
                true,
            )],
            ..Default::default()
        },
    ]
}

/// `348-pull-request-review`: the sample pull request's overview.
pub fn pull_request_overview(pr: &PullRequest) -> corvene_github::review::PullRequestOverview {
    use corvene_github::review::{
        OverviewLabel, OverviewReview, PendingReview, PullRequestOverview, ReviewActor,
        ReviewRequest, RollupState, TimelineItem,
    };
    let actor = |login: &str| {
        Some(ReviewActor {
            login: login.into(),
            avatar_url: None,
        })
    };
    PullRequestOverview {
        id: "PR_sample".into(),
        number: pr.number,
        title: pr.title.clone(),
        body: REVIEW_BODY.into(),
        state: "OPEN".into(),
        is_draft: pr.draft,
        author: actor(&pr.author),
        created_at: pr.created_at.clone(),
        merged_at: None,
        closed_at: None,
        url: pr.html_url().unwrap_or_default(),
        base_ref_name: pr.base.ref_name.clone(),
        head_ref_name: pr.head.ref_name.clone(),
        base_ref_oid: pr.base.sha.clone(),
        head_ref_oid: pr.head.sha.clone(),
        head_repository: pr.head.repository.as_ref().map(|r| r.full_name()),
        is_cross_repository: false,
        mergeable: "MERGEABLE".into(),
        review_decision: Some("CHANGES_REQUESTED".into()),
        additions: 48,
        deletions: 7,
        changed_files: 3,
        commit_count: 2,
        checks: Some(RollupState::Success),
        labels: vec![
            OverviewLabel {
                name: "enhancement".into(),
                color: "a2eeef".into(),
            },
            OverviewLabel {
                name: "markdown".into(),
                color: "0075ca".into(),
            },
        ],
        milestone: Some("v0.2".into()),
        assignees: vec![pr.author.clone()],
        review_requests: vec![ReviewRequest {
            name: "hubot".into(),
            avatar_url: None,
            team: false,
        }],
        reviews: vec![OverviewReview {
            id: "R_1".into(),
            author: actor("octocat"),
            state: "CHANGES_REQUESTED".into(),
            body: "A few things before this lands.".into(),
            submitted_at: Some(iso_ago(30 * 3600)),
            url: String::new(),
            comment_count: 2,
        }],
        pending_review: Some(PendingReview {
            id: "R_sample".into(),
            body: String::new(),
            comment_count: 1,
        }),
        viewer_login: "wasi-master".into(),
        viewer_can_update: true,
        timeline: vec![
            TimelineItem::Commit {
                oid: pr.base.sha.clone(),
                headline: "Parse Markdown bodies".into(),
                author: pr.author.clone(),
                date: iso_ago(3 * 86_400),
            },
            TimelineItem::ReviewRequested {
                actor: pr.author.clone(),
                reviewer: "octocat".into(),
                date: iso_ago(3 * 86_400 - 600),
            },
            TimelineItem::Comment {
                id: "IC_1".into(),
                author: actor("hubot"),
                body: "CI is green on this one.".into(),
                created_at: iso_ago(2 * 86_400),
                url: String::new(),
            },
            TimelineItem::Review(OverviewReview {
                id: "R_1".into(),
                author: actor("octocat"),
                state: "CHANGES_REQUESTED".into(),
                body: "A few things before this lands.".into(),
                submitted_at: Some(iso_ago(30 * 3600)),
                url: String::new(),
                comment_count: 2,
            }),
            TimelineItem::Commit {
                oid: pr.head.sha.clone(),
                headline: "Render pull request bodies as Markdown".into(),
                author: pr.author.clone(),
                date: iso_ago(2 * 3600),
            },
        ],
        timeline_more: 0,
    }
}
