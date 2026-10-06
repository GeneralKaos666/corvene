//! Pull request review over GraphQL (Corvene `348-pull-request-review`):
//! the review threads with their comments, the pull request overview
//! (description, reviewers, checks summary, timeline) and the writes a
//! review needs (reply, resolve, pending review comments, submit).
//!
//! GitHub Desktop has none of it: its `lib/api.ts` only fetches one review
//! or one comment for a notification over REST. GraphQL is used here
//! because the REST review-comment list has no thread structure, no
//! resolved state and no `line` for the current head, and because every
//! write (a comment into a pending review, submitting it) is one mutation.
//!
//! The operations carry names (`query ReviewThreads`, `mutation
//! AddReviewThread`, …): the parity harness's stub API answers by them.

use serde::{Deserialize, Serialize};

use crate::api::Client;
use crate::error::Result;

/// Threads per page (GitHub's maximum) and the most pages read.
const THREADS_PAGE: u32 = 100;
const THREADS_MAX_PAGES: usize = 20;
/// Timeline items read (the newest).
const TIMELINE_ITEMS: u32 = 80;

/// `author { login avatarUrl }`: a user, a bot, or nothing for a deleted
/// account ("ghost").
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewActor {
    pub login: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// `DiffSide`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiffSide {
    Left,
    Right,
}

impl DiffSide {
    pub fn graphql(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
}

/// `PullRequestReviewEvent`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewEvent {
    Comment,
    Approve,
    RequestChanges,
}

impl ReviewEvent {
    pub fn graphql(self) -> &'static str {
        match self {
            Self::Comment => "COMMENT",
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
        }
    }
}

/// One comment of a review thread (`PullRequestReviewComment`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewComment {
    /// The node id (every mutation takes it).
    pub id: String,
    pub database_id: Option<u64>,
    pub author: Option<ReviewActor>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    /// `state == PENDING`: part of the viewer's unsubmitted review.
    pub pending: bool,
    pub outdated: bool,
    pub viewer_can_update: bool,
    pub viewer_can_delete: bool,
    pub viewer_did_author: bool,
    /// The head commit the comment was made on.
    pub original_commit: Option<String>,
    /// The head commit the comment's `line` refers to now.
    pub commit: Option<String>,
    /// The diff lines around the comment as GitHub keeps them.
    pub diff_hunk: String,
    pub reply_to: Option<String>,
    /// The review the comment belongs to and its state.
    pub review_id: Option<String>,
    pub review_state: Option<String>,
}

/// `PullRequestReviewThread`
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewThread {
    pub id: String,
    pub path: String,
    /// The line in the current head's diff (`None` when outdated or for a
    /// file-level thread).
    pub line: Option<u32>,
    pub start_line: Option<u32>,
    /// The line in the diff of the commit the thread was started on.
    pub original_line: Option<u32>,
    pub original_start_line: Option<u32>,
    pub diff_side: Option<DiffSide>,
    pub start_diff_side: Option<DiffSide>,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub resolved_by: Option<String>,
    pub viewer_can_resolve: bool,
    pub viewer_can_unresolve: bool,
    pub viewer_can_reply: bool,
    pub comments: Vec<ReviewComment>,
}

impl ReviewThread {
    /// A thread on the file rather than a line: GitHub gives it no line
    /// at all and never marks it outdated.
    pub fn is_file_level(&self) -> bool {
        self.line.is_none() && self.original_line.is_none() && !self.is_outdated
    }

    /// The first comment's commit, the head the thread was started on.
    pub fn original_commit(&self) -> Option<&str> {
        self.comments
            .first()
            .and_then(|c| c.original_commit.as_deref())
    }

    /// The newest comment's `commit`: the head GitHub mapped `line` to.
    pub fn current_commit(&self) -> Option<&str> {
        self.comments.iter().rev().find_map(|c| c.commit.as_deref())
    }

    /// Every comment is part of the viewer's pending review.
    pub fn is_pending(&self) -> bool {
        !self.comments.is_empty() && self.comments.iter().all(|c| c.pending)
    }
}

/// The pull request's threads plus the ids the writes need.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewThreads {
    /// The pull request's node id.
    pub pull_request_id: String,
    pub threads: Vec<ReviewThread>,
}

/// A requested reviewer: a user or a team.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub name: String,
    pub avatar_url: Option<String>,
    pub team: bool,
}

/// A submitted review, as `latestOpinionatedReviews` lists them (the
/// newest approve / request-changes per reviewer) or a timeline shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewReview {
    pub id: String,
    pub author: Option<ReviewActor>,
    /// `APPROVED` | `CHANGES_REQUESTED` | `COMMENTED` | `DISMISSED` | `PENDING`
    pub state: String,
    pub body: String,
    pub submitted_at: Option<String>,
    pub url: String,
    pub comment_count: u64,
}

/// The viewer's unsubmitted review.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingReview {
    pub id: String,
    pub body: String,
    pub comment_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewLabel {
    pub name: String,
    pub color: String,
}

/// `statusCheckRollup.state`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RollupState {
    Expected,
    Error,
    Failure,
    Pending,
    Success,
}

