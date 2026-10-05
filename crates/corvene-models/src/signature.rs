//! Commit signatures (`1214-commit-signatures`; Corvene addition, GHD
//! `ui/history/commit-summary.tsx` shows none): what a commit's `gpgsig`
//! header holds and what local git and GitHub make of it.

use serde::{Deserialize, Serialize};

/// The armour of a commit's `gpgsig` header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignatureKind {
    Gpg,
    Ssh,
    X509,
}

impl SignatureKind {
    pub fn label(self) -> &'static str {
        match self {
            SignatureKind::Gpg => "GPG",
            SignatureKind::Ssh => "SSH",
            SignatureKind::X509 => "X.509",
        }
    }

    /// The kind of an armoured signature (`-----BEGIN PGP SIGNATURE-----`,
    /// `-----BEGIN SSH SIGNATURE-----`, gpgsm's `-----BEGIN SIGNED MESSAGE-----`).
    pub fn from_armor(signature: &[u8]) -> Option<Self> {
        let start = signature
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(signature.len());
        let s = &signature[start..];
        if s.starts_with(b"-----BEGIN PGP") {
            Some(SignatureKind::Gpg)
        } else if s.starts_with(b"-----BEGIN SSH SIGNATURE") {
            Some(SignatureKind::Ssh)
        } else if s.starts_with(b"-----BEGIN SIGNED MESSAGE") {
            Some(SignatureKind::X509)
        } else if s.is_empty() {
            None
        } else {
            // an armour git does not know still means "signed"
            Some(SignatureKind::Gpg)
        }
    }
}

/// The verdict on a signature, as the badge shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureState {
    Verified,
    /// Signed with a good signature, but the key is unknown, untrusted,
    /// expired or revoked.
    Unverified,
    Bad,
    /// Signed, but nothing here could check it (no allowed signers file,
    /// verifier missing, GitHub's verifier failing).
    CantVerify,
    Unsigned,
}

impl SignatureState {
    pub fn label(self) -> &'static str {
        match self {
            SignatureState::Verified => "Verified",
            SignatureState::Unverified => "Unverified",
            SignatureState::Bad => "Bad signature",
            SignatureState::CantVerify => "Can't verify",
            SignatureState::Unsigned => "Unsigned",
        }
    }
}

/// Why a signature is not [`SignatureState::Verified`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignatureReason {
    UnknownKey,
    Untrusted,
    ExpiredSignature,
    ExpiredKey,
    RevokedKey,
    /// SSH signatures need `gpg.ssh.allowedSignersFile`.
    NoAllowedSigners,
    /// `gpg`, `gpgsm` or `ssh-keygen` could not be run.
    ToolMissing(String),
    /// The verifier gave up (timed out or failed without saying why).
    Unreadable,
    /// GitHub's `GitSignatureState` for anything above does not cover.
    GitHub(String),
}

impl SignatureReason {
    pub fn describe(&self) -> String {
        match self {
            SignatureReason::UnknownKey => "The signing key is not known".into(),
            SignatureReason::Untrusted => "The signing key is not trusted".into(),
            SignatureReason::ExpiredSignature => "The signature has expired".into(),
            SignatureReason::ExpiredKey => "The signing key has expired".into(),
            SignatureReason::RevokedKey => "The signing key has been revoked".into(),
            SignatureReason::NoAllowedSigners => {
                "Set gpg.ssh.allowedSignersFile to verify SSH signatures".into()
            }
            SignatureReason::ToolMissing(tool) => format!("{tool} could not be run"),
            SignatureReason::Unreadable => "The signature could not be checked".into(),
            SignatureReason::GitHub(state) => github_state_description(state).into(),
        }
    }
}

