//! Port of GitHub Desktop's `app/test/unit/ui/email-attribution-warning-test.tsx`.
//!
//! GitHub Desktop's `GitEmailNotFoundWarning`
//! (`ui/lib/git-email-not-found-warning.tsx`) renders nothing without
//! accounts or with a blank email; otherwise ⚠️ or a green check, "This email
//! address does not match / matches your GitHub account. [Your commits will
//! be wrongly attributed.]", a "Learn more." link button when it does not
//! match, and the same sentence in an `AriaLiveContainer` for screen
//! readers. Whether it matches is `isAttributableEmailFor` over the accounts
//! (Corvene: `corvene_core::Account::is_attributable_email`).
//!
//! Corvene draws the warning inline in two GPUI views and has no function
//! returning its content, so [`git_email_not_found_warning`] is a stand-in:
//!
//! - `welcome.rs` `email_not_found_warning(account, email, cx)` (Welcome ›
//!   Configure Git): one account only, GitHub Desktop's copy and link, no
//!   screen-reader live text;
//! - `dialogs/preferences.rs` Git tab (`warn`): "This email address doesn't
//!   match your GitHub account, so your commits will be wrongly attributed."
//!   with a "Learn more" link to `…/setting-your-commit-email-address`, a
//!   case-insensitive comparison with the account emails only (no stealth
//!   addresses), and nothing when the email matches.
//!
//! Replace the stand-in with the Corvene function once there is one and
//! remove the `#[ignore]`s.

use corvene_core::Account;

/// GitHub Desktop's `getDotComAPIEndpoint()`.
const DOTCOM_API_ENDPOINT: &str = "https://api.github.com";

/// The indicator before the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // `Warning`: built by the real GitEmailNotFoundWarning content
enum Indicator {
    /// `<span className="warning-icon">⚠️</span>`
    Warning,
    /// `.green-circle` with a `.check-icon` octicon
    Check,
}

/// The "Learn more." `LinkButton`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LearnMore {
    text: String,
    aria_label: String,
    uri: String,
}

/// What GitHub Desktop's `GitEmailNotFoundWarning` renders.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GitEmailNotFoundWarningContent {
    indicator: Indicator,
    /// `buildScreenReaderMessage(isAttributableEmail)`, shown in the warning
    message: String,
    learn_more: Option<LearnMore>,
    /// The text of the `#git-email-not-found-warning-for-screen-readers`
    /// polite live region (the message, plus `AriaLiveContainer`'s suffix)
    screen_reader_message: String,
}

impl GitEmailNotFoundWarningContent {
    /// `.git-email-not-found-warning`'s `textContent`: the indicator, the
    /// message and the link's text.
    fn text_content(&self) -> String {
        let indicator = match self.indicator {
            Indicator::Warning => "⚠️",
            Indicator::Check => "",
        };
        let link = self.learn_more.as_ref().map_or("", |l| l.text.as_str());
        format!("{indicator}{}{link}", self.message)
    }
}

/// Stand-in for GitHub Desktop's `GitEmailNotFoundWarning` rendered with
/// `accounts` and `email`: `None` when it renders nothing.
fn git_email_not_found_warning(
    _accounts: &[Account],
    _email: &str,
) -> Option<GitEmailNotFoundWarningContent> {
    unimplemented!(
        "Corvene has no GitEmailNotFoundWarning content: welcome.rs and preferences.rs draw it inline (GPUI)"
    )
}

/// GitHub Desktop's `createAccount(email)`: `new Account('mona',
/// getDotComAPIEndpoint(), '', [createEmail(email)], '', 1, 'Mona')`, the
/// email verified, primary and public.
fn create_account(email: &str) -> Account {
    Account {
        endpoint: DOTCOM_API_ENDPOINT.to_string(),
        id: 1,
        login: "mona".to_string(),
        name: Some("Mona".to_string()),
        avatar_url: Some(String::new()),
        emails: vec![email.to_string()],
        scopes: Vec::new(),
        plan: None,
        private_primary_email: false,
    }
}

// GHD: unit/ui/email-attribution-warning-test.tsx › GitEmailNotFoundWarning › renders nothing when there are no accounts or the email is blank
#[test]
#[ignore = "ghd: missing: no GitEmailNotFoundWarning content fn (ui/lib/git-email-not-found-warning.tsx); welcome.rs (one account) and preferences.rs decide it inline in GPUI"]
fn renders_nothing_when_there_are_no_accounts_or_the_email_is_blank() {
    let no_accounts = git_email_not_found_warning(&[], "person@example.com");
    let blank_email = git_email_not_found_warning(&[create_account("mona@example.com")], " ");

    // `container.textContent === ''` and no `.git-email-not-found-warning`
    assert_eq!(no_accounts, None);
    assert_eq!(blank_email, None);
}

// GHD: unit/ui/email-attribution-warning-test.tsx › GitEmailNotFoundWarning › renders a mismatch warning, learn-more link, and screen-reader message
#[test]
#[ignore = "ghd: missing: no GitEmailNotFoundWarning content fn (ui/lib/git-email-not-found-warning.tsx); welcome.rs has GHD's copy, preferences.rs says 'doesn't match your GitHub account, so…' and links setting-your-commit-email-address"]
fn renders_a_mismatch_warning_learn_more_link_and_screen_reader_message() {
    let warning =
        git_email_not_found_warning(&[create_account("mona@example.com")], "other@example.com")
            .expect("a warning");

    let link = warning.learn_more.as_ref().expect("a learn-more link");

    assert!(
        warning
            .text_content()
            .contains("does not match your GitHub account")
    );
    assert_eq!(link.aria_label, "Learn more about commit attribution");
    assert_eq!(
        link.uri,
        "https://docs.github.com/en/github/committing-changes-to-your-project/why-are-my-commits-linked-to-the-wrong-user"
    );
    // `srOnly.getAttribute('aria-live') === 'polite'`: the screen-reader
    // message is the polite live region's text
    assert!(
        warning
            .screen_reader_message
            .starts_with("This email address does not match your GitHub account.")
    );
}

// GHD: unit/ui/email-attribution-warning-test.tsx › GitEmailNotFoundWarning › renders a success indicator without the learn-more link when the email matches
#[test]
#[ignore = "ghd: missing: no GitEmailNotFoundWarning content fn (ui/lib/git-email-not-found-warning.tsx); welcome.rs draws the check inline in GPUI, preferences.rs shows nothing on a match"]
fn renders_a_success_indicator_without_the_learn_more_link_when_the_email_matches() {
    let warning =
        git_email_not_found_warning(&[create_account("mona@example.com")], "mona@example.com")
            .expect("a warning");

    assert_eq!(warning.indicator, Indicator::Check);
    assert!(
        warning
            .text_content()
            .contains("matches your GitHub account")
    );
    assert_eq!(warning.learn_more, None);
}
