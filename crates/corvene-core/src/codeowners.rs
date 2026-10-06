//! Corvene `1314-code-owners` (desktop/desktop#20909): GitHub's CODEOWNERS
//! file, parsed and matched as GitHub does
//! (docs.github.com › About code owners). GHD has no CODEOWNERS support.
//!
//! - The file is the first of `.github/CODEOWNERS`, `CODEOWNERS` and
//!   `docs/CODEOWNERS` that exists; one of 3 MB or more is ignored.
//! - Each line is a pattern and its owners (`@user`, `@org/team` or an
//!   e-mail address); `#` starts a comment, also after the owners. A line
//!   GitHub would reject (`!` negation, `[ ]` ranges, an owner of another
//!   form) is skipped, the rest still apply.
//! - Patterns follow gitignore: a leading or inner `/` anchors to the root,
//!   a trailing `/` matches everything under that directory, `*` stays
//!   within one path segment, `**` crosses them. A pattern also owns
//!   everything under a directory it names, unless its last segment has a
//!   wildcard (`docs/*` owns `docs/a.md`, not `docs/sub/b.md`).
//! - Matching is case sensitive and the last matching line wins; a line
//!   without owners makes its files unowned.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use regex::Regex;
use serde::Serialize;

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::RepositoryState;

/// Where GitHub looks, in order.
pub const LOCATIONS: [&str; 3] = [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"];

/// GitHub ignores a CODEOWNERS file this large.
pub const MAX_SIZE: usize = 3 * 1024 * 1024;

#[derive(Clone, Debug)]
struct Rule {
    /// 1-based line number in the file.
    line: usize,
    pattern: String,
    regex: Regex,
    /// The pattern also owns what is under a directory it matches.
    owns_subtree: bool,
    owners: Vec<String>,
}

/// A parsed CODEOWNERS file.
#[derive(Clone, Debug, Default)]
pub struct CodeOwners {
    /// Where the file is (`.github/CODEOWNERS`…).
    pub location: String,
    /// The file's text (a reload that reads the same text changes nothing).
    text: String,
    rules: Vec<Rule>,
}

impl PartialEq for CodeOwners {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location && self.text == other.text
    }
}

/// The owners of one path: the last line that matched it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ownership {
    pub owners: Vec<String>,
    /// 1-based line number in [`CodeOwners::location`].
    pub line: usize,
    pub pattern: String,
}

