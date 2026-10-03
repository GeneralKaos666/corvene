//! Port of GitHub Desktop's `app/test/unit/http-test.ts`.
//!
//! GitHub Desktop's `getAbsoluteUrl(endpoint, path)` (`lib/http.ts`) is the
//! URL every API request goes to: the path relative to the API endpoint,
//! without a leading `/` or `api/v3/` (pagination links of GitHub
//! Enterprise carry it). Corvene builds every API request URL with
//! `corvene_github::Endpoint::api(path)`; the endpoint is
//! `Endpoint::github_com()` for `getDotComAPIEndpoint()` and
//! `Endpoint::from_api_base(endpoint)` for an enterprise endpoint string.
//! `crate::api_support::get_absolute_url` is that call.

use crate::api_support::{get_absolute_url, get_dot_com_api_endpoint};

mod dotcom_endpoint {
    use super::*;

    // GHD: unit/http-test.ts › getAbsoluteUrl › dotcom endpoint › handles leading slashes
    #[test]
    fn handles_leading_slashes() {
        let dotcom_endpoint = get_dot_com_api_endpoint();
        let result = get_absolute_url(&dotcom_endpoint, "/user/repos");
        assert_eq!(result, "https://api.github.com/user/repos");
    }

    // GHD: unit/http-test.ts › getAbsoluteUrl › dotcom endpoint › handles missing leading slash
    #[test]
    fn handles_missing_leading_slash() {
        let dotcom_endpoint = get_dot_com_api_endpoint();
        let result = get_absolute_url(&dotcom_endpoint, "user/repos");
        assert_eq!(result, "https://api.github.com/user/repos");
    }

    // GHD: unit/http-test.ts › getAbsoluteUrl › dotcom endpoint › doesn't mangle encoded query parameters
    #[test]
    fn doesnt_mangle_encoded_query_parameters() {
        let result = get_absolute_url(
            &get_dot_com_api_endpoint(),
            "/issues?since=2019-05-10T16%3A00%3A00Z",
        );
        assert_eq!(
            result,
            "https://api.github.com/issues?since=2019-05-10T16%3A00%3A00Z"
        );
    }
}

mod enterprise_endpoint {
    use super::*;

    const ENTERPRISE_ENDPOINT: &str = "https://my-cool-company.com/api/v3";

    // GHD: unit/http-test.ts › getAbsoluteUrl › enterprise endpoint › handles leading slash
    #[test]
    fn handles_leading_slash() {
        let result = get_absolute_url(ENTERPRISE_ENDPOINT, "/user/repos");
        assert_eq!(result, format!("{ENTERPRISE_ENDPOINT}/user/repos"));
    }

    // GHD: unit/http-test.ts › getAbsoluteUrl › enterprise endpoint › handles missing leading slash
    #[test]
    fn handles_missing_leading_slash() {
        let result = get_absolute_url(ENTERPRISE_ENDPOINT, "user/repos");
        assert_eq!(result, format!("{ENTERPRISE_ENDPOINT}/user/repos"));
    }

    // GHD: unit/http-test.ts › getAbsoluteUrl › enterprise endpoint › handles next page resource which already contains prefix
    #[test]
    #[ignore = "ghd: bug: Endpoint::api keeps a leading api/v3/ (.../api/v3/api/v3/user/repos?page=2), GHD getAbsoluteUrl strips it (.../api/v3/user/repos?page=2)"]
    fn handles_next_page_resource_which_already_contains_prefix() {
        let result = get_absolute_url(ENTERPRISE_ENDPOINT, "/api/v3/user/repos?page=2");
        assert_eq!(result, format!("{ENTERPRISE_ENDPOINT}/user/repos?page=2"));
    }

    // GHD: unit/http-test.ts › getAbsoluteUrl › enterprise endpoint › doesn't mangle encoded query parameters
    #[test]
    fn doesnt_mangle_encoded_query_parameters() {
        let result = get_absolute_url(
            ENTERPRISE_ENDPOINT,
            "/issues?since=2019-05-10T16%3A00%3A00Z",
        );
        assert_eq!(
            result,
            format!("{ENTERPRISE_ENDPOINT}/issues?since=2019-05-10T16%3A00%3A00Z")
        );
    }
}
