# Klause Design

Klause stores a text together with layers of annotations on it, and answers two kinds of request quickly: "give me
this stretch of text with everything marked on it, ready to render" and "find, filter, or count across the whole
text". It is a general library. The workload it is tuned for is a Bible: the full text, with words, verses,
chapters, books, morphology on every word, poetry and paragraph structure, footnotes, cross-references, and a user's
highlights kept in an application database.

Klause keeps documents in memory. Reading them from and writing them to files or databases belongs to the
application, through the `IntoDocument` trait and the document's read accessors.

## Concepts

- **`Document`**: an ID, attributes, a UTF-8 text that only grows at the end, and named layers.
- **`Span`**: one annotation. It marks some of the document's text, has attributes, and has an ID unique in its
  layer.
- **`Layer`**: a named group of spans. A *stored* layer keeps its spans in the document. A *source* layer asks your
  own code for its spans every time they are needed.
- **`Value`**: the type of data an attribute holds: null, bool, integer, float, or string.
- **`SpanRef`**: the address of one span: its layer ID and its span ID. It holds no span data.
- **`Selection`**: a sorted list of non-overlapping byte ranges of the text.
- **`Passage`**: one range of the text, together with every span that touches it.
- **`SpanFilter`**: a description of spans, such as "spans in `morph` whose `pos` is `verb`".
- **`Query`**: a builder that starts from the whole text, narrows or widens it step by step, and ends with a verb
  that returns a selection, passages, spans, or counts.
- **`IntoDocument`**: a trait your code implements to build a `Document` from your own storage.

## Terms

These words have exact meanings everywhere in this document and in the API docs.

- **Position**: a byte offset into the document's text, as `u32`. Always on a UTF-8 character boundary.
- **Range**: `start..end` of positions, end-exclusive. A Rust `&str` slices with it directly.
- **Point**: a range with `start == end`. It marks the gap between two characters, such as where a footnote marker
  goes.
- **A span's ranges**: the text positions a span marks. A span that targets text has the ranges it was given. A span
  that targets another span has that span's ranges, followed through any chain of such spans.
- **Overlap**: two non-empty ranges overlap when each starts before the other ends.
- **Touch**: a span touches a range when one of its non-empty ranges overlaps the range, or one of its points lies
  at or between the range's start and end.
- **Hits**: the selection a query holds after its latest step.

## Conventions

The API is fluent: values are built by chaining methods, and chains read as sentences.

- **Builders** never fail. `with_*` adds, `set_*` replaces, and `remove_*` removes:
  `Span::from(0..3).with_id(1).with_attr("pos", "noun")`. `Document`, `Layer`, `Span`, `SpanFilter`, and `Query`
  are all built this way.
- **Span selectors name their result**: `spans_in("morph")` and `all_spans()` create span filters;
  `span_id("verses", "GEN 3:15")` creates a reference to one span. They describe text for a query step or
  passage to read, and are resolved when it runs.
- **Query steps say how they change the hits**: `within`, `outside`, `in_layer`, `in_span`, `with_text`,
  `followed_by`, `expand_to`. Their errors surface from the final verb.
- **Methods that can fail with an `Error` start with a verb**: `push`, `add_span`, `get_passage`, `get_selection`.
- **Accessors are nouns** and return a value or an `Option`: `doc.text()`, `doc.layer(id)`, `span.attr(key)`.

## Documents

### Building

A document is built fluently with its attributes and layers, then text is pushed onto it.

```rust
use klause::{Document, Layer};

let mut doc = Document::new("BSB")
    .with_attr("title", "Berean Standard Bible")
    .with_attr("language", "en")
    .with_layer("words")                                   // a stored layer; a layer ID converts to `Layer`
    .with_layer(Layer::new("morph").with_name("Morphology"));
```

`with_layer` panics if the ID is taken, since builders don't return errors; `add_layer` adds a layer to an existing
document and fails with `DuplicateLayer` instead.

### Pushing text

A document's text only grows. `push` appends to the end and returns the range it added, so existing positions never
change.

