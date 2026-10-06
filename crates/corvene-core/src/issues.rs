//! Corvene `345-issues`: Repository › Issues…, the GitHub repository's
//! issues as a History mode, New Issue… and Create Branch from an issue.
//! GitHub Desktop has none of it: Repository › Create Issue on GitHub
//! opens the browser's form (`ui/app-menu`, `_openInBrowser`), and issues
//! only show up as `#` suggestions in the commit message
//! (`lib/stores/issues-store.ts`, which Corvene keeps in
//! [`crate::autocomplete`]).
//!
//! [`Dispatcher::show_issues`] swaps History's commit list for the list
//! ([`IssuesViewState`]): every open issue and the newest-updated closed
//! ones ([`CLOSED_PAGES`] pages), read through the REST API with the
//! account for the repository's endpoint. The selected issue's description
//! takes the commit view's place. New Issue… posts through the API; Create
//! Branch opens the usual dialog with the name GitHub itself would give
//! ([`issue_branch_name`]) and links the branch once it exists: its
//! description keeps the issue's URL and Create Pull Request prefills
//! "Closes #N"; after the first push GitHub's own `createLinkedBranch`
//! lists the branch under the issue's Development section (where the
//! account can push and the host knows the mutation, GitHub Enterprise
//! Server 3.7 and later; otherwise the local link is all there is).
//! Closing restores History's selection.

use corvene_github::{ApiIssue, Client, NewIssue};
use corvene_models::{GitHubRepository, Section};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::mco::Banner;
use crate::remote::spawn_bg;
use crate::state::{AppState, Popup, RepositoryState};

/// Pages of 100 closed issues read (the newest updated).
pub const CLOSED_PAGES: usize = 3;

/// Git config keys on a branch created from an issue.
pub const BRANCH_ISSUE_KEY: &str = "corvene-issue";
pub const BRANCH_ISSUE_NODE_KEY: &str = "corvene-issue-node";
pub const BRANCH_ISSUE_LINKED_KEY: &str = "corvene-issue-linked";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueLabel {
    pub name: String,
    /// Six hex digits (`#` stripped); empty when GitHub gave none.
    pub color: String,
    pub description: Option<String>,
}

