# Remaining work

Status as of 2026-11-21.

The core single-document API is implemented and tested. The main remaining work
is performance and release readiness. `DESIGN.md` describes the intended scope
and performance targets.

## Performance architecture

- [x] Add a lazy per-layer position index. Stored passage spans are looked up
  through a balanced interval index, rebuilt after span edits on the next lookup.
- [x] Add query planning. Between `expand_to` steps, selections run before text
  searches and proximity filters. Cost estimates order selection work and choose
  position or attribute lookups within narrowed hits. Text-search order and
  point-sensitive selection boundaries are preserved; see `DESIGN.md`.
- [ ] Cache folded text and its map back to original positions, extending the
  cache after pushes. Folded searches currently rebuild it on each call.
- [ ] Extend attribute indexing to numeric ranges. Exact-value filtering is
  indexed, but the planned numeric-range index is not implemented.

## Full-workload validation

The existing benchmarks do not implement the complete Bible workload described
in `DESIGN.md`. Add that fixture and measure these single-threaded release targets:

| Operation | Target |
| :-- | --: |
| Build from rows through the first query | < 1 s |
| Load a chapter passage with every layer | < 1 ms |
| Filter for verbs across the Bible | < 20 ms |
| Count morphology `strongs` values | < 50 ms |
| Reverse cross-reference lookup for one verse | < 1 ms |
| Folded search for a three-word phrase | < 30 ms |
| Resident memory with the Bible loaded | < 400 MB |

- [ ] Include every specified stored layer and the source layer of highlights.
- [ ] Measure cold and warm queries separately, including lazy index construction
  in first-query measurements.
- [ ] Measure resident memory alongside operation timings.

The current 790,000-word filtering fixture measures a warm filtered count at
4.53 ms and warm filtered selection at 5.06 ms. Its first filtered count takes
51.7 ms, including lazy index construction. These measurements establish the
warm filtering result only, not the full-workload or end-to-end targets.
See `benches/README.md` for fixture details and recorded results.

## Documentation and release readiness

- [ ] Replace the README's broken `Klause::new()` example with a working
  `Document` example.
- [ ] Correct outdated README descriptions of links and list values. Remove
  `Corpus` and `Anchor` from the current API description or clearly label them
  as future ideas.
- [ ] Migrate the disabled API sketches into compiling examples for supported
  features. `autoexamples = false` currently excludes them from normal checks;
  future-only sketches should remain clearly separate.
- [ ] Fill in the package description, keywords, and categories in `Cargo.toml`.

## Open design decisions

These are optional extensions or policy decisions, not established release
blockers:

- Should `followed_by` offer a way to return both parts of a pair as one hit?
- Should ranges be prevented from separating a letter from its combining marks,
  beyond the existing UTF-8 character-boundary requirement?
- Should editing produce cheap snapshots so readers can retain an older version
  without locking the same document?

Corpus queries, anchors, and built-in persistence appear in older sketches but
are not requirements of the current single-document design. File and database
I/O currently belong to applications through `IntoDocument` and read accessors.

## Suggested order

1. Add and run the complete workload benchmark.
2. Use those results to prioritize the remaining performance work.
3. Update documentation, examples, and package metadata before publishing.
