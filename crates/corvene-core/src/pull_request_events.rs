//! Corvene `354-pull-request-event-notifications` (desktop/desktop#21474):
//! notifications for pull request events of any repository an account can
//! see, next to GHD's Alive ones (`lib/stores/notifications-store.ts`, only
//! reviews, comments and failed checks of the selected repository's pull
//! requests, see [`crate::alive`]).
//!
//! Every signed-in GitHub account (flag `527-multiple-accounts`: each one,
//! with its own token) is polled in the background:
//! - the notifications API (`GET /notifications` with `If-Modified-Since`,
//!   no faster than its `X-Poll-Interval`) names the pull request threads
//!   that changed; each is then looked at for what happened since the last
//!   poll;
//! - when the API refuses the token (403 / 404: a fine-grained or app
//!   token, an Enterprise server that turned it off) the account falls back
//!   to the issue search (`review-requested:@me`, `author:@me`,
//!   `mentions:@me`, every [`SEARCH_INTERVAL`]), which cannot see team
//!   mentions.
//!
//! The events: a review asked of you or your team, an approval or changes
//! requested on your pull request (also on Enterprise, which has no Alive),
//! your pull request merged by someone else, and a mention of you or your
//! team. Each has a checkbox in Settings › Notifications; only events after
//! launch notify, and one already shown by Alive (the same review) is not
//! shown again. A click opens the repository in Corvene when it is listed
//! (the pull request's review view with `348-pull-request-review`, else the
//! GHD review dialog or the page on GitHub), or the page on GitHub; the
//! Settings choice "Open on GitHub" always does the latter.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime};

use corvene_github::api::{ApiPullRequest, ApiPullRequestReview, ApiPullRequestReviewState};
use corvene_github::pull_request_events::ApiNotificationThread;
use corvene_github::{Client, GitHubError};
use corvene_models::{Account, PullRequest, parse_iso8601};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::notifications::{review_verb, truncate_with_ellipsis};
use crate::persistence::{NotificationClickAction, PullRequestEventNotifications};
use crate::remote::spawn_bg;
use crate::state::Popup;

/// How often the poll loop wakes to see which accounts are due.
const TICK: Duration = Duration::from_secs(15);
/// The least time between two notifications API polls of one account
/// (GitHub's `X-Poll-Interval` is usually 60 s and may be longer).
const MIN_POLL_INTERVAL: Duration = Duration::from_secs(60);
/// The time between two searches of an account on the search fallback
/// (three searches each; the search API allows 30 a minute).
pub const SEARCH_INTERVAL: Duration = Duration::from_secs(180);
/// The time after a failed poll (network down, rate limited) before the
/// next try.
const RETRY_INTERVAL: Duration = Duration::from_secs(300);
/// How far back each poll looks again before the previous one (events are
/// keyed, so the overlap never shows one twice).
const OVERLAP: Duration = Duration::from_secs(120);
/// The posted notifications kept for the control hook.
const RECENT: usize = 20;

/// Where an account's events come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSource {
    NotificationsApi,
    Search,
}

/// One account's polling state.
#[derive(Clone, Debug)]
struct AccountPoll {
    source: EventSource,
    /// The notifications API's `Last-Modified`.
    last_modified: Option<String>,
    /// Events after this are new.
    watermark: SystemTime,
    next_poll: Instant,
    in_flight: bool,
}

/// A notification shown (newest last), for the control hook.
#[derive(Clone, Debug, Serialize)]
pub struct PostedEvent {
    pub title: String,
    pub body: String,
    pub notification: PullRequestEventNotification,
}

/// The poller's state (on [`crate::alive::AliveState`]).
#[derive(Default)]
pub struct PullRequestEventsState {
    /// `Account::key` → its polling state.
    polls: HashMap<String, AccountPoll>,
    /// Keys of the events already shown ([`PullRequestEvent::key`]), Alive's
    /// reviews included.
    pub seen: HashSet<String>,
    pub recent: Vec<PostedEvent>,
    started: bool,
}

impl PullRequestEventsState {
    /// `(account key, source)` of each polled account (control hook).
    pub fn sources(&self) -> Vec<(String, EventSource)> {
        let mut out: Vec<_> = self
            .polls
            .iter()
            .map(|(k, p)| (k.clone(), p.source))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// What happened on the pull request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PullRequestEvent {
    /// A review was asked of you, or of `team` (`org/slug`).
    ReviewRequested { team: Option<String> },
    /// Your pull request was approved or got changes requested.
    Reviewed { review: ApiPullRequestReview },
    /// `by` merged your pull request.
    Merged { by: String },
    /// `by` mentioned you (or `team`) in `body` (a comment, or the pull
    /// request's description).
    Mentioned {
        by: String,
        team: Option<String>,
        body: String,
        /// The comment's id; `None` for the description.
        comment: Option<u64>,
    },
}

/// One notification: the account, the pull request and the event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestEventNotification {
    /// `Account::key` of the account it came to.
    pub account: String,
    pub owner: String,
    pub name: String,
    pub pull_request: PullRequest,
    /// The pull request's page.
    pub html_url: String,
    pub event: PullRequestEvent,
}