```rust
let a = doc.push("In")?;                  // 0..2
doc.push(" ")?;
let b = doc.push("the")?;                 // 3..6

doc.len();                                // 6
doc.text();                               // "In the"
doc.slice(b.clone());                     // "the"
doc.attr("language");                     // Some(&Value::Str("en"))
```

### Normalisation

Each push is normalised to Unicode NFC, and the returned range points into the normalised text. A push is rejected
with `Error::SplitsCharacter` when normalising the last character already in the text together with the new text
would differ from that character followed by the normalised push, as happens when a push starts with a combining
mark such as a Hebrew vowel point. This keeps every earlier position valid.

### Pushing text with a span

`push_span` pushes text and adds a span over it. The closure receives the new span's builder to set its ID and
attributes. It returns the range, like `push`.

```rust
let r = doc.push_span("words", "beginning", |s| s.with_id(1_001_001_003_u64))?;
doc.push_span("words", "God", |s| s)?;          // an ID is assigned
```

The layer must already exist. Spans that cover several pushes, such as verses, are added afterwards from recorded
positions:

```rust
let start = doc.len();
// … push the words of the verse …
doc.add_span("verses", Span::from(start..doc.len()).with_id("GEN 1:1"))?;
```

### Loading from your storage

Implement `IntoDocument` for whatever holds your data: files, a database, a download. Build the document with the
same calls as above.

```rust
use klause::{Document, IntoDocument, Span};

struct BsbRows { verses: Vec<VerseRow> }

impl IntoDocument for BsbRows {
    type Error = MyError;                     // must implement `From<klause::Error>`

    fn into_document(self) -> Result<Document, MyError> {
        let mut doc = Document::new("BSB").with_layer("words").with_layer("verses");
        for verse in self.verses {
            let start = doc.len();
            for word in verse.words {
                doc.push_span("words", &word.text, |s| s.with_id(word.id))?;
                doc.push(&word.after)?;
            }
            doc.add_span("verses", Span::from(start..doc.len()).with_id(verse.id))?;
        }
        Ok(doc)
    }
}

let bible = BsbRows::read("bsb/")?.into_document()?;
```

`Document` implements `IntoDocument` by returning itself, so functions in your code can take `impl IntoDocument` and
accept either.

To write a document out, read it back through its accessors: `doc.id()`, `doc.attrs()`, `doc.text()`, and for each
layer in `doc.layers()`, `layer.id()`, `layer.name()`, `layer.attrs()`, `layer.is_source()`, and `layer.spans()`
(stored spans, by start). Source layers have no spans to write.

## Spans

### Targets

A span targets either text or another span.

```rust
use klause::Span;

Span::from(0..3)                              // text
Span::from([61..70, 75..83])                  // several ranges, read as one unit
Span::from(48..48)                            // a point
Span::on("words", 1_003_015_003_u64)          // another span, by layer and ID
Span::on_ref(&span_ref)                       // another span, by SpanRef
```

- `Span::from` takes a range, an array or `Vec` of ranges, or a range returned by `push`. Ranges are sorted, and
  ranges that overlap or meet end to start are merged into one.
- A span that targets another span follows it: its ranges are always that span's current ranges. Morphology tags
  target word spans this way.
- A span can only target a span that already exists, so chains of targets never form a cycle.

```rust
use klause::Target;

match span.target() {
    Target::Text(ranges) => { /* &[Range<u32>] */ }
    Target::Span(r)      => { doc.span(r); }
}

doc.ranges(&span_ref);        // Option<&[Range<u32>]>: the span's ranges, following targets
```

### Attributes

Each attribute key holds one or more values, in the order they were added. Adding a key again adds another value.

```rust
let s = Span::from(15..17)
    .with_attr("pos", "noun")
    .with_attr("strongs", "H7225");
s.attr("pos");                    // Some(&Value::Str("noun")): the first value
s.values("strongs").count();      // 1

let s = s.with_attr("strongs", "H7218");   // a second value for the same key
s.values("strongs").count();      // 2

let s = s.set_attr("strongs", "H7225");    // replaces every value of the key
let s = s.remove_attr("strongs");
s.attrs();                        // (key, value) pairs, keys in the order first added
```

