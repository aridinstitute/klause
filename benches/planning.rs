use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use klause::{Document, Span, spans_in};

fn document(words: usize) -> Document {
    let mut doc = Document::new("planning")
        .with_layer("words")
        .with_layer("chapters");
    doc.push(&"word. ".repeat(words)).unwrap();
    for i in 0..words {
        let start = (i * 6) as u32;
        doc.add_span(
            "words",
            Span::from(start..start + 4)
                .with_id(i as u64)
                .with_attr("pos", if i % 16 == 0 { "verb" } else { "noun" }),
        )
        .unwrap();
    }
    for (id, start) in (0..words).step_by(500).enumerate() {
        doc.add_span(
            "chapters",
            Span::from((start * 6) as u32..((start + 500).min(words) * 6) as u32)
                .with_id(id as u64),
        )
        .unwrap();
    }
    doc
}

fn run(doc: &Document, folded: bool) -> usize {
    let query = if folded {
        doc.find().with_text_folded("WORD")
    } else {
        doc.find().with_text("word")
    };
    query.in_span("chapters", 0_u64).count().unwrap()
}

fn planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("planning");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    let sizes = std::env::var("KLAUSE_BENCH_WORDS").map_or_else(
        |_| vec![1_000, 100_000, 790_000],
        |words| {
            vec![
                words
                    .parse()
                    .expect("KLAUSE_BENCH_WORDS must be a word count"),
            ]
        },
    );
    for words in sizes {
        assert!(words >= 500, "planning fixture needs at least 500 words");
        let doc = document(words);
        for folded in [false, true] {
            let name = if folded { "folded" } else { "literal" };
            let start = Instant::now();
            assert_eq!(run(&doc, folded), 500);
            println!("{name}/{words} first: {:?}", start.elapsed());
            group.bench_function(BenchmarkId::new(name, words), |b| {
                b.iter(|| black_box(run(&doc, folded)));
            });
        }
        // Construction of the query itself belongs in the measurement. In
        // particular, lazy attribute-index work must not escape into setup.
        group.bench_function(BenchmarkId::new("filter_cold", words), |b| {
            b.iter_batched_ref(
                || document(words),
                |doc| {
                    black_box(
                        doc.find()
                            .within(spans_in("words").with_attr("pos", "verb"))
                            .in_span("chapters", 0_u64)
                            .count()
                            .unwrap(),
                    )
                },
                BatchSize::PerIteration,
            );
        });
        assert_eq!(
            doc.find()
                .within(spans_in("words").with_attr("pos", "verb"))
                .in_span("chapters", 0_u64)
                .count()
                .unwrap(),
            32
        );
        group.bench_function(BenchmarkId::new("filter_warm", words), |b| {
            b.iter(|| {
                black_box(
                    doc.find()
                        .within(spans_in("words").with_attr("pos", "verb"))
                        .in_span("chapters", 0_u64)
                        .count()
                        .unwrap(),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(benches, planning);
criterion_main!(benches);