impl CodeOwners {
    /// Parse `text` (the file at `location`).
    pub fn parse(text: &str, location: &str) -> Self {
        let rules = text
            .lines()
            .enumerate()
            .filter_map(|(ix, line)| parse_line(line, ix + 1))
            .collect();
        CodeOwners {
            location: location.to_string(),
            text: text.to_string(),
            rules,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The owners of `path` (relative, `/`-separated); `None` when no line
    /// matches or the last matching one names no owners.
    pub fn owners_of(&self, path: &str) -> Option<Ownership> {
        let path = path.trim_start_matches('/');
        let rule = self.rules.iter().rev().find(|r| r.matches(path))?;
        (!rule.owners.is_empty()).then(|| Ownership {
            owners: rule.owners.clone(),
            line: rule.line,
            pattern: rule.pattern.clone(),
        })
    }
}

impl Rule {
    fn matches(&self, path: &str) -> bool {
        if self.regex.is_match(path) {
            return true;
        }
        if !self.owns_subtree {
            return false;
        }
        // a directory the pattern names owns everything under it
        path.match_indices('/')
            .any(|(ix, _)| self.regex.is_match(&path[..ix]))
    }
}

/// One line, or `None` for a blank line, a comment or a line GitHub would
/// reject.
fn parse_line(line: &str, number: usize) -> Option<Rule> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (pattern, rest) = split_pattern(line);
    if pattern.is_empty() || pattern.starts_with('!') || pattern.contains('[') {
        return None;
    }
    let mut owners = Vec::new();
    for token in rest.split_whitespace() {
        if token.starts_with('#') {
            break;
        }
        if !is_owner(token) {
            return None;
        }
        owners.push(token.to_string());
    }
    let (regex, owns_subtree) = compile(&pattern)?;
    Some(Rule {
        line: number,
        pattern,
        regex,
        owns_subtree,
        owners,
    })
}

/// The pattern (`\ ` is a space in it) and the rest of the line.
fn split_pattern(line: &str) -> (String, &str) {
    let mut pattern = String::new();
    let mut chars = line.char_indices().peekable();
    while let Some((ix, c)) = chars.next() {
        match c {
            '\\' if chars.peek().is_some_and(|(_, n)| *n == ' ') => {
                pattern.push(' ');
                chars.next();
            }
            c if c.is_whitespace() => return (pattern, &line[ix..]),
            c => pattern.push(c),
        }
    }
    (pattern, "")
}

/// `@user`, `@org/team` or an e-mail address.
fn is_owner(token: &str) -> bool {
    let name = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if let Some(rest) = token.strip_prefix('@') {
        return match rest.split_once('/') {
            Some((org, team)) => name(org) && name(team),
            None => name(rest),
        };
    }
    match token.split_once('@') {
        Some((user, domain)) => !user.is_empty() && domain.contains('.') && !domain.contains('@'),
        None => false,
    }
}

/// The regex for `pattern` and whether it also owns subtrees.
fn compile(pattern: &str) -> Option<(Regex, bool)> {
    let dir_only = pattern.ends_with('/');
    let body = pattern.trim_end_matches('/');
    let anchored = body.starts_with('/') || body.contains('/');
    let body = body.trim_start_matches('/');
    if body.is_empty() {
        // `/` alone: the whole repository
        return Regex::new("^.*$").ok().map(|r| (r, false));
    }
    let mut re = String::from("^");
    if !anchored {
        re.push_str("(?:.*/)?");
    }
    let mut rest = body;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**/") {
            re.push_str("(?:.*/)?");
            rest = after;
        } else if rest == "**" {
            re.push_str(".*");
            rest = "";
        } else if let Some(after) = rest.strip_prefix("**") {
            re.push_str(".*");
            rest = after;
        } else {
            let c = rest.chars().next()?;
            match c {
                '*' => re.push_str("[^/]*"),
                '?' => re.push_str("[^/]"),
                c => re.push_str(&regex::escape(&c.to_string())),
            }
            rest = &rest[c.len_utf8()..];
        }
    }
    re.push('$');
    let last = body.rsplit('/').next().unwrap_or(body);
    let owns_subtree = dir_only || !last.contains(['*', '?']);
    Regex::new(&re).ok().map(|r| (r, owns_subtree))
}

/// The most revisions whose CODEOWNERS file is kept.
const MAX_REVISIONS: usize = 32;

/// Which CODEOWNERS file a list follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeOwnersSource<'a> {
    /// The work tree's (the Changes list).
    WorkTree,
    /// The one of a commit or branch (History's selected commit, the pull
    /// request's base branch).
    Rev(&'a str),
}

/// A repository's CODEOWNERS files (`RepositoryState::code_owners`).
#[derive(Clone, Debug, Default)]
pub struct CodeOwnersState {
    /// The work tree's, read after every refresh.
    pub worktree: Option<Arc<CodeOwners>>,
    /// Revision → its file (`None`: it has none).
    at: HashMap<String, Option<Arc<CodeOwners>>>,
    loading: HashSet<String>,
    /// What counts as the user: `@login`, `@org/team` of each of the
    /// account's teams and its e-mail addresses (lower case).
    pub yours: Option<Arc<HashSet<String>>>,
    /// The account (`Account::key`, empty for none) `yours` is for.
    yours_for: Option<String>,
    teams_loading: bool,
}

/// One row's owners as the file lists show them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowOwners {
    pub ownership: Ownership,
    /// One of the owners is the user or one of their teams.
    pub yours: bool,
}

impl RowOwners {
    /// "@a", or "@a +2" for more owners.
    pub fn label(&self) -> String {
        let first = &self.ownership.owners[0];
        match self.ownership.owners.len() {
            1 => first.clone(),
            n => format!("{first} +{}", n - 1),
        }
    }

    /// GitHub's tooltip: "Owned by @a and @b (from CODEOWNERS line 3)".
    pub fn tooltip(&self, location: &str) -> String {
        let owners = &self.ownership.owners;
        let list = match owners.len() {
            1 => owners[0].clone(),
            2 => format!("{} and {}", owners[0], owners[1]),
            n => format!("{}, and {}", owners[..n - 1].join(", "), owners[n - 1]),
        };
        let file = location.rsplit('/').next().unwrap_or(location);
        format!("Owned by {list} (from {file} line {})", self.ownership.line)
    }
}

