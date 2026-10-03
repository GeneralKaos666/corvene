# ext-index

Builds the offline language extension index Corvene ships on the rolling
`packs` release: `language-extensions-index.json.gz`, file suffix → the
extensions of the open registries (Open VSX, Zed, Pulsar) that cover it.
The diff's "no syntax highlighting for .foo" hint and the Language
Extensions dialog's Find tab read it first and ask the registries only
when it has nothing (`crates/corvene-extensions/src/index.rs`).

    python3 tools/ext-index/crawl.py language-extensions-index.json --pages 10

`.github/workflows/ext-index.yml` runs it every week and uploads the
gzipped file plus a manifest entry of kind `language-extensions-index`
(version `YYYY.MM.DD`) to the `packs` release. Older apps skip the entry.
