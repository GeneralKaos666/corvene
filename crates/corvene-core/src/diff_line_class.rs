//! Corvene `1316-diff-moved-lines` and `1317-diff-stylistic-changes`: marks
//! changed diff lines that are not functional edits.
//!
//! - Moved (desktop/desktop#17002, like `git diff --color-moved=zebra
//!   --color-moved-ws=ignore-all-space`): a run of removed lines that comes
//!   back as a run of added lines elsewhere in the diff. As in git, a block
//!   needs at least 20 alphanumeric characters to count.
//! - Stylistic (desktop/desktop#20361): removed and added lines of one block
//!   of changes whose code tokens match once whitespace, line breaks, quote
//!   style, trailing commas and semicolons and redundant wrapping parens are
//!   set aside (a reformatter re-wrapping a call, re-indenting, swapping
//!   quotes).
//!
//! GitHub Desktop has neither: every changed line is an add or a delete.
//! The classification is a pure function of the rows so the UI runs it over
//! whatever rows it shows (expanded context included).

use std::collections::{HashMap, HashSet};

use corvene_models::DiffLineKind;

/// git's `COLOR_MOVED_MIN_ALNUM_COUNT`.
pub const MOVED_MIN_ALNUM: usize = 20;
/// Candidate added runs looked at per removed line (keeps common lines from
/// going quadratic).
const MAX_CANDIDATES: usize = 64;
/// Blocks of changes with more tokens than this are not checked for
/// stylistic changes.
const MAX_STYLISTIC_TOKENS: usize = 8_000;
/// How long one block's token diff may take (it runs while the diff
/// renders); past it the diff is coarser and finds fewer matches.
const TOKEN_DIFF_BUDGET: std::time::Duration = std::time::Duration::from_millis(30);
/// Diffs with more rows than this are not classified at all.
pub const MAX_ROWS: usize = 100_000;

/// One row as the classifier sees it.
#[derive(Clone, Copy, Debug)]
pub struct ClassLine<'a> {
    pub kind: DiffLineKind,
    pub text: &'a str,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// What a changed row is besides an add or a delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineMark {
    /// Part of a moved block. `alternate` flips between adjacent blocks
    /// (git's zebra); `counterpart` is the line number on the other side
    /// (old number for an added line, new number for a removed one).
    Moved {
        block: u32,
        alternate: bool,
        counterpart: Option<u32>,
    },
    /// Only whitespace or formatting changed.
    Stylistic,
}

/// Which marks to compute.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClassOptions {
    pub moved: bool,
    pub stylistic: bool,
}

impl ClassOptions {
    pub fn any(self) -> bool {
        self.moved || self.stylistic
    }
}

/// The mark of every row (same indexing as `lines`); `path` picks the
/// language rules for the stylistic check.
pub fn classify(lines: &[ClassLine], path: &str, options: ClassOptions) -> Vec<Option<LineMark>> {
    let mut marks = vec![None; lines.len()];
    if !options.any() || lines.len() > MAX_ROWS {
        return marks;
    }
    let groups = change_groups(lines);
    if options.moved {
        mark_moved(lines, &groups, &mut marks);
    }
    if options.stylistic {
        let lang = Lang::from_path(path);
        for range in group_ranges(&groups) {
            mark_stylistic(lines, range, lang, &mut marks);
        }
    }
    marks
}

/// For each row, the index of the block of consecutive changes it is in.
fn change_groups(lines: &[ClassLine]) -> Vec<Option<u32>> {
    let mut out = Vec::with_capacity(lines.len());
    let mut next = 0u32;
    let mut in_group = false;
    for line in lines {
        if is_change(line.kind) {
            if !in_group {
                in_group = true;
                next += 1;
            }
            out.push(Some(next - 1));
        } else {
            in_group = false;
            out.push(None);
        }
    }
    out
}

fn group_ranges(groups: &[Option<u32>]) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for (ix, g) in groups.iter().enumerate() {
        if g.is_none() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end == ix && groups[ix - 1] == *g => last.end = ix + 1,
            _ => out.push(ix..ix + 1),
        }
    }
    out
}

fn is_change(kind: DiffLineKind) -> bool {
    matches!(kind, DiffLineKind::Add | DiffLineKind::Delete)
}

