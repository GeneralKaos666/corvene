//! Port of GitHub Desktop's `app/test/unit/email-test.ts`.
//!
//! GitHub Desktop's `lookupPreferredEmail(account)` and
//! `isAttributableEmailFor(account, email)` (`lib/email.ts`) are
//! `corvene_models::Account::preferred_email` and
//! `Account::is_attributable_email`.
//!
//! GitHub Desktop's `Account` holds the API's addresses as they come from
//! `GET /user/emails` (`IAPIEmail`: `email`, `primary`, `verified`,
//! `visibility`); Corvene's `Account` keeps the verified addresses, primary
//! first, and whether the primary one is private (`private_primary_email`),
//! which `corvene_github::Client::current_user` works out from that same
//! answer. [`new_account`] (GitHub Desktop's `new Account(login, endpoint,
//! token, emails, avatarURL, id, name, plan)`) therefore serves the case's
//! `/user` and `/user/emails` answers from a local HTTP server to
//! `current_user`, so the conversion is Corvene's own, and then gives the
//! account the case's endpoint (it would be the local server's otherwise).
//! GitHub Desktop's id `-1` (no id) is `0`: Corvene's ids are unsigned, and
//! no expected value depends on the id in those cases.
//!
//! `getDotComAPIEndpoint()` is `Endpoint::github_com().api_base`,
//! `getEnterpriseAPIURL(url)` is `Endpoint::enterprise(url, false).api_base`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use corvene_github::{Client, Endpoint};
use corvene_models::Account;
use serde_json::json;

/// GitHub Desktop's `IAPIEmail`.
struct ApiEmail {
    email: &'static str,
    primary: bool,
    verified: bool,
    visibility: Option<&'static str>,
}

/// GitHub Desktop's `getDotComAPIEndpoint()`.
fn get_dot_com_api_endpoint() -> String {
    Endpoint::github_com().api_base
}

/// GitHub Desktop's `getEnterpriseAPIURL(endpoint)`.
fn get_enterprise_api_url(endpoint: &str) -> String {
    Endpoint::enterprise(endpoint, false)
        .expect("an enterprise address")
        .api_base
}

