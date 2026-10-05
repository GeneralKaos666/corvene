//! Commit signature verification (`1214-commit-signatures`; Corvene
//! addition, GHD `ui/history/commit-summary.tsx` shows no signature
//! status).
//!
//! Whether a commit is signed comes free with the History walk
//! ([`corvene_models::Commit::signature`], the `gpgsig` header). Verifying
//! runs `gpg`, `gpgsm` or `ssh-keygen` through git, so it happens here, one
//! commit at a time, only when asked.

use std::path::Path;
use std::sync::Arc;

use corvene_models::{LocalSignature, SignatureKind, SignatureReason, SignatureState};

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::{CancelToken, GitCommand};

/// `%G?` status, signer, key, fingerprint, primary key fingerprint, trust.
const FORMAT: &str = "%G?%x00%GS%x00%GK%x00%GF%x00%GP%x00%GT";

/// What git makes of `sha`'s signature, `kind` being the header's armour
/// (`None` for an unsigned commit, which needs no process).
pub fn verify_commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    sha: &str,
    kind: Option<SignatureKind>,
    cancel: Option<CancelToken>,
) -> Result<LocalSignature> {
    if kind.is_none() {
        return Ok(unsigned());
    }
    let mut cmd = GitCommand::new(git)
        .args([
            "-c",
            "log.showSignature=false",
            "show",
            "-s",
            "--no-patch",
            &format!("--format={FORMAT}"),
            sha,
            "--",
        ])
        .current_dir(workdir)
        .allow_any_exit_code();
    if let Some(cancel) = cancel {
        cmd = cmd.cancel_token(cancel);
    }
    let out = cmd.run()?;
    Ok(parse(&out.stdout, &out.stderr, kind))
}

fn unsigned() -> LocalSignature {
    LocalSignature {
        state: SignatureState::Unsigned,
        reason: None,
        kind: None,
        signer: None,
        key: None,
        fingerprint: None,
        primary_fingerprint: None,
        trust: None,
    }
}

/// `git show --format=FORMAT`'s output and stderr as a [`LocalSignature`].
pub fn parse(stdout: &[u8], stderr: &str, kind: Option<SignatureKind>) -> LocalSignature {
    let text = String::from_utf8_lossy(stdout);
    let mut fields = text.trim_end_matches(['\n', '\r']).split('\0');
    let status = fields.next().unwrap_or_default().trim().to_string();
    let mut field = || {
        fields
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "undefined")
            .map(str::to_string)
    };
    let signer = field();
    let key = field();
    let fingerprint = field();
    let primary_fingerprint = field().filter(|p| Some(p) != fingerprint.as_ref());
    let trust = field();

    use SignatureReason as R;
    use SignatureState as S;
    let (state, reason) = if let Some(tool) = missing_tool(stderr, kind) {
        // `ssh-keygen` that cannot run reports `B`; it says nothing about
        // the signature
        (S::CantVerify, Some(R::ToolMissing(tool)))
    } else if kind.is_none() {
        (S::Unsigned, None)
    } else {
        match status.as_str() {
            "G" => (S::Verified, None),
            "U" if kind == Some(SignatureKind::Ssh) && signer.is_none() => {
                (S::Unverified, Some(R::UnknownKey))
            }
            "U" => (S::Unverified, Some(R::Untrusted)),
            "X" => (S::Unverified, Some(R::ExpiredSignature)),
            "Y" => (S::Unverified, Some(R::ExpiredKey)),
            "R" => (S::Unverified, Some(R::RevokedKey)),
            "B" => (S::Bad, None),
            "E" => (S::Unverified, Some(R::UnknownKey)),
            _ if stderr.contains("allowedSignersFile") => {
                (S::CantVerify, Some(R::NoAllowedSigners))
            }
            _ => (S::CantVerify, Some(R::Unreadable)),
        }
    };
    LocalSignature {
        state,
        reason,
        kind,
        signer,
        key,
        fingerprint,
        primary_fingerprint,
        trust,
    }
}

