use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use klause::{Corpus, Document, SpanFilter, Value, spans_in};

const WORDS_PER_DOCUMENT: usize = 12_000;
const DOCUMENTS: [usize; 3] = [1, 12, 66];

fn fixture(documents: usize) -> Corpus {
    let mut corpus = Corpus::new();
    for n in 0..documents {
        let language = if n % 3 == 0 { "fr" } else { "en" };
        let mut doc = Document::new(format!("book-{n:02}"))
            .with_attr("language", language)
            .with_layer("words");
        for word in 0..WORDS_PER_DOCUMENT {
            let (text, pos) = if word % 8 == 0 {
                ("walk", "verb")
            } else {
                ("road", "noun")
            };
            doc.push_span("words", text, |span| span.with_attr("pos", pos))
                .unwrap();
            doc.push(" ").unwrap();
        }
        corpus.add_document(doc).unwrap();
    }
    corpus
}

fn verb() -> SpanFilter {
    spans_in("words").with_attr("pos", "verb")
}

fn across_documents(c: &mut Criterion) {
    let mut group = c.benchmark_group("corpus");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for n in DOCUMENTS {
        let corpus = fixture(n);
        let words = n * WORDS_PER_DOCUMENT;
        let verbs = words / 8;
        let french_verbs = n.div_ceil(3) * WORDS_PER_DOCUMENT / 8;
        let french = Value::from("fr");
        let filter = verb();
        let count_verbs = || corpus.find().count_spans(filter.clone()).unwrap();
        let count_phrase = || corpus.find().with_text("walk road road").count().unwrap();
        let count_french_verbs = || {
            corpus
                .find()
                .filter_documents(|doc| doc.attr("language") == Some(&french))
                .count_spans(filter.clone())
                .unwrap()
        };
        let count_values = || corpus.find().count_values("words", "pos").unwrap();
        assert_eq!(count_verbs(), verbs);
        assert_eq!(count_phrase(), verbs);
        assert_eq!(count_french_verbs(), french_verbs);
        assert_eq!(
            count_values(),
            [
                (Value::from("noun"), words - verbs),
                (Value::from("verb"), verbs)
            ]
        );

        group.throughput(Throughput::Elements(words as u64));
        group.bench_function(BenchmarkId::new("warm_span_count", n), |b| {
            b.iter(|| black_box(count_verbs()))
        });
        group.bench_function(BenchmarkId::new("literal_phrase_count", n), |b| {
            b.iter(|| black_box(count_phrase()))
        });
        group.bench_function(BenchmarkId::new("value_counts", n), |b| {
            b.iter(|| black_box(count_values()))
        });
        group.throughput(Throughput::Elements(
            (n.div_ceil(3) * WORDS_PER_DOCUMENT) as u64,
        ));
        group.bench_function(BenchmarkId::new("metadata_scoped_span_count", n), |b| {
            b.iter(|| black_box(count_french_verbs()))
        });
    }

    // Fresh documents include lazy index creation, not fixture construction.
    group.throughput(Throughput::Elements((66 * WORDS_PER_DOCUMENT) as u64));
    let filter = verb();
    group.bench_function(BenchmarkId::new("first_span_count", 66), |b| {
        b.iter_batched_ref(
            || fixture(66),
            |corpus| black_box(corpus.find().count_spans(filter.clone()).unwrap()),
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

criterion_group!(benches, across_documents);
criterion_main!(benches);
