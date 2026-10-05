//! Commit signature verdicts from GitHub (`1214-commit-signatures`; GHD
//! has none): the GraphQL `Commit.signature` of up to 100 commits per
//! query, about one point of the 5,000 an hour, where REST would spend a
//! request per commit.

use std::collections::HashMap;

use corvene_models::{GitHubSignature, SignatureKind};
use serde::Deserialize;

use crate::api::Client;
use crate::error::Result;

/// Commits per query.
pub const BATCH: usize = 100;

const SIGNATURE_FIELDS: &str = "__typename isValid state wasSignedByGitHub signer { login } \
     ... on GpgSignature { keyId } ... on SshSignature { keyFingerprint }";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiSignature {
    #[serde(rename = "__typename")]
    typename: String,
    is_valid: bool,
    state: String,
    #[serde(default)]
    was_signed_by_git_hub: bool,
    signer: Option<Login>,
    key_id: Option<String>,
    key_fingerprint: Option<String>,
}

#[derive(Deserialize)]
struct Login {
    login: String,
}

#[derive(Deserialize)]
struct ApiCommit {
    signature: Option<ApiSignature>,
}

#[derive(Deserialize)]
struct Data {
    repository: Option<HashMap<String, Option<ApiCommit>>>,
}

impl Client {
    /// GitHub's verdict on each of `shas` in `owner/name`. A commit GitHub
    /// does not have is missing from the map; an unsigned one maps to
    /// `None`.
    pub fn commit_signatures(
        &self,
        owner: &str,
        name: &str,
        shas: &[String],
    ) -> Result<HashMap<String, Option<GitHubSignature>>> {
        let mut out = HashMap::new();
        let shas: Vec<&String> = shas
            .iter()
            // a `GitObjectID` is a full id; one bad literal fails the whole query
            .filter(|s| matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .collect();
        for chunk in shas.chunks(BATCH) {
            let data: Data = self.post_graphql(
                &query(chunk),
                &serde_json::json!({ "owner": owner, "name": name }),
            )?;
            out.extend(parse(chunk, data));
        }
        Ok(out)
    }
}

fn query(shas: &[&String]) -> String {
    let mut q = String::from(
        "query($owner: String!, $name: String!) { repository(owner: $owner, name: $name) {",
    );
    for (i, sha) in shas.iter().enumerate() {
        q.push_str(&format!(
            " c{i}: object(oid: \"{sha}\") {{ ... on Commit {{ signature {{ {SIGNATURE_FIELDS} }} }} }}"
        ));
    }
    q.push_str(" } }");
    q
}

fn parse(shas: &[&String], data: Data) -> HashMap<String, Option<GitHubSignature>> {
    let Some(mut objects) = data.repository else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (i, sha) in shas.iter().enumerate() {
        let Some(Some(commit)) = objects.remove(&format!("c{i}")) else {
            continue;
        };
        out.insert((*sha).clone(), commit.signature.map(signature));
    }
    out
}

fn signature(s: ApiSignature) -> GitHubSignature {
    let (state, reason) = GitHubSignature::classify(s.is_valid, &s.state);
    GitHubSignature {
        state,
        reason,
        kind: match s.typename.as_str() {
            "GpgSignature" => Some(SignatureKind::Gpg),
            "SshSignature" => Some(SignatureKind::Ssh),
            "SmimeSignature" => Some(SignatureKind::X509),
            _ => None,
        },
        github_state: s.state,
        signer_login: s.signer.map(|l| l.login),
        key: s.key_id.or(s.key_fingerprint),
        was_signed_by_github: s.was_signed_by_git_hub,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::{SignatureReason, SignatureState};

    #[test]
    fn parses_aliased_objects() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let c = "c".repeat(40);
        let d = "d".repeat(40);
        let shas = [&a, &b, &c, &d];
        let data: Data = serde_json::from_str(
            r#"{"repository": {
                "c0": {"signature": {"__typename": "GpgSignature", "isValid": true,
                    "state": "VALID", "wasSignedByGitHub": true,
                    "signer": {"login": "web-flow"}, "keyId": "B5690EEEBB952194"}},
                "c1": {"signature": {"__typename": "SshSignature", "isValid": false,
                    "state": "UNKNOWN_KEY", "wasSignedByGitHub": false,
                    "signer": null, "keyFingerprint": "SHA256:abc"}},
                "c2": {"signature": null},
                "c3": null
            }}"#,
        )
        .unwrap();
        let out = parse(&shas, data);
        let first = out[&a].as_ref().unwrap();
        assert_eq!(first.state, SignatureState::Verified);
        assert_eq!(first.kind, Some(SignatureKind::Gpg));
        assert_eq!(first.signer_login.as_deref(), Some("web-flow"));
        assert!(first.was_signed_by_github);
        let second = out[&b].as_ref().unwrap();
        assert_eq!(second.state, SignatureState::Unverified);
        assert_eq!(second.reason, Some(SignatureReason::UnknownKey));
        assert_eq!(second.key.as_deref(), Some("SHA256:abc"));
        assert_eq!(out[&c], None);
        assert!(!out.contains_key(&d));
    }

    #[test]
    fn one_alias_per_commit() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let q = query(&[&a, &b]);
        assert!(q.contains(&format!("c0: object(oid: \"{a}\")")));
        assert!(q.contains(&format!("c1: object(oid: \"{b}\")")));
    }
}