- `Value` converts from `&str`, `String`, integers (stored as `i64`), floats (stored as `f64`), and `bool`.
- Passing an array or `Vec` to `with_attr` adds each element as a separate value.
- A filter on a key matches when any of the key's values matches.
- Integers and floats compare as numbers, so `2` equals `2.0`.
- Values sort as: null, then `false`, `true`, then numbers in numeric order, then strings in byte order.

`Document` and `Layer` have the same attribute builders and accessors.

### IDs

A span ID is a string or a `u64`, unique within its layer. `add_span` gives a span without an ID one more than the
largest integer ID in its layer, or 1 when the layer has none.

```rust
let r = doc.add_span("morph", Span::on("words", 1_001_001_003_u64).with_attr("pos", "noun"))?;
r.layer();                        // "morph"
r.id();                           // &SpanId::Int(1)

doc.span(&r);                     // Option<&Span>
```

`add_span` returns the new span's `SpanRef`. `SpanRef::new(layer, id)` builds one directly.

### Adding, replacing, removing

```rust
let r = doc.add_span("morph", Span::on("words", 1_001_001_003_u64).with_attr("pos", "noun"))?;
let tag = doc.span(&r).unwrap().clone().with_attr("state", "cstr");
doc.replace_span(&r, tag)?;       // keeps the ID
doc.remove_span(&r)?;
```

- `add_span` fails when the layer doesn't exist (`UnknownLayer`), the layer is a source layer (`ReadOnlyLayer`), the
  target span doesn't exist (`UnknownSpan`), a range is outside the text (`OutOfRange`) or off a character boundary
  (`NotCharBoundary`), or the ID is taken (`DuplicateId`).
- `replace_span` fails for the same reasons, and with `Cycle` when the new target is the span itself or a span that
  targets it, directly or through a chain. Spans that target the replaced span now follow its new ranges.
- `remove_span` also removes every span that targets the removed span, directly or through a chain, in every layer.

## Layers

```rust
use klause::Layer;

doc.add_layer(
    Layer::new("morph")
        .with_name("Morphology")
        .with_attr("source", "OpenScriptures")
        .with_attr("license", "CC-BY-4.0"),
)?;

doc.add_layer(Layer::source("highlights", Highlights { db }))?;   // see "Source layers"

doc.layer("morph").unwrap().name();       // "Morphology"
doc.layers();                             // in the order they were added
```

Layer IDs are any non-empty string, and `&str` converts to `Layer::new(id)`.

## Selectors and selections

### Selectors

Query steps and passages read text described by an `impl IntoSelection`. Selectors build one fluently:

```rust
use klause::{spans_in, all_spans, span_id, any_of};

span_id("verses", "GEN 3:15")                               // one span's ranges
spans_in("verses")                                          // every span in a layer
spans_in("morph").with_attr("pos", "verb")                  // has this value
spans_in("morph").with_attr("pos", any_of(["noun", "pn"]))  // has one of these values
spans_in("morph").with_attr("number", 2..)                  // has a number in this range
spans_in("morph").without_attr("stem", "qal")               // has no value equal to this
spans_in("morph").with_key("lemma")                         // has the key
spans_in("morph").without_key("gloss")                      // doesn't have the key
spans_in("verses").with_id("GEN 3:15")
spans_in("review").targeting(spans_in("morph").with_attr("pos", "verb"))   // its target matches
all_spans().with_attr("speaker", "Jesus")                // spans in any layer
```

- `span_id` returns a `SpanRef`. `spans_in` and `all_spans` return a `SpanFilter`; every condition in
  one filter must hold on the same span, and the selection is the union of the matching spans' ranges.
- Ranges (`0..47`, `..`, arrays and `Vec`s of ranges) and `Selection` are accepted as they are.
- Steps and verbs that take a filter, such as `expand_to` and `get_spans`, also accept a layer ID: `"verses"` means
  `spans_in("verses")`.

`IntoSelection` converts its input against a document:

```rust
pub trait IntoSelection {
    fn into_selection(self, doc: &Document) -> Result<Selection, Error>;
}
```

