# Klause

Status: draft for review. No code yet.

Klause stores text and the annotations made on it, kept apart. The text is never marked up. Annotations point
into it by position, so any number of them can overlap, nest, cross, or disagree without touching the text or each
other. This is called stand-off annotation.

Use it when a text needs more than one kind of markup at once: words, lines, and paragraphs from an edition;
morphology from one scholar and corrections from another; a reader's highlights from a database. Klause keeps
them in named layers, answers queries across all of them, and returns text with every annotation that touches it,
ready to render.

It parses nothing. It has no source formats, reference syntax, or word rules of its own. You push text, describe
spans, and ask for ranges, spans, or IDs; your importer or domain crate decides what they mean.

## Philosophy

- **The text is a fact; annotations are opinions.** A document's text is normalised to NFC and only ever grows, so
  positions never move. Annotations can be wrong, overlap, contradict each other, or be replaced without touching it.
- **Every annotation is a span, and every span lives in a layer.** A layer is a unit of trust — one scholar, one
  importer, one database — with its own metadata. It can be read, saved, or left out of a result on its own.
- **Positions, not markup.** Spans are byte offsets into UTF-8 text, end-exclusive and on character boundaries, so
  a `&str` slices with them directly and most renderers accept them unchanged.
- **No domain knowledge.** Klause parses nothing and defines no vocabularies. Layers, keys, and values are yours,
  which is why one library serves editions, morphology, corrections, highlights, and links.
- **Your storage stays yours.** A source layer is read live from your own database; Klause never writes to it, so a
  change you save shows up in the next passage or query.
- **Results are shaped by the question.** Query filters never fail and may be written in any order; the final verb
  decides whether you get a selection, passages, spans, counts, or value frequencies.
- **Reading is the common case.** A passage carries the text with every span that touches it, in layer order, ready
  to render.

## Capabilities

