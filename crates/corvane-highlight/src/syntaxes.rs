//! The grammar sets, both compiled in: two-face's collection (bat's Sublime
//! grammars) first, then the `syntax-core` set (syntect's default packages
//! plus 330 TextMate grammars for languages nothing else covers, converted
//! from GitHub Linguist's collection by tools/tm-grammars:
//! `assets/syntaxes.packdump`), so the TextMate additions two-face lacks
//! still apply ([`sets`]).

use std::sync::{Arc, OnceLock};

use syntect::parsing::SyntaxSet;

/// The core set: syntect's defaults plus the converted TextMate grammars.
fn core() -> &'static Arc<SyntaxSet> {
    static SET: OnceLock<Arc<SyntaxSet>> = OnceLock::new();
    SET.get_or_init(|| {
        // the tools that write the dump (`pack-builder`) run without it
        #[cfg(not(feature = "pack-builder"))]
        {
            let dump: &[u8] = include_bytes!("../assets/syntaxes.packdump");
            match syntect::dumps::from_reader::<SyntaxSet, _>(dump) {
                Ok(set) => return Arc::new(set),
                Err(err) => tracing::warn!("the compiled-in grammar dump: {err}"),
            }
        }
        Arc::new(SyntaxSet::load_defaults_newlines())
    })
}

/// The sets diffs highlight with, in lookup order: [`current`], then the
/// core set (the TextMate additions two-face lacks).
pub fn sets() -> Vec<Arc<SyntaxSet>> {
    vec![current(), core().clone()]
}

/// The grammar set diffs highlight with first: two-face's collection.
pub fn current() -> Arc<SyntaxSet> {
    static SET: OnceLock<Arc<SyntaxSet>> = OnceLock::new();
    SET.get_or_init(|| Arc::new(two_face::syntax::extra_newlines()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_set_knows_rust_and_markdown() {
        let set = current();
        assert!(set.find_syntax_by_extension("rs").is_some());
        assert!(set.find_syntax_by_extension("md").is_some());
    }

    #[test]
    fn the_core_set_follows_two_faces() {
        let sets = sets();
        assert_eq!(sets.len(), 2);
        // two-face has TOML, syntect's defaults do not
        assert!(sets[0].find_syntax_by_extension("toml").is_some());
    }
}
