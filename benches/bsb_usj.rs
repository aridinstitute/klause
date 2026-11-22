use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use klause::{Document, Span, spans_in};

#[path = "../tests/common/bsb_usj.rs"]
mod bsb_usj;

const PHRASE: &str = "for god so loved the world";
fn word_count(doc: &Document) -> usize {
    doc.find()
        .count_spans(spans_in("words").with_attr("token", "jerusalem"))
        .unwrap()
}

fn real_bible(c: &mut Criterion) {
    let mut group = c.benchmark_group("bsb_usj");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let doc = bsb_usj::load().unwrap();
    // The untimed setup builds a fresh document without a warmed word index.
    group.bench_function("first_word_attribute_count", |b| {
        b.iter_batched_ref(
            || bsb_usj::load().unwrap(),
            |doc| black_box(word_count(doc)),
            BatchSize::PerIteration,
        );
    });
    assert!(word_count(&doc) > 0);
    group.bench_function("word_attribute_count", |b| {
        b.iter(|| black_box(word_count(&doc)))
    });

    let chapter_count = || {
        doc.find()
            .count_spans(
                spans_in("verses")
                    .with_attr("book", "JHN")
                    .with_attr("chapter", 1),
            )
            .unwrap()
    };
    assert_eq!(chapter_count(), 51);
    group.bench_function("combined_verse_attribute_count", |b| {
        b.iter(|| black_box(chapter_count()));
    });

    let phrase = || {
        doc.find()
            .in_span("books", "JHN")
            .with_text_folded(PHRASE)
            .get_spans("verses")
            .unwrap()
    };
    assert_eq!(phrase().len(), 1);
    group.bench_function("scoped_folded_phrase_to_verses", |b| {
        b.iter(|| black_box(phrase()))
    });

    let full_phrase = || {
        doc.find()
            .with_text_folded(PHRASE)
            .get_spans("verses")
            .unwrap()
    };
    let fold = || doc.find().with_text_folded(PHRASE).get_selection().unwrap();
    let hits = fold();
    let verses_from_hits = || doc.find().within(&hits).get_spans("verses").unwrap();
    let verses = verses_from_hits();
    assert_eq!(hits.ranges().len(), 1);
    assert_eq!(verses.len(), 1);
    assert!(
        full_phrase()
            .iter()
            .map(Span::id)
            .eq(verses.iter().map(Span::id))
    );
    group.bench_function("full_folded_phrase_selection", |b| {
        b.iter(|| black_box(fold()))
    });
    group.bench_function("verses_from_full_phrase_hits", |b| {
        b.iter(|| black_box(verses_from_hits()))
    });
    group.bench_function("full_folded_phrase_to_verses", |b| {
        b.iter(|| black_box(full_phrase()))
    });
    group.throughput(Throughput::Bytes(u64::from(doc.len())));
    group.bench_function("load_and_convert", |b| {
        b.iter_batched(
            || (),
            |()| black_box(bsb_usj::load().unwrap()),
            BatchSize::PerIteration,
        );
    });

    group.finish();
}

criterion_group!(benches, real_bible);
criterion_main!(benches);
