//! Port of GitHub Desktop's
//! `app/test/unit/ui/welcome-and-sign-in-wrappers-test.tsx` (the
//! endpoint-entry case is skipped in `tools/ghd-tests/skips/ui2.tsv`).
//!
//! Corvene's counterparts are GPUI views dispatching to `corvene_core`,
//! with nothing to read back, so each case calls a stand-in returning what
//! the GitHub Desktop component shows:
//!
//! - `SignIn` (`ui/lib/sign-in.tsx`) in the `ExistingAccountWarning` step:
//!   "You're already signed in to <host> with the account <login>. If you
//!   continue, you will first be signed out." above the authentication form
//!   (its "Sign in using your browser" link calls
//!   `requestBrowserAuthentication`); in the `Success` step it renders
//!   nothing. Corvene has no existing-account warning: its sign-in dialog
//!   (`crates/corvene-ui/src/dialogs/sign_in.rs`) always starts at the
//!   address or authentication step, and a successful sign-in replaces the
//!   endpoint's account silently (`Dispatcher` sign-in completion,
//!   `accounts.retain(|a| a.endpoint != account.endpoint)` in
//!   `crates/corvene-core/src/dispatcher.rs`). [`sign_in`] takes GitHub
//!   Desktop's sign-in state ([`SignInState`], its `Account` being
//!   `corvene_core::Account`).
//! - `SignInEnterprise` (`ui/welcome/sign-in-enterprise.tsx`): the Welcome
//!   step "Sign in to your GitHub Enterprise" around `SignIn`, nothing
//!   without a sign-in state, Cancel returning to the Start step. Corvene's
//!   Welcome flow (`crates/corvene-ui/src/welcome.rs`) has only Start and
//!   Configure Git: its "Sign in to GitHub Enterprise" button opens the
//!   sign-in dialog (`Popup::SignIn { enterprise: true }`, titled "Sign in
//!   to GitHub Enterprise") instead of an in-flow step. [`sign_in_enterprise`]
//!   is a stand-in.
//! - `ConfigureGit` (`ui/welcome/configure-git.tsx`): "Configure Git", "This
//!   is used to identify the commits you create. …", a Finish button
//!   (`done`) and Cancel returning to Start without `done`. Corvene draws it
//!   inline in `Welcome::configure_git` (Cancel: `Welcome::back_to_start`),
//!   so [`configure_git`] is a stand-in.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs, which is what `fireEvent.click` then
//! checks. GitHub Desktop's `WelcomeStep` is [`WelcomeStep`] (Corvene's
//! `welcome.rs` `Step` is private). Not ported (React DOM only): the
//! `children` buttons `SignIn` renders unchanged.

use corvene_core::Account;

/// GitHub Desktop's `WelcomeStep` (the steps these cases reach).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WelcomeStep {
    Start,
}

/// GitHub Desktop's sign-in store state (`SignInState`, the steps these
/// cases render).
#[allow(dead_code)] // read by the real SignIn content
enum SignInState {
    ExistingAccountWarning {
        endpoint: String,
        existing_account: Account,
    },
    Authentication {
        endpoint: String,
    },
    Success,
}

/// What a link or button of `SignIn` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real SignIn content
enum SignInAction {
    /// `dispatcher.requestBrowserAuthentication()`
    RequestBrowserAuthentication,
}

/// `SignIn`'s `.existing-account-warning` paragraph.
#[allow(dead_code)] // filled by the real SignIn content
struct ExistingAccountWarning {
    /// The paragraph's whole text.
    text: String,
    /// The first `Ref`: the endpoint's host.
    host: String,
    /// The second `Ref`: the existing account's login.
    login: String,
}

/// What GitHub Desktop's `SignIn` shows.
#[allow(dead_code)] // filled by the real SignIn content
struct SignInContent {
    existing_account_warning: Option<ExistingAccountWarning>,
    /// The form's links and buttons: label and what each does.
    actions: Vec<(String, SignInAction)>,
}

/// Stand-in for GitHub Desktop's `SignIn` (`ui/lib/sign-in.tsx`) for
/// `sign_in_state`, `None` when it renders nothing. Replace it with the
/// Corvene function once there is one and remove the `#[ignore]`.
fn sign_in(_sign_in_state: &SignInState) -> Option<SignInContent> {
    unimplemented!(
        "Corvene has no existing-account warning: a sign-in replaces the endpoint's account silently"
    )
}

/// What a button of `SignInEnterprise` or `ConfigureGit` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real Welcome step contents
enum WelcomeAction {
    /// `advance(step)`
    Advance(WelcomeStep),
    /// `done()`
    Done,
}

/// What GitHub Desktop's `SignInEnterprise` shows.
#[allow(dead_code)] // filled by the real SignInEnterprise content
struct SignInEnterpriseContent {
    /// The `<h1>`.
    title: String,
    /// Its own buttons (the `SignIn` children): label and what each does.
    buttons: Vec<(String, WelcomeAction)>,
}

/// Stand-in for GitHub Desktop's `SignInEnterprise`
/// (`ui/welcome/sign-in-enterprise.tsx`), `None` when it renders nothing.
/// Replace it with the Corvene function once there is one and remove the
/// `#[ignore]`.
fn sign_in_enterprise(_sign_in_state: Option<&SignInState>) -> Option<SignInEnterpriseContent> {
    unimplemented!(
        "Corvene's Welcome flow has no SignInEnterprise step: its Enterprise button opens the sign-in dialog"
    )
}

