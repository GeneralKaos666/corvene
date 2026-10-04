//! Port of GitHub Desktop's
//! `app/test/unit/ui/structural-and-text-components-test.tsx`
//! (`CommitAttribution`, `AccessText`, `HighlightText`; the `Form` /
//! `DialogContent` / `UiView` wrappers are skipped in
//! `tools/ghd-tests/skips/ui2.tsv`).
//!
//! - `CommitAttribution` (`ui/lib/commit-attribution.tsx`, the History
//!   rows and the selected commit's summary) names one or two authors
//!   ("Mona", "Mona, Hubot") or counts them ("3 people"). Corvene's
//!   counterpart is `corvene_ui::history::commit_attribution(avatar_users)`,
//!   the text of the History row's byline (`history.rs`) and of the selected
//!   commit's meta row (`selected_commit.rs`); GitHub Desktop's `IAvatarUser`
//!   is `corvene_core::AvatarUser` (`getAvatarUsersForCommit`:
//!   `corvene_core::get_avatar_users_for_commit`).
//! - `AccessText` (`ui/lib/access-text.tsx`) splits a label at its access
//!   key (`&`, `&&` a literal ampersand) into the text and the key.
//!   Corvene's counterpart is `corvene_ui::views_menu::parse_mnemonic`, the
//!   in-window menu bar's and context menus' label parser (Linux, Windows,
//!   Android); macOS menus are AppKit's and their labels carry no `&`, so
//!   the module (and the ported case) does not exist there. GitHub Desktop
//!   likewise only uses `AccessText` in its in-window app menu
//!   (`ui/app-menu`), which macOS does not show. The
//!   `.access-key.highlight` class (the key underlined while Alt is held)
//!   is React DOM and not ported.
//! - `HighlightText` (`ui/lib/highlight-text.tsx`) marks runs of matched
//!   characters (`<mark>`) between unmatched runs (`<span>`). Corvene's
//!   counterpart is `corvene_ui::repository_list::bold_ranges`, the byte
//!   ranges `autocompletion::highlighted` draws bold (made reachable for
//!   this port; `highlighted` itself returns an opaque `StyledText`).

use std::ops::Range;

use corvene_core::AvatarUser;
use corvene_ui::history::commit_attribution;
use corvene_ui::repository_list::bold_ranges;

/// GitHub Desktop's `createAvatarUser(name)`.
fn create_avatar_user(name: &str) -> AvatarUser {
    AvatarUser {
        name: name.to_string(),
        email: format!("{}@example.com", name.to_lowercase()),
        avatar_url: None,
        endpoint: None,
    }
}

/// The `<span>` runs `HighlightText` renders around the marked `ranges`
/// of `text` (the unmatched stretches between them).
fn unmarked_runs(text: &str, ranges: &[Range<usize>]) -> Vec<String> {
    let mut runs = Vec::new();
    let mut start = 0;
    for range in ranges {
        if range.start > start {
            runs.push(text[start..range.start].to_string());
        }
        start = range.end;
    }
    if start < text.len() {
        runs.push(text[start..].to_string());
    }
    runs
}

// GHD: unit/ui/structural-and-text-components-test.tsx › structural and text components › renders commit attribution for one, two, and many authors
#[test]
fn renders_commit_attribution_for_one_two_and_many_authors() {
    let mona = create_avatar_user("Mona");
    let hubot = create_avatar_user("Hubot");
    let desktop = create_avatar_user("Desktop");

    let attributions = [
        commit_attribution(&[&mona]),
        commit_attribution(&[&mona, &hubot]),
        commit_attribution(&[&mona, &hubot, &desktop]),
    ];

    assert_eq!(
        attributions
            .iter()
            .map(|attribution| attribution.text.as_str())
            .collect::<Vec<_>>(),
        ["Mona", "Mona, Hubot", "3 people"]
    );

    let author_names: Vec<&str> = attributions
        .iter()
        .flat_map(|attribution| attribution.authors.iter().map(String::as_str))
        .collect();

    assert_eq!(author_names, ["Mona", "Mona", "Hubot"]);
}

// GHD: unit/ui/structural-and-text-components-test.tsx › structural and text components › renders access text with a screen-reader fallback and optional highlighting
#[cfg(not(target_os = "macos"))]
#[test]
fn renders_access_text_with_a_screen_reader_fallback_and_optional_highlighting() {
    use corvene_ui::views_menu::parse_mnemonic;

    let (text, access_key) = parse_mnemonic("E&xit");
    let (plain, _) = parse_mnemonic("Plain text");

    // `.access-key`: the character after the ampersand
    let highlighted_key = access_key.and_then(|ix| text[ix..].chars().next());
    assert_eq!(highlighted_key, Some('x'));
    // `.sr-only`: the label without the ampersand
    assert_eq!(text, "Exit");
    assert_eq!(plain, "Plain text");
}

// GHD: unit/ui/structural-and-text-components-test.tsx › structural and text components › renders contiguous highlight runs as mark elements
#[test]
fn renders_contiguous_highlight_runs_as_mark_elements() {
    let text = "branch";

    let ranges: Vec<Range<usize>> = bold_ranges(text, &[1, 2, 3])
        .into_iter()
        .map(|(range, _)| range)
        .collect();

    // the first `<mark>`
    let mark = ranges.first().map(|range| &text[range.clone()]);
    let spans = unmarked_runs(text, &ranges);

    assert_eq!(mark, Some("ran"));
    assert!(spans.iter().any(|span| span == "b"));
    assert!(spans.iter().any(|span| span == "ch"));
}