/// GitHub's description of a `GitSignatureState`.
fn github_state_description(state: &str) -> &'static str {
    match state {
        "BAD_CERT" => "The signing certificate or its chain could not be verified",
        "BAD_EMAIL" => "The email used for signing is invalid",
        "EXPIRED_KEY" => "The signing key has expired",
        "GPGVERIFY_ERROR" | "GPGVERIFY_UNAVAILABLE" => "GitHub's verification service failed",
        "INVALID" => "The signature is invalid",
        "MALFORMED_SIG" => "The signature is malformed",
        "NOT_SIGNING_KEY" => "The key is not allowed to sign",
        "NO_USER" => "The signing email is not known to GitHub",
        "OCSP_ERROR" => "The certificate revocation check failed",
        "OCSP_PENDING" => "The certificate revocation check is pending",
        "OCSP_REVOKED" => "A certificate in the chain has been revoked",
        "UNKNOWN_KEY" => "The signing key is not known to GitHub",
        "UNKNOWN_SIG_TYPE" => "GitHub does not know this signature type",
        "UNSIGNED" => "Unsigned",
        "UNVERIFIED_EMAIL" => "The signing email is not verified on GitHub",
        "VALID" => "Verified by GitHub",
        _ => "GitHub could not verify the signature",
    }
}

/// What local git (`%G?` and friends) says about a commit's signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSignature {
    pub state: SignatureState,
    pub reason: Option<SignatureReason>,
    pub kind: Option<SignatureKind>,
    /// `%GS`
    pub signer: Option<String>,
    /// `%GK`
    pub key: Option<String>,
    /// `%GF`
    pub fingerprint: Option<String>,
    /// `%GP`, when it differs from the subkey's.
    pub primary_fingerprint: Option<String>,
    /// `%GT`
    pub trust: Option<String>,
}

/// What GitHub (the GraphQL `Commit.signature`) says about a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubSignature {
    pub state: SignatureState,
    pub reason: Option<SignatureReason>,
    /// The raw `GitSignatureState`.
    pub github_state: String,
    pub kind: Option<SignatureKind>,
    pub signer_login: Option<String>,
    /// `keyId` (GPG) or `keyFingerprint` (SSH).
    pub key: Option<String>,
    pub was_signed_by_github: bool,
}

impl GitHubSignature {
    /// The badge state for GitHub's `isValid` + `state`.
    pub fn classify(is_valid: bool, state: &str) -> (SignatureState, Option<SignatureReason>) {
        if is_valid {
            return (SignatureState::Verified, None);
        }
        match state {
            "UNSIGNED" => (SignatureState::Unsigned, None),
            "INVALID" | "MALFORMED_SIG" | "BAD_CERT" => (
                SignatureState::Bad,
                Some(SignatureReason::GitHub(state.into())),
            ),
            "UNKNOWN_KEY" => (
                SignatureState::Unverified,
                Some(SignatureReason::UnknownKey),
            ),
            "EXPIRED_KEY" => (
                SignatureState::Unverified,
                Some(SignatureReason::ExpiredKey),
            ),
            "OCSP_REVOKED" => (
                SignatureState::Unverified,
                Some(SignatureReason::RevokedKey),
            ),
            "BAD_EMAIL" | "UNVERIFIED_EMAIL" | "NO_USER" | "NOT_SIGNING_KEY" => (
                SignatureState::Unverified,
                Some(SignatureReason::GitHub(state.into())),
            ),
            _ => (
                SignatureState::CantVerify,
                Some(SignatureReason::GitHub(state.into())),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_kinds() {
        assert_eq!(
            SignatureKind::from_armor(b"-----BEGIN PGP SIGNATURE-----\n\nabc"),
            Some(SignatureKind::Gpg)
        );
        assert_eq!(
            SignatureKind::from_armor(b"-----BEGIN SSH SIGNATURE-----\nabc"),
            Some(SignatureKind::Ssh)
        );
        assert_eq!(
            SignatureKind::from_armor(b" -----BEGIN SIGNED MESSAGE-----\nabc"),
            Some(SignatureKind::X509)
        );
        assert_eq!(SignatureKind::from_armor(b""), None);
    }

    #[test]
    fn github_states() {
        assert_eq!(
            GitHubSignature::classify(true, "VALID").0,
            SignatureState::Verified
        );
        assert_eq!(
            GitHubSignature::classify(false, "INVALID").0,
            SignatureState::Bad
        );
        assert_eq!(
            GitHubSignature::classify(false, "UNKNOWN_KEY"),
            (
                SignatureState::Unverified,
                Some(SignatureReason::UnknownKey)
            )
        );
        assert_eq!(
            GitHubSignature::classify(false, "GPGVERIFY_UNAVAILABLE").0,
            SignatureState::CantVerify
        );
        assert_eq!(
            GitHubSignature::classify(false, "UNSIGNED").0,
            SignatureState::Unsigned
        );
    }
}
