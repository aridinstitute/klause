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

These fixtures do **not** implement the complete Bible workload in `DESIGN.md`.
They measure one attribute-filter path but not source layers, passage loading,
value counting, folded text search, or resident memory, so they do not establish
those targets or the end-to-end first-query target.

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

The position index is built only when a stored layer is included in a passage.
It stores each resolved range separately, prunes subtrees by maximum end, and
deduplicates matching slots. Adds, removals, and replacements invalidate affected
layers, including downstream target layers. Text pushes alone keep it valid.

These passage measurements cover 1.58 million word/morphology spans at the
largest size, not all layers in the complete Bible workload. They do not establish
the full-workload chapter, memory, or build-through-first-query targets.
