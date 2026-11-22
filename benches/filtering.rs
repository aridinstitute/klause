use std::hint::black_box;
use std::ops::Range;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use klause::{
    Distance, Document, Layer, SourceError, Span, SpanFilter, SpanSource, Value, ValueMatcher,
    all_spans, any_of, spans_in,
};

const WORDS: usize = 790_000;

fn annotated(count: usize) -> Document {
    let mut doc = Document::new("filtering")
        .with_layer("words")
        .with_layer("morph");
    let mut text = String::with_capacity(count * 5);
    for _ in 0..count {
        text.push_str("word ");
    }
    doc.push(&text).unwrap();

    for i in 0..count {
        let start = (i * 5) as u32;
        doc.add_span("words", Span::from(start..start + 4).with_id(i as u64))
            .unwrap();
        let pos = if i % 16 == 0 {
            "verb"
        } else if i % 3 == 0 {
            "noun"
        } else {
            "other"
        };
        doc.add_span(
            "morph",
            Span::on("words", i as u64)
                .with_id(i as u64)
                .with_attr("pos", pos),
        )
        .unwrap();
    }

    doc
}

fn filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("filtering");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for count in [1_000, 100_000, WORDS] {
        let doc = annotated(count);
        let expected = count.div_ceil(16);
        assert_eq!(
            doc.find()
                .count_spans(spans_in("morph").with_attr("pos", "verb"))
                .unwrap(),
            expected
        );

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("first_attribute_filter_count_spans", count),
            &count,
            |b, &n| {
                b.iter_batched_ref(
                    || annotated(n),
                    |doc| {
                        black_box(
                            doc.find()
                                .count_spans(spans_in("morph").with_attr("pos", "verb"))
                                .unwrap(),
                        )
                    },
                    BatchSize::PerIteration,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("attribute_filter_count_spans", count),
            &count,
            |b, _| {
                b.iter(|| {
                    black_box(
                        doc.find()
                            .count_spans(spans_in("morph").with_attr("pos", "verb"))
                            .unwrap(),
                    )
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("attribute_filter_selection", count),
            &count,
            |b, _| {
                b.iter(|| {
                    let selection = doc
                        .find()
                        .within(spans_in("morph").with_attr("pos", "verb"))
                        .get_selection()
                        .unwrap();
                    black_box(selection.ranges().len())
                });
            },
        );
    }
    group.finish();
}

// A source with range-based loading but no filter pushdown or external I/O.
struct GeneratedMorph;

impl SpanSource for GeneratedMorph {
    fn get_spans(&self, range: Range<u32>) -> Result<Vec<Span>, SourceError> {
        Ok((range.start / 5..range.end.div_ceil(5))
            .filter(|&i| range.start < i * 5 + 4 && i * 5 < range.end)
            .map(|i| {
                Span::from(i * 5..i * 5 + 4)
                    .with_id(i as u64)
                    .with_attr("pos", if i % 16 == 0 { "verb" } else { "other" })
            })
            .collect())
    }
}

fn filter_fixture(words: usize) -> Document {
    let mut doc = Document::new("filter_types")
        .with_layer("words")
        .with_layer("morph")
        .with_layer(Layer::source("generated_morph", GeneratedMorph));
    doc.push(&"word ".repeat(words)).unwrap();
    for i in 0..words {
        let start = (i * 5) as u32;
        let verb = i % 16 == 0;
        let word = Span::from(start..start + 4).with_id(i as u64);
        let word = if verb {
            word.with_attr("verb", true)
        } else {
            word
        };
        doc.add_span("words", word).unwrap();

        let morph = Span::on("words", i as u64)
            .with_id(i as u64)
            .with_attr("pos", if verb { "verb" } else { "other" })
            .with_attr("bucket", if verb { i as i64 % 32 / 16 } else { 2 })
            .with_attr("score", (i % 16) as f64 / 2.0);
        let morph = if verb {
            morph
        } else {
            morph.with_attr("common", true)
        };
        doc.add_span("morph", morph).unwrap();
    }
    doc
}

struct ZeroScore;

impl ValueMatcher for ZeroScore {
    fn matches(&self, value: &Value) -> bool {
        matches!(value, Value::Float(score) if *score == 0.0)
    }
}

fn filter_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_types");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    for words in [1_000, 100_000, WORDS] {
        let doc = filter_fixture(words);
        let rare = words.div_ceil(16);
        let morph = || spans_in("morph");
        let cases = [
            ("layer_only", morph(), words),
            ("all_layers", all_spans(), 3 * words),
            ("exact_string", morph().with_attr("pos", "verb"), rare),
            (
                "exact_integer",
                morph().with_attr("bucket", 0_i64),
                words.div_ceil(32),
            ),
            ("exact_float", morph().with_attr("score", 0.0), rare),
            (
                "exact_bool",
                spans_in("words").with_attr("verb", true),
                rare,
            ),
            ("bounded_range", morph().with_attr("bucket", 0..2), rare),
            ("open_range", morph().with_attr("score", 7.5..), words / 16),
            ("any_of", morph().with_attr("bucket", any_of([0, 1])), rare),
            (
                "custom_matcher",
                morph().with_attr("score", ZeroScore),
                rare,
            ),
            ("without_attr", morph().without_attr("pos", "other"), rare),
            ("with_key", spans_in("words").with_key("verb"), rare),
            ("without_key", morph().without_key("common"), rare),
            ("with_id", morph().with_id((words / 2) as u64), 1),
            (
                "targeting",
                morph().targeting(spans_in("words").with_key("verb")),
                rare,
            ),
            (
                "combined",
                morph().with_attr("pos", "verb").with_attr("bucket", 0..1),
                words.div_ceil(32),
            ),
            (
                "source_fallback",
                spans_in("generated_morph").with_attr("pos", "verb"),
                rare,
            ),
        ];
        for (name, filter, expected) in cases {
            let run = |filter: SpanFilter| doc.find().count_spans(filter).unwrap();
            assert_eq!(run(filter.clone()), expected, "{name}/{words}");
            group.bench_function(BenchmarkId::new(name, words), |b| {
                // Unlike the historical cases, keep filter construction and
                // cloning outside the measurement so predicate setup doesn't
                // dominate the cheaper queries.
                b.iter_batched(
                    || filter.clone(),
                    |filter| black_box(run(filter)),
                    BatchSize::SmallInput,
                );
            });
        }
    }
    group.finish();
}

fn query_filters(c: &mut Criterion) {
    type QueryCase = (&'static str, fn(&Document) -> usize, fn(usize) -> usize);
    let cases: [QueryCase; 3] = [
        (
            "in_layer",
            |doc| doc.find().in_layer("morph").count().unwrap(),
            |words| words,
        ),
        (
            "outside",
            |doc| {
                doc.find()
                    .outside(spans_in("morph").with_attr("pos", "verb"))
                    .count()
                    .unwrap()
            },
            |words| words.div_ceil(16),
        ),
        (
            "expand_to",
            |doc| {
                doc.find()
                    .within(1..2)
                    .expand_to(spans_in("morph").with_attr("pos", "verb"))
                    .count()
                    .unwrap()
            },
            |_| 1,
        ),
    ];
    let mut group = c.benchmark_group("query_filters");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    for words in [1_000, 100_000, WORDS] {
        let doc = filter_fixture(words);
        for (name, run, expected) in cases {
            assert_eq!(run(&doc), expected(words), "{name}/{words}");
            group.bench_function(BenchmarkId::new(name, words), |b| {
                b.iter(|| black_box(run(&doc)));
            });
        }
    }
    group.finish();
}

fn proximity(c: &mut Criterion) {
    let mut group = c.benchmark_group("proximity");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    for words in [128, 512, WORDS] {
        let doc = filter_fixture(words);
        for (distance_name, distance) in [
            ("adjacent", Distance::adjacent()),
            ("bytes", Distance::bytes(1)),
            ("span_count", Distance::at_most(0, "words")),
        ] {
            for (direction, follows, expected) in [
                ("followed_by", true, words / 16 - 1),
                ("preceded_by", false, words / 16),
            ] {
                let run = || {
                    let hits = doc.find().within(spans_in("words").without_key("verb"));
                    let verbs = spans_in("morph").with_attr("pos", "verb");
                    let query = if follows {
                        hits.followed_by(verbs, distance.clone())
                    } else {
                        hits.preceded_by(verbs, distance.clone())
                    };
                    query.count().unwrap()
                };
                let name = format!("{direction}_{distance_name}");
                assert_eq!(run(), expected, "{name}/{words}");
                group.bench_function(BenchmarkId::new(name, words), |b| {
                    b.iter(|| black_box(run()));
                });
            }
        }
    }
    group.finish();
}

criterion_group!(benches, filtering, filter_types, query_filters, proximity);
criterion_main!(benches);
