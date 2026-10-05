//! Commit signature badges (`1214-commit-signatures`; Corvene addition,
//! GHD `ui/history/commit-summary.tsx` and `ui/history/commit-list-item.tsx`
//! show no signature status): github.com's outlined "Verified" pill in the
//! selected commit's header and a 12 px shield after a History row's
//! indicators. The verdicts come from `corvene_core::signatures`.

use corvene_core::signatures::{Priority, SignatureEntry, Source};
use corvene_core::{AppState, Commit, Dispatcher, SignatureKind, SignatureState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, c, primer};
use crate::widgets::rich_tooltip;

/// `1214-commit-signatures`
pub(crate) fn enabled(cx: &App) -> bool {
    AppState::try_global(cx).is_some_and(|s| {
        s.read(cx)
            .flags
            .bool(corvene_core::flags::ids::COMMIT_SIGNATURES)
    })
}

/// `1215-verify-visible-signatures` (only with `1214`)
pub(crate) fn verify_rows(cx: &App) -> bool {
    AppState::try_global(cx).is_some_and(|s| {
        let flags = &s.read(cx).flags;
        flags.bool(corvene_core::flags::ids::COMMIT_SIGNATURES)
            && flags.bool(corvene_core::flags::ids::VERIFY_VISIBLE_SIGNATURES)
    })
}

fn color(state: Option<SignatureState>, cx: &App) -> Hsla {
    match state {
        Some(SignatureState::Verified) => c(primer::GREEN_500),
        Some(SignatureState::Unverified) => c(primer::YELLOW_700),
        Some(SignatureState::Bad) => c(primer::RED_500),
        _ => cx.ghd().text_secondary,
    }
}

fn row_icon_for(state: Option<SignatureState>) -> Octicon {
    match state {
        Some(SignatureState::Verified) => Octicon::ShieldCheck,
        Some(SignatureState::Unverified) => Octicon::Unverified,
        Some(SignatureState::Bad) => Octicon::ShieldX,
        _ => Octicon::Shield,
    }
}

fn label(state: Option<SignatureState>, loading: bool) -> &'static str {
    match state {
        Some(state) => state.label(),
        None if loading => "Verifying…",
        None => "Signed",
    }
}

/// The tooltip: the verdict (bold) and why, who signed with which key, and
/// what GitHub and local git each said.
pub(crate) fn tooltip_text(kind: SignatureKind, entry: Option<&SignatureEntry>) -> (String, usize) {
    let verdict = entry.and_then(SignatureEntry::verdict);
    let loading = entry.is_none_or(SignatureEntry::loading);
    let github = entry.and_then(SignatureEntry::github);
    let local = entry.and_then(SignatureEntry::local);
    let reason = match verdict {
        Some((_, Source::GitHub)) => github.and_then(|g| g.reason.as_ref()),
        Some((_, Source::Local)) => local.and_then(|l| l.reason.as_ref()),
        None => None,
    };
    let mut heading = label(verdict.map(|(s, _)| s), loading).to_string();
    if let Some(reason) = reason {
        heading = format!("{heading}: {}", reason.describe());
    }
    let bold = heading.len();
    let mut lines = vec![heading];

    let signer = local
        .and_then(|l| l.signer.clone())
        .or_else(|| github.and_then(|g| g.signer_login.as_ref().map(|l| format!("@{l}"))));
    if github.is_some_and(|g| g.was_signed_by_github) {
        lines.push("Signed by GitHub".into());
    } else if let Some(signer) = signer {
        lines.push(format!("Signed by {signer}"));
    }
    let key = local
        .and_then(|l| l.key.clone().or_else(|| l.fingerprint.clone()))
        .or_else(|| github.and_then(|g| g.key.clone()));
    match key {
        Some(key) => lines.push(format!("{} key {key}", kind.label())),
        None => lines.push(format!("{} signature", kind.label())),
    }
    if let Some(local) = local {
        if let Some(fp) = local
            .fingerprint
            .as_ref()
            .filter(|fp| Some(*fp) != local.key.as_ref())
        {
            lines.push(format!("Fingerprint {fp}"));
        }
        if let Some(primary) = &local.primary_fingerprint {
            lines.push(format!("Primary key {primary}"));
        }
    }

    let mut sources = Vec::new();
    if let Some(github) = github {
        sources.push(format!("GitHub: {}", github.state.label()));
    }
    if let Some(local) = local {
        let mut line = format!("This computer: {}", local.state.label());
        if let Some(trust) = &local.trust {
            line.push_str(&format!(" (trust {trust})"));
        }
        if verdict.is_some_and(|(_, s)| s == Source::GitHub)
            && let Some(reason) = &local.reason
        {
            line.push_str(&format!(", {}", reason.describe().to_lowercase()));
        }
        sources.push(line);
    }
    if !sources.is_empty() {
        lines.push(String::new());
        lines.extend(sources);
    }
    (lines.join("\n"), bold)
}

