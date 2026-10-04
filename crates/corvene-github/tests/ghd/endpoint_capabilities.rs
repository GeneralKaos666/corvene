//! Port of GitHub Desktop's `app/test/unit/endpoint-capabilities-test.ts`.
//!
//! GitHub Desktop's `endpointSatisfies(constraint, getVersion)(endpoint)`
//! (`lib/endpoint-capabilities.ts`) decides whether an endpoint (GitHub.com,
//! a `*.ghe.com` host or a GitHub Enterprise Server of a given version,
//! assumed to be 3.1.0 when unknown) meets a feature's constraint; the
//! `supports*` capability checks are built on it. Corvene deliberately has
//! no such check: `.docs/deviations.md` › CI check runs reduces "GHES
//! version gating (`lib/endpoint-capabilities.ts`) … to GitHub.com vs.
//! Enterprise" (`corvene_github::Endpoint::is_dotcom`). The cases call a
//! stand-in ([`endpoint_satisfies`]) and are ignored as that deviation.
//! `getDotComAPIEndpoint()` is `Endpoint::github_com().api_base`.

use crate::api_support::get_dot_com_api_endpoint;

/// GitHub Desktop's `VersionConstraint`: `es` is `boolean | string` (a
/// semver range such as `>= 3.1.0`). Only the stand-in reads it, and it
/// does not yet.
#[derive(Clone, Default)]
#[allow(dead_code)]
struct VersionConstraint {
    dotcom: Option<bool>,
    ghe: Option<bool>,
    es: Option<Es>,
}

#[derive(Clone)]
#[allow(dead_code)]
enum Es {
    Supported(bool),
    Range(&'static str),
}

impl From<bool> for Es {
    fn from(value: bool) -> Self {
        Es::Supported(value)
    }
}

impl From<&'static str> for Es {
    fn from(value: &'static str) -> Self {
        Es::Range(value)
    }
}

/// Stand-in for GitHub Desktop's `endpointSatisfies(constraint,
/// getVersion)` (`lib/endpoint-capabilities.ts`): the predicate an
/// endpoint must pass, `getVersion` giving the endpoint's GitHub Enterprise
/// Server version (`None` when unknown). Replace it with the Corvene
/// function if the deviation is ever dropped.
fn endpoint_satisfies(
    constraint: VersionConstraint,
    get_version: impl Fn(&str) -> Option<String>,
) -> impl Fn(&str) -> bool {
    move |_endpoint: &str| {
        let _ = (&constraint, &get_version);
        unimplemented!("Corvene has no endpointSatisfies (GHES version gating)")
    }
}

fn test_dot_com(constraint: bool, endpoint_version: Option<&str>) -> bool {
    test_endpoint(
        &get_dot_com_api_endpoint(),
        VersionConstraint {
            dotcom: Some(constraint),
            ghe: Some(false),
            es: Some(false.into()),
        },
        endpoint_version,
    )
}

fn test_ghes(constraint: impl Into<Es>, endpoint_version: Option<&str>) -> bool {
    test_endpoint(
        "https://ghe.io",
        VersionConstraint {
            dotcom: Some(false),
            ghe: Some(false),
            es: Some(constraint.into()),
        },
        endpoint_version,
    )
}

fn test_ghe_dot_com(constraint: bool) -> bool {
    test_endpoint(
        "https://corp.ghe.com",
        VersionConstraint {
            dotcom: Some(false),
            ghe: Some(constraint),
            es: Some(false.into()),
        },
        None,
    )
}

fn test_endpoint(
    endpoint: &str,
    constraint: VersionConstraint,
    endpoint_version: Option<&str>,
) -> bool {
    // `forceUnwrap('Couldn't parse endpoint version', parse(endpointVersion))`:
    // every version the cases pass is a full `major.minor.patch`
    let version = endpoint_version.map(|v| {
        assert_eq!(v.split('.').count(), 3, "Couldn't parse endpoint version");
        v.to_string()
    });
    endpoint_satisfies(constraint, move |_| version.clone())(endpoint)
}