/// What GitHub Desktop's `ConfigureGit` shows.
#[allow(dead_code)] // filled by the real ConfigureGit content
struct ConfigureGitContent {
    /// The `<h1>`.
    title: String,
    /// The `.welcome-text` paragraph.
    text: String,
    /// The form's buttons: label and what each does.
    buttons: Vec<(String, WelcomeAction)>,
}

/// Stand-in for GitHub Desktop's `ConfigureGit`
/// (`ui/welcome/configure-git.tsx`). Replace it with the Corvene function
/// once there is one and remove the `#[ignore]`.
fn configure_git(
    _accounts: &[Account],
    _global_user_name: Option<&str>,
    _global_user_email: Option<&str>,
) -> ConfigureGitContent {
    unimplemented!(
        "Corvene has no ConfigureGit content: Welcome::configure_git draws the step inside the GPUI view"
    )
}

/// The action of the button labelled `label`, `None` when there is none.
fn action_of<A: Copy>(actions: &[(String, A)], label: &str) -> Option<A> {
    actions
        .iter()
        .find(|(button, _)| button == label)
        .map(|(_, action)| *action)
}

/// GitHub Desktop's `createExistingAccountWarningState()`: github.com with
/// the account `mona` (`new Account('mona', 'https://api.github.com',
/// 'token', [], '', 1, 'Mona Lisa')`; Corvene keeps the token in the
/// Keychain, not in `Account`).
fn create_existing_account_warning_state() -> SignInState {
    SignInState::ExistingAccountWarning {
        endpoint: "https://api.github.com".to_string(),
        existing_account: Account {
            endpoint: "https://api.github.com".to_string(),
            id: 1,
            login: "mona".to_string(),
            name: Some("Mona Lisa".to_string()),
            avatar_url: None,
            emails: Vec::new(),
            scopes: Vec::new(),
            plan: None,
            private_primary_email: false,
        },
    }
}

// GHD: unit/ui/welcome-and-sign-in-wrappers-test.tsx › welcome and sign-in wrappers › renders warning and browser-authentication states in the shared sign-in wrapper
#[test]
#[ignore = "ghd: missing: no ExistingAccountWarning sign-in step (ui/lib/sign-in.tsx); Corvene's sign-in replaces the endpoint's account silently (dispatcher.rs accounts.retain)"]
fn renders_warning_and_browser_authentication_states_in_the_shared_sign_in_wrapper() {
    let view =
        sign_in(&create_existing_account_warning_state()).expect("SignIn renders the warning step");

    let warning = view
        .existing_account_warning
        .expect("SignIn renders the existing-account warning");
    // `getByText(…, { exact: false })`
    assert!(warning.text.contains("You're already signed in to"));
    assert!(warning.host.contains("github.com"));
    // `getByText('mona')`
    assert_eq!(warning.login, "mona");

    // `getByRole('link', { name: 'Sign in using your browser' })`, clicked
    // once
    assert_eq!(
        action_of(&view.actions, "Sign in using your browser"),
        Some(SignInAction::RequestBrowserAuthentication)
    );

    // `view.rerender(…)` in the `Success` step: nothing at all
    assert!(sign_in(&SignInState::Success).is_none());
}

// GHD: unit/ui/welcome-and-sign-in-wrappers-test.tsx › welcome and sign-in wrappers › renders the enterprise welcome step only when sign-in state exists and its cancel button returns to start
#[test]
#[ignore = "ghd: missing: no SignInEnterprise welcome step (ui/welcome/sign-in-enterprise.tsx); Corvene's Welcome opens the sign-in dialog (Popup::SignIn { enterprise: true }) instead"]
fn renders_the_enterprise_welcome_step_only_with_sign_in_state_and_cancel_returns_to_start() {
    // `signInState={null}`: nothing at all
    assert!(sign_in_enterprise(None).is_none());

    let state = SignInState::Authentication {
        endpoint: "https://api.github.com".to_string(),
    };
    let view =
        sign_in_enterprise(Some(&state)).expect("SignInEnterprise renders with a sign-in state");

    assert_eq!(view.title, "Sign in to your GitHub Enterprise");
    // Cancel, clicked once
    assert_eq!(
        action_of(&view.buttons, "Cancel"),
        Some(WelcomeAction::Advance(WelcomeStep::Start))
    );
}

// GHD: unit/ui/welcome-and-sign-in-wrappers-test.tsx › welcome and sign-in wrappers › renders the configure-git welcome step and returns to start when cancelled
#[test]
#[ignore = "ghd: missing: no ConfigureGit content (ui/welcome/configure-git.tsx); Welcome::configure_git draws the step inline in the GPUI view"]
fn renders_the_configure_git_welcome_step_and_returns_to_start_when_cancelled() {
    let view = configure_git(&[], None, None);

    assert_eq!(view.title, "Configure Git");
    // `getByText(…, { exact: false })`
    assert!(
        view.text
            .contains("This is used to identify the commits you create.")
    );
    // `getByRole('button', { name: 'Finish' })`
    assert!(action_of(&view.buttons, "Finish").is_some());

    // Cancel, clicked once: back to Start, `done` not called
    assert_eq!(
        action_of(&view.buttons, "Cancel"),
        Some(WelcomeAction::Advance(WelcomeStep::Start))
    );
}
