//! Port of GitHub Desktop's `app/test/unit/api-test.ts`.
//!
//! - `getNextPagePathWithIncreasingPageSize(response)` (`lib/api.ts`, used
//!   by `fetchUpdatedPullRequests` to ramp the page size up while following
//!   the `Link: <…>; rel="next"` header) has no Corvene counterpart:
//!   `corvene_github::Client` never reads `Link` headers and pages with a
//!   fixed `per_page=100&page=N` instead. The cases call a stand-in
//!   ([`get_next_page_path_with_increasing_page_size`], taking the
//!   response's headers) and are ignored until it exists.
//! - `API.getDiffChangesCommitMessage` is the Copilot commit message call;
//!   Copilot is left out by design, so that case is skipped
//!   (`tools/ghd-tests/skips/api.tsv`).
//! - `API.fetchProtectedBranches(owner, name)` (`GET
//!   repos/{owner}/{name}/branches?protected=true`, feeding
//!   `updateBranchProtectionsFromAPI`) has no Corvene counterpart either
//!   (Corvene only asks `push_control` for the current branch). GitHub
//!   Desktop stubs `API.request`; here the stubbed response comes from a
//!   local HTTP server (`api_support::serve`) that a real `Client` talks
//!   to, and the failing request is a connection to a port nothing listens
//!   on. The call itself is a stand-in ([`fetch_protected_branches`]).

use std::collections::HashMap;

use corvene_github::{Client, Endpoint};
use serde_json::json;

use crate::api_support::{StubResponse, serve, unreachable_endpoint};

/// The response headers GitHub Desktop's cases build (`new Headers(…)`).
type Headers = Vec<(String, String)>;

/// Stand-in for GitHub Desktop's `getNextPagePathWithIncreasingPageSize(response)`
/// (`lib/api.ts`): the path of the `rel="next"` link in the response's
/// `Link` header, with `per_page` doubled (at most 100) and `page`
/// recomputed when the items received so far line up with the doubled page
/// size; `None` without a next link. Replace it with the `corvene_github`
/// function once there is one and remove the `#[ignore]`s.
fn get_next_page_path_with_increasing_page_size(_headers: &Headers) -> Option<String> {
    unimplemented!("corvene_github has no getNextPagePathWithIncreasingPageSize")
}

/// GitHub Desktop's `IPageInfo`.
#[derive(Clone, Copy)]
struct PageInfo {
    per_page: i64,
    page: i64,
}

fn create_headers_with_next_link(url: &str) -> Headers {
    vec![("Link".to_string(), format!("<{url}>; rel=\"next\""))]
}

/// `URL.parse(path, true)`: the pathname and the query parameters.
fn parse_path(path: &str) -> (String, HashMap<String, String>) {
    let (pathname, query) = path.split_once('?').unwrap_or((path, ""));
    let query = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (key.to_string(), value.to_string())
        })
        .collect();
    (pathname.to_string(), query)
}

fn assert_next(current: PageInfo, expected: PageInfo) {
    let headers = create_headers_with_next_link(&format!(
        "/items?per_page={}&page={}",
        current.per_page, current.page
    ));

    let next_path = get_next_page_path_with_increasing_page_size(&headers);

    assert!(next_path.is_some());
    let (pathname, query) = parse_path(&next_path.unwrap());

    assert_eq!(pathname, "/items");

    // `parseInt` of a missing parameter is `NaN`, which equals nothing
    let per_page: Option<i64> = query.get("per_page").and_then(|v| v.parse().ok());
    let page: Option<i64> = query.get("page").and_then(|v| v.parse().ok());

    assert_eq!(per_page, Some(expected.per_page));
    assert_eq!(page, Some(expected.page));
    let (per_page, page) = (per_page.unwrap(), page.unwrap());

    // If getNextPagePathWithIncreasingPageSize has fiddled with the
    // page size or page number we want to ensure that the next page will
    // get us more items than what we've gotten thus far.
    if current.per_page != per_page || current.page != page {
        let received_current = current.per_page * current.page;
        let received_next = per_page * page;

        assert!(received_next > received_current);
    }
}

fn info(per_page: i64, page: i64) -> PageInfo {
    PageInfo { per_page, page }
}