mod endpoint_satisfies {
    use super::*;

    // GHD: unit/endpoint-capabilities-test.ts › endpoint-capabilities › endpointSatisfies › recognizes github.com
    #[test]
    #[ignore = "ghd: deviation: deviations.md › CI check runs: GHES version gating (lib/endpoint-capabilities.ts endpointSatisfies) is reduced to GitHub.com vs Enterprise"]
    fn recognizes_github_com() {
        assert!(test_dot_com(true, None));
        assert!(!test_dot_com(false, None));
    }

    // GHD: unit/endpoint-capabilities-test.ts › endpoint-capabilities › endpointSatisfies › recognizes GHES
    #[test]
    #[ignore = "ghd: deviation: deviations.md › CI check runs: GHES version gating (lib/endpoint-capabilities.ts endpointSatisfies) is reduced to GitHub.com vs Enterprise"]
    fn recognizes_ghes() {
        assert!(!test_ghes(false, None));
        assert!(test_ghes(true, None));
    }

    // GHD: unit/endpoint-capabilities-test.ts › endpoint-capabilities › endpointSatisfies › recognizes GHAE
    #[test]
    #[ignore = "ghd: deviation: deviations.md › CI check runs: GHES version gating (lib/endpoint-capabilities.ts endpointSatisfies) is reduced to GitHub.com vs Enterprise"]
    fn recognizes_ghae() {
        assert!(!test_ghe_dot_com(false));
        assert!(test_ghe_dot_com(true));
    }

    // If we can't determine the actual version of a GitHub Enterprise Server
    // instance we'll assume it's running the oldest still supported version
    // of GHES. This is defined in the `assumedGHESVersion` constant in
    // endpoint-capabilities.ts and needs to be updated periodically.
    // GHD: unit/endpoint-capabilities-test.ts › endpoint-capabilities › endpointSatisfies › assumes GHES versions
    #[test]
    #[ignore = "ghd: deviation: deviations.md › CI check runs: GHES version gating (lib/endpoint-capabilities.ts endpointSatisfies) is reduced to GitHub.com vs Enterprise"]
    fn assumes_ghes_versions() {
        assert!(!test_ghes(">= 3.1.1", None));
        assert!(test_ghes(">= 3.1.0", None));
    }

    // GHD: unit/endpoint-capabilities-test.ts › endpoint-capabilities › endpointSatisfies › parses semver ranges
    #[test]
    #[ignore = "ghd: deviation: deviations.md › CI check runs: GHES version gating (lib/endpoint-capabilities.ts endpointSatisfies) is reduced to GitHub.com vs Enterprise"]
    fn parses_semver_ranges() {
        assert!(test_ghes(">= 1", Some("1.0.0")));
        assert!(!test_ghes("> 1.0.0", Some("1.0.0")));
        assert!(test_ghes("> 0.9.9", Some("1.0.0")));
    }

    // GHD: unit/endpoint-capabilities-test.ts › endpoint-capabilities › endpointSatisfies › deals with common cases (smoketest)
    #[test]
    #[ignore = "ghd: deviation: deviations.md › CI check runs: GHES version gating (lib/endpoint-capabilities.ts endpointSatisfies) is reduced to GitHub.com vs Enterprise"]
    fn deals_with_common_cases_smoketest() {
        assert!(test_endpoint(
            "https://api.github.com",
            VersionConstraint {
                dotcom: Some(true),
                ghe: Some(false),
                es: Some(">= 3.0.0".into()),
            },
            None,
        ));

        assert!(test_endpoint(
            "https://ghe.io",
            VersionConstraint {
                dotcom: Some(false),
                ghe: Some(false),
                es: Some(">= 3.1.0".into()),
            },
            Some("3.1.0"),
        ));
    }
}
