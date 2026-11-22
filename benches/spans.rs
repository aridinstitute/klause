use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use klause::{Document, Span, SpanRef};

const WORDS: usize = 790_000;

fn words(count: usize) -> Document {
    let mut doc = Document::new("benchmark")
        .with_layer("words")
        .with_layer("morph")
        .with_layer("review");
    doc.push(&"word. ".repeat(count)).unwrap();
    for i in 0..count {
        let start = (i * 6) as u32;
        doc.add_span("words", Span::from(start..start + 4).with_id(i as u64))
            .unwrap();
    }
    doc
}

fn annotated(count: usize) -> Document {
    let mut doc = words(count);
    for i in 0..count {
        doc.add_span(
            "morph",
            Span::on("words", i as u64)
                .with_id(i as u64)
                .with_attr("pos", "noun")
                .with_attr("strongs", "H7225"),
        )
        .unwrap();
    }
    doc
}

fn spans(c: &mut Criterion) {
    let mut group = c.benchmark_group("spans");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for count in [1_000, 100_000, WORDS] {
        group.throughput(Throughput::Elements((count * 2) as u64));
        group.bench_with_input(
            BenchmarkId::new("build_words_and_morph", count),
            &count,
            |b, &n| {
                b.iter_batched(|| n, |n| black_box(annotated(n)), BatchSize::PerIteration);
            },
        );

        group.throughput(Throughput::Elements(1));
        let mut doc = annotated(count);
        let word = SpanRef::new("words", (count / 2) as u64);
        let tag = SpanRef::new("morph", (count / 2) as u64);
        doc.add_span("review", Span::on_ref(&tag)).unwrap();
        let start = (count / 2 * 6) as u32;
        let chapter = start..start + (count.min(500) * 6) as u32;
        group.bench_function(BenchmarkId::new("passage_cold", count), |b| {
            b.iter_batched_ref(
                || annotated(count),
                |doc| black_box(doc.get_passage(chapter.clone()).unwrap()),
                BatchSize::PerIteration,
            );
        });
        doc.get_passage(chapter.clone()).unwrap();
        group.bench_function(BenchmarkId::new("passage_warm", count), |b| {
            b.iter(|| black_box(doc.get_passage(chapter.clone()).unwrap()));
        });
        group.bench_function(BenchmarkId::new("cached_ranges", count), |b| {
            b.iter(|| black_box(doc.ranges(black_box(&tag)).unwrap()));
        });
        group.bench_function(BenchmarkId::new("replace_chain", count), |b| {
            let mut offset = 0;
            b.iter(|| {
                offset ^= 6;
                doc.replace_span(&word, Span::from(offset..offset + 4))
                    .unwrap();
                black_box(doc.ranges(&tag));
            });
        });
        group.bench_function(BenchmarkId::new("remove_chain", count), |b| {
            // Setup and destruction are excluded; unrelated spans stay loaded.
            b.iter_batched_ref(
                || {
                    let mut doc = annotated(count);
                    let tag = SpanRef::new("morph", (count / 2) as u64);
                    doc.add_span("review", Span::on_ref(&tag)).unwrap();
                    doc
                },
                |doc| doc.remove_span(&word).unwrap(),
                BatchSize::PerIteration,
            );
        });
    }

    for count in [1_000, 100_000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("remove_fanout", count), &count, |b, &n| {
            b.iter_batched_ref(
                || {
                    let mut doc = words(1);
                    for _ in 0..n {
                        doc.add_span("morph", Span::on("words", 0_u64)).unwrap();
                    }
                    doc
                },
                |doc| doc.remove_span(&SpanRef::new("words", 0_u64)).unwrap(),
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, spans);
criterion_main!(benches);
