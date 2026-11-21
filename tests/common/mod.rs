#![allow(dead_code)]

use std::{
    fmt,
    ops::Range,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use klause::{Document, SourceError, Span, SpanSource, Target};

/// The text of [`genesis`].
pub const GENESIS: &str =
    "In the beginning God created the heavens and the earth. Now the earth was formless and void.";

/// Genesis 1:1–2 with books, chapters, verses, words, morphology, and a footnote.
///
/// - `words` are numbered 1 to 17 in text order.
/// - `morph` targets every word with `pos`, and every word except articles with `strongs`.
/// - Verse 1 includes the space after it, so the verses meet. Verses have a `number`.
/// - `notes` has one point, right after the full stop of verse 1.
///
/// Part of speech counts: noun 7, art 4, conj 2, verb 2, adv 1, prep 1.
pub fn genesis() -> Document {
    type Word = (
        &'static str,
        &'static str,
        Option<&'static str>,
        &'static str,
    );
    let verses: [(&str, i64, &[Word]); 2] = [
        (
            "GEN 1:1",
            1,
            &[
                ("In", "prep", Some("H9002"), " "),
                ("the", "art", None, " "),
                ("beginning", "noun", Some("H7225"), " "),
                ("God", "noun", Some("H430"), " "),
                ("created", "verb", Some("H1254"), " "),
                ("the", "art", None, " "),
                ("heavens", "noun", Some("H8064"), " "),
                ("and", "conj", Some("H853"), " "),
                ("the", "art", None, " "),
                ("earth", "noun", Some("H776"), ". "),
            ],
        ),
        (
            "GEN 1:2",
            2,
            &[
                ("Now", "adv", Some("H1961a"), " "),
                ("the", "art", None, " "),
                ("earth", "noun", Some("H776"), " "),
                ("was", "verb", Some("H1961"), " "),
                ("formless", "noun", Some("H8414"), " "),
                ("and", "conj", Some("H853"), " "),
                ("void", "noun", Some("H922"), "."),
            ],
        ),
    ];

    let mut doc = Document::new("BSB")
        .with_layer("books")
        .with_layer("chapters")
        .with_layer("verses")
        .with_layer("words")
        .with_layer("morph")
        .with_layer("notes");
    let mut word_id = 0_u64;
    for (verse_id, number, words) in verses {
        let start = doc.len();
        for &(text, pos, strongs, after) in words {
            word_id += 1;
            doc.push_span("words", text, |span| span.with_id(word_id))
                .unwrap();
            doc.push(after).unwrap();
            let mut tag = Span::on("words", word_id).with_attr("pos", pos);
            if let Some(strongs) = strongs {
                tag = tag.with_attr("strongs", strongs);
            }
            doc.add_span("morph", tag).unwrap();
        }
        doc.add_span(
            "verses",
            Span::from(start..doc.len())
                .with_id(verse_id)
                .with_attr("number", number),
        )
        .unwrap();
    }
    doc.add_span("chapters", Span::from(0..doc.len()).with_id("GEN 1"))
        .unwrap();
    doc.add_span("books", Span::from(0..doc.len()).with_id("GEN"))
        .unwrap();
    let note = at(".", 0).end;
    doc.add_span(
        "notes",
        Span::from(note..note).with_attr("body", "Or the land"),
    )
    .unwrap();
    doc
}

/// The range of the `nth` (from 0) occurrence of `needle` in [`GENESIS`].
pub fn at(needle: &str, nth: usize) -> Range<u32> {
    let start = GENESIS
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("{needle:?} occurs fewer than {} times", nth + 1))
        .0 as u32;
    start..start + needle.len() as u32
}

/// The range of verse 1 (including its trailing space) and verse 2 in [`GENESIS`].
pub fn verse(number: usize) -> Range<u32> {
    let split = at("Now", 0).start;
    match number {
        1 => 0..split,
        2 => split..GENESIS.len() as u32,
        _ => panic!("no verse {number}"),
    }
}

/// An application database of highlights, shared with the test so it can change between reads.
#[derive(Clone, Default)]
pub struct Highlights {
    pub rows: Arc<Mutex<Vec<Span>>>,
    pub down: Arc<AtomicBool>,
}

impl Highlights {
    pub fn save(&self, span: Span) {
        self.rows.lock().unwrap().push(span);
    }
}

#[derive(Debug)]
struct DatabaseDown;

impl fmt::Display for DatabaseDown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("database is down")
    }
}

impl std::error::Error for DatabaseDown {}

impl SpanSource for Highlights {
    fn get_spans(&self, range: Range<u32>) -> Result<Vec<Span>, SourceError> {
        if self.down.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SourceError::new(DatabaseDown));
        }
        let touches = |text: &Range<u32>| {
            if text.is_empty() {
                range.start <= text.start && text.start <= range.end
            } else {
                text.start < range.end && range.start < text.end
            }
        };
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|span| match span.target() {
                Target::Text(ranges) => ranges.iter().any(touches),
                // Klause resolves targets; a database of highlights can't.
                Target::Span(_) => true,
            })
            .cloned()
            .collect())
    }
}