### Selections

```rust
let a = doc.get_selection(0..47)?;
let b = doc.get_selection([42..47, 61..70])?;

a.union(&b);  a.intersect(&b);  a.subtract(&b);   // each returns a new Selection
b.ranges();                                       // &[42..47, 61..70]
b.contains(44);                                   // true
b.is_empty();
```

A `Selection` holds sorted ranges that neither overlap nor meet end to start; building one merges any that do.
Points are kept as empty ranges unless they lie at or between the start and end of a non-empty range. A selection
holds no borrow of the document, so it can be stored, compared, and passed back in.

Intersecting keeps the overlapping parts of non-empty ranges, and keeps a point of either side when it lies at or
between the start and end of a range on the other side.

## Passages

```rust
let p  = doc.get_passage(0..47)?;                              // a range
let p  = doc.get_passage(span_id("verses", "GEN 3:15"))?;  // a span's text
let p  = doc.get_passage(..)?;                                 // the whole document
let ps = doc.get_passages([0..3, 42..47])?;                    // Vec<Passage>, one per range

doc.get_passage([0..3, 42..47]);                               // Err(NotOne { found: 2 })
```

`get_passage` takes the selection of its input and fails with `NotOne` unless it holds exactly one range.
`get_passages` returns one passage per range of the selection, in text order. A selection merges ranges that meet,
so consecutive verses become one passage; for one passage per verse, call `get_passage` on each verse.

A passage contains every span, in the layers it includes, that touches its range. A span that targets another span
has that span's ranges, so it is included whenever its target is.

A passage is owned, cheap to clone, and `Send + Sync`. It shares the document's text instead of copying it.

```rust
p.range();                  // 0..142, in document positions
p.text();                   // the text of that range

for s in p.spans() {        // every included layer, in the order the layers were added, then by start
    s.layer();              // "structure"
    s.local();              // [0..47]: its ranges relative to p.text(), clipped to the passage
    s.text();               // ["And I will put enmity between you and the woman"]
    s.starts_inside();      // true: its first range starts within the passage
    s.ends_inside();        // true: its last range ends within the passage
    s.attr("indent");       // `PassageSpan` derefs to `Span`
    s.to_ref();             // SpanRef
}

p.spans_in("words");        // one layer
p.error("highlights");      // Some(&SourceError) if that source failed; everything else still loads
```

Adding `p.range().start` to a local position gives the document position.

## Queries

`doc.find()` starts with the whole text as its hits. Each step changes the hits; the final verb decides what comes
back.

```rust
use klause::{spans_in, Distance};

// Verses with a construct noun directly followed by a proper noun.
doc.find()
    .within(spans_in("morph").with_attr("pos", "noun").with_attr("state", "cstr"))
    .followed_by(spans_in("morph").with_attr("pos", "pn"), Distance::adjacent())
    .get_spans("verses")?;
```

### Steps

| Step                      | Hits afterwards                                                                          |
| :------------------------ | :--------------------------------------------------------------------------------------- |
| `.within(x)`              | The parts of the hits inside `x`'s selection.                                            |
| `.outside(x)`             | The parts of the hits not inside `x`'s selection.                                        |
| `.in_layer(id)`           | `.within(spans_in(id))`: the parts of the hits inside any span of the layer.         |
| `.in_span(layer, id)`     | `.within(span_id(layer, id))`: the parts of the hits inside that span.               |
| `.with_text(t)`           | Every occurrence of the string `t` that lies wholly inside one hit range.                |
| `.with_text_folded(t)`    | The same, comparing case-insensitively and ignoring combining marks.                     |
| `.followed_by(f, d)`      | The hit ranges that have a range of a span matching `f` starting after them within `d`.  |
| `.preceded_by(f, d)`      | The hit ranges that have a range of a span matching `f` ending before them within `d`.   |
| `.expand_to(f)`           | The union of the ranges of every span matching `f` that touches a hit range.             |

`x` is any `IntoSelection`; `f` is a `SpanFilter` or a layer ID.

