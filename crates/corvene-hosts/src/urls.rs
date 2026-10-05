//! Web pages of a hosted repository, per host (no network).

use corvene_models::{GitHubRepository, HostEndpoint, HostKind};

use crate::http::encode;

/// A branch name in a URL path: each segment encoded, `/` kept.
fn branch_path(branch: &str) -> String {
    branch.split('/').map(encode).collect::<Vec<_>>().join("/")
}

/// The page of pull (merge) request `number`.
pub fn pull_request_url(kind: HostKind, repo: &GitHubRepository, number: u64) -> String {
    let base = &repo.html_url;
    match kind {
        HostKind::GitHub => format!("{base}/pull/{number}"),
        HostKind::GitLab => format!("{base}/-/merge_requests/{number}"),
        HostKind::Gitea => format!("{base}/pulls/{number}"),
        HostKind::Bitbucket => format!("{base}/pull-requests/{number}"),
    }
}

/// The host's form for a new pull (merge) request from `head` (on `fork`
/// when it is not `target` itself) into `base` (the default branch when
/// `None`).
pub fn new_pull_request_url(
    kind: HostKind,
    target: &GitHubRepository,
    fork: Option<&GitHubRepository>,
    head: &str,
    base: Option<&str>,
) -> String {
    let base = base.or(target.default_branch.as_deref());
    match kind {
        HostKind::GitHub => {
            let head = match fork {
                Some(fork) => format!("{}:{}", encode(&fork.owner), branch_path(head)),
                None => branch_path(head),
            };
            match base {
                Some(base) => format!(
                    "{}/compare/{}...{head}?expand=1",
                    target.html_url,
                    branch_path(base),
                ),
                None => format!("{}/pull/new/{head}", target.html_url),
            }
        }
        HostKind::GitLab => {
            // a fork's form is opened on the fork; it targets the parent
            let on = fork.unwrap_or(target);
            let mut url = format!(
                "{}/-/merge_requests/new?merge_request%5Bsource_branch%5D={}",
                on.html_url,
                encode(head)
            );
            if let Some(base) = base {
                url.push_str(&format!(
                    "&merge_request%5Btarget_branch%5D={}",
                    encode(base)
                ));
            }
            url
        }
        HostKind::Gitea => {
            let head = match fork {
                Some(fork) => format!("{}:{}", encode(&fork.owner), branch_path(head)),
                None => branch_path(head),
            };
            format!(
                "{}/compare/{}...{head}",
                target.html_url,
                branch_path(base.unwrap_or("main")),
            )
        }
        HostKind::Bitbucket => {
            let on = fork.unwrap_or(target);
            let mut url = format!(
                "{}/pull-requests/new?source={}&t=1",
                on.html_url,
                encode(head)
            );
            if let Some(base) = base.filter(|_| fork.is_none()) {
                url.push_str(&format!("&dest={}", encode(base)));
            }
            url
        }
    }
}

/// A commit's page.
pub fn commit_url(kind: HostKind, repo: &GitHubRepository, sha: &str) -> String {
    let base = &repo.html_url;
    match kind {
        HostKind::GitHub | HostKind::Gitea => format!("{base}/commit/{sha}"),
        HostKind::GitLab => format!("{base}/-/commit/{sha}"),
        HostKind::Bitbucket => format!("{base}/commits/{sha}"),
    }
}

/// A branch's page.
pub fn branch_url(kind: HostKind, repo: &GitHubRepository, branch: &str) -> String {
    let base = &repo.html_url;
    let branch = branch_path(branch);
    match kind {
        HostKind::GitHub => format!("{base}/tree/{branch}"),
        HostKind::GitLab => format!("{base}/-/tree/{branch}"),
        HostKind::Gitea => format!("{base}/src/branch/{branch}"),
        HostKind::Bitbucket => format!("{base}/branch/{branch}"),
    }
}

/// Branch › Compare on <host>: `branch` against the default branch.
pub fn compare_url(kind: HostKind, repo: &GitHubRepository, branch: &str) -> String {
    let base_branch = repo.default_branch.as_deref().unwrap_or("main");
    let base = &repo.html_url;
    match kind {
        HostKind::GitHub => format!("{base}/compare/{}", branch_path(branch)),
        HostKind::GitLab => format!(
            "{base}/-/compare/{}...{}",
            branch_path(base_branch),
            branch_path(branch)
        ),
        HostKind::Gitea => format!(
            "{base}/compare/{}...{}",
            branch_path(base_branch),
            branch_path(branch)
        ),
        // Bitbucket's compare page has no stable URL form: the branch page
        // shows its commits ahead and behind
        HostKind::Bitbucket => branch_url(kind, repo, branch),
    }
}

