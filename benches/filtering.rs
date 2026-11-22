use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use klause::{Document, Span, spans_in};

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
                    BatchSize::SmallInput,
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

criterion_group!(benches, filtering);
criterion_main!(benches);
