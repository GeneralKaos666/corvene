//! Port of GitHub Desktop's `app/test/unit/release-notes-test.ts`.
//!
//! GitHub Desktop's `lib/release-notes.ts` is
//! `corvene_core::release_notes`, fed from a GitHub release's Markdown body
//! instead of GitHub Desktop's changelog feed (deviations.md › "Help › Show
//! Release Notes"): the feed's one string per entry is one list item of the
//! body, so [`release_body`] turns the test's `notes` array into `- <entry>`
//! lines. The flag `504-release-notes-heading-kinds` is off (its
//! `github-desktop` value), so untagged items are dropped as in GitHub
//! Desktop.
//!
//! - `parseReleaseEntries(notes)` is
//!   `parse_release_body_with(release_body(notes), false)`.
//! - `getReleaseSummary(release)` is what `Dispatcher::show_release_notes`
//!   does with a fetched release: `release_summary(version,
//!   parse_iso8601(published_at), parse_release_body_with(body, false))`.
//!   GitHub Desktop formats `datePublished` there (`formatDate(…, {
//!   dateStyle: 'long' })`); Corvene keeps a `SystemTime` and the
//!   `ReleaseNotes` dialog formats it (`dialogs/release_notes.rs`:
//!   `format_pattern("MMMM d, yyyy", &local_time(d))`), so
//!   [`date_published`] makes that same call.

use std::time::SystemTime;

use corvene_core::parse_iso8601;
use corvene_core::release_notes::{
    ReleaseNote, ReleaseNoteKind, ReleaseSummary, parse_release_body_with, release_summary,
};
use corvene_ui::format::{format_pattern, local_time};

/// GitHub Desktop's `ReleaseMetadata`.
struct ReleaseMetadata {
    #[allow(dead_code)]
    name: &'static str,
    notes: Vec<&'static str>,
    pub_date: &'static str,
    version: &'static str,
}

/// The changelog feed's entries as a release body: one list item each.
fn release_body(notes: &[&str]) -> String {
    notes
        .iter()
        .map(|note| format!("- {note}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// GitHub Desktop's `parseReleaseEntries(notes)`.
fn parse_release_entries(notes: &[&str]) -> Vec<ReleaseNote> {
    parse_release_body_with(&release_body(notes), false)
}

/// GitHub Desktop's `getReleaseSummary(latestRelease)`.
fn get_release_summary(latest_release: &ReleaseMetadata) -> ReleaseSummary {
    release_summary(
        latest_release.version,
        parse_iso8601(latest_release.pub_date),
        parse_release_entries(&latest_release.notes),
    )
}

/// `ReleaseSummary.datePublished` as the dialog shows it.
fn date_published(summary: &ReleaseSummary) -> String {
    let date: SystemTime = summary
        .date_published
        .expect("the release has a publication date");
    format_pattern("MMMM d, yyyy", &local_time(date))
}

// GHD: unit/release-notes-test.ts › release-notes › parseReleaseEntries › formats lowercased fixed message
#[test]
fn formats_lowercased_fixed_message() {
    let values = ["[fixed] something else"];

    let result = parse_release_entries(&values);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].kind, ReleaseNoteKind::Fixed);
    assert_eq!(result[0].message, "something else");
}

// GHD: unit/release-notes-test.ts › release-notes › parseReleaseEntries › formats uppercased fixed message
#[test]
fn formats_uppercased_fixed_message() {
    let values = ["[Fixed] and another thing"];

    let result = parse_release_entries(&values);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].kind, ReleaseNoteKind::Fixed);
    assert_eq!(result[0].message, "and another thing");
}

// GHD: unit/release-notes-test.ts › release-notes › parseReleaseEntries › uses [Other] for unrecognized category
#[test]
fn uses_other_for_unrecognized_category() {
    let values = ["[Foo] we did a thing!"];

    let result = parse_release_entries(&values);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].kind, ReleaseNoteKind::Other);
    assert_eq!(result[0].message, "we did a thing!");
}

// GHD: unit/release-notes-test.ts › release-notes › getReleaseSummary › can render 1.0.11 layout
#[test]
fn can_render_1_0_11_layout() {
    let one_oh_eleve_release = ReleaseMetadata {
        name: "",
        notes: vec![
            "[New] Highlight substring matches in the \"Branches\" and \"Repositories\" list when filtering - #910. Thanks @JordanMussi!",
            "[New] Add preview for ico files - #3531. Thanks @serhiivinichuk!",
            "[New] Fallback to Gravatar for loading avatars - #821",
            "[New] Provide syntax highlighting for Visual Studio project files - #3552. Thanks @saul!",
            "[New] Provide syntax highlighting for F# fsx and fsi files - #3544. Thanks @saul!",
            "[New] Provide syntax highlighting for Kotlin files - #3555. Thanks @ziggy42!",
            "[New] Provide syntax highlighting for Clojure - #3523. Thanks @mtkp!",
            "[Improved] Toggle the \"Repository List\" from the menu - #2638. Thanks @JordanMussi!",
            "[Improved] Prevent saving of disallowed character strings for your name and email  - #3204",
            "[Improved] Error messages now appear at the top of the \"Create a New Repository\" dialog - #3571. Thanks @http-request!",
            "[Improved] \"Repository List\" header is now \"Github.com\" for consistency - #3567. Thanks @iFun!",
            "[Improved] Rename the \"Install Update\" button to \"Quit and Install Update\" - #3494. Thanks @say25!",
            "[Fixed] Fix ordering of commit history when your branch and tracking branch have both changed  - #2737",
            "[Fixed] Prevent creating a branch that starts with a period - #3013. Thanks @JordanMussi!",
            "[Fixed] Branch names are properly encoded when creating a pull request - #3509",
            "[Fixed] Re-enable all the menu items after closing a popup - #3533",
            "[Fixed] Removes option to delete remote branch after it's been deleted - #2964. Thanks @JordanMussi!",
            "[Fixed] Windows: Detects available editors and shells now works even when the group policy blocks write registry access - #3105 #3405",
            "[Fixed] Windows: Menu items are no longer truncated - #3547",
            "[Fixed] Windows: Prevent disabled menu items from being accessed - #3391 #1521",
        ],
        pub_date: "2017-12-14T01:20:26Z",
        version: "1.0.11",
    };

    let result = get_release_summary(&one_oh_eleve_release);
    assert_eq!(result.latest_version, "1.0.11");
    assert!(date_published(&result).contains("2017"));
    assert_eq!(result.bugfixes.len(), 8);
    assert_eq!(result.enhancements.len(), 12);
}