impl CodeOwnersState {
    /// The file `source` names, once loaded.
    pub fn file(&self, source: CodeOwnersSource) -> Option<&Arc<CodeOwners>> {
        match source {
            CodeOwnersSource::WorkTree => self.worktree.as_ref(),
            CodeOwnersSource::Rev(rev) => self.at.get(rev)?.as_ref(),
        }
    }

    /// The owners of `path` in `source`'s file.
    pub fn row(&self, source: CodeOwnersSource, path: &str) -> Option<RowOwners> {
        let ownership = self.file(source)?.owners_of(path)?;
        Some(RowOwners {
            yours: self.is_yours(&ownership.owners),
            ownership,
        })
    }

    pub fn is_yours(&self, owners: &[String]) -> bool {
        self.yours
            .as_ref()
            .is_some_and(|y| owners.iter().any(|o| y.contains(&o.to_lowercase())))
    }

    /// The owners of `paths` with their file counts: owners that are not
    /// the user first, then by count; `(owner, files, yours)`.
    pub fn summary<'a>(
        &self,
        source: CodeOwnersSource,
        paths: impl IntoIterator<Item = &'a str>,
    ) -> Vec<(String, usize, bool)> {
        let Some(file) = self.file(source) else {
            return Vec::new();
        };
        let mut counts: HashMap<String, usize> = HashMap::new();
        for path in paths {
            if let Some(o) = file.owners_of(path) {
                for owner in o.owners {
                    *counts.entry(owner).or_default() += 1;
                }
            }
        }
        let mut out: Vec<(String, usize, bool)> = counts
            .into_iter()
            .map(|(owner, n)| {
                let yours = self.is_yours(std::slice::from_ref(&owner));
                (owner, n, yours)
            })
            .collect();
        out.sort_by(|a, b| a.2.cmp(&b.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
        out
    }
}

impl RepositoryState {
    /// `1314-code-owners`: the owners of `path` (the flag is checked by
    /// the caller).
    pub fn code_owners_of(&self, source: CodeOwnersSource, path: &str) -> Option<RowOwners> {
        self.code_owners.row(source, path)
    }
}

impl Dispatcher {
    /// `1314-code-owners`: read the work tree's CODEOWNERS file (after a
    /// refresh), then the user's teams once.
    pub fn refresh_code_owners(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx).read(cx).flags.bool(crate::flags::ids::CODE_OWNERS) {
            return;
        }
        let Some((_, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || load_worktree(&workdir).map(Arc::new),
            move |file, cx| {
                let has = file.is_some();
                Self::state(cx).update(cx, |s, cx| {
                    let owners = &mut s.repo_state_mut(id).code_owners;
                    if owners.worktree.as_deref() != file.as_deref() {
                        owners.worktree = file;
                        cx.notify();
                    }
                });
                if has {
                    Self::load_code_owner_teams(id, cx);
                }
            },
        );
    }

    /// `1314-code-owners`: read the CODEOWNERS file of `rev` (a commit:
    /// once; a branch, `reload`: again, its tip may have moved).
    pub fn load_code_owners_at(id: u64, rev: String, reload: bool, cx: &mut dyn Host) {
        let go = {
            let s = Self::state(cx).read(cx);
            s.flags.bool(crate::flags::ids::CODE_OWNERS)
                && s.repo_states.get(&id).is_some_and(|rs| {
                    !rs.code_owners.loading.contains(&rev)
                        && (reload || !rs.code_owners.at.contains_key(&rev))
                })
        };
        if !go {
            return;
        }
        let Some((_, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, _| {
            s.repo_state_mut(id).code_owners.loading.insert(rev.clone());
        });
        let key = rev.clone();
        spawn_bg(
            cx,
            move || load_at(&workdir, &rev).map(Arc::new),
            move |file, cx| {
                let has = file.is_some();
                Self::state(cx).update(cx, |s, cx| {
                    let owners = &mut s.repo_state_mut(id).code_owners;
                    owners.loading.remove(&key);
                    if owners.at.len() >= MAX_REVISIONS && !owners.at.contains_key(&key) {
                        owners.at.clear();
                    }
                    if owners.at.get(&key).map(|f| f.as_deref()) != Some(file.as_deref()) {
                        owners.at.insert(key, file);
                        cx.notify();
                    }
                });
                if has {
                    Self::load_code_owner_teams(id, cx);
                }
            },
        );
    }

    /// What counts as the user for repository `id`: its account's login,
    /// e-mail addresses and teams (`GET /user/teams`), asked once.
    fn load_code_owner_teams(id: u64, cx: &mut dyn Host) {
        let state = Self::state(cx);
        let account = {
            let s = state.read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let account = s.account_for_repository(id).cloned();
            let key = account.as_ref().map(|a| a.key()).unwrap_or_default();
            // asked already for this account (a sign-in asks again)
            if rs.code_owners.teams_loading || rs.code_owners.yours_for.as_ref() == Some(&key) {
                return;
            }
            account
        };
        let key = account.as_ref().map(|a| a.key()).unwrap_or_default();
        let Some(account) = account else {
            // no account: nothing is the user's
            state.update(cx, |s, cx| {
                let owners = &mut s.repo_state_mut(id).code_owners;
                owners.yours = Some(Arc::default());
                owners.yours_for = Some(key);
                cx.notify();
            });
            return;
        };
        let mut yours: HashSet<String> = account.emails.iter().map(|e| e.to_lowercase()).collect();
        yours.insert(format!("@{}", account.login.to_lowercase()));
        let Some(client) = crate::alive::account_client(&account) else {
            state.update(cx, |s, cx| {
                let owners = &mut s.repo_state_mut(id).code_owners;
                owners.yours = Some(Arc::new(yours));
                owners.yours_for = Some(key);
                cx.notify();
            });
            return;
        };
        state.update(cx, |s, _| s.repo_state_mut(id).code_owners.teams_loading = true);
        spawn_bg(
            cx,
            move || client.user_teams(),
            move |teams, cx| {
                match teams {
                    Ok(teams) => yours.extend(teams.into_iter().map(|t| t.to_lowercase())),
                    Err(err) => tracing::debug!(%err, "could not list the user's teams"),
                }
                Self::state(cx).update(cx, |s, cx| {
                    let owners = &mut s.repo_state_mut(id).code_owners;
                    owners.teams_loading = false;
                    owners.yours = Some(Arc::new(yours));
                    owners.yours_for = Some(key);
                    cx.notify();
                });
            },
        );
    }
}

/// The work tree's CODEOWNERS file.
pub fn load_worktree(root: &Path) -> Option<CodeOwners> {
    LOCATIONS.iter().find_map(|location| {
        let bytes = std::fs::read(root.join(location)).ok()?;
        from_bytes(&bytes, location)
    })
}

/// The CODEOWNERS file of revision `rev`.
pub fn load_at(root: &Path, rev: &str) -> Option<CodeOwners> {
    LOCATIONS.iter().find_map(|location| {
        let bytes = corvene_git::handle::blob_bytes(root, rev, location)?;
        from_bytes(&bytes, location)
    })
}

fn from_bytes(bytes: &[u8], location: &str) -> Option<CodeOwners> {
    if bytes.len() >= MAX_SIZE {
        tracing::debug!(location, "CODEOWNERS is too large; GitHub ignores it");
        return None;
    }
    Some(CodeOwners::parse(&String::from_utf8_lossy(bytes), location))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owners(file: &CodeOwners, path: &str) -> Option<Vec<String>> {
        file.owners_of(path).map(|o| o.owners)
    }

    fn one(pattern: &str, path: &str) -> bool {
        CodeOwners::parse(&format!("{pattern} @o"), "CODEOWNERS")
            .owners_of(path)
            .is_some()
    }

    #[test]
    fn matches_the_docs_examples() {
        assert!(one("*", "a/b/c.txt"));
        assert!(one("*.js", "src/app/main.js"));
        assert!(!one("*.js", "src/app/main.jsx"));
        assert!(one("docs/*", "docs/getting-started.md"));
        assert!(!one("docs/*", "docs/build-app/troubleshooting.md"));
        assert!(one("/build/logs/", "build/logs/2024/a.log"));
        assert!(!one("/build/logs/", "src/build/logs/a.log"));
        assert!(one("apps/", "apps/x.rb"));
        assert!(one("apps/", "deep/apps/x/y.rb"));
        assert!(one("/docs/", "docs/a/b.md"));
        assert!(!one("/docs/", "src/docs/a.md"));
        assert!(one("**/logs", "build/logs/a.log"));
        assert!(one("**/logs", "deeply/nested/logs/x"));
        assert!(one("docs/**", "docs/a/b/c.md"));
        assert!(!one("docs/**", "src/docs/a.md"));
        assert!(one("/scripts", "scripts/x/y.sh"));
        assert!(one("README.md", "pkg/README.md"));
        assert!(!one("Readme.md", "README.md"));
    }

    #[test]
    fn last_match_wins_and_empty_owners_unown() {
        let file = CodeOwners::parse(
            "# comment\n\
             *       @global-owner1 @global-owner2\n\
             *.js    @js-owner #This is an inline comment.\n\
             /apps/ @octocat\n\
             /apps/github\n\
             docs/*  docs@example.com\n",
            ".github/CODEOWNERS",
        );
        assert_eq!(
            owners(&file, "README.md"),
            Some(vec!["@global-owner1".into(), "@global-owner2".into()])
        );
        assert_eq!(owners(&file, "src/a.js"), Some(vec!["@js-owner".into()]));
        assert_eq!(owners(&file, "apps/web/a.rb"), Some(vec!["@octocat".into()]));
        assert_eq!(owners(&file, "apps/github/a.rb"), None);
        assert_eq!(owners(&file, "docs/a.md"), Some(vec!["docs@example.com".into()]));
        let o = file.owners_of("src/a.js").unwrap();
        assert_eq!((o.line, o.pattern.as_str()), (3, "*.js"));
    }

    #[test]
    fn skips_lines_github_rejects() {
        let file = CodeOwners::parse(
            "* @all\n!*.md @nobody\n*.[ch] @c\n*.rs not-an-owner\n[Section]\n\\#x @hash\n",
            "CODEOWNERS",
        );
        assert_eq!(owners(&file, "a.md"), Some(vec!["@all".into()]));
        assert_eq!(owners(&file, "a.c"), Some(vec!["@all".into()]));
        assert_eq!(owners(&file, "a.rs"), Some(vec!["@all".into()]));
        assert!(is_owner("@org/team-name"));
        assert!(is_owner("@user_1"));
        assert!(!is_owner("@org/"));
        assert!(!is_owner("user"));
    }

    #[test]
    fn escaped_spaces_and_question_marks() {
        assert!(one("my\\ file.txt", "my file.txt"));
        assert!(one("file?.txt", "file1.txt"));
        assert!(!one("file?.txt", "file/.txt"));
    }

    #[test]
    fn labels_tooltips_and_summaries() {
        let file = CodeOwners::parse("* @org/web\n*.rs @me @org/core @hubot\n", ".github/CODEOWNERS");
        let mut state = CodeOwnersState {
            worktree: Some(Arc::new(file)),
            yours: Some(Arc::new(["@me".to_string()].into_iter().collect())),
            ..Default::default()
        };
        let row = state.row(CodeOwnersSource::WorkTree, "src/a.rs").unwrap();
        assert!(row.yours);
        assert_eq!(row.label(), "@me +2");
        assert_eq!(
            row.tooltip(".github/CODEOWNERS"),
            "Owned by @me, @org/core, and @hubot (from CODEOWNERS line 2)"
        );
        let web = state.row(CodeOwnersSource::WorkTree, "index.html").unwrap();
        assert!(!web.yours);
        assert_eq!(web.tooltip("CODEOWNERS"), "Owned by @org/web (from CODEOWNERS line 1)");
        let summary = state.summary(CodeOwnersSource::WorkTree, ["a.rs", "b.rs", "c.html"]);
        assert_eq!(summary[0], ("@hubot".to_string(), 2, false));
        assert_eq!(summary.last().unwrap(), &("@me".to_string(), 2, true));
        assert!(state.row(CodeOwnersSource::Rev("abc"), "a.rs").is_none());
        state.yours = None;
        assert!(!state.row(CodeOwnersSource::WorkTree, "a.rs").unwrap().yours);
    }

    #[test]
    fn loads_the_first_location_from_the_work_tree() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/CODEOWNERS"), "* @docs").unwrap();
        std::fs::write(dir.path().join(".github/CODEOWNERS"), "* @github").unwrap();
        let file = load_worktree(dir.path()).unwrap();
        assert_eq!(file.location, ".github/CODEOWNERS");
        assert_eq!(owners(&file, "x"), Some(vec!["@github".into()]));
        assert!(load_worktree(&dir.path().join("missing")).is_none());
    }
}