/// One timeline entry, newest last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineItem {
    Commit {
        oid: String,
        headline: String,
        author: String,
        date: String,
    },
    Comment {
        id: String,
        author: Option<ReviewActor>,
        body: String,
        created_at: String,
        url: String,
    },
    Review(OverviewReview),
    ReviewRequested {
        actor: String,
        reviewer: String,
        date: String,
    },
    ReviewDismissed {
        actor: String,
        date: String,
    },
    Labeled {
        actor: String,
        label: OverviewLabel,
        added: bool,
        date: String,
    },
    Renamed {
        actor: String,
        from: String,
        to: String,
        date: String,
    },
    ForcePushed {
        actor: String,
        before: String,
        after: String,
        date: String,
    },
    ReadyForReview {
        actor: String,
        date: String,
    },
    ConvertedToDraft {
        actor: String,
        date: String,
    },
    Merged {
        actor: String,
        into: String,
        date: String,
    },
    Closed {
        actor: String,
        date: String,
    },
    Reopened {
        actor: String,
        date: String,
    },
    HeadRefDeleted {
        actor: String,
        date: String,
    },
}

/// `PullRequest` as the overview pane shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestOverview {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub body: String,
    /// `OPEN` | `CLOSED` | `MERGED`
    pub state: String,
    pub is_draft: bool,
    pub author: Option<ReviewActor>,
    pub created_at: String,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub url: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub base_ref_oid: String,
    pub head_ref_oid: String,
    /// The head repository's `owner/name` (`None` when deleted).
    pub head_repository: Option<String>,
    pub is_cross_repository: bool,
    /// `MERGEABLE` | `CONFLICTING` | `UNKNOWN`
    pub mergeable: String,
    /// `APPROVED` | `CHANGES_REQUESTED` | `REVIEW_REQUIRED` | `None`
    pub review_decision: Option<String>,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub commit_count: u64,
    pub checks: Option<RollupState>,
    pub labels: Vec<OverviewLabel>,
    pub milestone: Option<String>,
    pub assignees: Vec<String>,
    pub review_requests: Vec<ReviewRequest>,
    pub reviews: Vec<OverviewReview>,
    pub pending_review: Option<PendingReview>,
    pub viewer_login: String,
    pub viewer_can_update: bool,
    pub timeline: Vec<TimelineItem>,
    /// Timeline entries the query did not read (the oldest).
    pub timeline_more: u64,
}

/// A thread to start: a line or a range of lines on one side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewThread {
    pub path: String,
    pub line: u32,
    pub side: DiffSide,
    /// A range: its first line (and the side it is on).
    pub start_line: Option<u32>,
    pub start_side: Option<DiffSide>,
    pub body: String,
}

impl NewThread {
    fn input(&self) -> serde_json::Value {
        let mut input = serde_json::json!({
            "path": self.path,
            "line": self.line,
            "side": self.side.graphql(),
            "body": self.body,
        });
        if let Some(start) = self.start_line.filter(|s| *s != self.line) {
            input["startLine"] = serde_json::json!(start);
            input["startSide"] = serde_json::json!(self.start_side.unwrap_or(self.side).graphql());
        }
        input
    }
}

// ---- wire types ----------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireActor {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

impl From<WireActor> for ReviewActor {
    fn from(a: WireActor) -> Self {
        Self {
            login: a.login,
            avatar_url: a.avatar_url,
        }
    }
}

#[derive(Deserialize)]
struct WireOid {
    oid: String,
}