// GHD: unit/release-notes-test.ts › release-notes › getReleaseSummary › can render 1.0.10 layout
#[test]
fn can_render_1_0_10_layout() {
    let one_oh_ten_release = ReleaseMetadata {
        name: "",
        notes: vec![
            "[New] ColdFusion Builder is now a supported external editor - #3336 #3321. Thanks @AtomicCons!",
            "[New] VSCode Insiders build is now a supported external editor - #3441. Thanks @say25!",
            "[New] BBEdit is now a supported external editor - #3467. Thanks @NiklasBr!",
            "[New] Hyper is now a supported shell on Windows too - #3455. Thanks @JordanMussi!",
            "[New] Swift is now syntax highlighted - #3305. Thanks @agisilaos!",
            "[New] Vue.js is now syntax highlighted - #3368. Thanks @wanecek!",
            "[New] CoffeeScript is now syntax highlighted - #3356. Thanks @agisilaos!",
            "[New] Cypher is now syntax highlighted - #3440. Thanks @say25!",
            "[New] .hpp is now syntax highlighted as C++ - #3420. Thanks @say25!",
            "[New] ML-like languages are now syntax highlighted - #3401. Thanks @say25!",
            "[New] Objective-C is now syntax highlighted - #3355. Thanks @koenpunt!",
            "[New] SQL is now syntax highlighted - #3389. Thanks @say25!",
            "[Improved] Better message on the 'Publish Branch' button when HEAD is unborn - #3344. Thanks @Venkat5694!",
            "[Improved] Better error message when trying to push to an archived repository - #3084. Thanks @agisilaos!",
            "[Improved] Avoid excessive background fetching when switching repositories - #3329",
            "[Improved] Ignore menu events sent when a modal is shown - #3308",
            "[Fixed] Parse changed files whose paths include a newline - #3271",
            "[Fixed] Parse file type changes - #3334",
            "[Fixed] Windows: 'Open without Git' would present the dialog again instead of actually opening a shell without git - #3290",
            "[Fixed] Avoid text selection when dragging resizable dividers - #3268",
            "[Fixed] Windows: Removed the title attribute on the Windows buttons so that they no longer leave their tooltips hanging around - #3348. Thanks @j-f1!",
            "[Fixed] Windows: Detect VS Code when installed to non-standard locations - #3304",
            "[Fixed] Hitting Return would select the first item in a filter list when the filter text was empty - #3447",
            "[Fixed] Add some missing keyboard shortcuts - #3327. Thanks @say25!",
            "[Fixed] Handle \"304 Not Modified\" responses - #3399",
            "[Fixed] Don't overwrite an existing .gitattributes when creating a new repository - #3419. Thanks @strafe!",
        ],
        pub_date: "2017-12-05T17:05:01Z",
        version: "1.0.10",
    };

    let result = get_release_summary(&one_oh_ten_release);
    assert_eq!(result.latest_version, "1.0.10");
    assert!(date_published(&result).contains("2017"));
    assert_eq!(result.bugfixes.len(), 10);
    assert_eq!(result.enhancements.len(), 16);
}

// GHD: unit/release-notes-test.ts › release-notes › getReleaseSummary › can render 1.0.9 layout
#[test]
fn can_render_1_0_9_layout() {
    let one_oh_nine_release = ReleaseMetadata {
        name: "",
        notes: vec![
            "[New] ColdFusion Builder is now available as an option for External Editor - #3336 #3321. Thanks @AtomicCons!",
            "[New] Swift code is now syntax highlighted - #3305. Thanks @agisilaos!",
            "[Improved] Better message on the 'Publish Branch' button when HEAD is unborn - #3344. Thanks @Venkat5694!",
            "[Improved] Better error message when trying to push to an archived repository - #3084. Thanks @agisilaos!",
            "[Fixed] Parse changed files whose paths include a newline - #3271",
            "[Fixed] Parse file type changes - #3334",
            "[Fixed] Windows: 'Open without Git' would present the dialog again instead of actually opening a shell without git - #3290",
            "[Fixed] Avoid text selection when dragging resizable dividers - #3268",
        ],
        pub_date: "2017-11-16T21:53:23Z",
        version: "1.0.9",
    };

    let result = get_release_summary(&one_oh_nine_release);
    assert_eq!(result.latest_version, "1.0.9");
    assert!(date_published(&result).contains("2017"));
    assert_eq!(result.bugfixes.len(), 4);
    assert_eq!(result.enhancements.len(), 4);
}
