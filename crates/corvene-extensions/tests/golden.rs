//! The run-time converter must agree with tools/tm-grammars/sync.py, the
//! build-time one: the Linguist grammars under tests/fixtures/linguist were
//! converted by the Python script (the `.sublime-syntax` files) and are
//! converted again here from their TextMate JSON; the two documents must
//! say the same thing.

use std::path::Path;

use corvene_extensions::tm::convert::{Options, convert};
use corvene_extensions::tm::emit::emit;
use corvene_extensions::tm::{GrammarFormat, parse_text};
use corvene_extensions::value::{Value, parse_yaml};

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/linguist")
}

/// Maps with sorted keys, so emission order does not matter; empty
/// contexts are dropped from both sides.
fn normalize(value: &Value) -> Value {
    match value {
        Value::Map(entries) => {
            let mut out: Vec<(String, Value)> = entries
                .iter()
                .filter(|(_, v)| !matches!(v, Value::List(l) if l.is_empty()))
                .map(|(k, v)| (k.clone(), normalize(v)))
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(out)
        }
        Value::List(items) => Value::List(items.iter().map(normalize).collect()),
        other => other.clone(),
    }
}

fn check(scope: &str) {
    let json = std::fs::read_to_string(fixtures().join(format!("{scope}.json"))).expect("json");
    let expected_text =
        std::fs::read_to_string(fixtures().join(format!("{scope}.sublime-syntax"))).expect("yaml");
    let expected = parse_yaml(&expected_text).expect("expected yaml");
    let grammar = parse_text(&json, GrammarFormat::Json).expect("grammar");
    let options = Options {
        name: expected.str_of("name").map(str::to_string),
        file_extensions: expected.strings_of("file_extensions"),
        first_line_match: expected.str_of("first_line_match").map(str::to_string),
        hidden: expected
            .get("hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let (syntax, _report) = convert(&grammar, &options);
    let actual = parse_yaml(&emit(&syntax)).expect("emitted yaml");
    let (actual, expected) = (normalize(&actual), normalize(&expected));
    if actual != expected {
        // point at the first differing context
        let contexts = |v: &Value| v.get("contexts").cloned().unwrap_or(Value::Null);
        let (a, e) = (contexts(&actual), contexts(&expected));
        if let (Value::Map(a), Value::Map(e)) = (&a, &e) {
            for (name, value) in a {
                let other = e.iter().find(|(k, _)| k == name).map(|(_, v)| v);
                if other != Some(value) {
                    panic!("{scope}: context {name} differs\n got: {value:?}\nwant: {other:?}");
                }
            }
            for (name, _) in e {
                assert!(
                    a.iter().any(|(k, _)| k == name),
                    "{scope}: missing context {name}"
                );
            }
        }
        assert_eq!(actual, expected, "{scope}: top-level fields differ");
    }
}

#[test]
fn etc_matches_the_build_time_conversion() {
    check("etc");
}

#[test]
fn go_mod_matches_the_build_time_conversion() {
    check("go.mod");
}

#[test]
fn go_sum_matches_the_build_time_conversion() {
    check("go.sum");
}
