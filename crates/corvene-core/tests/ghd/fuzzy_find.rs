//! Port of GitHub Desktop's `app/test/unit/fuzzy-find-test.ts`.
//!
//! GitHub Desktop's `match(query, items, getKey)` (`lib/fuzzy-find.ts`)
//! fuzzy-matches every key of an item (here `getText` of
//! `ui/lib/filter-list.tsx`: the item's `text`, a title and a subtitle),
//! keeps the items with a hit in any key and sorts them by the score of the
//! joined keys. Corvene has the matcher for one text
//! (`corvene_core::filter::fuzzy_match`, fuzzaldrin's `match` / `score`)
//! but no multi-key `match`: each list combines its keys itself
//! (`corvene_ui::pull_request_list::matches_filter` tries the title, then
//! the subtitle). The cases call a stand-in and are ignored until
//! `corvene_core::filter` has one.

/// An `IFilterListItem` of the test: an id and the texts to match.
struct Item {
    #[allow(dead_code)]
    id: &'static str,
    text: Vec<&'static str>,
}

/// GitHub Desktop's `IMatches`: the matched character positions in the
/// first (title) and second (subtitle) key.
#[allow(dead_code)]
struct IMatches {
    title: Vec<usize>,
    subtitle: Vec<usize>,
}

/// GitHub Desktop's `IMatch<T>`.
#[allow(dead_code)]
struct IMatch<'a, T> {
    /// `0 <= score <= 1`
    score: f32,
    item: &'a T,
    matches: IMatches,
}

/// Stand-in for GitHub Desktop's `match(query, items, getKey)`
/// (`lib/fuzzy-find.ts`). Replace it with the `corvene_core::filter`
/// function once there is one and remove the `#[ignore]`s.
fn match_<'a, T>(
    _query: &str,
    _items: &'a [T],
    _get_key: impl Fn(&T) -> Vec<String>,
) -> Vec<IMatch<'a, T>> {
    unimplemented!("corvene_core::filter has no multi-key fuzzy match (lib/fuzzy-find.ts match)")
}

/// `getText` of `ui/lib/filter-list.tsx`: `item['text']`.
fn get_text(item: &Item) -> Vec<String> {
    item.text.iter().map(|t| t.to_string()).collect()
}

fn items() -> Vec<Item> {
    vec![
        Item {
            id: "300",
            text: vec!["add fix for ...", "opened 5 days ago by bob"],
        },
        Item {
            id: "500",
            text: vec!["add support", "#4653 opened 3 days ago by damaneice "],
        },
        Item {
            id: "500",
            text: vec!["add an awesome feature", "#7564 opened 10 days ago by ... "],
        },
    ]
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find matching item when searching by pull request number
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no multi-key match over an item's title and subtitle (lib/fuzzy-find.ts match)"]
fn should_find_matching_item_when_searching_by_pull_request_number() {
    let items = items();
    let results = match_("4653", &items, get_text);

    assert_eq!(results.len(), 1);
    assert!(results[0].item.text.join("").contains("4653"));
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find matching item when searching by author
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no multi-key match over an item's title and subtitle (lib/fuzzy-find.ts match)"]
fn should_find_matching_item_when_searching_by_author() {
    let items = items();
    let results = match_("damaneice", &items, get_text);

    assert_eq!(results.len(), 1);
    assert!(results[0].item.text.join("").contains("damaneice"));
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find matching item when by title
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no multi-key match over an item's title and subtitle (lib/fuzzy-find.ts match)"]
fn should_find_matching_item_when_by_title() {
    let items = items();
    let results = match_("awesome feature", &items, get_text);

    assert_eq!(results.len(), 1);
    assert!(results[0].item.text.join("").contains("awesome feature"));
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find nothing
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no multi-key match over an item's title and subtitle (lib/fuzzy-find.ts match)"]
fn should_find_nothing() {
    let items = items();
    let results = match_("$%^", &items, get_text);

    assert_eq!(results.len(), 0);
}
