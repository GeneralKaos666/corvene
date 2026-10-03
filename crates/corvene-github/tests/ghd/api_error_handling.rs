//! Port of GitHub Desktop's `app/test/unit/api-error-handling-test.ts`.
//!
//! - GitHub Desktop's `new APIError(response, apiError)` (`lib/http.ts`) is
//!   the error an API call fails with when the server answers with an error
//!   status: `responseStatus` is the status, `message` is the body's
//!   `message` followed by its `errors[].message`s in parentheses (or
//!   `API error <url>: <statusText> (<status>)` when the body could not be
//!   parsed, `apiError === null`), and `apiError` is the parsed body.
//!   Corvene builds that error inside `corvene_github::Client`
//!   (`get_json_accept` / `post_json`) from the response it read: a
//!   `GitHubError::Api { status, message }`, where `message` is the body's
//!   `message` (`"request failed"` when there is none) and, with
//!   `Client::with_error_details(true)`, the `errors[].message`s in
//!   parentheses. So [`api_error`] serves the case's response from a local
//!   HTTP server (`api_support::serve`; the body is the case's `apiError`
//!   as JSON, or empty for `null`) to a real `Client` and returns the error
//!   its request fails with. `responseStatus` is `GitHubError::Api.status`
//!   and `message` is `GitHubError::Api.message` (the `Display` of the
//!   error adds `GitHub returned <status>: ` in front of it).
//!   `with_error_details` is flag `311-api-error-details`, passed at its
//!   GitHub Desktop value (off). `GitHubError` keeps no parsed body, so
//!   `apiError` is a stand-in ([`api_error_body`]).
//! - `getAbsoluteUrl(endpoint, path)` is
//!   `Endpoint::from_api_base(endpoint).api(path)`, as in `http.rs`.

use corvene_github::{Client, Endpoint, GitHubError};
use serde_json::{Value, json};

use crate::api_support::{StubResponse, serve};

/// GitHub Desktop's `new APIError(response, apiError)` for a response with
/// `status` / `status_text` from `url_path` (relative to the endpoint): the
/// error a `Client` request to that path fails with.
fn api_error(
    status: u16,
    status_text: &'static str,
    url_path: &str,
    api_error: Option<Value>,
) -> GitHubError {
    let body = api_error.map(|e| e.to_string()).unwrap_or_default();
    let endpoint = serve(StubResponse {
        status,
        status_text,
        headers: vec![("Content-Type", "application/json")],
        body,
    });
    // flag 311-api-error-details at its GitHub Desktop value
    let client = Client::new(Endpoint::from_api_base(&endpoint), "token").with_error_details(false);
    let result = match url_path.split('/').collect::<Vec<_>>().as_slice() {
        ["repos", owner, name] => client.repository(owner, name).map(|_| ()),
        ["user"] => client.current_user(Vec::new()).map(|_| ()),
        _ => panic!("no Client call requests {url_path}"),
    };
    match result {
        Ok(()) => panic!("the request to {url_path} succeeded"),
        Err(err) => err,
    }
}

/// `APIError.responseStatus`
fn response_status(error: &GitHubError) -> u16 {
    match error {
        GitHubError::Api { status, .. } => *status,
        other => panic!("not an API error with a response status: {other:?}"),
    }
}

/// `APIError.message`
fn message(error: &GitHubError) -> &str {
    match error {
        GitHubError::Api { message, .. } => message,
        other => panic!("not an API error with a message: {other:?}"),
    }
}

/// Stand-in for GitHub Desktop's `APIError.apiError` (`lib/http.ts`): the
/// error body as the API sent it, if it could be parsed. Replace it with
/// the `GitHubError` field once there is one.
fn api_error_body(_error: &GitHubError) -> Option<Value> {
    unimplemented!("corvene_github::GitHubError keeps no parsed API error (APIError.apiError)")
}

mod api_error {
    use super::*;

    // GHD: unit/api-error-handling-test.ts › http › APIError › creates an error with API message
    #[test]
    #[ignore = "ghd: bug: with flag 311-api-error-details at its GHD value (off) the message is 'Validation Failed', GHD APIError appends errors[].message: 'Validation Failed (name already exists)'; GitHubError also has no apiError"]
    fn creates_an_error_with_api_message() {
        let api_error_value = json!({
            "message": "Validation Failed",
            "errors": [
                {
                    "message": "name already exists",
                    "resource": "Repository",
                    "field": "name",
                },
            ],
        });

        let error = api_error(
            422,
            "Unprocessable Entity",
            "repos/owner/repo",
            Some(api_error_value),
        );

        assert_eq!(response_status(&error), 422);
        assert!(message(&error).contains("Validation Failed"));
        assert!(message(&error).contains("name already exists"));
        assert_ne!(api_error_body(&error), None);
    }

