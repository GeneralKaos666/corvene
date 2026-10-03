//! Each editor format is found, read and prepared into an installed folder.

use std::io::Write;
use std::path::{Path, PathBuf};

use corvene_extensions::archive::Limits;
use corvene_extensions::install::{self, GrammarKind, Resolution, Source, SourceKind, Status};
use corvene_extensions::manifest::{Format, GrammarRef};
use corvene_extensions::scan::{scan, scan_dir};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn source() -> Source {
    Source {
        kind: SourceKind::LocalFolder,
        url: None,
        path: None,
        sha256: None,
        editor: None,
        registry_id: None,
    }
}

fn resolver(grammar: &GrammarRef) -> Resolution {
    match grammar {
        GrammarRef::TreeSitter { name, .. } if name == "foo" => {
            Resolution::Bundled { name: "foo".into() }
        }
        _ => Resolution::NeedsBuild,
    }
}

#[test]
fn vscode_extension() {
    let scanned = scan_dir(&fixtures().join("vscode-ext")).expect("scan");
    let m = &scanned.manifest;
    assert_eq!(m.format, Some(Format::VsCode));
    assert_eq!(m.name, "sample-langs");
    assert_eq!(m.display_name.as_deref(), Some("Sample Languages"));
    assert_eq!(m.publisher.as_deref(), Some("corvene-tests"));
    assert_eq!(
        m.repository.as_deref(),
        Some("https://github.com/example/sample-langs")
    );
    assert_eq!(m.grammars.len(), 3);
    let foo = m.languages.iter().find(|l| l.id == "foo").expect("foo");
    assert_eq!(foo.suffixes, vec!["foo", "foo2", "foo.bar"]);
    assert_eq!(foo.filenames, vec!["foofile", ".fooconfig"]);
    assert_eq!(foo.first_line.as_deref(), Some("^#!.*\\bfoo\\b"));
    assert_eq!(foo.grammar.as_deref(), Some("foo"));
    assert_eq!(
        m.languages
            .iter()
            .find(|l| l.id == "no-grammar")
            .and_then(|l| l.grammar.clone()),
        None
    );

    let out = tempfile::tempdir().expect("tempdir");
    let dir = out.path().join("local.sample-langs");
    let prepared =
        install::prepare::prepare(&scanned, "local.sample-langs", source(), &dir, &resolver)
            .expect("prepare");
    let md = &prepared.metadata;
    assert_eq!(md.display_name, "Sample Languages");
    assert_eq!(md.grammars.len(), 3);
    let foo = md
        .grammars
        .iter()
        .find(|g| g.name == "foo")
        .expect("foo grammar");
    assert_eq!(foo.status, Status::Ok);
    assert_eq!(foo.kind, GrammarKind::TextMate);
    assert!(dir.join(foo.file.as_deref().expect("file")).is_file());
    // the plist grammar's lookbehind is one Oniguruma rejects: rejected, not deferred
    let bar = md
        .grammars
        .iter()
        .find(|g| g.name == "bar")
        .expect("bar grammar");
    assert_eq!(bar.status, Status::Rejected, "{:?}", bar.error);
    assert!(bar.file.is_none());
    // the injection grammar is converted (hidden) and its injection noted
    let inject = md
        .grammars
        .iter()
        .find(|g| g.name == "inject.foo")
        .expect("inject");
    assert_eq!(inject.status, Status::Ok);
    assert!(dir.join("source/package.json").is_file());
    assert!(dir.join("report.json").is_file());
    assert!(dir.join(install::METADATA_FILE).is_file());
    assert!(md.usable());
    assert_eq!(install::read(&dir).expect("read").id, "local.sample-langs");
    let listed = install::list(out.path());
    assert_eq!(listed.len(), 1);
}

