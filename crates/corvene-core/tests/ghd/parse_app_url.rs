//! Port of GitHub Desktop's `app/test/unit/parse-app-url-test.ts`.
//!
//! GitHub Desktop's `parseAppURL` (`lib/parse-app-url.ts`) is
//! `corvene_core::app_url::parse_app_url`. Its `URLActionType` is
//! `UrlAction`: the action's `name` is the variant (`'unknown'` is
//! `UrlAction::Unknown`, `'oauth'` is `UrlAction::OAuth`,
//! `'open-repository-from-url'` is `UrlAction::OpenRepositoryFromUrl`) and
//! the fields are the variant's fields. Corvene registers `x-corvene://`, but
//! the parser reads the action from the host whatever the scheme, as GitHub
//! Desktop's does, so the test URLs keep GitHub Desktop's schemes.

use corvene_core::app_url::{UrlAction, parse_app_url};

/// `result.name` of GitHub Desktop's `URLActionType`.
fn name(action: &UrlAction) -> &'static str {
    match action {
        UrlAction::OAuth { .. } => "oauth",
        UrlAction::OpenRepositoryFromUrl { .. } => "open-repository-from-url",
        UrlAction::Unknown { .. } => "unknown",
        // Corvene's own actions (`openLocalRepo`, `importGitConfig`, `flags`)
        _ => "corvene-only action",
    }
}

/// The fields of an `IOpenRepositoryFromURLAction` (`result as
/// IOpenRepositoryFromURLAction`).
struct OpenRepo {
    url: String,
    branch: Option<String>,
    pr: Option<String>,
    filepath: Option<String>,
}

fn as_open_repo(action: UrlAction) -> OpenRepo {
    match action {
        UrlAction::OpenRepositoryFromUrl {
            url,
            branch,
            pr,
            filepath,
        } => OpenRepo {
            url,
            branch,
            pr,
            filepath,
        },
        other => panic!("not an open-repository-from-url action: {other:?}"),
    }
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › returns unknown by default
#[test]
fn returns_unknown_by_default() {
    assert_eq!(name(&parse_app_url("")), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › oauth › returns right name
#[test]
fn oauth_returns_right_name() {
    let result = parse_app_url(
        "x-github-client://oauth?code=18142422&state=e4cd2dea-1567-46aa-8eb2-c7f56e943187",
    );
    assert_eq!(name(&result), "oauth");

    let UrlAction::OAuth { code, .. } = result else {
        panic!("not an oauth action");
    };
    assert_eq!(code, "18142422");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › returns right name
#[test]
fn https_returns_right_name() {
    let result = parse_app_url("github-mac://openRepo/https://github.com/desktop/desktop");
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "https://github.com/desktop/desktop");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › returns unknown when no remote defined
#[test]
fn https_returns_unknown_when_no_remote_defined() {
    let result = parse_app_url("github-mac://openRepo/");
    assert_eq!(name(&result), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › adds branch name if set
#[test]
fn https_adds_branch_name_if_set() {
    let result = parse_app_url(
        "github-mac://openRepo/https://github.com/desktop/desktop?branch=cancel-2fa-flow",
    );
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "https://github.com/desktop/desktop");
    assert_eq!(open_repo.branch.as_deref(), Some("cancel-2fa-flow"));
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › adds pull request ID if found
#[test]
fn https_adds_pull_request_id_if_found() {
    let result = parse_app_url(
        "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=pr%2F1569&pr=1569",
    );
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "https://github.com/octokit/octokit.net");
    assert_eq!(open_repo.branch.as_deref(), Some("pr/1569"));
    assert_eq!(open_repo.pr.as_deref(), Some("1569"));
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › returns unknown for unexpected pull request input
#[test]
fn https_returns_unknown_for_unexpected_pull_request_input() {
    let result = parse_app_url(
        "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=bar&pr=foo",
    );
    assert_eq!(name(&result), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › returns unknown for invalid branch name
#[test]
fn https_returns_unknown_for_invalid_branch_name() {
    // branch=<>
    let result =
        parse_app_url("github-mac://openRepo/https://github.com/octokit/octokit.net?branch=%3C%3E");
    assert_eq!(name(&result), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via HTTPS › adds file path if found
#[test]
fn https_adds_file_path_if_found() {
    let result = parse_app_url(
        "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=master&filepath=Octokit.Reactive%2FOctokit.Reactive.csproj",
    );
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "https://github.com/octokit/octokit.net");
    assert_eq!(open_repo.branch.as_deref(), Some("master"));
    assert_eq!(
        open_repo.filepath.as_deref(),
        Some("Octokit.Reactive/Octokit.Reactive.csproj")
    );
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › returns right name
#[test]
fn ssh_returns_right_name() {
    let result = parse_app_url("github-mac://openRepo/git@github.com/desktop/desktop");
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "git@github.com/desktop/desktop");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › returns unknown when no remote defined
#[test]
fn ssh_returns_unknown_when_no_remote_defined() {
    let result = parse_app_url("github-mac://openRepo/");
    assert_eq!(name(&result), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › adds branch name if set
#[test]
fn ssh_adds_branch_name_if_set() {
    let result = parse_app_url(
        "github-mac://openRepo/git@github.com/desktop/desktop?branch=cancel-2fa-flow",
    );
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "git@github.com/desktop/desktop");
    assert_eq!(open_repo.branch.as_deref(), Some("cancel-2fa-flow"));
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › adds pull request ID if found
#[test]
fn ssh_adds_pull_request_id_if_found() {
    let result = parse_app_url(
        "github-mac://openRepo/git@github.com/octokit/octokit.net?branch=pr%2F1569&pr=1569",
    );
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "git@github.com/octokit/octokit.net");
    assert_eq!(open_repo.branch.as_deref(), Some("pr/1569"));
    assert_eq!(open_repo.pr.as_deref(), Some("1569"));
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › returns unknown for unexpected pull request input
#[test]
fn ssh_returns_unknown_for_unexpected_pull_request_input() {
    let result =
        parse_app_url("github-mac://openRepo/git@github.com/octokit/octokit.net?branch=bar&pr=foo");
    assert_eq!(name(&result), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › returns unknown for invalid branch name
#[test]
fn ssh_returns_unknown_for_invalid_branch_name() {
    // branch=<>
    let result =
        parse_app_url("github-mac://openRepo/git@github.com/octokit/octokit.net?branch=%3C%3E");
    assert_eq!(name(&result), "unknown");
}

// GHD: unit/parse-app-url-test.ts › parseAppURL › openRepo via SSH › adds file path if found
#[test]
fn ssh_adds_file_path_if_found() {
    let result = parse_app_url(
        "github-mac://openRepo/git@github.com/octokit/octokit.net?branch=master&filepath=Octokit.Reactive%2FOctokit.Reactive.csproj",
    );
    assert_eq!(name(&result), "open-repository-from-url");

    let open_repo = as_open_repo(result);
    assert_eq!(open_repo.url, "git@github.com/octokit/octokit.net");
    assert_eq!(open_repo.branch.as_deref(), Some("master"));
    assert_eq!(
        open_repo.filepath.as_deref(),
        Some("Octokit.Reactive/Octokit.Reactive.csproj")
    );
}
