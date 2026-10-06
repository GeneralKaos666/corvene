//! Corvene `1223-checkout-from-fork` (desktop/desktop#17939): Branch ›
//! Check Out from Fork… checks out a branch of another fork of the
//! repository, named by its owner (`alice`), `owner/name`, `owner:branch` or
//! a URL. GitHub Desktop only reaches fork branches through their pull
//! requests (`app-store.ts#_findPullRequestBranch`, a hidden, pruned
//! `github-desktop-<owner>` remote and a `pr/<n>` branch).
//!
//! The fork is looked up on GitHub (`GET /repos/{owner}/{name}`, then the
//! network's forks for one under another name); elsewhere its URL is the
//! default remote's with the owner swapped. Its branches come from `git
//! ls-remote --heads`, so any host works. Checking one out reuses a remote
//! whose URL matches the fork, else adds one named after the owner (kept
//! until removed in Repository Settings › Remotes, `1109-remote-manager`),
//! fetches only that branch and creates `<owner>/<branch>` tracking it. The
//! owner suggestions are the network's newest forks and the owners of open
//! pull requests from forks.

use corvene_models::{
    Branch, BranchKind, GitHubRepository, Remote, clone_url_like_remote, split_remote,
    url_matches_remote,
};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// What the owner field names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForkInput {
    pub owner: String,
    /// The fork's repository name when given (`owner/name`).
    pub name: Option<String>,
    /// A URL given directly.
    pub url: Option<String>,
    /// A branch given with it (`owner:branch`, `…/tree/<branch>`).
    pub branch: Option<String>,
}

/// A GitHub owner or `owner/name` part: letters, digits, `-`, `_`, `.`.
fn plain_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Path segments that start a web page under `owner/name` rather than name
/// the repository.
const WEB_PAGES: &[&str] = &[
    "-",
    "tree",
    "pull",
    "pulls",
    "blob",
    "commit",
    "commits",
    "issues",
    "compare",
    "branches",
    "actions",
    "releases",
    "merge_requests",
];

fn looks_like_url(s: &str) -> bool {
    s.contains("://")
        || s.starts_with("git@")
        || s.split_once(':')
            .is_some_and(|(prefix, _)| prefix.contains('@') && !prefix.contains('/'))
}

/// Parse the owner field: `alice`, `alice/name`, `alice:branch`,
/// `alice/name:branch`, or a clone or web URL (a GitHub-style
/// `…/owner/name/tree/<branch>` names the branch too).
pub fn parse_fork_input(input: &str) -> Option<ForkInput> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    if looks_like_url(input) {
        let (_, path) = split_remote(input)?;
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        // a web page's path after `owner/name` (`/tree/<branch>`, `/pull/7`,
        // GitLab's `/-/…`)
        let page = segments
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, s)| WEB_PAGES.contains(s))
            .map(|(ix, _)| ix);
        let repo_segments = &segments[..page.unwrap_or(segments.len())];
        if repo_segments.len() < 2 {
            return None;
        }
        let owner = repo_segments[..repo_segments.len() - 1].join("/");
        let branch = page
            .and_then(|ix| {
                let rest = &segments[ix..];
                // GitLab's `/-/tree/<branch>`
                let rest = rest.strip_prefix(&["-"][..]).unwrap_or(rest);
                rest.strip_prefix(&["tree"][..]).map(|b| b.join("/"))
            })
            .filter(|b| !b.is_empty());
        let url = match page {
            Some(_) => {
                let repo_path = repo_segments.join("/");
                let prefix = &input[..input.find(&repo_path)?];
                format!("{prefix}{repo_path}")
            }
            None => input.trim_end_matches('/').to_string(),
        };
        return Some(ForkInput {
            owner,
            name: repo_segments
                .last()
                .map(|n| n.trim_end_matches(".git").to_string()),
            url: Some(url),
            branch,
        });
    }
    let (repo, branch) = match input.split_once(':') {
        Some((repo, branch)) => (
            repo,
            Some(branch.trim().to_string()).filter(|b| !b.is_empty()),
        ),
        None => (input, None),
    };
    let (owner, name) = match repo.split_once('/') {
        Some((owner, name)) => (owner, Some(name)),
        None => (repo, None),
    };
    if !plain_name(owner) || name.is_some_and(|n| !plain_name(n)) {
        return None;
    }
    Some(ForkInput {
        owner: owner.to_string(),
        name: name.map(str::to_string),
        url: None,
        branch,
    })
}