impl From<corvene_github::ApiLabel> for IssueLabel {
    fn from(label: corvene_github::ApiLabel) -> Self {
        Self {
            name: label.name,
            color: label.color.trim_start_matches('#').to_string(),
            description: label.description.filter(|d| !d.is_empty()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueRow {
    pub number: u64,
    pub title: String,
    pub open: bool,
    /// `completed` | `not_planned` | `reopened`, when the host says.
    pub state_reason: Option<String>,
    pub body: String,
    pub html_url: String,
    /// GraphQL id, for `createLinkedBranch`.
    pub node_id: Option<String>,
    pub author: String,
    /// ISO-8601.
    pub created_at: Option<String>,
    pub updated_at: String,
    pub labels: Vec<IssueLabel>,
    /// Logins.
    pub assignees: Vec<String>,
    pub comments: u64,
}

impl IssueRow {
    /// `gh` supplies the page URL when the API record has none.
    pub fn from_api(issue: ApiIssue, gh: &GitHubRepository) -> Self {
        let html_url = issue
            .html_url
            .unwrap_or_else(|| format!("{}/issues/{}", gh.html_url, issue.number));
        Self {
            number: issue.number,
            title: issue.title,
            open: issue.state != "closed",
            state_reason: issue.state_reason,
            body: issue.body.unwrap_or_default(),
            html_url,
            node_id: issue.node_id,
            author: issue.user.map(|u| u.login).unwrap_or_default(),
            created_at: issue.created_at,
            updated_at: issue.updated_at,
            labels: issue.labels.into_iter().map(IssueLabel::from).collect(),
            assignees: issue.assignees.into_iter().map(|u| u.login).collect(),
            comments: issue.comments,
        }
    }

    /// Closed as not planned (the grey state icon).
    pub fn not_planned(&self) -> bool {
        !self.open && self.state_reason.as_deref() == Some("not_planned")
    }

    pub fn assigned_to(&self, login: &str) -> bool {
        self.assignees.iter().any(|a| a.eq_ignore_ascii_case(login))
    }
}

/// The header's state filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IssueFilter {
    #[default]
    Open,
    Closed,
    /// Open issues assigned to the signed-in account.
    AssignedToMe,
}

impl IssueFilter {
    pub const ALL: [IssueFilter; 3] = [Self::Open, Self::Closed, Self::AssignedToMe];

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
            Self::AssignedToMe => "Assigned to me",
        }
    }
}

/// A branch created from an issue, linked once the branch exists
/// ([`Dispatcher::create_branch`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingIssueLink {
    pub number: u64,
    pub html_url: String,
    pub node_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct IssuesViewState {
    /// Every open issue, newest first.
    pub open: Vec<IssueRow>,
    /// The newest-updated closed issues.
    pub closed: Vec<IssueRow>,
    pub filter: IssueFilter,
    /// Only issues carrying this label.
    pub label_filter: Option<String>,
    /// The selected issue's number.
    pub selected: Option<u64>,
    pub loading: bool,
    pub loaded: bool,
    pub reload_pending: bool,
    /// The last load failed (its message); the rows are the previous ones.
    pub error: Option<String>,
    /// No account for the repository's endpoint.
    pub signed_out: bool,
    /// The repository's labels and assignable users (New Issue…).
    pub labels: Vec<IssueLabel>,
    pub assignees: Vec<String>,
    pub meta_loading: bool,
    pub meta_loaded: bool,
    /// History's selection when the list opened, restored on close.
    pub saved_selection: Vec<String>,
}

impl IssuesViewState {
    /// The rows the state filter picks from.
    pub fn rows(&self) -> &[IssueRow] {
        match self.filter {
            IssueFilter::Open | IssueFilter::AssignedToMe => &self.open,
            IssueFilter::Closed => &self.closed,
        }
    }

    /// The rows after the state filter, the label filter and the typed
    /// filter (`crate::filter::match_keys` on `#N title`, the author and
    /// the labels).
    pub fn visible(&self, own_login: Option<&str>, query: &str) -> Vec<&IssueRow> {
        self.rows()
            .iter()
            .filter(|row| match self.filter {
                IssueFilter::AssignedToMe => own_login.is_some_and(|me| row.assigned_to(me)),
                _ => true,
            })
            .filter(|row| {
                self.label_filter
                    .as_ref()
                    .is_none_or(|l| row.labels.iter().any(|x| &x.name == l))
            })
            .filter(|row| {
                let query = query.trim();
                if query.is_empty() {
                    return true;
                }
                // `match_keys` reads a title and a subtitle key: the author
                // and the labels share the second one
                let mut subtitle = row.author.clone();
                for label in &row.labels {
                    subtitle.push(' ');
                    subtitle.push_str(&label.name);
                }
                let keys = [format!("#{} {}", row.number, row.title), subtitle];
                crate::filter::match_keys(query, &keys).is_some()
            })
            .collect()
    }

    pub fn find(&self, number: u64) -> Option<&IssueRow> {
        self.open
            .iter()
            .chain(self.closed.iter())
            .find(|r| r.number == number)
    }

    pub fn selected_row(&self) -> Option<&IssueRow> {
        self.find(self.selected?)
    }