fn squash_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn alnum_count(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

// ---- moved lines ----

fn mark_moved(lines: &[ClassLine], groups: &[Option<u32>], marks: &mut [Option<LineMark>]) {
    let keys: Vec<Option<String>> = lines
        .iter()
        .map(|l| is_change(l.kind).then(|| squash_whitespace(l.text)))
        .collect();
    let mut added: HashMap<&str, Vec<usize>> = HashMap::new();
    for (ix, line) in lines.iter().enumerate() {
        if line.kind == DiffLineKind::Add
            && let Some(key) = keys[ix].as_deref()
            && !key.is_empty()
        {
            added.entry(key).or_default().push(ix);
        }
    }
    let matches = |d: usize, a: usize| -> bool {
        if lines[d].kind != DiffLineKind::Delete || lines[a].kind != DiffLineKind::Add {
            return false;
        }
        if keys[d] != keys[a] {
            return false;
        }
        // in the same block of changes, a line that differs only in
        // whitespace was edited in place (a stylistic change), not moved
        groups[d] != groups[a] || lines[d].text == lines[a].text
    };
    let mut taken = vec![false; lines.len()];
    let mut pairs: Vec<(usize, usize, u32)> = Vec::new();
    let mut block = 0u32;
    let mut d = 0;
    while d < lines.len() {
        let start_ok =
            lines[d].kind == DiffLineKind::Delete && !taken[d] && alnum_count(lines[d].text) > 0;
        let Some(candidates) = start_ok
            .then(|| keys[d].as_deref().and_then(|k| added.get(k)))
            .flatten()
        else {
            d += 1;
            continue;
        };
        let mut best: Option<(usize, usize)> = None;
        for &a in candidates
            .iter()
            .filter(|&&a| !taken[a])
            .take(MAX_CANDIDATES)
        {
            let mut len = 0;
            while d + len < lines.len()
                && a + len < lines.len()
                && !taken[d + len]
                && !taken[a + len]
                && matches(d + len, a + len)
            {
                len += 1;
            }
            // a block does not end on blank lines
            while len > 0 && keys[d + len - 1].as_deref().is_some_and(str::is_empty) {
                len -= 1;
            }
            if len > 0 && best.is_none_or(|(_, l)| len > l) {
                best = Some((a, len));
            }
        }
        match best {
            Some((a, len))
                if (0..len)
                    .map(|k| alnum_count(lines[d + k].text))
                    .sum::<usize>()
                    >= MOVED_MIN_ALNUM =>
            {
                for k in 0..len {
                    taken[d + k] = true;
                    taken[a + k] = true;
                    pairs.push((d + k, a + k, block));
                }
                block += 1;
                d += len;
            }
            _ => d += 1,
        }
    }
    for &(d, a, block) in &pairs {
        marks[d] = Some(LineMark::Moved {
            block,
            alternate: false,
            counterpart: lines[a].new,
        });
        marks[a] = Some(LineMark::Moved {
            block,
            alternate: false,
            counterpart: lines[d].old,
        });
    }
    // zebra: a block touching the previous moved block on the same side
    // takes the other shade
    let mut previous: Option<(u32, bool)> = None;
    for (ix, mark) in marks.iter_mut().enumerate() {
        let continues = ix > 0 && lines[ix - 1].kind == lines[ix].kind;
        match mark {
            Some(LineMark::Moved {
                block, alternate, ..
            }) => {
                let shade = match previous {
                    Some((b, shade)) if continues && b == *block => shade,
                    Some((_, shade)) if continues => !shade,
                    _ => false,
                };
                *alternate = shade;
                previous = Some((*block, shade));
            }
            _ => previous = None,
        }
    }
}

// ---- stylistic changes ----

/// Language rules for normalising tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    /// JavaScript and TypeScript: line-final semicolons are optional.
    Script,
    /// Python: `(x,)` is a tuple, so a trailing comma before `)` matters.
    Python,
    Other,
}

impl Lang {
    /// `'` and `"` quote the same strings.
    fn swaps_quotes(self) -> bool {
        matches!(self, Lang::Script | Lang::Python)
    }

    fn from_path(path: &str) -> Lang {
        let ext = path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "vue" | "svelte" => {
                Lang::Script
            }
            "py" | "pyi" | "pyw" => Lang::Python,
            _ => Lang::Other,
        }
    }
}

/// One code token of a row: word or single other character.
#[derive(Debug)]
struct Token<'a> {
    text: &'a str,
    row: usize,
    line_end: bool,
}