/// Answer `GET …/user` with `user` and `GET …/user/emails` with `emails`
/// (anything else is a 404) until the test ends; returns the API base.
fn serve_user(user: String, emails: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            let path = request_line.split(' ').nth(1).unwrap_or_default();
            let path = path.split('?').next().unwrap_or_default();
            let (status, body) = if path.ends_with("/user/emails") {
                ("200 OK", emails.as_str())
            } else if path.ends_with("/user") {
                ("200 OK", user.as_str())
            } else {
                ("404 Not Found", r#"{"message":"Not Found"}"#)
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    format!("http://127.0.0.1:{port}/api/v3")
}

/// GitHub Desktop's `new Account(login, endpoint, token, emails, avatarURL,
/// id, name, plan)`, as Corvene builds it from the API (see the module doc).
#[allow(clippy::too_many_arguments)]
fn new_account(
    login: &str,
    endpoint: &str,
    token: &str,
    emails: &[ApiEmail],
    avatar_url: &str,
    id: u64,
    name: &str,
    plan: &str,
) -> Account {
    let user = json!({
        "id": id,
        "login": login,
        "name": name,
        "avatar_url": avatar_url,
        "plan": { "name": plan },
    })
    .to_string();
    let emails = serde_json::Value::Array(
        emails
            .iter()
            .map(|e| {
                json!({
                    "email": e.email,
                    "primary": e.primary,
                    "verified": e.verified,
                    "visibility": e.visibility,
                })
            })
            .collect(),
    )
    .to_string();
    let base = serve_user(user, emails);
    let mut account = Client::new(Endpoint::from_api_base(&base), token)
        .current_user(Vec::new())
        .expect("current_user");
    account.endpoint = endpoint.to_string();
    account
}

mod lookup_preferred_email {
    use super::*;

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › returns a stealth email address for empty list
    #[test]
    fn returns_a_stealth_email_address_for_empty_list() {
        let account = new_account(
            "shiftkey",
            &get_dot_com_api_endpoint(),
            "",
            &[],
            "",
            1234,
            "Caps Lock",
            "free",
        );

        assert_eq!(
            account.preferred_email(),
            "1234+shiftkey@users.noreply.github.com"
        );
    }

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › returns a stealth email address for empty list from GHES
    #[test]
    fn returns_a_stealth_email_address_for_empty_list_from_ghes() {
        let account = new_account(
            "shiftkey",
            "https://github.example.com/api/v3",
            "",
            &[],
            "",
            1234,
            "Caps Lock",
            "free",
        );

        assert_eq!(
            account.preferred_email(),
            "1234+shiftkey@users.noreply.github.example.com"
        );
    }

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › returns the primary if it has public visibility
    #[test]
    fn returns_the_primary_if_it_has_public_visibility() {
        let emails = [
            ApiEmail {
                email: "shiftkey@example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "shiftkey@users.noreply.github.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "my-primary-email@example.com",
                primary: true,
                verified: true,
                visibility: Some("public"),
            },
        ];

        let account = new_account(
            "shiftkey",
            &get_dot_com_api_endpoint(),
            "",
            &emails,
            "",
            0,
            "Caps Lock",
            "free",
        );

        assert_eq!(account.preferred_email(), "my-primary-email@example.com");
    }

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › returns the primary if it has null visibility
    #[test]
    fn returns_the_primary_if_it_has_null_visibility() {
        let emails = [
            ApiEmail {
                email: "shiftkey@example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "shiftkey@users.noreply.github.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "my-primary-email@example.com",
                primary: true,
                verified: true,
                visibility: None,
            },
        ];

        let account = new_account(
            "shiftkey",
            &get_dot_com_api_endpoint(),
            "",
            &emails,
            "",
            0,
            "Caps Lock",
            "free",
        );

        assert_eq!(account.preferred_email(), "my-primary-email@example.com");
    }

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › returns the noreply if there is no public address
    #[test]
    fn returns_the_noreply_if_there_is_no_public_address() {
        let emails = [
            ApiEmail {
                email: "shiftkey@example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "shiftkey@users.noreply.github.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "my-primary-email@example.com",
                primary: true,
                verified: true,
                visibility: Some("private"),
            },
        ];

        let account = new_account(
            "shiftkey",
            &get_dot_com_api_endpoint(),
            "",
            &emails,
            "",
            0,
            "Caps Lock",
            "free",
        );

        assert_eq!(
            account.preferred_email(),
            "shiftkey@users.noreply.github.com"
        );
    }

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › returns the noreply if there is no public address for GitHub Enterprise as well
    #[test]
    fn returns_the_noreply_if_there_is_no_public_address_for_github_enterprise_as_well() {
        let emails = [
            ApiEmail {
                email: "shiftkey@example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "shiftkey@users.noreply.github.example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "my-primary-email@example.com",
                primary: true,
                verified: true,
                visibility: Some("private"),
            },
        ];

        let account = new_account(
            "shiftkey",
            &get_enterprise_api_url("https://github.example.com"),
            "",
            &emails,
            "",
            0,
            "Caps Lock",
            "free",
        );

        assert_eq!(
            account.preferred_email(),
            "shiftkey@users.noreply.github.example.com"
        );
    }

    // GHD: unit/email-test.ts › emails › lookupPreferredEmail › uses first email if nothing special found
    #[test]
    fn uses_first_email_if_nothing_special_found() {
        let emails = [
            ApiEmail {
                email: "shiftkey@example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "github-primary@example.com",
                primary: false,
                verified: true,
                visibility: None,
            },
        ];

        let account = new_account(
            "shiftkey",
            &get_dot_com_api_endpoint(),
            "",
            &emails,
            "",
            0,
            "Caps Lock",
            "free",
        );

        assert_eq!(account.preferred_email(), "shiftkey@example.com");
    }
}

mod is_attributable_email_for {
    use super::*;

    // GHD: unit/email-test.ts › emails › isAttributableEmailFor › considers all email addresses on the account as well as legacy and modern stealth emails
    #[test]
    fn considers_all_email_addresses_on_the_account_as_well_as_legacy_and_modern_stealth_emails() {
        let emails = [
            ApiEmail {
                email: "personal@gmail.com",
                primary: true,
                verified: true,
                visibility: Some("public"),
            },
            ApiEmail {
                email: "company@github.com",
                primary: false,
                verified: true,
                visibility: None,
            },
            ApiEmail {
                email: "niik@users.noreply.github.com",
                primary: false,
                verified: true,
                visibility: None,
            },
        ];

        let endpoint = get_dot_com_api_endpoint();
        let account = new_account("niik", &endpoint, "", &emails, "", 123, "", "free");

        assert!(account.is_attributable_email("personal@gmail.com"));
        assert!(account.is_attributable_email("company@github.com"));
        assert!(account.is_attributable_email("niik@users.noreply.github.com"));
        assert!(account.is_attributable_email("123+niik@users.noreply.github.com"));
    }

    // GHD: unit/email-test.ts › emails › isAttributableEmailFor › considers stealth emails when account has no emails
    #[test]
    fn considers_stealth_emails_when_account_has_no_emails() {
        let endpoint = get_dot_com_api_endpoint();
        let account = new_account("niik", &endpoint, "", &[], "", 123, "", "free");

        assert!(account.is_attributable_email("niik@users.noreply.github.com"));
        assert!(account.is_attributable_email("123+niik@users.noreply.github.com"));
    }

    // GHD: unit/email-test.ts › emails › isAttributableEmailFor › considers stealth emails for GitHub Enterprise
    #[test]
    fn considers_stealth_emails_for_github_enterprise() {
        let endpoint = get_dot_com_api_endpoint();
        let account = new_account("niik", &endpoint, "", &[], "", 123, "", "free");

        assert!(account.is_attributable_email("niik@users.noreply.github.com"));
        assert!(account.is_attributable_email("123+niik@users.noreply.github.com"));
    }

    // GHD: unit/email-test.ts › emails › isAttributableEmailFor › considers email adresses in a case-insensitive manner
    #[test]
    fn considers_email_adresses_in_a_case_insensitive_manner() {
        let account = new_account(
            "niik",
            &get_dot_com_api_endpoint(),
            "",
            &[ApiEmail {
                email: "niik@GITHUB.COM",
                verified: true,
                primary: true,
                visibility: Some("public"),
            }],
            "",
            123,
            "",
            "free",
        );

        assert!(account.is_attributable_email("niik@github.com"));
        assert!(account.is_attributable_email("niik@users.noreply.github.com"));
        assert!(account.is_attributable_email("123+niik@users.noreply.github.com"));
    }
}
