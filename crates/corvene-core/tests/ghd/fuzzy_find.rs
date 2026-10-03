//! Port of GitHub Desktop's `app/test/unit/fuzzy-find-test.ts`.
//!
//! GitHub Desktop's `match(query, items, getKey)` (`lib/fuzzy-find.ts`)
//! fuzzy-matches every key of an item (here `getText` of
//! `ui/lib/filter-list.tsx`: the item's `text`, a title and a subtitle),
//! keeps the items with a hit in the title or subtitle and sorts them by
//! the score of the joined keys. It is `corvene_core::filter::match_items`
//! (`IMatch` is `filter::Match`), over Corvene's one-text matcher
//! `filter::fuzzy_match` (fuzzaldrin's `match` / `score`).

use corvene_core::filter::match_items as match_;

/// An `IFilterListItem` of the test: an id and the texts to match.
struct Item {
    #[allow(dead_code)]
    id: &'static str,
    text: Vec<&'static str>,
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
fn should_find_matching_item_when_searching_by_pull_request_number() {
    let items = items();
    let results = match_("4653", &items, get_text);

    assert_eq!(results.len(), 1);
    assert!(results[0].item.text.join("").contains("4653"));
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find matching item when searching by author
#[test]
fn should_find_matching_item_when_searching_by_author() {
    let items = items();
    let results = match_("damaneice", &items, get_text);

    assert_eq!(results.len(), 1);
    assert!(results[0].item.text.join("").contains("damaneice"));
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find matching item when by title
#[test]
fn should_find_matching_item_when_by_title() {
    let items = items();
    let results = match_("awesome feature", &items, get_text);

    assert_eq!(results.len(), 1);
    assert!(results[0].item.text.join("").contains("awesome feature"));
}

// GHD: unit/fuzzy-find-test.ts › fuzzy find › should find nothing
#[test]
fn should_find_nothing() {
    let items = items();
    let results = match_("$%^", &items, get_text);

    assert_eq!(results.len(), 0);
}
