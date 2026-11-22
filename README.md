# Klause

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#license)

Klause is a Rust library for storing, reading, and querying annotated text.
Use a `Document` for one text or a `Corpus` to search and aggregate across several.
Klause keeps text and stored annotations in memory; file and database I/O belong to your application.

## Definitions

- **Document** — An ID, metadata, append-only UTF-8 text, and named annotation layers. Positions are document-local byte offsets into NFC-normalized text.
- **Corpus** — Documents with unique IDs; queries run independently on the selected documents and can aggregate the results.
- **Span** — An annotation with an ID, attributes, and a target: one or more text ranges (including points), or another span in the same document.
- **Layer** — A named group of spans. A stored layer owns its spans; a source layer reads them live from application storage.
- **Value** — A scalar: null, bool, integer, float, or string. An attribute key can hold multiple values.
- **Selection** — A sorted set of text ranges that can be unioned, intersected, and subtracted.
- **Passage** — A range of text and the spans that touch it in the included layers.
- **Query** — A builder that narrows or expands hits and returns selections, passages, spans, or counts.

## Installation

Klause is not published on crates.io yet. Add it as a Git dependency:

```toml
[dependencies]
klause = { git = "https://github.com/aridinstitute/klause" }
```

For reproducible builds, pin a commit with `rev`. Disable default features
to build Klause with `alloc` and without `std`.

## Usage

```rust
use klause::{Document, spans_in};

fn main() -> Result<(), klause::Error> {
    let mut doc = Document::new("greeting").with_layer("words");
    doc.push("Hello, ")?;
    doc.push_span("words", "world", |span| span.with_attr("pos", "noun"))?;
    doc.push("!")?;

    let passage = doc
        .find()
        .within(spans_in("words").with_attr("pos", "noun"))
        .get_passage()?;
    assert_eq!(passage.text(), "world");
    Ok(())
}
```

Use `get_passage` to render text with its annotations, `get_spans` to
retrieve matching annotations, or `count_spans` and `count_values` to aggregate.
Passages own their annotation data and can outlive the document borrow. To load
application-managed annotations on demand, add a source layer with
`Layer::source` and implement `SpanSource`.

### Querying multiple documents

```rust
use klause::{Corpus, Document, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Corpus::new();
    for (id, text) in [("fr", "Café"), ("en", "cafe")] {
        let mut doc = Document::new(id).with_layer("words");
        doc.push_span("words", text, |span| span.with_attr("pos", "noun"))?;
        corpus.add_document(doc)?;
    }

    let counts = corpus.find().with_text_folded("cafe").count_values("words", "pos")?;
    assert_eq!(counts, [(Value::from("noun"), 2)]);
    Ok(())
}
```

Use `in_documents` to select IDs or `filter_documents` to select by metadata or
layer availability. A required layer missing from a selected document is an
error that names the document. Selection, passage, and span results retain their
document IDs. Text matches, offsets, and span targets never cross document
boundaries. Corpus-wide annotation layers, cross-document links, and built-in
persistence are not implemented.

Run the [quickstart](examples/quickstart.rs) or [corpus example](examples/corpus.rs)
with `cargo run --example quickstart` or `cargo run --example corpus`. See the
[design](DESIGN.md) for semantics, or run `cargo doc --no-deps --open` for the API reference.

## Tests and benchmarks

```sh
cargo test
cargo check --no-default-features
cargo bench --bench bible_workload
cargo bench --bench bible_workload -- --rss
```

The [benchmark notes](benches/README.md) cover the full synthetic Bible workload,
including cold and warm queries, folded search, and resident memory. The
[corpus benchmarks](benches/README.md#cross-document-corpus) measure queries
across up to 66 synthetic documents.

### Real Bible fixture

The 66-book [Berean Standard Bible USJ fixture](tests/fixtures/bsb_usj/) is used
by both the integration test and the Criterion benchmark:

```sh
cargo test --test bsb_usj -- --nocapture
cargo bench --bench bsb_usj
```

The test-only loader builds one `Document` with book, chapter, verse, word,
structure, and footnote layers. It indexes verse text, leaving editorial headings
and footnote bodies out of the searchable text. The benchmark measures loading
and conversion, word and verse filters, and folded phrase search separately
from verse materialization.

The Berean Standard Bible text is [dedicated to the public domain](https://berean.bible/terms.htm).

## Minimum supported Rust version (MSRV)

Klause requires Rust 1.88 or newer.

## Contributing

Bug reports and feature requests are welcome in [GitHub Issues](https://github.com/aridinstitute/klause/issues).

## License

This project is licensed under the [MIT License](LICENSE).