#[test]
fn vsix_archive_is_unpacked_and_scanned() {
    let root = fixtures().join("vscode-ext");
    let tmp = tempfile::tempdir().expect("tempdir");
    let vsix = tmp.path().join("sample-langs-1.2.3.vsix");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&vsix).expect("create"));
    let options = zip::write::SimpleFileOptions::default();
    writer
        .start_file("[Content_Types].xml", options)
        .expect("start");
    writer.write_all(b"<Types/>").expect("write");
    for file in [
        "package.json",
        "syntaxes/foo.tmLanguage.json",
        "syntaxes/bar.tmLanguage",
        "syntaxes/inject.json",
    ] {
        writer
            .start_file(format!("extension/{file}"), options)
            .expect("start");
        writer
            .write_all(&std::fs::read(root.join(file)).expect("read"))
            .expect("write");
    }
    writer.finish().expect("finish");
    let staging = tmp.path().join("staging");
    let scanned = scan(&vsix, &staging, &Limits::default()).expect("scan");
    assert_eq!(scanned.manifest.format, Some(Format::VsCode));
    assert_eq!(scanned.manifest.name, "sample-langs");
    assert!(scanned.root.ends_with("unpacked/extension"));
}

#[test]
fn atom_package() {
    let scanned = scan_dir(&fixtures().join("atom-pkg")).expect("scan");
    let m = &scanned.manifest;
    assert_eq!(m.format, Some(Format::Atom));
    assert_eq!(m.name, "language-baz");
    let baz = m.languages.iter().find(|l| l.id == "Baz").expect("baz");
    assert_eq!(baz.suffixes, vec!["baz", "bazfile"]);
    assert_eq!(baz.first_line.as_deref(), Some("^#!.*\\bbaz\\b"));
    let qux = m
        .grammars
        .iter()
        .find(|g| matches!(g, GrammarRef::TreeSitter { .. }))
        .expect("tree-sitter grammar");
    let GrammarRef::TreeSitter {
        name,
        repository,
        rev,
        ..
    } = qux
    else {
        unreachable!()
    };
    assert_eq!(name, "qux");
    assert_eq!(
        repository.as_deref(),
        Some("https://github.com/example/tree-sitter-qux")
    );
    assert_eq!(rev.as_deref(), Some("deadbeef"));

    let out = tempfile::tempdir().expect("tempdir");
    let prepared = install::prepare::prepare(
        &scanned,
        "local.language-baz",
        source(),
        &out.path().join("x"),
        &resolver,
    )
    .expect("prepare");
    let baz = prepared
        .metadata
        .grammars
        .iter()
        .find(|g| g.name == "Baz")
        .expect("baz");
    assert_eq!(baz.status, Status::Ok, "{:?}", baz.error);
    let text = std::fs::read_to_string(
        out.path()
            .join("x")
            .join(baz.file.as_deref().expect("file")),
    )
    .expect("read");
    assert!(text.contains("\\\\$[A-Za-z_]+"), "{text}");
    let qux = prepared
        .metadata
        .grammars
        .iter()
        .find(|g| g.name == "qux")
        .expect("qux");
    assert_eq!(qux.kind, GrammarKind::TreeSitter);
    assert_eq!(qux.status, Status::Rejected); // no queries in a legacy tree-sitter grammar
    assert_eq!(qux.resolution, Resolution::NeedsBuild);
}