#[derive(Deserialize)]
struct WireId {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireReviewRef {
    id: String,
    state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireComment {
    id: String,
    #[serde(default)]
    database_id: Option<u64>,
    #[serde(default)]
    author: Option<WireActor>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    outdated: bool,
    #[serde(default)]
    viewer_can_update: bool,
    #[serde(default)]
    viewer_can_delete: bool,
    #[serde(default)]
    viewer_did_author: bool,
    #[serde(default)]
    original_commit: Option<WireOid>,
    #[serde(default)]
    commit: Option<WireOid>,
    #[serde(default)]
    diff_hunk: String,
    #[serde(default)]
    reply_to: Option<WireId>,
    #[serde(default)]
    pull_request_review: Option<WireReviewRef>,
}

impl From<WireComment> for ReviewComment {
    fn from(c: WireComment) -> Self {
        Self {
            id: c.id,
            database_id: c.database_id,
            author: c.author.map(Into::into),
            body: c.body,
            created_at: c.created_at,
            updated_at: c.updated_at,
            url: c.url,
            pending: c.state == "PENDING",
            outdated: c.outdated,
            viewer_can_update: c.viewer_can_update,
            viewer_can_delete: c.viewer_can_delete,
            viewer_did_author: c.viewer_did_author,
            original_commit: c.original_commit.map(|o| o.oid),
            commit: c.commit.map(|o| o.oid),
            diff_hunk: c.diff_hunk,
            reply_to: c.reply_to.map(|r| r.id),
            review_id: c.pull_request_review.as_ref().map(|r| r.id.clone()),
            review_state: c.pull_request_review.map(|r| r.state),
        }
    }
}

#[derive(Deserialize)]
struct Nodes<T> {
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<T>>,
}

impl<T> Nodes<T> {
    fn flatten(self) -> Vec<T> {
        self.nodes.into_iter().flatten().collect()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireThread {
    id: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    start_line: Option<u32>,
    #[serde(default)]
    original_line: Option<u32>,
    #[serde(default)]
    original_start_line: Option<u32>,
    #[serde(default)]
    diff_side: Option<DiffSide>,
    #[serde(default)]
    start_diff_side: Option<DiffSide>,
    #[serde(default)]
    is_resolved: bool,
    #[serde(default)]
    is_outdated: bool,
    #[serde(default)]
    resolved_by: Option<WireActor>,
    #[serde(default)]
    viewer_can_resolve: bool,
    #[serde(default)]
    viewer_can_unresolve: bool,
    #[serde(default)]
    viewer_can_reply: bool,
    comments: Nodes<WireComment>,
}

impl From<WireThread> for ReviewThread {
    fn from(t: WireThread) -> Self {
        Self {
            id: t.id,
            path: t.path,
            line: t.line,
            start_line: t.start_line,
            original_line: t.original_line,
            original_start_line: t.original_start_line,
            diff_side: t.diff_side,
            start_diff_side: t.start_diff_side,
            is_resolved: t.is_resolved,
            is_outdated: t.is_outdated,
            resolved_by: t.resolved_by.map(|a| a.login),
            viewer_can_resolve: t.viewer_can_resolve,
            viewer_can_unresolve: t.viewer_can_unresolve,
            viewer_can_reply: t.viewer_can_reply,
            comments: t.comments.flatten().into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireThreadPage {
    page_info: PageInfo,
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<WireThread>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireThreadsPullRequest {
    id: String,
    review_threads: WireThreadPage,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRepository<T> {
    pull_request: Option<T>,
}

#[derive(Deserialize)]
struct WireRepositoryData<T> {
    repository: Option<WireRepository<T>>,
}

const COMMENT_FIELDS: &str = "id databaseId body createdAt updatedAt url state outdated \
     viewerCanUpdate viewerCanDelete viewerDidAuthor diffHunk \
     author { login avatarUrl } originalCommit { oid } commit { oid } \
     replyTo { id } pullRequestReview { id state }";

fn threads_query() -> String {
    format!(
        "query ReviewThreads($owner: String!, $name: String!, $number: Int!, $after: String) {{ \
           repository(owner: $owner, name: $name) {{ pullRequest(number: $number) {{ id \
             reviewThreads(first: {THREADS_PAGE}, after: $after) {{ \
               pageInfo {{ hasNextPage endCursor }} \
               nodes {{ id path line startLine originalLine originalStartLine diffSide \
                 startDiffSide isResolved isOutdated resolvedBy {{ login }} \
                 viewerCanResolve viewerCanUnresolve viewerCanReply \
                 comments(first: 100) {{ nodes {{ {COMMENT_FIELDS} }} }} }} }} }} }} }}"
    )
}

// ---- overview wire types ---------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireReview {
    id: String,
    #[serde(default)]
    author: Option<WireActor>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    submitted_at: Option<String>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    comments: Option<TotalCount>,
}

impl From<WireReview> for OverviewReview {
    fn from(r: WireReview) -> Self {
        Self {
            id: r.id,
            author: r.author.map(Into::into),
            state: r.state,
            body: r.body,
            submitted_at: r.submitted_at,
            url: r.url,
            comment_count: r.comments.map(|c| c.total_count).unwrap_or(0),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TotalCount {
    #[serde(default)]
    total_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireLabel {
    name: String,
    #[serde(default)]
    color: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireLogin {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTitle {
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNameWithOwner {
    name_with_owner: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRequestedReviewer {
    #[serde(rename = "__typename", default)]
    typename: String,
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireReviewRequest {
    #[serde(default)]
    requested_reviewer: Option<WireRequestedReviewer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRollup {
    state: RollupState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireHeadCommit {
    #[serde(default)]
    status_check_rollup: Option<WireRollup>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCommitNode {
    commit: WireHeadCommit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCommits {
    #[serde(default)]
    total_count: u64,
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<WireCommitNode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTimelineCommitAuthor {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    user: Option<WireLogin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTimelineCommit {
    oid: String,
    #[serde(default)]
    message_headline: String,
    #[serde(default)]
    author: Option<WireTimelineCommitAuthor>,
    #[serde(default)]
    committed_date: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAbbrev {
    #[serde(default)]
    abbreviated_oid: String,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
#[serde(rename_all = "camelCase")]
enum WireTimelineItem {
    #[serde(rename = "PullRequestCommit")]
    Commit { commit: WireTimelineCommit },
    #[serde(rename = "IssueComment", rename_all = "camelCase")]
    Comment {
        id: String,
        #[serde(default)]
        author: Option<WireActor>,
        #[serde(default)]
        body: String,
        #[serde(default)]
        created_at: String,
        #[serde(default)]
        url: String,
    },
    #[serde(rename = "PullRequestReview")]
    Review(WireReview),
    #[serde(rename = "ReviewRequestedEvent", rename_all = "camelCase")]
    ReviewRequested {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        requested_reviewer: Option<WireRequestedReviewer>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "ReviewDismissedEvent", rename_all = "camelCase")]
    ReviewDismissed {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "LabeledEvent", rename_all = "camelCase")]
    Labeled {
        #[serde(default)]
        actor: Option<WireLogin>,
        label: WireLabel,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "UnlabeledEvent", rename_all = "camelCase")]
    Unlabeled {
        #[serde(default)]
        actor: Option<WireLogin>,
        label: WireLabel,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "RenamedTitleEvent", rename_all = "camelCase")]
    Renamed {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        previous_title: String,
        #[serde(default)]
        current_title: String,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "HeadRefForcePushedEvent", rename_all = "camelCase")]
    ForcePushed {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        before_commit: Option<WireAbbrev>,
        #[serde(default)]
        after_commit: Option<WireAbbrev>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "ReadyForReviewEvent", rename_all = "camelCase")]
    ReadyForReview {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "ConvertToDraftEvent", rename_all = "camelCase")]
    ConvertToDraft {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "MergedEvent", rename_all = "camelCase")]
    Merged {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        merge_ref_name: String,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "ClosedEvent", rename_all = "camelCase")]
    Closed {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "ReopenedEvent", rename_all = "camelCase")]
    Reopened {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(rename = "HeadRefDeletedEvent", rename_all = "camelCase")]
    HeadRefDeleted {
        #[serde(default)]
        actor: Option<WireLogin>,
        #[serde(default)]
        created_at: String,
    },
    #[serde(other)]
    Other,
}

fn login(actor: Option<WireLogin>) -> String {
    actor
        .map(|a| a.login)
        .unwrap_or_else(|| "ghost".to_string())
}

fn reviewer(r: Option<WireRequestedReviewer>) -> Option<ReviewRequest> {
    let r = r?;
    let team = r.typename == "Team";
    let name = if team { r.name? } else { r.login.or(r.name)? };
    Some(ReviewRequest {
        name,
        avatar_url: r.avatar_url,
        team,
    })
}

impl WireTimelineItem {
    fn convert(self) -> Option<TimelineItem> {
        Some(match self {
            Self::Commit { commit } => TimelineItem::Commit {
                oid: commit.oid,
                headline: commit.message_headline,
                author: commit
                    .author
                    .and_then(|a| a.user.map(|u| u.login).or(a.name))
                    .unwrap_or_default(),
                date: commit.committed_date,
            },
            Self::Comment {
                id,
                author,
                body,
                created_at,
                url,
            } => TimelineItem::Comment {
                id,
                author: author.map(Into::into),
                body,
                created_at,
                url,
            },
            Self::Review(r) => {
                // a pending review is the viewer's own draft, listed apart
                if r.state == "PENDING" {
                    return None;
                }
                TimelineItem::Review(r.into())
            }
            Self::ReviewRequested {
                actor,
                requested_reviewer,
                created_at,
            } => TimelineItem::ReviewRequested {
                actor: login(actor),
                reviewer: reviewer(requested_reviewer)
                    .map(|r| r.name)
                    .unwrap_or_default(),
                date: created_at,
            },
            Self::ReviewDismissed { actor, created_at } => TimelineItem::ReviewDismissed {
                actor: login(actor),
                date: created_at,
            },
            Self::Labeled {
                actor,
                label,
                created_at,
            } => TimelineItem::Labeled {
                actor: login(actor),
                label: OverviewLabel {
                    name: label.name,
                    color: label.color,
                },
                added: true,
                date: created_at,
            },
            Self::Unlabeled {
                actor,
                label,
                created_at,
            } => TimelineItem::Labeled {
                actor: login(actor),
                label: OverviewLabel {
                    name: label.name,
                    color: label.color,
                },
                added: false,
                date: created_at,
            },
            Self::Renamed {
                actor,
                previous_title,
                current_title,
                created_at,
            } => TimelineItem::Renamed {
                actor: login(actor),
                from: previous_title,
                to: current_title,
                date: created_at,
            },
            Self::ForcePushed {
                actor,
                before_commit,
                after_commit,
                created_at,
            } => TimelineItem::ForcePushed {
                actor: login(actor),
                before: before_commit.map(|c| c.abbreviated_oid).unwrap_or_default(),
                after: after_commit.map(|c| c.abbreviated_oid).unwrap_or_default(),
                date: created_at,
            },
            Self::ReadyForReview { actor, created_at } => TimelineItem::ReadyForReview {
                actor: login(actor),
                date: created_at,
            },
            Self::ConvertToDraft { actor, created_at } => TimelineItem::ConvertedToDraft {
                actor: login(actor),
                date: created_at,
            },
            Self::Merged {
                actor,
                merge_ref_name,
                created_at,
            } => TimelineItem::Merged {
                actor: login(actor),
                into: merge_ref_name,
                date: created_at,
            },
            Self::Closed { actor, created_at } => TimelineItem::Closed {
                actor: login(actor),
                date: created_at,
            },
            Self::Reopened { actor, created_at } => TimelineItem::Reopened {
                actor: login(actor),
                date: created_at,
            },
            Self::HeadRefDeleted { actor, created_at } => TimelineItem::HeadRefDeleted {
                actor: login(actor),
                date: created_at,
            },
            Self::Other => return None,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTimeline {
    #[serde(default)]
    total_count: u64,
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<WireTimelineItem>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePendingReview {
    id: String,
    #[serde(default)]
    author: Option<WireLogin>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    comments: Option<TotalCount>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireOverview {
    id: String,
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    author: Option<WireActor>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    closed_at: Option<String>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    base_ref_name: String,
    #[serde(default)]
    head_ref_name: String,
    #[serde(default)]
    base_ref_oid: String,
    #[serde(default)]
    head_ref_oid: String,
    #[serde(default)]
    head_repository: Option<WireNameWithOwner>,
    #[serde(default)]
    is_cross_repository: bool,
    #[serde(default)]
    mergeable: String,
    #[serde(default)]
    review_decision: Option<String>,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default)]
    changed_files: u64,
    #[serde(default)]
    viewer_can_update: bool,
    commits: Option<WireCommits>,
    labels: Option<Nodes<WireLabel>>,
    #[serde(default)]
    milestone: Option<WireTitle>,
    assignees: Option<Nodes<WireLogin>>,
    review_requests: Option<Nodes<WireReviewRequest>>,
    latest_opinionated_reviews: Option<Nodes<WireReview>>,
    pending_reviews: Option<Nodes<WirePendingReview>>,
    timeline_items: Option<WireTimeline>,
}

#[derive(Deserialize)]
struct WireOverviewData {
    repository: Option<WireRepository<WireOverview>>,
    viewer: WireLogin,
}

fn overview_query() -> String {
    format!(
        "query PullRequestOverview($owner: String!, $name: String!, $number: Int!) {{ \
           viewer {{ login }} \
           repository(owner: $owner, name: $name) {{ pullRequest(number: $number) {{ \
             id number title body state isDraft createdAt mergedAt closedAt url \
             author {{ login avatarUrl }} \
             baseRefName headRefName baseRefOid headRefOid isCrossRepository \
             headRepository {{ nameWithOwner }} \
             mergeable reviewDecision additions deletions changedFiles viewerCanUpdate \
             commits(last: 1) {{ totalCount nodes {{ commit {{ statusCheckRollup {{ state }} }} }} }} \
             labels(first: 30) {{ nodes {{ name color }} }} \
             milestone {{ title }} \
             assignees(first: 20) {{ nodes {{ login }} }} \
             reviewRequests(first: 30) {{ nodes {{ requestedReviewer {{ __typename \
               ... on User {{ login avatarUrl }} ... on Team {{ name }} \
               ... on Bot {{ login avatarUrl }} }} }} }} \
             latestOpinionatedReviews(first: 50) {{ nodes {{ id state body submittedAt url \
               author {{ login avatarUrl }} comments {{ totalCount }} }} }} \
             pendingReviews: reviews(states: [PENDING], first: 5) {{ nodes {{ id body \
               author {{ login }} comments {{ totalCount }} }} }} \
             timelineItems(last: {TIMELINE_ITEMS}, itemTypes: [PULL_REQUEST_COMMIT, \
               ISSUE_COMMENT, PULL_REQUEST_REVIEW, REVIEW_REQUESTED_EVENT, \
               REVIEW_DISMISSED_EVENT, LABELED_EVENT, UNLABELED_EVENT, RENAMED_TITLE_EVENT, \
               HEAD_REF_FORCE_PUSHED_EVENT, READY_FOR_REVIEW_EVENT, CONVERT_TO_DRAFT_EVENT, \
               MERGED_EVENT, CLOSED_EVENT, REOPENED_EVENT, HEAD_REF_DELETED_EVENT]) {{ \
               totalCount nodes {{ __typename \
                 ... on PullRequestCommit {{ commit {{ oid messageHeadline committedDate \
                   author {{ name user {{ login }} }} }} }} \
                 ... on IssueComment {{ id body createdAt url author {{ login avatarUrl }} }} \
                 ... on PullRequestReview {{ id state body submittedAt url \
                   author {{ login avatarUrl }} comments {{ totalCount }} }} \
                 ... on ReviewRequestedEvent {{ createdAt actor {{ login }} \
                   requestedReviewer {{ __typename ... on User {{ login }} \
                   ... on Team {{ name }} ... on Bot {{ login }} }} }} \
                 ... on ReviewDismissedEvent {{ createdAt actor {{ login }} }} \
                 ... on LabeledEvent {{ createdAt actor {{ login }} label {{ name color }} }} \
                 ... on UnlabeledEvent {{ createdAt actor {{ login }} label {{ name color }} }} \
                 ... on RenamedTitleEvent {{ createdAt actor {{ login }} previousTitle currentTitle }} \
                 ... on HeadRefForcePushedEvent {{ createdAt actor {{ login }} \
                   beforeCommit {{ abbreviatedOid }} afterCommit {{ abbreviatedOid }} }} \
                 ... on ReadyForReviewEvent {{ createdAt actor {{ login }} }} \
                 ... on ConvertToDraftEvent {{ createdAt actor {{ login }} }} \
                 ... on MergedEvent {{ createdAt actor {{ login }} mergeRefName }} \
                 ... on ClosedEvent {{ createdAt actor {{ login }} }} \
                 ... on ReopenedEvent {{ createdAt actor {{ login }} }} \
                 ... on HeadRefDeletedEvent {{ createdAt actor {{ login }} }} }} }} }} }} }}"
    )
}

fn convert_overview(o: WireOverview, viewer: String) -> PullRequestOverview {
    let (commit_count, checks) = match o.commits {
        Some(c) => (
            c.total_count,
            c.nodes
                .into_iter()
                .flatten()
                .next()
                .and_then(|n| n.commit.status_check_rollup)
                .map(|r| r.state),
        ),
        None => (0, None),
    };
    let pending_review = o
        .pending_reviews
        .map(Nodes::flatten)
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.author.as_ref().is_none_or(|a| a.login == viewer))
        .map(|r| PendingReview {
            id: r.id,
            body: r.body,
            comment_count: r.comments.map(|c| c.total_count).unwrap_or(0),
        });
    let (timeline, timeline_total) = match o.timeline_items {
        Some(t) => (
            t.nodes
                .into_iter()
                .flatten()
                .filter_map(WireTimelineItem::convert)
                .collect::<Vec<_>>(),
            t.total_count,
        ),
        None => (Vec::new(), 0),
    };
    PullRequestOverview {
        id: o.id,
        number: o.number,
        title: o.title,
        body: o.body,
        state: o.state,
        is_draft: o.is_draft,
        author: o.author.map(Into::into),
        created_at: o.created_at,
        merged_at: o.merged_at,
        closed_at: o.closed_at,
        url: o.url,
        base_ref_name: o.base_ref_name,
        head_ref_name: o.head_ref_name,
        base_ref_oid: o.base_ref_oid,
        head_ref_oid: o.head_ref_oid,
        head_repository: o.head_repository.map(|r| r.name_with_owner),
        is_cross_repository: o.is_cross_repository,
        mergeable: o.mergeable,
        review_decision: o.review_decision,
        additions: o.additions,
        deletions: o.deletions,
        changed_files: o.changed_files,
        commit_count,
        checks,
        labels: o
            .labels
            .map(Nodes::flatten)
            .unwrap_or_default()
            .into_iter()
            .map(|l| OverviewLabel {
                name: l.name,
                color: l.color,
            })
            .collect(),
        milestone: o.milestone.map(|m| m.title),
        assignees: o
            .assignees
            .map(Nodes::flatten)
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.login)
            .collect(),
        review_requests: o
            .review_requests
            .map(Nodes::flatten)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| reviewer(r.requested_reviewer))
            .collect(),
        reviews: o
            .latest_opinionated_reviews
            .map(Nodes::flatten)
            .unwrap_or_default()
            .into_iter()
            .map(Into::into)
            .collect(),
        pending_review,
        viewer_login: viewer,
        viewer_can_update: o.viewer_can_update,
        timeline_more: timeline_total.saturating_sub(u64::from(TIMELINE_ITEMS)),
        timeline,
    }
}

// ---- mutations wire -------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadPayload {
    thread: Option<WireId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPayload {
    pull_request_review: Option<WireReviewRef>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentPayload {
    comment: Option<WireId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedThread {
    id: String,
    #[serde(default)]
    is_resolved: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolvePayload {
    thread: Option<ResolvedThread>,
}

fn missing(what: &str) -> crate::error::GitHubError {
    crate::error::GitHubError::api(200, format!("GraphQL answered without the {what}"))
}

impl Client {
    /// Every review thread of the pull request, oldest first, with the
    /// pull request's node id.
    pub fn review_threads(&self, owner: &str, name: &str, number: u64) -> Result<ReviewThreads> {
        let query = threads_query();
        let mut after: Option<String> = None;
        let mut out = ReviewThreads::default();
        for _ in 0..THREADS_MAX_PAGES {
            let data: WireRepositoryData<WireThreadsPullRequest> = self.post_graphql(
                &query,
                &serde_json::json!({
                    "owner": owner,
                    "name": name,
                    "number": number,
                    "after": after,
                }),
            )?;
            let Some(pr) = data.repository.and_then(|r| r.pull_request) else {
                return Err(missing("pull request"));
            };
            out.pull_request_id = pr.id;
            let page = pr.review_threads;
            out.threads
                .extend(page.nodes.into_iter().flatten().map(ReviewThread::from));
            if !page.page_info.has_next_page {
                break;
            }
            after = page.page_info.end_cursor;
            if after.is_none() {
                break;
            }
        }
        Ok(out)
    }

    /// The pull request as the overview pane shows it.
    pub fn pull_request_overview(
        &self,
        owner: &str,
        name: &str,
        number: u64,
    ) -> Result<PullRequestOverview> {
        let data: WireOverviewData = self.post_graphql(
            &overview_query(),
            &serde_json::json!({ "owner": owner, "name": name, "number": number }),
        )?;
        let Some(pr) = data.repository.and_then(|r| r.pull_request) else {
            return Err(missing("pull request"));
        };
        Ok(convert_overview(pr, data.viewer.login))
    }

    /// `addPullRequestReviewThreadReply`: a reply, posted at once, or into
    /// the pending review `review_id` (then it stays pending until that
    /// review is submitted). The new comment's node id.
    pub fn reply_to_review_thread(
        &self,
        thread_id: &str,
        body: &str,
        review_id: Option<&str>,
    ) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            add_pull_request_review_thread_reply: Option<CommentPayload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation AddThreadReply($thread: ID!, $body: String!, $review: ID) { \
             addPullRequestReviewThreadReply(input: {pullRequestReviewThreadId: $thread, \
             body: $body, pullRequestReviewId: $review}) { comment { id } } }",
            &serde_json::json!({ "thread": thread_id, "body": body, "review": review_id }),
        )?;
        data.add_pull_request_review_thread_reply
            .and_then(|p| p.comment)
            .map(|c| c.id)
            .ok_or_else(|| missing("comment"))
    }

    /// `resolveReviewThread` / `unresolveReviewThread`. The thread's new
    /// resolved state.
    pub fn set_review_thread_resolved(&self, thread_id: &str, resolved: bool) -> Result<bool> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            resolve_review_thread: Option<ResolvePayload>,
            unresolve_review_thread: Option<ResolvePayload>,
        }
        let query = if resolved {
            "mutation ResolveThread($thread: ID!) { resolveReviewThread(input: {threadId: $thread}) \
             { thread { id isResolved } } }"
        } else {
            "mutation UnresolveThread($thread: ID!) { unresolveReviewThread(input: {threadId: $thread}) \
             { thread { id isResolved } } }"
        };
        let data: Data =
            self.post_graphql_strict(query, &serde_json::json!({ "thread": thread_id }))?;
        data.resolve_review_thread
            .or(data.unresolve_review_thread)
            .and_then(|p| p.thread)
            .filter(|t| t.id == thread_id)
            .map(|t| t.is_resolved)
            .ok_or_else(|| missing("thread"))
    }

    /// `addPullRequestReview` without an event: the viewer's pending
    /// review on `commit`, to collect comments in. Its node id.
    pub fn start_pending_review(&self, pull_request_id: &str, commit: &str) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            add_pull_request_review: Option<ReviewPayload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation StartReview($pr: ID!, $commit: GitObjectID) { \
             addPullRequestReview(input: {pullRequestId: $pr, commitOID: $commit}) \
             { pullRequestReview { id state } } }",
            &serde_json::json!({ "pr": pull_request_id, "commit": commit }),
        )?;
        data.add_pull_request_review
            .and_then(|p| p.pull_request_review)
            .map(|r| r.id)
            .ok_or_else(|| missing("review"))
    }

    /// `addPullRequestReviewThread` into the pending review `review_id`.
    /// The new thread's node id.
    pub fn add_review_thread(&self, review_id: &str, thread: &NewThread) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            add_pull_request_review_thread: Option<ThreadPayload>,
        }
        let mut input = thread.input();
        input["pullRequestReviewId"] = serde_json::json!(review_id);
        let data: Data = self.post_graphql_strict(
            "mutation AddReviewThread($input: AddPullRequestReviewThreadInput!) { \
             addPullRequestReviewThread(input: $input) { thread { id } } }",
            &serde_json::json!({ "input": input }),
        )?;
        data.add_pull_request_review_thread
            .and_then(|p| p.thread)
            .map(|t| t.id)
            .ok_or_else(|| missing("thread"))
    }

    /// GitHub's "Add single comment": a review with this one thread,
    /// submitted as a comment at once. The review's node id.
    pub fn add_single_review_comment(
        &self,
        pull_request_id: &str,
        commit: &str,
        thread: &NewThread,
    ) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            add_pull_request_review: Option<ReviewPayload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation AddSingleComment($pr: ID!, $commit: GitObjectID, \
             $threads: [DraftPullRequestReviewThread]) { \
             addPullRequestReview(input: {pullRequestId: $pr, commitOID: $commit, \
             event: COMMENT, threads: $threads}) { pullRequestReview { id state } } }",
            &serde_json::json!({
                "pr": pull_request_id,
                "commit": commit,
                "threads": [thread.input()],
            }),
        )?;
        data.add_pull_request_review
            .and_then(|p| p.pull_request_review)
            .map(|r| r.id)
            .ok_or_else(|| missing("review"))
    }

    /// `submitPullRequestReview`: the pending review goes out as a
    /// comment, an approval or a request for changes.
    pub fn submit_review(&self, review_id: &str, event: ReviewEvent, body: &str) -> Result<()> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            submit_pull_request_review: Option<ReviewPayload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation SubmitReview($review: ID!, $event: PullRequestReviewEvent!, $body: String) { \
             submitPullRequestReview(input: {pullRequestReviewId: $review, event: $event, \
             body: $body}) { pullRequestReview { id state } } }",
            &serde_json::json!({ "review": review_id, "event": event.graphql(), "body": body }),
        )?;
        data.submit_pull_request_review
            .and_then(|p| p.pull_request_review)
            .map(|_| ())
            .ok_or_else(|| missing("review"))
    }

    /// `addPullRequestReview` with an event and no threads: a review that
    /// is only a verdict (and a summary).
    pub fn add_review(
        &self,
        pull_request_id: &str,
        commit: &str,
        event: ReviewEvent,
        body: &str,
    ) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            add_pull_request_review: Option<ReviewPayload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation AddReview($pr: ID!, $commit: GitObjectID, $event: PullRequestReviewEvent, \
             $body: String) { addPullRequestReview(input: {pullRequestId: $pr, \
             commitOID: $commit, event: $event, body: $body}) { pullRequestReview { id state } } }",
            &serde_json::json!({
                "pr": pull_request_id,
                "commit": commit,
                "event": event.graphql(),
                "body": body,
            }),
        )?;
        data.add_pull_request_review
            .and_then(|p| p.pull_request_review)
            .map(|r| r.id)
            .ok_or_else(|| missing("review"))
    }

    /// `deletePullRequestReview`: the pending review and its comments.
    pub fn delete_pending_review(&self, review_id: &str) -> Result<()> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            delete_pull_request_review: Option<ReviewPayload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation DeleteReview($review: ID!) { deletePullRequestReview(input: \
             {pullRequestReviewId: $review}) { pullRequestReview { id state } } }",
            &serde_json::json!({ "review": review_id }),
        )?;
        data.delete_pull_request_review
            .map(|_| ())
            .ok_or_else(|| missing("review"))
    }

    /// `updatePullRequestReviewComment`
    pub fn update_review_comment(&self, comment_id: &str, body: &str) -> Result<()> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Payload {
            pull_request_review_comment: Option<WireId>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            update_pull_request_review_comment: Option<Payload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation UpdateReviewComment($comment: ID!, $body: String!) { \
             updatePullRequestReviewComment(input: {pullRequestReviewCommentId: $comment, \
             body: $body}) { pullRequestReviewComment { id } } }",
            &serde_json::json!({ "comment": comment_id, "body": body }),
        )?;
        data.update_pull_request_review_comment
            .and_then(|p| p.pull_request_review_comment)
            .map(|_| ())
            .ok_or_else(|| missing("comment"))
    }

