//! Test-only reader for the supplied BSB USJ export, not a general USJ importer.

use std::{error::Error, fs, ops::Range, path::Path};

use klause::{Document, Span};
use serde::Deserialize;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BOOKS: &str = "    GEN EXO LEV NUM DEU JOS JDG RUT 1SA 2SA 1KI 2KI 1CH 2CH EZR NEH EST     JOB PSA PRO ECC SNG ISA JER LAM EZK DAN HOS JOL AMO OBA JON MIC NAM HAB ZEP HAG ZEC MAL     MAT MRK LUK JHN ACT ROM 1CO 2CO GAL EPH PHP COL 1TH 2TH 1TI 2TI TIT PHM HEB     JAS 1PE 2PE 1JN 2JN 3JN JUD REV";

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[serde(rename = "USJ")]
    Usj,
    Book,
    Chapter,
    Verse,
    Para,
    Char,
    Note,
    Ref,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Part {
    Text(String),
    Node(Node),
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "type")]
    kind: Kind,
    #[serde(default)]
    marker: String,
    #[serde(default)]
    content: Vec<Part>,
    code: Option<String>,
    number: Option<String>,
    loc: Option<String>,
}

impl Node {
    fn number(&self) -> Result<i64> {
        Ok(self
            .number
            .as_deref()
            .ok_or("missing milestone number")?
            .parse()?)
    }
}

/// Loads the 66 fixture files in canonical order into one searchable document.
pub fn load() -> Result<Document> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bsb_usj");
    let mut doc = Document::new("BSB").with_attr("title", "Berean Standard Bible");
    for layer in ["books", "chapters", "verses", "structure", "notes", "words"] {
        doc.add_layer(layer)?;
    }
    for code in BOOKS.split_whitespace() {
        let path = dir.join(format!("{code}.usj"));
        let loaded = (|| -> Result<()> {
            let root: Node = serde_json::from_slice(&fs::read(&path)?)?;
            if !matches!(root.kind, Kind::Usj)
                || !matches!(root.content.first(), Some(Part::Node(book))
                    if matches!(book.kind, Kind::Book) && book.code.as_deref() == Some(code))
            {
                return Err("unexpected USJ root or book code".into());
            }
            Book::new(&mut doc, code).read(&root.content)
        })();
        loaded.map_err(|err| format!("{}: {err}", path.display()))?;
    }
    add_words(&mut doc)?;
    Ok(doc)
}

struct Book<'a> {
    doc: &'a mut Document,
    code: &'a str,
    chapter: Option<(i64, u32)>,
    verse: Option<(i64, u32)>,
    separator: &'static str,
}

impl<'a> Book<'a> {
    fn new(doc: &'a mut Document, code: &'a str) -> Self {
        Self {
            doc,
            code,
            chapter: None,
            verse: None,
            separator: "\n",
        }
    }

    fn read(mut self, content: &[Part]) -> Result<()> {
        let start = self.doc.len();
        self.walk(content)?;
        self.end_chapter()?;
        self.doc.add_span(
            "books",
            Span::from(self.range_since(start)).with_id(self.code),
        )?;
        Ok(())
    }

    fn walk(&mut self, content: &[Part]) -> Result<()> {
        for part in content {
            let node = match part {
                Part::Text(text) => {
                    self.append(text)?;
                    continue;
                }
                Part::Node(node) => node,
            };
            match node.kind {
                Kind::Chapter => {
                    self.end_chapter()?;
                    self.chapter = Some((node.number()?, self.doc.len()));
                    self.separator = "\n";
                }
                Kind::Verse => {
                    self.end_verse()?;
                    if self.chapter.is_none() {
                        return Err("verse before a chapter".into());
                    }
                    self.verse = Some((node.number()?, self.doc.len()));
                    self.space();
                }
                Kind::Para if matches!(
                    node.marker.as_str(),
                    "h" | "toc1" | "toc2" | "toc3" | "mt1" | "mt2" | "ms" | "mr" | "s1" | "s2" | "r"
                ) => {}
                // Unnumbered Psalm division labels aren't verse text. Numbered
                // superscriptions are real verses and must be traversed.
                Kind::Para if node.marker == "d" && !node.content.iter().any(|part| {
                    matches!(part, Part::Node(child) if matches!(child.kind, Kind::Verse))
                }) => {}
                Kind::Para | Kind::Char | Kind::Ref => {
                    let start = self.doc.len();
                    if matches!(node.kind, Kind::Para) {
                        self.separator = "\n";
                    }
                    self.walk(&node.content)?;
                    let range = self.range_since(start);
                    if !range.is_empty() && !matches!(node.kind, Kind::Ref) {
                        let kind = if matches!(node.kind, Kind::Para) { "para" } else { "char" };
                        self.doc.add_span(
                            "structure",
                            Span::from(range)
                                .with_attr("type", kind)
                                .with_attr("marker", node.marker.as_str()),
                        )?;
                    }
                }
                Kind::Note => self.note(node)?,
                Kind::Book => {}
                Kind::Usj => return Err("unexpected nested USJ root".into()),
            }
        }
        Ok(())
    }