mod get_next_page_path_with_increasing_page_size {
    use super::*;

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › returns null when there's no link header
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn returns_null_when_theres_no_link_header() {
        assert!(get_next_page_path_with_increasing_page_size(&Vec::new()).is_none());
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › returns raw link when missing page size
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn returns_raw_link_when_missing_page_size() {
        let next_path = get_next_page_path_with_increasing_page_size(
            &create_headers_with_next_link("/items?page=2"),
        );

        assert_eq!(next_path.as_deref(), Some("/items?page=2"));
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › returns raw link when missing page number
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn returns_raw_link_when_missing_page_number() {
        let next_path = get_next_page_path_with_increasing_page_size(
            &create_headers_with_next_link("/items?per_page=10"),
        );

        assert_eq!(next_path.as_deref(), Some("/items?per_page=10"));
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › does not increase page size when not aligned
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn does_not_increase_page_size_when_not_aligned() {
        let next_path = get_next_page_path_with_increasing_page_size(
            &create_headers_with_next_link("/items?per_page=10&page=2"),
        );

        assert_eq!(next_path.as_deref(), Some("/items?per_page=10&page=2"));
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › increases page size on alignment with an initial page size of 10
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn increases_page_size_on_alignment_with_an_initial_page_size_of_10() {
        assert_next(info(10, 2), info(10, 2));
        assert_next(info(10, 3), info(20, 2));
        assert_next(info(20, 2), info(20, 2));
        assert_next(info(20, 3), info(40, 2));
        assert_next(info(40, 2), info(40, 2));
        assert_next(info(40, 3), info(80, 2));
        assert_next(info(80, 3), info(80, 3));
        assert_next(info(80, 4), info(80, 4));
        assert_next(info(80, 5), info(80, 5));
        assert_next(info(80, 6), info(100, 5));
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › increases page size on alignment with an initial page size of 5
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn increases_page_size_on_alignment_with_an_initial_page_size_of_5() {
        assert_next(info(5, 2), info(5, 2));
        assert_next(info(5, 3), info(10, 2));
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › increases page size on alignment with an initial page size of 1
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn increases_page_size_on_alignment_with_an_initial_page_size_of_1() {
        assert_next(info(1, 2), info(1, 2));
        assert_next(info(1, 3), info(2, 2));
        assert_next(info(2, 3), info(4, 2));
        assert_next(info(4, 2), info(4, 2));
        assert_next(info(4, 3), info(8, 2));
        assert_next(info(8, 2), info(8, 2));
        assert_next(info(8, 3), info(16, 2));
        assert_next(info(16, 2), info(16, 2));
        assert_next(info(16, 3), info(32, 2));
        assert_next(info(32, 2), info(32, 2));
        assert_next(info(32, 3), info(64, 2));
    }

    // GHD: unit/api-test.ts › API › getNextPagePathWithIncreasingPageSize › doesn't increase page size when page size is 100
    #[test]
    #[ignore = "ghd: missing: corvene_github has no getNextPagePathWithIncreasingPageSize (lib/api.ts); Client never reads Link headers"]
    fn doesnt_increase_page_size_when_page_size_is_100() {
        assert_next(info(100, 2), info(100, 2));
        assert_next(info(100, 3), info(100, 3));
        assert_next(info(100, 4), info(100, 4));
        assert_next(info(100, 5), info(100, 5));
        assert_next(info(100, 6), info(100, 6));
        assert_next(info(100, 7), info(100, 7));
        assert_next(info(100, 8), info(100, 8));
        assert_next(info(100, 9), info(100, 9));
        assert_next(info(100, 10), info(100, 10));
    }
}

/// GitHub Desktop's `IAPIBranch` (`lib/api.ts`). Replace it with the
/// `corvene_github` type that comes with [`fetch_protected_branches`].
#[derive(Debug, PartialEq, Eq)]
struct ApiBranch {
    name: String,
    protected: bool,
}

/// Stand-in for GitHub Desktop's `API.fetchProtectedBranches(owner, name)`
/// (`lib/api.ts`): `GET repos/{owner}/{name}/branches?protected=true`, the
/// parsed branches on success, `None` when the request fails or answers
/// with an error. Replace it with the `Client` method once there is one and
/// remove the `#[ignore]`s.
fn fetch_protected_branches(_api: &Client, _owner: &str, _name: &str) -> Option<Vec<ApiBranch>> {
    unimplemented!("corvene_github::Client has no fetchProtectedBranches")
}

mod fetch_protected_branches {
    use super::*;

    /// GitHub Desktop's `createAPI(request)`: an API whose requests all
    /// reach `endpoint` (a stub server, or nothing).
    fn create_api(endpoint: String) -> Client {
        Client::new(Endpoint::from_api_base(&endpoint), "token")
    }

    // GHD: unit/api-test.ts › API › fetchProtectedBranches › returns the protected branches from a successful request
    #[test]
    #[ignore = "ghd: missing: corvene_github::Client has no fetchProtectedBranches (lib/api.ts API.fetchProtectedBranches)"]
    fn returns_the_protected_branches_from_a_successful_request() {
        let branches = json!([
            { "name": "main", "protected": true },
            { "name": "release", "protected": true },
        ]);
        let api = create_api(serve(StubResponse::new(200, branches.to_string())));

        assert_eq!(
            fetch_protected_branches(&api, "desktop", "desktop"),
            Some(vec![
                ApiBranch {
                    name: "main".into(),
                    protected: true,
                },
                ApiBranch {
                    name: "release".into(),
                    protected: true,
                },
            ])
        );
    }

    // GHD: unit/api-test.ts › API › fetchProtectedBranches › returns an empty array when the request succeeds without protected branches
    #[test]
    #[ignore = "ghd: missing: corvene_github::Client has no fetchProtectedBranches (lib/api.ts API.fetchProtectedBranches)"]
    fn returns_an_empty_array_when_the_request_succeeds_without_protected_branches() {
        let api = create_api(serve(StubResponse::new(200, json!([]).to_string())));

        assert_eq!(
            fetch_protected_branches(&api, "desktop", "desktop"),
            Some(vec![])
        );
    }

    // GHD: unit/api-test.ts › API › fetchProtectedBranches › returns null when the request fails
    #[test]
    #[ignore = "ghd: missing: corvene_github::Client has no fetchProtectedBranches (lib/api.ts API.fetchProtectedBranches)"]
    fn returns_null_when_the_request_fails() {
        let api = create_api(unreachable_endpoint());

        assert_eq!(fetch_protected_branches(&api, "desktop", "desktop"), None);
    }

    // GHD: unit/api-test.ts › API › fetchProtectedBranches › returns null when the repository is not found
    #[test]
    #[ignore = "ghd: missing: corvene_github::Client has no fetchProtectedBranches (lib/api.ts API.fetchProtectedBranches)"]
    fn returns_null_when_the_repository_is_not_found() {
        let api = create_api(serve(StubResponse::new(
            404,
            json!({ "message": "Not Found" }).to_string(),
        )));

        assert_eq!(fetch_protected_branches(&api, "desktop", "desktop"), None);
    }
}
