//! Port of GitHub Desktop's `app/test/unit/ssh-test.ts`.
//!
//! GitHub Desktop's `parseAddSSHHostPrompt(prompt)` (`lib/ssh/ssh.ts`)
//! reads ssh's "The authenticity of host … can't be established" prompt,
//! which reaches its askpass trampoline through `SSH_ASKPASS`, for the
//! `AddSSHHost` dialog. It is `corvene_git::parse_add_ssh_host_prompt`
//! (`AddSshHostInfo` for its result), which Corvene's askpass helper
//! (`crates/corvene/src/askpass.rs`) uses to recognise the question.

use corvene_git::{AddSshHostInfo, parse_add_ssh_host_prompt};

// GHD: unit/ssh-test.ts › SSH › parsing SSH prompts › extracts info from github.com host key fingerprint
#[test]
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