/// Asks for `commit`'s verdict (selected repository) if the cache lacks it;
/// unsigned commits need nothing.
pub(crate) fn touch(commit: &Commit, priority: Priority, cx: &mut App) {
    if commit.signature.is_none() {
        return;
    }
    let Some(id) = AppState::try_global(cx).and_then(|s| s.read(cx).selected) else {
        return;
    };
    Dispatcher::touch_signature(id, &commit.sha, commit.signature, priority, cx);
}

/// The selected commit's pill, after the SHA: `None` for an unsigned commit.
pub(crate) fn header_pill(
    commit: &Commit,
    entry: Option<&SignatureEntry>,
    cx: &App,
) -> Option<AnyElement> {
    let kind = commit.signature?;
    let verdict = entry.and_then(SignatureEntry::verdict).map(|(s, _)| s);
    let loading = entry.is_none_or(SignatureEntry::loading);
    let color = color(verdict, cx);
    let (text, bold) = tooltip_text(kind, entry);
    Some(
        div()
            .id("commit-signature")
            .flex_none()
            .flex()
            .items_center()
            .mr(SPACING())
            // github.com's `.signed-commit-badge`: an outlined pill
            .h(zpx(18.))
            .px(zpx(7.))
            .rounded(zpx(9.))
            .border_1()
            .border_color(color)
            .text_color(color)
            .text_size(FONT_SIZE_XS())
            .font_weight(FontWeight::MEDIUM)
            .child(label(verdict, loading))
            .tooltip(rich_tooltip(text, 0..bold))
            .tooltip_show_delay(crate::widgets::TOOLTIP_DELAY)
            .into_any_element(),
    )
}

/// A signed History row's shield: coloured by the cached verdict, grey
/// until the commit has been verified. `selected` replaces the colour on
/// a selected row (the badge text colour there).
pub(crate) fn row_icon(commit: &Commit, selected: Option<Hsla>, cx: &App) -> Option<AnyElement> {
    let kind = commit.signature?;
    let entry = AppState::try_global(cx).and_then(|s| {
        s.read(cx)
            .selected_state()
            .and_then(|rs| rs.signatures.get(&commit.sha).cloned())
    });
    let verdict = entry
        .as_ref()
        .and_then(SignatureEntry::verdict)
        .map(|(s, _)| s);
    let (text, bold) = tooltip_text(kind, entry.as_ref());
    Some(
        div()
            .id(SharedString::from(format!("signature-{}", commit.sha)))
            .flex_none()
            .ml(SPACING_HALF())
            .flex()
            .items_center()
            .child(
                octicon(
                    row_icon_for(verdict),
                    selected.unwrap_or_else(|| color(verdict, cx)),
                )
                .size(zpx(12.)),
            )
            .tooltip(rich_tooltip(text, 0..bold))
            .tooltip_show_delay(crate::widgets::TOOLTIP_DELAY)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_core::signatures::{GitHubSlot, LocalSlot};
    use corvene_core::{LocalSignature, SignatureReason};

    #[test]
    fn tooltip_names_signer_key_and_reason() {
        let entry = SignatureEntry::new(
            LocalSlot::Done(LocalSignature {
                state: SignatureState::Unverified,
                reason: Some(SignatureReason::UnknownKey),
                kind: Some(SignatureKind::Gpg),
                signer: None,
                key: Some("4AEE18F83AFDEB23".into()),
                fingerprint: None,
                primary_fingerprint: None,
                trust: None,
            }),
            GitHubSlot::Missing,
        );
        let (text, bold) = tooltip_text(SignatureKind::Gpg, Some(&entry));
        assert_eq!(&text[..bold], "Unverified: The signing key is not known");
        assert!(text.contains("GPG key 4AEE18F83AFDEB23"));
        assert!(text.contains("This computer: Unverified"));
        let (text, _) = tooltip_text(SignatureKind::Ssh, None);
        assert!(text.starts_with("Verifying…\nSSH signature"));
    }
}