impl PullRequestEventNotification {
    /// The dedup key: one notification per event.
    pub fn key(&self) -> String {
        let pr = format!(
            "{}/{}#{}",
            self.owner.to_lowercase(),
            self.name.to_lowercase(),
            self.pull_request.number
        );
        match &self.event {
            PullRequestEvent::ReviewRequested { .. } => {
                format!("review-requested:{}:{pr}", self.account)
            }
            PullRequestEvent::Reviewed { review } => review_key(review.id),
            PullRequestEvent::Merged { .. } => format!("merged:{pr}"),
            PullRequestEvent::Mentioned { comment, .. } => match comment {
                Some(id) => format!("mention:{}:{id}", self.account),
                None => format!("mention:{}:{pr}", self.account),
            },
        }
    }

    pub fn title(&self) -> String {
        match &self.event {
            PullRequestEvent::ReviewRequested { team: None } => "Your review was requested".into(),
            PullRequestEvent::ReviewRequested { team: Some(team) } => {
                format!("A review was requested from @{team}")
            }
            // GHD's review notification title
            PullRequestEvent::Reviewed { review } => format!(
                "@{} {} your pull request",
                review.user.login,
                review_verb(review.state)
            ),
            PullRequestEvent::Merged { by } => format!("@{by} merged your pull request"),
            PullRequestEvent::Mentioned { by, team: None, .. } => {
                format!("@{by} mentioned you in a pull request")
            }
            PullRequestEvent::Mentioned {
                by, team: Some(t), ..
            } => format!("@{by} mentioned @{t} in a pull request"),
        }
    }

    pub fn body(&self) -> String {
        let pr = &self.pull_request;
        let head = format!("{} #{}", pr.title, pr.number);
        match &self.event {
            PullRequestEvent::ReviewRequested { .. } | PullRequestEvent::Merged { .. } => {
                format!("{head}\n{}/{} by @{}", self.owner, self.name, pr.author)
            }
            PullRequestEvent::Reviewed { review } => {
                format!("{head}\n{}", truncate_with_ellipsis(&review.body, 50))
            }
            PullRequestEvent::Mentioned { body, .. } => {
                format!("{head}\n{}", truncate_with_ellipsis(body, 50))
            }
        }
    }

    fn wanted(&self, settings: &PullRequestEventNotifications) -> bool {
        match self.event {
            PullRequestEvent::ReviewRequested { .. } => settings.review_requested,
            PullRequestEvent::Reviewed { .. } => settings.reviews,
            PullRequestEvent::Merged { .. } => settings.merged,
            PullRequestEvent::Mentioned { .. } => settings.mentions,
        }
    }
}

/// The dedup key of a review, shared with Alive's review notifications.
pub fn review_key(id: u64) -> String {
    format!("review:{id}")
}

/// What the background poll of one account found.
struct PollOutcome {
    events: Vec<PullRequestEventNotification>,
    source: EventSource,
    last_modified: Option<String>,
    /// The time the next poll starts looking from.
    watermark: SystemTime,
    wait: Duration,
    /// The token was revoked.
    token_invalidated: bool,
}

/// The inputs of one background poll.
struct PollRequest {
    account_key: String,
    login: String,
    source: EventSource,
    last_modified: Option<String>,
    watermark: SystemTime,
    settings: PullRequestEventNotifications,
}

impl Dispatcher {
    /// Start the poll loop (once, at launch). It idles while the flag,
    /// Settings › Notifications or every event checkbox is off.
    pub fn start_pull_request_event_polling(cx: &mut dyn Host) {
        let state = Self::state(cx);
        let started = state.update(cx, |s, _| {
            std::mem::replace(&mut s.alive.pull_request_events.started, true)
        });
        if started {
            return;
        }
        cx.spawn(async move |cx: &mut AsyncCtx| {
            loop {
                cx.update(|cx| Self::poll_pull_request_events(false, cx));
                cx.background_executor().timer(TICK).await;
            }
        })
        .detach();
    }

    /// Whether the poller should run now.
    fn pull_request_events_enabled(cx: &dyn Host) -> bool {
        let s = Self::state(cx).read(cx);
        s.flags
            .bool(crate::flags::ids::PULL_REQUEST_EVENT_NOTIFICATIONS)
            && s.settings.notifications_enabled
            && s.settings.pull_request_event_notifications.any()
    }