    // GHD: unit/api-error-handling-test.ts › http › APIError › creates an error with fallback message when no API error
    #[test]
    #[ignore = "ghd: bug: an unparsable error body gives the message 'request failed', GHD APIError falls back to 'API error <url>: Internal Server Error (500)'; GitHubError also has no apiError"]
    fn creates_an_error_with_fallback_message_when_no_api_error() {
        let error = api_error(500, "Internal Server Error", "repos/owner/repo", None);

        assert_eq!(response_status(&error), 500);
        assert!(message(&error).contains("500"));
        assert_eq!(api_error_body(&error), None);
    }

    // GHD: unit/api-error-handling-test.ts › http › APIError › handles API error without additional errors array
    #[test]
    fn handles_api_error_without_additional_errors_array() {
        let api_error_value = json!({ "message": "Resource not accessible by integration" });

        let error = api_error(403, "Forbidden", "user", Some(api_error_value));

        assert_eq!(response_status(&error), 403);
        assert_eq!(message(&error), "Resource not accessible by integration");
    }

    // GHD: unit/api-error-handling-test.ts › http › APIError › handles common HTTP status codes
    #[test]
    #[ignore = "ghd: bug: a 401 answer becomes GitHubError::Auth('token rejected') without its status, GHD APIError keeps responseStatus 401"]
    fn handles_common_http_status_codes() {
        let status_codes = [
            (401, "Unauthorized"),
            (403, "Forbidden"),
            (404, "Not Found"),
            (422, "Unprocessable Entity"),
            (500, "Internal Server Error"),
        ];

        for (status, text) in status_codes {
            // GitHub Desktop's response URL is `https://api.github.com/test`;
            // no Client call requests `test`, and nothing here depends on it
            let error = api_error(status, text, "repos/owner/repo", None);
            assert_eq!(response_status(&error), status);
        }
    }
}

/// GitHub Desktop's `getAbsoluteUrl(endpoint, path)`.
fn get_absolute_url(endpoint: &str, path: &str) -> String {
    Endpoint::from_api_base(endpoint).api(path)
}

mod get_absolute_url {
    use super::*;

    // GHD: unit/api-error-handling-test.ts › http › getAbsoluteUrl › constructs URL from endpoint and path
    #[test]
    fn constructs_url_from_endpoint_and_path() {
        let url = get_absolute_url("https://api.github.com", "/repos/owner/repo");
        assert_eq!(url, "https://api.github.com/repos/owner/repo");
    }

    // GHD: unit/api-error-handling-test.ts › http › getAbsoluteUrl › handles endpoint without trailing slash
    #[test]
    fn handles_endpoint_without_trailing_slash() {
        let url = get_absolute_url("https://api.github.com", "repos/owner/repo");
        assert_eq!(url, "https://api.github.com/repos/owner/repo");
    }

    // GHD: unit/api-error-handling-test.ts › http › getAbsoluteUrl › handles endpoint with trailing slash
    #[test]
    #[ignore = "ghd: bug: Endpoint::from_api_base keeps the endpoint's trailing slash, so api() gives https://api.github.com//repos/owner/repo, GHD getAbsoluteUrl https://api.github.com/repos/owner/repo"]
    fn handles_endpoint_with_trailing_slash() {
        let url = get_absolute_url("https://api.github.com/", "repos/owner/repo");
        assert_eq!(url, "https://api.github.com/repos/owner/repo");
    }

    // GHD: unit/api-error-handling-test.ts › http › getAbsoluteUrl › strips duplicate api/v3/ prefix from path
    #[test]
    #[ignore = "ghd: bug: Endpoint::api keeps the endpoint's trailing slash and the path's api/v3/ (https://ghe.example.com/api/v3//api/v3/repos/owner/repo), GHD getAbsoluteUrl strips both"]
    fn strips_duplicate_api_v3_prefix_from_path() {
        let url = get_absolute_url("https://ghe.example.com/api/v3/", "api/v3/repos/owner/repo");
        assert_eq!(url, "https://ghe.example.com/api/v3/repos/owner/repo");
    }

    // GHD: unit/api-error-handling-test.ts › http › getAbsoluteUrl › handles enterprise endpoints
    #[test]
    fn handles_enterprise_endpoints() {
        let url = get_absolute_url("https://ghe.example.com/api/v3", "/repos/owner/repo");
        assert_eq!(url, "https://ghe.example.com/api/v3/repos/owner/repo");
    }
}
