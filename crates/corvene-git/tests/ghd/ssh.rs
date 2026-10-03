//! Port of GitHub Desktop's `app/test/unit/ssh-test.ts`.
//!
//! GitHub Desktop's `parseAddSSHHostPrompt(prompt)` (`lib/ssh/ssh.ts`)
//! reads ssh's "The authenticity of host … can't be established" prompt,
//! which reaches its askpass trampoline through `SSH_ASKPASS`, for the
//! `AddSSHHost` dialog. Corvene removes `SSH_ASKPASS` from remote commands
//! (`corvene_git::AskpassEnv`) and its askpass helper only answers git's
//! username / password prompts (`crates/corvene/src/askpass.rs`
//! `parse_prompt`), so [`parse_add_ssh_host_prompt`] stands in for it.

/// GitHub Desktop's result of `parseAddSSHHostPrompt`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AddSshHostInfo {
    host: String,
    ip: String,
    fingerprint: String,
    key_type: String,
}

/// Stand-in for GitHub Desktop's `parseAddSSHHostPrompt(prompt)`.
fn parse_add_ssh_host_prompt(_prompt: &str) -> Option<AddSshHostInfo> {
    unimplemented!("Corvene has no parseAddSSHHostPrompt (lib/ssh/ssh.ts)")
}

// GHD: unit/ssh-test.ts › SSH › parsing SSH prompts › extracts info from github.com host key fingerprint
#[test]
#[ignore = "ghd: missing: Corvene does not handle ssh's unknown host prompt, no parseAddSSHHostPrompt (lib/ssh/ssh.ts)"]
fn extracts_info_from_github_com_host_key_fingerprint() {
    let prompt = "The authenticity of host 'github.com (140.82.121.3)' can't be established.
RSA key fingerprint is SHA256:nThbg6kXUpJWGl7E1IGOCspRomTxdCARLviKw6E5SY8.
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

    let info = parse_add_ssh_host_prompt(prompt);

    assert_eq!(
        info,
        Some(AddSshHostInfo {
            host: "github.com".into(),
            ip: "140.82.121.3".into(),
            fingerprint: "SHA256:nThbg6kXUpJWGl7E1IGOCspRomTxdCARLviKw6E5SY8".into(),
            key_type: "RSA".into(),
        })
    );
}

// GHD: unit/ssh-test.ts › SSH › parsing SSH prompts › extracts info from fake host key fingerprint
#[test]
#[ignore = "ghd: missing: Corvene does not handle ssh's unknown host prompt, no parseAddSSHHostPrompt (lib/ssh/ssh.ts)"]
fn extracts_info_from_fake_host_key_fingerprint() {
    let prompt = "The authenticity of host 'my-domain.com (1.2.3.4)' can't be established.
FAKE-TYPE key fingerprint is ThisIsAFakeFingerprintForTestingPurposes.
This key is not known by any other names.
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

    let info = parse_add_ssh_host_prompt(prompt);

    assert_eq!(
        info,
        Some(AddSshHostInfo {
            host: "my-domain.com".into(),
            ip: "1.2.3.4".into(),
            fingerprint: "ThisIsAFakeFingerprintForTestingPurposes".into(),
            key_type: "FAKE-TYPE".into(),
        })
    );
}

// GHD: unit/ssh-test.ts › SSH › parsing SSH prompts › extracts info from fake host key fingerprint when keys of different type are available
#[test]
#[ignore = "ghd: missing: Corvene does not handle ssh's unknown host prompt, no parseAddSSHHostPrompt (lib/ssh/ssh.ts)"]
fn extracts_info_from_fake_host_key_fingerprint_when_keys_of_different_type_are_available() {
    let prompt = "The authenticity of host 'my-domain.com (1.2.3.4)' can't be established
but keys of different type are already known for this host.
FAKE-TYPE key fingerprint is ThisIsAFakeFingerprintForTestingPurposes.
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

    let info = parse_add_ssh_host_prompt(prompt);

    assert_eq!(
        info,
        Some(AddSshHostInfo {
            host: "my-domain.com".into(),
            ip: "1.2.3.4".into(),
            fingerprint: "ThisIsAFakeFingerprintForTestingPurposes".into(),
            key_type: "FAKE-TYPE".into(),
        })
    );
}

// GHD: unit/ssh-test.ts › SSH › parsing SSH prompts › extract info when [fingerprint] option is not present
#[test]
#[ignore = "ghd: missing: Corvene does not handle ssh's unknown host prompt, no parseAddSSHHostPrompt (lib/ssh/ssh.ts)"]
fn extract_info_when_fingerprint_option_is_not_present() {
    let prompt = "The authenticity of host 'my-domain.com (1.2.3.4)' can't be established.
FAKE-TYPE key fingerprint is ThisIsAFakeFingerprintForTestingPurposes.
This key is not known by any other names.
Are you sure you want to continue connecting (yes/no)? ";

    let info = parse_add_ssh_host_prompt(prompt);

    assert_eq!(
        info,
        Some(AddSshHostInfo {
            host: "my-domain.com".into(),
            ip: "1.2.3.4".into(),
            fingerprint: "ThisIsAFakeFingerprintForTestingPurposes".into(),
            key_type: "FAKE-TYPE".into(),
        })
    );
}