    /// `deletePullRequestReviewComment`
    pub fn delete_review_comment(&self, comment_id: &str) -> Result<()> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            delete_pull_request_review_comment: Option<serde_json::Value>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation DeleteReviewComment($comment: ID!) { deletePullRequestReviewComment(input: \
             {id: $comment}) { pullRequestReview { id } } }",
            &serde_json::json!({ "comment": comment_id }),
        )?;
        data.delete_pull_request_review_comment
            .map(|_| ())
            .ok_or_else(|| missing("comment"))
    }

    /// `addComment` on the pull request's conversation. The comment's id.
    pub fn add_pull_request_comment(&self, pull_request_id: &str, body: &str) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Edge {
            node: Option<WireId>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Payload {
            comment_edge: Option<Edge>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            add_comment: Option<Payload>,
        }
        let data: Data = self.post_graphql_strict(
            "mutation AddConversationComment($subject: ID!, $body: String!) { \
             addComment(input: {subjectId: $subject, body: $body}) { commentEdge { node { id } } } }",
            &serde_json::json!({ "subject": pull_request_id, "body": body }),
        )?;
        data.add_comment
            .and_then(|p| p.comment_edge)
            .and_then(|e| e.node)
            .map(|n| n.id)
            .ok_or_else(|| missing("comment"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_page_parses_and_flags_pending_and_file_level() {
        let json = r#"{
            "id": "PR_1",
            "reviewThreads": {
                "pageInfo": {"hasNextPage": false, "endCursor": null},
                "nodes": [
                    {
                        "id": "T_1", "path": "src/main.rs", "line": 3, "startLine": null,
                        "originalLine": 3, "originalStartLine": null, "diffSide": "RIGHT",
                        "startDiffSide": null, "isResolved": false, "isOutdated": false,
                        "resolvedBy": null, "viewerCanResolve": true, "viewerCanUnresolve": true,
                        "viewerCanReply": true,
                        "comments": {"nodes": [{
                            "id": "C_1", "databaseId": 10, "body": "nit", "createdAt": "2024-01-01T00:00:00Z",
                            "updatedAt": "2024-01-01T00:00:00Z", "url": "u", "state": "PENDING",
                            "outdated": false, "viewerCanUpdate": true, "viewerCanDelete": true,
                            "viewerDidAuthor": true, "diffHunk": "@@", "author": {"login": "me", "avatarUrl": null},
                            "originalCommit": {"oid": "abc"}, "commit": {"oid": "abc"}, "replyTo": null,
                            "pullRequestReview": {"id": "R_1", "state": "PENDING"}
                        }]}
                    },
                    {
                        "id": "T_2", "path": "README.md", "line": null, "originalLine": null,
                        "diffSide": "RIGHT", "isResolved": true, "isOutdated": false,
                        "resolvedBy": {"login": "mona"},
                        "comments": {"nodes": [null]}
                    }
                ]
            }
        }"#;
        let pr: WireThreadsPullRequest = serde_json::from_str(json).unwrap();
        let threads: Vec<ReviewThread> = pr
            .review_threads
            .nodes
            .into_iter()
            .flatten()
            .map(Into::into)
            .collect();
        assert_eq!(threads.len(), 2);
        assert!(threads[0].is_pending());
        assert!(!threads[0].is_file_level());
        assert_eq!(threads[0].comments[0].review_id.as_deref(), Some("R_1"));
        assert_eq!(threads[0].original_commit(), Some("abc"));
        assert!(threads[1].is_file_level());
        assert_eq!(threads[1].resolved_by.as_deref(), Some("mona"));
        assert!(threads[1].comments.is_empty());
    }

    #[test]
    fn timeline_items_convert_and_skip_unknown() {
        let json = serde_json::json!({
            "totalCount": 3,
            "nodes": [
                {"__typename": "PullRequestCommit", "commit": {"oid": "a1", "messageHeadline": "Fix",
                 "committedDate": "2024-01-01T00:00:00Z", "author": {"name": "Mona", "user": {"login": "mona"}}}},
                {"__typename": "PullRequestReview", "id": "R_2", "state": "APPROVED", "body": "",
                 "submittedAt": "2024-01-02T00:00:00Z", "url": "u", "author": {"login": "hubot"},
                 "comments": {"totalCount": 2}},
                {"__typename": "PullRequestReview", "id": "R_3", "state": "PENDING", "body": "",
                 "url": "u", "author": {"login": "me"}},
                {"__typename": "SomethingNew", "createdAt": "x"},
                {"__typename": "MergedEvent", "createdAt": "2024-01-03T00:00:00Z", "actor": null,
                 "mergeRefName": "main"}
            ]
        });
        let t: WireTimeline = serde_json::from_value(json).unwrap();
        let items: Vec<TimelineItem> = t
            .nodes
            .into_iter()
            .flatten()
            .filter_map(WireTimelineItem::convert)
            .collect();
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], TimelineItem::Commit { author, .. } if author == "mona"));
        assert!(matches!(&items[1], TimelineItem::Review(r) if r.comment_count == 2));
        assert!(
            matches!(&items[2], TimelineItem::Merged { actor, into, .. } if actor == "ghost" && into == "main")
        );
    }

    #[test]
    fn new_thread_input_only_sends_a_range_when_it_is_one() {
        let single = NewThread {
            path: "a".into(),
            line: 4,
            side: DiffSide::Right,
            start_line: Some(4),
            start_side: None,
            body: "b".into(),
        };
        assert!(single.input().get("startLine").is_none());
        let range = NewThread {
            start_line: Some(2),
            ..single
        };
        let input = range.input();
        assert_eq!(input["startLine"], 2);
        assert_eq!(input["startSide"], "RIGHT");
    }

    #[test]
    fn overview_picks_the_viewers_pending_review() {
        let json = serde_json::json!({
            "viewer": {"login": "me"},
            "repository": {"pullRequest": {
                "id": "PR_1", "number": 7, "title": "T", "body": "", "state": "OPEN",
                "commits": {"totalCount": 2, "nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS"}}}]},
                "pendingReviews": {"nodes": [{"id": "R_9", "body": "", "author": {"login": "me"},
                    "comments": {"totalCount": 3}}]},
                "reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "Team", "name": "core"}},
                    {"requestedReviewer": {"__typename": "User", "login": "hubot", "avatarUrl": null}}]},
                "timelineItems": {"totalCount": 100, "nodes": []}
            }}
        });
        let data: WireOverviewData = serde_json::from_value(json).unwrap();
        let pr = data.repository.unwrap().pull_request.unwrap();
        let o = convert_overview(pr, data.viewer.login);
        assert_eq!(o.commit_count, 2);
        assert_eq!(o.checks, Some(RollupState::Success));
        assert_eq!(o.pending_review.as_ref().map(|r| r.comment_count), Some(3));
        assert_eq!(o.review_requests.len(), 2);
        assert!(o.review_requests[0].team);
        assert_eq!(o.timeline_more, 20);
    }
}
