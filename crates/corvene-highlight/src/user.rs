//! Grammars the user installed (Corvene addition: language extensions,
//! `corvene_extensions`). Two registries feed this layer: a syntect set of
//! the converted TextMate / Sublime grammars, installed whole by
//! [`install_set`], and the user tree-sitter grammars registered in
//! [`super::treesitter::library`]. Both are consulted before the built-in
//! engines for the files an extension claims when the extension *prefers*
//! its grammar, and after them otherwise ([`crate::highlight_lines_with`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use syntect::parsing::SyntaxSet;

use crate::Span;

/// The user syntect set and who owns each syntax in it.
pub struct UserSyntaxes {
    pub set: Arc<SyntaxSet>,
    /// syntax name → (extension id, prefers its grammar over the built-ins)
    pub owners: HashMap<String, (String, bool)>,
}

/// Which registry claimed a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimKind {
    Syntect,
    TreeSitter,
}

/// An extension's claim on a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserClaim {
    pub extension: String,
    pub preferred: bool,
    pub kind: ClaimKind,
}

fn store() -> &'static RwLock<Option<Arc<UserSyntaxes>>> {
    static STORE: OnceLock<RwLock<Option<Arc<UserSyntaxes>>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(None))
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Changes whenever the user set changes (installed, cleared, a preference
/// flipped), so views re-highlight and caches drop stale tokens.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

fn bump() {
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Use `set` as the user grammars; `owners` maps each syntax's name to its
/// extension and preference.
pub fn install_set(set: SyntaxSet, owners: HashMap<String, (String, bool)>) {
    if let Ok(mut guard) = store().write() {
        *guard = Some(Arc::new(UserSyntaxes {
            set: Arc::new(set),
            owners,
        }));
    }
    bump();
}

/// Drop the user set.
pub fn clear() {
    if let Ok(mut guard) = store().write()
        && guard.take().is_some()
    {
        bump();
    }
}

/// Whether any user grammar is installed.
pub fn installed() -> bool {
    store().read().is_ok_and(|g| g.is_some()) || super::treesitter::has_user_grammars()
}

/// Flip an extension's preference without rebuilding the set.
pub fn set_owner_preference(extension: &str, preferred: bool) {
    let mut changed = false;
    if let Ok(mut guard) = store().write()
        && let Some(current) = guard.as_ref()
    {
        let mut owners = current.owners.clone();
        for (id, flag) in owners.values_mut() {
            if id == extension && *flag != preferred {
                *flag = preferred;
                changed = true;
            }
        }
        if changed {
            *guard = Some(Arc::new(UserSyntaxes {
                set: current.set.clone(),
                owners,
            }));
        }
    }
    if changed {
        bump();
    }
}

fn current() -> Option<Arc<UserSyntaxes>> {
    store().read().ok().and_then(|g| g.clone())
}

/// The extension claiming `path`, if any: a syntect grammar by extension,
/// file name or first line, else a user tree-sitter grammar.
pub fn claims(path: &str, first_line: &str) -> Option<UserClaim> {
    if let Some(user) = current()
        && let Some(syntax) = crate::syntax_for(&user.set, path, first_line)
        && let Some((extension, preferred)) = user.owners.get(&syntax.name)
    {
        return Some(UserClaim {
            extension: extension.clone(),
            preferred: *preferred,
            kind: ClaimKind::Syntect,
        });
    }
    super::treesitter::user_claim(path, first_line).map(|(extension, preferred)| UserClaim {
        extension,
        preferred,
        kind: ClaimKind::TreeSitter,
    })
}

/// Highlight with a user grammar. `preferred_only` takes only extensions
/// that prefer their grammar over the built-ins; `prefix_only` skips user
/// tree-sitter grammars (they parse whole files, see
/// [`crate::highlight_prefix`]).
pub(crate) fn highlight(
    path: &str,
    lines: &[&str],
    stop: usize,
    preferred_only: bool,
    prefix_only: bool,
) -> Option<Vec<Vec<Span>>> {
    let first = lines.first().copied().unwrap_or("");
    if let Some(user) = current()
        && let Some(syntax) = crate::syntax_for(&user.set, path, first)
        && let Some((_, preferred)) = user.owners.get(&syntax.name)
        && (*preferred || !preferred_only)
    {
        return Some(crate::syntect_highlight_in(&user.set, syntax, lines, stop));
    }
    if prefix_only {
        return None;
    }
    super::treesitter::highlight_user(path, lines, crate::MAX_HIGHLIGHT_BYTES, preferred_only)
}

/// Syntax names in the user set (the details panel).
pub fn syntax_names() -> Vec<String> {
    current()
        .map(|u| u.set.syntaxes().iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenClass;
    use std::sync::Mutex;
    use syntect::parsing::{SyntaxDefinition, SyntaxSetBuilder};

    /// The user set is process-wide: these tests take turns.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    const ZZZ: &str = "%YAML 1.2\n---\nname: Zzz\nscope: source.zzz\nfile_extensions:\n  - zzz\n  - js\ncontexts:\n  main:\n    - match: \\b(function|zork)\\b\n      scope: comment.keyword.zzz\n";

    fn install(preferred: bool) {
        let def = SyntaxDefinition::load_from_str(ZZZ, true, None).expect("syntax");
        let mut builder = SyntaxSetBuilder::new();
        builder.add(def);
        install_set(
            builder.build(),
            HashMap::from([("Zzz".to_string(), ("local.zzz".to_string(), preferred))]),
        );
    }

    fn first_class(path: &str, line: &str) -> Option<TokenClass> {
        crate::highlight_lines(path, [line]).and_then(|s| s[0].first().map(|s| s.class))
    }

    #[test]
    fn a_user_grammar_claims_its_suffix() {
        let _guard = lock();
        let before = generation();
        install(true);
        assert_ne!(generation(), before);
        assert_eq!(
            claims("dir/a.zzz", ""),
            Some(UserClaim {
                extension: "local.zzz".into(),
                preferred: true,
                kind: ClaimKind::Syntect,
            })
        );
        assert_eq!(first_class("a.zzz", "zork x"), Some(TokenClass::Comment));
        assert_eq!(syntax_names(), vec!["Zzz".to_string()]);
        assert!(installed());
        clear();
        assert_eq!(claims("a.zzz", ""), None);
        assert_eq!(first_class("a.zzz", "zork x"), None);
    }

    #[test]
    fn preference_decides_against_the_built_in_mode() {
        let _guard = lock();
        // `.js` has a CodeMirror mode: `function` is a keyword there
        install(true);
        assert_eq!(first_class("a.js", "function f() {}"), Some(TokenClass::Comment));
        set_owner_preference("local.zzz", false);
        assert_eq!(first_class("a.js", "function f() {}"), Some(TokenClass::Keyword));
        assert_eq!(claims("a.js", "").map(|c| c.preferred), Some(false));
        // a file nothing built in covers still gets the user grammar
        assert_eq!(first_class("a.zzz", "zork x"), Some(TokenClass::Comment));
        // the prefix path agrees
        let prefix = crate::highlight_prefix("a.zzz", &["zork", "zork"], 1).expect("prefix");
        assert_eq!(prefix.len(), 1);
        assert!(!crate::has_builtin_highlighting(crate::Engine::GitHubDesktop, "a.zzz", ""));
        assert!(crate::has_builtin_highlighting(crate::Engine::GitHubDesktop, "a.js", ""));
        clear();
    }
}
