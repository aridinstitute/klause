use std::ops::Range;

use klause::{Document, Layer, SourceError, Span, SpanSource};

pub const BOOKS: usize = 66;
pub const CHAPTERS: usize = 1_189;
pub const VERSES: usize = 31_102;
pub const WORDS: usize = 790_000;
pub const STRUCTURE: usize = 50_000;
pub const NOTES: usize = 20_000;
pub const XREFS: usize = 340_000;
pub const HIGHLIGHTS: usize = 10_000;
pub const STRONGS: usize = 5_600;
pub const CHAPTER: u64 = (CHAPTERS / 2) as u64;
pub const PHRASE: &str = "alpha beta gamma";
const PHRASE_WORDS: [&str; 3] = ["Álpha", "béta", "gámma"];
const PHRASE_PERIOD: usize = 4_096;
pub const PHRASE_MATCHES: usize = VERSES.div_ceil(PHRASE_PERIOD);
const WORD_BANK: [&str; 16] = [
    "Light", "over", "water", "and", "earth", "voice", "rose", "again", "from", "dust", "stone",
    "river", "bread", "under", "fields", "peace",
];

// Prepared in memory before timing. The build still appends text and inserts every annotation.
pub struct Rows {
    text: String,
    words: Vec<Range<u32>>,
    verses: Vec<Range<u32>>,
    verse_ids: Vec<String>,
    strongs: Vec<String>,
}

impl Rows {
    pub fn new() -> Self {
        let mut text = String::with_capacity(4_500_000);
        let mut words = Vec::with_capacity(WORDS);
        let mut verses = Vec::with_capacity(VERSES);
        for verse in 0..VERSES {
            if verse != 0 {
                text.push('\n');
            }
            let start = text.len() as u32;
            for (local, word) in partition(verse, VERSES, WORDS).enumerate() {
                if local != 0 {
                    text.push(' ');
                }
                let token = if verse % PHRASE_PERIOD == 0 && (10..13).contains(&local) {
                    PHRASE_WORDS[local - 10]
                } else {
                    WORD_BANK[(word * 7 + verse) % WORD_BANK.len()]
                };
                let start = text.len() as u32;
                text.push_str(token);
                words.push(start..text.len() as u32);
            }
            verses.push(start..text.len() as u32);
        }
        Self {
            text,
            words,
            verses,
            verse_ids: (0..VERSES).map(|i| format!("verse:{i}")).collect(),
            strongs: (1..=STRONGS).map(|i| format!("H{i}")).collect(),
        }
    }

    pub fn xref_destination(&self) -> &str {
        &self.verse_ids[VERSES / 2]
    }

    pub fn build(&self) -> Document {
        let mut doc = Document::new("synthetic-bible");
        for layer in [
            "books",
            "chapters",
            "verses",
            "words",
            "morph",
            "structure",
            "notes",
            "xrefs",
        ] {
            doc.add_layer(layer).unwrap();
        }
        doc.push(&self.text).unwrap();
        for book in 0..BOOKS {
            let chapters = partition(book, BOOKS, CHAPTERS);
            let first = partition(chapters.start, CHAPTERS, VERSES).start;
            let end = partition(chapters.end - 1, CHAPTERS, VERSES).end;
            doc.add_span(
                "books",
                Span::from(cover(&self.verses, first..end)).with_id(book as u64),
            )
            .unwrap();
        }
        for chapter in 0..CHAPTERS {
            let verses = partition(chapter, CHAPTERS, VERSES);
            doc.add_span(
                "chapters",
                Span::from(cover(&self.verses, verses)).with_id(chapter as u64),
            )
            .unwrap();
        }
        let extra_lines = STRUCTURE - VERSES;
        for (verse, range) in self.verses.iter().enumerate() {
            doc.add_span(
                "verses",
                Span::from(range.clone()).with_id(self.verse_ids[verse].as_str()),
            )
            .unwrap();
            let mut words = partition(verse, VERSES, WORDS);
            if !partition(verse, VERSES, extra_lines).is_empty() {
                let middle = (words.start + words.end) / 2;
                doc.add_span(
                    "structure",
                    line(cover(&self.words, words.start..middle), 1),
                )
                .unwrap();
                words.start = middle;
            }
            doc.add_span("structure", line(cover(&self.words, words), 0))
                .unwrap();
        }
        for (word, range) in self.words.iter().enumerate() {
            let id = word as u64;
            doc.add_span("words", Span::from(range.clone()).with_id(id))
                .unwrap();
            let pos = match word % 8 {
                0 => "verb",
                1..=3 => "noun",
                4 => "prep",
                _ => "other",
            };
            // Half the words draw from 128 common IDs, half from the whole vocabulary.
            let strong = (word / 2) % if word % 2 == 0 { 128 } else { STRONGS };
            doc.add_span(
                "morph",
                Span::on("words", id)
                    .with_attr("pos", pos)
                    .with_attr("strongs", self.strongs[strong].as_str()),
            )
            .unwrap();
        }
        for i in 0..NOTES {
            let at = self.verses[i * VERSES / NOTES].end;
            doc.add_span(
                "notes",
                Span::from(at..at).with_attr("body", "An alternate reading."),
            )
            .unwrap();
        }
        for i in 0..XREFS {
            let origin = i % VERSES;
            let destination = (origin + 1 + i / VERSES) % VERSES;
            doc.add_span(
                "xrefs",
                Span::on("verses", self.verse_ids[origin].as_str())
                    .with_attr("to", self.verse_ids[destination].as_str()),
            )
            .unwrap();
        }
        let ranges = (0..HIGHLIGHTS)
            .map(|i| self.words[partition(i, HIGHLIGHTS, WORDS).start].clone())
            .collect();
        doc.add_layer(Layer::source("highlights", Highlights(ranges)))
            .unwrap();
        doc
    }
}

fn partition(i: usize, groups: usize, items: usize) -> Range<usize> {
    let offset = |i: usize| (i as u64 * items as u64 / groups as u64) as usize;
    offset(i)..offset(i + 1)
}

fn cover(ranges: &[Range<u32>], indices: Range<usize>) -> Range<u32> {
    ranges[indices.start].start..ranges[indices.end - 1].end
}

fn line(range: Range<u32>, indent: i32) -> Span {
    Span::from(range)
        .with_attr("type", "line")
        .with_attr("indent", indent)
}

struct Highlights(Vec<Range<u32>>);

impl SpanSource for Highlights {
    fn get_spans(&self, range: Range<u32>) -> Result<Vec<Span>, SourceError> {
        if range.is_empty() {
            return Ok(Vec::new());
        }
        let first = self.0.partition_point(|r| r.end <= range.start);
        Ok(self
            .0
            .iter()
            .enumerate()
            .skip(first)
            .take_while(|(_, r)| r.start < range.end)
            .map(|(id, r)| {
                Span::from(r.clone())
                    .with_id(id as u64)
                    .with_attr("color", "yellow")
            })
            .collect())
    }
}
