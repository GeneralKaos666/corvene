//! GHD `GitEmailNotFoundWarning` (`ui/lib/git-email-not-found-warning.tsx`):
//! under a commit email field, ⚠️ "This email address does not match your
//! GitHub account. Your commits will be wrongly attributed." and a "Learn
//! more." link, or a green check and "This email address matches your
//! GitHub account.", with the sentence repeated in a polite live region for
//! screen readers. Shown by Welcome › Configure Git (`welcome.rs`, the first
//! account, as GHD's `ConfigureGitUser`) and Settings › Git when "Other
//! email" is chosen (`dialogs/preferences.rs`, every account, as GHD's
//! `GitConfigUserForm`).
//!
//! Deviation: the live region's text follows the email field at once (GHD's
//! `AriaLiveContainer` re-announces it 1 s after typing stops).

use corvene_core::Account;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::zpx;
use crate::widgets::{Inline, ListRowA11y, link_button, paragraph};

/// The "Learn more." link's destination.
const COMMIT_ATTRIBUTION_URL: &str = "https://docs.github.com/en/github/committing-changes-to-your-project/why-are-my-commits-linked-to-the-wrong-user";

/// The indicator before the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    /// `<span className="warning-icon">⚠️</span>`
    Warning,
    /// `.green-circle` with a `.check-icon` octicon
    Check,
}

/// The "Learn more." `LinkButton`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnMore {
    pub text: String,
    pub aria_label: String,
    pub uri: String,
}

/// What GHD `GitEmailNotFoundWarning` renders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitEmailNotFoundWarningContent {
    pub indicator: Indicator,
    /// `buildScreenReaderMessage(isAttributableEmail)`, shown in the warning
    pub message: String,
    pub learn_more: Option<LearnMore>,
    /// The text of the `#git-email-not-found-warning-for-screen-readers`
    /// polite live region (the message, plus `AriaLiveContainer`'s suffix)
    pub screen_reader_message: String,
}

impl GitEmailNotFoundWarningContent {
    /// `.git-email-not-found-warning`'s `textContent`: the indicator, the
    /// message and the link's text.
    pub fn text_content(&self) -> String {
        let indicator = match self.indicator {
            Indicator::Warning => "⚠️",
            Indicator::Check => "",
        };
        let link = self.learn_more.as_ref().map_or("", |l| l.text.as_str());
        format!("{indicator}{}{link}", self.message)
    }
}

/// GHD `getAccountTypeDescription`.
fn account_type_description(accounts: &[Account]) -> &'static str {
    match accounts {
        [account] if account.is_dotcom() => "your GitHub account",
        [_] => "your GitHub Enterprise account",
        _ => "either of your GitHub.com nor GitHub Enterprise accounts",
    }
}

/// GHD `GitEmailNotFoundWarning` for `accounts` and `email`: `None` when it
/// renders nothing (no accounts, or a blank email).
pub fn git_email_not_found_warning(
    accounts: &[Account],
    email: &str,
) -> Option<GitEmailNotFoundWarningContent> {
    if accounts.is_empty() || email.trim().is_empty() {
        return None;
    }
    // `isAttributableEmailFor`
    let attributable = accounts.iter().any(|a| a.is_attributable_email(email));
    let (verb, info) = if attributable {
        ("matches", "")
    } else {
        (
            "does not match",
            "Your commits will be wrongly attributed. ",
        )
    };
    let message = format!(
        "This email address {verb} {}. {info}",
        account_type_description(accounts)
    );
    Some(GitEmailNotFoundWarningContent {
        indicator: if attributable {
            Indicator::Check
        } else {
            Indicator::Warning
        },
        // `AriaLiveContainer.buildMessage`'s first suffix
        screen_reader_message: format!("{message}\u{a0}\u{a0}"),
        message,
        learn_more: (!attributable).then(|| LearnMore {
            text: "Learn more.".into(),
            aria_label: "Learn more about commit attribution".into(),
            uri: COMMIT_ATTRIBUTION_URL.into(),
        }),
    })
}

/// Draws `content`: the indicator, the message and the link in one
/// paragraph (`line_height` apart, the link at `link_size`), then the live
/// region. `id` keys the link and the live region.
pub fn git_email_not_found_warning_element(
    id: &'static str,
    content: GitEmailNotFoundWarningContent,
    link_size: Pixels,
    line_height: Pixels,
    cx: &App,
) -> Div {
    let t = cx.ghd();
    let GitEmailNotFoundWarningContent {
        indicator,
        message,
        learn_more,
        screen_reader_message,
    } = content;
    let mut parts: Vec<Inline> = Vec::new();
    parts.push(match indicator {
        Indicator::Warning => crate::widgets::emoji("⚠️").into_any_element().into(),
        Indicator::Check => div()
            .size(zpx(12.))
            .mr(zpx(5.))
            .rounded_full()
            .bg(t.color_new)
            .flex()
            .items_center()
            .justify_center()
            .child(octicon(Octicon::Check, t.background).size(zpx(10.)))
            .into_any_element()
            .into(),
    });
    parts.push(message.into());
    if let Some(LearnMore {
        text,
        aria_label,
        uri,
    }) = learn_more
    {
        parts.push(
            link_button(SharedString::from(format!("{id}-learn-more")), text, cx)
                .aria_label(aria_label)
                .text_size(link_size)
                .on_click(move |_, _, cx| corvene_core::Dispatcher::open_url(&uri, cx))
                .into_any_element()
                .into(),
        );
    }
    div()
        .relative()
        .flex()
        .flex_col()
        .child(paragraph(parts).line_height(line_height))
        .child(
            // `#git-email-not-found-warning-for-screen-readers` (`.sr-only`)
            div()
                .id(SharedString::from(format!("{id}-for-screen-readers")))
                .absolute()
                .size_0()
                .overflow_hidden()
                .a11y_live(screen_reader_message),
        )
}