/// `url` (a clone URL of `…/<namespace>/<name>`) with `owner` in place of the
/// namespace and `name` in place of the repository name when given.
pub fn url_with_owner(url: &str, owner: &str, name: Option<&str>) -> Option<String> {
    let (_, path) = split_remote(url)?;
    let trimmed = path.trim_end_matches('/');
    let (namespace, old_name) = trimmed.rsplit_once('/')?;
    let git_suffix = old_name.ends_with(".git");
    let new_name = match name {
        Some(n) if git_suffix && !n.ends_with(".git") => format!("{n}.git"),
        Some(n) => n.to_string(),
        None => old_name.to_string(),
    };
    let start = url.rfind(trimmed)?;
    let namespace_start = start + trimmed.len() - old_name.len() - 1 - namespace.len();
    Some(format!(
        "{}{owner}/{new_name}{}",
        &url[..namespace_start],
        &url[start + trimmed.len()..]
    ))
}

/// The name of the remote added for `owner`'s fork: the owner (a GitLab
/// namespace's `/` as `-`), else `<owner>-fork`, `<owner>-fork-2`, … when a
/// remote of that name points elsewhere.
pub fn fork_remote_name(owner: &str, remotes: &[Remote]) -> String {
    let base = owner.replace('/', "-");
    let base = if corvene_git::remote_name_is_valid(&base) {
        base
    } else {
        "fork".to_string()
    };
    let taken = |n: &str| remotes.iter().any(|r| r.name == n);
    if !taken(&base) {
        return base;
    }
    let fork = format!("{base}-fork");
    if !taken(&fork) {
        return fork;
    }
    (2..)
        .map(|n| format!("{fork}-{n}"))
        .find(|n| !taken(n))
        .unwrap_or(fork)
}