/// `'` reads as `"` where both quote the same strings (`swap_quotes`).
fn tokens_of<'a>(text: &'a str, row: usize, swap_quotes: bool, out: &mut Vec<Token<'a>>) {
    let first = out.len();
    let mut word: Option<usize> = None;
    for (ix, c) in text.char_indices() {
        let is_word = c.is_alphanumeric() || c == '_';
        if let Some(start) = word
            && !is_word
        {
            out.push(Token {
                text: &text[start..ix],
                row,
                line_end: false,
            });
            word = None;
        }
        if is_word {
            word.get_or_insert(ix);
        } else if !c.is_whitespace() {
            let token = match c {
                '\'' if swap_quotes => "\"",
                _ => &text[ix..ix + c.len_utf8()],
            };
            out.push(Token {
                text: token,
                row,
                line_end: false,
            });
        }
    }
    if let Some(start) = word {
        out.push(Token {
            text: &text[start..],
            row,
            line_end: false,
        });
    }
    if out.len() > first
        && let Some(last) = out.last_mut()
    {
        last.line_end = true;
    }
}

/// Drop the tokens formatting adds or removes: trailing commas before a
/// closing bracket, line-final semicolons (scripts) and parens that wrap a
/// whole assigned or returned value.
fn normalise(tokens: Vec<Token>, lang: Lang) -> Vec<Token> {
    let n = tokens.len();
    let mut drop = vec![false; n];
    let closing = |t: &str| matches!(t, ")" | "]" | "}");
    for ix in 0..n {
        let next = tokens.get(ix + 1).map(|t| t.text);
        match tokens[ix].text {
            "," => {
                // `(a,)` is a one-element tuple in Python and Rust; only
                // scripts allow a trailing comma in any call or list
                let before_close = match (lang, next) {
                    (Lang::Script, Some(t)) => closing(t),
                    (_, Some(t)) => matches!(t, "]" | "}"),
                    _ => false,
                };
                if before_close {
                    drop[ix] = true;
                }
            }
            ";" if lang == Lang::Script && (tokens[ix].line_end || next == Some("}")) => {
                drop[ix] = true;
            }
            _ => {}
        }
    }
    // matching parens
    let mut stack: Vec<usize> = Vec::new();
    let mut pair = vec![None; n];
    for (ix, token) in tokens.iter().enumerate() {
        match token.text {
            "(" => stack.push(ix),
            ")" => {
                if let Some(open) = stack.pop() {
                    pair[open] = Some(ix);
                }
            }
            _ => {}
        }
    }
    for open in 0..n {
        let Some(close) = pair[open] else { continue };
        // the token before `(` starts a value: a keyword, `:`, an
        // assignment (not `==`, `!=`, `<=`, `>=`) or an arrow `=>`
        let at = |back: usize| open.checked_sub(back).map(|p| tokens[p].text);
        let operator = |t: Option<&str>| matches!(t, Some("=" | "!" | "<" | ">"));
        let after_value_start = match at(1) {
            Some("return" | "yield" | "await" | ":") => true,
            Some("=") => !operator(at(2)),
            Some(">") => at(2) == Some("=") && !operator(at(3)),
            _ => false,
        };
        let next = (close + 1..n)
            .find(|&ix| !drop[ix])
            .map(|ix| tokens[ix].text);
        let ends_value =
            tokens[close].line_end || next.is_none_or(|t| matches!(t, ";" | "," | ")" | "]" | "}"));
        if after_value_start && ends_value {
            drop[open] = true;
            drop[close] = true;
        }
    }
    tokens
        .into_iter()
        .zip(drop)
        .filter_map(|(t, d)| (!d).then_some(t))
        .collect()
}