`with_text_folded` compares the NFD form of both strings with combining marks (general category `Mn`) removed and
full Unicode lowercasing applied. "elohim" matches "ʾĔlōhîm" except for the leading ʾ (a letter, not a mark), and
"אלהים" matches "אֱלֹהִים" regardless of vowel points and cantillation. Matches are reported as ranges of the original
text.

`Distance` is one of:

- `Distance::adjacent()`: only whitespace (`White_Space`) and punctuation (general category `P*`) in between;
- `Distance::at_most(n, layer)`: at most `n` spans of `layer` lie wholly in between;
- `Distance::bytes(n)`: at most `n` bytes in between.

**Order.** Steps other than `expand_to` are intersections, so between two `expand_to` steps their order doesn't
change the result, and the query runs the cheapest first. `expand_to` replaces the hits, so steps before it see the
old hits and steps after it see the new ones.

**Passage layers.** `.with_layers(["words", "structure"])` limits which layers' spans the passages from
`get_passages` and `get_passage` contain. It doesn't affect the other steps.

### Final verbs

| Verb                          | Returns               | Result                                                                    |
| :---------------------------- | :-------------------- | :------------------------------------------------------------------------ |
| `.get_selection()`            | `Selection`           | The hits.                                                                 |
| `.get_passages()`             | `Vec<Passage>`        | One passage per hit range.                                                |
| `.get_passage()`              | `Passage`             | The passage of the only hit range; `NotOne` otherwise.                    |
| `.get_spans(f)`               | `Vec<Span>`           | Spans matching `f` that touch a hit, by start, then layer order.          |
| `.count()`                    | `usize`               | The number of hit ranges.                                                 |
| `.count_spans(f)`             | `usize`               | The number of spans matching `f` that touch a hit.                        |
| `.count_values(layer, key)`   | `Vec<(Value, usize)>` | For each value of `key`, the number of spans in `layer` touching a hit that have it. Most frequent first, ties in value order. |

Final verbs fail when a step names a layer that doesn't exist (`UnknownLayer`), or needs a source layer that failed
(`Source`). A query can't give correct results without that layer, so unlike a passage, it fails.

The document shortcuts are these chains:

```rust
doc.get_selection(x)    // doc.find().within(x).get_selection()
doc.get_passages(x)     // doc.find().within(x).get_passages()
doc.get_passage(x)      // doc.find().within(x).get_passage()
```

## Source layers

A source layer reads its spans from your own storage, such as a database, every time they are needed, without
caching. A change you save there shows up in the next passage or query. Klause never writes to a source.

```rust
use std::ops::Range;
use klause::{Span, SpanFilter, SpanSource, SourceError};

struct Highlights { db: Db }

impl SpanSource for Highlights {
    /// Every span that touches `range`.
    fn get_spans(&self, range: Range<u32>) -> Result<Vec<Span>, SourceError> {
        let rows = self.db.highlights(range.start, range.end).map_err(SourceError::new)?;
        Ok(rows.into_iter().map(|row| {
            Span::from(row.start..row.end).with_id(row.id).with_attr("color", row.color)
        }).collect())
    }

    /// Optional. Answers a filter on this layer directly, so a query doesn't call
    /// `get_spans` over the whole text.
    fn find_spans(&self, filter: &SpanFilter) -> Option<Result<Vec<Span>, SourceError>> {
        None
    }
}
```

- `SpanSource` requires `Send + Sync`.
- Source spans must have IDs. They may target stored spans; a source span whose target doesn't exist is skipped.
- When a source fails while a passage is built, the passage still loads, and `p.error(layer)` reports it.

## Errors

```rust
enum Error {
    UnknownLayer(String),
    DuplicateLayer(String),
    UnknownSpan(SpanRef),
    DuplicateId(SpanRef),
    Cycle(SpanRef),                           // `replace_span` would make a span its own target
    ReadOnlyLayer(String),                    // editing a source layer
    OutOfRange(Range<u32>),
    NotCharBoundary(u32),
    SplitsCharacter,                          // a push that would change the text before it
    TooLong,                                  // the text would pass u32::MAX bytes
    NotOne { found: usize },                  // `get_passage` on zero or several ranges
    Source { layer: String, error: SourceError },
}
```