    /// The labels the chips row offers: the repository's when read, else
    /// the ones the listed issues carry, each once, by name.
    pub fn chip_labels(&self) -> Vec<IssueLabel> {
        let mut out: Vec<IssueLabel> = if self.meta_loaded && !self.labels.is_empty() {
            self.labels.clone()
        } else {
            let mut seen = std::collections::HashSet::new();
            self.rows()
                .iter()
                .flat_map(|r| r.labels.iter())
                .filter(|l| seen.insert(l.name.clone()))
                .cloned()
                .collect()
        };
        out.sort_by_key(|l| l.name.to_lowercase());
        out
    }
}

/// The open list of the selected repository, `None` while the flag is off.
pub fn issues_of<'a>(s: &AppState, rs: &'a RepositoryState) -> Option<&'a IssuesViewState> {
    if !s.flags.bool(crate::flags::ids::ISSUES) {
        return None;
    }
    rs.issues.as_ref()
}

/// The branch name GitHub gives a branch made from an issue:
/// `<number>-<title>` with the title lowercased, every run of characters
/// that is not a letter or digit turned into one `-`, and the whole cut
/// at 60 characters (without a trailing `-`). A title without letters or
/// digits leaves the number alone.
pub fn issue_branch_name(number: u64, title: &str) -> String {
    const MAX: usize = 60;
    let mut slug = String::new();
    let mut dash = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            dash = false;
            slug.extend(ch.to_lowercase());
        } else if !dash && !slug.is_empty() {
            dash = true;
            slug.push('-');
        }
    }
    let mut name = number.to_string();
    let slug = slug.trim_end_matches('-');
    if !slug.is_empty() {
        name.push('-');
        name.push_str(slug);
    }
    if name.chars().count() > MAX {
        name = name.chars().take(MAX).collect();
    }
    name.trim_end_matches('-').to_string()
}

/// `?body=Closes #N` (or `&body=`) on a compare / new pull request URL.
pub fn with_closes_issue(url: &str, number: u64) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}body=Closes%20%23{number}")
}

struct Loaded {
    open: Vec<IssueRow>,
    closed: Vec<IssueRow>,
}

