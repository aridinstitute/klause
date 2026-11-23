use std::{
    hint::black_box,
    process::{self, Command},
    time::{Duration, Instant},
};

use criterion::{BatchSize, Criterion};
use klause::{Document, Passage, Selection, Span, span_id, spans_in};

#[path = "bible_workload/fixture.rs"]
mod fixture;
use fixture::{
    BOOKS, CHAPTER, CHAPTERS, HIGHLIGHTS, NOTES, PHRASE, PHRASE_MATCHES, Rows, STRONGS, STRUCTURE,
    VERSES, WORDS, XREFS,
};

const OVERLAY_SPANS: usize = 10_000;

fn insert_overlay(doc: &mut Document) {
    for i in 0..OVERLAY_SPANS {
        let start = (i * (WORDS - 2) / OVERLAY_SPANS) as u64;
        doc.add_span(
            "overlay",
            Span::between(span_id("words", start), span_id("words", start + 2))
                .with_attr("color", "yellow"),
        )
        .unwrap();
    }
}
fn chapter(doc: &Document) -> Passage {
    doc.get_passage(span_id("chapters", CHAPTER)).unwrap()
}

fn verbs(doc: &Document) -> Selection {
    doc.find()
        .within(spans_in("morph").with_attr("pos", "verb"))
        .get_selection()
        .unwrap()
}

fn reverse_xrefs(doc: &Document, destination: &str) -> Vec<Span> {
    doc.find()
        .within(spans_in("xrefs").with_attr("to", destination.to_owned()))
        .get_spans("verses")
        .unwrap()
}

fn bible_workload(c: &mut Criterion) {
    let rows = Rows::new();
    let destination = rows.xref_destination();
    let mut group = c.benchmark_group("bible_workload");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("build_through_first_chapter", |b| {
        b.iter_batched(
            || (),
            |()| {
                let doc = rows.build();
                drop(black_box(chapter(&doc)));
                black_box(doc)
            },
            BatchSize::PerIteration,
        );
    });
    // Each cold sample uses a new document; building and dropping it are untimed.
    group.bench_function("chapter_cold", |b| {
        b.iter_batched_ref(
            || rows.build(),
            |doc| drop(black_box(chapter(doc))),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("verbs_cold", |b| {
        b.iter_batched_ref(
            || rows.build(),
            |doc| drop(black_box(verbs(doc))),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("reverse_xrefs_cold", |b| {
        b.iter_batched_ref(
            || rows.build(),
            |doc| drop(black_box(reverse_xrefs(doc, destination))),
            BatchSize::PerIteration,
        );
    });

    let doc = rows.build();
    assert!((4_490_000..4_510_000).contains(&doc.len()));
    let passage = chapter(&doc);
    for (layer, count) in [
        ("books", BOOKS),
        ("chapters", CHAPTERS),
        ("verses", VERSES),
        ("words", WORDS),
        ("morph", WORDS),
        ("structure", STRUCTURE),
        ("notes", NOTES),
        ("xrefs", XREFS),
    ] {
        assert_eq!(doc.layer(layer).unwrap().spans().count(), count, "{layer}");
        assert!(passage.spans_in(layer).next().is_some(), "{layer}");
    }
    assert!(passage.spans_in("highlights").next().is_some());
    assert_eq!(doc.find().count_spans("highlights").unwrap(), HIGHLIGHTS);
    drop(passage);

    let count_strongs = || doc.find().count_values("morph", "strongs").unwrap();
    let counts = count_strongs();
    assert_eq!(counts.len(), STRONGS);
    assert_eq!(counts.iter().map(|(_, count)| count).sum::<usize>(), WORDS);
    assert_eq!(verbs(&doc).ranges().len(), WORDS / 8);
    assert_eq!(reverse_xrefs(&doc, destination).len(), 11);

    let fold = || doc.find().with_text_folded(PHRASE).get_selection().unwrap();
    let combined = || {
        doc.find()
            .with_text_folded(PHRASE)
            .get_spans("verses")
            .unwrap()
    };
    let hits = fold();
    let verses_from_hits = || doc.find().within(&hits).get_spans("verses").unwrap();
    let verses = verses_from_hits();
    assert_eq!(hits.ranges().len(), PHRASE_MATCHES);
    assert_eq!(verses.len(), PHRASE_MATCHES);
    assert!(
        combined()
            .iter()
            .map(Span::id)
            .eq(verses.iter().map(Span::id))
    );

    group.bench_function("chapter_warm", |b| b.iter(|| black_box(chapter(&doc))));
    group.bench_function("verbs_warm", |b| b.iter(|| black_box(verbs(&doc))));
    group.bench_function("strongs_counts", |b| b.iter(|| black_box(count_strongs())));
    group.bench_function("reverse_xrefs_warm", |b| {
        b.iter(|| black_box(reverse_xrefs(&doc, destination)))
    });
    group.bench_function("folded_phrase_selection", |b| b.iter(|| black_box(fold())));
    group.bench_function("verses_from_phrase_hits", |b| {
        b.iter(|| black_box(verses_from_hits()))
    });
    group.bench_function("folded_phrase_to_verses", |b| {
        b.iter(|| black_box(combined()))
    });
    let base = doc.freeze_layers();
    group.bench_function("overlay_insert_10000", |b| {
        b.iter_batched_ref(
            || base.clone().with_layer("overlay"),
            insert_overlay,
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

// Sample current process RSS (not peak RSS) without running Criterion or retaining rows.
fn report_rss(label: &str) {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &process::id().to_string()])
        .output()
        .expect("RSS measurement requires ps (macOS or Linux)");
    assert!(
        output.status.success(),
        "ps failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let kib: u64 = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    println!(
        "{label}: {:.1} MB ({kib} KiB)",
        kib as f64 * 1024.0 / 1_000_000.0
    );
}

fn overlay_rss() {
    let base = Rows::new().build().freeze_layers();
    let mut doc = base.clone().with_layer("overlay");
    report_rss("Frozen base and empty overlay");
    let start = Instant::now();
    insert_overlay(&mut doc);
    let elapsed = start.elapsed();
    report_rss("Frozen base and 10,000 overlay spans");
    assert_eq!(doc.layer("overlay").unwrap().spans().count(), OVERLAY_SPANS);
    for layer in base.layers() {
        assert!(
            std::ptr::eq(layer, doc.layer(layer.id()).unwrap()),
            "copied {}",
            layer.id()
        );
    }
    println!("Inserted {OVERLAY_SPANS} spans in {elapsed:.2?}; all base layers still shared");
}
fn main() {
    if std::env::args().any(|arg| arg == "--overlay-rss") {
        overlay_rss();
        return;
    }
    if std::env::args().any(|arg| arg == "--rss") {
        report_rss("Before loading");
        let (doc, destination) = {
            let rows = Rows::new();
            (rows.build(), rows.xref_destination().to_owned())
        };
        report_rss("Loaded, indexes unbuilt");
        drop(black_box(chapter(&doc)));
        drop(black_box(verbs(&doc)));
        drop(black_box(reverse_xrefs(&doc, &destination)));
        report_rss("After chapter, verb and reverse-reference queries");
        return;
    }
    let mut criterion = Criterion::default().configure_from_args();
    bible_workload(&mut criterion);
    criterion.final_summary();
}