## Example: a Bible

### Building

One `Document` holds the whole Bible. Each verse's words are spans in `words`; `verses`, `chapters`, and `books`
cover them; `structure` marks poetry lines; `morph` targets words; `notes` holds footnote points; `xrefs` holds
cross-references. In an application this code lives in an `IntoDocument` implementation.

```rust
use klause::{Document, Span};

let mut bible = Document::new("BSB")
    .with_attr("title", "Berean Standard Bible")
    .with_layer("books")
    .with_layer("chapters")
    .with_layer("verses")
    .with_layer("words")
    .with_layer("structure")
    .with_layer("morph")
    .with_layer("notes")
    .with_layer("xrefs");

// Genesis 3:15: one row per poetry line, with its indent and each word with the text after it.
let lines: [(i64, &[(&str, &str)]); 4] = [
    (1, &[("And", " "), ("I", " "), ("will", " "), ("put", " "), ("enmity", " "), ("between", " "),
          ("you", " "), ("and", " "), ("the", " "), ("woman", ", ")]),
    (2, &[("and", " "), ("between", " "), ("your", " "), ("seed", " "), ("and", " "), ("her", " "),
          ("seed", ". ")]),
    (1, &[("He", " "), ("will", " "), ("crush", " "), ("your", " "), ("head", ", ")]),
    (2, &[("and", " "), ("you", " "), ("will", " "), ("strike", " "), ("his", " "), ("heel", ".”")]),
];

let verse_start = bible.len();
let mut n: u64 = 0;
for (indent, words) in lines {
    let line_start = bible.len();
    let mut line_end = line_start;
    for &(word, after) in words {
        n += 1;
        line_end = bible.push_span("words", word, |s| s.with_id(1_003_015_000 + n))?.end;
        bible.push(after)?;
    }
    bible.add_span("structure", Span::from(line_start..line_end)
        .with_attr("type", "line")
        .with_attr("indent", indent))?;
}
bible.add_span("verses", Span::from(verse_start..bible.len()).with_id("GEN 3:15"))?;

// Morphology on a word.
bible.add_span("morph", Span::on("words", 1_003_015_005_u64)
    .with_attr("pos", "noun")
    .with_attr("strongs", "H342"))?;

// A footnote marker after the last word.
bible.add_span("notes", Span::from(bible.len()..bible.len())
    .with_attr("body", "Or He will bruise your head, and you will bruise his heel"))?;

// A cross-reference from this verse to another, named by verse ID.
bible.add_span("xrefs", Span::on("verses", "GEN 3:15").with_attr("to", "ROM 16:20"))?;
```

### Reading a verse for display

```rust
use klause::span_id;

let p = bible.get_passage(span_id("verses", "GEN 3:15"))?;

for line in p.spans_in("structure") {
    line.text();                      // ["And I will put enmity between you and the woman"]
    line.attr("indent");              // Some(&Value::Int(1))
}
for note in p.spans_in("notes") {
    note.local();                     // [142..142]: draw the marker here
}
for x in p.spans_in("xrefs") {
    let to = x.attr("to").unwrap();   // "ROM 16:20"
}

// A plain reading view: text and poetry lines only.
bible.find().in_span("verses", "GEN 3:15").with_layers(["structure"]).get_passage()?;
```

### Searching and counting

```rust
use klause::{spans_in, Distance};

// Verses that use a word.
bible.find().within(spans_in("morph").with_attr("strongs", "H342")).get_spans("verses")?;

// Verses that cross-reference Romans 16:20: the reverse direction of `xrefs`.
bible.find().within(spans_in("xrefs").with_attr("to", "ROM 16:20")).get_spans("verses")?;

// A phrase, ignoring case, within one book.
bible.find().in_span("books", "GEN").with_text_folded("seed of the woman").get_passages()?;

// Lemma frequencies in a chapter.
bible.find().in_span("chapters", "GEN 3").count_values("morph", "strongs")?;

// Nouns directly followed by a verb, anywhere.
bible.find()
    .within(spans_in("morph").with_attr("pos", "noun"))
    .followed_by(spans_in("morph").with_attr("pos", "verb"), Distance::adjacent())
    .count()?;
```