fn mark_stylistic(
    lines: &[ClassLine],
    range: std::ops::Range<usize>,
    lang: Lang,
    marks: &mut [Option<LineMark>],
) {
    let rows: Vec<usize> = range.filter(|&ix| marks[ix].is_none()).collect();
    if rows.is_empty() {
        return;
    }
    let mut del = Vec::new();
    let mut add = Vec::new();
    for &ix in &rows {
        match lines[ix].kind {
            DiffLineKind::Delete => tokens_of(lines[ix].text, ix, lang.swaps_quotes(), &mut del),
            DiffLineKind::Add => tokens_of(lines[ix].text, ix, lang.swaps_quotes(), &mut add),
            _ => {}
        }
    }
    if del.len() + add.len() > MAX_STYLISTIC_TOKENS {
        return;
    }
    let (del, add) = (normalise(del, lang), normalise(add, lang));
    // rows left with no tokens (blank lines, a lone `;` or `)`) follow the
    // rest of the block
    let mut has_tokens: HashMap<usize, bool> = rows.iter().map(|&r| (r, false)).collect();
    for t in del.iter().chain(&add) {
        has_tokens.insert(t.row, true);
    }
    let mut ok: HashMap<usize, bool> = rows.iter().map(|&r| (r, true)).collect();
    let mut links: HashMap<usize, HashSet<usize>> = HashMap::new();
    let del_text: Vec<&str> = del.iter().map(|t| t.text).collect();
    let add_text: Vec<&str> = add.iter().map(|t| t.text).collect();
    let deadline = std::time::Instant::now() + TOKEN_DIFF_BUDGET;
    for op in similar::capture_diff_slices_deadline(
        similar::Algorithm::Myers,
        &del_text,
        &add_text,
        Some(deadline),
    ) {
        let (tag, old, new) = op.as_tag_tuple();
        if tag == similar::DiffTag::Equal {
            for (o, n) in old.zip(new) {
                let (rd, ra) = (del[o].row, add[n].row);
                links.entry(rd).or_default().insert(ra);
                links.entry(ra).or_default().insert(rd);
            }
        } else {
            for t in del[old].iter().chain(&add[new]) {
                ok.insert(t.row, false);
            }
        }
    }
    // a row is stylistic only if every row it shares tokens with is too:
    // spread "functional" along the links
    let mut work: Vec<usize> = ok.iter().filter(|(_, ok)| !**ok).map(|(r, _)| *r).collect();
    while let Some(row) = work.pop() {
        for linked in links.get(&row).into_iter().flatten() {
            if ok.insert(*linked, false) == Some(true) {
                work.push(*linked);
            }
        }
    }
    let token_rows: Vec<usize> = rows.iter().copied().filter(|r| has_tokens[r]).collect();
    let all_token_rows_ok = token_rows.iter().all(|r| ok[r]);
    for &row in &rows {
        let stylistic = if has_tokens[&row] {
            ok[&row] && links.contains_key(&row)
        } else {
            all_token_rows_ok
        };
        if stylistic {
            marks[row] = Some(LineMark::Stylistic);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(spec: &[&'static str]) -> Vec<ClassLine<'static>> {
        let (mut old, mut new) = (1u32, 1u32);
        spec.iter()
            .map(|s| {
                let (kind, text) = match s.split_at(1) {
                    ("+", t) => (DiffLineKind::Add, t),
                    ("-", t) => (DiffLineKind::Delete, t),
                    ("@", t) => (DiffLineKind::Hunk, t),
                    (_, t) => (DiffLineKind::Context, t),
                };
                let (o, n) = match kind {
                    DiffLineKind::Add => (None, Some(new)),
                    DiffLineKind::Delete => (Some(old), None),
                    DiffLineKind::Context => (Some(old), Some(new)),
                    DiffLineKind::Hunk => (None, None),
                };
                if o.is_some() {
                    old += 1;
                }
                if n.is_some() {
                    new += 1;
                }
                ClassLine {
                    kind,
                    text,
                    old: o,
                    new: n,
                }
            })
            .collect()
    }

    const BOTH: ClassOptions = ClassOptions {
        moved: true,
        stylistic: true,
    };

    fn kinds(marks: &[Option<LineMark>]) -> String {
        marks
            .iter()
            .map(|m| match m {
                None => '.',
                Some(LineMark::Stylistic) => 's',
                Some(LineMark::Moved {
                    alternate: false, ..
                }) => 'm',
                Some(LineMark::Moved {
                    alternate: true, ..
                }) => 'M',
            })
            .collect()
    }

    #[test]
    fn moved_block_is_marked_on_both_sides() {
        let l = lines(&[
            "-fn helper() -> usize {",
            "-    compute_something()",
            "-}",
            " fn main() {",
            "     run();",
            " }",
            "+fn helper() -> usize {",
            "+    compute_something()",
            "+}",
        ]);
        let marks = classify(&l, "a.rs", BOTH);
        assert_eq!(kinds(&marks), "mmm...mmm");
        assert_eq!(
            marks[0],
            Some(LineMark::Moved {
                block: 0,
                alternate: false,
                counterpart: Some(4)
            })
        );
        assert_eq!(
            marks[6],
            Some(LineMark::Moved {
                block: 0,
                alternate: false,
                counterpart: Some(1)
            })
        );
    }

    #[test]
    fn short_blocks_are_not_moves() {
        let l = lines(&["-x = 1", " a", "+x = 1"]);
        assert_eq!(kinds(&classify(&l, "a", BOTH)), "...");
    }

    #[test]
    fn moved_with_new_indentation_still_counts() {
        let l = lines(&[
            "-let total = items.iter().sum();",
            " context",
            "+    let total = items.iter().sum();",
        ]);
        assert_eq!(kinds(&classify(&l, "a.rs", BOTH)), "m.m");
    }

    #[test]
    fn adjacent_blocks_alternate() {
        let l = lines(&[
            "-first_block_function_call_here();",
            "-second_block_function_call_here();",
            " middle",
            "+second_block_function_call_here();",
            "+first_block_function_call_here();",
        ]);
        assert_eq!(kinds(&classify(&l, "a", BOTH)), "mM.mM");
    }

    #[test]
    fn reindent_in_place_is_stylistic_not_moved() {
        let l = lines(&[
            "-  let value = compute_the_value(input);",
            "+    let value = compute_the_value(input);",
        ]);
        assert_eq!(kinds(&classify(&l, "a.rs", BOTH)), "ss");
    }

    #[test]
    fn rewrapped_call_is_stylistic() {
        let l = lines(&[
            "-foo(alpha, beta, 'gamma');",
            "+foo(",
            "+  alpha,",
            "+  beta,",
            "+  \"gamma\",",
            "+)",
        ]);
        assert_eq!(kinds(&classify(&l, "a.ts", BOTH)), "ssssss");
    }

    #[test]
    fn wrapping_parens_are_stylistic() {
        let l = lines(&[
            "-  return <div>hi</div>;",
            "+  return (",
            "+    <div>hi</div>",
            "+  );",
        ]);
        assert_eq!(kinds(&classify(&l, "a.tsx", BOTH)), "ssss");
    }

    #[test]
    fn functional_change_is_not_stylistic() {
        let l = lines(&["-foo(alpha, beta)", "+foo(alpha, gamma)"]);
        assert_eq!(kinds(&classify(&l, "a", BOTH)), "..");
    }

    #[test]
    fn mixed_block_marks_only_the_stylistic_rows() {
        let l = lines(&["-let a   = 1;", "-let b = 2;", "+let a = 1;", "+let b = 3;"]);
        assert_eq!(kinds(&classify(&l, "a.rs", BOTH)), "s.s.");
    }

    #[test]
    fn semicolons_matter_outside_scripts() {
        let l = lines(&["-    value;", "+    value"]);
        assert_eq!(kinds(&classify(&l, "a.rs", BOTH)), "..");
        assert_eq!(kinds(&classify(&l, "a.js", BOTH)), "ss");
    }

    #[test]
    fn language_rules_keep_real_changes() {
        // a char and a string in Rust
        let l = lines(&["-let c = 'a';", "+let c = \"a\";"]);
        assert_eq!(kinds(&classify(&l, "a.rs", BOTH)), "..");
        assert_eq!(kinds(&classify(&l, "a.ts", BOTH)), "ss");
        // a one-element tuple in Rust
        let l = lines(&["-let t = (a,);", "+let t = (a);"]);
        assert_eq!(kinds(&classify(&l, "a.rs", BOTH)), "..");
        // parens after a comparison change precedence
        let l = lines(&["-return a == (b || c);", "+return a == b || c;"]);
        assert_eq!(kinds(&classify(&l, "a.ts", BOTH)), "..");
        let l = lines(&["-const f = () => (value);", "+const f = () => value;"]);
        assert_eq!(kinds(&classify(&l, "a.ts", BOTH)), "ss");
    }

    #[test]
    fn python_tuple_comma_matters() {
        let l = lines(&["-x = (a,)", "+x = (a)"]);
        assert_eq!(kinds(&classify(&l, "a.py", BOTH)), "..");
    }

    #[test]
    fn pure_additions_are_functional_but_blank_lines_are_not() {
        let l = lines(&["+do_something();", " ctx", "+", "+   "]);
        assert_eq!(kinds(&classify(&l, "a", BOTH)), "..ss");
        let l = lines(&["+do_something();", "+"]);
        assert_eq!(kinds(&classify(&l, "a", BOTH)), "..");
    }

    #[test]
    fn options_select_marks() {
        let l = lines(&["-  a_long_identifier_name();", "+a_long_identifier_name();"]);
        let none = ClassOptions::default();
        assert_eq!(kinds(&classify(&l, "a", none)), "..");
        let moved_only = ClassOptions {
            moved: true,
            stylistic: false,
        };
        assert_eq!(kinds(&classify(&l, "a", moved_only)), "..");
    }
}