    /// Control hook `pr-events-poll`: poll every account now.
    pub fn poll_pull_request_events_now(cx: &mut dyn Host) {
        Self::poll_pull_request_events(true, cx);
    }

    /// One tick: start a background poll for each account that is due
    /// (`force`: each one not already polling).
    fn poll_pull_request_events(force: bool, cx: &mut dyn Host) {
        let state = Self::state(cx);
        if !Self::pull_request_events_enabled(cx) {
            // a later enable starts afresh: no backlog of what happened
            // while it was off
            state.update(cx, |s, _| s.alive.pull_request_events.polls.clear());
            return;
        }
        let (accounts, settings) = {
            let s = state.read(cx);
            (
                s.accounts.clone(),
                s.settings.pull_request_event_notifications,
            )
        };
        let keys: HashSet<String> = accounts.iter().map(Account::key).collect();
        let now = Instant::now();
        let due: Vec<(Account, PollRequest)> = state.update(cx, |s, _| {
            let polls = &mut s.alive.pull_request_events.polls;
            polls.retain(|key, _| keys.contains(key));
            let mut due = Vec::new();
            for account in &accounts {
                let poll = polls.entry(account.key()).or_insert_with(|| AccountPoll {
                    source: EventSource::NotificationsApi,
                    last_modified: None,
                    watermark: SystemTime::now(),
                    next_poll: now,
                    in_flight: false,
                });
                if poll.in_flight || (!force && poll.next_poll > now) {
                    continue;
                }
                poll.in_flight = true;
                due.push((
                    account.clone(),
                    PollRequest {
                        account_key: account.key(),
                        login: account.login.clone(),
                        source: poll.source,
                        last_modified: poll.last_modified.clone(),
                        watermark: poll.watermark,
                        settings,
                    },
                ));
            }
            due
        });
        for (account, request) in due {
            let Some(client) = crate::alive::account_client(&account) else {
                state.update(cx, |s, _| {
                    if let Some(p) = s.alive.pull_request_events.polls.get_mut(&account.key()) {
                        p.in_flight = false;
                        p.next_poll = Instant::now() + RETRY_INTERVAL;
                    }
                });
                continue;
            };
            let key = request.account_key.clone();
            spawn_bg(
                cx,
                move || poll_account(&client, request),
                move |outcome, cx| Self::pull_request_events_polled(key, account, outcome, cx),
            );
        }
    }

    fn pull_request_events_polled(
        key: String,
        account: Account,
        outcome: PollOutcome,
        cx: &mut dyn Host,
    ) {
        let state = Self::state(cx);
        state.update(cx, |s, _| {
            if let Some(p) = s.alive.pull_request_events.polls.get_mut(&key) {
                p.in_flight = false;
                if p.source != outcome.source {
                    info!(key, source = ?outcome.source, "pull request events now come from the search");
                }
                p.source = outcome.source;
                p.last_modified = outcome.last_modified.clone();
                p.watermark = outcome.watermark;
                p.next_poll = Instant::now() + outcome.wait;
            }
        });
        if outcome.token_invalidated {
            Self::token_invalidated(&account.endpoint, &account.login, cx);
            return;
        }
        for notification in outcome.events {
            Self::notify_pull_request_event_notification(notification, cx);
        }
    }

    /// Show `notification` unless it was shown before, its checkbox is off
    /// or the flag / Settings › Notifications turned off meanwhile.
    pub fn notify_pull_request_event_notification(
        notification: PullRequestEventNotification,
        cx: &mut dyn Host,
    ) {
        if !Self::pull_request_events_enabled(cx) {
            return;
        }
        let state = Self::state(cx);
        let wanted = notification.wanted(&state.read(cx).settings.pull_request_event_notifications);
        let key = notification.key();
        let fresh = state.update(cx, |s, _| s.alive.pull_request_events.seen.insert(key));
        if !wanted || !fresh {
            return;
        }
        Self::post_pull_request_event_notification(notification, cx);
    }