    fn end_verse(&mut self) -> Result<()> {
        if let Some((number, start)) = self.verse.take() {
            let (chapter, _) = self.chapter.ok_or("verse before a chapter")?;
            self.doc.add_span(
                "verses",
                Span::from(self.range_since(start))
                    .with_id(format!("{} {chapter}:{number}", self.code).as_str())
                    .with_attr("book", self.code)
                    .with_attr("chapter", chapter)
                    .with_attr("number", number),
            )?;
        }
        Ok(())
    }

    fn end_chapter(&mut self) -> Result<()> {
        self.end_verse()?;
        if let Some((number, start)) = self.chapter.take() {
            self.doc.add_span(
                "chapters",
                Span::from(self.range_since(start))
                    .with_id(format!("{} {number}", self.code).as_str())
                    .with_attr("book", self.code)
                    .with_attr("number", number),
            )?;
        }
        Ok(())
    }

    fn note(&mut self, node: &Node) -> Result<()> {
        if let (Some((chapter, _)), Some((verse, _))) = (self.chapter, self.verse) {
            let mut body = String::new();
            let mut refs = Vec::new();
            note_text(&node.content, &mut body, &mut refs);
            let at = self.doc.len();
            let mut span = Span::from(at..at)
                .with_attr("marker", node.marker.as_str())
                .with_attr("body", body.trim())
                .with_attr("verse", format!("{} {chapter}:{verse}", self.code));
            if !refs.is_empty() {
                span = span.with_attr("to", refs);
            }
            self.doc.add_span("notes", span)?;
        }
        Ok(())
    }

    fn space(&mut self) {
        if self.separator.is_empty() {
            self.separator = " ";
        }
    }

    // Defer whitespace so notes and verse ends stay before the next separator.
    fn append(&mut self, text: &str) -> Result<()> {
        if self.verse.is_none() {
            return Ok(());
        }
        if text.starts_with(char::is_whitespace) {
            self.space();
        }
        let body = text.trim();
        if !body.is_empty() {
            if self.doc.len() != 0 && !self.separator.is_empty() {
                self.doc.push(self.separator)?;
            }
            self.separator = "";
            self.doc.push(body)?;
        }
        if text.ends_with(char::is_whitespace) {
            self.space();
        }
        Ok(())
    }

    fn range_since(&self, start: u32) -> Range<u32> {
        let tail = &self.doc.text()[start as usize..];
        self.doc.len() - tail.trim_start().len() as u32..self.doc.len()
    }
}

fn note_text(content: &[Part], body: &mut String, refs: &mut Vec<String>) {
    for part in content {
        match part {
            Part::Text(text) => body.push_str(text),
            Part::Node(node) if node.marker == "fr" => {}
            Part::Node(node) => {
                if let Some(loc) = &node.loc {
                    refs.push(loc.clone());
                }
                note_text(&node.content, body, refs);
            }
        }
    }
}

// Tokenize the normalized Document text so word offsets are UTF-8 byte offsets.
// For this workload a word is a contiguous run of Unicode alphanumerics.
fn add_words(doc: &mut Document) -> Result<()> {
    let text = doc.text().to_owned();
    let mut start = None;
    for (at, ch) in text.char_indices().chain([(text.len(), ' ')]) {
        if ch.is_alphanumeric() {
            start.get_or_insert(at);
        } else if let Some(start) = start.take() {
            doc.add_span(
                "words",
                Span::from(start as u32..at as u32)
                    .with_attr("token", text[start..at].to_lowercase()),
            )?;
        }
    }
    Ok(())
}
