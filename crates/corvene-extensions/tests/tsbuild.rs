//! A grammar built from source, offline: the csv grammar's generated
//! parser from `target/grammar-src` (tools/ts-queries/fetch.py) packed
//! into a tarball served by `file://`, compiled with the system compiler,
//! verified with `nm`. Skipped when the sources or a compiler are missing.

use std::io::Write;
use std::path::Path;

use corvene_extensions::tsbuild::{BuildPlan, Stage, build, compiler::Compiler};

fn tar_gz_of(src: &Path, top: &str, out: &Path) {
    let file = std::fs::File::create(out).expect("create");
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut builder = tar::Builder::new(gz);
    builder
        .append_dir_all(format!("{top}/src"), src)
        .expect("append");
    builder
        .into_inner()
        .expect("tar")
        .finish()
        .expect("gz")
        .flush()
        .expect("flush");
}

#[test]
fn builds_a_grammar_from_a_local_tarball() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/grammar-src/csv/src");
    if !src.join("parser.c").is_file() {
        eprintln!("skipped: no grammar sources under target/grammar-src");
        return;
    }
    let compiler = match Compiler::find() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("skipped: {err}");
            return;
        }
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    let tarball = tmp.path().join("source.tar.gz");
    tar_gz_of(&src, "tree-sitter-csv-abcdef12", &tarball);
    let cache = tmp.path().join("cache");
    let mut plan = BuildPlan::new(
        "test.csv",
        "csv",
        "https://github.com/tree-sitter-grammars/tree-sitter-csv",
        "abcdef1234567890",
        None,
        &cache,
    )
    .expect("plan");
    plan.tarball_url = format!("file://{}", tarball.display());
    let mut stages = Vec::new();
    let verify = |library: &Path| -> Result<(), String> {
        let output = std::process::Command::new("nm")
            .arg("-gU")
            .arg(library)
            .output()
            .map_err(|e| e.to_string())?;
        let symbols = String::from_utf8_lossy(&output.stdout);
        if symbols.contains("corvene_grammars_v1") && symbols.contains("tree_sitter_csv") {
            Ok(())
        } else {
            Err(format!("symbols missing:\n{symbols}"))
        }
    };
    let built = build(&plan, &compiler, &mut |stage| stages.push(stage), &verify).expect("builds");
    assert!(built.library.is_file());
    assert_eq!(built.symbol, "tree_sitter_csv");
    assert!(built.log.is_file());
    assert!(
        stages
            .iter()
            .any(|s| matches!(s, Stage::Compiling(f) if f == "parser.c")),
        "{stages:?}"
    );
    assert!(matches!(stages.last(), Some(Stage::Verifying)));
    // a second build reuses the unpacked source and refuses a failing verifier
    let err = build(&plan, &compiler, &mut |_| {}, &|_| Err("nope".to_string()))
        .expect_err("verifier fails");
    assert!(err.to_string().contains("nope"), "{err}");
    assert!(built.library.is_file(), "the first library stays");
}