impl Dispatcher {
    /// Repository › Issues…: History lists the issues.
    pub fn show_issues(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::ISSUES)
        {
            return;
        }
        Self::show_section(id, Section::History, cx);
        let opened = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.issues.as_ref().is_some_and(|i| i.loaded || i.loading) {
                return false;
            }
            let saved = rs.selected_commits.clone();
            let issues = rs.issues.get_or_insert_with(Default::default);
            issues.saved_selection = saved;
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            true
        });
        if opened {
            Self::close_blame(id, cx);
            Self::close_recent_activity(id, cx);
            Self::close_releases(id, cx);
            Self::close_tags(id, cx);
            Self::load_issues(id, cx);
        }
    }

    /// The header's close button and Escape: back to History, its
    /// selection as it was.
    pub fn close_issues(id: u64, cx: &mut dyn Host) {
        let saved = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_states.get_mut(&id)?;
            let issues = rs.issues.take()?;
            if !issues.loaded && !issues.loading {
                // only the New Issue… metadata was cached, nothing shown
                return None;
            }
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            Some(issues.saved_selection)
        });
        if let Some(saved) = saved
            && !saved.is_empty()
        {
            Self::select_commits(id, saved, cx);
        }
    }

    /// The GitHub repository the issues belong to (a fork's parent while
    /// contributing upstream) and the API for it.
    fn issues_target(id: u64, cx: &dyn Host) -> Option<GitHubRepository> {
        Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.non_fork_github().cloned())
    }

    /// Read the lists (opened, refreshed). One load at a time; one asked
    /// for meanwhile runs after it.
    pub fn load_issues(id: u64, cx: &mut dyn Host) {
        let gh = Self::issues_target(id, cx);
        let api = gh
            .as_ref()
            .and_then(|gh| Self::api_for_repository(id, gh, cx));
        let (sso_hint, error_details) = {
            let s = Self::state(cx).read(cx);
            (
                s.flags.bool(crate::flags::ids::API_SAML_SSO_HINT),
                s.flags.bool(crate::flags::ids::API_ERROR_DETAILS),
            )
        };
        let start = Self::state(cx).update(cx, |s, cx| {
            let Some(issues) = s.repo_state_mut(id).issues.as_mut() else {
                return false;
            };
            if issues.loading {
                issues.reload_pending = true;
                return false;
            }
            issues.signed_out = api.is_none() && issues.open.is_empty() && issues.closed.is_empty();
            if api.is_none() {
                issues.loaded = true;
                cx.notify();
                return false;
            }
            issues.loading = true;
            issues.error = None;
            cx.notify();
            true
        });
        let (Some(gh), Some((endpoint, token, _))) = (gh, api.filter(|_| start)) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let client = Client::new(endpoint, token)
                    .with_sso_hint(sso_hint)
                    .with_error_details(error_details);
                let open =
                    client.issues(&gh.owner, &gh.name, corvene_github::IssueState::Open, None)?;
                let closed = client.closed_issues(&gh.owner, &gh.name, CLOSED_PAGES)?;
                corvene_github::Result::Ok(Loaded {
                    open: open
                        .into_iter()
                        .map(|i| IssueRow::from_api(i, &gh))
                        .collect(),
                    closed: closed
                        .into_iter()
                        .map(|i| IssueRow::from_api(i, &gh))
                        .collect(),
                })
            },
            move |result, cx| {
                let reload = Self::state(cx).update(cx, |s, cx| {
                    let Some(issues) = s.repo_state_mut(id).issues.as_mut() else {
                        return false;
                    };
                    issues.loading = false;
                    issues.loaded = true;
                    let reload = std::mem::take(&mut issues.reload_pending);
                    match result {
                        Ok(loaded) => {
                            issues.open = loaded.open;
                            issues.closed = loaded.closed;
                            issues.error = None;
                            if issues.selected.is_some_and(|n| issues.find(n).is_none()) {
                                issues.selected = None;
                            }
                        }
                        Err(err) => {
                            if err.is_token_invalidated() {
                                warn!(id, %err, "issues: token invalidated");
                            }
                            issues.error = Some(err.to_string());
                        }
                    }
                    cx.notify();
                    reload
                });
                if reload {
                    Self::load_issues(id, cx);
                }
            },
        );
    }

    /// Open / Closed / Assigned to me.
    pub fn set_issue_filter(id: u64, filter: IssueFilter, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(issues) = s.repo_state_mut(id).issues.as_mut()
                && issues.filter != filter
            {
                issues.filter = filter;
                cx.notify();
            }
        });
    }

    /// A label chip: only issues with that label (`None` for all).
    pub fn set_issue_label_filter(id: u64, label: Option<String>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(issues) = s.repo_state_mut(id).issues.as_mut()
                && issues.label_filter != label
            {
                issues.label_filter = label;
                cx.notify();
            }
        });
    }

    /// The selected issue, shown in the commit view's place.
    pub fn select_issue(id: u64, number: Option<u64>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(issues) = s.repo_state_mut(id).issues.as_mut()
                && issues.selected != number
            {
                issues.selected = number;
                cx.notify();
            }
        });
    }

    /// The repository's labels and assignable users for New Issue… (read
    /// once per session; kept on the view state, created without opening
    /// the list when needed).
    pub fn load_issue_meta(id: u64, cx: &mut dyn Host) {
        let Some(gh) = Self::issues_target(id, cx) else {
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for_repository(id, &gh, cx) else {
            return;
        };
        let start = Self::state(cx).update(cx, |s, cx| {
            let issues = s
                .repo_state_mut(id)
                .issues
                .get_or_insert_with(Default::default);
            if issues.meta_loading || issues.meta_loaded {
                return false;
            }
            issues.meta_loading = true;
            cx.notify();
            true
        });
        if !start {
            return;
        }
        spawn_bg(
            cx,
            move || {
                let client = Client::new(endpoint, token);
                let labels = client.labels(&gh.owner, &gh.name);
                let assignees = client.assignees(&gh.owner, &gh.name);
                (labels, assignees)
            },
            move |(labels, assignees), cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(issues) = s.repo_state_mut(id).issues.as_mut() else {
                        return;
                    };
                    issues.meta_loading = false;
                    issues.meta_loaded = true;
                    match labels {
                        Ok(labels) => {
                            issues.labels = labels.into_iter().map(IssueLabel::from).collect();
                            issues.labels.sort_by_key(|l| l.name.to_lowercase());
                        }
                        Err(err) => warn!(id, %err, "issues: labels"),
                    }
                    match assignees {
                        Ok(users) => {
                            issues.assignees = users.into_iter().map(|u| u.login).collect();
                            issues.assignees.sort_by_key(|l| l.to_lowercase());
                        }
                        Err(err) => warn!(id, %err, "issues: assignees"),
                    }
                    cx.notify();
                });
            },
        );
    }

    /// New Issue… › Create Issue: `POST /repos/{owner}/{name}/issues`, then
    /// the new issue is listed and selected (when the list is open) and a
    /// banner links to it.
    pub fn create_issue_via_api(id: u64, new: NewIssue, cx: &mut dyn Host) {
        let Some(gh) = Self::issues_target(id, cx) else {
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for_repository(id, &gh, cx) else {
            Self::show_error(
                "Could not create issue",
                format!("Sign in to {} first.", endpoint_host(&gh)),
                cx,
            );
            return;
        };
        let (sso_hint, error_details) = {
            let s = Self::state(cx).read(cx);
            (
                s.flags.bool(crate::flags::ids::API_SAML_SSO_HINT),
                s.flags.bool(crate::flags::ids::API_ERROR_DETAILS),
            )
        };
        Self::close_popup_if(|p| matches!(p, Popup::NewIssue { .. }), cx);
        let gh_for_row = gh.clone();
        spawn_bg(
            cx,
            move || {
                Client::new(endpoint, token)
                    .with_sso_hint(sso_hint)
                    .with_error_details(error_details)
                    .create_issue(&gh.owner, &gh.name, &new)
            },
            move |result, cx| match result {
                Ok(issue) => {
                    let row = IssueRow::from_api(issue, &gh_for_row);
                    info!(id, number = row.number, "issue created");
                    Self::set_banner(
                        Banner::IssueCreated {
                            number: row.number,
                            html_url: row.html_url.clone(),
                        },
                        cx,
                    );
                    Self::state(cx).update(cx, |s, cx| {
                        let rs = s.repo_state_mut(id);
                        if let Some(issues) = rs.issues.as_mut()
                            && issues.loaded
                        {
                            issues.open.retain(|r| r.number != row.number);
                            issues.open.insert(0, row.clone());
                            issues.selected = Some(row.number);
                            issues.filter = IssueFilter::Open;
                            issues.label_filter = None;
                            cx.notify();
                        }
                    });
                    // the `#` suggestions learn about it too
                    Self::refresh_issues(&gh_for_row, cx);
                }
                Err(err) => Self::show_error("Could not create issue", err.to_string(), cx),
            },
        );
    }

    /// Create Branch (the issue view, a row's menu): the Create Branch
    /// dialog prefilled with [`issue_branch_name`]; the branch is linked
    /// to the issue once it exists.
    pub fn create_branch_from_issue(id: u64, number: u64, cx: &mut dyn Host) {
        let row = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.issues.as_ref())
            .and_then(|issues| issues.find(number))
            .cloned();
        let Some(row) = row else {
            return;
        };
        // the dialog first (showing a Create Branch dialog forgets any
        // pending issue), then the issue it is for; the link is taken by
        // `create_branch` or dropped by the next Create Branch dialog
        Self::show_popup(
            Popup::CreateBranch {
                repo: id,
                target_sha: None,
                initial_name: issue_branch_name(row.number, &row.title),
            },
            cx,
        );
        Self::state(cx).update(cx, |s, _| {
            s.repo_state_mut(id).pending_issue_link = Some(PendingIssueLink {
                number: row.number,
                html_url: row.html_url.clone(),
                node_id: row.node_id.clone(),
            });
        });
    }

    /// A branch creation failed: forget the issue it was for.
    pub fn clear_pending_issue_link(id: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, _| {
            if let Some(rs) = s.repo_states.get_mut(&id) {
                rs.pending_issue_link = None;
            }
        });
    }

    /// After [`Dispatcher::create_branch`] made `branch` for the pending
    /// issue: its description keeps the issue's URL and the issue's
    /// number and node id are remembered for the push-time link.
    pub(crate) fn record_issue_branch(id: u64, branch: &str, cx: &mut dyn Host) {
        let link = Self::state(cx).update(cx, |s, _| {
            s.repo_states
                .get_mut(&id)
                .and_then(|rs| rs.pending_issue_link.take())
        });
        let Some(link) = link else {
            return;
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let branch = branch.to_string();
        spawn_bg(
            cx,
            move || {
                let set = |key: &str, value: &str| {
                    corvene_git::set_local_config_value(
                        git.clone(),
                        &workdir,
                        &format!("branch.{branch}.{key}"),
                        value,
                    )
                };
                set("description", &link.html_url)?;
                set(BRANCH_ISSUE_KEY, &link.number.to_string())?;
                if let Some(node) = &link.node_id {
                    set(BRANCH_ISSUE_NODE_KEY, node)?;
                }
                corvene_git::error::Result::Ok(())
            },
            move |result, _| {
                if let Err(err) = result {
                    warn!(id, %err, "could not record the branch's issue");
                }
            },
        );
    }

    /// After a push of `branch`: when it was made from an issue and is
    /// not linked on GitHub yet, `createLinkedBranch` lists it under the
    /// issue's Development section. Silent when the host or the account
    /// cannot (the local link stays).
    pub(crate) fn link_pushed_branch_to_issue(id: u64, branch: String, cx: &mut dyn Host) {
        let Some(gh) = Self::issues_target(id, cx) else {
            return;
        };
        let Some(repo_node) = gh.node_id.clone() else {
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for_repository(id, &gh, cx) else {
            return;
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let tip = {
            let s = Self::state(cx).read(cx);
            s.repo_states
                .get(&id)
                .and_then(|rs| rs.info.as_ref())
                .and_then(|info| info.branches.iter().find(|b| b.name == branch))
                .and_then(|b| b.tip.clone())
        };
        let Some(tip) = tip else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let get = |key: &str| {
                    corvene_git::local_config_value(
                        git.clone(),
                        &workdir,
                        &format!("branch.{branch}.{key}"),
                    )
                };
                if get(BRANCH_ISSUE_KEY).is_none() || get(BRANCH_ISSUE_LINKED_KEY).is_some() {
                    return None;
                }
                let issue_node = get(BRANCH_ISSUE_NODE_KEY)?;
                let linked = Client::new(endpoint, token)
                    .create_linked_branch(&issue_node, &repo_node, &tip, &branch)
                    .map_err(|e| e.to_string());
                if linked == Ok(true)
                    && let Err(err) = corvene_git::set_local_config_value(
                        git,
                        &workdir,
                        &format!("branch.{branch}.{BRANCH_ISSUE_LINKED_KEY}"),
                        "1",
                    )
                {
                    warn!(%err, "could not record the issue link");
                }
                Some((branch, linked))
            },
            move |result, _| match result {
                Some((branch, Ok(true))) => info!(id, %branch, "branch linked to its issue"),
                Some((branch, Ok(false))) => {
                    warn!(id, %branch, "GitHub did not link the branch to its issue")
                }
                Some((branch, Err(err))) => {
                    warn!(id, %branch, %err, "could not link the branch to its issue")
                }
                None => {}
            },
        );
    }

    /// The issue number the current branch was made from, when it was
    /// (`branch.<name>.corvene-issue`), read on the calling thread.
    pub(crate) fn current_branch_issue(id: u64, cx: &dyn Host) -> Option<u64> {
        let s = Self::state(cx).read(cx);
        if !s.flags.bool(crate::flags::ids::ISSUES) {
            return None;
        }
        let branch = s
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .and_then(|info| info.current_branch())
            .map(|b| b.name.clone())?;
        let (git, workdir) = Self::repo_context(id, cx)?;
        corvene_git::local_config_value(
            git,
            &workdir,
            &format!("branch.{branch}.{BRANCH_ISSUE_KEY}"),
        )
        .and_then(|v| v.trim().parse().ok())
    }

    pub fn open_issue_on_github(id: u64, number: u64, cx: &mut dyn Host) {
        let url = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.issues.as_ref())
            .and_then(|i| i.find(number))
            .map(|r| r.html_url.clone());
        if let Some(url) = url {
            Self::open_url(&url, cx);
        }
    }
}