/// The verifier git could not start (`fatal: cannot exec '…'` /
/// `cannot run gpg`), named for the signature's kind.
fn missing_tool(stderr: &str, kind: Option<SignatureKind>) -> Option<String> {
    let failed = stderr
        .lines()
        .any(|l| l.contains("cannot exec") || l.contains("cannot run"));
    failed.then(|| {
        match kind {
            Some(SignatureKind::Ssh) => "ssh-keygen",
            Some(SignatureKind::X509) => "gpgsm",
            _ => "gpg",
        }
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn have(tool: &str) -> bool {
        Command::new(tool)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success() || !o.stderr.is_empty())
    }

    struct Fixture {
        dir: tempfile::TempDir,
        git: Arc<GitBinary>,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let f = Fixture {
                dir,
                git: Arc::new(crate::find_git().unwrap()),
            };
            std::fs::create_dir(f.repo()).unwrap();
            f.run(&["init", "-q", "-b", "main"], &[]);
            f.run(&["config", "commit.gpgsign", "false"], &[]);
            f
        }

        fn repo(&self) -> std::path::PathBuf {
            self.dir.path().join("r")
        }

        fn run(&self, args: &[&str], env: &[(&str, &str)]) -> String {
            let out = Command::new("git")
                .args(args)
                .current_dir(self.repo())
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .envs(env.iter().copied())
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        }

        fn commit(&self, message: &str, sign: &[&str], env: &[(&str, &str)]) -> String {
            let mut args = sign.to_vec();
            args.extend(["commit", "-q", "--allow-empty", "-m", message]);
            self.run(&args, env);
            self.run(&["rev-parse", "HEAD"], &[])
        }

        fn verify(&self, sha: &str, kind: SignatureKind, env: &[(&str, &str)]) -> LocalSignature {
            crate::process::with_env(env, || {
                verify_commit(self.git.clone(), &self.repo(), sha, Some(kind), None).unwrap()
            })
        }

        /// A tampered copy of `sha` (message changed, signature kept).
        fn tamper(&self, sha: &str) -> String {
            let raw = self.run(&["cat-file", "commit", sha], &[]);
            let file = self.dir.path().join("tampered");
            std::fs::write(&file, raw.replacen("\n\nsigned", "\n\ntampered", 1) + "\n").unwrap();
            self.run(
                &["hash-object", "-t", "commit", "-w", file.to_str().unwrap()],
                &[],
            )
        }
    }

    #[test]
    fn unsigned_needs_no_process() {
        let f = Fixture::new();
        let sha = f.commit("plain", &[], &[]);
        let sig = verify_commit(f.git.clone(), &f.repo(), &sha, None, None).unwrap();
        assert_eq!(sig.state, SignatureState::Unsigned);
    }

    #[test]
    fn ssh_signatures() {
        if !have("ssh-keygen") {
            return;
        }
        let f = Fixture::new();
        let key = f.dir.path().join("key");
        let status = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "t@example.com", "-f"])
            .arg(&key)
            .status()
            .unwrap();
        assert!(status.success());
        let public = format!("{}.pub", key.display());
        let sign = [
            "-c",
            "gpg.format=ssh",
            "-c",
            &format!("user.signingkey={public}"),
            "-c",
            "commit.gpgsign=true",
        ];
        let sha = f.commit("signed", &sign, &[]);
        let sha_bad = f.tamper(&sha);

        // no allowed signers file: say so rather than "bad"
        let sig = f.verify(&sha, SignatureKind::Ssh, &[]);
        assert_eq!(sig.state, SignatureState::CantVerify, "{sig:?}");
        assert_eq!(sig.reason, Some(SignatureReason::NoAllowedSigners));

        let allowed = f.dir.path().join("allowed");
        let line = std::fs::read_to_string(&public).unwrap();
        std::fs::write(&allowed, format!("t@example.com {line}")).unwrap();
        f.run(
            &[
                "config",
                "gpg.ssh.allowedSignersFile",
                allowed.to_str().unwrap(),
            ],
            &[],
        );
        let sig = f.verify(&sha, SignatureKind::Ssh, &[]);
        assert_eq!(sig.state, SignatureState::Verified, "{sig:?}");
        assert_eq!(sig.signer.as_deref(), Some("t@example.com"));
        assert!(sig.fingerprint.as_deref().unwrap().starts_with("SHA256:"));

        let sig = f.verify(&sha_bad, SignatureKind::Ssh, &[]);
        assert_eq!(sig.state, SignatureState::Bad, "{sig:?}");

        // a key the allowed signers file does not list
        std::fs::write(&allowed, "").unwrap();
        let sig = f.verify(&sha, SignatureKind::Ssh, &[]);
        assert_eq!(sig.state, SignatureState::Unverified, "{sig:?}");
        assert_eq!(sig.reason, Some(SignatureReason::UnknownKey));

        // `ssh-keygen` that cannot run is not a bad signature
        f.run(
            &["config", "gpg.ssh.program", "/nonexistent/ssh-keygen"],
            &[],
        );
        let sig = f.verify(&sha, SignatureKind::Ssh, &[]);
        assert_eq!(sig.state, SignatureState::CantVerify, "{sig:?}");
        assert_eq!(
            sig.reason,
            Some(SignatureReason::ToolMissing("ssh-keygen".into()))
        );
    }

    #[test]
    fn gpg_signatures() {
        if !have("gpg") {
            return;
        }
        let f = Fixture::new();
        let home = f.dir.path().join("g");
        let empty = f.dir.path().join("e");
        for dir in [&home, &empty] {
            std::fs::create_dir(dir).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        let home_s = home.to_str().unwrap();
        let generated = Command::new("gpg")
            .env("GNUPGHOME", home_s)
            .args([
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--quick-gen-key",
                "T <t@example.com>",
                "ed25519",
                "sign",
                "never",
            ])
            .output()
            .unwrap();
        if !generated.status.success() {
            // no gpg-agent here (sandboxed CI); nothing to sign with
            eprintln!(
                "skipped: gpg --quick-gen-key failed: {}",
                String::from_utf8_lossy(&generated.stderr)
            );
            return;
        }
        let env = [("GNUPGHOME", home_s)];
        let sha = f.commit(
            "signed",
            &[
                "-c",
                "gpg.format=openpgp",
                "-c",
                "user.signingkey=t@example.com",
                "-c",
                "commit.gpgsign=true",
            ],
            &env,
        );
        let sha_bad = f.tamper(&sha);

        let sig = f.verify(&sha, SignatureKind::Gpg, &env);
        assert_eq!(sig.state, SignatureState::Verified, "{sig:?}");
        assert_eq!(sig.signer.as_deref(), Some("T <t@example.com>"));
        assert!(sig.key.is_some() && sig.fingerprint.is_some());

        let sig = f.verify(&sha_bad, SignatureKind::Gpg, &env);
        assert_eq!(sig.state, SignatureState::Bad, "{sig:?}");

        let sig = f.verify(
            &sha,
            SignatureKind::Gpg,
            &[("GNUPGHOME", empty.to_str().unwrap())],
        );
        assert_eq!(sig.state, SignatureState::Unverified, "{sig:?}");
        assert_eq!(sig.reason, Some(SignatureReason::UnknownKey));
        assert!(sig.key.is_some());

        f.run(&["config", "gpg.program", "/nonexistent/gpg"], &[]);
        let sig = f.verify(&sha, SignatureKind::Gpg, &env);
        assert_eq!(sig.reason, Some(SignatureReason::ToolMissing("gpg".into())));

        for home in [home_s, empty.to_str().unwrap()] {
            let _ = Command::new("gpgconf")
                .env("GNUPGHOME", home)
                .args(["--kill", "gpg-agent"])
                .status();
        }
    }

    #[test]
    fn parses_gpg_output() {
        let sig = parse(
            b"G\0T <t@example.com>\0ABCD\0FFFF\0FFFF\0ultimate\n",
            "",
            Some(SignatureKind::Gpg),
        );
        assert_eq!(sig.state, SignatureState::Verified);
        assert_eq!(sig.primary_fingerprint, None);
        assert_eq!(sig.trust.as_deref(), Some("ultimate"));
        let sig = parse(b"E\0\0ABCD\0\0\0undefined\n", "", Some(SignatureKind::Gpg));
        assert_eq!(sig.state, SignatureState::Unverified);
        assert_eq!(sig.trust, None);
        let sig = parse(b"N\0\0\0\0\0undefined\n", "", Some(SignatureKind::X509));
        assert_eq!(sig.state, SignatureState::CantVerify);
    }
}