    fn post_pull_request_event_notification(
        notification: PullRequestEventNotification,
        cx: &mut dyn Host,
    ) {
        let (title, body) = (notification.title(), notification.body());
        let Ok(payload) = serde_json::to_string(&notification) else {
            return;
        };
        // one identifier per event, short enough for a Windows toast tag
        let identifier = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            notification.key().hash(&mut hasher);
            format!("corvene-pr-event-{:016x}", hasher.finish())
        };
        info!(%identifier, %title, "showing pull request event notification");
        Self::state(cx).update(cx, |s, _| {
            let recent = &mut s.alive.pull_request_events.recent;
            recent.push(PostedEvent {
                title: title.clone(),
                body: body.clone(),
                notification,
            });
            if recent.len() > RECENT {
                recent.remove(0);
            }
        });
        corvene_platform::notifications::show(
            &identifier,
            &title,
            &body,
            Some(&payload),
            |result| match result {
                Ok(()) => debug!("notification posted"),
                Err(err) => warn!(%err, "notification not shown"),
            },
        );
    }

    /// A click on a pull request event notification; `false` when
    /// `payload` is not one.
    pub fn pull_request_event_notification_clicked(payload: &str, cx: &mut dyn Host) -> bool {
        let Ok(n) = serde_json::from_str::<PullRequestEventNotification>(payload) else {
            return false;
        };
        Self::open_pull_request_event(n, cx);
        true
    }

    /// Where a click goes: see the module doc.
    pub fn open_pull_request_event(n: PullRequestEventNotification, cx: &mut dyn Host) {
        let state = Self::state(cx);
        let (action, repo, review_view) = {
            let s = state.read(cx);
            let repo = listed_repository(s, &n);
            (
                s.settings.pull_request_notification_click,
                repo,
                s.flags.bool(crate::flags::ids::PULL_REQUEST_REVIEW),
            )
        };
        let repo = match (action, repo) {
            (NotificationClickAction::OpenInCorvene, Some(repo)) => repo,
            _ => {
                Self::open_url(&n.html_url, cx);
                return;
            }
        };
        if state.read(cx).selected != Some(repo) {
            Self::select_repository(repo, cx);
        }
        if review_view {
            Self::review_pull_request(repo, n.pull_request, cx);
            return;
        }
        match n.event {
            // GHD's dialog for a review notification
            PullRequestEvent::Reviewed { review } => {
                let (should_change_repository, should_checkout_branch) =
                    crate::notifications::switch_flags(state.read(cx), repo, &n.pull_request);
                Self::show_popup(
                    Popup::PullRequestReview {
                        repo,
                        pull_request: n.pull_request,
                        review,
                        should_checkout_branch,
                        should_change_repository,
                    },
                    cx,
                );
            }
            _ => Self::open_url(&n.html_url, cx),
        }
    }

    /// `CORVENE_POPUP=pr-event:<kind>`: a sample event of `kind`
    /// (`review-requested`, `reviewed`, `merged`, `mentioned`) for the
    /// selected repository, through the notification path.
    pub fn simulate_pull_request_event_notification(kind: &str, cx: &mut dyn Host) {
        let state = Self::state(cx);
        let (pull_request, account) = {
            let s = state.read(cx);
            let pr = match s.selected {
                Some(id) => crate::samples::pull_request(id, cx),
                None => crate::samples::sample_pull_request(),
            };
            let account = s.accounts.first().map(Account::key).unwrap_or_default();
            (pr, account)
        };
        let github = pull_request.base.repository.clone();
        let (owner, name) = github
            .as_ref()
            .map(|g| (g.owner.clone(), g.name.clone()))
            .unwrap_or_default();
        let event = match kind {
            "reviewed" => PullRequestEvent::Reviewed {
                review: crate::samples::review(ApiPullRequestReviewState::Approved),
            },
            "merged" => PullRequestEvent::Merged {
                by: "octocat".into(),
            },
            "mentioned" => PullRequestEvent::Mentioned {
                by: "octocat".into(),
                team: None,
                body: crate::samples::comment().body,
                comment: Some(crate::samples::comment().id),
            },
            _ => PullRequestEvent::ReviewRequested { team: None },
        };
        let notification = PullRequestEventNotification {
            account,
            owner,
            name,
            html_url: pull_request.html_url().unwrap_or_default(),
            pull_request,
            event,
        };
        // a sample may be shown again
        state.update(cx, |s, _| {
            s.alive.pull_request_events.seen.remove(&notification.key());
        });
        Self::post_pull_request_event_notification(notification, cx);
    }
}

/// The listed repository a notification belongs to: the repository itself
/// or a fork of it, one that uses the notification's account first.
fn listed_repository(s: &crate::state::AppState, n: &PullRequestEventNotification) -> Option<u64> {
    let same = |owner: &str, name: &str| {
        owner.eq_ignore_ascii_case(&n.owner) && name.eq_ignore_ascii_case(&n.name)
    };
    let matching: Vec<u64> = s
        .repositories
        .iter()
        .filter(|r| !r.missing)
        .filter(|r| {
            r.github.as_ref().is_some_and(|g| {
                same(&g.owner, &g.name)
                    || g.parent.as_ref().is_some_and(|p| same(&p.owner, &p.name))
            })
        })
        .map(|r| r.id)
        .collect();
    matching
        .iter()
        .copied()
        .find(|id| {
            s.account_for_repository(*id)
                .is_some_and(|a| a.key() == n.account)
        })
        .or_else(|| matching.first().copied())
}

