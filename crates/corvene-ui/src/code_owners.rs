//! Corvene `1314-code-owners` (desktop/desktop#20909): a file row's
//! CODEOWNERS owners (the first one and "+N", or a shield icon, per
//! Settings › Appearance › Code owners) with GitHub's "Owned by …" tooltip,
//! and the owners summary of Preview Pull Request. GHD shows no owners. The
//! owners come from `corvene_core::codeowners`.

use corvene_core::codeowners::{CodeOwnersSource, RowOwners};
use corvene_core::{AppState, CodeOwnersDisplay};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::GhdTooltip;

/// The owners of `path` in repository `id`'s `source` file, and the file's
/// location; `None` with the flag off or no owner.
pub fn row_owners(
    id: Option<u64>,
    source: CodeOwnersSource,
    path: &str,
    cx: &App,
) -> Option<(RowOwners, String)> {
    let s = AppState::try_global(cx)?.read(cx);
    if !s.flags.bool(corvene_core::flags::ids::CODE_OWNERS) {
        return None;
    }
    let rs = s.repo_states.get(&id?)?;
    let row = rs.code_owners_of(source, path)?;
    let location = rs.code_owners.file(source)?.location.clone();
    Some((row, location))
}

/// The badge before a row's status icon. `selected` is the colour of a
/// focused selected row (its text colour wins over the owner colours).
pub fn badge(
    key: impl Into<SharedString>,
    owners: &RowOwners,
    location: &str,
    selected: Option<Hsla>,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let display = AppState::try_global(cx)
        .map(|s| s.read(cx).settings.code_owners_display)
        .unwrap_or_default();
    let colour = selected.unwrap_or(if owners.yours {
        t.link
    } else {
        t.text_secondary
    });
    let key: SharedString = key.into();
    let base = div()
        .id(ElementId::Name(format!("code-owners-{key}").into()))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .text_size(FONT_SIZE_SM())
        .text_color(colour)
        .ghd_tooltip(owners.tooltip(location));
    match display {
        CodeOwnersDisplay::Icon => base.child(octicon(Octicon::Shield, colour)).into_any_element(),
        CodeOwnersDisplay::Label => base
            .max_w(zpx(140.))
            .child(div().min_w_0().truncate().child(owners.label()))
            .into_any_element(),
    }
}

/// Preview Pull Request's owners line: each owner of the changed files and
/// their file count, other people's teams first, the user's marked
/// "(you)"; `None` without a CODEOWNERS file or an owned file.
pub fn summary<'a>(
    id: u64,
    base: &str,
    paths: impl IntoIterator<Item = &'a str>,
    cx: &App,
) -> Option<Div> {
    let s = AppState::try_global(cx)?.read(cx);
    if !s.flags.bool(corvene_core::flags::ids::CODE_OWNERS) {
        return None;
    }
    let owners = s
        .repo_states
        .get(&id)?
        .code_owners
        .summary(CodeOwnersSource::Rev(base), paths);
    if owners.is_empty() {
        return None;
    }
    let t = cx.ghd();
    let mut row = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(SPACING_HALF())
        .text_size(FONT_SIZE_SM())
        .text_color(t.text_secondary)
        .child(octicon(Octicon::Shield, t.text_secondary))
        .child("Owners:");
    let last = owners.len() - 1;
    for (ix, (owner, files, yours)) in owners.into_iter().enumerate() {
        let noun = if files == 1 { "file" } else { "files" };
        let you = if yours { ", you" } else { "" };
        let sep = if ix < last { "," } else { "" };
        row = row.child(
            div()
                .text_color(if yours { t.link } else { t.text })
                .child(format!("{owner} ({files} {noun}{you}){sep}")),
        );
    }
    Some(row)
}
