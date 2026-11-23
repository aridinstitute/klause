# Benchmarks

Run the stored-span benchmarks:

```sh
cargo bench --bench spans
```

Run the span-filtering benchmarks:

```sh
cargo bench --bench filtering
```

The filtering fixture has 1,000, 100,000, or 790,000 words with morphology spans
tagged as verbs, nouns, or other. It measures attribute-filtered span counts
and the selection produced by filtering for verbs. Fixture construction is
excluded from the timed operations.

For a per-filter comparison at 100,000 words, or a separate proximity run:

```sh
cargo bench --bench filtering -- '^filter_types/.*/100000$'
cargo bench --bench filtering -- '^proximity/'
# All filtering cases on the 790,000-word fixture:
cargo bench --bench filtering -- '/790000$'
```

| Group | What it measures |
| :-- | :-- |
| `filter_types` | Warm span counts for layer-only and all-layer filters, exact values (string, integer, float, bool), bounded and open numeric ranges, `any_of`, a custom matcher, attribute negation, key presence and absence, span ID, target span, and combined predicates. `source_fallback` uses an in-memory generated source with no pushdown or external I/O; `all_layers` includes that layer. |
| `query_filters` | Warm range counts for `in_layer`, `outside`, and `expand_to`. See `planning` below for chapter-scoped `in_span`, `within`, and literal/folded text search. |
| `proximity` | Both directions (`followed_by` / `preceded_by`) with adjacent, byte, and intervening-span distances at 128, 512, and 790,000 words. |

Each new case checks its expected count before timing. `filter_types` excludes
fixture creation, filter construction/cloning, and first-use index creation;
`query_filters` and `proximity` include query construction. Most predicate cases
match about one word in 16, but layer-only and all-layer cases match whole
layers, span ID matches one span, and the integer and combined cases match
about one word in 32. Use these as representative workloads, not as costs per
match or as end-to-end timings.

The fixture has 1,000, 100,000, or 790,000 words, each with one morphology span
targeting it and carrying `pos` and `strongs`. The largest has 4.74 MB of text
and 1.58 million spans. Edits use a word → morphology → review chain with all
other spans still loaded. Fanout removal deletes one word and all its tags.

Criterion uses 10 samples, a 1-second warmup, and a 3-second measurement target
(extended automatically for large inputs). Building includes fixture generation
but excludes destruction. Removal excludes fixture setup and document destruction;
its targets are freshly built for each operation. Replacement and range lookup
reuse a document. These are single-threaded release measurements, not compilation
times or end-to-end application timings.

## Local results, 2026-11-21

Darwin, 14 logical CPUs, 48 GB RAM. Criterion point estimates:

| Operation | 1,000 words | 100,000 words | 790,000 words |
| :-- | --: | --: | --: |
| Build words and morphology | 330 µs | 44.7 ms | 386 ms |
| Cached morphology ranges | 13.7 ns | 21.1 ns | 25.4 ns |
| Replace word, update two descendants | 78.8 ns | 93.2 ns | 102 ns |
| Remove word and two descendants | 281 ns | 2.43 µs | 5.88 µs |

Removing 100,000 tags and their target: **10.9 ms**.

The existing creation suite was also run:

```sh
cargo bench --bench document_creation -- --sample-size 10 --warm-up-time 1 --measurement-time 3
```

At 4.5 MB: plain text **33.5 ms**; annotated rows (750,000 words and 250,000
verse spans) **288 ms**.

## Filtering results, 2026-11-21

Darwin, 14 logical CPUs, 48 GB RAM. Criterion point estimates from
`cargo bench --bench filtering -- --sample-size 10 --warm-up-time 1 --measurement-time 3`:

| Operation | 1,000 words | 100,000 words | 790,000 words |
| :-- | --: | --: | --: |
| First attribute-filtered span count | 57.6 µs | 6.10 ms | 51.7 ms |
| Warm attribute-filtered span count | 831 ns | 85.1 µs | 4.53 ms |
| Warm attribute-filtered selection | 1.92 µs | 140 µs | 5.06 ms |

The first count includes lazy index construction; warm operations reuse the
index. The largest warmed filter and selection are both below the 20 ms target.