/// `CORVENE_POPUP=issues`: sample issues in the view, no API.
pub fn install_samples(id: u64, cx: &mut dyn Host) {
    let gh = crate::samples::github_repository(id, cx);
    let label = |name: &str, color: &str| IssueLabel {
        name: name.to_string(),
        color: color.to_string(),
        description: None,
    };
    let row =
        |number: u64, title: &str, open: bool, ago: u64, labels: Vec<IssueLabel>, me: bool| {
            IssueRow {
                number,
                title: title.to_string(),
                open,
                state_reason: (!open).then(|| "completed".to_string()),
                body: format!(
                    "### Steps\n\n1. Open the app\n2. Watch it\n\nSee #{} for the history.",
                    number.saturating_sub(1).max(1)
                ),
                html_url: format!("{}/issues/{number}", gh.html_url),
                node_id: None,
                author: "octocat".to_string(),
                created_at: Some(crate::samples::iso_ago(ago)),
                updated_at: crate::samples::iso_ago(ago / 2),
                labels,
                assignees: if me {
                    vec!["me".to_string()]
                } else {
                    Vec::new()
                },
                comments: number % 4,
            }
        };
    let open = vec![
        row(
            42,
            "Crash on launch with an empty repository list",
            true,
            3_600 * 5,
            vec![label("bug", "d73a4a")],
            true,
        ),
        row(
            41,
            "Dark mode for the tutorial panel",
            true,
            86_400 * 2,
            vec![
                label("enhancement", "a2eeef"),
                label("good first issue", "7057ff"),
            ],
            false,
        ),
        row(
            38,
            "Document the release process",
            true,
            86_400 * 9,
            vec![label("documentation", "0075ca")],
            false,
        ),
    ];
    let closed = vec![row(
        37,
        "Typo in the welcome screen",
        false,
        86_400 * 30,
        vec![label("bug", "d73a4a")],
        false,
    )];
    install_rows(id, open, closed, cx);
}

