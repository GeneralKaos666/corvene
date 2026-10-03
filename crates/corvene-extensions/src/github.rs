//! Installing from a GitHub repository: `owner/repo`, `owner/repo@ref`,
//! `owner/repo/sub/path` or a github.com URL. The tarball comes from
//! codeload.github.com (no API, no rate limit); `HEAD` is the default
//! branch.

use crate::ExtensionError;

/// A parsed repository reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    /// a branch, tag or commit; `HEAD` when none was given
    pub reference: String,
    /// a folder inside the repository holding the extension
    pub path: Option<String>,
}

impl RepoRef {
    /// Parse `owner/repo[@ref][/sub/path]`, `https://github.com/owner/repo
    /// [/tree/<ref>[/sub/path]]`, or `git@github.com:owner/repo.git`.
    pub fn parse(text: &str) -> Result<Self, ExtensionError> {
        let bad = || {
            ExtensionError::NotAnExtension(format!(
                "{text:?} is not a GitHub repository (owner/repo, owner/repo@branch, or a github.com URL)"
            ))
        };
        let text = text.trim().trim_end_matches('/');
        let (owner, rest) = if let Some(rest) = text
            .strip_prefix("https://github.com/")
            .or_else(|| text.strip_prefix("http://github.com/"))
            .or_else(|| text.strip_prefix("github.com/"))
        {
            let mut parts = rest.splitn(2, '/');
            let owner = parts.next().ok_or_else(bad)?;
            let rest = parts.next().ok_or_else(bad)?;
            // owner/repo[.git][/tree/<ref>[/path]]
            let (repo, tail) = match rest.split_once('/') {
                Some((repo, tail)) => (repo, Some(tail)),
                None => (rest, None),
            };
            let repo = repo.strip_suffix(".git").unwrap_or(repo);
            let (reference, path) = match tail {
                Some(tail) => {
                    let tail = tail
                        .strip_prefix("tree/")
                        .or_else(|| tail.strip_prefix("blob/"))
                        .or_else(|| tail.strip_prefix("commit/"))
                        .ok_or_else(bad)?;
                    match tail.split_once('/') {
                        Some((reference, path)) => (reference.to_string(), Some(path.to_string())),
                        None => (tail.to_string(), None),
                    }
                }
                None => ("HEAD".to_string(), None),
            };
            return Self::checked(owner, repo, reference, path).ok_or_else(bad);
        } else if let Some(rest) = text.strip_prefix("git@github.com:") {
            let rest = rest.strip_suffix(".git").unwrap_or(rest);
            let (owner, repo) = rest.split_once('/').ok_or_else(bad)?;
            return Self::checked(owner, repo, "HEAD".to_string(), None).ok_or_else(bad);
        } else {
            let (owner, rest) = text.split_once('/').ok_or_else(bad)?;
            (owner, rest)
        };
        // repo[@ref][/path]
        let (repo_ref, path) = match rest.split_once('/') {
            Some((r, p)) => (r, Some(p.to_string())),
            None => (rest, None),
        };
        let (repo, reference) = match repo_ref.split_once('@') {
            Some((repo, reference)) => (repo, reference.to_string()),
            None => (repo_ref, "HEAD".to_string()),
        };
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        Self::checked(owner, repo, reference, path).ok_or_else(bad)
    }

    fn checked(owner: &str, repo: &str, reference: String, path: Option<String>) -> Option<Self> {
        let ok = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                && s != "."
                && s != ".."
        };
        if !ok(owner)
            || !ok(repo)
            || reference.is_empty()
            || reference.contains("..")
            || reference.contains(['/', ' ']) && reference != "HEAD" && !reference.contains('/')
        {
            return None;
        }
        if reference.contains(' ') || reference.contains("..") {
            return None;
        }
        let path = path
            .map(|p| p.trim_matches('/').to_string())
            .filter(|p| !p.is_empty());
        if path
            .as_deref()
            .is_some_and(|p| p.split('/').any(|c| c == ".." || c.is_empty()))
        {
            return None;
        }
        Some(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            reference,
            path,
        })
    }

    /// `https://github.com/owner/repo`.
    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }

    /// The tarball of `reference`.
    pub fn tarball_url(&self) -> String {
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/{}",
            self.owner,
            self.repo,
            self.reference.replace('/', "%2F")
        )
    }

    /// `owner.repo[.path-slug]`: the install id's tail.
    pub fn id_parts(&self) -> (String, String) {
        let name = match &self.path {
            Some(path) => format!("{}-{}", self.repo, path.replace('/', "-")),
            None => self.repo.clone(),
        };
        (self.owner.clone(), name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_spellings() {
        let r = RepoRef::parse("tree-sitter/tree-sitter-ocaml").expect("short");
        assert_eq!(
            (
                r.owner.as_str(),
                r.repo.as_str(),
                r.reference.as_str(),
                r.path.as_deref()
            ),
            ("tree-sitter", "tree-sitter-ocaml", "HEAD", None)
        );
        assert_eq!(
            r.tarball_url(),
            "https://codeload.github.com/tree-sitter/tree-sitter-ocaml/tar.gz/HEAD"
        );
        let r = RepoRef::parse("owner/repo@v1.2/ext/sub").expect("ref and path");
        assert_eq!(r.reference, "v1.2");
        assert_eq!(r.path.as_deref(), Some("ext/sub"));
        assert_eq!(r.id_parts(), ("owner".into(), "repo-ext-sub".into()));
        let r = RepoRef::parse("https://github.com/zed-extensions/zig/tree/main/languages")
            .expect("url");
        assert_eq!(r.reference, "main");
        assert_eq!(r.path.as_deref(), Some("languages"));
        let r = RepoRef::parse("https://github.com/a/b.git").expect("git url");
        assert_eq!((r.repo.as_str(), r.reference.as_str()), ("b", "HEAD"));
        let r = RepoRef::parse("git@github.com:a/b.git").expect("ssh");
        assert_eq!(r.url(), "https://github.com/a/b");
        for bad in [
            "nonsense",
            "a/../b",
            "https://gitlab.com/a/b",
            "a/b@c d",
            "a/b/../x",
            "",
        ] {
            assert!(RepoRef::parse(bad).is_err(), "{bad}");
        }
    }
}
