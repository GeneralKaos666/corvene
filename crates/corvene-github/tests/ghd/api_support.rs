//! Helpers shared by `corvene-github`'s ports of GitHub Desktop's API tests:
//! the `lib/api.ts` / `lib/http.ts` URL helpers, on Corvene's
//! `corvene_github::Endpoint`. The local HTTP server those tests answer
//! from is `corvene_test_support::http`.

use corvene_github::Endpoint;

/// GitHub Desktop's `getDotComAPIEndpoint()` (`lib/api.ts`):
/// `Endpoint::github_com().api_base`.
pub fn get_dot_com_api_endpoint() -> String {
    Endpoint::github_com().api_base
}

/// GitHub Desktop's `getAbsoluteUrl(endpoint, path)` (`lib/http.ts`): the
/// URL Corvene builds for every API request,
/// `Endpoint::from_api_base(endpoint).api(path)`.
pub fn get_absolute_url(endpoint: &str, path: &str) -> String {
    Endpoint::from_api_base(endpoint).api(path)
}