fn install_rows(id: u64, open: Vec<IssueRow>, closed: Vec<IssueRow>, cx: &mut dyn Host) {
    Dispatcher::state(cx).update(cx, |s, cx| {
        let rs = s.repo_state_mut(id);
        let issues = rs.issues.get_or_insert_with(Default::default);
        issues.labels = open
            .iter()
            .chain(closed.iter())
            .flat_map(|r| r.labels.iter().cloned())
            .fold(Vec::new(), |mut acc: Vec<IssueLabel>, l| {
                if !acc.iter().any(|x| x.name == l.name) {
                    acc.push(l);
                }
                acc
            });
        issues.assignees = vec!["me".to_string(), "octocat".to_string()];
        issues.meta_loaded = true;
        issues.open = open;
        issues.closed = closed;
        issues.loaded = false;
        issues.loading = false;
        issues.signed_out = false;
        cx.notify();
    });
}

/// The host name for "sign in to …" messages.
fn endpoint_host(gh: &GitHubRepository) -> String {
    corvene_github::Endpoint::from_api_base(&gh.endpoint)
        .host()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_names_follow_github() {
        assert_eq!(
            issue_branch_name(101, "Crash on launch"),
            "101-crash-on-launch"
        );
        assert_eq!(
            issue_branch_name(7, "  Fix: the \"thing\" (again)! "),
            "7-fix-the-thing-again"
        );
        assert_eq!(issue_branch_name(12, "Ünïcödé façade"), "12-ünïcödé-façade");
        assert_eq!(issue_branch_name(3, "???"), "3");
        assert_eq!(issue_branch_name(3, ""), "3");
        let long = issue_branch_name(
            1234,
            "A very long issue title that keeps going on and on and on past the limit",
        );
        assert!(long.chars().count() <= 60, "{long}");
        assert!(long.chars().count() >= 55, "{long}");
        assert!(!long.ends_with('-'), "{long}");
        assert_eq!(
            issue_branch_name(5, "exactly sixty characters long title words here x-y-"),
            "5-exactly-sixty-characters-long-title-words-here-x-y"
        );
    }

    fn row(number: u64, title: &str, open: bool, labels: &[&str], assignees: &[&str]) -> IssueRow {
        IssueRow {
            number,
            title: title.to_string(),
            open,
            state_reason: None,
            body: String::new(),
            html_url: format!("https://github.com/o/r/issues/{number}"),
            node_id: None,
            author: "octocat".to_string(),
            created_at: None,
            updated_at: String::new(),
            labels: labels
                .iter()
                .map(|l| IssueLabel {
                    name: l.to_string(),
                    color: "ff0000".to_string(),
                    description: None,
                })
                .collect(),
            assignees: assignees.iter().map(|a| a.to_string()).collect(),
            comments: 0,
        }
    }

    #[test]
    fn filters_rows() {
        let mut state = IssuesViewState {
            open: vec![
                row(1, "Crash on launch", true, &["bug"], &["me"]),
                row(2, "Add dark mode", true, &["enhancement"], &[]),
            ],
            closed: vec![row(3, "Old bug", false, &["bug"], &["me"])],
            ..Default::default()
        };
        let numbers = |rows: Vec<&IssueRow>| rows.iter().map(|r| r.number).collect::<Vec<_>>();
        assert_eq!(numbers(state.visible(None, "")), vec![1, 2]);
        assert_eq!(numbers(state.visible(None, "dark")), vec![2]);
        assert_eq!(numbers(state.visible(None, "#1")), vec![1]);
        assert_eq!(numbers(state.visible(None, "bug")), vec![1]);
        state.label_filter = Some("bug".to_string());
        assert_eq!(numbers(state.visible(None, "")), vec![1]);
        state.label_filter = None;
        state.filter = IssueFilter::AssignedToMe;
        assert_eq!(numbers(state.visible(Some("ME"), "")), vec![1]);
        assert_eq!(numbers(state.visible(None, "")), Vec::<u64>::new());
        state.filter = IssueFilter::Closed;
        assert_eq!(numbers(state.visible(None, "")), vec![3]);
        assert_eq!(state.chip_labels().len(), 1);
        state.filter = IssueFilter::Open;
        assert_eq!(
            state
                .chip_labels()
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>(),
            vec!["bug", "enhancement"]
        );
    }

    #[test]
    fn closes_parameter() {
        assert_eq!(
            with_closes_issue("https://github.com/o/r/pull/new/feat", 7),
            "https://github.com/o/r/pull/new/feat?body=Closes%20%237"
        );
        assert_eq!(
            with_closes_issue("https://github.com/o/r/compare/feat?expand=1", 7),
            "https://github.com/o/r/compare/feat?expand=1&body=Closes%20%237"
        );
    }
}
