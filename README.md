# Klause

[![Crates.io](https://img.shields.io/crates/v/klause.svg)](https://crates.io/crates/klause)
[![Docs.rs](https://docs.rs/klause/badge.svg)](https://docs.rs/klause)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#license)

Klause is a document data model designed for efficient text extraction, annotation, and searching.

## Overview

Klause is a Rust library that makes working with annotated documents easy. The API is designed to be
easy to learn and use, and the data model is very flexible, allowing annotation and querying of documents
such as ancient texts, the Bible, and more.

- **Flexible Data Model** — Text is annotated by spans and attributes, allowing lots of flexibility in data storage.
- **Plain-Text Model** — Plain, unannotated text is stored as its own property, allowing tradition text querying using tools like regex.
- **Speed** — Klause is designed to filter and aggregate as fast as possible.

## Definitions

- **Document** — An ID, attributes, and a text that only ever grows, plus the stored layers of spans annotated on it.
- **Span** — One annotation in a layer: a target (text ranges, a point, another span, or a link), attributes, and an ID.
- **Layer** — A named set of spans. Stored layers live in the document; source layers are read live from your own storage.
- **Value** — An attribute value: null, bool, integer, float, string, or a list of those.
- **Selection** — A sorted set of text ranges that can be unioned, intersected, and subtracted.
- **Passage** — A range of text with every span that touches it, ready to render.
- **Query** — A fluent builder that filters text and returns a selection, passages, spans, or counts.
- **Corpus** — Several documents, for queries across them, links between them, and layers they share.
- **Anchor** — A checksum, quote, and surrounding context that finds a stored span again in a new edition.

## Installation

Add Klause to your `Cargo.toml`:

```toml
[dependencies]
klause = "0.1.0"
```

Or with [cargo-add](https://github.com/rust-lang/cargo):

```sh
cargo add klause
```

## Usage

```rust
use klause::Document;

fn main() {
    let klause = Klause::new();
    println!("{klause:?}");
}
```

For more examples, see the [`examples/`](./examples) directory and the
[API documentation](https://docs.rs/klause).

## Minimum supported Rust version (MSRV)

Klause requires Rust 1.85 or newer.

## Contributing

I'm not currently accepting contributions, but feel free to make an issue if you want a feature or find a bug!

## License

This project is licensed under the [MIT License](LICENSE).