### Highlights from a database

```rust
use klause::{Layer, span_id};

bible.add_layer(Layer::source("highlights", Highlights { db: db.clone() }))?;

// The user selects `local`, a range of `p.text()` on screen.
let p = bible.get_passage(span_id("verses", "GEN 3:15"))?;
let start = p.range().start;
let whole_words = bible.find()
    .within(local.start + start..local.end + start)
    .expand_to("words")
    .get_selection()?;
db.save_highlight(whole_words.ranges(), "yellow")?;

bible.get_passage(span_id("verses", "GEN 3:15"))?.spans_in("highlights");   // includes it

// Everything the user highlighted in Genesis, as whole verses.
bible.find().in_span("books", "GEN").in_layer("highlights").expand_to("verses").get_passages()?;
```

## Implementation

### Storage

- **Text.** An `Arc<String>`. `push` uses `Arc::make_mut`, so it copies the text only while a passage still shares
  the old one. Passages hold the `Arc` and their range.
- **Spans.** Each stored layer keeps its spans in a `Vec` indexed by a slot number, with a map from `SpanId` to
  slot. Each slot also caches the span's resolved ranges, so reading never walks target chains.
- **Targets.** A reverse map from each span to the spans that target it. `replace_span` uses it to update the cached
  ranges of every span downstream; `remove_span` uses it to cascade.
- **Position index.** Per layer, an index answering "which slots touch this range" in O(log n + k), such as spans
  sorted by start with a running maximum end. Rebuild it lazily after edits, on the first read, so building a
  document with many `add_span` calls sorts once.
- **Attribute index.** Per layer, each distinct string value is stored once and given a `u32`. For each key, a map
  from value to the sorted list of slots that have it, and for numeric values, the slots sorted by value so a range
  filter is a binary search. Filters intersect these slot lists before touching span data. Built lazily like the
  position index.
- **Folded text.** The folded copy of the text and its map back to original positions are built on the first
  `with_text_folded` and extended on later pushes.

### Queries

A query collects its steps, then runs them when the final verb is called. Each run of steps between `expand_to`
steps is planned together: estimate each step's cost from index sizes, evaluate the cheapest first, and pass the
narrowed hits to the rest. Text search runs only inside the current hits.

### Dependencies

`unicode-normalization` for NFC and NFD, `unicode-properties` (or equivalent) for general categories and
`White_Space`, and `memchr` for substring search. Add others only when they replace substantial code.

### Performance targets

Measure against a generated Bible-sized document in a `benches/` suite: about 4.5 MB of text, 66 books, 1,189
chapters, 31,102 verses, 790,000 words, 790,000 morphology spans with `pos` and `strongs`, 50,000 structure spans,
20,000 notes, 340,000 cross-references, and a source layer of 10,000 highlights. Release build, one thread, a
current laptop:

| Operation                                                              | Target    |
| :--------------------------------------------------------------------- | :-------- |
| Building the document from rows in memory, through its first query     | < 1 s     |
| `get_passage` of one chapter with every layer                          | < 1 ms    |
| `within(spans_in("morph").with_attr("pos", "verb"))` over the Bible | < 20 ms   |
| `count_values("morph", "strongs")` over the Bible                      | < 50 ms   |
| Reverse cross-reference lookup for one verse                           | < 1 ms    |
| `with_text_folded` of a three-word phrase over the Bible               | < 30 ms   |
| Resident memory with the Bible loaded                                  | < 400 MB  |

## Open questions

1. **What `followed_by` returns.** It keeps the first part of a pair. Should there be a way to get both parts as one
   hit?
2. **Letters and their marks.** Ranges must start and end on character boundaries. Should a range also be kept from
   separating a letter from the combining marks after it?
3. **Concurrent edits.** Editing takes `&mut Document`, so reading on one thread while editing on another needs a
   lock. Should an edit instead produce a cheap new snapshot while readers keep the old one?