/// The blocking poll of one account.
fn poll_account(client: &Client, request: PollRequest) -> PollOutcome {
    let started = SystemTime::now();
    let mut outcome = PollOutcome {
        events: Vec::new(),
        source: request.source,
        last_modified: request.last_modified.clone(),
        watermark: request.watermark,
        wait: RETRY_INTERVAL,
        token_invalidated: false,
    };
    let result = match request.source {
        EventSource::NotificationsApi => match poll_notifications_api(client, &request) {
            Ok((events, last_modified, server_now, wait)) => {
                outcome.last_modified = last_modified;
                outcome.wait = wait;
                Ok((events, server_now.unwrap_or(started)))
            }
            // the token or the server does not allow the notifications API
            Err(GitHubError::Api {
                status: 403 | 404,
                ref message,
                ..
            }) => {
                info!(login = %request.login, %message, "notifications API refused; searching instead");
                outcome.source = EventSource::Search;
                outcome.last_modified = None;
                poll_search(client, &request).map(|events| {
                    outcome.wait = SEARCH_INTERVAL;
                    (events, started)
                })
            }
            Err(err) => Err(err),
        },
        EventSource::Search => poll_search(client, &request).map(|events| {
            outcome.wait = SEARCH_INTERVAL;
            (events, started)
        }),
    };
    match result {
        Ok((events, now)) => {
            outcome.events = events;
            outcome.watermark = now
                .checked_sub(OVERLAP)
                .unwrap_or(now)
                .max(request.watermark);
        }
        Err(err) => {
            outcome.token_invalidated = err.is_token_invalidated();
            warn!(%err, login = %request.login, "pull request events poll failed");
        }
    }
    outcome
}

/// The notifications API half: `(events, Last-Modified, the server's
/// clock, the wait before the next poll)`.
#[allow(clippy::type_complexity)]
fn poll_notifications_api(
    client: &Client,
    request: &PollRequest,
) -> corvene_github::Result<(
    Vec<PullRequestEventNotification>,
    Option<String>,
    Option<SystemTime>,
    Duration,
)> {
    let poll = client.notifications(request.last_modified.as_deref())?;
    let wait = poll
        .poll_interval
        .map(Duration::from_secs)
        .unwrap_or(MIN_POLL_INTERVAL)
        .max(MIN_POLL_INTERVAL);
    let server_now = poll.date.as_deref().and_then(parse_http_date);
    let last_modified = poll.last_modified.or(request.last_modified.clone());
    let Some(threads) = poll.threads else {
        return Ok((Vec::new(), last_modified, server_now, wait));
    };
    let mut events = Vec::new();
    for thread in threads {
        if thread.subject.kind != "PullRequest" || !newer(&thread.updated_at, request.watermark) {
            continue;
        }
        match thread_events(client, request, &thread) {
            Ok(found) => events.extend(found),
            Err(err) if err.is_token_invalidated() => return Err(err),
            Err(err) => debug!(%err, thread = %thread.id, "could not look at the thread"),
        }
    }
    Ok((events, last_modified, server_now, wait))
}

/// The pull request number at the end of a `…/pulls/{n}` API URL.
fn pull_request_number(url: &str) -> Option<u64> {
    url.rsplit('/').next()?.parse().ok()
}

/// What happened on one changed thread since the watermark.
fn thread_events(
    client: &Client,
    request: &PollRequest,
    thread: &ApiNotificationThread,
) -> corvene_github::Result<Vec<PullRequestEventNotification>> {
    let Some(number) = thread.subject.url.as_deref().and_then(pull_request_number) else {
        return Ok(Vec::new());
    };
    let (owner, name) = (
        thread.repository.owner.login.as_str(),
        thread.repository.name.as_str(),
    );
    let settings = &request.settings;
    match thread.reason.as_str() {
        "review_requested" if settings.review_requested => {
            let pr = client.pull_request(owner, name, number)?;
            Ok(review_request_event(client, request, owner, name, pr)
                .into_iter()
                .collect())
        }
        "review_requested" => Ok(Vec::new()),
        "mention" | "team_mention" => {
            if !settings.mentions {
                return Ok(Vec::new());
            }
            let pr = client.pull_request(owner, name, number)?;
            let comment = match thread.subject.latest_comment_url.as_deref() {
                Some(url) => client.comment_at(url)?,
                None => None,
            };
            let mention = match comment {
                Some(c) => mention_in(
                    &c.body,
                    &c.user.login,
                    &c.created_at,
                    Some(c.id),
                    request,
                    thread.reason == "team_mention",
                ),
                // the description of a pull request that was just opened
                None => mention_in(
                    pr.body.as_deref().unwrap_or_default(),
                    &pr.user.login,
                    &pr.created_at,
                    None,
                    request,
                    thread.reason == "team_mention",
                ),
            };
            Ok(mention
                .map(|event| notification_for(client, request, owner, name, pr, event))
                .into_iter()
                .collect())
        }
        // the user's own pull requests: merged, reviewed (threads the user
        // only watches are left alone)
        "author" | "comment" | "state_change" | "manual" => {
            own_pull_request_events(client, request, owner, name, number, None)
        }
        _ => Ok(Vec::new()),
    }
}

