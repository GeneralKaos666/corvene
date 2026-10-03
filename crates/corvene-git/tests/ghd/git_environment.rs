//! Port of GitHub Desktop's `app/test/unit/git/environment-test.ts`.
//!
//! GitHub Desktop's `envForProxy(remoteUrl, env, resolve)`
//! (`lib/git/environment.ts`) adds `http_proxy` / `https_proxy` for a
//! remote operation from the system proxy (`resolveGitProxy`, Electron's
//! `session.resolveProxy`) unless the environment already names one.
//! Corvene resolves no proxy at all (git only sees the proxy settings it
//! inherits or finds in its configuration), so [`env_for_proxy`] stands in
//! for it. A rejected resolver promise is `Err`, an `undefined` result
//! `Ok(None)`.

use std::collections::HashMap;

const HTTP_PROXY_URL: &str = "http://proxy:8888/";
const HTTPS_PROXY_URL: &str = "https://proxy:8888/";

/// What `resolveGitProxy` stands for: the proxy URL for a remote URL.
type Resolver = dyn Fn(&str) -> Result<Option<String>, String>;

fn null_resolver(_url: &str) -> Result<Option<String>, String> {
    Ok(None)
}

fn throwing_resolver(_url: &str) -> Result<Option<String>, String> {
    Err("such error".to_string())
}

fn default_resolver(url: &str) -> Result<Option<String>, String> {
    if url.starts_with("http://") {
        Ok(Some(HTTP_PROXY_URL.to_string()))
    } else if url.starts_with("https://") {
        Ok(Some(HTTPS_PROXY_URL.to_string()))
    } else {
        Ok(None)
    }
}

/// Stand-in for GitHub Desktop's `envForProxy(remoteUrl, env, resolve)`
/// (`lib/git/environment.ts`): the variables to add to a remote
/// operation's environment, `None` for none.
fn env_for_proxy(
    _remote_url: &str,
    _env: &HashMap<String, String>,
    _resolve: &Resolver,
) -> Option<HashMap<String, String>> {
    unimplemented!("Corvene resolves no proxy (envForProxy, lib/git/environment.ts)")
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › sets the correct environment variable based on protocol
#[test]
#[ignore = "ghd: missing: Corvene resolves no system proxy for git (envForProxy, lib/git/environment.ts)"]
fn sets_the_correct_environment_variable_based_on_protocol() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("https_proxy", HTTPS_PROXY_URL)]))
    );

    assert_eq!(
        env_for_proxy("http://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("http_proxy", HTTP_PROXY_URL)]))
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › fails gracefully if resolver throws
#[test]
#[ignore = "ghd: missing: Corvene resolves no system proxy for git (envForProxy, lib/git/environment.ts)"]
fn fails_gracefully_if_resolver_throws() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &throwing_resolver),
        None
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › it doesn't set any variables if resolver returns undefined
#[test]
#[ignore = "ghd: missing: Corvene resolves no system proxy for git (envForProxy, lib/git/environment.ts)"]
fn it_doesnt_set_any_variables_if_resolver_returns_undefined() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &null_resolver),
        None
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › sets the correct environment variable based on protocol #2
#[test]
#[ignore = "ghd: missing: Corvene resolves no system proxy for git (envForProxy, lib/git/environment.ts)"]
fn sets_the_correct_environment_variable_based_on_protocol_2() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("https_proxy", HTTPS_PROXY_URL)]))
    );

    assert_eq!(
        env_for_proxy("http://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("http_proxy", HTTP_PROXY_URL)]))
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › ignores unknown protocols
#[test]
#[ignore = "ghd: missing: Corvene resolves no system proxy for git (envForProxy, lib/git/environment.ts)"]
fn ignores_unknown_protocols() {
    assert_eq!(
        env_for_proxy("ftp://github.com/", &env(&[]), &default_resolver),
        None
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › does not override existing environment variables
#[test]
#[ignore = "ghd: missing: Corvene resolves no system proxy for git (envForProxy, lib/git/environment.ts)"]
fn does_not_override_existing_environment_variables() {
    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("https_proxy", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("HTTPS_PROXY", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "http://github.com/",
            &env(&[("http_proxy", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("ALL_PROXY", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("all_proxy", "foo")]),
            &default_resolver
        ),
        None
    );
}