#[test]
fn zed_extension() {
    let scanned = scan_dir(&fixtures().join("zed-ext")).expect("scan");
    let m = &scanned.manifest;
    assert_eq!(m.format, Some(Format::Zed));
    assert_eq!(m.name, "foo-zed");
    assert_eq!(m.display_name.as_deref(), Some("Foo for Zed"));
    assert_eq!(m.publisher.as_deref(), Some("Someone"));
    let lang = &m.languages[0];
    assert_eq!(lang.name.as_deref(), Some("Foo"));
    assert_eq!(lang.suffixes, vec!["foo", "foofile"]);
    assert_eq!(lang.filenames, vec!["foofile"]);
    assert_eq!(lang.grammar.as_deref(), Some("foo"));
    let GrammarRef::TreeSitter {
        repository,
        rev,
        highlights,
        injections,
        locals,
        wasm,
        ..
    } = &m.grammars[0]
    else {
        panic!("tree-sitter");
    };
    assert_eq!(
        repository.as_deref(),
        Some("https://github.com/example/tree-sitter-foo")
    );
    assert_eq!(
        rev.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert!(highlights.is_some() && injections.is_some() && locals.is_none());
    assert_eq!(wasm.as_deref(), Some(Path::new("grammars/foo.wasm")));

    let out = tempfile::tempdir().expect("tempdir");
    let prepared = install::prepare::prepare(
        &scanned,
        "zed.foo-zed",
        source(),
        &out.path().join("x"),
        &resolver,
    )
    .expect("prepare");
    let g = &prepared.metadata.grammars[0];
    assert_eq!(g.resolution, Resolution::Bundled { name: "foo".into() });
    assert_eq!(g.queries.as_deref(), Some("queries/foo"));
    assert!(out.path().join("x/queries/foo/highlights.scm").is_file());
    assert!(prepared.metadata.usable());
}

#[test]
fn tmbundle_sublime_and_raw() {
    let bundle = scan_dir(&fixtures().join("sample.tmbundle")).expect("scan");
    assert_eq!(bundle.manifest.format, Some(Format::TmBundle));
    assert_eq!(bundle.manifest.name, "Sample Bundle");
    assert_eq!(bundle.manifest.languages[0].suffixes, vec!["quux", "qx"]);

    let sublime = scan_dir(&fixtures().join("sublime-pkg")).expect("scan");
    assert_eq!(sublime.manifest.format, Some(Format::Sublime));
    assert_eq!(sublime.manifest.grammars.len(), 2);
    let corge = sublime
        .manifest
        .languages
        .iter()
        .find(|l| l.id == "source.corge")
        .expect("corge");
    assert_eq!(corge.suffixes, vec!["corge", "corgefile"]);
    assert_eq!(corge.filenames, vec!["corgefile"]);
    let out = tempfile::tempdir().expect("tempdir");
    let prepared = install::prepare::prepare(
        &sublime,
        "local.sublime-pkg",
        source(),
        &out.path().join("x"),
        &resolver,
    )
    .expect("prepare");
    assert!(
        prepared
            .metadata
            .grammars
            .iter()
            .all(|g| g.status == Status::Ok && g.kind == GrammarKind::Sublime)
    );

    let raw = scan(
        &fixtures().join("raw/grault.YAML-tmLanguage"),
        out.path(),
        &Limits::default(),
    )
    .expect("scan");
    assert_eq!(raw.manifest.format, Some(Format::Raw));
    assert_eq!(raw.manifest.name, "grault");
    assert_eq!(raw.manifest.display_name.as_deref(), Some("Grault"));
    assert_eq!(raw.manifest.languages[0].suffixes, vec!["grault"]);
    let prepared = install::prepare::prepare(
        &raw,
        "local.grault",
        source(),
        &out.path().join("y"),
        &resolver,
    )
    .expect("prepare");
    assert_eq!(
        prepared.metadata.grammars[0].status,
        Status::Ok,
        "{:?}",
        prepared.metadata.grammars[0].error
    );
}

#[test]
fn a_repository_is_searched_and_merged() {
    let scanned = scan_dir(&fixtures().join("repo")).expect("scan");
    let m = &scanned.manifest;
    assert_eq!(m.name, "repo");
    // the VS Code extension under packages/one/ext and the bundle, not node_modules
    assert!(m.grammars.iter().any(|g| g.name() == "foo"));
    assert!(m.grammars.iter().any(|g| g.name() == "source.quux"));
    let GrammarRef::TextMate { path, .. } =
        m.grammars.iter().find(|g| g.name() == "foo").expect("foo")
    else {
        panic!("textmate");
    };
    assert_eq!(
        path,
        Path::new("packages/one/ext/syntaxes/foo.tmLanguage.json")
    );
    let out = tempfile::tempdir().expect("tempdir");
    let prepared = install::prepare::prepare(
        &scanned,
        "github.example.repo",
        source(),
        &out.path().join("x"),
        &resolver,
    )
    .expect("prepare");
    assert!(
        prepared
            .metadata
            .grammars
            .iter()
            .any(|g| g.name == "foo" && g.status == Status::Ok)
    );
    assert!(
        prepared
            .metadata
            .grammars
            .iter()
            .any(|g| g.name == "source.quux" && g.status == Status::Ok)
    );
}

#[test]
fn nothing_usable_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("README.md"), "hi").expect("write");
    let err = scan_dir(dir.path()).expect_err("no extension");
    assert!(err.to_string().contains("no grammar"), "{err}");
}
