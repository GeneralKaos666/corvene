//! GHD `IAvatarUser` and `getAvatarUsersForCommit` (`models/avatar.ts`,
//! `lib/web-flow-committer.ts`): the people a commit is attributed to, in
//! order: its author, its `Co-Authored-By` co-authors and its committer
//! when that is someone else (not the author, a co-author or GitHub's web
//! flow committer), each identity once.

use corvene_models::{Commit, GitHubRepository};

/// GHD `getDotComAPIEndpoint()`.
const DOTCOM_API_ENDPOINT: &str = "https://api.github.com";

/// GHD `IAvatarUser`: the minimum needed to show a user's avatar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarUser {
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    /// The endpoint of the repository the user is associated with (`None`
    /// for repositories not on GitHub).
    pub endpoint: Option<String>,
}

/// GHD `getAvatarUserFromAuthor`.
fn avatar_user(name: &str, email: &str, github: Option<&GitHubRepository>) -> AvatarUser {
    AvatarUser {
        name: name.to_string(),
        email: email.to_string(),
        avatar_url: None,
        endpoint: github.map(|g| g.endpoint.clone()),
    }
}

/// GHD `isWebFlowCommitter`: GitHub's web flow committed the commit (pull
/// request merges and edits on the website).
pub fn is_web_flow_committer(commit: &Commit, github: &GitHubRepository) -> bool {
    let (name, email) = (&commit.committer.name, &commit.committer.email);
    if github.endpoint == DOTCOM_API_ENDPOINT {
        name == "GitHub" && email == "noreply@github.com"
    } else {
        name == "GitHub Enterprise"
    }
}

/// GHD `getStealthEmailHostForEndpoint`.
fn stealth_email_host(endpoint: &str) -> String {
    if endpoint == DOTCOM_API_ENDPOINT || endpoint.is_empty() {
        return "users.noreply.github.com".to_string();
    }
    let host = endpoint
        .split_once("://")
        .map_or(endpoint, |(_, rest)| rest)
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    format!("users.noreply.{host}")
}

/// GHD `parseStealthEmail(email, endpoint)?.login`: the login of a no-reply
/// address of `endpoint` (`[<id>+]<login>@users.noreply.<host>`).
fn stealth_email_login(email: &str, endpoint: &str) -> Option<String> {
    let (local, host) = email.rsplit_once('@')?;
    if !host.eq_ignore_ascii_case(&stealth_email_host(endpoint)) {
        return None;
    }
    // `^(?:(\d+)\+)?(.+?)@…`
    let login = match local.split_once('+') {
        Some((id, login)) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => login,
        _ => local,
    };
    (!login.is_empty()).then(|| login.to_string())
}

/// GHD `getAvatarUsersForCommit`.
pub fn get_avatar_users_for_commit(
    github: Option<&GitHubRepository>,
    commit: &Commit,
) -> Vec<AvatarUser> {
    let co_authors = commit.co_authors();
    let mut users = vec![avatar_user(
        &commit.author.name,
        &commit.author.email,
        github,
    )];
    users.extend(
        co_authors
            .iter()
            .map(|a| avatar_user(&a.name, &a.email, github)),
    );
    let authored_by_committer = commit.author.name == commit.committer.name
        && commit.author.email == commit.committer.email;
    let co_authored_by_committer = co_authors
        .iter()
        .any(|a| a.name == commit.committer.name && a.email == commit.committer.email);
    let web_flow = github.is_some_and(|g| is_web_flow_committer(commit, g));
    if !authored_by_committer && !web_flow && !co_authored_by_committer {
        users.push(avatar_user(
            &commit.committer.name,
            &commit.committer.email,
            github,
        ));
    }
    // Copilot's agent commits as `copilot-swe-agent[bot]` with Copilot's
    // no-reply address; GitHub.com shows them as Copilot
    if let Some(github) = github {
        for user in &mut users {
            if user.name == "copilot-swe-agent[bot]"
                && stealth_email_login(&user.email, &github.endpoint).as_deref() == Some("Copilot")
            {
                user.name = "Copilot".to_string();
            }
        }
    }
    // `new Map(avatarUsers.map(x => [x.name + x.email, x]))`: one per
    // identity, at its first place, the last duplicate's values
    let mut unique: Vec<AvatarUser> = Vec::new();
    for user in users {
        match unique
            .iter_mut()
            .find(|u| format!("{}{}", u.name, u.email) == format!("{}{}", user.name, user.email))
        {
            Some(existing) => *existing = user,
            None => unique.push(user),
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::CommitIdentity;

    fn identity(name: &str, email: &str) -> CommitIdentity {
        CommitIdentity {
            name: name.into(),
            email: email.into(),
            seconds: 0,
            offset: 0,
        }
    }

    fn commit(author: CommitIdentity, committer: CommitIdentity, trailers: &[&str]) -> Commit {
        Commit {
            sha: "a".repeat(40),
            summary: "Fix".into(),
            body: String::new(),
            author,
            committer,
            parents: Vec::new(),
            trailers: trailers
                .iter()
                .map(|v| ("Co-Authored-By".to_string(), v.to_string()))
                .collect(),
            tags: Vec::new(),
        }
    }

    fn names(users: &[AvatarUser]) -> Vec<&str> {
        users.iter().map(|u| u.name.as_str()).collect()
    }

    #[test]
    fn author_co_authors_then_a_different_committer() {
        let c = commit(
            identity("Mona", "mona@example.com"),
            identity("Hubot", "hubot@example.com"),
            &["Desktop <desktop@example.com>", "Mona <mona@example.com>"],
        );
        assert_eq!(
            names(&get_avatar_users_for_commit(None, &c)),
            ["Mona", "Desktop", "Hubot"]
        );
    }

    #[test]
    fn the_web_flow_committer_is_left_out() {
        let github = GitHubRepository {
            endpoint: DOTCOM_API_ENDPOINT.into(),
            owner: "desktop".into(),
            name: "desktop".into(),
            html_url: "https://github.com/desktop/desktop".into(),
            clone_url: String::new(),
            default_branch: None,
            private: false,
            fork: false,
            parent: None,
            archived: false,
            permissions: None,
            allow_forking: None,
        };
        let c = commit(
            identity("Mona", "mona@example.com"),
            identity("GitHub", "noreply@github.com"),
            &[],
        );
        assert_eq!(
            names(&get_avatar_users_for_commit(Some(&github), &c)),
            ["Mona"]
        );
        assert_eq!(
            names(&get_avatar_users_for_commit(None, &c)),
            ["Mona", "GitHub"]
        );
    }

    #[test]
    fn copilot_stealth_logins() {
        assert_eq!(
            stealth_email_login(
                "198982749+Copilot@users.noreply.github.com",
                DOTCOM_API_ENDPOINT
            )
            .as_deref(),
            Some("Copilot")
        );
        assert_eq!(
            stealth_email_login("copilot@example.com", DOTCOM_API_ENDPOINT),
            None
        );
    }
}