/// A review request of `pr` that is still open (`None` when the user and
/// their teams are no longer asked, or the pull request is closed).
fn review_request_event(
    client: &Client,
    request: &PollRequest,
    owner: &str,
    name: &str,
    pr: ApiPullRequest,
) -> Option<PullRequestEventNotification> {
    if pr.state != "open" || pr.user.login.eq_ignore_ascii_case(&request.login) {
        return None;
    }
    let you = pr
        .requested_reviewers
        .iter()
        .any(|u| u.login.eq_ignore_ascii_case(&request.login));
    let team = if you {
        None
    } else {
        // the notification says one of the user's teams was asked
        Some(format!("{owner}/{}", pr.requested_teams.first()?.slug))
    };
    Some(notification_for(
        client,
        request,
        owner,
        name,
        pr,
        PullRequestEvent::ReviewRequested { team },
    ))
}

/// Merges and reviews of the user's pull request `number` since the
/// watermark; `pr` when it was fetched already.
fn own_pull_request_events(
    client: &Client,
    request: &PollRequest,
    owner: &str,
    name: &str,
    number: u64,
    pr: Option<ApiPullRequest>,
) -> corvene_github::Result<Vec<PullRequestEventNotification>> {
    let settings = &request.settings;
    if !settings.merged && !settings.reviews {
        return Ok(Vec::new());
    }
    let pr = match pr {
        Some(pr) => pr,
        None => client.pull_request(owner, name, number)?,
    };
    if !pr.user.login.eq_ignore_ascii_case(&request.login) {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    if settings.reviews {
        for review in client.pull_request_reviews(owner, name, number)? {
            if is_notified_review(&review, request) {
                events.push(PullRequestEvent::Reviewed { review });
            }
        }
    }
    if settings.merged
        && let (Some(merged_at), Some(by)) = (pr.merged_at.as_deref(), pr.merged_by.as_ref())
        && newer(merged_at, request.watermark)
        && !by.login.eq_ignore_ascii_case(&request.login)
    {
        events.push(PullRequestEvent::Merged {
            by: by.login.clone(),
        });
    }
    Ok(events
        .into_iter()
        .map(|event| notification_for(client, request, owner, name, pr.clone(), event))
        .collect())
}

/// An approval or a request for changes by someone else, since the
/// watermark.
fn is_notified_review(review: &ApiPullRequestReview, request: &PollRequest) -> bool {
    matches!(
        review.state,
        ApiPullRequestReviewState::Approved | ApiPullRequestReviewState::ChangesRequested
    ) && !review.user.login.eq_ignore_ascii_case(&request.login)
        && newer(&review.submitted_at, request.watermark)
}

/// The search fallback: review requests, the user's pull requests and
/// mentions updated since the watermark.
fn poll_search(
    client: &Client,
    request: &PollRequest,
) -> corvene_github::Result<Vec<PullRequestEventNotification>> {
    let since = crate::samples::iso_at(request.watermark);
    let settings = &request.settings;
    let mut events = Vec::new();
    let each = |query: &str| -> corvene_github::Result<Vec<(String, String, u64)>> {
        Ok(client
            .search_pull_requests(query)?
            .into_iter()
            .filter(|item| newer(&item.updated_at, request.watermark))
            .filter_map(|item| {
                let (owner, name) = item.repository()?;
                Some((owner, name, item.number))
            })
            .collect())
    };
    if settings.review_requested {
        for (owner, name, number) in each(&format!(
            "is:pr is:open review-requested:@me updated:>={since}"
        ))? {
            let pr = client.pull_request(&owner, &name, number)?;
            events.extend(review_request_event(client, request, &owner, &name, pr));
        }
    }
    if settings.merged || settings.reviews {
        for (owner, name, number) in each(&format!("is:pr author:@me updated:>={since}"))? {
            events.extend(own_pull_request_events(
                client, request, &owner, &name, number, None,
            )?);
        }
    }
    if settings.mentions {
        for (owner, name, number) in each(&format!("is:pr mentions:@me updated:>={since}"))? {
            let pr = client.pull_request(&owner, &name, number)?;
            let mut found: Vec<PullRequestEvent> = client
                .issue_comments_since(&owner, &name, number, &since)?
                .into_iter()
                .filter_map(|c| {
                    mention_in(
                        &c.body,
                        &c.user.login,
                        &c.created_at,
                        Some(c.id),
                        request,
                        false,
                    )
                })
                .collect();
            found.extend(mention_in(
                pr.body.as_deref().unwrap_or_default(),
                &pr.user.login,
                &pr.created_at,
                None,
                request,
                false,
            ));
            for event in found {
                events.push(notification_for(
                    client,
                    request,
                    &owner,
                    &name,
                    pr.clone(),
                    event,
                ));
            }
        }
    }
    Ok(events)
}

/// A mention of the user (or, `team`, of a team: any `@org/team`) in
/// `body`, written by someone else after the watermark.
fn mention_in(
    body: &str,
    by: &str,
    created_at: &str,
    comment: Option<u64>,
    request: &PollRequest,
    team: bool,
) -> Option<PullRequestEvent> {
    if by.eq_ignore_ascii_case(&request.login) || !newer(created_at, request.watermark) {
        return None;
    }
    let team = if mentions_login(body, &request.login) {
        None
    } else if team {
        Some(first_team_mention(body)?)
    } else {
        return None;
    };
    Some(PullRequestEvent::Mentioned {
        by: by.to_string(),
        team,
        body: body.to_string(),
        comment,
    })
}

/// `@login` in `text` as a whole mention (not `@login-other`, not inside
/// an e-mail address, not `@login/team`).
pub fn mentions_login(text: &str, login: &str) -> bool {
    let lower = text.to_lowercase();
    let needle = format!("@{}", login.to_lowercase());
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/';
    lower.match_indices(&needle).any(|(ix, _)| {
        let before = lower[..ix].chars().next_back();
        let after = lower[ix + needle.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
            && !after.is_some_and(is_name)
    })
}

/// The first `@org/team` mention in `text`.
pub fn first_team_mention(text: &str) -> Option<String> {
    let name = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.';
    text.match_indices('@').find_map(|(ix, _)| {
        if text[..ix]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            return None;
        }
        let rest = &text[ix + 1..];
        let org_len = rest.find(|c: char| !name(c)).unwrap_or(rest.len());
        let after_org = rest[org_len..].strip_prefix('/')?;
        let team_len = after_org
            .find(|c: char| !name(c))
            .unwrap_or(after_org.len());
        (org_len > 0 && team_len > 0).then(|| {
            format!(
                "{}/{}",
                &rest[..org_len],
                after_org[..team_len].trim_end_matches('.')
            )
        })
    })
}

fn notification_for(
    client: &Client,
    request: &PollRequest,
    owner: &str,
    name: &str,
    pr: ApiPullRequest,
    event: PullRequestEvent,
) -> PullRequestEventNotification {
    let html_url = pr.html_url.clone().unwrap_or_else(|| {
        client
            .endpoint()
            .web(&format!("{owner}/{name}/pull/{}", pr.number))
    });
    let (pull_request, _) = crate::pull_requests::convert_pull_request(client, pr);
    PullRequestEventNotification {
        account: request.account_key.clone(),
        owner: owner.to_string(),
        name: name.to_string(),
        pull_request,
        html_url,
        event,
    }
}

/// Whether ISO-8601 `timestamp` is after `watermark`.
fn newer(timestamp: &str, watermark: SystemTime) -> bool {
    parse_iso8601(timestamp).is_some_and(|t| t > watermark)
}

/// An HTTP `Date` (`Tue, 06 Oct 2026 10:00:00 GMT`).
pub fn parse_http_date(value: &str) -> Option<SystemTime> {
    let mut parts = value.split_whitespace().skip(1);
    let day: u32 = parts.next()?.parse().ok()?;
    let month = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: u32 = parts.next()?.parse().ok()?;
    let time = parts.next()?;
    parse_iso8601(&format!("{year:04}-{month:02}-{day:02}T{time}Z"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PollRequest {
        PollRequest {
            account_key: "https://api.github.com|wasi-master".into(),
            login: "wasi-master".into(),
            source: EventSource::NotificationsApi,
            last_modified: None,
            watermark: parse_iso8601("2026-10-06T10:00:00Z").unwrap(),
            settings: PullRequestEventNotifications::default(),
        }
    }

    #[test]
    fn finds_whole_mentions_only() {
        assert!(mentions_login("cc @Wasi-Master please", "wasi-master"));
        assert!(mentions_login("@wasi-master.", "wasi-master"));
        assert!(!mentions_login("@wasi-master-bot", "wasi-master"));
        assert!(!mentions_login("me@wasi-master.dev", "wasi-master"));
        assert!(!mentions_login("@wasi-master/team", "wasi-master"));
    }

    #[test]
    fn finds_team_mentions() {
        assert_eq!(
            first_team_mention("ping @github/desktop-team."),
            Some("github/desktop-team".into())
        );
        assert_eq!(first_team_mention("hi @octocat"), None);
        assert_eq!(first_team_mention("mail a@b/c"), None);
    }

    #[test]
    fn mentions_need_someone_else_after_the_watermark() {
        let r = request();
        let after = "2026-10-06T10:05:00Z";
        assert!(mention_in("@wasi-master look", "octocat", after, Some(1), &r, false).is_some());
        assert!(mention_in("@wasi-master look", "wasi-master", after, None, &r, false).is_none());
        assert!(
            mention_in(
                "@wasi-master look",
                "octocat",
                "2026-10-06T09:00:00Z",
                None,
                &r,
                false
            )
            .is_none()
        );
        assert_eq!(
            mention_in("@org/reviewers look", "octocat", after, None, &r, true),
            Some(PullRequestEvent::Mentioned {
                by: "octocat".into(),
                team: Some("org/reviewers".into()),
                body: "@org/reviewers look".into(),
                comment: None,
            })
        );
        assert!(mention_in("@org/reviewers look", "octocat", after, None, &r, false).is_none());
    }

    #[test]
    fn parses_http_dates() {
        assert_eq!(
            parse_http_date("Tue, 06 Oct 2026 10:00:00 GMT"),
            parse_iso8601("2026-10-06T10:00:00Z")
        );
        assert_eq!(parse_http_date("garbage"), None);
    }

    #[test]
    fn titles_bodies_and_keys() {
        let n = |event| PullRequestEventNotification {
            account: "a|me".into(),
            owner: "Octo".into(),
            name: "Hello".into(),
            pull_request: crate::samples::sample_pull_request(),
            html_url: String::new(),
            event,
        };
        let rr = n(PullRequestEvent::ReviewRequested { team: None });
        assert_eq!(rr.title(), "Your review was requested");
        assert_eq!(
            rr.body(),
            "Render pull request bodies as Markdown #42\nOcto/Hello by @wasi-master"
        );
        assert_eq!(rr.key(), "review-requested:a|me:octo/hello#42");
        let team = n(PullRequestEvent::ReviewRequested {
            team: Some("octo/core".into()),
        });
        assert_eq!(team.title(), "A review was requested from @octo/core");
        let merged = n(PullRequestEvent::Merged { by: "hubot".into() });
        assert_eq!(merged.title(), "@hubot merged your pull request");
        let review = crate::samples::review(ApiPullRequestReviewState::ChangesRequested);
        let reviewed = n(PullRequestEvent::Reviewed {
            review: review.clone(),
        });
        assert_eq!(
            reviewed.title(),
            "@octocat requested changes on your pull request"
        );
        assert_eq!(reviewed.key(), review_key(review.id));
        let off = PullRequestEventNotifications {
            merged: false,
            ..Default::default()
        };
        assert!(!merged.wanted(&off));
        assert!(rr.wanted(&off));
    }

    #[test]
    fn reviews_by_others_after_the_watermark() {
        let r = request();
        let mut review = crate::samples::review(ApiPullRequestReviewState::Approved);
        review.submitted_at = "2026-10-06T10:01:00Z".into();
        assert!(is_notified_review(&review, &r));
        review.state = ApiPullRequestReviewState::Commented;
        assert!(!is_notified_review(&review, &r));
        review.state = ApiPullRequestReviewState::Approved;
        review.user.login = "wasi-master".into();
        assert!(!is_notified_review(&review, &r));
    }

    #[test]
    fn payload_round_trips() {
        let n = PullRequestEventNotification {
            account: "a|me".into(),
            owner: "o".into(),
            name: "r".into(),
            pull_request: crate::samples::sample_pull_request(),
            html_url: "https://github.com/o/r/pull/42".into(),
            event: PullRequestEvent::Merged { by: "hubot".into() },
        };
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(
            serde_json::from_str::<PullRequestEventNotification>(&json).unwrap(),
            n
        );
        // not a GHD pull request notification
        assert!(
            serde_json::from_str::<crate::notifications::PullRequestNotification>(&json).is_err()
        );
    }
}