/// Repository › Create Issue on <host>.
pub fn new_issue_url(kind: HostKind, repo: &GitHubRepository) -> String {
    let base = &repo.html_url;
    match kind {
        HostKind::GitHub => format!("{base}/issues/new/choose"),
        HostKind::GitLab => format!("{base}/-/issues/new"),
        HostKind::Gitea | HostKind::Bitbucket => format!("{base}/issues/new"),
    }
}

/// Where the user creates the token the sign-in dialog asks for (GitLab's
/// form comes filled in with a name and the scopes Corvene needs).
pub fn token_settings_url(endpoint: &HostEndpoint) -> String {
    match endpoint.kind {
        HostKind::GitHub => endpoint.web("settings/tokens/new?scopes=repo,workflow,read:user,user:email&description=Corvene"),
        HostKind::GitLab => endpoint.web(
            "-/user_settings/personal_access_tokens?name=Corvene&scopes=api,read_user,write_repository",
        ),
        HostKind::Gitea => endpoint.web("user/settings/applications"),
        HostKind::Bitbucket => "https://id.atlassian.com/manage-profile/security/api-tokens".into(),
    }
}

/// Where an OAuth application for Corvene is registered.
pub fn oauth_app_settings_url(endpoint: &HostEndpoint) -> String {
    match endpoint.kind {
        HostKind::GitHub => endpoint.web("settings/applications/new"),
        HostKind::GitLab => endpoint.web("-/user_settings/applications"),
        HostKind::Gitea => endpoint.web("user/settings/applications"),
        HostKind::Bitbucket => endpoint.web("account/settings/"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(html: &str) -> GitHubRepository {
        GitHubRepository {
            endpoint: String::new(),
            owner: "o".into(),
            name: "n".into(),
            html_url: html.into(),
            clone_url: String::new(),
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

    #[test]
    fn builds_host_urls() {
        let gl = repo("https://gitlab.com/g/s/p");
        assert_eq!(
            new_pull_request_url(HostKind::GitLab, &gl, None, "feat/x", None),
            "https://gitlab.com/g/s/p/-/merge_requests/new?merge_request%5Bsource_branch%5D=feat%2Fx&merge_request%5Btarget_branch%5D=main"
        );
        assert_eq!(
            pull_request_url(HostKind::GitLab, &gl, 4),
            "https://gitlab.com/g/s/p/-/merge_requests/4"
        );
        assert_eq!(
            branch_url(HostKind::GitLab, &gl, "feat/x"),
            "https://gitlab.com/g/s/p/-/tree/feat/x"
        );
        let gt = repo("https://codeberg.org/o/n");
        let fork = GitHubRepository {
            owner: "me".into(),
            ..repo("https://codeberg.org/me/n")
        };
        assert_eq!(
            new_pull_request_url(HostKind::Gitea, &gt, Some(&fork), "fix", Some("dev")),
            "https://codeberg.org/o/n/compare/dev...me:fix"
        );
        assert_eq!(
            pull_request_url(HostKind::Gitea, &gt, 9),
            "https://codeberg.org/o/n/pulls/9"
        );
        let bb = repo("https://bitbucket.org/ws/r");
        assert_eq!(
            new_pull_request_url(HostKind::Bitbucket, &bb, None, "fix", None),
            "https://bitbucket.org/ws/r/pull-requests/new?source=fix&t=1&dest=main"
        );
        assert_eq!(
            commit_url(HostKind::Bitbucket, &bb, "abc"),
            "https://bitbucket.org/ws/r/commits/abc"
        );
        assert_eq!(
            compare_url(HostKind::GitLab, &gl, "x"),
            "https://gitlab.com/g/s/p/-/compare/main...x"
        );
        assert_eq!(
            new_issue_url(HostKind::GitLab, &gl),
            "https://gitlab.com/g/s/p/-/issues/new"
        );
    }
}