| Capability | What it gives you |
| :--------- | :---------------- |
| [Documents](#documents) | Text that only grows and is never marked up, with document attributes and the byte range of every push. |
| [Spans](#spans) | Annotations over ranges, points, several ranges at once, other spans, or a link between two places. |
| [Layers](#layers) | Named, independent sets of spans, each with its own ID, metadata, and lifecycle. |
| [Passages](#reading-passages) | A range of text with every span that touches it, ready to render. |
| [Selections](#selections) | Sets of ranges that union, intersect, and subtract. |
| [Queries](#queries) | Fluent filters over position, attributes, text, and neighbours, with a final verb that shapes the result. |
| [Source layers](#source-layers) | Annotations read live from your own storage instead of the document. |
| [Corpora](#corpora) | Several documents queried together, with links between them and layers they share. |
| [Anchors](#keeping-spans-valid) | A checksum, quote, and context to find a stored span again in a new edition. |
| [Saving](#saving-and-loading) | Documents, single layers, and corpora, in a versioned format. |

Start with the [quick start](#quick-start), then the [concepts](#concepts); every later section builds on them.
The document ends with [errors](#errors), [performance](#performance), a [comparison with STAM](#comparison-with-stam),
and the [open questions](#open-questions).

## Quick start

```rust
use klause::{Document, Span, spans};

let mut doc = Document::new("greeting");

let hello = doc.push_span("words", "Hello", |s| s)?;   // pushes the text and adds a span over it
doc.push(", ")?;
let world = doc.push_span("words", "world", |s| s)?;
doc.push("!")?;

doc.add_span("pos", Span::from(hello).with_attr("tag", "interjection"))?;
doc.add_span("pos", Span::from(world).with_attr("tag", "noun"))?;

let nouns = doc.find().covered_by(spans("pos").attr("tag", "noun")).get_selection()?;
nouns.ranges();                         // [("greeting", 7..12)]

let p = doc.get_passage(..)?;           // the whole document
p.text();                               // "Hello, world!"
for span in p.spans() {
    span.layer();                       // "words", "words", "pos", "pos"
    p.text_of(&span);                   // ["Hello"], ["world"], ["Hello"], ["world"]
}
```

## Concepts

- **`Document`**: an ID, attributes, a text, and the layers of spans on that text. Most work happens here.
- **`Span`**: a target, attributes, and an ID, in one layer. The target is some text, another span, or a link from
  one place to another.
- **`Layer`**: a named set of spans. *Stored* layers live in the document and can be edited. *Source* layers are
  read live from your own storage.
- **`Value`**: an attribute value: null, bool, integer, float, string, or a list of those.
- **`Selection`**: a sorted set of text ranges. Filtering is intersection.
- **`Passage`**: a range of text with every span that touches it.
- **`Query`**: a fluent builder. Filters choose text; the final verb shapes the result.
- **`Corpus`**: several documents, for queries across them, links between them, and layers they share. You only
  need it when one document isn't enough.

### Conventions

- **Positions are byte offsets** into the document's UTF-8 text, as `u32`, end-exclusive, and always on character
  boundaries. A Rust `&str` slices with them directly, and most text renderers take byte ranges.
- **Operations start with a verb:** `push`, `add_span`, `get_passage`, `find`, `count_in`.
- **Fluent steps say what they add or limit:** `with_attr`, `within`, `covered_by`, `expand_to`. They never fail;
  errors come from the final verb.
- **Accessors are nouns:** `p.text()`, `span.layer()`, `doc.len()`.

## Documents

### Pushing text

A document's text only grows. `push` appends to the end and returns the byte range of what it added, so existing
positions never move.

```rust
use klause::Document;

let mut doc = Document::new("KAI 181")
    .with_attr("language", "Moabite")
    .with_attr("site", "Dhiban")
    .with_attr("date", -840);           // years as integers, negative for BCE

let a = doc.push("ʾnk")?;               // 0..4: positions are bytes, and ʾ takes two
doc.push(" . ")?;
let b = doc.push("mšʿ")?;               // 7..12

doc.len();                              // 12
doc.text();                             // "ʾnk . mšʿ"
doc.slice(b.clone());                   // "mšʿ"
doc.attr("language");                   // Some(&Value::Str("Moabite"))
```

### Pushing text with a span

`push_span` pushes text and adds a span over it in one step. The closure receives the new span, so you can give it an
ID and attributes. It returns the range, like `push`.

```rust
let r = doc.push_span("words", "bn", |s| s.with_id("1.3").with_attr("restored", false))?;
doc.push_span("words", "kmšyt", |s| s)?;   // no ID or attributes
```

Spans that cover several pushes, such as lines or verses, are added afterwards from the positions you recorded:

```rust
let start = doc.len();
// … push the words of the line …
doc.add_span("lines", Span::from(start..doc.len()).with_id("1"))?;
```

### Normalisation

Each push is normalised to Unicode NFC, and the returned range points into the normalised text. A push may not start
with a combining mark (such as a vowel point or accent), because it would attach to the last character of the previous
push: `Error::SplitsCharacter`.

## Spans

### Targets

```rust
use klause::{Span, SpanRef};

Span::from(0..3)                              // some text
Span::from([61..70, 75..83])                  // several ranges, read as one unit: "your seed" … "her seed"
Span::from(48..48)                            // a point: a position between two characters
Span::on(SpanRef::new("words", "1.3"))        // another span: a span about a span
Span::link(0..4, 7..12)                       // from -> to, within the document
Span::link(SpanRef::new("words", "1.1"), SpanRef::new("words", "1.2"))
```

- `Span::from` takes a range, an array or `Vec` of ranges, or anything `push` returned. Ranges are kept sorted, and
  ranges that touch are merged.
- A point marks a position: a lacuna of unknown length, an insertion, a note marker between words.
- A span on a span covers the same text as its target, so text queries find it. A span can only target a span that
  already exists, so chains of spans never form a cycle. Removing a span removes every span on it.
- A link has a direction and covers the text of both ends. Each end is text or a span. Links between documents
  live in a [corpus](#corpora).

```rust
use klause::Target;

match span.target() {
    Target::Text(loc)         => { loc.ranges(); loc.bounds(); }   // [61..70, 75..83], 61..83
    Target::Span(r)           => { doc.span(r)?; }
    Target::Link { from, to } => { doc.get_passage(to)?; }         // either end works as a selection
}

doc.resolve(&span);           // the text a span covers, following spans on spans
```

### Attributes

Attributes are an ordered list of keys, each with one or more values. Adding a key again adds another value; you
never need to decide in advance whether a key holds a list.

```rust
use klause::Value;

let s = Span::from(15..17)
    .with_attr("pos", "noun")
    .with_attr("lemma", "bn");
s.attr("lemma");                  // Some(&Value::Str("bn"))

let s = s.with_attr("lemma", "bnh");                   // a second candidate lemma
s.attr("lemma");                  // Some(&Value::List([Str("bn"), Str("bnh")]))
s.values("lemma").count();        // 2: one item per value, whether there is one or many

let s = s.set_attr("lemma", "bn");                     // replaces every value of the key
let s = s.without_attr("lemma");
s.attrs();                        // (key, value) pairs in the order they were first added
```

- Values convert from `&str`, `String`, integers, floats, and `bool`.
- Passing an array or `Vec` adds each element as a value, the same as calling `with_attr` once per element. Lists
  don't nest.
- Filters on a key match if any of its values match.

### IDs

Every span has an ID, unique within its layer and document. An ID is a string or a `u64`. `add_span` assigns a ULID
to a span that has none. A `SpanRef` names a span by layer and ID.

```rust
let r = doc.add_span("morph", Span::on(SpanRef::new("words", "1.3")).with_attr("pos", "noun"))?;
r.layer();                        // "morph"
r.id();                           // SpanId::Str("01JD3Q8Z5K7M2N4P6R8T0V2X4Y")

doc.span(&r)?;                    // Span, or Err(UnknownSpan)
SpanRef::from(&span);             // any span converts to its reference
```

### Adding, replacing, removing

```rust
let word = SpanRef::new("words", "1.3");

let r = doc.add_span("morph", Span::on(&word).with_attr("pos", "noun"))?;

doc.replace_span(&r, Span::on(&word).with_attr("pos", "noun").with_attr("state", "cstr"))?;   // keeps the ID
doc.remove_span(&r)?;             // also removes every span on it
```

Adding a span to a layer that doesn't exist creates a stored layer with that ID. `add_span` and `replace_span` fail
on a source layer (`ReadOnlyLayer`), a target span that doesn't exist, a range outside the text or off a character
boundary, or a duplicate ID.

## Layers

```rust
use klause::Layer;

doc.add_layer(
    Layer::new("morph")
        .with_name("Morphology")
        .with_attr("creator", "A. Scholar")     // layer metadata
        .with_attr("license", "CC-BY-4.0"),
)?;

doc.add_layer(Layer::from_source(highlights))?; // read live, see "Source layers"

doc.layer("morph")?.name();                     // "Morphology"
doc.layer("morph")?.attr("license");            // Some(&Value::Str("CC-BY-4.0"))
doc.layers();                                   // in the order they were added
```

Layer IDs can't contain dots, because filters write `"layer.key"`.

## Reading passages

`get_passage` takes a range, a span, or the whole document and returns the text with everything that touches it.

```rust
let p  = doc.get_passage(0..47)?;                           // a range
let p  = doc.get_passage(SpanRef::new("verses", "3:15"))?;  // the text a span covers
let p  = doc.get_passage(..)?;                              // the whole document
let ps = doc.get_passages(&selection)?;                     // Vec<Passage>, in text order

doc.get_passage([0..3, 42..47]);                            // Err(NotOne { found: 2 }): two separate ranges
```

A passage is owned, cheap to clone, and `Send`, so it can be cached, kept in UI state, or sent to another thread.
Its text is shared with the document, not copied.

```rust
p.doc();                    // "GEN"
p.range();                  // 0..142, in document positions
p.text();                   // "And I will put enmity between you and the woman, … his heel.”"

for span in p.spans() {     // every layer, in the order the layers were added
    span.layer();           // "structure"
    p.local(&span);         // [0..47]: its ranges inside the passage, relative to p.text(), clipped
    p.text_of(&span);       // ["And I will put enmity between you and the woman"]
    span.begins_before(&p); // false
    span.ends_after(&p);    // false
}

p.spans_in("words");                            // one layer
p.layer("highlights").unwrap().error();         // Some(SourceError) if that source failed; the text still loads
p.snap(5..12, "words");                         // Some(6..14): a selection in p.text(), widened to whole
                                                // words, in document positions
```

A passage includes:

- every span with a range that overlaps it, and every point inside it or at its edges;
- every link with either end inside it;
- every span on a span already included, at any depth.

## Selections

```rust
let a = doc.get_selection(0..47)?;
let b = doc.get_selection([42..47, 61..70])?;

a.union(&b);  a.intersect(&b);  a.subtract(&b);   // each returns a new Selection
b.ranges();                                       // [("GEN", 42..47), ("GEN", 61..70)]
b.contains("GEN", 44);                            // true
b.is_empty();
```

A `Selection` is plain data with no borrow of the document. It can be stored, compared, and passed back in. Anything
that implements `IntoSelection` can be: ranges, `..`, arrays of ranges, `SpanRef`, `&Span`, link ends, and
`Selection` itself. Corpora add document IDs and [document filters](#corpora).

## Queries

`doc.find()` starts with all of the document's text. Each step narrows, widens, or maps the current hits; the final
verb decides what comes back.

```rust
use klause::{spans, Distance};

// Lines holding a construct noun directly followed by a proper name.
doc.find()
    .covered_by(spans("morph").attr("pos", "noun").attr("state", "cstr"))
    .followed_by(spans("morph").attr("pos", "pn"), Distance::adjacent())
    .expand_to("lines")
    .get_passages()?;
```

### Filters

A filter describes spans. Every condition in one filter must hold on the same span.

```rust
use klause::{spans, any_spans, any_of};

spans("morph").attr("pos", "verb")                       // has this value
spans("morph").attr("pos", any_of(["noun", "pn"]))       // has one of these values
spans("morph").not_attr("stem", "qal")                   // doesn't have this value
spans("morph").has("lemma")                              // has the key, any value
spans("morph").lacks("gloss")
spans("structure").attr("indent", 2..)                   // a number in this range
spans("verses").id("3:15")
spans("review").on(spans("morph").attr("pos", "verb"))   // spans whose target span matches a filter
any_spans().attr("is_parable", true)                     // a span in any layer
```

A layer ID converts to a filter, so `expand_to("lines")` means `expand_to(spans("lines"))`.

### Steps

| Step                    | Effect on the hits                                                                   |
| :---------------------- | :----------------------------------------------------------------------------------- |
| `.within(s)`            | Keeps the text inside `s`, any `IntoSelection`.                                      |
| `.covered_by(f)`        | Keeps the text covered by a span matching `f`.                                       |
| `.not_covered_by(f)`    | Removes the text covered by a span matching `f`.                                     |
| `.in_layer(id)`         | `covered_by(spans(id))`.                                                             |
| `.with_attr(k, v)`      | `covered_by(any_spans().attr(k, v))`.                                                |
| `.with_text(t)`         | Keeps exact matches of `t`.                                                          |
| `.with_text_folded(t)`  | Keeps matches of `t`, ignoring case and accents.                                     |
| `.containing(f)`        | Keeps whole hits that contain text covered by a span matching `f`.                   |
| `.followed_by(f, d)`    | Keeps hits with a match of `f` after them, within distance `d`.                      |
| `.preceded_by(f, d)`    | Keeps hits with a match of `f` before them, within distance `d`.                     |
| `.near(f, d)`           | Either of the two.                                                                   |
| `.expand_to(f)`         | Widens each hit to the spans matching `f` that cover it.                             |
| `.follow(layer)`        | Replaces the hits with the `to` ends of links in `layer` whose `from` end touches them. |
| `.follow_back(layer)`   | Replaces the hits with the `from` ends of links whose `to` end touches them.         |

`Distance` is one of:

- `Distance::adjacent()`: nothing but spaces and punctuation in between;
- `Distance::spans("words", 0..=2)`: at most two spans of a layer in between;
- `Distance::bytes(0..=40)`.

**Order.** Filter steps combine with AND, in any order; the query runs the cheapest first. `expand_to`, `follow`, and
`follow_back` change what the hits are, so they apply exactly where they appear: filters before them see the old
hits, and filters after them see the new ones.

**OR.** `any_of` covers one key with several values. For anything else, run two queries and `union` the selections.

| Option step           | Effect                                                                         |
| :-------------------- | :----------------------------------------------------------------------------- |
| `.only_layers(ids)`   | Return only these layers' spans in passages, and limit `any_spans()` to them.  |
| `.exclude_layer(id)`  | Leave one layer out of passages and `any_spans()`.                             |

### Final verbs

| Verb                         | Returns               | Notes                                                                 |
| :--------------------------- | :-------------------- | :-------------------------------------------------------------------- |
| `.get_selection()`           | `Selection`           | The hits. Cheap.                                                      |
| `.get_passages()`            | `Vec<Passage>`        | One passage per hit; hits that touch are merged.                      |
| `.get_passage()`             | `Passage`             | Errors unless there is exactly one.                                   |
| `.get_spans(layer)`          | `Vec<Span>`           | The spans in `layer` that touch a hit.                                |
| `.count()`                   | `usize`               | The number of hit ranges.                                             |
| `.count_in(f)`               | `usize`               | The number of spans matching `f` that contain a hit.                  |
| `.count_values("layer.key")` | `Vec<(Value, usize)>` | Values of `key` on spans in `layer` that touch a hit. Most frequent first, ties in value order. |

Errors from any step surface here: an unknown layer, or a source layer that a filter needs and that failed. A query
can't give correct results without that source, so unlike a passage, it fails.

The shortcuts are these chains:

```rust
doc.get_selection(s)    // doc.find().within(s).get_selection()
doc.get_passages(s)     // doc.find().within(s).get_passages()
doc.get_passage(s)      // doc.find().within(s).get_passage()
```

## Source layers

A source layer reads its spans from your own storage, such as a database, every time they're needed, without caching.
A change you save there shows up in the next passage or query.

```rust
use std::ops::Range;
use klause::{Span, SpanFilter, SpanSource, SourceError};

struct Highlights { db: Db }

impl SpanSource for Highlights {
    fn id(&self) -> &str { "highlights" }
    fn name(&self) -> &str { "Highlights" }

    // Every span with text in `range` of document `doc`, including links with an end there.
    fn get_spans(&self, doc: &str, range: Range<u32>) -> Result<Vec<Span>, SourceError> {
        let rows = self.db.highlights(doc, range.start, range.end).map_err(SourceError::new)?;
        Ok(rows.into_iter().map(|row| {
            Span::from(row.start..row.end).with_id(row.id).with_attr("color", row.color)
        }).collect())
    }

    // Optional. Answers a query filter directly, so the query doesn't have to call `get_spans`
    // over everything it searches.
    fn find_spans(&self, doc: &str, filter: &SpanFilter) -> Option<Result<Vec<Span>, SourceError>> {
        None
    }
}
```

Klause never writes to a source; your code does. If a source fails while building a passage, the passage still
loads, and that layer's entry reports the error.

## Corpora

A corpus holds several documents. Use one when you need:

- queries across documents;
- links between documents;
- a layer shared by every document, such as one highlights source for a whole library.

```rust
use klause::{Corpus, Layer, Span, SpanRef, docs, spans};

let mut lib = Corpus::new();
lib.add_document(gen)?;
lib.add_document(isa)?;
lib.add_document(rom)?;

lib.add_layer(Layer::new("echoes"))?;                   // a corpus layer
lib.add_layer(Layer::from_source(highlights))?;         // asked about every document

lib.document("ROM")?.len();
lib.document_mut("ROM")?.add_span("notes", Span::from(0..9))?;   // documents keep their own layers
```

Spans in a corpus layer must say which document they're in. Ranges take the form `(doc, range)`, and span
references take `.in_doc(doc)`:

```rust
lib.add_span("echoes", Span::link(
    SpanRef::new("verses", "10:15").in_doc("ROM"),
    SpanRef::new("verses", "52:7").in_doc("ISA"),
).with_attr("kind", "quotation"))?;

lib.add_span("echoes", Span::link(("ROM", 2_210..2_301), ("ISA", 41_880..41_940)))?;
```

Passages and queries work the same as on a document. They include each document's own layers and the corpus's
layers.

```rust
lib.get_passage("KAI 181")?;                              // a document ID: the whole document
lib.get_passage(("GEN", 0..47))?;
lib.get_passage(SpanRef::new("verses", "3:15").in_doc("GEN"))?;
lib.get_selection(["ROM", "1CO", "2CO", "GAL"])?;          // several whole documents

lib.find()
    .within(docs().attr("language", "Moabite").attr("date", ..-800))   // a document filter
    .covered_by(spans("morph").attr("pos", "verb"))
    .count_values("morph.lemma")?;
```

## Keeping spans valid

A document's existing text never changes, so stored positions stay correct for as long as you keep the document. A
new edition of a text is a new document, and spans saved against the old one may no longer fit. Save an anchor
alongside any span you store outside the document:

```rust
let anchor = doc.anchor(&target)?;        // Anchor { checksum, quote, prefix, suffix }
db.save(&span, &anchor);

new_edition.verify(&target, &anchor);     // Ok(()) or Err(Stale)
```

`quote` is the covered text, and `prefix` and `suffix` are a few words on either side. Together they're enough to
find the text again in a new edition by searching.

## Saving and loading

```rust
doc.save("kai-181.klause")?;                        // text, attributes, stored layers, and indexes
let doc = Document::open("kai-181.klause")?;        // indexes are loaded, not rebuilt

doc.save_layer("morph", "kai-181.morph.klause")?;   // one layer on its own, to share or version separately
doc.add_layer(Layer::open("kai-181.morph.klause")?)?;

lib.save("library/")?;                              // every document, plus the corpus's layers
let lib = Corpus::open("library/")?;
```

Source layers are never saved; they belong to you. The file format is versioned.

## Errors

```rust
enum Error {
    UnknownDocument(String),
    UnknownLayer(String),
    UnknownSpan(SpanRef),
    DuplicateId { layer: String, id: SpanId },
    OutOfRange(Range<u32>),
    NotCharBoundary(u32),
    SplitsCharacter,                          // a push that starts with a combining mark
    DocumentRequired,                         // a corpus-layer span that doesn't name its document
    ReadOnlyLayer(String),                    // editing a source layer
    NotOne { found: usize },                  // `get_passage` on zero or several passages
    Source { layer: String, error: SourceError },   // a query needed a source that failed
    Format(FormatError),                      // reading or writing a saved file
}
```

## Performance

- Stored layers are indexed by position, ID, and attribute value.
- Passages and selections share the document's text rather than copying it.
- A source layer is called on every passage that might include it. A query that filters on a source calls
  `get_spans` over everything it searches, unless the source implements `find_spans`.
- A corpus query looks at every document unless `within` narrows it.
- Typical costs to measure: loading one passage, loading many, counting values (such as lemmas) within a passage, a
  list of passages, and a whole corpus, and filtering on attributes (such as morphology).

## Comparison with STAM

[STAM](https://github.com/annotation/stam) is a data model for stand-off annotation with a Rust implementation.
Klause borrows several of its ideas: spans with several ranges, points, spans on spans, directional links,
values stored once, comparison filters, position relations, NFC text, and checksums for detecting changed text. It
differs in these ways:

| `klause`                                             | STAM                                                     |
| :--------------------------------------------------- | :------------------------------------------------------- |
| Byte offsets                                         | Unicode code point offsets                               |
| A span belongs to one layer, which is also where it's stored | Data sets are vocabularies; an annotation can use several |
| Source layers read live from your storage            | Everything in memory                                     |
| `Passage`: text with every span, ready to render     | Assembled from iterators                                 |
| Results shaped by the final verb, including counts   | Results are annotations or text selections               |
| Spans edited in place, keeping their IDs             | Annotations are immutable; delete and recreate           |
| Attributes on the span                               | Separate data objects with their own IDs                 |
| A fluent query builder that orders filters itself    | Iterators, or a query language run in the order written  |
| MIT                                                  | GPL-3.0                                                  |

## Open questions

Draft only; this section goes away once the API is settled.

1. **Concurrent edits.** Editing takes `&mut Document`, so reading on one thread while editing on another needs a
   lock. Is that enough, or should an edit produce a cheap new snapshot while readers keep the old one?
2. **Rich text values.** A footnote body may need italics. Should a body be a small document of its own (with spans
   for styling, searchable like any text) referenced by a new `Value::Doc`, or stay a plain string?
3. **What `followed_by` returns.** It keeps only the first part (`bn`). Should there be a way to get both parts
   (`bn . kmšyt`) as one hit?
4. **Letters and their marks.** Spans must start and end on character boundaries. Should they also be kept from
   splitting a letter from its vowel points or accents?
5. **Replacing a target.** When a span is replaced, spans on it keep pointing at it. Should they be flagged when the
   text it covers changes?
6. **Spans on spans in passages.** A passage includes them at any depth. Should there be a depth limit?
7. **Creating layers implicitly.** `add_span` creates a missing stored layer, which is convenient but hides typos
   until a query names the right layer. Should it require `add_layer` first?
8. **What `push_span` returns.** It returns the range. Should it return the new span's `SpanRef` too?