Lazy reverse-dependency storage avoids allocating dependency lists for
unreferenced layers. Compared with the initial eager implementation in this
session, the largest annotated-row build changed from 291 ms to 288 ms (within
Criterion's noise threshold); the word/morphology build changed from 374 ms to
386 ms, and chain removal from 3.90 µs to 5.88 µs. This is not an across-the-board
speedup.

These original measurements cover one attribute filter, not the full Bible
workload in `DESIGN.md`. The expanded filtering cases use synthetic data and
an in-memory source; they do not measure real source I/O, value counting,
resident memory, or the end-to-end first-query target.

## Filtering after the 100 ms optimization pass

Release-mode Criterion run: all 87 filtering cases completed. The table shows
all 29 filtering cases measured at 790,000 words, plus the four chapter-scoped
planning cases on a 790,000-word document. The upper bound is from Criterion's
estimate interval, not a per-query tail latency. The largest estimate was
69.23 ms (source fallback), with an upper bound of 69.68 ms.

| Benchmark | Point estimate | Upper bound |
| :-- | --: | --: |
| `filtering/first_attribute_filter_count_spans` | 52.19 ms | 54.02 ms |
| `filtering/attribute_filter_count_spans` | 3.92 ms | 4.06 ms |
| `filtering/attribute_filter_selection` | 3.22 ms | 3.29 ms |
| `filter_types/layer_only` | 155.08 µs | 155.63 µs |
| `filter_types/all_layers` | 66.34 ms | 67.48 ms |
| `filter_types/exact_string` | 4.94 ms | 5.16 ms |
| `filter_types/exact_integer` | 1.55 ms | 1.68 ms |
| `filter_types/exact_float` | 3.97 ms | 4.25 ms |
| `filter_types/exact_bool` | 2.46 ms | 2.52 ms |
| `filter_types/bounded_range` | 15.94 ms | 16.01 ms |
| `filter_types/open_range` | 15.65 ms | 15.89 ms |
| `filter_types/any_of` | 17.31 ms | 17.84 ms |
| `filter_types/custom_matcher` | 15.19 ms | 15.27 ms |
| `filter_types/without_attr` | 17.56 ms | 18 ms |
| `filter_types/with_key` | 14.87 ms | 15.17 ms |
| `filter_types/without_key` | 16.38 ms | 16.48 ms |
| `filter_types/with_id` | 10.95 ms | 11 ms |
| `filter_types/targeting` | 45.47 ms | 45.59 ms |
| `filter_types/combined` | 6.69 ms | 6.71 ms |
| `filter_types/source_fallback` | 69.23 ms | 69.68 ms |
| `query_filters/in_layer` | 2.45 ms | 2.54 ms |
| `query_filters/outside` | 5.65 ms | 5.99 ms |
| `query_filters/expand_to` | 5.76 ms | 5.88 ms |
| `proximity/followed_by_adjacent` | 25.27 ms | 25.31 ms |
| `proximity/preceded_by_adjacent` | 25.81 ms | 26.14 ms |
| `proximity/followed_by_bytes` | 24.62 ms | 25.31 ms |
| `proximity/preceded_by_bytes` | 24.54 ms | 24.67 ms |
| `proximity/followed_by_span_count` | 30.08 ms | 30.15 ms |
| `proximity/preceded_by_span_count` | 30.07 ms | 30.19 ms |
| `planning/literal` | 2.194 µs | 2.199 µs |
| `planning/folded` | 60.488 µs | 61.043 µs |
| `planning/filter_cold` | 13.97 ms | 14.2 ms |
| `planning/filter_warm` | 8.076 µs | 8.11 µs |

The previous full run measured `outside` at 1.30 s, `all_layers` at 471.21 ms,
and `expand_to` at 36.59 ms on this size. Span-count proximity at 512 words
went from 2.12–2.36 ms to 9.27–9.32 µs; no pre-optimization proximity timings
were taken at 790,000 words.

These are span counts and selection/range queries on synthetic corpora, not
end-to-end timings or measurements of real source I/O or materializing large
result sets.

## Position-index results, 2026-11-21

Run the passage cases in the stored-span suite:

```sh
cargo bench --bench spans -- passage
```

These use the word/morphology fixture above and load a 500-word passage from its
middle. Cold operations use a fresh document and include index construction but
exclude fixture setup and document destruction. Warm operations reuse the
indexes and also include the one review span targeting the middle word. Warm
timings include passage destruction; Criterion's batched cold timings do not.

Darwin, 14 logical CPUs, 48 GB RAM, single-threaded release, Criterion point estimates:

| Operation | 1,000 words | 100,000 words | 790,000 words |
| :-- | --: | --: | --: |
| Cold passage, before position index | 285 µs | 33.7 ms | 267 ms |
| Cold passage, with position index | 78.8 µs | 2.36 ms | 24.9 ms |
| Warm passage, before position index | 315 µs | 32.7 ms | 264 ms |
| Warm passage, with position index | 108 µs | 108 µs | 108 µs |

The position index is built when a stored layer is included in a passage or a
query chooses a position lookup within narrowed hits.
It stores each resolved range separately, prunes subtrees by maximum end, and
deduplicates matching slots. Adds, removals, and replacements invalidate affected
layers, including downstream target layers. Text pushes alone keep it valid.

These passage measurements cover 1.58 million word/morphology spans at the
largest size, not all layers in the complete Bible workload. They do not establish
the full-workload chapter, memory, or build-through-first-query targets.

## Query-planning results, 2026-11-21

```sh
cargo bench --bench planning
# Measure the largest input in its own fresh process:
KLAUSE_BENCH_WORDS=790000 cargo bench --bench planning
```

This fixture has 1,000, 100,000, or 790,000 words, each with a `pos` attribute,
plus one chapter per 500 words. Text is `"word. "` repeated. The measured text
queries put literal/folded search **before** `in_span("chapters", 0)` in the
builder chain and must return exactly 500 matches. The filter query puts
`within(spans_in("words").with_attr("pos", "verb"))` before that chapter
restriction and must return 32 matches (every sixteenth word is a verb).

The planner applies the chapter restriction first. The filter can then use the
position index for local candidates instead of indexing attributes across the
whole document. Query construction and execution are both timed. Cold filter
measurements use a newly built document per iteration, excluding document setup
and destruction but including any lazy index construction. Text and warm filter
timings reuse one document and include result destruction.

Same machine and Criterion settings as above. Before/after point estimates:

| Operation | Before, 1,000 words | After, 1,000 words | Before, 790,000 words | After, 790,000 words |
| :-- | --: | --: | --: | --: |
| Literal search in chapter | 8.69 µs | 3.38 µs | 6.62 ms | 3.42 µs |
| Folded search in chapter | 118 µs | 58.9 µs | 99.7 ms | 58.3 µs |
| Cold verb filter in chapter | 48.5 µs | 13.2 µs | 50.8 ms | 12.3 ms |
| Warm verb filter in chapter | 2.26 µs | 8.40 µs | 4.42 ms | 8.37 µs |

In the fresh 790,000-word process, the first literal search took **34.3 µs** and
the first folded search **75.7 µs**. Fixture construction is excluded. These
first calls are reported separately from Criterion's warmed samples.

The smallest warm filter regresses by about 6 µs: without a built attribute
index, the cost model chooses a local position lookup and tests roughly 500
spans, whereas the old warm path already had an attribute index with only 63
verb candidates across the entire 1,000-word document. These estimates are not
a promise that every query gets the optimal plan.

Planning does not build indexes or read sources just to estimate costs.
Unknown custom and source selectors retain their position among selection
steps, as do selectors that might contain points. Text-search order is
preserved; selection restrictions still run before text searches regardless of
builder order. `expand_to` is a hard boundary. See `DESIGN.md` for the semantics.

This benchmark is not the complete Bible fixture and does not establish the
whole-document folded-search target, full-workload resident memory, proximity
performance, or the build-through-first-query target.

## Cross-document corpus

```sh
cargo bench --locked --bench corpus -- --noplot
```

The generated corpus contains 1, 12, or 66 documents, each with 12,000 word
spans. One in eight words is tagged as a verb, and one in three documents has
`language = "fr"`. The timed cases include query construction but not fixture
construction. Warm cases reuse the documents and indexes. The first span count
uses a fresh corpus on each iteration and includes lazy attribute indexing.

Apple M4 Pro, single-threaded release build, Criterion point estimates:

| Operation | 1 document | 12 documents | 66 documents (792,000 words) |
| :-- | --: | --: | --: |
| Warm verb span count | 17.0 µs | 293 µs | 7.29 ms |
| Literal three-word phrase count | 26.5 µs | 325 µs | 1.75 ms |
| Part-of-speech value counts | 167 µs | 3.00 ms | 22.36 ms |
| Metadata-scoped warm verb span count | 17.0 µs | 73.3 µs | 814 µs |
| First verb span count | — | — | 46.56 ms |

The baseline value-count case took 93.34 ms at 66 documents. Counting from
borrowed stored spans instead of cloning them, and avoiding a per-span
allocation for duplicate checks, brought that case down to 22.36 ms. This case
has only two distinct values and does not measure high-cardinality aggregation
such as Strong's frequencies. This suite also does not validate the complete
single-document Bible workload or its memory target.

## Full Bible workload

```sh
cargo bench --locked --bench bible_workload -- --noplot
cargo bench --locked --bench bible_workload -- --rss
cargo bench --locked --bench bsb_usj -- 'full_.*phrase' --noplot
```

`bible_workload` generates one document with about 4.5 MB of text, 66 books,
1,189 chapters, 31,102 verses, 790,000 words and as many morphology spans,
50,000 structure spans, 20,000 point notes, and 340,000 cross-references. Every
morphology span has a part of speech and one of 5,600 Strong’s IDs; half the
words draw from 128 common IDs. A range-indexed, in-memory source serves
10,000 highlights without external I/O. The fixture is deterministic and
models annotation volumes, not the distribution of a particular Bible export.

Apple M4 Pro, macOS arm64, 48 GiB RAM, single-threaded release build. Timings
are Criterion point estimates (10 samples). Cold cases use a fresh document
and include lazy index construction but exclude document build and destruction.
Build-through-first-chapter starts with prepared rows, includes constructing the
document and reading an all-layer chapter, and excludes destroying the document.

| Operation | Design target | Cold | Warm |
| :-- | --: | --: | --: |
| Build through first chapter | < 1 s | 661 ms | — |
| Chapter passage, all layers | < 1 ms | 46.5 ms | 224 µs |
| Bible-wide verb selection | < 20 ms | 149 ms | 9.79 ms |
| Bible-wide Strong’s counts | < 50 ms | — | 1.84 s |
| Reverse cross-reference lookup | < 1 ms | 55.9 ms | 2.42 ms |

The standalone `--rss` process uses `ps` on macOS/Linux to sample current
resident memory, not peak memory. It reported **759.7 MB** loaded (before
indexes), and **947.7 MB** after chapter, verb, and reverse-reference queries,
against a **400 MB** target. The process started at 2.2 MB. Rows are dropped
before the loaded snapshot; RSS can still include allocator-retained pages.

### Folded search vs. verse materialization

The folded-selection case includes matching and returning hit ranges.
The verse-only case starts from those precomputed ranges; it times the current
`get_spans` path, including filtering and materialization. The combined case
runs the normal query in one chain. These are independent timings, not an
additive breakdown. The synthetic query is three words with eight hits;
real BSB uses “for god so loved the world” and finds one verse.

| Operation | Full synthetic workload | Real BSB USJ |
| :-- | --: | --: |
| Folded search → selection | 93.6 ms | 78.3 ms |
| Precomputed hits → verse spans | 2.32 ms | 6.25 ms |
| Folded search → verse spans | 97.6 ms | 88.4 ms |

Matching dominates the combined search on both fixtures and exceeds the 30 ms
target on its own. Verse materialization is smaller but still enough to exceed
the 1 ms budget for reverse lookup, which uses the same span-returning path.

### Keyed aggregation and streaming fold

Same machine and release settings; default features. Before values are from the
baseline above. These cases were re-run; the other baselines were not remeasured.

| Operation | Before | After | Target |
| :-- | --: | --: | --: |
| Synthetic Strong’s counts (5,600 distinct values) | 1.84 s | 36.3 ms | < 50 ms |
| Synthetic folded phrase → selection | 93.6 ms | 10.0 ms | < 30 ms |
| Synthetic precomputed hits → verse spans | 2.32 ms | 2.46 ms | — |
| Synthetic folded phrase → verse spans | 97.6 ms | 12.5 ms | — |
| Real BSB folded phrase → selection | 78.3 ms | 9.45 ms | — |
| Real BSB precomputed hits → verse spans | 6.25 ms | 6.64 ms | — |
| Real BSB folded phrase → verse spans | 88.4 ms | 15.8 ms | — |

High-cardinality counts now use a per-query hash map (`BTreeMap` without
`std`), while the two-value corpus fixture stays on a small linear accumulator.
Its 66-document value count was 23.1 ms versus the earlier 22.36 ms.
Folded search now streams matches with pattern-sized scratch storage and no
persistent cache. Verse materialization was not changed.

### Compact layer storage

Same synthetic fixture and release settings. The RSS run drops the fixture rows
before sampling. Timing estimates below are from paired Criterion `--quick`
runs and indicate direction and scale, not statistical significance.

| Operation | Before | After |
| :-- | --: | --: |
| Loaded RSS, before indexes | 765.1 MB | 297.8 MB |
| RSS after chapter, verb, and reverse-reference queries | 946.3 MB | 349.1 MB |
| Build through first chapter | 672 ms | 737 ms |
| Cold chapter passage | 47.3 ms | 19.0 ms |
| Warm chapter passage | 220 µs | 265 µs |
| Cold verb selection | 155 ms | 35.3 ms |
| Warm verb selection | 8.81 ms | 1.59 ms |
| Strong’s counts | 36.6 ms | 23.3 ms |
| Warm reverse-reference lookup | 2.34 ms | 232 µs |

A word uses 16 bytes of column data (ID and range); a same-ID morphology span
uses 32 (ID, resolved range, and two attribute pairs). These figures omit
dictionary entries, spare vector capacity, auxiliary indexes, and any spans
materialized for borrowed public reads.
