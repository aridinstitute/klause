use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use klause::{Document, Span};

// 18 bytes per row; the largest input is about 4.5 MB and 750,000 words.
const WORDS: [&str; 3] = ["In", "the", "beginning."];
const ROW: &str = "In the beginning.\n";
const ROW_COUNTS: [usize; 4] = [1, 100, 10_000, 250_000];

fn document_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("document_creation");

    for rows in ROW_COUNTS {
        let text = ROW.repeat(rows);
        let bytes = text.len();
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::new("plain_text", bytes), &text, |b, text| {
            b.iter_batched(
                || text.as_str(),
                |text| {
                    let mut doc = Document::new("benchmark");
                    doc.push(black_box(text)).unwrap();
                    doc
                },
                // Keep destruction out of the timing without retaining many documents.
                BatchSize::PerIteration,
            );
        });

        group.bench_with_input(
            BenchmarkId::new("annotated_rows", bytes),
            &rows,
            |b, &rows| {
                b.iter_batched(
                    || rows,
                    |rows| {
                        let mut doc = Document::new("benchmark")
                            .with_layer("words")
                            .with_layer("verses");

                        for _ in 0..rows {
                            let start = doc.len();
                            for (idx, word) in WORDS.into_iter().enumerate() {
                                if idx > 0 {
                                    doc.push(" ").unwrap();
                                }
                                doc.push_span("words", word, |span| span).unwrap();
                            }
                            doc.push("\n").unwrap();
                            doc.add_span("verses", Span::from(start..doc.len()))
                                .unwrap();
                        }

                        doc
                    },
                    BatchSize::PerIteration,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(benches, document_creation);
criterion_main!(benches);