/// The local branch for `owner`'s `branch`: `<owner>/<branch>`, with `-2`,
/// `-3`, … when a local branch of that name tracks something else.
pub fn fork_branch_name(owner: &str, branch: &str, local: &[&str]) -> String {
    // `alice` blocks `alice/fix` (and `alice/fix/x` blocks `alice/fix`): a
    // ref can't also be a directory of refs
    let clashes = |name: &str| {
        local.iter().any(|l| {
            *l == name
                || name
                    .strip_prefix(*l)
                    .is_some_and(|rest| rest.starts_with('/'))
                || l.strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    };
    let owner = owner.replace('/', "-");
    let base = [format!("{owner}/{branch}"), format!("{owner}-{branch}")]
        .into_iter()
        .find(|n| !clashes(n))
        .unwrap_or_else(|| format!("{owner}-{branch}"));
    if !clashes(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|n| !clashes(n))
        .unwrap_or(base)
}

/// An owner offered under the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkSuggestion {
    pub owner: String,
    /// `owner/name`.
    pub full_name: String,
    /// "Pull request #7" or "Fork".
    pub detail: String,
}

/// The fork found for the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkSource {
    pub owner: String,
    /// `owner/name`, or the URL.
    pub label: String,
    pub url: String,
    /// Its branches (`ls-remote --heads`).
    pub branches: Vec<String>,
    /// The branch the input named, when the fork has it.
    pub preselect: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ForkLookup {
    #[default]
    Idle,
    Loading,
    Found(ForkSource),
    Failed(String),
}

/// `RepositoryState.fork_checkout`: the Check Out from Fork dialog's data.
#[derive(Clone, Debug, Default)]
pub struct ForkCheckoutState {
    pub suggestions: Vec<ForkSuggestion>,
    pub suggestions_loading: bool,
    pub lookup: ForkLookup,
    /// The field's text the lookup is for.
    pub looked_up: String,
    pub checking_out: bool,
    pub error: Option<String>,
    generation: u64,
}

/// What the background lookup needs.
struct LookupContext {
    input: ForkInput,
    /// The default remote's URL.
    base_url: Option<String>,
    github: Option<GitHubRepository>,
    api: Option<(corvene_github::Endpoint, String)>,
    network: Option<GitHubRepository>,
    keep_ssh: bool,
}

fn resolve_url(cx: &LookupContext) -> Result<(String, String), String> {
    let input = &cx.input;
    if let Some(url) = &input.url {
        return Ok((url.clone(), url.clone()));
    }
    let like = |url: String| match (&cx.base_url, cx.keep_ssh) {
        (Some(base), true) => clone_url_like_remote(&url, base),
        _ => url,
    };
    if let (Some(gh), Some((endpoint, token))) = (&cx.github, &cx.api) {
        let client = corvene_github::Client::new(endpoint.clone(), token.clone());
        let name = input.name.clone().unwrap_or_else(|| gh.name.clone());
        match client.repository_clone_info(&input.owner, &name, false) {
            Ok(Some(info)) => return Ok((like(info.url), format!("{}/{name}", input.owner))),
            Ok(None) => {}
            Err(err) => return Err(format!("Could not look up {}/{name}: {err}", input.owner)),
        }
        // a fork under another name: the network's forks
        if input.name.is_none() {
            let root = cx.network.as_ref().unwrap_or(gh);
            match client.forks(&root.owner, &root.name, 3) {
                Ok(forks) => {
                    if let Some(fork) = forks
                        .iter()
                        .find(|f| f.owner.eq_ignore_ascii_case(&input.owner))
                    {
                        return Ok((
                            like(fork.clone_url.clone()),
                            format!("{}/{}", fork.owner, fork.name),
                        ));
                    }
                }
                Err(err) => warn!(%err, "could not list forks"),
            }
        }
        return Err(format!(
            "{} has no repository named {name}. Type owner/name when their fork has another \
             name, or paste its URL.",
            input.owner
        ));
    }
    let base = cx
        .base_url
        .as_deref()
        .ok_or("This repository has no remote to find the fork from. Paste the fork's URL.")?;
    let url = url_with_owner(base, &input.owner, input.name.as_deref())
        .ok_or("Could not make the fork's URL from this repository's remote. Paste its URL.")?;
    let label = match &input.name {
        Some(name) => format!("{}/{name}", input.owner),
        None => url.clone(),
    };
    Ok((url, label))
}

impl Dispatcher {
    /// Branch › Check Out from Fork…, with the field set to `owner` (and
    /// `branch` picked once the fork's branches load).
    pub fn show_checkout_from_fork(
        id: u64,
        owner: Option<String>,
        branch: Option<String>,
        cx: &mut dyn Host,
    ) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::CHECKOUT_FROM_FORK)
        {
            return;
        }
        Self::close_foldout(cx);
        Self::state(cx).update(cx, |s, cx| {
            let generation = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.fork_checkout.as_ref())
                .map_or(0, |f| f.generation + 1);
            s.repo_state_mut(id).fork_checkout = Some(ForkCheckoutState {
                generation,
                ..Default::default()
            });
            cx.notify();
        });
        let prefill = match (&owner, &branch) {
            (Some(owner), Some(branch)) => format!("{owner}:{branch}"),
            (Some(owner), None) => owner.clone(),
            _ => String::new(),
        };
        Self::show_popup(
            Popup::CheckoutFromFork {
                repo: id,
                input: prefill.clone(),
            },
            cx,
        );
        Self::load_fork_suggestions(id, cx);
        if !prefill.is_empty() {
            Self::look_up_fork(id, prefill, cx);
        }
    }

    fn update_fork_checkout(
        id: u64,
        generation: Option<u64>,
        cx: &mut dyn Host,
        f: impl FnOnce(&mut ForkCheckoutState),
    ) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(state) = s
                .repo_states
                .get_mut(&id)
                .and_then(|rs| rs.fork_checkout.as_mut())
                .filter(|st| generation.is_none_or(|g| st.generation == g))
            {
                f(state);
                cx.notify();
            }
        });
    }

    /// The owners offered: those of open pull requests from forks, then
    /// the network's newest forks (GitHub only).
    fn load_fork_suggestions(id: u64, cx: &mut dyn Host) {
        let (own_owner, pr_owners, network, api, generation) = {
            let s = Self::state(cx).read(cx);
            let github = s.repository(id).and_then(|r| r.github.clone());
            let network = s.pull_request_repository(id).or_else(|| github.clone());
            let mut pr_owners: Vec<ForkSuggestion> = Vec::new();
            if let Some(cache) = network
                .as_ref()
                .and_then(|n| s.pull_requests.get(&crate::pull_requests::cache_key(n)))
            {
                for pr in &cache.pull_requests {
                    let Some(head) = &pr.head.repository else {
                        continue;
                    };
                    if network
                        .as_ref()
                        .is_some_and(|n| n.owner.eq_ignore_ascii_case(&head.owner))
                        || pr_owners.iter().any(|p| p.owner == head.owner)
                    {
                        continue;
                    }
                    pr_owners.push(ForkSuggestion {
                        owner: head.owner.clone(),
                        full_name: format!("{}/{}", head.owner, head.name),
                        detail: format!("Pull request #{}", pr.number),
                    });
                }
            }
            let api = github
                .as_ref()
                .and_then(|gh| Self::api_for_repository(id, gh, cx))
                .map(|(endpoint, token, _)| (endpoint, token));
            let generation = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.fork_checkout.as_ref())
                .map(|f| f.generation);
            (
                github.map(|g| g.owner),
                pr_owners,
                network.filter(|_| s.repository(id).is_some_and(|r| r.github.is_some())),
                api,
                generation,
            )
        };
        let Some(generation) = generation else {
            return;
        };
        let own = own_owner.clone();
        Self::update_fork_checkout(id, Some(generation), cx, |st| {
            st.suggestions = pr_owners
                .iter()
                .filter(|p| own.as_deref() != Some(p.owner.as_str()))
                .cloned()
                .collect();
            st.suggestions_loading = network.is_some() && api.is_some();
        });
        let (Some(network), Some((endpoint, token))) = (network, api) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                corvene_github::Client::new(endpoint, token).forks(&network.owner, &network.name, 1)
            },
            move |result, cx| {
                Self::update_fork_checkout(id, Some(generation), cx, |st| {
                    st.suggestions_loading = false;
                    match result {
                        Ok(forks) => {
                            for fork in forks {
                                if own_owner.as_deref() == Some(fork.owner.as_str())
                                    || st.suggestions.iter().any(|s| s.owner == fork.owner)
                                {
                                    continue;
                                }
                                st.suggestions.push(ForkSuggestion {
                                    full_name: format!("{}/{}", fork.owner, fork.name),
                                    owner: fork.owner,
                                    detail: "Fork".to_string(),
                                });
                            }
                        }
                        Err(err) => warn!(%err, "could not list forks"),
                    }
                });
            },
        );
    }

    /// Find the fork the field names and list its branches.
    pub fn look_up_fork(id: u64, text: String, cx: &mut dyn Host) {
        let Some(input) = parse_fork_input(&text) else {
            Self::update_fork_checkout(id, None, cx, |st| {
                st.looked_up = text.clone();
                st.lookup = ForkLookup::Failed(
                    "Type the fork's owner (alice), owner/name, owner:branch or its URL."
                        .to_string(),
                );
            });
            return;
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (lookup, generation) = {
            let s = Self::state(cx).read(cx);
            let info = s.repo_states.get(&id).and_then(|rs| rs.info.as_ref());
            let base_url = info
                .and_then(|i| corvene_git::find_default_remote(&i.remotes))
                .map(|r| r.url.clone());
            let github = s.repository(id).and_then(|r| r.github.clone());
            let api = github
                .as_ref()
                .and_then(|gh| Self::api_for_repository(id, gh, cx))
                .map(|(endpoint, token, _)| (endpoint, token));
            let network = github.as_ref().and_then(|gh| gh.parent.as_deref().cloned());
            let generation = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.fork_checkout.as_ref())
                .map(|f| f.generation);
            (
                LookupContext {
                    input,
                    base_url,
                    github,
                    api,
                    network,
                    keep_ssh: s.flags.bool(crate::flags::ids::FORK_REMOTES_KEEP_SSH),
                },
                generation,
            )
        };
        let Some(generation) = generation else {
            return;
        };
        Self::update_fork_checkout(id, Some(generation), cx, |st| {
            st.looked_up = text.clone();
            st.lookup = ForkLookup::Loading;
            st.error = None;
        });
        let askpass = Self::askpass_env_for_repository(id, cx);
        spawn_bg(
            cx,
            move || -> Result<ForkSource, String> {
                let (url, label) = resolve_url(&lookup)?;
                let branches = corvene_git::ls_remote_heads(git, &workdir, &url, askpass.as_ref())
                    .map_err(|err| format!("Could not list the branches of {label}: {err}"))?;
                let preselect = lookup.input.branch.clone().filter(|b| branches.contains(b));
                Ok(ForkSource {
                    owner: lookup.input.owner.clone(),
                    label,
                    url,
                    branches,
                    preselect,
                })
            },
            move |result, cx| {
                Self::update_fork_checkout(id, Some(generation), cx, |st| {
                    if st.looked_up != text {
                        return;
                    }
                    st.lookup = match result {
                        Ok(source) => ForkLookup::Found(source),
                        Err(message) => ForkLookup::Failed(message),
                    };
                });
            },
        );
    }

    /// Check out the found fork's `branch`: a remote for the fork (one with
    /// a matching URL, else a new one named after the owner), a fetch of
    /// that branch alone, and `<owner>/<branch>` tracking it (an existing
    /// local branch tracking it is reused).
    pub fn checkout_from_fork(id: u64, branch: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (source, remotes, branches) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let Some(ForkLookup::Found(source)) = rs
                .and_then(|rs| rs.fork_checkout.as_ref())
                .map(|f| f.lookup.clone())
            else {
                return;
            };
            let info = rs.and_then(|rs| rs.info.as_ref());
            (
                source,
                info.map(|i| i.remotes.clone()).unwrap_or_default(),
                info.map(|i| i.branches.clone()).unwrap_or_default(),
            )
        };
        if !source.branches.contains(&branch) {
            return;
        }
        Self::update_fork_checkout(id, None, cx, |st| {
            st.checking_out = true;
            st.error = None;
        });
        let askpass = Self::askpass_env_for_repository(id, cx);
        spawn_bg(
            cx,
            move || -> Result<Branch, String> {
                let remote = match remotes
                    .iter()
                    .find(|r| url_matches_remote(&source.url, &r.url))
                {
                    Some(remote) => remote.name.clone(),
                    None => {
                        let name = fork_remote_name(&source.owner, &remotes);
                        corvene_git::add_remote(git.clone(), &workdir, &name, &source.url)
                            .map_err(|err| format!("Could not add the remote {name}: {err}"))?;
                        name
                    }
                };
                corvene_git::fetch_remote_branch(
                    git.clone(),
                    &workdir,
                    &remote,
                    &branch,
                    askpass.as_ref(),
                )
                .map_err(|err| format!("Could not fetch {branch} from {remote}: {err}"))?;
                let upstream = format!("{remote}/{branch}");
                if let Some(local) = branches.iter().find(|b| {
                    b.kind == BranchKind::Local && b.upstream_short() == Some(upstream.as_str())
                }) {
                    return Ok(local.clone());
                }
                let local: Vec<&str> = branches
                    .iter()
                    .filter(|b| b.kind == BranchKind::Local)
                    .map(|b| b.name.as_str())
                    .collect();
                let name = fork_branch_name(&source.owner, &branch, &local);
                corvene_git::create_tracking_branch(git.clone(), &workdir, &name, &remote, &branch)
                    .map_err(|err| format!("Could not create the branch {name}: {err}"))?;
                corvene_git::open_repository(&workdir)
                    .ok()
                    .and_then(|info| {
                        info.branches
                            .into_iter()
                            .find(|b| b.kind == BranchKind::Local && b.name == name)
                    })
                    .ok_or_else(|| format!("Could not create the branch {name}."))
            },
            move |result, cx| match result {
                Ok(branch) => {
                    Self::state(cx).update(cx, |s, cx| {
                        let rs = s.repo_state_mut(id);
                        rs.fork_checkout = None;
                        if let Some(info) = rs.info.as_mut()
                            && !info
                                .branches
                                .iter()
                                .any(|b| b.name == branch.name && b.kind == branch.kind)
                        {
                            info.branches.push(branch.clone());
                        }
                        cx.notify();
                    });
                    Self::close_popup_if(|p| matches!(p, Popup::CheckoutFromFork { .. }), cx);
                    Self::checkout_branch(id, branch.name, None, cx);
                }
                Err(message) => Self::update_fork_checkout(id, None, cx, |st| {
                    st.checking_out = false;
                    st.error = Some(message);
                }),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(name: &str) -> Remote {
        Remote {
            name: name.into(),
            url: format!("https://github.com/{name}/x.git"),
        }
    }

    #[test]
    fn parses_owner_forms() {
        let p = parse_fork_input(" alice ").unwrap();
        assert_eq!((p.owner.as_str(), p.name, p.branch), ("alice", None, None));
        let p = parse_fork_input("alice/other:fix/typo").unwrap();
        assert_eq!(p.owner, "alice");
        assert_eq!(p.name.as_deref(), Some("other"));
        assert_eq!(p.branch.as_deref(), Some("fix/typo"));
        assert!(parse_fork_input("al ice").is_none());
        assert!(parse_fork_input("").is_none());
    }

    #[test]
    fn parses_urls() {
        let p = parse_fork_input("https://github.com/alice/repo/tree/fix/typo").unwrap();
        assert_eq!(p.owner, "alice");
        assert_eq!(p.url.as_deref(), Some("https://github.com/alice/repo"));
        assert_eq!(p.branch.as_deref(), Some("fix/typo"));
        let p = parse_fork_input("https://github.com/alice/repo/pull/5").unwrap();
        assert_eq!(
            (p.owner.as_str(), p.name.as_deref()),
            ("alice", Some("repo"))
        );
        assert_eq!(p.url.as_deref(), Some("https://github.com/alice/repo"));
        assert_eq!(p.branch, None);
        let p = parse_fork_input("git@github.com:alice/repo.git").unwrap();
        assert_eq!(p.owner, "alice");
        assert_eq!(p.name.as_deref(), Some("repo"));
        assert_eq!(p.url.as_deref(), Some("git@github.com:alice/repo.git"));
        let p = parse_fork_input("https://gitlab.com/group/sub/repo/-/tree/main").unwrap();
        assert_eq!(p.owner, "group/sub");
        assert_eq!(p.url.as_deref(), Some("https://gitlab.com/group/sub/repo"));
        assert_eq!(p.branch.as_deref(), Some("main"));
    }

    #[test]
    fn swaps_the_owner_in_a_url() {
        assert_eq!(
            url_with_owner("https://github.com/octo/repo.git", "alice", None).as_deref(),
            Some("https://github.com/alice/repo.git")
        );
        assert_eq!(
            url_with_owner("git@github.com:octo/repo.git", "alice", Some("other")).as_deref(),
            Some("git@github.com:alice/other.git")
        );
        assert_eq!(
            url_with_owner("https://gitlab.com/group/sub/repo", "bob", None).as_deref(),
            Some("https://gitlab.com/bob/repo")
        );
    }

    #[test]
    fn names_remotes_and_branches() {
        assert_eq!(fork_remote_name("alice", &[remote("origin")]), "alice");
        assert_eq!(fork_remote_name("alice", &[remote("alice")]), "alice-fork");
        assert_eq!(
            fork_remote_name("alice", &[remote("alice"), remote("alice-fork")]),
            "alice-fork-2"
        );
        assert_eq!(fork_remote_name("group/sub", &[]), "group-sub");
        assert_eq!(fork_branch_name("alice", "fix", &["main"]), "alice/fix");
        assert_eq!(
            fork_branch_name("alice", "fix", &["alice/fix", "alice/fix-2"]),
            "alice-fix"
        );
        assert_eq!(fork_branch_name("alice", "fix", &["alice"]), "alice-fix");
        assert_eq!(
            fork_branch_name("alice", "fix", &["alice", "alice-fix"]),
            "alice-fix-2"
        );
    }
}
